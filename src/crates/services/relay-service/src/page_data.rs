//! Per-page mutable runtime data (KV / SQLite / Blobs), keyed by
//! (user_id, slug, generation). Survives version deploy/rollback, while a
//! delete/recreate cycle receives a fresh, isolated generation.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use bitfun_page_function_runtime::{PageHost, PageMeta};
use dashmap::DashMap;
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::limits::Limit;
use sha2::{Digest, Sha256};
use tokio::runtime::Handle;

use crate::db::{page_kv, DbPool, PageRow};

pub const MAX_BLOB_ID_BYTES: usize = 128;
pub const MAX_BLOB_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_BLOB_FILES_PER_PAGE: u64 = 2_048;
pub const MAX_BLOB_FILES_PER_USER: u64 = 10_000;
pub const MAX_MUTABLE_BYTES_PER_PAGE: u64 = 64 * 1024 * 1024;
pub const MAX_MUTABLE_BYTES_PER_USER: u64 = 256 * 1024 * 1024;
pub const MAX_PAGE_DB_BYTES: u64 = 20 * 1024 * 1024;
pub const MAX_DB_SQL_BYTES: usize = 64 * 1024;
pub const MAX_DB_PARAMS_BYTES: usize = 256 * 1024;
pub const MAX_DB_PARAMS: usize = 256;
pub const MAX_DB_QUERY_ROWS: usize = 1_000;
pub const MAX_DB_QUERY_BYTES: usize = 2 * 1024 * 1024;
const MAX_DB_VALUE_BYTES: i32 = 2 * 1024 * 1024;
const LEGACY_BLOB_LAYOUT_MARKER: &str = ".legacy-blob-layout-v1";
#[cfg(not(test))]
const MAX_DB_OPERATION_TIME: Duration = Duration::from_secs(1);
#[cfg(test)]
const MAX_DB_OPERATION_TIME: Duration = Duration::from_millis(50);

/// Root directory for page-data
/// (`{base}/{user_id}/{slug}/{generation}/...`).
#[derive(Clone)]
pub struct PageDataStore {
    base_dir: PathBuf,
    user_mutation_locks: Arc<DashMap<String, Arc<Mutex<()>>>>,
}

impl PageDataStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        let base_dir = base_dir.into();
        let _ = std::fs::create_dir_all(&base_dir);
        Self {
            base_dir,
            user_mutation_locks: Arc::new(DashMap::new()),
        }
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    fn page_root(&self, user_id: &str, slug: &str) -> PathBuf {
        self.base_dir.join(user_id).join(slug)
    }

    fn page_dir(&self, user_id: &str, slug: &str, generation: &str) -> PathBuf {
        self.page_root(user_id, slug).join(generation)
    }

    fn db_path(&self, user_id: &str, slug: &str, generation: &str) -> PathBuf {
        self.page_dir(user_id, slug, generation).join("db.sqlite")
    }

    fn blobs_dir(&self, user_id: &str, slug: &str, generation: &str) -> PathBuf {
        self.page_dir(user_id, slug, generation).join("blobs-v2")
    }

    pub fn cleanup_page(&self, user_id: &str, slug: &str) {
        let lock = self.user_mutation_lock(user_id);
        let Ok(_guard) = lock.lock() else {
            return;
        };
        if let Ok(Some(dir)) = self.existing_page_root(user_id, slug) {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// Assign any pre-generation PageData layout to `generation` and make the
    /// generation directory ready. Calling this before deleting the Page row
    /// ensures that a crash cannot let a later Page with the same slug adopt
    /// the old mutable data.
    pub fn prepare_generation(
        &self,
        user_id: &str,
        slug: &str,
        generation: &str,
    ) -> Result<PathBuf> {
        let lock = self.user_mutation_lock(user_id);
        let _guard = lock
            .lock()
            .map_err(|_| anyhow!("page-data mutation lock poisoned"))?;
        self.ensure_generation_dir_locked(user_id, slug, generation)
    }

    fn ensure_generation_dir_locked(
        &self,
        user_id: &str,
        slug: &str,
        generation: &str,
    ) -> Result<PathBuf> {
        validate_generation(generation)?;
        ensure_directory(&self.base_dir)?;
        ensure_directory(&self.base_dir.join(user_id))?;
        let root = self.page_root(user_id, slug);
        ensure_directory(&root)?;

        let existing_generations = generation_directories(&root)?;
        let may_adopt_legacy = existing_generations.is_empty()
            || existing_generations
                .iter()
                .all(|existing| existing == generation);
        let target = self.page_dir(user_id, slug, generation);
        ensure_directory(&target)?;
        if may_adopt_legacy {
            migrate_legacy_page_data(&root, &target)?;
        }
        Ok(target)
    }

    fn existing_page_root(&self, user_id: &str, slug: &str) -> Result<Option<PathBuf>> {
        if existing_directory(&self.base_dir)?.is_none()
            || existing_directory(&self.base_dir.join(user_id))?.is_none()
        {
            return Ok(None);
        }
        existing_directory(&self.page_root(user_id, slug))
    }

    fn user_mutation_lock(&self, user_id: &str) -> Arc<Mutex<()>> {
        Arc::clone(
            self.user_mutation_locks
                .entry(user_id.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .value(),
        )
    }

    pub fn blob_put(
        &self,
        user_id: &str,
        slug: &str,
        generation: &str,
        blob_id: &str,
        content_type: &str,
        data: &[u8],
    ) -> Result<()> {
        validate_blob_id(blob_id)?;
        validate_content_type(content_type)?;
        if data.len() > MAX_BLOB_BYTES {
            return Err(anyhow!(
                "blob exceeds the {} byte operation limit",
                MAX_BLOB_BYTES
            ));
        }
        let lock = self.user_mutation_lock(user_id);
        let _guard = lock
            .lock()
            .map_err(|_| anyhow!("page-data mutation lock poisoned"))?;
        let page_dir = self.ensure_generation_dir_locked(user_id, slug, generation)?;
        let blobs = self.blobs_dir(user_id, slug, generation);
        ensure_directory(&blobs)?;
        ensure_directory(&blobs.join(".metadata"))?;
        migrate_legacy_blob_locked(&page_dir, &blobs, blob_id)?;
        let storage_name = blob_storage_name(blob_id);
        let path = blobs.join(&storage_name);
        let meta = blob_meta_path(&blobs, &storage_name);

        if !path.exists() {
            let page_blob_files = page_blob_file_count(&page_dir)?;
            let user_blob_files = user_blob_file_count(&self.base_dir.join(user_id))?;
            if page_blob_files >= MAX_BLOB_FILES_PER_PAGE {
                return Err(anyhow!("page blob file quota exceeded"));
            }
            if user_blob_files >= MAX_BLOB_FILES_PER_USER {
                return Err(anyhow!("account blob file quota exceeded"));
            }
        }

        let replaced_bytes = file_len(&path).saturating_add(file_len(&meta));
        let added_bytes = (data.len() as u64).saturating_add(content_type.len() as u64);
        let page_bytes = directory_size(&page_dir)?;
        let user_bytes = directory_size(&self.base_dir.join(user_id))?;
        enforce_storage_quota(page_bytes, user_bytes, replaced_bytes, added_bytes)?;

        std::fs::write(&path, data).map_err(|e| anyhow!("write blob: {e}"))?;
        std::fs::write(&meta, content_type).map_err(|e| anyhow!("write blob meta: {e}"))?;
        Ok(())
    }

    pub fn blob_get(
        &self,
        user_id: &str,
        slug: &str,
        generation: &str,
        blob_id: &str,
    ) -> Result<Option<(String, Vec<u8>)>> {
        validate_blob_id(blob_id)?;
        let lock = self.user_mutation_lock(user_id);
        let _guard = lock
            .lock()
            .map_err(|_| anyhow!("page-data mutation lock poisoned"))?;
        let page_dir = self.ensure_generation_dir_locked(user_id, slug, generation)?;
        let blobs = self.blobs_dir(user_id, slug, generation);
        ensure_directory(&blobs)?;
        ensure_directory(&blobs.join(".metadata"))?;
        migrate_legacy_blob_locked(&page_dir, &blobs, blob_id)?;
        let storage_name = blob_storage_name(blob_id);
        let path = blobs.join(&storage_name);
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(anyhow!("read blob metadata: {error}")),
        };
        if metadata.file_type().is_symlink() {
            return Err(anyhow!("blob path must not be a symlink"));
        }
        if !metadata.is_file() {
            return Ok(None);
        }
        let size = metadata.len();
        if size > MAX_BLOB_BYTES as u64 {
            return Err(anyhow!("stored blob exceeds the read limit"));
        }
        let data = std::fs::read(&path).map_err(|e| anyhow!("read blob: {e}"))?;
        let meta = blob_meta_path(&blobs, &storage_name);
        let content_type = read_small_metadata(&meta)
            .or_else(|| legacy_blob_content_type(&page_dir, blob_id))
            .unwrap_or_else(|| "application/octet-stream".to_string());
        Ok(Some((content_type, data)))
    }

    pub fn blob_delete(
        &self,
        user_id: &str,
        slug: &str,
        generation: &str,
        blob_id: &str,
    ) -> Result<bool> {
        validate_blob_id(blob_id)?;
        let lock = self.user_mutation_lock(user_id);
        let _guard = lock
            .lock()
            .map_err(|_| anyhow!("page-data mutation lock poisoned"))?;
        let page_dir = self.ensure_generation_dir_locked(user_id, slug, generation)?;
        let blobs = self.blobs_dir(user_id, slug, generation);
        ensure_directory(&blobs)?;
        ensure_directory(&blobs.join(".metadata"))?;
        migrate_legacy_blob_locked(&page_dir, &blobs, blob_id)?;
        let storage_name = blob_storage_name(blob_id);
        let path = blobs.join(&storage_name);
        let meta = blob_meta_path(&blobs, &storage_name);
        let existed = path.exists();
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&meta);
        Ok(existed)
    }

    pub fn db_execute(
        &self,
        user_id: &str,
        slug: &str,
        generation: &str,
        sql: &str,
        params_json: &str,
    ) -> Result<String> {
        validate_db_input(sql, params_json)?;
        let params = parse_db_params(params_json)?;
        let lock = self.user_mutation_lock(user_id);
        let _guard = lock
            .lock()
            .map_err(|_| anyhow!("page-data mutation lock poisoned"))?;
        self.ensure_generation_dir_locked(user_id, slug, generation)?;
        let path = self.db_path(user_id, slug, generation);
        ensure_regular_file_or_missing(&path)?;
        let mut conn =
            rusqlite::Connection::open(&path).map_err(|e| anyhow!("open page db: {e}"))?;
        configure_connection(&conn);
        configure_database_quota(self, &conn, user_id, slug, generation, &path)?;

        let tx = conn
            .transaction()
            .map_err(|e| anyhow!("begin transaction: {e}"))?;
        tx.authorizer(Some(authorize_execute));
        let result = tx.execute(
            sql,
            rusqlite::params_from_iter(params.iter().map(json_to_sql)),
        );
        tx.authorizer(None::<fn(AuthContext<'_>) -> Authorization>);
        let changes = result.map_err(|e| anyhow!("execute: {e}"))?;
        let logical_bytes = database_logical_bytes(&tx)?;
        if logical_bytes > MAX_PAGE_DB_BYTES {
            return Err(anyhow!("page database quota exceeded"));
        }
        tx.commit().map_err(|e| anyhow!("commit: {e}"))?;
        enforce_current_storage_quota(self, user_id, slug, generation)?;
        Ok(serde_json::json!({ "ok": true, "changes": changes }).to_string())
    }

    pub fn db_query(
        &self,
        user_id: &str,
        slug: &str,
        generation: &str,
        sql: &str,
        params_json: &str,
    ) -> Result<String> {
        validate_db_input(sql, params_json)?;
        let params = parse_db_params(params_json)?;
        let lock = self.user_mutation_lock(user_id);
        let _guard = lock
            .lock()
            .map_err(|_| anyhow!("page-data mutation lock poisoned"))?;
        self.ensure_generation_dir_locked(user_id, slug, generation)?;
        let path = self.db_path(user_id, slug, generation);
        ensure_regular_file_or_missing(&path)?;
        let conn = rusqlite::Connection::open(&path).map_err(|e| anyhow!("open page db: {e}"))?;
        configure_connection(&conn);
        conn.authorizer(Some(authorize_query));
        let mut stmt = conn.prepare(sql).map_err(|e| anyhow!("prepare: {e}"))?;
        if !stmt.readonly() {
            return Err(anyhow!("DB.query only accepts read-only statements"));
        }
        let col_count = stmt.column_count();
        let col_names: Vec<String> = (0..col_count)
            .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
            .collect();
        let mut rows_out = Vec::new();
        let mut response_bytes = 0usize;
        let mut rows = stmt
            .query(rusqlite::params_from_iter(params.iter().map(json_to_sql)))
            .map_err(|e| anyhow!("query: {e}"))?;
        while let Some(row) = rows.next().map_err(|e| anyhow!("row: {e}"))? {
            if rows_out.len() >= MAX_DB_QUERY_ROWS {
                return Err(anyhow!(
                    "query exceeds the {} row result limit",
                    MAX_DB_QUERY_ROWS
                ));
            }
            let mut obj = serde_json::Map::new();
            for (i, name) in col_names.iter().enumerate() {
                let val = match row.get_ref(i).map_err(|e| anyhow!("get: {e}"))? {
                    rusqlite::types::ValueRef::Null => serde_json::Value::Null,
                    rusqlite::types::ValueRef::Integer(n) => serde_json::json!(n),
                    rusqlite::types::ValueRef::Real(n) => serde_json::json!(n),
                    rusqlite::types::ValueRef::Text(t) => {
                        serde_json::Value::String(String::from_utf8_lossy(t).into_owned())
                    }
                    rusqlite::types::ValueRef::Blob(b) => serde_json::Value::String(B64.encode(b)),
                };
                obj.insert(name.clone(), val);
            }
            let value = serde_json::Value::Object(obj);
            let row_bytes = serde_json::to_vec(&value)
                .map_err(|e| anyhow!("serialize query row: {e}"))?
                .len();
            if response_bytes.saturating_add(row_bytes) > MAX_DB_QUERY_BYTES {
                return Err(anyhow!(
                    "query exceeds the {} byte result limit",
                    MAX_DB_QUERY_BYTES
                ));
            }
            response_bytes = response_bytes.saturating_add(row_bytes);
            rows_out.push(value);
        }
        Ok(serde_json::json!({ "ok": true, "rows": rows_out }).to_string())
    }
}

fn validate_generation(generation: &str) -> Result<()> {
    if generation.len() != 32 || !generation.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(anyhow!("invalid page-data generation"));
    }
    Ok(())
}

fn is_generation_name(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|value| {
        value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn generation_directories(page_root: &Path) -> Result<Vec<String>> {
    let mut generations = Vec::new();
    for entry in std::fs::read_dir(page_root)
        .map_err(|e| anyhow!("read PageData root {}: {e}", page_root.display()))?
    {
        let entry = entry.map_err(|e| anyhow!("read PageData entry: {e}"))?;
        if !is_generation_name(&entry.file_name()) {
            continue;
        }
        let metadata = std::fs::symlink_metadata(entry.path())
            .map_err(|e| anyhow!("inspect PageData generation: {e}"))?;
        if metadata.file_type().is_symlink() {
            return Err(anyhow!("PageData generation must not be a symlink"));
        }
        if !metadata.is_dir() {
            return Err(anyhow!("PageData generation path must be a directory"));
        }
        generations.push(entry.file_name().to_string_lossy().into_owned());
    }
    Ok(generations)
}

fn migrate_legacy_page_data(page_root: &Path, generation_dir: &Path) -> Result<()> {
    for entry_name in [
        "db.sqlite",
        "db.sqlite-wal",
        "db.sqlite-shm",
        "db.sqlite-journal",
        "blobs",
    ] {
        let source = page_root.join(entry_name);
        let metadata = match std::fs::symlink_metadata(&source) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(anyhow!(
                    "inspect legacy PageData path {}: {error}",
                    source.display()
                ))
            }
        };
        if metadata.file_type().is_symlink() {
            return Err(anyhow!(
                "legacy PageData path must not be a symlink: {}",
                source.display()
            ));
        }
        if entry_name == "blobs" {
            create_legacy_blob_layout_marker(generation_dir)?;
        }
        let target = generation_dir.join(entry_name);
        if target.exists() {
            return Err(anyhow!(
                "legacy PageData migration target already exists: {}",
                target.display()
            ));
        }
        std::fs::rename(&source, &target).map_err(|e| {
            anyhow!(
                "move legacy PageData {} to generation: {e}",
                source.display()
            )
        })?;
    }
    Ok(())
}

fn create_legacy_blob_layout_marker(generation_dir: &Path) -> Result<()> {
    let marker = generation_dir.join(LEGACY_BLOB_LAYOUT_MARKER);
    ensure_regular_file_or_missing(&marker)?;
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&marker)
    {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(anyhow!(
            "create legacy PageData blob-layout marker {}: {error}",
            marker.display()
        )),
    }
}

fn has_legacy_blob_layout(generation_dir: &Path) -> Result<bool> {
    let marker = generation_dir.join(LEGACY_BLOB_LAYOUT_MARKER);
    match std::fs::symlink_metadata(&marker) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(anyhow!(
            "legacy PageData blob-layout marker must not be a symlink"
        )),
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(_) => Err(anyhow!(
            "legacy PageData blob-layout marker must be a regular file"
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(anyhow!(
            "inspect legacy PageData blob-layout marker: {error}"
        )),
    }
}

/// Move one pre-hash blob into the portable hashed layout while the caller
/// holds the account mutation lock. A full-directory rewrite cannot safely
/// distinguish `foo.meta` (legacy metadata for `foo`) from a real blob whose
/// id is `foo.meta`, so migration is intentionally lazy and id-scoped.
fn migrate_legacy_blob_locked(
    generation_dir: &Path,
    blobs_dir: &Path,
    blob_id: &str,
) -> Result<()> {
    if !has_legacy_blob_layout(generation_dir)? || !is_legacy_blob_id(blob_id) {
        return Ok(());
    }

    let storage_name = blob_storage_name(blob_id);
    let legacy_blobs_dir = generation_dir.join("blobs");

    migrate_legacy_blob_file(
        &legacy_blobs_dir.join(blob_id),
        &blobs_dir.join(&storage_name),
        "data",
    )?;
    migrate_legacy_blob_file(
        &legacy_blobs_dir.join(".metadata").join(blob_id),
        &blob_meta_path(blobs_dir, &storage_name),
        "metadata",
    )?;
    Ok(())
}

fn migrate_legacy_blob_file(source: &Path, target: &Path, kind: &str) -> Result<()> {
    let source_metadata = match std::fs::symlink_metadata(source) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(anyhow!("inspect legacy blob {kind}: {error}")),
    };
    if source_metadata.file_type().is_symlink() || !source_metadata.is_file() {
        return Err(anyhow!("legacy blob {kind} must be a regular file"));
    }

    match std::fs::symlink_metadata(target) {
        Ok(target_metadata) => {
            if target_metadata.file_type().is_symlink() || !target_metadata.is_file() {
                return Err(anyhow!("hashed blob {kind} must be a regular file"));
            }
            if !regular_files_equal(source, target, source_metadata.len(), target_metadata.len())? {
                return Err(anyhow!(
                    "legacy blob {kind} conflicts with existing hashed storage"
                ));
            }
            std::fs::remove_file(source)
                .map_err(|error| anyhow!("remove duplicate legacy blob {kind}: {error}"))?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::rename(source, target)
                .map_err(|error| anyhow!("move legacy blob {kind} to hashed storage: {error}"))
        }
        Err(error) => Err(anyhow!("inspect hashed blob {kind}: {error}")),
    }
}

fn regular_files_equal(left: &Path, right: &Path, left_len: u64, right_len: u64) -> Result<bool> {
    if left_len != right_len {
        return Ok(false);
    }
    let left = std::fs::read(left).map_err(|error| anyhow!("read legacy blob: {error}"))?;
    let right = std::fs::read(right).map_err(|error| anyhow!("read hashed blob: {error}"))?;
    Ok(left == right)
}

fn legacy_blob_content_type(generation_dir: &Path, blob_id: &str) -> Option<String> {
    if !is_legacy_blob_id(blob_id) || !has_legacy_blob_layout(generation_dir).ok()? {
        return None;
    }
    let legacy_blobs_dir = generation_dir.join("blobs");
    read_small_metadata(&legacy_blobs_dir.join(".metadata").join(blob_id))
        // The sibling form is read-only during migration: deleting or moving
        // it could destroy a distinct blob whose id happens to end in `.meta`.
        .or_else(|| read_small_metadata(&legacy_blobs_dir.join(format!("{blob_id}.meta"))))
}

fn ensure_directory(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(anyhow!(
            "page-data directory must not be a symlink: {}",
            path.display()
        )),
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(anyhow!(
            "page-data directory path is not a directory: {}",
            path.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => std::fs::create_dir(path)
            .map_err(|e| anyhow!("create page-data directory {}: {e}", path.display())),
        Err(error) => Err(anyhow!(
            "inspect page-data directory {}: {error}",
            path.display()
        )),
    }
}

fn existing_directory(path: &Path) -> Result<Option<PathBuf>> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(anyhow!(
            "page-data directory must not be a symlink: {}",
            path.display()
        )),
        Ok(metadata) if metadata.is_dir() => Ok(Some(path.to_path_buf())),
        Ok(_) => Err(anyhow!(
            "page-data directory path is not a directory: {}",
            path.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(anyhow!(
            "inspect page-data directory {}: {error}",
            path.display()
        )),
    }
}

fn ensure_regular_file_or_missing(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(anyhow!(
            "page-data file must not be a symlink: {}",
            path.display()
        )),
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(anyhow!(
            "page-data file path is not a regular file: {}",
            path.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(anyhow!(
            "inspect page-data file {}: {error}",
            path.display()
        )),
    }
}

fn validate_blob_id(blob_id: &str) -> Result<()> {
    if blob_id.is_empty()
        || blob_id.len() > MAX_BLOB_ID_BYTES
        || blob_id.chars().any(char::is_control)
    {
        return Err(anyhow!("invalid blob id"));
    }
    Ok(())
}

fn is_legacy_blob_id(blob_id: &str) -> bool {
    if blob_id.is_empty()
        || blob_id.len() > MAX_BLOB_ID_BYTES
        || matches!(blob_id, "." | ".." | ".metadata")
        || blob_id.contains('/')
        || blob_id.contains('\\')
        || blob_id.chars().any(char::is_control)
    {
        return false;
    }

    #[cfg(windows)]
    {
        if blob_id
            .chars()
            .any(|character| matches!(character, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
            || blob_id.ends_with(' ')
            || blob_id.ends_with('.')
        {
            return false;
        }
        let stem = blob_id
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.as_bytes()[3].is_ascii_digit()
                && stem.as_bytes()[3] != b'0')
        {
            return false;
        }
    }

    true
}

fn blob_storage_name(blob_id: &str) -> String {
    Sha256::digest(blob_id.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn validate_content_type(content_type: &str) -> Result<()> {
    if content_type.is_empty()
        || content_type.len() > 256
        || content_type.chars().any(char::is_control)
    {
        return Err(anyhow!("invalid blob content type"));
    }
    Ok(())
}

fn blob_meta_path(blobs_dir: &Path, storage_name: &str) -> PathBuf {
    blobs_dir.join(".metadata").join(storage_name)
}

fn file_len(path: &Path) -> u64 {
    path.metadata().map_or(0, |metadata| metadata.len())
}

fn read_small_metadata(path: &Path) -> Option<String> {
    if file_len(path) > 256 {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

fn directory_size(path: &Path) -> Result<u64> {
    if !path.exists() {
        return Ok(0);
    }
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|e| anyhow!("inspect page-data path {}: {e}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(anyhow!(
            "page-data path must not contain symlinks: {}",
            path.display()
        ));
    }
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    if !metadata.is_dir() {
        return Ok(0);
    }

    let mut total = 0u64;
    for entry in std::fs::read_dir(path)
        .map_err(|e| anyhow!("read page-data directory {}: {e}", path.display()))?
    {
        let entry = entry.map_err(|e| anyhow!("read page-data entry: {e}"))?;
        total = total
            .checked_add(directory_size(&entry.path())?)
            .ok_or_else(|| anyhow!("page-data size overflow"))?;
    }
    Ok(total)
}

fn direct_file_count(path: &Path) -> Result<u64> {
    if !path.exists() {
        return Ok(0);
    }
    let mut count = 0u64;
    for entry in std::fs::read_dir(path)
        .map_err(|e| anyhow!("read blob directory {}: {e}", path.display()))?
    {
        let entry = entry.map_err(|e| anyhow!("read blob entry: {e}"))?;
        let metadata = std::fs::symlink_metadata(entry.path())
            .map_err(|e| anyhow!("inspect blob entry: {e}"))?;
        if metadata.file_type().is_symlink() {
            return Err(anyhow!("blob directory must not contain symlinks"));
        }
        if metadata.is_file() {
            count = count
                .checked_add(1)
                .ok_or_else(|| anyhow!("blob file count overflow"))?;
        }
    }
    Ok(count)
}

fn page_blob_file_count(page_dir: &Path) -> Result<u64> {
    direct_file_count(&page_dir.join("blobs"))?
        .checked_add(direct_file_count(&page_dir.join("blobs-v2"))?)
        .ok_or_else(|| anyhow!("page blob file count overflow"))
}

fn user_blob_file_count(user_dir: &Path) -> Result<u64> {
    if !user_dir.exists() {
        return Ok(0);
    }
    let mut count = 0u64;
    for page in
        std::fs::read_dir(user_dir).map_err(|e| anyhow!("read account page-data directory: {e}"))?
    {
        let page = page.map_err(|e| anyhow!("read account page-data entry: {e}"))?;
        let metadata = std::fs::symlink_metadata(page.path())
            .map_err(|e| anyhow!("inspect account page-data entry: {e}"))?;
        if metadata.file_type().is_symlink() {
            return Err(anyhow!("page-data directory must not contain symlinks"));
        }
        if metadata.is_dir() {
            // Count the pre-generation layout until it has been migrated.
            count = count
                .checked_add(page_blob_file_count(&page.path())?)
                .ok_or_else(|| anyhow!("account blob file count overflow"))?;
            for generation in std::fs::read_dir(page.path())
                .map_err(|e| anyhow!("read PageData generation directory: {e}"))?
            {
                let generation =
                    generation.map_err(|e| anyhow!("read PageData generation entry: {e}"))?;
                if !is_generation_name(&generation.file_name()) {
                    continue;
                }
                let generation_metadata = std::fs::symlink_metadata(generation.path())
                    .map_err(|e| anyhow!("inspect PageData generation entry: {e}"))?;
                if generation_metadata.file_type().is_symlink() {
                    return Err(anyhow!("PageData generation must not be a symlink"));
                }
                if !generation_metadata.is_dir() {
                    return Err(anyhow!("PageData generation path must be a directory"));
                }
                count = count
                    .checked_add(page_blob_file_count(&generation.path())?)
                    .ok_or_else(|| anyhow!("account blob file count overflow"))?;
            }
        }
    }
    Ok(count)
}

fn enforce_storage_quota(
    page_bytes: u64,
    user_bytes: u64,
    replaced_bytes: u64,
    added_bytes: u64,
) -> Result<()> {
    let projected_page = page_bytes
        .saturating_sub(replaced_bytes)
        .checked_add(added_bytes)
        .ok_or_else(|| anyhow!("page-data size overflow"))?;
    if projected_page > MAX_MUTABLE_BYTES_PER_PAGE {
        return Err(anyhow!(
            "page mutable storage exceeds the {} byte quota",
            MAX_MUTABLE_BYTES_PER_PAGE
        ));
    }
    let projected_user = user_bytes
        .saturating_sub(replaced_bytes)
        .checked_add(added_bytes)
        .ok_or_else(|| anyhow!("account page-data size overflow"))?;
    if projected_user > MAX_MUTABLE_BYTES_PER_USER {
        return Err(anyhow!(
            "account mutable storage exceeds the {} byte quota",
            MAX_MUTABLE_BYTES_PER_USER
        ));
    }
    Ok(())
}

fn enforce_current_storage_quota(
    store: &PageDataStore,
    user_id: &str,
    slug: &str,
    generation: &str,
) -> Result<()> {
    enforce_storage_quota(
        directory_size(&store.page_dir(user_id, slug, generation))?,
        directory_size(&store.base_dir.join(user_id))?,
        0,
        0,
    )
}

fn validate_db_input(sql: &str, params_json: &str) -> Result<()> {
    if sql.trim().is_empty() || sql.len() > MAX_DB_SQL_BYTES {
        return Err(anyhow!(
            "SQL must be non-empty and at most {} bytes",
            MAX_DB_SQL_BYTES
        ));
    }
    if params_json.len() > MAX_DB_PARAMS_BYTES {
        return Err(anyhow!(
            "SQL parameters exceed the {} byte limit",
            MAX_DB_PARAMS_BYTES
        ));
    }
    Ok(())
}

fn parse_db_params(params_json: &str) -> Result<Vec<serde_json::Value>> {
    let params: Vec<serde_json::Value> =
        serde_json::from_str(params_json).map_err(|e| anyhow!("invalid SQL parameters: {e}"))?;
    if params.len() > MAX_DB_PARAMS {
        return Err(anyhow!(
            "SQL parameters exceed the {} item limit",
            MAX_DB_PARAMS
        ));
    }
    Ok(params)
}

fn configure_connection(conn: &rusqlite::Connection) {
    conn.set_limit(Limit::SQLITE_LIMIT_LENGTH, MAX_DB_VALUE_BYTES);
    conn.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, MAX_DB_SQL_BYTES as i32);
    conn.set_limit(Limit::SQLITE_LIMIT_COLUMN, 128);
    conn.set_limit(Limit::SQLITE_LIMIT_EXPR_DEPTH, 64);
    conn.set_limit(Limit::SQLITE_LIMIT_COMPOUND_SELECT, 16);
    conn.set_limit(Limit::SQLITE_LIMIT_FUNCTION_ARG, 32);
    conn.set_limit(Limit::SQLITE_LIMIT_ATTACHED, 0);
    conn.set_limit(Limit::SQLITE_LIMIT_VARIABLE_NUMBER, MAX_DB_PARAMS as i32);
    conn.set_limit(Limit::SQLITE_LIMIT_TRIGGER_DEPTH, 8);
    conn.set_limit(Limit::SQLITE_LIMIT_WORKER_THREADS, 0);
    let started = Instant::now();
    conn.progress_handler(
        1_000,
        Some(move || started.elapsed() >= MAX_DB_OPERATION_TIME),
    );
    let _ = conn.busy_timeout(Duration::from_millis(250));
}

fn configure_database_quota(
    store: &PageDataStore,
    conn: &rusqlite::Connection,
    user_id: &str,
    slug: &str,
    generation: &str,
    db_path: &Path,
) -> Result<()> {
    let page_bytes = directory_size(&store.page_dir(user_id, slug, generation))?;
    let user_bytes = directory_size(&store.base_dir.join(user_id))?;
    enforce_storage_quota(page_bytes, user_bytes, 0, 0)?;

    let current_db_bytes = file_len(db_path);
    if current_db_bytes > MAX_PAGE_DB_BYTES {
        return Err(anyhow!("page database already exceeds its quota"));
    }
    let remaining_page = MAX_MUTABLE_BYTES_PER_PAGE.saturating_sub(page_bytes);
    let remaining_user = MAX_MUTABLE_BYTES_PER_USER.saturating_sub(user_bytes);
    let allowed_db_bytes =
        MAX_PAGE_DB_BYTES.min(current_db_bytes.saturating_add(remaining_page.min(remaining_user)));
    let page_size: u64 = conn
        .query_row("PRAGMA page_size", [], |row| row.get(0))
        .map_err(|e| anyhow!("read database page size: {e}"))?;
    let max_pages = (allowed_db_bytes / page_size).max(1);
    let _: u64 = conn
        .query_row(&format!("PRAGMA max_page_count = {max_pages}"), [], |row| {
            row.get(0)
        })
        .map_err(|e| anyhow!("set database quota: {e}"))?;
    Ok(())
}

fn database_logical_bytes(conn: &rusqlite::Connection) -> Result<u64> {
    let page_count: u64 = conn
        .query_row("PRAGMA page_count", [], |row| row.get(0))
        .map_err(|e| anyhow!("read database page count: {e}"))?;
    let page_size: u64 = conn
        .query_row("PRAGMA page_size", [], |row| row.get(0))
        .map_err(|e| anyhow!("read database page size: {e}"))?;
    Ok(page_count.saturating_mul(page_size))
}

fn authorize_execute(context: AuthContext<'_>) -> Authorization {
    match context.action {
        AuthAction::Attach { .. }
        | AuthAction::Detach { .. }
        | AuthAction::Pragma { .. }
        | AuthAction::Transaction { .. }
        | AuthAction::Savepoint { .. }
        | AuthAction::CreateTempIndex { .. }
        | AuthAction::CreateTempTable { .. }
        | AuthAction::CreateTempTrigger { .. }
        | AuthAction::CreateTempView { .. }
        | AuthAction::DropTempIndex { .. }
        | AuthAction::DropTempTable { .. }
        | AuthAction::DropTempTrigger { .. }
        | AuthAction::DropTempView { .. }
        | AuthAction::CreateVtable { .. }
        | AuthAction::DropVtable { .. }
        | AuthAction::Analyze { .. }
        | AuthAction::Reindex { .. }
        | AuthAction::Unknown { .. } => Authorization::Deny,
        AuthAction::Function { function_name } if forbidden_db_function(function_name) => {
            Authorization::Deny
        }
        _ => Authorization::Allow,
    }
}

fn authorize_query(context: AuthContext<'_>) -> Authorization {
    match context.action {
        AuthAction::Read { .. } | AuthAction::Select | AuthAction::Recursive => {
            Authorization::Allow
        }
        AuthAction::Function { function_name } if !forbidden_db_function(function_name) => {
            Authorization::Allow
        }
        _ => Authorization::Deny,
    }
}

fn forbidden_db_function(function_name: &str) -> bool {
    matches!(
        function_name.to_ascii_lowercase().as_str(),
        "load_extension" | "readfile" | "writefile" | "edit" | "shell"
    )
}

fn json_to_sql(v: &serde_json::Value) -> rusqlite::types::Value {
    match v {
        serde_json::Value::Null => rusqlite::types::Value::Null,
        serde_json::Value::Bool(b) => rusqlite::types::Value::Integer(i64::from(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                rusqlite::types::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                rusqlite::types::Value::Real(f)
            } else {
                rusqlite::types::Value::Text(n.to_string())
            }
        }
        serde_json::Value::String(s) => rusqlite::types::Value::Text(s.clone()),
        other => rusqlite::types::Value::Text(other.to_string()),
    }
}

/// Sync PageHost bridging account DB KV + page-data store + version assets.
pub struct RelayPageHost {
    pub db: Arc<DbPool>,
    pub page_data: PageDataStore,
    pub user_id: String,
    pub slug: String,
    pub generation: String,
    pub meta: PageMeta,
    pub asset_store: Arc<dyn crate::WebAssetStore>,
    pub asset_key: String,
}

impl RelayPageHost {
    fn block_on_kv<T>(
        &self,
        fut: impl std::future::Future<Output = Result<T>>,
    ) -> Result<T, String> {
        match Handle::try_current() {
            Ok(handle) => {
                tokio::task::block_in_place(|| handle.block_on(fut)).map_err(|e| e.to_string())
            }
            Err(_) => {
                // Outside tokio (unit tests): create a tiny runtime.
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| e.to_string())?;
                rt.block_on(fut).map_err(|e| e.to_string())
            }
        }
    }

    fn ensure_current_page(&self) -> Result<(), String> {
        let db = Arc::clone(&self.db);
        let user_id = self.user_id.clone();
        let slug = self.slug.clone();
        let generation = self.generation.clone();
        self.block_on_kv(async move {
            let page = PageRow::get(&db, &user_id, &slug).await?;
            if page.is_some_and(|page| page.generation == generation) {
                Ok(())
            } else {
                Err(anyhow!("page no longer exists"))
            }
        })
    }
}

impl PageHost for RelayPageHost {
    fn kv_get(&self, key: &str) -> Result<Option<String>, String> {
        let db = Arc::clone(&self.db);
        let user_id = self.user_id.clone();
        let slug = self.slug.clone();
        let key = key.to_string();
        self.block_on_kv(async move { page_kv::get(&db, &user_id, &slug, &key).await })
    }

    fn kv_put(&self, key: &str, value: &str) -> Result<(), String> {
        self.ensure_current_page()?;
        let lock = self.page_data.user_mutation_lock(&self.user_id);
        let _guard = lock
            .lock()
            .map_err(|_| "page-data mutation lock poisoned".to_string())?;
        let db = Arc::clone(&self.db);
        let user_id = self.user_id.clone();
        let slug = self.slug.clone();
        let key = key.to_string();
        let value = value.to_string();
        self.block_on_kv(async move { page_kv::put(&db, &user_id, &slug, &key, &value).await })
    }

    fn kv_delete(&self, key: &str) -> Result<bool, String> {
        self.ensure_current_page()?;
        let db = Arc::clone(&self.db);
        let user_id = self.user_id.clone();
        let slug = self.slug.clone();
        let key = key.to_string();
        self.block_on_kv(async move { page_kv::delete(&db, &user_id, &slug, &key).await })
    }

    fn kv_list(&self) -> Result<Vec<String>, String> {
        let db = Arc::clone(&self.db);
        let user_id = self.user_id.clone();
        let slug = self.slug.clone();
        self.block_on_kv(async move { page_kv::list_keys(&db, &user_id, &slug).await })
    }

    fn db_execute(&self, sql: &str, params_json: &str) -> Result<String, String> {
        self.ensure_current_page()?;
        self.page_data
            .db_execute(
                &self.user_id,
                &self.slug,
                &self.generation,
                sql,
                params_json,
            )
            .map_err(|e| e.to_string())
    }

    fn db_query(&self, sql: &str, params_json: &str) -> Result<String, String> {
        self.page_data
            .db_query(
                &self.user_id,
                &self.slug,
                &self.generation,
                sql,
                params_json,
            )
            .map_err(|e| e.to_string())
    }

    fn blob_put(&self, blob_id: &str, content_type: &str, data_b64: &str) -> Result<(), String> {
        self.ensure_current_page()?;
        const MAX_ENCODED_BLOB_BYTES: usize = MAX_BLOB_BYTES.div_ceil(3) * 4;
        if data_b64.len() > MAX_ENCODED_BLOB_BYTES {
            return Err(format!(
                "encoded blob exceeds the {MAX_ENCODED_BLOB_BYTES} byte operation limit"
            ));
        }
        let data = B64.decode(data_b64).map_err(|e| e.to_string())?;
        self.page_data
            .blob_put(
                &self.user_id,
                &self.slug,
                &self.generation,
                blob_id,
                content_type,
                &data,
            )
            .map_err(|e| e.to_string())
    }

    fn blob_get(&self, blob_id: &str) -> Result<Option<(String, String)>, String> {
        match self
            .page_data
            .blob_get(&self.user_id, &self.slug, &self.generation, blob_id)
            .map_err(|e| e.to_string())?
        {
            Some((ct, data)) => Ok(Some((ct, B64.encode(data)))),
            None => Ok(None),
        }
    }

    fn blob_delete(&self, blob_id: &str) -> Result<bool, String> {
        self.ensure_current_page()?;
        self.page_data
            .blob_delete(&self.user_id, &self.slug, &self.generation, blob_id)
            .map_err(|e| e.to_string())
    }

    fn assets_get(&self, path: &str) -> Result<Option<(String, Vec<u8>)>, String> {
        let path = path.trim_start_matches('/');
        if path.contains("..") {
            return Err("invalid path".into());
        }
        let bytes = self.asset_store.get_file_exact(&self.asset_key, path);
        Ok(bytes.map(|b| {
            let ct = mime_from_path(path).to_string();
            (ct, b)
        }))
    }

    fn page_meta(&self) -> PageMeta {
        self.meta.clone()
    }
}

fn mime_from_path(p: &str) -> &'static str {
    match p.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

pub fn default_page_data_dir(room_web_dir: &Path) -> PathBuf {
    room_web_dir.join("page-data")
}

#[cfg(test)]
mod tests {
    use super::*;

    const GENERATION_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const GENERATION_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[test]
    fn storage_projection_enforces_page_and_account_quotas() {
        assert!(enforce_storage_quota(
            MAX_MUTABLE_BYTES_PER_PAGE,
            MAX_MUTABLE_BYTES_PER_PAGE,
            10,
            10,
        )
        .is_ok());
        assert!(enforce_storage_quota(
            MAX_MUTABLE_BYTES_PER_PAGE,
            MAX_MUTABLE_BYTES_PER_PAGE,
            0,
            1,
        )
        .is_err());
        assert!(enforce_storage_quota(0, MAX_MUTABLE_BYTES_PER_USER, 0, 1,).is_err());
    }

    #[test]
    fn blob_limits_are_enforced_and_metadata_names_do_not_collide() {
        let temp = tempfile::tempdir().unwrap();
        let store = PageDataStore::new(temp.path());
        let oversized = vec![0u8; MAX_BLOB_BYTES + 1];
        assert!(store
            .blob_put(
                "u1",
                "site",
                GENERATION_A,
                "too-big",
                "application/octet-stream",
                &oversized
            )
            .unwrap_err()
            .to_string()
            .contains("operation limit"));

        store
            .blob_put("u1", "site", GENERATION_A, "item", "text/plain", b"first")
            .unwrap();
        store
            .blob_put(
                "u1",
                "site",
                GENERATION_A,
                "item.meta",
                "application/octet-stream",
                b"second",
            )
            .unwrap();
        assert_eq!(
            store
                .blob_get("u1", "site", GENERATION_A, "item")
                .unwrap()
                .unwrap(),
            ("text/plain".to_string(), b"first".to_vec())
        );
        assert_eq!(
            store
                .blob_get("u1", "site", GENERATION_A, "item.meta")
                .unwrap()
                .unwrap(),
            ("application/octet-stream".to_string(), b"second".to_vec())
        );
    }

    #[test]
    fn blob_ids_use_portable_collision_free_internal_names() {
        let temp = tempfile::tempdir().unwrap();
        let store = PageDataStore::new(temp.path());
        for (id, value) in [
            ("A", b"upper".as_slice()),
            ("a", b"lower".as_slice()),
            ("é", b"composed".as_slice()),
            ("e\u{301}", b"decomposed".as_slice()),
        ] {
            store
                .blob_put("u1", "site", GENERATION_A, id, "text/plain", value)
                .unwrap();
        }
        for (id, expected) in [
            ("A", b"upper".as_slice()),
            ("a", b"lower".as_slice()),
            ("é", b"composed".as_slice()),
            ("e\u{301}", b"decomposed".as_slice()),
        ] {
            assert_eq!(
                store
                    .blob_get("u1", "site", GENERATION_A, id)
                    .unwrap()
                    .unwrap()
                    .1,
                expected
            );
        }
        assert_ne!(blob_storage_name("A"), blob_storage_name("a"));
        assert_ne!(blob_storage_name("é"), blob_storage_name("e\u{301}"));
    }

    #[test]
    fn legacy_blob_layout_is_lazily_migrated_and_remains_mutable() {
        let temp = tempfile::tempdir().unwrap();
        let store = PageDataStore::new(temp.path());
        let legacy_blobs = temp.path().join("u1/site/blobs");
        std::fs::create_dir_all(legacy_blobs.join(".metadata")).unwrap();
        std::fs::write(legacy_blobs.join("item"), b"legacy").unwrap();
        std::fs::write(legacy_blobs.join(".metadata/item"), "text/plain").unwrap();

        assert_eq!(
            store
                .blob_get("u1", "site", GENERATION_A, "item")
                .unwrap()
                .unwrap(),
            ("text/plain".to_string(), b"legacy".to_vec())
        );
        let migrated_blobs = store.blobs_dir("u1", "site", GENERATION_A);
        assert!(!store
            .page_dir("u1", "site", GENERATION_A)
            .join("blobs/item")
            .exists());
        assert!(migrated_blobs.join(blob_storage_name("item")).is_file());
        assert!(store
            .page_dir("u1", "site", GENERATION_A)
            .join(LEGACY_BLOB_LAYOUT_MARKER)
            .is_file());

        // Repeating migration is a no-op; writes and deletes continue through
        // the portable storage name after the upgrade.
        assert!(store
            .blob_get("u1", "site", GENERATION_A, "item")
            .unwrap()
            .is_some());
        store
            .blob_put(
                "u1",
                "site",
                GENERATION_A,
                "item",
                "application/json",
                b"{}",
            )
            .unwrap();
        assert_eq!(
            store
                .blob_get("u1", "site", GENERATION_A, "item")
                .unwrap()
                .unwrap(),
            ("application/json".to_string(), b"{}".to_vec())
        );
        assert!(store
            .blob_delete("u1", "site", GENERATION_A, "item")
            .unwrap());
        assert!(store
            .blob_get("u1", "site", GENERATION_A, "item")
            .unwrap()
            .is_none());
    }

    #[test]
    fn legacy_sibling_metadata_is_read_without_destroying_another_possible_blob() {
        let temp = tempfile::tempdir().unwrap();
        let store = PageDataStore::new(temp.path());
        let legacy_blobs = temp.path().join("u1/site/blobs");
        std::fs::create_dir_all(&legacy_blobs).unwrap();
        std::fs::write(legacy_blobs.join("item"), b"legacy").unwrap();
        std::fs::write(legacy_blobs.join("item.meta"), "text/custom").unwrap();

        assert_eq!(
            store
                .blob_get("u1", "site", GENERATION_A, "item")
                .unwrap()
                .unwrap(),
            ("text/custom".to_string(), b"legacy".to_vec())
        );
        let legacy_sibling = store
            .page_dir("u1", "site", GENERATION_A)
            .join("blobs/item.meta");
        assert!(legacy_sibling.is_file());

        store
            .blob_put("u1", "site", GENERATION_A, "item", "text/new", b"new")
            .unwrap();
        assert_eq!(
            store
                .blob_get("u1", "site", GENERATION_A, "item")
                .unwrap()
                .unwrap()
                .0,
            "text/new"
        );
        assert!(store
            .blob_delete("u1", "site", GENERATION_A, "item")
            .unwrap());
        assert!(legacy_sibling.is_file());
    }

    #[test]
    fn legacy_blob_conflict_never_overwrites_different_hashed_data() {
        let temp = tempfile::tempdir().unwrap();
        let store = PageDataStore::new(temp.path());
        let legacy_blobs = temp.path().join("u1/site/blobs");
        std::fs::create_dir_all(&legacy_blobs).unwrap();
        let hashed_name = blob_storage_name("item");
        std::fs::write(legacy_blobs.join("item"), b"legacy").unwrap();
        let current_blobs = temp
            .path()
            .join("u1/site")
            .join(GENERATION_A)
            .join("blobs-v2");
        std::fs::create_dir_all(&current_blobs).unwrap();
        std::fs::write(current_blobs.join(&hashed_name), b"current").unwrap();

        let error = store
            .blob_get("u1", "site", GENERATION_A, "item")
            .unwrap_err();
        assert!(error.to_string().contains("conflicts"));
        let migrated_blobs = store.blobs_dir("u1", "site", GENERATION_A);
        assert_eq!(
            std::fs::read(
                store
                    .page_dir("u1", "site", GENERATION_A)
                    .join("blobs/item")
            )
            .unwrap(),
            b"legacy"
        );
        assert_eq!(
            std::fs::read(migrated_blobs.join(hashed_name)).unwrap(),
            b"current"
        );
    }

    #[test]
    fn legacy_id_equal_to_another_ids_hash_stays_isolated_in_v2_storage() {
        let temp = tempfile::tempdir().unwrap();
        let store = PageDataStore::new(temp.path());
        let id_b = "B";
        let id_a = blob_storage_name(id_b);
        let legacy_blobs = temp.path().join("u1/site/blobs");
        std::fs::create_dir_all(legacy_blobs.join(".metadata")).unwrap();
        std::fs::write(legacy_blobs.join(&id_a), b"legacy-a").unwrap();
        std::fs::write(legacy_blobs.join(".metadata").join(&id_a), "text/a").unwrap();

        store
            .blob_put("u1", "site", GENERATION_A, id_b, "text/b", b"current-b")
            .unwrap();
        let page_dir = store.page_dir("u1", "site", GENERATION_A);
        assert_eq!(page_blob_file_count(&page_dir).unwrap(), 2);
        assert_eq!(
            store
                .blob_get("u1", "site", GENERATION_A, &id_a)
                .unwrap()
                .unwrap(),
            ("text/a".to_string(), b"legacy-a".to_vec())
        );
        assert_eq!(
            store
                .blob_get("u1", "site", GENERATION_A, id_b)
                .unwrap()
                .unwrap(),
            ("text/b".to_string(), b"current-b".to_vec())
        );
        assert_eq!(page_blob_file_count(&page_dir).unwrap(), 2);

        assert!(store.blob_delete("u1", "site", GENERATION_A, id_b).unwrap());
        assert!(store
            .blob_get("u1", "site", GENERATION_A, id_b)
            .unwrap()
            .is_none());
        assert_eq!(
            store
                .blob_get("u1", "site", GENERATION_A, &id_a)
                .unwrap()
                .unwrap()
                .1,
            b"legacy-a"
        );
        assert!(store
            .blob_delete("u1", "site", GENERATION_A, &id_a)
            .unwrap());
    }

    #[test]
    fn ids_that_were_not_legacy_filenames_use_only_hashed_storage() {
        let temp = tempfile::tempdir().unwrap();
        let store = PageDataStore::new(temp.path());
        store
            .blob_put(
                "u1",
                "site",
                GENERATION_A,
                "folder/item:portable",
                "text/plain",
                b"value",
            )
            .unwrap();
        let blobs = store.blobs_dir("u1", "site", GENERATION_A);
        assert!(blobs
            .join(blob_storage_name("folder/item:portable"))
            .is_file());
        assert!(!is_legacy_blob_id("folder/item:portable"));
    }

    #[test]
    fn database_denies_file_attachment_and_caps_query_rows() {
        let temp = tempfile::tempdir().unwrap();
        let store = PageDataStore::new(temp.path().join("page-data"));
        let attached = temp.path().join("outside.sqlite");
        let error = store
            .db_execute(
                "u1",
                "site",
                GENERATION_A,
                &format!("ATTACH DATABASE '{}' AS outside", attached.display()),
                "[]",
            )
            .unwrap_err();
        assert!(
            error.to_string().contains("not authorized")
                || error.to_string().contains("too many attached")
        );
        assert!(!attached.exists());

        let error = store
            .db_query(
                "u1",
                "site",
                GENERATION_A,
                "WITH RECURSIVE items(n) AS (VALUES(1) UNION ALL SELECT n + 1 FROM items WHERE n <= 1000) SELECT n FROM items",
                "[]",
            )
            .unwrap_err();
        assert!(error.to_string().contains("row result limit"));
    }

    #[test]
    fn database_execute_accepts_normal_schema_and_data_changes() {
        let temp = tempfile::tempdir().unwrap();
        let store = PageDataStore::new(temp.path());
        store
            .db_execute(
                "u1",
                "site",
                GENERATION_A,
                "CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT)",
                "[]",
            )
            .unwrap();
        store
            .db_execute(
                "u1",
                "site",
                GENERATION_A,
                "INSERT INTO notes (body) VALUES (?)",
                r#"["hello"]"#,
            )
            .unwrap();
        let output = store
            .db_query("u1", "site", GENERATION_A, "SELECT body FROM notes", "[]")
            .unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(json["rows"][0]["body"], "hello");
    }

    #[test]
    fn delete_recreate_generation_cannot_observe_old_page_data() {
        let temp = tempfile::tempdir().unwrap();
        let store = PageDataStore::new(temp.path());
        store
            .blob_put("u1", "site", GENERATION_A, "secret", "text/plain", b"old")
            .unwrap();
        store
            .db_execute(
                "u1",
                "site",
                GENERATION_A,
                "CREATE TABLE old_data (value TEXT)",
                "[]",
            )
            .unwrap();

        // Simulate a process crash after relational deletion but before the
        // best-effort filesystem cleanup. The recreated Page gets generation B.
        assert!(store
            .blob_get("u1", "site", GENERATION_B, "secret")
            .unwrap()
            .is_none());
        let output = store
            .db_query(
                "u1",
                "site",
                GENERATION_B,
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'old_data'",
                "[]",
            )
            .unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(json["rows"], serde_json::json!([]));
        assert!(store.page_dir("u1", "site", GENERATION_A).exists());
        assert!(store.page_dir("u1", "site", GENERATION_B).exists());
    }

    #[test]
    fn database_long_running_query_is_interrupted() {
        let temp = tempfile::tempdir().unwrap();
        let store = PageDataStore::new(temp.path());
        let started = Instant::now();
        let error = store
            .db_query(
                "u1",
                "site",
                GENERATION_A,
                "WITH RECURSIVE items(n) AS (VALUES(1) UNION ALL SELECT n + 1 FROM items WHERE n < 1000000000) SELECT sum(n) FROM items",
                "[]",
            )
            .unwrap_err();
        assert!(error.to_string().contains("interrupted"));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
