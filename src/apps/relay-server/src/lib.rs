//! BitFun Relay Server Library
//!
//! Shared relay logic used by both the standalone relay-server binary and
//! the embedded relay running inside the desktop process.
//!
//! The relay is a stateless HTTP-to-WebSocket bridge:
//!   - Desktop clients connect via WebSocket
//!   - Mobile clients interact via HTTP POST
//!   - The relay forwards encrypted payloads without inspection
//!   - Per-room mobile-web static files are managed via `WebAssetStore`

pub mod relay;
pub mod routes;

pub use relay::room::{RoomManager, ResponsePayload};
pub use routes::api::AppState;

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;

// ── WebAssetStore trait ───────────────────────────────────────────────

/// Abstract storage for per-room mobile-web static assets.
///
/// The standalone relay uses `DiskAssetStore` (filesystem-backed), while
/// the embedded relay uses `MemoryAssetStore` (in-memory DashMap-backed).
pub trait WebAssetStore: Send + Sync + 'static {
    /// Check if content with this SHA-256 hash exists in the store.
    fn has_content(&self, hash: &str) -> bool;

    /// Store content by its SHA-256 hash. No-op if already present.
    fn store_content(&self, hash: &str, data: Vec<u8>) -> Result<(), String>;

    /// Associate a relative file path within a room to a stored content hash.
    fn map_to_room(&self, room_id: &str, rel_path: &str, hash: &str) -> Result<(), String>;

    /// Retrieve file content for serving. Falls back to `index.html` if the
    /// requested path doesn't exist (SPA routing).
    fn get_file(&self, room_id: &str, path: &str) -> Option<Vec<u8>>;

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
}

impl MemoryAssetStore {
    pub fn new() -> Self {
        Self {
            content_store: DashMap::new(),
            room_manifests: DashMap::new(),
        }
    }
}

impl Default for MemoryAssetStore {
    fn default() -> Self {
        Self::new()
    }
}

impl WebAssetStore for MemoryAssetStore {
    fn has_content(&self, hash: &str) -> bool {
        self.content_store.contains_key(hash)
    }

    fn store_content(&self, hash: &str, data: Vec<u8>) -> Result<(), String> {
        self.content_store
            .entry(hash.to_string())
            .or_insert_with(|| Arc::new(data));
        Ok(())
    }

    fn map_to_room(&self, room_id: &str, rel_path: &str, hash: &str) -> Result<(), String> {
        self.room_manifests
            .entry(room_id.to_string())
            .or_default()
            .insert(rel_path.to_string(), hash.to_string());
        Ok(())
    }

    fn get_file(&self, room_id: &str, path: &str) -> Option<Vec<u8>> {
        let manifest = self.room_manifests.get(room_id)?;
        let hash = manifest
            .get(path)
            .or_else(|| manifest.get("index.html"))?;
        let content = self.content_store.get(hash)?;
        Some(content.value().as_ref().clone())
    }

    fn has_room_files(&self, room_id: &str) -> bool {
        self.room_manifests.contains_key(room_id)
    }

    fn cleanup_room(&self, room_id: &str) {
        self.room_manifests.remove(room_id);
    }
}

// ── DiskAssetStore ────────────────────────────────────────────────────

/// Filesystem-backed asset store. Used by the standalone relay server.
///
/// Content is stored in `{base_dir}/_store/{hash}` and symlinked into
/// per-room directories `{base_dir}/{room_id}/{path}`.
pub struct DiskAssetStore {
    base_dir: String,
    known_hashes: DashMap<String, u64>,
}

impl DiskAssetStore {
    pub fn new(base_dir: &str) -> Self {
        let store_dir = std::path::PathBuf::from(base_dir).join("_store");
        let _ = std::fs::create_dir_all(&store_dir);

        let known: DashMap<String, u64> = DashMap::new();
        if store_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&store_dir) {
                for entry in entries.flatten() {
                    if let Ok(meta) = entry.metadata() {
                        if meta.is_file() {
                            if let Some(name) = entry.file_name().to_str() {
                                known.insert(name.to_string(), meta.len());
                            }
                        }
                    }
                }
            }
        }
        tracing::info!(
            "DiskAssetStore initialized with {} entries from {base_dir}",
            known.len()
        );
        Self {
            base_dir: base_dir.to_string(),
            known_hashes: known,
        }
    }

    fn store_dir(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.base_dir).join("_store")
    }

    fn room_dir(&self, room_id: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.base_dir).join(room_id)
    }
}

impl WebAssetStore for DiskAssetStore {
    fn has_content(&self, hash: &str) -> bool {
        self.known_hashes.contains_key(hash)
    }

    fn store_content(&self, hash: &str, data: Vec<u8>) -> Result<(), String> {
        let store_path = self.store_dir().join(hash);
        if !store_path.exists() {
            std::fs::write(&store_path, &data).map_err(|e| e.to_string())?;
            self.known_hashes
                .insert(hash.to_string(), data.len() as u64);
        }
        Ok(())
    }

    fn map_to_room(&self, room_id: &str, rel_path: &str, hash: &str) -> Result<(), String> {
        let store_path = self.store_dir().join(hash);
        let dest = self.room_dir(room_id).join(rel_path);
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::remove_file(&dest);
        create_link(&store_path, &dest).map_err(|e| e.to_string())
    }

    fn get_file(&self, room_id: &str, path: &str) -> Option<Vec<u8>> {
        let room_dir = self.room_dir(room_id);
        let target = room_dir.join(path);
        let file = if target.is_file() {
            target
        } else {
            room_dir.join("index.html")
        };
        if file.is_file() {
            std::fs::read(&file).ok()
        } else {
            None
        }
    }

    fn has_room_files(&self, room_id: &str) -> bool {
        self.room_dir(room_id).exists()
    }

    fn cleanup_room(&self, room_id: &str) {
        let dir = self.room_dir(room_id);
        if dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&dir) {
                tracing::warn!("Failed to clean up room web dir {}: {e}", dir.display());
            } else {
                tracing::info!("Cleaned up room web dir for {room_id}");
            }
        }
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
) -> Router {
    let state = AppState {
        room_manager,
        start_time,
        asset_store,
    };

    Router::new()
        .route("/health", get(routes::api::health_check))
        .route("/api/info", get(routes::api::server_info))
        .route("/api/rooms/:room_id/pair", post(routes::api::pair))
        .route("/api/rooms/:room_id/command", post(routes::api::command))
        .route(
            "/api/rooms/:room_id/upload-web",
            post(routes::api::upload_web).layer(DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route(
            "/api/rooms/:room_id/check-web-files",
            post(routes::api::check_web_files),
        )
        .route(
            "/api/rooms/:room_id/upload-web-files",
            post(routes::api::upload_web_files).layer(DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route("/r/*rest", get(routes::api::serve_room_web_catchall))
        .route("/ws", get(routes::websocket::websocket_handler))
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(state)
}
