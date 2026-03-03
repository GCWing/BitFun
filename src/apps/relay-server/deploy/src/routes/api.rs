//! REST API routes for the relay server.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;

use crate::relay::room::{BufferedMessage, MessageDirection};
use crate::relay::RoomManager;

#[derive(Clone)]
pub struct AppState {
    pub room_manager: Arc<RoomManager>,
    pub start_time: std::time::Instant,
    /// Base directory for per-room uploaded mobile-web files.
    pub room_web_dir: String,
    /// Global content-addressed file store: sha256 hex -> stored on disk at `{room_web_dir}/_store/{hash}`.
    pub content_store: Arc<ContentStore>,
}

/// Tracks which SHA-256 hashes are already persisted in the `_store/` directory.
pub struct ContentStore {
    known_hashes: DashMap<String, u64>,
}

impl ContentStore {
    pub fn new(store_dir: &std::path::Path) -> Self {
        let known: DashMap<String, u64> = DashMap::new();
        if store_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(store_dir) {
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
        tracing::info!("Content store initialized with {} entries", known.len());
        Self { known_hashes: known }
    }

    pub fn contains(&self, hash: &str) -> bool {
        self.known_hashes.contains_key(hash)
    }

    pub fn insert(&self, hash: String, size: u64) {
        self.known_hashes.insert(hash, size);
    }
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub rooms: usize,
    pub connections: usize,
}

pub async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
        rooms: state.room_manager.room_count(),
        connections: state.room_manager.connection_count(),
    })
}

#[derive(Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    pub protocol_version: u8,
}

pub async fn server_info() -> Json<ServerInfo> {
    Json(ServerInfo {
        name: "BitFun Relay Server".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        protocol_version: 1,
    })
}

#[derive(Deserialize)]
pub struct JoinRoomRequest {
    pub device_id: String,
    pub device_type: String,
    pub public_key: String,
}

/// `POST /api/rooms/:room_id/join`
pub async fn join_room(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(body): Json<JoinRoomRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let conn_id = state.room_manager.next_conn_id();
    let existing_peer = state.room_manager.get_peer_info(&room_id, conn_id);

    let ok = state.room_manager.join_room(
        &room_id,
        conn_id,
        &body.device_id,
        &body.device_type,
        &body.public_key,
        None, // HTTP client, no websocket tx
    );

    if ok {
        let joiner_notification = serde_json::to_string(&crate::routes::websocket::OutboundProtocol::PeerJoined {
            device_id: body.device_id.clone(),
            device_type: body.device_type.clone(),
            public_key: body.public_key.clone(),
        }).unwrap_or_default();
        state.room_manager.send_to_others_in_room(&room_id, conn_id, &joiner_notification);

        if let Some((peer_did, peer_dt, peer_pk)) = existing_peer {
            Ok(Json(serde_json::json!({
                "status": "joined",
                "peer": {
                    "device_id": peer_did,
                    "device_type": peer_dt,
                    "public_key": peer_pk
                }
            })))
        } else {
            Ok(Json(serde_json::json!({
                "status": "joined",
                "peer": null
            })))
        }
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

#[derive(Deserialize)]
pub struct RelayMessageRequest {
    pub device_id: String,
    pub encrypted_data: String,
    pub nonce: String,
}

/// `POST /api/rooms/:room_id/message`
pub async fn relay_message(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(body): Json<RelayMessageRequest>,
) -> StatusCode {
    // Find conn_id by device_id in the room
    if let Some(conn_id) = state.room_manager.get_conn_id_by_device(&room_id, &body.device_id) {
        if state.room_manager.relay_message(conn_id, &body.encrypted_data, &body.nonce) {
            StatusCode::OK
        } else {
            StatusCode::NOT_FOUND
        }
    } else {
        StatusCode::UNAUTHORIZED
    }
}

#[derive(Deserialize)]
pub struct PollQuery {
    pub since_seq: Option<u64>,
    pub device_type: Option<String>,
}

#[derive(Serialize)]
pub struct PollResponse {
    pub messages: Vec<BufferedMessage>,
    pub peer_connected: bool,
}

/// `GET /api/rooms/:room_id/poll?since_seq=0&device_type=mobile`
pub async fn poll_messages(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Query(query): Query<PollQuery>,
) -> Result<Json<PollResponse>, StatusCode> {
    let since = query.since_seq.unwrap_or(0);
    let direction = match query.device_type.as_deref() {
        Some("desktop") => MessageDirection::ToDesktop,
        _ => MessageDirection::ToMobile,
    };
    
    let peer_connected = state.room_manager.has_peer(&room_id, query.device_type.as_deref().unwrap_or("mobile"));
    let messages = state.room_manager.poll_messages(&room_id, direction, since);
    
    Ok(Json(PollResponse { messages, peer_connected }))
}

#[derive(Deserialize)]
pub struct AckRequest {
    pub ack_seq: u64,
    pub device_type: Option<String>,
}

/// `POST /api/rooms/:room_id/ack`
pub async fn ack_messages(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(body): Json<AckRequest>,
) -> StatusCode {
    let direction = match body.device_type.as_deref() {
        Some("desktop") => MessageDirection::ToDesktop,
        _ => MessageDirection::ToMobile,
    };
    state
        .room_manager
        .ack_messages(&room_id, direction, body.ack_seq);
    StatusCode::OK
}

// ── Per-room mobile-web upload & serving ───────────────────────────────────

#[derive(Deserialize)]
pub struct UploadWebRequest {
    pub files: HashMap<String, String>,
}

/// `POST /api/rooms/:room_id/upload-web`
///
/// Desktop uploads mobile-web dist files (base64-encoded) so the mobile
/// browser can load the exact same version the desktop is running.
/// Now uses the global content store + symlinks to avoid storing duplicates.
pub async fn upload_web(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(body): Json<UploadWebRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};

    if !state.room_manager.room_exists(&room_id) {
        return Err(StatusCode::NOT_FOUND);
    }

    let store_dir = std::path::PathBuf::from(&state.room_web_dir).join("_store");
    let _ = std::fs::create_dir_all(&store_dir);

    let room_dir = std::path::PathBuf::from(&state.room_web_dir).join(&room_id);
    if let Err(e) = std::fs::create_dir_all(&room_dir) {
        tracing::error!("Failed to create room web dir {}: {e}", room_dir.display());
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let mut written = 0usize;
    let mut reused = 0usize;
    for (rel_path, b64_content) in &body.files {
        if rel_path.contains("..") {
            continue;
        }
        let decoded = B64.decode(b64_content).map_err(|_| StatusCode::BAD_REQUEST)?;
        let hash = hex_sha256(&decoded);

        let store_path = store_dir.join(&hash);
        if !store_path.exists() {
            std::fs::write(&store_path, &decoded)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            state.content_store.insert(hash.clone(), decoded.len() as u64);
            written += 1;
        } else {
            reused += 1;
        }

        let dest = room_dir.join(rel_path);
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::remove_file(&dest);
        create_link(&store_path, &dest)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    tracing::info!(
        "Room {room_id}: upload-web complete (new={written}, reused={reused})"
    );
    Ok(Json(serde_json::json!({
        "status": "ok",
        "files_written": written,
        "files_reused": reused
    })))
}

// ── Incremental upload protocol ────────────────────────────────────────────

#[derive(Deserialize)]
pub struct FileManifestEntry {
    pub path: String,
    pub hash: String,
    #[allow(dead_code)]
    pub size: u64,
}

#[derive(Deserialize)]
pub struct CheckWebFilesRequest {
    pub files: Vec<FileManifestEntry>,
}

#[derive(Serialize)]
pub struct CheckWebFilesResponse {
    pub needed: Vec<String>,
    pub existing_count: usize,
    pub total_count: usize,
}

/// `POST /api/rooms/:room_id/check-web-files`
///
/// Accepts a manifest of file metadata (path, sha256, size). Registers the
/// room's file manifest and returns which files the server still needs. Files
/// whose hash already exists in the global content store are skipped.
pub async fn check_web_files(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(body): Json<CheckWebFilesRequest>,
) -> Result<Json<CheckWebFilesResponse>, StatusCode> {
    if !state.room_manager.room_exists(&room_id) {
        return Err(StatusCode::NOT_FOUND);
    }

    let store_dir = std::path::PathBuf::from(&state.room_web_dir).join("_store");
    let _ = std::fs::create_dir_all(&store_dir);

    let room_dir = std::path::PathBuf::from(&state.room_web_dir).join(&room_id);
    let _ = std::fs::create_dir_all(&room_dir);

    let mut needed = Vec::new();
    let mut existing_count = 0usize;
    let total_count = body.files.len();

    for entry in &body.files {
        if entry.path.contains("..") {
            continue;
        }
        if state.content_store.contains(&entry.hash) {
            existing_count += 1;
            let store_path = store_dir.join(&entry.hash);
            let dest = room_dir.join(&entry.path);
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::remove_file(&dest);
            let _ = create_link(&store_path, &dest);
        } else {
            needed.push(entry.path.clone());
        }
    }

    tracing::info!(
        "Room {room_id}: check-web-files total={total_count}, existing={existing_count}, needed={}",
        needed.len()
    );

    Ok(Json(CheckWebFilesResponse {
        needed,
        existing_count,
        total_count,
    }))
}

#[derive(Deserialize)]
pub struct UploadWebFilesEntry {
    pub content: String,
    pub hash: String,
}

#[derive(Deserialize)]
pub struct UploadWebFilesRequest {
    pub files: HashMap<String, UploadWebFilesEntry>,
}

/// `POST /api/rooms/:room_id/upload-web-files`
///
/// Upload only the files that the server requested via `check-web-files`.
/// Each entry includes the base64 content and its expected sha256 hash.
/// Files are stored in the global content store and symlinked into the room.
pub async fn upload_web_files(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(body): Json<UploadWebFilesRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};

    if !state.room_manager.room_exists(&room_id) {
        return Err(StatusCode::NOT_FOUND);
    }

    let store_dir = std::path::PathBuf::from(&state.room_web_dir).join("_store");
    let _ = std::fs::create_dir_all(&store_dir);

    let room_dir = std::path::PathBuf::from(&state.room_web_dir).join(&room_id);
    let _ = std::fs::create_dir_all(&room_dir);

    let mut stored = 0usize;
    for (rel_path, entry) in &body.files {
        if rel_path.contains("..") {
            continue;
        }
        let decoded = B64.decode(&entry.content).map_err(|_| StatusCode::BAD_REQUEST)?;
        let actual_hash = hex_sha256(&decoded);
        if actual_hash != entry.hash {
            tracing::warn!(
                "Room {room_id}: hash mismatch for {rel_path} (expected={}, actual={actual_hash})",
                entry.hash
            );
            return Err(StatusCode::BAD_REQUEST);
        }

        let store_path = store_dir.join(&actual_hash);
        if !store_path.exists() {
            std::fs::write(&store_path, &decoded)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            state
                .content_store
                .insert(actual_hash.clone(), decoded.len() as u64);
        }

        let dest = room_dir.join(rel_path);
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::remove_file(&dest);
        create_link(&store_path, &dest)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        stored += 1;
    }

    tracing::info!("Room {room_id}: upload-web-files stored {stored} new files");
    Ok(Json(serde_json::json!({ "status": "ok", "files_stored": stored })))
}

fn hex_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Create a symlink (Unix) or hard link fallback (Windows).
fn create_link(
    original: &std::path::Path,
    link: &std::path::Path,
) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(original, link)
    }
    #[cfg(not(unix))]
    {
        std::fs::hard_link(original, link)
            .or_else(|_| std::fs::copy(original, link).map(|_| ()))
    }
}

/// `GET /r/{*rest}` — serve per-room mobile-web static files.
///
/// The `rest` path is expected to be `room_id` or `room_id/file/path`.
/// Falls back to `index.html` for SPA routing.
pub async fn serve_room_web_catchall(
    State(state): State<AppState>,
    Path(rest): Path<String>,
) -> Result<axum::response::Response, StatusCode> {
    use axum::body::Body;
    use axum::http::header;
    use axum::response::IntoResponse;

    let rest = rest.trim_start_matches('/');
    let (room_id, file_path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx + 1..]),
        None => (rest, ""),
    };

    if room_id.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    let room_dir = std::path::PathBuf::from(&state.room_web_dir).join(room_id);
    if !room_dir.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let target = if file_path.is_empty() {
        room_dir.join("index.html")
    } else {
        room_dir.join(file_path)
    };

    let file = if target.is_file() {
        target
    } else {
        room_dir.join("index.html")
    };

    if !file.is_file() {
        return Err(StatusCode::NOT_FOUND);
    }

    let content = std::fs::read(&file).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mime = mime_from_path(&file);

    Ok(([(header::CONTENT_TYPE, mime)], Body::from(content)).into_response())
}

fn mime_from_path(p: &std::path::Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ttf") => "font/ttf",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// Remove the per-room web directory (called on room cleanup).
pub fn cleanup_room_web(room_web_dir: &str, room_id: &str) {
    let dir = std::path::PathBuf::from(room_web_dir).join(room_id);
    if dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            tracing::warn!("Failed to clean up room web dir {}: {e}", dir.display());
        } else {
            tracing::info!("Cleaned up room web dir for {room_id}");
        }
    }
}
