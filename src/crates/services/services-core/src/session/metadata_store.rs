//! Session metadata file and index IO owner.
//!
//! Workspace-to-sessions-root resolution stays in product assembly. This module
//! owns the provider-neutral metadata file layout under an already resolved
//! sessions root.

use super::layout::SessionStorageLayout;
use super::metadata::{
    build_session_index_snapshot, remove_session_index_entry, upsert_session_index_entry,
};
use super::page::{
    build_session_metadata_page, build_session_metadata_page_with_options,
    empty_session_metadata_page,
};
use super::types::{SessionMetadata, StoredSessionIndexFile, StoredSessionMetadataFile};
use super::SessionMetadataPage;
use crate::file_lock::{FileLock, FileLockError, FileLockMode};
use crate::json_store::{JsonFileStore, JsonFileStoreError};
use bitfun_core_types::validate_session_id;
use log::{error, warn};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::fs;
use tokio::sync::Mutex;

static SESSION_INDEX_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();

/// How many times `remove_dir_all` is retried when deleting a session
/// directory. Windows can transiently hold file handles (antivirus scan,
/// delayed close) that make an immediate deletion fail; the retries absorb
/// that window instead of losing the deletion.
const RETRY_REMOVE_DIR_ATTEMPTS: u32 = 5;
/// Delay between directory-removal retries.
const RETRY_REMOVE_DIR_DELAY: std::time::Duration = std::time::Duration::from_millis(50);

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
    #[error("Failed to lock Session index {path}: {source}")]
    LockSessionIndex {
        path: PathBuf,
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

    async fn lock_index_file(&self) -> Result<FileLock, SessionMetadataStoreError> {
        fs::create_dir_all(self.sessions_root())
            .await
            .map_err(|source| SessionMetadataStoreError::CreateSessionDir { source })?;
        let lock_path = self.sessions_root().join(".index.lock");
        let task_path = lock_path.clone();
        tokio::task::spawn_blocking(move || FileLock::acquire(&task_path, FileLockMode::Exclusive))
            .await
            .map_err(|error| SessionMetadataStoreError::LockSessionIndex {
                path: lock_path.clone(),
                source: std::io::Error::other(error),
            })?
            .map_err(|error| SessionMetadataStoreError::LockSessionIndex {
                path: lock_path,
                source: match error {
                    FileLockError::Open(source) | FileLockError::Unavailable(source) => source,
                },
            })
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

    /// Scan every metadata directory under the sessions root, skipping
    /// directories whose metadata.json is unreadable or damaged.
    ///
    /// Best-effort by contract: a single damaged session must not take down
    /// the whole listing/index rebuild (the remaining healthy sessions are
    /// still returned). Damaged sessions are surfaced explicitly — an
    /// `error!`-level log per scan (upgraded from `warn!`, d4-P2-6) so the
    /// "session silently disappeared" case is observable in product logs, and
    /// the count is exposed through [`Self::scan_metadata_dirs_reporting`]
    /// for callers that want to react (e.g. quarantine or repair).
    async fn scan_metadata_dirs(&self) -> Result<Vec<SessionMetadata>, SessionMetadataStoreError> {
        Ok(self.scan_metadata_dirs_reporting().await?.0)
    }

    /// Like [`Self::scan_metadata_dirs`] but also returns the session ids
    /// whose metadata could not be loaded (damaged/unreadable). Healthy
    /// sessions are unaffected; the damaged ids let a caller surface or
    /// quarantine the problem instead of silently dropping those sessions.
    async fn scan_metadata_dirs_reporting(
        &self,
    ) -> Result<(Vec<SessionMetadata>, Vec<String>), SessionMetadataStoreError> {
        if !self.sessions_root().exists() {
            return Ok((Vec::new(), Vec::new()));
        }

        // Collect session IDs first (directory listing), then load metadata in parallel.
        let mut session_ids = Vec::new();
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
            session_ids.push(entry.file_name().to_string_lossy().to_string());
        }

        // Load metadata in parallel to reduce directory rebuild latency.
        let handles: Vec<_> = session_ids
            .iter()
            .map(|sid| {
                let sid = sid.clone();
                async move {
                    let metadata = self.load_metadata(&sid).await;
                    (sid, metadata)
                }
            })
            .collect();

        let results = futures::future::join_all(handles).await;

        let mut metadata_list = Vec::new();
        let mut damaged_ids = Vec::new();
        for (session_id, result) in results {
            match result {
                Ok(Some(metadata)) => metadata_list.push(metadata),
                Ok(None) => {}
                Err(error) => {
                    // d4-P2-6: damaged per-session metadata must not be
                    // silently skipped. Error-level so the "session
                    // disappeared from every list" case is explicitly
                    // observable; best-effort listing of healthy sessions is
                    // preserved.
                    error!(
                        "Failed to rebuild session index entry: session_id={}, error={}",
                        session_id, error
                    );
                    damaged_ids.push(session_id);
                }
            }
        }

        metadata_list.sort_by_key(|metadata| std::cmp::Reverse(metadata.last_active_at));
        Ok((metadata_list, damaged_ids))
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

    async fn rebuild_index_snapshot_locked(
        &self,
    ) -> Result<(StoredSessionIndexFile, Vec<SessionMetadata>), SessionMetadataStoreError> {
        let metadata_list = self.scan_metadata_dirs().await?;
        let (index, visible_sessions) =
            build_session_index_snapshot(metadata_list, current_unix_ms());
        self.write_json_atomic(&self.index_path(), &index).await?;
        Ok((index, visible_sessions))
    }

    async fn rebuild_index_locked(
        &self,
    ) -> Result<Vec<SessionMetadata>, SessionMetadataStoreError> {
        self.rebuild_index_snapshot_locked()
            .await
            .map(|(_, visible_sessions)| visible_sessions)
    }

    /// Load the rebuildable Session index while the caller owns both index locks.
    ///
    /// Per-session `metadata.json` files are authoritative. Older BitFun versions
    /// can leave `index.json` missing, empty, or truncated if the machine stops
    /// during the Windows direct-overwrite fallback. Treat only index
    /// deserialization failures as recoverable; real filesystem errors must still
    /// reach the caller.
    async fn read_or_rebuild_index_locked(
        &self,
    ) -> Result<(StoredSessionIndexFile, bool), SessionMetadataStoreError> {
        let index_path = self.index_path();
        match self
            .read_json_optional::<StoredSessionIndexFile>(&index_path)
            .await
        {
            Ok(Some(index)) => Ok((index, false)),
            Ok(None) => self
                .rebuild_index_snapshot_locked()
                .await
                .map(|(index, _)| (index, true)),
            Err(error) if error.is_deserialization() => {
                warn!(
                    "Session index is unreadable; rebuilding from per-session metadata: path={}, error={}",
                    index_path.display(),
                    error
                );
                self.rebuild_index_snapshot_locked()
                    .await
                    .map(|(index, _)| (index, true))
            }
            Err(error) => Err(error),
        }
    }

    async fn upsert_index_entry_locked(
        &self,
        metadata: &SessionMetadata,
        metadata_file_created: bool,
    ) -> Result<(), SessionMetadataStoreError> {
        let (existing_index, rebuilt) = self.read_or_rebuild_index_locked().await?;
        if rebuilt {
            return Ok(());
        }
        let index = upsert_session_index_entry(
            Some(existing_index),
            metadata,
            metadata_file_created,
            0,
            current_unix_ms(),
        );
        self.write_json_atomic(&self.index_path(), &index).await
    }

    async fn remove_index_entry_locked(
        &self,
        session_id: &str,
        metadata_file_count_delta: isize,
    ) -> Result<(), SessionMetadataStoreError> {
        let (existing_index, rebuilt) = self.read_or_rebuild_index_locked().await?;
        if rebuilt {
            return Ok(());
        }
        let Some(index) = remove_session_index_entry(
            Some(existing_index),
            session_id,
            metadata_file_count_delta,
            current_unix_ms(),
        ) else {
            return Ok(());
        };
        self.write_json_atomic(&self.index_path(), &index).await
    }

    pub async fn list_metadata(&self) -> Result<Vec<SessionMetadata>, SessionMetadataStoreError> {
        self.list_metadata_with_options(false).await
    }

    /// Lists session metadata. With `include_internal` the visible index is
    /// bypassed and every metadata directory is scanned (same semantics as
    /// `list_metadata_including_internal`), so hidden Subagent/Ephemeral
    /// sessions become visible for full conversation management.
    pub async fn list_metadata_with_options(
        &self,
        include_internal: bool,
    ) -> Result<Vec<SessionMetadata>, SessionMetadataStoreError> {
        if include_internal {
            return self.list_metadata_including_internal().await;
        }
        if !self.sessions_root().exists() {
            return Ok(Vec::new());
        }

        let lock = self.get_index_lock().await;
        let _guard = lock.lock().await;
        let _file_guard = self.lock_index_file().await?;
        let index_path = self.index_path();
        let (index, _) = self.read_or_rebuild_index_locked().await?;
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

        Ok(index.sessions)
    }

    pub async fn list_metadata_page(
        &self,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<SessionMetadataPage, SessionMetadataStoreError> {
        self.list_metadata_page_with_options(cursor, limit, false).await
    }

    /// Paginated variant of [`list_metadata_with_options`]. With
    /// `include_internal` the visible index is bypassed and the page is built
    /// from a full metadata scan so hidden sessions participate in pagination.
    pub async fn list_metadata_page_with_options(
        &self,
        cursor: Option<&str>,
        limit: usize,
        include_internal: bool,
    ) -> Result<SessionMetadataPage, SessionMetadataStoreError> {
        if include_internal {
            let mut sessions = self.scan_metadata_dirs().await?;
            sessions.sort_by_key(|metadata| std::cmp::Reverse(metadata.last_active_at));
            return Ok(build_session_metadata_page_with_options(
                sessions,
                cursor,
                limit,
                true,
            ));
        }
        if !self.sessions_root().exists() {
            return Ok(empty_session_metadata_page());
        }

        let limit = limit.max(1);
        let lock = self.get_index_lock().await;
        let _guard = lock.lock().await;
        let _file_guard = self.lock_index_file().await?;
        let index_path = self.index_path();
        let (index, _) = self.read_or_rebuild_index_locked().await?;
        let indexed_sessions = if index.metadata_file_count < index.sessions.len() {
            warn!(
                "Session index has invalid metadata count before page read (index: {}, sessions: {}), rebuilding: {}",
                index.metadata_file_count,
                index.sessions.len(),
                index_path.display()
            );
            self.rebuild_index_locked().await?
        } else {
            index.sessions
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
        let _file_guard = self.lock_index_file().await?;
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
        let _file_guard = self.lock_index_file().await?;
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
        let _file_guard = self.lock_index_file().await?;
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
            // Windows (and some filesystems) can transiently fail to remove a
            // directory whose files were just written: handles may still be
            // closing or antivirus/indexing may hold a short-lived handle.
            // Retry a few times with a small delay before giving up so the
            // deletion is not silently lost.
            let mut last_error: Option<std::io::Error> = None;
            for attempt in 0..RETRY_REMOVE_DIR_ATTEMPTS {
                match fs::remove_dir_all(&dir).await {
                    Ok(()) => {
                        last_error = None;
                        break;
                    }
                    Err(source) => {
                        last_error = Some(source);
                        if attempt + 1 < RETRY_REMOVE_DIR_ATTEMPTS {
                            tokio::time::sleep(RETRY_REMOVE_DIR_DELAY).await;
                        }
                    }
                }
            }
            if let Some(source) = last_error {
                return Err(SessionMetadataStoreError::DeleteSessionDir { source });
            }
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

    #[test]
    fn index_lock_child_holds_the_cross_process_guard() {
        if std::env::var_os("BITFUN_SESSION_INDEX_LOCK_CHILD").is_none() {
            return;
        }
        let sessions_root =
            PathBuf::from(std::env::var_os("BITFUN_SESSION_INDEX_ROOT").expect("index lock root"));
        let ready_path = PathBuf::from(
            std::env::var_os("BITFUN_SESSION_INDEX_READY").expect("index lock ready path"),
        );
        let release_path = PathBuf::from(
            std::env::var_os("BITFUN_SESSION_INDEX_RELEASE").expect("index lock release path"),
        );
        std::fs::create_dir_all(&sessions_root).expect("sessions root");
        let _guard = FileLock::acquire(&sessions_root.join(".index.lock"), FileLockMode::Exclusive)
            .expect("child index lock");
        std::fs::write(&ready_path, b"ready").expect("publish child readiness");
        while !release_path.exists() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[tokio::test]
    async fn metadata_save_waits_for_a_cross_process_index_writer() {
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};

        let dir = tempdir().expect("tempdir");
        let ready_path = dir.path().join("child-ready");
        let release_path = dir.path().join("child-release");
        let mut child = Command::new(std::env::current_exe().expect("test executable"))
            .arg("--exact")
            .arg("session::metadata_store::tests::index_lock_child_holds_the_cross_process_guard")
            .arg("--nocapture")
            .env("BITFUN_SESSION_INDEX_LOCK_CHILD", "1")
            .env("BITFUN_SESSION_INDEX_ROOT", dir.path())
            .env("BITFUN_SESSION_INDEX_READY", &ready_path)
            .env("BITFUN_SESSION_INDEX_RELEASE", &release_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn index lock child");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !ready_path.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        if !ready_path.exists() {
            let _ = child.kill();
            let _ = child.wait();
            panic!("index lock child did not become ready");
        }

        let store = SessionMetadataStore::new(dir.path());
        let mut save =
            tokio::spawn(async move { store.save_metadata(&metadata("session-a", 10)).await });
        let blocked = tokio::time::timeout(Duration::from_millis(50), &mut save)
            .await
            .is_err();

        std::fs::write(&release_path, b"release").expect("release child index lock");
        save.await.expect("save task").expect("metadata save");
        assert!(child.wait().expect("index lock child").success());
        assert!(
            blocked,
            "metadata save must wait while another process owns the index"
        );
    }

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
    async fn metadata_store_recovers_empty_index_while_saving_new_metadata() {
        let dir = tempdir().expect("tempdir");
        let store = SessionMetadataStore::new(dir.path());
        store
            .save_metadata(&metadata("historical", 20))
            .await
            .expect("save historical metadata");
        let historical_turn = store
            .session_dir("historical")
            .join("turns")
            .join("turn-0000.json");
        fs::create_dir_all(historical_turn.parent().expect("turn parent"))
            .await
            .expect("create historical turns directory");
        fs::write(&historical_turn, b"historical turn payload")
            .await
            .expect("write historical turn sentinel");

        fs::write(store.index_path(), b"")
            .await
            .expect("simulate an empty index after an interrupted write");
        store
            .save_metadata(&metadata("new-session", 10))
            .await
            .expect("a corrupt derived index must not block a new session");

        let listed = store.list_metadata().await.expect("list rebuilt metadata");
        assert_eq!(
            listed
                .iter()
                .map(|value| value.session_id.as_str())
                .collect::<Vec<_>>(),
            vec!["historical", "new-session"]
        );
        assert_eq!(
            fs::read(&historical_turn)
                .await
                .expect("historical turn must remain readable"),
            b"historical turn payload"
        );
        let rebuilt = store
            .read_json_optional::<StoredSessionIndexFile>(&store.index_path())
            .await
            .expect("read rebuilt index")
            .expect("rebuilt index exists");
        assert_eq!(rebuilt.metadata_file_count, 2);
        assert_eq!(rebuilt.sessions.len(), 2);
    }

    #[tokio::test]
    async fn metadata_store_page_recovers_truncated_index() {
        let dir = tempdir().expect("tempdir");
        let store = SessionMetadataStore::new(dir.path());
        store
            .save_metadata(&metadata("older", 10))
            .await
            .expect("save older metadata");
        store
            .save_metadata(&metadata("newer", 20))
            .await
            .expect("save newer metadata");
        fs::write(store.index_path(), br#"{"schema_version":2,"updated_at":"#)
            .await
            .expect("simulate a truncated index");

        let page = store
            .list_metadata_page(None, 10)
            .await
            .expect("paged listing must rebuild a truncated index");

        assert_eq!(
            page.sessions
                .iter()
                .map(|value| value.session_id.as_str())
                .collect::<Vec<_>>(),
            vec!["newer", "older"]
        );
    }

    #[tokio::test]
    async fn metadata_store_delete_recovers_corrupt_index_and_preserves_other_sessions() {
        let dir = tempdir().expect("tempdir");
        let store = SessionMetadataStore::new(dir.path());
        store
            .save_metadata(&metadata("historical", 20))
            .await
            .expect("save historical metadata");
        store
            .save_metadata(&metadata("partial-create", 10))
            .await
            .expect("save partial create metadata");
        fs::write(store.index_path(), b"")
            .await
            .expect("simulate an empty index");

        store
            .delete_session_dir_and_index("partial-create")
            .await
            .expect("cleanup must rebuild the corrupt index");

        assert!(!store.session_dir("partial-create").exists());
        assert!(store.session_dir("historical").exists());
        let listed = store.list_metadata().await.expect("list surviving session");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].session_id, "historical");
    }

    #[tokio::test]
    async fn metadata_store_rebuilds_missing_index_before_save_without_hiding_history() {
        let dir = tempdir().expect("tempdir");
        let store = SessionMetadataStore::new(dir.path());
        store
            .save_metadata(&metadata("historical", 20))
            .await
            .expect("save historical metadata");
        fs::remove_file(store.index_path())
            .await
            .expect("simulate the replace gap left by an older version");

        store
            .save_metadata(&metadata("new-session", 10))
            .await
            .expect("save with a missing derived index");

        let listed = store.list_metadata().await.expect("list rebuilt metadata");
        assert_eq!(
            listed
                .iter()
                .map(|value| value.session_id.as_str())
                .collect::<Vec<_>>(),
            vec!["historical", "new-session"]
        );
    }

    #[tokio::test]
    async fn metadata_store_rebuilds_legacy_index_without_metadata_file_count() {
        let dir = tempdir().expect("tempdir");
        let store = SessionMetadataStore::new(dir.path());
        let historical = metadata("historical", 20);
        store
            .save_metadata(&historical)
            .await
            .expect("save historical metadata");
        let legacy_index = serde_json::json!({
            "schema_version": 2,
            "updated_at": 1,
            "sessions": [historical]
        });
        fs::write(
            store.index_path(),
            serde_json::to_vec(&legacy_index).expect("serialize legacy index"),
        )
        .await
        .expect("write legacy index");

        let listed = store
            .list_metadata()
            .await
            .expect("legacy index remains upgrade-compatible");

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].session_id, "historical");
        let rebuilt = store
            .read_json_optional::<StoredSessionIndexFile>(&store.index_path())
            .await
            .expect("read upgraded index")
            .expect("upgraded index exists");
        assert_eq!(rebuilt.metadata_file_count, 1);
    }

    #[tokio::test]
    async fn metadata_store_does_not_treat_index_io_errors_as_corruption() {
        let dir = tempdir().expect("tempdir");
        let store = SessionMetadataStore::new(dir.path());
        store
            .save_metadata(&metadata("session-a", 10))
            .await
            .expect("save metadata");
        fs::remove_file(store.index_path())
            .await
            .expect("remove index file");
        fs::create_dir(store.index_path())
            .await
            .expect("replace index with an unreadable directory");

        let error = store
            .list_metadata()
            .await
            .expect_err("filesystem errors must not be swallowed as corrupt JSON");

        assert!(!error.is_deserialization());
        assert!(store.index_path().is_dir());
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
    async fn metadata_store_with_options_includes_hidden_sessions() {
        let dir = tempdir().expect("tempdir");
        let store = SessionMetadataStore::new(dir.path());
        let mut hidden = metadata("hidden", 30);
        hidden.session_kind = bitfun_core_types::SessionKind::Subagent;
        store
            .save_metadata(&hidden)
            .await
            .expect("save hidden metadata");

        assert!(store
            .list_metadata_with_options(false)
            .await
            .expect("visible list")
            .is_empty());
        assert_eq!(
            store
                .list_metadata_with_options(true)
                .await
                .expect("full list")
                .len(),
            1
        );
        assert_eq!(
            store
                .list_metadata_page_with_options(None, 10, true)
                .await
                .expect("full page")
                .sessions
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
