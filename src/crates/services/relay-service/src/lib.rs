//! BitFun Relay Service
//!
//! Shared relay logic used by both the standalone relay-server binary and
//! the embedded relay running inside the desktop process.
//!
//! The relay is a stateless HTTP-to-WebSocket bridge:
//!   - Desktop clients connect via WebSocket
//!   - Mobile clients interact via HTTP POST
//!   - The relay forwards encrypted payloads without inspection
//!   - Per-room mobile-web static files are managed via `WebAssetStore`

pub mod admin;
pub mod db;
pub mod page_data;
pub mod relay;
pub mod routes;

pub use relay::room::{ResponsePayload, RoomManager};
pub use routes::api::AppState;

use axum::extract::DefaultBodyLimit;
use axum::http::{header, HeaderValue, Method};
use axum::routing::{get, post};
use axum::Router;
use dashmap::DashMap;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

const DEFAULT_MEMORY_ASSET_STORE_MAX_BYTES: u64 = 256 * 1024 * 1024;
pub const DEFAULT_DISK_ASSET_STORE_MAX_BYTES: u64 = 1024 * 1024 * 1024;
const ASSET_CAPACITY_ERROR: &str = "relay asset store capacity exceeded";

pub(crate) fn asset_store_error_status(error: String) -> axum::http::StatusCode {
    if error == ASSET_CAPACITY_ERROR {
        tracing::warn!("Relay asset upload rejected because the configured capacity is full");
        axum::http::StatusCode::INSUFFICIENT_STORAGE
    } else {
        tracing::error!("Relay asset store operation failed: {error}");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    }
}

/// Apply conservative browser security headers to API and hosted-web
/// responses. Operators can still add a stricter CSP at their reverse proxy;
/// a global CSP here would incorrectly constrain user-authored BitFun Pages.
pub async fn relay_security_headers(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::http::{HeaderName, HeaderValue};

    let no_store = request.uri().path() == "/health" || request.uri().path().starts_with("/api/");
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    for (name, value) in [
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "SAMEORIGIN"),
        ("referrer-policy", "no-referrer"),
        (
            "permissions-policy",
            "camera=(), microphone=(), geolocation=()",
        ),
        ("strict-transport-security", "max-age=31536000"),
    ] {
        headers
            .entry(HeaderName::from_static(name))
            .or_insert(HeaderValue::from_static(value));
    }
    if no_store {
        headers
            .entry(axum::http::header::CACHE_CONTROL)
            .or_insert(HeaderValue::from_static("no-store"));
    }
    response
}

/// Validate a caller-controlled asset namespace or file path before it is
/// joined beneath the relay's asset root.
///
/// `PathBuf::join` discards its left-hand side when the right-hand side is
/// absolute. Rejecting every non-normal component here is therefore a security
/// boundary, not just input cleanup. Backslashes are rejected on every host so
/// a path accepted on Unix cannot become an escape after moving the same data
/// to Windows.
pub(crate) fn validated_asset_relative_path(raw: &str) -> Result<PathBuf, String> {
    if raw.is_empty() || raw.len() > 1024 || raw.contains('\\') || raw.chars().any(char::is_control)
    {
        return Err("asset path must be a non-empty portable relative path".to_string());
    }

    let path = Path::new(raw);
    if path.is_absolute() {
        return Err("absolute asset paths are not allowed".to_string());
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) if value.as_encoded_bytes().len() <= 255 => {
                normalized.push(value)
            }
            Component::Normal(_) => return Err("asset path segment is too long".to_string()),
            Component::Prefix(_)
            | Component::RootDir
            | Component::CurDir
            | Component::ParentDir => {
                return Err("asset path contains a forbidden component".to_string())
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err("asset path must not be empty".to_string());
    }
    Ok(normalized)
}

fn validated_asset_namespace(raw: &str) -> Result<PathBuf, String> {
    let namespace = validated_asset_relative_path(raw)?;
    if matches!(raw, "_store" | "page-data" | "pages") {
        return Err("asset namespace is reserved by the relay".to_string());
    }
    Ok(namespace)
}

pub(crate) fn is_valid_content_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

pub(crate) fn normalized_browser_origin(raw: &str) -> Option<String> {
    let value = raw.trim().trim_end_matches('/');
    if value == "*" {
        return Some(value.to_string());
    }
    let uri = value.parse::<axum::http::Uri>().ok()?;
    let scheme = uri.scheme_str()?;
    if !matches!(scheme, "http" | "https") || uri.query().is_some() {
        return None;
    }
    let authority = uri.authority()?.as_str();
    if authority.contains('@') || !matches!(uri.path(), "" | "/") {
        return None;
    }
    Some(format!("{scheme}://{authority}"))
}

fn content_matches_hash(hash: &str, data: &[u8]) -> bool {
    use sha2::{Digest, Sha256};

    if !is_valid_content_hash(hash) {
        return false;
    }
    let actual = Sha256::digest(data);
    format!("{actual:x}") == hash
}

// ── WebAssetStore trait ───────────────────────────────────────────────

/// Abstract storage for per-room mobile-web static assets.
///
/// The standalone relay uses `DiskAssetStore` (filesystem-backed), while
/// the embedded relay uses `MemoryAssetStore` (in-memory DashMap-backed).
pub trait WebAssetStore: Send + Sync + 'static {
    /// Total content-addressed bytes currently retained by this process/store.
    fn stored_bytes(&self) -> u64;

    /// Configured global content-addressed capacity.
    fn max_store_bytes(&self) -> u64;

    /// Check if content with this SHA-256 hash exists in the store.
    fn has_content(&self, hash: &str) -> bool;

    /// Store content by its SHA-256 hash. No-op if already present.
    fn store_content(&self, hash: &str, data: Vec<u8>) -> Result<(), String>;

    /// Associate a relative file path within a room to a stored content hash.
    fn map_to_room(&self, room_id: &str, rel_path: &str, hash: &str) -> Result<(), String>;

    /// Retrieve file content for serving. Falls back to `index.html` if the
    /// requested path doesn't exist (SPA routing).
    fn get_file(&self, room_id: &str, path: &str) -> Option<Vec<u8>>;

    /// Retrieve file content for an exact path (no SPA index.html fallback).
    fn get_file_exact(&self, room_id: &str, path: &str) -> Option<Vec<u8>>;

    /// List `(rel_path, content_hash)` entries in a room manifest.
    fn list_room_entries(&self, room_id: &str) -> Vec<(String, String)>;

    /// Total size in bytes of all files mapped in a room.
    fn room_total_bytes(&self, room_id: &str) -> u64;

    /// Copy all path→hash mappings from one room namespace to another.
    fn copy_room(&self, from_room_id: &str, to_room_id: &str) -> Result<(), String>;

    /// Check if any web files have been uploaded for this room.
    fn has_room_files(&self, room_id: &str) -> bool;

    /// Remove all uploaded web files for a room.
    fn cleanup_room(&self, room_id: &str);
}

// ── MemoryAssetStore ──────────────────────────────────────────────────

/// In-memory asset store backed by DashMap. Used by the embedded relay.
pub struct MemoryAssetStore {
    content_store: DashMap<String, Arc<Vec<u8>>>,
    room_manifests: DashMap<String, HashMap<String, String>>,
    stored_bytes: std::sync::Mutex<u64>,
    mutation_lock: std::sync::Mutex<()>,
    max_store_bytes: u64,
}

impl MemoryAssetStore {
    pub fn new() -> Self {
        Self::new_with_max_bytes(DEFAULT_MEMORY_ASSET_STORE_MAX_BYTES)
    }

    pub fn new_with_max_bytes(max_store_bytes: u64) -> Self {
        Self {
            content_store: DashMap::new(),
            room_manifests: DashMap::new(),
            stored_bytes: std::sync::Mutex::new(0),
            mutation_lock: std::sync::Mutex::new(()),
            max_store_bytes,
        }
    }
}

impl Default for MemoryAssetStore {
    fn default() -> Self {
        Self::new()
    }
}

impl WebAssetStore for MemoryAssetStore {
    fn stored_bytes(&self) -> u64 {
        self.stored_bytes
            .lock()
            .map(|value| *value)
            .unwrap_or(u64::MAX)
    }

    fn max_store_bytes(&self) -> u64 {
        self.max_store_bytes
    }

    fn has_content(&self, hash: &str) -> bool {
        is_valid_content_hash(hash) && self.content_store.contains_key(hash)
    }

    fn store_content(&self, hash: &str, data: Vec<u8>) -> Result<(), String> {
        if !content_matches_hash(hash, &data) {
            return Err("content does not match its lowercase SHA-256 digest".to_string());
        }
        if self.content_store.contains_key(hash) {
            return Ok(());
        }
        let _mutation = self
            .mutation_lock
            .lock()
            .map_err(|_| "relay asset store mutation lock poisoned".to_string())?;
        let mut stored_bytes = self
            .stored_bytes
            .lock()
            .map_err(|_| "relay asset store accounting lock poisoned".to_string())?;
        if self.content_store.contains_key(hash) {
            return Ok(());
        }
        let new_total = stored_bytes
            .checked_add(data.len() as u64)
            .ok_or_else(|| ASSET_CAPACITY_ERROR.to_string())?;
        if new_total > self.max_store_bytes {
            return Err(ASSET_CAPACITY_ERROR.to_string());
        }
        self.content_store.insert(hash.to_string(), Arc::new(data));
        *stored_bytes = new_total;
        Ok(())
    }

    fn map_to_room(&self, room_id: &str, rel_path: &str, hash: &str) -> Result<(), String> {
        validated_asset_namespace(room_id)?;
        validated_asset_relative_path(rel_path)?;
        if !is_valid_content_hash(hash) {
            return Err("content hash must be a lowercase SHA-256 digest".to_string());
        }
        if !self.content_store.contains_key(hash) {
            return Err("content hash is not present in the relay store".to_string());
        }
        let _mutation = self
            .mutation_lock
            .lock()
            .map_err(|_| "relay asset store mutation lock poisoned".to_string())?;
        if !self.content_store.contains_key(hash) {
            return Err("content hash is not present in the relay store".to_string());
        }
        self.room_manifests
            .entry(room_id.to_string())
            .or_default()
            .insert(rel_path.to_string(), hash.to_string());
        Ok(())
    }

    fn get_file(&self, room_id: &str, path: &str) -> Option<Vec<u8>> {
        validated_asset_namespace(room_id).ok()?;
        validated_asset_relative_path(path).ok()?;
        let manifest = self.room_manifests.get(room_id)?;
        let hash = manifest.get(path).or_else(|| manifest.get("index.html"))?;
        let content = self.content_store.get(hash)?;
        Some(content.value().as_ref().clone())
    }

    fn get_file_exact(&self, room_id: &str, path: &str) -> Option<Vec<u8>> {
        validated_asset_namespace(room_id).ok()?;
        validated_asset_relative_path(path).ok()?;
        let manifest = self.room_manifests.get(room_id)?;
        let hash = manifest.get(path)?;
        let content = self.content_store.get(hash)?;
        Some(content.value().as_ref().clone())
    }

    fn list_room_entries(&self, room_id: &str) -> Vec<(String, String)> {
        if validated_asset_namespace(room_id).is_err() {
            return Vec::new();
        }
        self.room_manifests
            .get(room_id)
            .map(|m| m.iter().map(|(p, h)| (p.clone(), h.clone())).collect())
            .unwrap_or_default()
    }

    fn room_total_bytes(&self, room_id: &str) -> u64 {
        if validated_asset_namespace(room_id).is_err() {
            return 0;
        }
        self.room_manifests
            .get(room_id)
            .map(|m| {
                m.values()
                    .filter_map(|h| self.content_store.get(h).map(|c| c.len() as u64))
                    .sum()
            })
            .unwrap_or(0)
    }

    fn copy_room(&self, from_room_id: &str, to_room_id: &str) -> Result<(), String> {
        let entries = self.list_room_entries(from_room_id);
        if entries.is_empty() {
            return Err("source room has no files".to_string());
        }
        self.cleanup_room(to_room_id);
        for (path, hash) in entries {
            self.map_to_room(to_room_id, &path, &hash)?;
        }
        Ok(())
    }

    fn has_room_files(&self, room_id: &str) -> bool {
        if validated_asset_namespace(room_id).is_err() {
            return false;
        }
        self.room_manifests.contains_key(room_id)
    }

    fn cleanup_room(&self, room_id: &str) {
        if validated_asset_namespace(room_id).is_err() {
            return;
        }
        let Ok(_mutation) = self.mutation_lock.lock() else {
            tracing::warn!("Failed to lock in-memory relay asset store for cleanup");
            return;
        };
        let Some((_, removed_manifest)) = self.room_manifests.remove(room_id) else {
            return;
        };
        let candidates = removed_manifest.into_values().collect::<HashSet<_>>();
        let still_referenced = self
            .room_manifests
            .iter()
            .flat_map(|manifest| manifest.value().values().cloned().collect::<Vec<_>>())
            .collect::<HashSet<_>>();
        let mut reclaimed = 0u64;
        for hash in candidates.difference(&still_referenced) {
            if let Some((_, content)) = self.content_store.remove(hash) {
                reclaimed = reclaimed.saturating_add(content.len() as u64);
            }
        }
        if reclaimed > 0 {
            if let Ok(mut stored_bytes) = self.stored_bytes.lock() {
                *stored_bytes = stored_bytes.saturating_sub(reclaimed);
            }
            tracing::info!("Reclaimed {reclaimed} unreferenced in-memory relay asset bytes");
        }
    }
}

// ── DiskAssetStore ────────────────────────────────────────────────────

/// Filesystem-backed asset store. Used by the standalone relay server.
///
/// Content is stored in `{base_dir}/_store/{hash}` and symlinked into
/// per-room directories `{base_dir}/{room_id}/{path}`.
pub struct DiskAssetStore {
    base_dir: PathBuf,
    known_hashes: DashMap<String, u64>,
    stored_bytes: std::sync::Mutex<u64>,
    mutation_lock: std::sync::Mutex<()>,
    max_store_bytes: u64,
}

impl DiskAssetStore {
    pub fn new(base_dir: &str) -> Self {
        Self::new_with_max_bytes(base_dir, DEFAULT_DISK_ASSET_STORE_MAX_BYTES)
    }

    pub fn new_with_max_bytes(base_dir: &str, max_store_bytes: u64) -> Self {
        let requested_base = PathBuf::from(base_dir);
        if let Err(error) = std::fs::create_dir_all(&requested_base) {
            tracing::warn!("Failed to create relay asset root {base_dir}: {error}");
        }
        let base_dir = std::fs::canonicalize(&requested_base).unwrap_or_else(|_| {
            if requested_base.is_absolute() {
                requested_base
            } else {
                std::env::current_dir()
                    .unwrap_or_default()
                    .join(requested_base)
            }
        });
        let store_dir = base_dir.join("_store");
        if let Err(error) = std::fs::create_dir_all(&store_dir) {
            tracing::warn!(
                "Failed to create relay content store {}: {error}",
                store_dir.display()
            );
        }

        let known: DashMap<String, u64> = DashMap::new();
        if store_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&store_dir) {
                for entry in entries.flatten() {
                    if entry
                        .file_type()
                        .map(|file_type| file_type.is_file())
                        .unwrap_or(false)
                    {
                        if let Ok(meta) = entry.metadata() {
                            if let Some(name) = entry.file_name().to_str() {
                                if is_valid_content_hash(name) {
                                    known.insert(name.to_string(), meta.len());
                                }
                            }
                        }
                    }
                }
            }
        }
        tracing::info!(
            "DiskAssetStore initialized with {} entries ({} bytes, capacity {} bytes) from {}",
            known.len(),
            known.iter().map(|entry| *entry.value()).sum::<u64>(),
            max_store_bytes,
            base_dir.display()
        );
        let stored_bytes = known.iter().map(|entry| *entry.value()).sum();
        if stored_bytes > max_store_bytes {
            tracing::warn!(
                "Relay asset store already exceeds configured capacity; existing content remains readable but new content is blocked"
            );
        }
        Self {
            base_dir,
            known_hashes: known,
            stored_bytes: std::sync::Mutex::new(stored_bytes),
            mutation_lock: std::sync::Mutex::new(()),
            max_store_bytes,
        }
    }

    fn store_dir(&self) -> PathBuf {
        self.base_dir.join("_store")
    }

    fn safe_relative_path(&self, relative: PathBuf) -> Result<PathBuf, String> {
        let candidate = self.base_dir.join(relative);

        // Refuse traversal through a pre-existing symlink. This also protects
        // installations that may contain directories created by an older,
        // vulnerable relay version.
        let mut existing_ancestor = candidate.as_path();
        while !existing_ancestor.exists() {
            existing_ancestor = existing_ancestor
                .parent()
                .ok_or_else(|| "asset path has no existing ancestor".to_string())?;
        }
        let canonical_ancestor = std::fs::canonicalize(existing_ancestor)
            .map_err(|error| format!("canonicalize asset path: {error}"))?;
        if !canonical_ancestor.starts_with(&self.base_dir) {
            return Err("asset path escapes the configured asset root".to_string());
        }

        Ok(candidate)
    }

    fn room_dir(&self, room_id: &str) -> Result<PathBuf, String> {
        self.safe_relative_path(validated_asset_namespace(room_id)?)
    }

    fn room_file_path(&self, room_id: &str, rel_path: &str) -> Result<PathBuf, String> {
        let room = validated_asset_namespace(room_id)?;
        let file = validated_asset_relative_path(rel_path)?;
        self.safe_relative_path(room.join(file))
    }

    fn referenced_candidate_hashes(&self, candidates: &HashSet<String>) -> HashSet<String> {
        fn walk(
            dir: &Path,
            store_dir: &Path,
            page_data_dir: &Path,
            candidates: &HashSet<String>,
            referenced: &mut HashSet<String>,
        ) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path == store_dir || path == page_data_dir {
                    continue;
                }
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if file_type.is_dir() {
                    walk(&path, store_dir, page_data_dir, candidates, referenced);
                    continue;
                }

                let linked_hash = std::fs::canonicalize(&path).ok().and_then(|canonical| {
                    canonical
                        .strip_prefix(store_dir)
                        .ok()
                        .and_then(|relative| relative.file_name())
                        .and_then(|name| name.to_str())
                        .filter(|hash| candidates.contains(*hash))
                        .map(str::to_string)
                });
                if let Some(hash) = linked_hash {
                    referenced.insert(hash);
                    continue;
                }

                // Windows may use a hard link or copy instead of a symlink;
                // hash only candidate-backed regular files as a fallback.
                if file_type.is_file() {
                    if let Ok(bytes) = std::fs::read(&path) {
                        use sha2::{Digest, Sha256};
                        let hash = format!("{:x}", Sha256::digest(bytes));
                        if candidates.contains(&hash) {
                            referenced.insert(hash);
                        }
                    }
                }
            }
        }

        let mut referenced = HashSet::new();
        walk(
            &self.base_dir,
            &self.store_dir(),
            &self.base_dir.join("page-data"),
            candidates,
            &mut referenced,
        );
        referenced
    }
}

impl WebAssetStore for DiskAssetStore {
    fn stored_bytes(&self) -> u64 {
        self.stored_bytes
            .lock()
            .map(|value| *value)
            .unwrap_or(u64::MAX)
    }

    fn max_store_bytes(&self) -> u64 {
        self.max_store_bytes
    }

    fn has_content(&self, hash: &str) -> bool {
        is_valid_content_hash(hash) && self.known_hashes.contains_key(hash)
    }

    fn store_content(&self, hash: &str, data: Vec<u8>) -> Result<(), String> {
        if !content_matches_hash(hash, &data) {
            return Err("content does not match its lowercase SHA-256 digest".to_string());
        }
        if self.known_hashes.contains_key(hash) {
            return Ok(());
        }
        let _mutation = self
            .mutation_lock
            .lock()
            .map_err(|_| "relay asset store mutation lock poisoned".to_string())?;
        let mut stored_bytes = self
            .stored_bytes
            .lock()
            .map_err(|_| "relay asset store accounting lock poisoned".to_string())?;
        if self.known_hashes.contains_key(hash) {
            return Ok(());
        }
        let store_path = self.store_dir().join(hash);
        if store_path.is_file() {
            let size = std::fs::metadata(&store_path)
                .map_err(|error| error.to_string())?
                .len();
            self.known_hashes.insert(hash.to_string(), size);
            *stored_bytes = stored_bytes.saturating_add(size);
            return Ok(());
        }

        let new_total = stored_bytes
            .checked_add(data.len() as u64)
            .ok_or_else(|| ASSET_CAPACITY_ERROR.to_string())?;
        if new_total > self.max_store_bytes {
            return Err(ASSET_CAPACITY_ERROR.to_string());
        }

        let temp_path = self
            .store_dir()
            .join(format!(".{hash}.{}.tmp", uuid::Uuid::new_v4()));
        std::fs::write(&temp_path, &data).map_err(|e| e.to_string())?;
        if let Err(error) = std::fs::rename(&temp_path, &store_path) {
            let _ = std::fs::remove_file(&temp_path);
            return Err(error.to_string());
        }
        self.known_hashes
            .insert(hash.to_string(), data.len() as u64);
        *stored_bytes = new_total;
        Ok(())
    }

    fn map_to_room(&self, room_id: &str, rel_path: &str, hash: &str) -> Result<(), String> {
        if !is_valid_content_hash(hash) {
            return Err("content hash must be a lowercase SHA-256 digest".to_string());
        }
        if !self.has_content(hash) {
            return Err("content hash is not present in the relay store".to_string());
        }
        let _mutation = self
            .mutation_lock
            .lock()
            .map_err(|_| "relay asset store mutation lock poisoned".to_string())?;
        if !self.has_content(hash) {
            return Err("content hash is not present in the relay store".to_string());
        }
        let store_path = self.store_dir().join(hash);
        if !store_path.is_file() {
            return Err("content hash is not present in the relay store".to_string());
        }
        let dest = self.room_file_path(room_id, rel_path)?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        if dest.exists() || std::fs::symlink_metadata(&dest).is_ok() {
            std::fs::remove_file(&dest).map_err(|error| error.to_string())?;
        }
        create_link(&store_path, &dest).map_err(|e| e.to_string())
    }

    fn get_file(&self, room_id: &str, path: &str) -> Option<Vec<u8>> {
        self.room_dir(room_id).ok()?;
        let target = self.room_file_path(room_id, path).ok()?;
        let file = if target.is_file() {
            target
        } else {
            self.room_file_path(room_id, "index.html").ok()?
        };
        if file.is_file() {
            std::fs::read(&file).ok()
        } else {
            None
        }
    }

    fn get_file_exact(&self, room_id: &str, path: &str) -> Option<Vec<u8>> {
        let target = self.room_file_path(room_id, path).ok()?;
        if target.is_file() {
            std::fs::read(&target).ok()
        } else {
            None
        }
    }

    fn list_room_entries(&self, room_id: &str) -> Vec<(String, String)> {
        let Ok(room_dir) = self.room_dir(room_id) else {
            return Vec::new();
        };
        if !room_dir.is_dir() {
            return Vec::new();
        }
        let mut out = Vec::new();
        fn walk(
            base: &std::path::Path,
            dir: &std::path::Path,
            store_dir: &std::path::Path,
            out: &mut Vec<(String, String)>,
        ) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(base, &path, store_dir, out);
                } else if path.is_file() {
                    let rel = path
                        .strip_prefix(base)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    // Prefer content hash from symlink target name under _store.
                    let hash = std::fs::canonicalize(&path)
                        .ok()
                        .and_then(|canon| {
                            canon
                                .file_name()
                                .and_then(|n| n.to_str().map(|s| s.to_string()))
                                .filter(|name| store_dir.join(name).exists())
                        })
                        .unwrap_or_else(|| {
                            use sha2::{Digest, Sha256};
                            let bytes = std::fs::read(&path).unwrap_or_default();
                            let digest = Sha256::digest(&bytes);
                            digest.iter().map(|b| format!("{b:02x}")).collect()
                        });
                    out.push((rel, hash));
                }
            }
        }
        walk(&room_dir, &room_dir, &self.store_dir(), &mut out);
        out
    }

    fn copy_room(&self, from_room_id: &str, to_room_id: &str) -> Result<(), String> {
        let entries = self.list_room_entries(from_room_id);
        if entries.is_empty() {
            return Err("source room has no files".to_string());
        }
        self.cleanup_room(to_room_id);
        for (path, hash) in entries {
            if !self.has_content(&hash) {
                // Materialize content if hash was computed from file bytes.
                if let Some(bytes) = self.get_file_exact(from_room_id, &path) {
                    self.store_content(&hash, bytes)?;
                }
            }
            self.map_to_room(to_room_id, &path, &hash)?;
        }
        Ok(())
    }

    fn has_room_files(&self, room_id: &str) -> bool {
        self.room_dir(room_id)
            .map(|path| path.exists())
            .unwrap_or(false)
    }

    fn room_total_bytes(&self, room_id: &str) -> u64 {
        let Ok(room_dir) = self.room_dir(room_id) else {
            return 0;
        };
        if !room_dir.is_dir() {
            return 0;
        }
        fn walk(dir: &std::path::Path, total: &mut u64) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, total);
                } else if let Ok(meta) = std::fs::metadata(&path) {
                    if meta.is_file() {
                        *total = total.saturating_add(meta.len());
                    }
                }
            }
        }
        let mut total = 0u64;
        walk(&room_dir, &mut total);
        total
    }

    fn cleanup_room(&self, room_id: &str) {
        let Ok(dir) = self.room_dir(room_id) else {
            tracing::warn!("Rejected unsafe relay asset namespace during cleanup");
            return;
        };
        if dir == self.base_dir || dir == self.store_dir() {
            tracing::warn!("Refused to clean protected relay asset directory");
            return;
        }
        let Ok(_mutation) = self.mutation_lock.lock() else {
            tracing::warn!("Failed to lock disk relay asset store for cleanup");
            return;
        };
        let candidates = self
            .list_room_entries(room_id)
            .into_iter()
            .map(|(_, hash)| hash)
            .filter(|hash| is_valid_content_hash(hash))
            .collect::<HashSet<_>>();
        if dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&dir) {
                tracing::warn!("Failed to clean up room web dir {}: {e}", dir.display());
            } else {
                tracing::info!("Cleaned up room web dir for {room_id}");
            }
        }
        if candidates.is_empty() {
            return;
        }
        let still_referenced = self.referenced_candidate_hashes(&candidates);
        let mut reclaimed = 0u64;
        for hash in candidates.difference(&still_referenced) {
            let store_path = self.store_dir().join(hash);
            match std::fs::remove_file(&store_path) {
                Ok(()) => {
                    if let Some((_, size)) = self.known_hashes.remove(hash) {
                        reclaimed = reclaimed.saturating_add(size);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if let Some((_, size)) = self.known_hashes.remove(hash) {
                        reclaimed = reclaimed.saturating_add(size);
                    }
                }
                Err(error) => {
                    tracing::warn!("Failed to reclaim relay asset {hash}: {error}");
                }
            }
        }
        if reclaimed > 0 {
            if let Ok(mut stored_bytes) = self.stored_bytes.lock() {
                *stored_bytes = stored_bytes.saturating_sub(reclaimed);
            }
            tracing::info!("Reclaimed {reclaimed} unreferenced disk relay asset bytes");
        }
    }
}

#[cfg(test)]
mod asset_store_security_tests {
    use super::*;

    #[test]
    fn asset_paths_reject_absolute_and_non_normal_components() {
        for invalid in [
            "",
            "/tmp/relay-owned",
            "../outside",
            "room/../../outside",
            "./room",
            "room\\..\\outside",
        ] {
            assert!(
                validated_asset_relative_path(invalid).is_err(),
                "path should be rejected: {invalid}"
            );
        }
        assert!(validated_asset_relative_path("pages/user-1/site/v/v1").is_ok());
        assert!(validated_asset_relative_path("assets/app.js").is_ok());
        assert!(validated_asset_namespace("pages/user-1/site/v/v1").is_ok());
        for reserved in ["_store", "page-data", "pages"] {
            assert!(validated_asset_namespace(reserved).is_err());
        }
        assert!(is_valid_content_hash(&"a".repeat(64)));
        assert!(!is_valid_content_hash("../room/index.html"));
        assert!(!is_valid_content_hash(&"A".repeat(64)));
    }

    #[test]
    fn disk_asset_store_cannot_write_or_delete_outside_its_root() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_file = outside.path().join("keep.txt");
        std::fs::write(&outside_file, b"keep").unwrap();

        let store = DiskAssetStore::new(root.path().to_str().unwrap());
        use sha2::{Digest, Sha256};
        let hash = format!("{:x}", Sha256::digest(b"owned"));
        store.store_content(&hash, b"owned".to_vec()).unwrap();

        for reserved in ["_store", "page-data", "pages"] {
            assert!(store.map_to_room(reserved, &hash, &hash).is_err());
        }
        assert_eq!(
            std::fs::read(store.store_dir().join(&hash)).unwrap(),
            b"owned"
        );
        assert!(store
            .map_to_room("room-a", "index.html", "../outside/keep.txt")
            .is_err());
        assert!(store
            .map_to_room("room-a", outside_file.to_str().unwrap(), &hash)
            .is_err());
        assert!(store.map_to_room("../outside", "file.txt", &hash).is_err());
        store.cleanup_room(outside.path().to_str().unwrap());

        assert_eq!(std::fs::read(&outside_file).unwrap(), b"keep");
    }

    #[test]
    fn asset_stores_enforce_global_content_capacity_without_double_counting() {
        use sha2::{Digest, Sha256};

        let first = b"12345".to_vec();
        let second = b"67890".to_vec();
        let first_hash = format!("{:x}", Sha256::digest(&first));
        let second_hash = format!("{:x}", Sha256::digest(&second));

        let memory = MemoryAssetStore::new_with_max_bytes(first.len() as u64);
        memory.store_content(&first_hash, first.clone()).unwrap();
        memory.store_content(&first_hash, first.clone()).unwrap();
        memory
            .map_to_room("room-a", "index.html", &first_hash)
            .unwrap();
        memory
            .map_to_room("room-b", "index.html", &first_hash)
            .unwrap();
        assert_eq!(
            memory
                .store_content(&second_hash, second.clone())
                .unwrap_err(),
            ASSET_CAPACITY_ERROR
        );
        memory.cleanup_room("room-a");
        assert!(memory.has_content(&first_hash));
        memory.cleanup_room("room-b");
        assert!(!memory.has_content(&first_hash));
        memory.store_content(&second_hash, second.clone()).unwrap();

        let root = tempfile::tempdir().unwrap();
        let disk =
            DiskAssetStore::new_with_max_bytes(root.path().to_str().unwrap(), first.len() as u64);
        disk.store_content(&first_hash, first.clone()).unwrap();
        disk.store_content(&first_hash, first).unwrap();
        disk.map_to_room("room-a", "index.html", &first_hash)
            .unwrap();
        disk.map_to_room("room-b", "index.html", &first_hash)
            .unwrap();
        assert_eq!(
            disk.store_content(&second_hash, second).unwrap_err(),
            ASSET_CAPACITY_ERROR
        );
        assert!(!disk.store_dir().join(second_hash).exists());
        disk.cleanup_room("room-a");
        assert!(disk.has_content(&first_hash));
        disk.cleanup_room("room-b");
        assert!(!disk.has_content(&first_hash));
    }
}

fn create_link(original: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(original, link)
    }
    #[cfg(not(unix))]
    {
        std::fs::hard_link(original, link).or_else(|_| std::fs::copy(original, link).map(|_| ()))
    }
}

// ── Router builder ────────────────────────────────────────────────────

/// Build the relay router with all API, WebSocket, and static-file routes.
///
/// Both the standalone binary and the embedded relay call this function,
/// passing their own `WebAssetStore` implementation.
pub fn build_relay_router(
    room_manager: Arc<RoomManager>,
    asset_store: Arc<dyn WebAssetStore>,
    start_time: std::time::Instant,
    db: Option<std::sync::Arc<crate::db::DbPool>>,
    host_version: &'static str,
) -> Router {
    build_relay_router_with_page_data(
        room_manager,
        asset_store,
        start_time,
        db,
        host_version,
        None,
    )
}

/// Like [`build_relay_router`], with an explicit page-data directory for Page Functions.
pub fn build_relay_router_with_page_data(
    room_manager: Arc<RoomManager>,
    asset_store: Arc<dyn WebAssetStore>,
    start_time: std::time::Instant,
    db: Option<std::sync::Arc<crate::db::DbPool>>,
    host_version: &'static str,
    page_data_dir: Option<std::path::PathBuf>,
) -> Router {
    build_relay_router_with_page_data_and_origins(
        room_manager,
        asset_store,
        start_time,
        db,
        host_version,
        page_data_dir,
        Vec::new(),
    )
}

/// Build a relay with an explicit browser-origin allowlist. An empty list is
/// same-origin only. `*` remains available for intentionally public relays but
/// should not be combined with account APIs.
pub fn build_relay_router_with_page_data_and_origins(
    room_manager: Arc<RoomManager>,
    asset_store: Arc<dyn WebAssetStore>,
    start_time: std::time::Instant,
    db: Option<std::sync::Arc<crate::db::DbPool>>,
    host_version: &'static str,
    page_data_dir: Option<std::path::PathBuf>,
    cors_allow_origins: Vec<String>,
) -> Router {
    let page_data = page_data_dir.map(crate::page_data::PageDataStore::new);
    let cors_allow_origins = cors_allow_origins
        .into_iter()
        .filter_map(|origin| match normalized_browser_origin(&origin) {
            Some(origin) => Some(origin),
            None => {
                tracing::warn!("Ignoring invalid relay CORS origin {origin:?}");
                None
            }
        })
        .collect::<Vec<_>>();
    let state = AppState {
        room_manager,
        start_time,
        asset_store,
        db,
        page_data,
        login_rate_limiter: std::sync::Arc::new(crate::routes::auth::LoginRateLimiter::new()),
        device_manager: crate::relay::DeviceManager::new(),
        cors_allow_origins: Arc::new(cors_allow_origins.clone()),
    };

    let router = Router::new()
        .route(
            "/health",
            get(move |state| routes::api::health_check_for_host(state, host_version)),
        )
        .route(
            "/api/info",
            get(move || routes::api::server_info_for_host(host_version)),
        )
        .route(
            "/api/auth/login/challenge",
            post(routes::auth::login_challenge),
        )
        .route("/api/auth/login", post(routes::auth::login))
        .route("/api/auth/logout", post(routes::auth::logout))
        .route("/api/auth/delegate", post(routes::auth::delegate))
        .route("/api/rooms/{room_id}/pair", post(routes::api::pair))
        .route(
            "/api/rooms/{room_id}/command",
            post(routes::api::command).layer(DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route(
            "/api/rooms/{room_id}/upload-web",
            post(routes::api::upload_web).layer(DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route(
            "/api/rooms/{room_id}/check-web-files",
            post(routes::api::check_web_files),
        )
        .route(
            "/api/rooms/{room_id}/upload-web-files",
            post(routes::api::upload_web_files).layer(DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route("/r/{*rest}", get(routes::api::serve_room_web_catchall))
        .route("/ws", get(routes::websocket::websocket_handler))
        .merge(routes::sync::sync_router())
        .merge(routes::devices::device_router())
        .merge(routes::pages::pages_router())
        .with_state(state)
        .layer(axum::middleware::from_fn(relay_security_headers));

    if cors_allow_origins.is_empty() {
        return router;
    }

    use tower_http::cors::{AllowOrigin, CorsLayer};
    let allow_origin = if cors_allow_origins.iter().any(|origin| origin == "*") {
        tracing::warn!("Relay browser CORS is configured for every origin");
        AllowOrigin::any()
    } else {
        let values = cors_allow_origins
            .iter()
            .filter_map(|origin| match origin.parse::<HeaderValue>() {
                Ok(value) => Some(value),
                Err(error) => {
                    tracing::warn!("Ignoring invalid relay CORS origin {origin:?}: {error}");
                    None
                }
            })
            .collect::<Vec<_>>();
        AllowOrigin::list(values)
    };
    router.layer(
        CorsLayer::new()
            .allow_origin(allow_origin)
            .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
            .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn get_json(app: Router, path: &str) -> serde_json::Value {
        let response = app
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .expect("relay router request should complete");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("relay router response body should be readable");
        serde_json::from_slice(&body).expect("relay router response should be JSON")
    }

    #[tokio::test]
    async fn router_exposes_health_and_server_info() {
        let app = build_relay_router(
            RoomManager::new(),
            Arc::new(MemoryAssetStore::new()),
            std::time::Instant::now(),
            None,
            "test-host-version",
        );

        let health = get_json(app.clone(), "/health").await;
        assert_eq!(health["status"], "healthy");
        assert_eq!(health["rooms"], 0);
        assert_eq!(health["connections"], 0);
        assert_eq!(health["version"], "test-host-version");
        assert_eq!(health["asset_store_bytes"], 0);
        assert_eq!(
            health["asset_store_max_bytes"],
            DEFAULT_MEMORY_ASSET_STORE_MAX_BYTES
        );

        let headers_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            headers_response.headers()["x-content-type-options"],
            "nosniff"
        );
        assert_eq!(headers_response.headers()["referrer-policy"], "no-referrer");

        let info = get_json(app, "/api/info").await;
        assert_eq!(info["name"], "BitFun Relay Server");
        assert_eq!(info["version"], "test-host-version");
        assert_eq!(info["protocol_version"], 2);
    }
}
