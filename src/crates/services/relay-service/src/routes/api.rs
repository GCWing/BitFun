//! REST API routes for the relay server.
//!
//! Provides two HTTP endpoints for mobile clients:
//! - POST /api/rooms/:room_id/pair — initiate pairing
//! - POST /api/rooms/:room_id/command — send encrypted commands
//!
//! Both endpoints bridge the HTTP request to the desktop via WebSocket
//! using correlation-based request-response matching.
//!
//! File-serving and upload endpoints use the `WebAssetStore` trait,
//! so the same handlers work for both disk-backed and memory-backed stores.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::relay::RoomManager;
use crate::routes::websocket::OutboundProtocol;
use crate::WebAssetStore;

#[cfg(not(test))]
const DESKTOP_ENQUEUE_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(test)]
const DESKTOP_ENQUEUE_TIMEOUT: Duration = Duration::from_millis(25);
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_DEVICE_NAME_BYTES: usize = 256;
const MAX_PUBLIC_KEY_BYTES: usize = 512;
const MAX_ENCRYPTED_PAYLOAD_BYTES: usize = 10 * 1024 * 1024;
const MAX_NONCE_BYTES: usize = 256;
const MAX_ROOM_WEB_FILES: usize = 4096;

fn is_valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn is_valid_display_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

fn is_valid_encrypted_payload(encrypted_data: &str, nonce: &str) -> bool {
    !encrypted_data.is_empty()
        && encrypted_data.len() <= MAX_ENCRYPTED_PAYLOAD_BYTES
        && !nonce.is_empty()
        && nonce.len() <= MAX_NONCE_BYTES
        && !nonce.chars().any(char::is_control)
}

#[derive(Clone)]
pub struct AppState {
    pub room_manager: Arc<RoomManager>,
    pub start_time: std::time::Instant,
    pub asset_store: Arc<dyn WebAssetStore>,
    /// Optional account database. When `None`, the relay runs in pure-relay
    /// mode (no account features); the embedded relay passes `None`.
    pub db: Option<Arc<crate::db::DbPool>>,
    /// Optional per-page mutable data root (KV/SQLite/blobs). Required for Page Functions data plane.
    pub page_data: Option<crate::page_data::PageDataStore>,
    /// Per-IP rate limiter for auth endpoints (brute-force protection).
    pub login_rate_limiter: Arc<crate::routes::auth::LoginRateLimiter>,
    /// Per-user online device registry for account-based device routing.
    pub device_manager: Arc<crate::relay::DeviceManager>,
    /// Browser origins explicitly allowed to call this relay. An empty list
    /// means same-origin only; non-browser clients normally omit Origin.
    pub cors_allow_origins: Arc<Vec<String>>,
}

// ── Health & Info ──────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub rooms: usize,
    pub connections: usize,
    pub account_features: bool,
    pub device_connections: usize,
    pub pending_room_requests: usize,
    pub pending_device_rpcs: usize,
    pub asset_store_bytes: u64,
    pub asset_store_max_bytes: u64,
}

pub async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    health_check_for_host(State(state), env!("CARGO_PKG_VERSION")).await
}

pub(crate) async fn health_check_for_host(
    State(state): State<AppState>,
    host_version: &'static str,
) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: host_version.to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
        rooms: state.room_manager.room_count(),
        connections: state.room_manager.connection_count(),
        account_features: state.db.is_some(),
        device_connections: state.device_manager.connection_count(),
        pending_room_requests: state.room_manager.pending_request_count(),
        pending_device_rpcs: state.device_manager.pending_rpc_count(),
        asset_store_bytes: state.asset_store.stored_bytes(),
        asset_store_max_bytes: state.asset_store.max_store_bytes(),
    })
}

#[derive(Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    pub protocol_version: u8,
}

pub async fn server_info() -> Json<ServerInfo> {
    server_info_for_host(env!("CARGO_PKG_VERSION")).await
}

pub(crate) async fn server_info_for_host(host_version: &'static str) -> Json<ServerInfo> {
    Json(ServerInfo {
        name: "BitFun Relay Server".to_string(),
        version: host_version.to_string(),
        protocol_version: 2,
    })
}

// ── Pair & Command (HTTP-to-WS bridge) ────────────────────────────────────

#[derive(Deserialize)]
pub struct PairRequest {
    pub public_key: String,
    pub device_id: String,
    pub device_name: String,
}

#[derive(Serialize)]
pub struct PairResponse {
    pub encrypted_data: String,
    pub nonce: String,
}

/// `POST /api/rooms/:room_id/pair`
///
/// Mobile sends its public key to initiate pairing. The relay forwards this
/// to the desktop via WebSocket and waits for the encrypted challenge response.
pub async fn pair(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(body): Json<PairRequest>,
) -> Result<Json<PairResponse>, StatusCode> {
    if !crate::relay::room::is_valid_room_id(&room_id)
        || !is_valid_identifier(&body.device_id)
        || !is_valid_display_text(&body.device_name, MAX_DEVICE_NAME_BYTES)
        || !is_valid_display_text(&body.public_key, MAX_PUBLIC_KEY_BYTES)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !state.room_manager.has_desktop(&room_id) {
        return Err(StatusCode::NOT_FOUND);
    }

    let correlation_id = generate_correlation_id();
    let Some((_pending_guard, rx)) = state
        .room_manager
        .try_register_pending(&room_id, correlation_id.clone())
    else {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };

    let ws_msg = serde_json::to_string(&OutboundProtocol::PairRequest {
        correlation_id: correlation_id.clone(),
        public_key: body.public_key,
        device_id: body.device_id,
        device_name: body.device_name,
    })
    .unwrap_or_default();

    if let Err(status) = send_to_desktop_with_backpressure_timeout(&state, &room_id, &ws_msg).await
    {
        state.room_manager.cancel_pending(&correlation_id);
        return Err(status);
    }

    match tokio::time::timeout(Duration::from_secs(30), rx).await {
        Ok(Ok(payload)) => Ok(Json(PairResponse {
            encrypted_data: payload.encrypted_data,
            nonce: payload.nonce,
        })),
        Err(_) => {
            state.room_manager.cancel_pending(&correlation_id);
            Err(StatusCode::GATEWAY_TIMEOUT)
        }
        Ok(Err(_)) => {
            state.room_manager.cancel_pending(&correlation_id);
            Err(StatusCode::GATEWAY_TIMEOUT)
        }
    }
}

#[derive(Deserialize)]
pub struct CommandRequest {
    pub encrypted_data: String,
    pub nonce: String,
}

#[derive(Serialize)]
pub struct CommandResponse {
    pub encrypted_data: String,
    pub nonce: String,
}

/// `POST /api/rooms/:room_id/command`
///
/// Mobile sends an encrypted command. The relay forwards it to the desktop
/// via WebSocket, waits for the encrypted response, and returns it.
pub async fn command(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(body): Json<CommandRequest>,
) -> Result<Json<CommandResponse>, StatusCode> {
    if !crate::relay::room::is_valid_room_id(&room_id)
        || !is_valid_encrypted_payload(&body.encrypted_data, &body.nonce)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !state.room_manager.has_desktop(&room_id) {
        return Err(StatusCode::NOT_FOUND);
    }

    let correlation_id = generate_correlation_id();
    let Some((_pending_guard, rx)) = state
        .room_manager
        .try_register_pending(&room_id, correlation_id.clone())
    else {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };

    let ws_msg = serde_json::to_string(&OutboundProtocol::Command {
        correlation_id: correlation_id.clone(),
        encrypted_data: body.encrypted_data,
        nonce: body.nonce,
    })
    .unwrap_or_default();

    if let Err(status) = send_to_desktop_with_backpressure_timeout(&state, &room_id, &ws_msg).await
    {
        state.room_manager.cancel_pending(&correlation_id);
        return Err(status);
    }

    match tokio::time::timeout(Duration::from_secs(60), rx).await {
        Ok(Ok(payload)) => Ok(Json(CommandResponse {
            encrypted_data: payload.encrypted_data,
            nonce: payload.nonce,
        })),
        Err(_) => {
            state.room_manager.cancel_pending(&correlation_id);
            Err(StatusCode::GATEWAY_TIMEOUT)
        }
        Ok(Err(_)) => {
            state.room_manager.cancel_pending(&correlation_id);
            Err(StatusCode::GATEWAY_TIMEOUT)
        }
    }
}

async fn send_to_desktop_with_backpressure_timeout(
    state: &AppState,
    room_id: &str,
    ws_msg: &str,
) -> Result<(), StatusCode> {
    match tokio::time::timeout(
        DESKTOP_ENQUEUE_TIMEOUT,
        state.room_manager.send_to_desktop(room_id, ws_msg),
    )
    .await
    {
        Ok(true) => Ok(()),
        Ok(false) | Err(_) => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

fn generate_correlation_id() -> String {
    let bytes: [u8; 16] = rand::random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ── Per-room mobile-web upload & serving ───────────────────────────────────

fn hex_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[derive(Deserialize)]
pub struct UploadWebRequest {
    pub files: HashMap<String, String>,
}

/// `POST /api/rooms/:room_id/upload-web`
pub async fn upload_web(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(body): Json<UploadWebRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !state.room_manager.room_exists(&room_id) {
        return Err(StatusCode::NOT_FOUND);
    }
    if body.files.len() > MAX_ROOM_WEB_FILES
        || body
            .files
            .keys()
            .any(|path| crate::validated_asset_relative_path(path).is_err())
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    let asset_store = Arc::clone(&state.asset_store);
    let room_id_for_io = room_id.clone();
    let (written, reused) = tokio::task::spawn_blocking(move || {
        process_upload_web(asset_store, &room_id_for_io, body.files)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)??;

    tracing::info!("Room {room_id}: upload-web complete (new={written}, reused={reused})");
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
pub async fn check_web_files(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(body): Json<CheckWebFilesRequest>,
) -> Result<Json<CheckWebFilesResponse>, StatusCode> {
    if !state.room_manager.room_exists(&room_id) {
        return Err(StatusCode::NOT_FOUND);
    }
    if body.files.len() > MAX_ROOM_WEB_FILES
        || body.files.iter().any(|entry| {
            crate::validated_asset_relative_path(&entry.path).is_err()
                || !crate::is_valid_content_hash(&entry.hash)
        })
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    let asset_store = Arc::clone(&state.asset_store);
    let room_id_for_io = room_id.clone();
    let response = tokio::task::spawn_blocking(move || {
        process_check_web_files(asset_store, &room_id_for_io, body.files)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!(
        "Room {room_id}: check-web-files total={total_count}, existing={existing_count}, needed={needed_count}",
        total_count = response.total_count,
        existing_count = response.existing_count,
        needed_count = response.needed.len()
    );

    Ok(Json(response))
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
pub async fn upload_web_files(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(body): Json<UploadWebFilesRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !state.room_manager.room_exists(&room_id) {
        return Err(StatusCode::NOT_FOUND);
    }
    if body.files.len() > MAX_ROOM_WEB_FILES
        || body.files.iter().any(|(path, entry)| {
            crate::validated_asset_relative_path(path).is_err()
                || !crate::is_valid_content_hash(&entry.hash)
        })
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    let asset_store = Arc::clone(&state.asset_store);
    let room_id_for_io = room_id.clone();
    let stored = tokio::task::spawn_blocking(move || {
        process_upload_web_files(asset_store, &room_id_for_io, body.files)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)??;

    tracing::info!("Room {room_id}: upload-web-files stored {stored} new files");
    Ok(Json(
        serde_json::json!({ "status": "ok", "files_stored": stored }),
    ))
}

/// `GET /r/{*rest}` — serve per-room mobile-web static files.
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

    let lookup_path = if file_path.is_empty() {
        "index.html"
    } else {
        file_path
    }
    .to_string();
    crate::validated_asset_relative_path(room_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    crate::validated_asset_relative_path(&lookup_path).map_err(|_| StatusCode::BAD_REQUEST)?;

    let asset_store = Arc::clone(&state.asset_store);
    let room_id_for_io = room_id.to_string();
    let lookup_path_for_io = lookup_path.clone();
    let content = tokio::task::spawn_blocking(move || {
        asset_store.get_file(&room_id_for_io, &lookup_path_for_io)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let mime = mime_from_path(&lookup_path);
    Ok((
        [
            (header::CONTENT_TYPE, mime),
            (header::X_FRAME_OPTIONS, "DENY"),
        ],
        Body::from(content),
    )
        .into_response())
}

fn process_upload_web(
    asset_store: Arc<dyn WebAssetStore>,
    room_id: &str,
    files: HashMap<String, String>,
) -> Result<(usize, usize), StatusCode> {
    let mut written = 0usize;
    let mut reused = 0usize;
    for (rel_path, b64_content) in files {
        crate::validated_asset_relative_path(&rel_path).map_err(|_| StatusCode::BAD_REQUEST)?;
        let decoded = B64
            .decode(b64_content)
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        let hash = hex_sha256(&decoded);

        if !asset_store.has_content(&hash) {
            asset_store
                .store_content(&hash, decoded)
                .map_err(crate::asset_store_error_status)?;
            written += 1;
        } else {
            reused += 1;
        }

        asset_store
            .map_to_room(room_id, &rel_path, &hash)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok((written, reused))
}

fn process_check_web_files(
    asset_store: Arc<dyn WebAssetStore>,
    room_id: &str,
    files: Vec<FileManifestEntry>,
) -> CheckWebFilesResponse {
    let mut needed = Vec::new();
    let mut existing_count = 0usize;
    let total_count = files.len();

    for entry in files {
        if crate::validated_asset_relative_path(&entry.path).is_err()
            || !crate::is_valid_content_hash(&entry.hash)
        {
            needed.push(entry.path);
            continue;
        }
        if asset_store.has_content(&entry.hash) {
            existing_count += 1;
            let _ = asset_store.map_to_room(room_id, &entry.path, &entry.hash);
        } else {
            needed.push(entry.path);
        }
    }

    CheckWebFilesResponse {
        needed,
        existing_count,
        total_count,
    }
}

fn process_upload_web_files(
    asset_store: Arc<dyn WebAssetStore>,
    room_id: &str,
    files: HashMap<String, UploadWebFilesEntry>,
) -> Result<usize, StatusCode> {
    let mut stored = 0usize;
    for (rel_path, entry) in files {
        crate::validated_asset_relative_path(&rel_path).map_err(|_| StatusCode::BAD_REQUEST)?;
        let decoded = B64
            .decode(&entry.content)
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        let actual_hash = hex_sha256(&decoded);
        if actual_hash != entry.hash {
            tracing::warn!(
                "Room {room_id}: hash mismatch for {rel_path} (expected={}, actual={actual_hash})",
                entry.hash
            );
            return Err(StatusCode::BAD_REQUEST);
        }

        if !asset_store.has_content(&actual_hash) {
            asset_store
                .store_content(&actual_hash, decoded)
                .map_err(crate::asset_store_error_status)?;
            stored += 1;
        }

        asset_store
            .map_to_room(room_id, &rel_path, &actual_hash)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(stored)
}

fn mime_from_path(p: &str) -> &'static str {
    match p.rsplit('.').next() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::room::OutboundMessage;
    use crate::MemoryAssetStore;
    use axum::extract::{Path, State};
    use axum::Json;
    use std::collections::HashMap;
    use tokio::sync::mpsc;

    fn test_state(room_manager: Arc<RoomManager>) -> AppState {
        AppState {
            room_manager,
            start_time: std::time::Instant::now(),
            asset_store: Arc::new(MemoryAssetStore::new()),
            db: None,
            page_data: None,
            login_rate_limiter: Arc::new(crate::routes::auth::LoginRateLimiter::new()),
            device_manager: crate::relay::DeviceManager::new(),
            cors_allow_origins: Arc::new(Vec::new()),
        }
    }

    #[tokio::test]
    async fn pair_reports_backpressure_before_response_timeout() {
        let room_manager = RoomManager::new();
        let (tx, _rx) = mpsc::channel(1);
        tx.send(OutboundMessage::text("queued"))
            .await
            .expect("queue should accept first message");
        room_manager.create_room("room-a", 1, "desktop-a", "public-key", tx);

        let result = tokio::time::timeout(
            Duration::from_millis(100),
            pair(
                State(test_state(room_manager)),
                Path("room-a".to_string()),
                Json(PairRequest {
                    public_key: "mobile-key".to_string(),
                    device_id: "mobile-a".to_string(),
                    device_name: "Mobile A".to_string(),
                }),
            ),
        )
        .await
        .expect("backpressure should return before the response timeout");

        assert!(matches!(result, Err(StatusCode::SERVICE_UNAVAILABLE)));
    }

    #[tokio::test]
    async fn room_web_upload_rejects_absolute_file_paths() {
        let room_manager = RoomManager::new();
        let (tx, _rx) = mpsc::channel(1);
        assert!(room_manager.create_room("room-a", 1, "desktop-a", "public-key", tx));
        let mut files = HashMap::new();
        files.insert("/tmp/relay-owned".to_string(), B64.encode(b"owned"));

        let result = upload_web(
            State(test_state(room_manager)),
            Path("room-a".to_string()),
            Json(UploadWebRequest { files }),
        )
        .await;

        assert!(matches!(result, Err(StatusCode::BAD_REQUEST)));
    }
}
