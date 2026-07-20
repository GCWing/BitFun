//! Session metadata file and index IO owner.
//!
//! Workspace-to-sessions-root resolution stays in product assembly. This module
//! owns the provider-neutral metadata file layout under an already resolved
//! sessions root.

use super::layout::SessionStorageLayout;
use super::metadata::{
    build_session_index_snapshot, remove_session_index_entry, upsert_session_index_entry,
};
use super::page::{build_session_metadata_page, empty_session_metadata_page};
use super::types::{SessionMetadata, StoredSessionIndexFile, StoredSessionMetadataFile};
use super::SessionMetadataPage;
use crate::json_store::{JsonFileStore, JsonFileStoreError};
use bitfun_core_types::validate_session_id;
use log::warn;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::fs;
use tokio::sync::Mutex;

static SESSION_INDEX_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();

#[derive(Debug, Error)]
pub enum SessionMetadataStoreError {
    #[error(transparent)]
    Json(#[from] JsonFileStoreError),
    #[error("Failed to read sessions root: {source}")]
    ReadSessionsRoot {
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to read session directory entry: {source}")]
    ReadSessionDirectoryEntry {
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to get file type: {source}")]
    GetFileType {
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to create session directory: {source}")]
    CreateSessionDir {
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to delete session directory: {source}")]
    DeleteSessionDir {
        #[source]
        source: std::io::Error,
    },
    #[error("Invalid session ID: {0}")]
    InvalidSessionId(String),
    #[error("Failed to resolve session storage path {path}: {source}")]
    ResolveSessionStoragePath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Session path escapes the sessions root: path={path}, root={root}")]
    UnsafeSessionStoragePath { path: PathBuf, root: PathBuf },
}

impl SessionMetadataStoreError {
    pub fn is_deserialization(&self) -> bool {
        matches!(self, Self::Json(error) if error.is_deserialization())
    }

    pub fn is_serialization(&self) -> bool {
        matches!(self, Self::Json(error) if error.is_serialization())
    }
}

#[derive(Debug, Clone)]
pub struct SessionMetadataStore {
    layout: SessionStorageLayout,
    json_store: JsonFileStore,
}

impl SessionMetadataStore {
    pub fn new(sessions_root: impl Into<PathBuf>) -> Self {
        Self {
            layout: SessionStorageLayout::new(sessions_root),
            json_store: JsonFileStore,
        }
    }

    pub fn sessions_root(&self) -> &Path {
        self.layout.sessions_root()
    }

    fn index_path(&self) -> PathBuf {
        self.layout.index_path()
    }

    fn session_dir(&self, session_id: &str) -> PathBuf {
        self.layout.session_dir(session_id)
    }

    fn metadata_path(&self, session_id: &str) -> PathBuf {
        self.layout.metadata_path(session_id)
    }

    async fn get_index_lock(&self) -> Arc<Mutex<()>> {
        let index_path = self.index_path();
        let registry = SESSION_INDEX_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut registry_guard = registry.lock().await;
        registry_guard
            .entry(index_path)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    async fn read_json_optional<T: serde::de::DeserializeOwned>(
        &self,
        path: &Path,
    ) -> Result<Option<T>, SessionMetadataStoreError> {
        self.json_store
            .read_optional(path)
            .await
            .map_err(SessionMetadataStoreError::from)
    }

    async fn write_json_atomic<T: serde::Serialize>(
        &self,
        path: &Path,
        value: &T,
    ) -> Result<(), SessionMetadataStoreError> {
        self.json_store
            .write_atomic(path, value)
            .await
            .map_err(SessionMetadataStoreError::from)
    }

    async fn scan_metadata_dirs(&self) -> Result<Vec<SessionMetadata>, SessionMetadataStoreError> {
        if !self.sessions_root().exists() {
            return Ok(Vec::new());
        }

        let mut metadata_list = Vec::new();
        let mut entries = fs::read_dir(self.sessions_root())
            .await
            .map_err(|source| SessionMetadataStoreError::ReadSessionsRoot { source })?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|source| SessionMetadataStoreError::ReadSessionDirectoryEntry { source })?
        {
            let file_type = entry
                .file_type()
                .await
                .map_err(|source| SessionMetadataStoreError::GetFileType { source })?;
            if !file_type.is_dir() {
                continue;
            }

            let session_id = entry.file_name().to_string_lossy().to_string();
            match self.load_metadata(&session_id).await {
                Ok(Some(metadata)) => metadata_list.push(metadata),
                Ok(None) => {}
                Err(error) => {
                    warn!(
                        "Failed to rebuild session index entry: session_id={}, error={}",
                        session_id, error
                    );
                }
            }
        }

        metadata_list.sort_by_key(|metadata| std::cmp::Reverse(metadata.last_active_at));
        Ok(metadata_list)
    }

    async fn count_metadata_dirs(&self) -> Result<usize, SessionMetadataStoreError> {
        if !self.sessions_root().exists() {
            return Ok(0);
        }

        let mut count = 0;
        let mut entries = fs::read_dir(self.sessions_root())
            .await
            .map_err(|source| SessionMetadataStoreError::ReadSessionsRoot { source })?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|source| SessionMetadataStoreError::ReadSessionDirectoryEntry { source })?
        {
            let file_type = entry
                .file_type()
                .await
                .map_err(|source| SessionMetadataStoreError::GetFileType { source })?;
            if !file_type.is_dir() {
                continue;
            }

            let session_id = entry.file_name().to_string_lossy().to_string();
            if self.metadata_path(&session_id).exists() {
                count += 1;
            }
        }

        Ok(count)
    }

    async fn rebuild_index_locked(
        &self,
    ) -> Result<Vec<SessionMetadata>, SessionMetadataStoreError> {
        let metadata_list = self.scan_metadata_dirs().await?;
        let (index, visible_sessions) =
            build_session_index_snapshot(metadata_list, current_unix_ms());
        self.write_json_atomic(&self.index_path(), &index).await?;
        Ok(visible_sessions)
    }

    async fn upsert_index_entry_locked(
        &self,
        metadata: &SessionMetadata,
        metadata_file_created: bool,
    ) -> Result<(), SessionMetadataStoreError> {
        let index_path = self.index_path();
        let existing_index = self
            .read_json_optional::<StoredSessionIndexFile>(&index_path)
            .await?;
        let disk_metadata_file_count = if existing_index.is_some() {
            0
        } else {
            self.count_metadata_dirs().await?
        };
        let index = upsert_session_index_entry(
            existing_index,
            metadata,
            metadata_file_created,
            disk_metadata_file_count,
            current_unix_ms(),
        );
        self.write_json_atomic(&index_path, &index).await
    }

    async fn remove_index_entry_locked(
        &self,
        session_id: &str,
        metadata_file_count_delta: isize,
    ) -> Result<(), SessionMetadataStoreError> {
        let index_path = self.index_path();
        let existing_index = self
            .read_json_optional::<StoredSessionIndexFile>(&index_path)
            .await?;
        let Some(index) = remove_session_index_entry(
            existing_index,
            session_id,
            metadata_file_count_delta,
            current_unix_ms(),
        ) else {
            return Ok(());
        };
        self.write_json_atomic(&index_path, &index).await
    }

    pub async fn list_metadata(&self) -> Result<Vec<SessionMetadata>, SessionMetadataStoreError> {
        if !self.sessions_root().exists() {
            return Ok(Vec::new());
        }

        let lock = self.get_index_lock().await;
        let _guard = lock.lock().await;
        let index_path = self.index_path();
        if let Some(index) = self
            .read_json_optional::<StoredSessionIndexFile>(&index_path)
            .await?
        {
            let has_stale_entry = index
                .sessions
                .iter()
                .any(|metadata| !self.metadata_path(&metadata.session_id).exists());
            if has_stale_entry {
                warn!(
                    "Session index contains stale entries, rebuilding: {}",
                    index_path.display()
                );
                return self.rebuild_index_locked().await;
            }

            let disk_count = self.count_metadata_dirs().await?;
            if index.metadata_file_count != disk_count {
                warn!(
                    "Session index incomplete (index: {}, disk: {}), rebuilding: {}",
                    index.metadata_file_count,
                    disk_count,
                    index_path.display()
                );
                return self.rebuild_index_locked().await;
            }

            return Ok(index.sessions);
        }

        self.rebuild_index_locked().await
    }

    pub async fn list_metadata_page(
        &self,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<SessionMetadataPage, SessionMetadataStoreError> {
        if !self.sessions_root().exists() {
            return Ok(empty_session_metadata_page());
        }

        let limit = limit.max(1);
        let lock = self.get_index_lock().await;
        let _guard = lock.lock().await;
        let index_path = self.index_path();
        let indexed_sessions = if let Some(index) = self
            .read_json_optional::<StoredSessionIndexFile>(&index_path)
            .await?
        {
            if index.metadata_file_count < index.sessions.len() {
                warn!(
                    "Session index has invalid metadata count before page read (index: {}, sessions: {}), rebuilding: {}",
                    index.metadata_file_count,
                    index.sessions.len(),
                    index_path.display()
                );
                self.rebuild_index_locked().await?
            } else {
                index.sessions
            }
        } else {
            self.rebuild_index_locked().await?
        };

        let page = build_session_metadata_page(indexed_sessions, cursor, limit);
        let has_stale_page_entry = page
            .sessions
            .iter()
            .any(|metadata| !self.metadata_path(&metadata.session_id).exists());
        if !has_stale_page_entry {
            return Ok(page);
        }

        warn!(
            "Session index page contains stale entries, rebuilding before page read: {}",
            index_path.display()
        );
        let rebuilt_sessions = self.rebuild_index_locked().await?;
        Ok(build_session_metadata_page(rebuilt_sessions, cursor, limit))
    }

    pub async fn list_metadata_including_internal(
        &self,
    ) -> Result<Vec<SessionMetadata>, SessionMetadataStoreError> {
        self.scan_metadata_dirs().await
    }

    pub async fn rebuild_index(&self) -> Result<Vec<SessionMetadata>, SessionMetadataStoreError> {
        let lock = self.get_index_lock().await;
        let _guard = lock.lock().await;
        self.rebuild_index_locked().await
    }

    pub async fn save_metadata(
        &self,
        metadata: &SessionMetadata,
    ) -> Result<(), SessionMetadataStoreError> {
        validate_session_id(&metadata.session_id)
            .map_err(SessionMetadataStoreError::InvalidSessionId)?;
        self.ensure_session_dir(&metadata.session_id).await?;
        let metadata_path = self.metadata_path(&metadata.session_id);
        let file = StoredSessionMetadataFile::new(metadata.clone());

        let lock = self.get_index_lock().await;
        let _guard = lock.lock().await;
        let metadata_file_created = !metadata_path.exists();
        self.write_json_atomic(&metadata_path, &file).await?;
        if !metadata.should_hide_from_user_lists() {
            self.upsert_index_entry_locked(metadata, metadata_file_created)
                .await
        } else {
            self.remove_index_entry_locked(
                &metadata.session_id,
                if metadata_file_created { 1 } else { 0 },
            )
            .await
        }
    }

    pub async fn load_metadata(
        &self,
        session_id: &str,
    ) -> Result<Option<SessionMetadata>, SessionMetadataStoreError> {
        validate_session_id(session_id).map_err(SessionMetadataStoreError::InvalidSessionId)?;
        let path = self.metadata_path(session_id);
        Ok(self
            .read_json_optional::<StoredSessionMetadataFile>(&path)
            .await?
            .map(|file| file.metadata))
    }

    pub async fn delete_session_dir_and_index(
        &self,
        session_id: &str,
    ) -> Result<(), SessionMetadataStoreError> {
        validate_session_id(session_id).map_err(SessionMetadataStoreError::InvalidSessionId)?;
        let lock = self.get_index_lock().await;
        let _guard = lock.lock().await;
        let dir = self.session_dir(session_id);
        let metadata_file_removed = self.metadata_path(session_id).exists();
        if dir.exists() {
            let root = fs::canonicalize(self.sessions_root())
                .await
                .map_err(
                    |source| SessionMetadataStoreError::ResolveSessionStoragePath {
                        path: self.sessions_root().to_path_buf(),
                        source,
                    },
                )?;
            let resolved_dir = fs::canonicalize(&dir).await.map_err(|source| {
                SessionMetadataStoreError::ResolveSessionStoragePath {
                    path: dir.clone(),
                    source,
                }
            })?;
            if resolved_dir == root || !resolved_dir.starts_with(&root) {
                return Err(SessionMetadataStoreError::UnsafeSessionStoragePath {
                    path: resolved_dir,
                    root,
                });
            }
            fs::remove_dir_all(&dir)
                .await
                .map_err(|source| SessionMetadataStoreError::DeleteSessionDir { source })?;
        }

        self.remove_index_entry_locked(session_id, if metadata_file_removed { -1 } else { 0 })
            .await
    }

    async fn ensure_session_dir(
        &self,
        session_id: &str,
    ) -> Result<PathBuf, SessionMetadataStoreError> {
        let dir = self.session_dir(session_id);
        fs::create_dir_all(&dir)
            .await
            .map_err(|source| SessionMetadataStoreError::CreateSessionDir { source })?;
        Ok(dir)
    }
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{SessionStatus, StoredSessionIndexFile};
    use tempfile::tempdir;

    fn metadata(session_id: &str, last_active_at: u64) -> SessionMetadata {
        let mut metadata = SessionMetadata::new(
            session_id.to_string(),
            format!("Session {session_id}"),
            "agentic".to_string(),
            "model".to_string(),
        );
        metadata.last_active_at = last_active_at;
        metadata
    }

    #[tokio::test]
    async fn metadata_store_saves_visible_metadata_and_updates_index() {
        let dir = tempdir().expect("tempdir");
        let store = SessionMetadataStore::new(dir.path());

        store
            .save_metadata(&metadata("session-a", 10))
            .await
            .expect("save metadata");

        let loaded = store
            .load_metadata("session-a")
            .await
            .expect("load metadata")
            .expect("metadata exists");
        assert_eq!(loaded.session_id, "session-a");

        let listed = store.list_metadata().await.expect("list metadata");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].session_id, "session-a");
    }

    #[tokio::test]
    async fn metadata_store_rebuilds_stale_index_entries() {
        let dir = tempdir().expect("tempdir");
        let store = SessionMetadataStore::new(dir.path());
        store
            .save_metadata(&metadata("existing", 20))
            .await
            .expect("save metadata");

        let stale = StoredSessionIndexFile {
            schema_version: super::super::types::SESSION_STORAGE_SCHEMA_VERSION,
            metadata_file_count: 2,
            updated_at: 1,
            sessions: vec![metadata("missing", 30), metadata("existing", 20)],
        };
        store
            .write_json_atomic(&store.index_path(), &stale)
            .await
            .expect("write stale index");

        let listed = store.list_metadata().await.expect("list metadata");
        assert_eq!(
            listed
                .iter()
                .map(|value| value.session_id.as_str())
                .collect::<Vec<_>>(),
            vec!["existing"]
        );
    }

    #[tokio::test]
    async fn metadata_store_rebuild_index_counts_hidden_metadata_files() {
        let dir = tempdir().expect("tempdir");
        let store = SessionMetadataStore::new(dir.path());

        store
            .save_metadata(&metadata("visible-a", 20))
            .await
            .expect("save visible metadata");

        let mut hidden = metadata("hidden", 30);
        hidden.session_kind = bitfun_core_types::SessionKind::Subagent;
        store
            .save_metadata(&hidden)
            .await
            .expect("save hidden metadata");

        let visible = store.rebuild_index().await.expect("rebuild index");
        assert_eq!(
            visible
                .iter()
                .map(|value| value.session_id.as_str())
                .collect::<Vec<_>>(),
            vec!["visible-a"]
        );

        let index = store
            .read_json_optional::<StoredSessionIndexFile>(&store.index_path())
            .await
            .expect("read index")
            .expect("index exists");
        assert_eq!(index.sessions.len(), 1);
        assert_eq!(index.metadata_file_count, 2);
    }

    #[tokio::test]
    async fn metadata_store_hides_internal_sessions_from_visible_index() {
        let dir = tempdir().expect("tempdir");
        let store = SessionMetadataStore::new(dir.path());
        let mut hidden = metadata("hidden", 30);
        hidden.session_kind = bitfun_core_types::SessionKind::Subagent;
        hidden.status = SessionStatus::Active;
        hidden.relationship = Some(crate::session::SessionRelationship {
            kind: Some(crate::session::SessionRelationshipKind::Subagent),
            parent_session_id: Some("parent".to_string()),
            ..Default::default()
        });

        store
            .save_metadata(&hidden)
            .await
            .expect("save hidden metadata");

        assert!(store
            .list_metadata()
            .await
            .expect("visible list")
            .is_empty());
        assert_eq!(
            store
                .list_metadata_including_internal()
                .await
                .expect("all metadata")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn metadata_store_delete_session_updates_visible_index() {
        let dir = tempdir().expect("tempdir");
        let store = SessionMetadataStore::new(dir.path());
        store
            .save_metadata(&metadata("session-a", 10))
            .await
            .expect("save metadata");
        assert_eq!(
            store
                .list_metadata()
                .await
                .expect("list before delete")
                .len(),
            1
        );

        store
            .delete_session_dir_and_index("session-a")
            .await
            .expect("delete session");

        assert!(store
            .load_metadata("session-a")
            .await
            .expect("load")
            .is_none());
        assert!(store
            .list_metadata()
            .await
            .expect("list after delete")
            .is_empty());
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn metadata_store_preserves_existing_non_traversing_component_ids() {
        let dir = tempdir().expect("tempdir");
        let store = SessionMetadataStore::new(dir.path());
        let session_id = "legacy:session:1";

        store
            .save_metadata(&metadata(session_id, 10))
            .await
            .expect("save legacy metadata");
        assert!(store
            .load_metadata(session_id)
            .await
            .expect("load legacy metadata")
            .is_some());
        store
            .delete_session_dir_and_index(session_id)
            .await
            .expect("delete legacy session");
        assert!(!dir.path().join(session_id).exists());
    }

    #[tokio::test]
    async fn metadata_store_rejects_session_delete_path_traversal() {
        let parent = tempdir().expect("parent tempdir");
        let sessions_root = parent.path().join("sessions");
        std::fs::create_dir_all(&sessions_root).expect("sessions root");
        let sentinel = parent.path().join("sentinel");
        std::fs::create_dir_all(&sentinel).expect("sentinel");
        std::fs::write(sentinel.join("keep.txt"), "keep").expect("sentinel file");
        let store = SessionMetadataStore::new(&sessions_root);

        for unsafe_id in ["..", "../sentinel", "C:\\sentinel"] {
            assert!(
                store.delete_session_dir_and_index(unsafe_id).await.is_err(),
                "unsafe session id must fail: {unsafe_id}"
            );
        }

        assert_eq!(
            std::fs::read_to_string(sentinel.join("keep.txt")).expect("sentinel remains"),
            "keep"
        );
    }

    #[tokio::test]
    async fn metadata_store_rejects_path_like_ids_for_reads_and_writes() {
        let dir = tempdir().expect("tempdir");
        let store = SessionMetadataStore::new(dir.path());

        assert!(store.load_metadata("../outside").await.is_err());
        assert!(store
            .save_metadata(&metadata("../outside", 10))
            .await
            .is_err());
        assert!(!dir
            .path()
            .parent()
            .expect("parent")
            .join("outside")
            .exists());
    }
}
