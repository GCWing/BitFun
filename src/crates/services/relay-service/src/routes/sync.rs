//! Token-authenticated sync endpoints for encrypted session/settings blobs.
//!
//! Each handler validates the `Authorization: Bearer <token>` header via a
//! shared helper (the relay stays zero-knowledge: it only stores/returns
//! AES-GCM ciphertext encrypted client-side with the account master key).

use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::db::{AuthToken, SyncSessionRow, SyncSettingsRow};
use crate::routes::api::AppState;

/// Max request body for encrypted session/settings upserts.
///
/// Session sync uploads a full encrypted `SessionBundle` (metadata + all
/// turns). Axum's default limit is ~2 MiB and will return HTTP 413 for long
/// conversations. Keep an explicit ceiling so reverse proxies and operators
/// can align `client_max_body_size` (nginx) / equivalent limits.
pub const SYNC_BODY_LIMIT: usize = 64 * 1024 * 1024;
const MAX_ENCRYPTED_BLOB_BYTES: usize = 48 * 1024 * 1024;
const MAX_SESSION_ID_BYTES: usize = 256;
const MAX_NONCE_BYTES: usize = 256;
const MAX_SYNC_SESSIONS_PER_USER: i64 = 2048;
const MAX_SYNC_SESSION_BYTES_PER_USER: i64 = 128 * 1024 * 1024;

fn valid_session_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_SESSION_ID_BYTES
        && !value.chars().any(char::is_control)
}

fn valid_encrypted_blob(encrypted_data: &str, nonce: &str, version: i64) -> bool {
    !encrypted_data.is_empty()
        && encrypted_data.len() <= MAX_ENCRYPTED_BLOB_BYTES
        && !nonce.is_empty()
        && nonce.len() <= MAX_NONCE_BYTES
        && !nonce.chars().any(char::is_control)
        && version > 0
}

/// Validated principal extracted from the bearer token.
pub struct AuthUser {
    pub user_id: String,
    #[allow(dead_code)]
    pub device_id: String,
}

/// Validate the bearer token in `headers`; returns the owning user/device.
pub async fn validate_auth(state: &AppState, headers: &HeaderMap) -> Result<AuthUser, StatusCode> {
    let token = extract_bearer_token(headers).ok_or(StatusCode::UNAUTHORIZED)?;
    validate_token(state, &token).await
}

/// Extract `Bearer` token from the `Authorization` header.
pub fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Validate a raw token string against the account database.
pub async fn validate_token(state: &AppState, token: &str) -> Result<AuthUser, StatusCode> {
    let db = state.db.as_ref().ok_or(StatusCode::NOT_IMPLEMENTED)?;
    let auth = AuthToken::find(db, token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !auth.is_device_token() {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(AuthUser {
        user_id: auth.user_id,
        device_id: auth.device_id,
    })
}

pub fn sync_router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/sync/sessions",
            post(sessions_upsert)
                .get(sessions_list)
                .layer(DefaultBodyLimit::max(SYNC_BODY_LIMIT)),
        )
        .route(
            "/api/sync/sessions/{session_id}",
            get(sessions_get).delete(sessions_delete),
        )
        .route(
            "/api/sync/settings",
            post(settings_upsert)
                .get(settings_get)
                .layer(DefaultBodyLimit::max(SYNC_BODY_LIMIT)),
        )
}

// ── Session sync ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SessionUpsertRequest {
    pub session_id: String,
    pub encrypted_data: String,
    pub nonce: String,
    pub version: i64,
}

#[derive(Serialize)]
pub struct SessionBlob {
    pub session_id: String,
    pub encrypted_data: String,
    pub nonce: String,
    pub version: i64,
    pub updated_at: i64,
}

#[derive(Serialize)]
pub struct SessionListResponse {
    pub sessions: Vec<SessionBlob>,
}

async fn sessions_upsert(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<SessionUpsertRequest>,
) -> Result<StatusCode, StatusCode> {
    let auth = validate_auth(&state, &headers).await?;
    if !valid_session_id(&body.session_id)
        || !valid_encrypted_blob(&body.encrypted_data, &body.nonce, body.version)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let db = state.db.as_ref().ok_or(StatusCode::NOT_IMPLEMENTED)?;
    let stored = SyncSessionRow::upsert_with_quota(
        db,
        &auth.user_id,
        &body.session_id,
        &body.encrypted_data,
        &body.nonce,
        body.version,
        MAX_SYNC_SESSIONS_PER_USER,
        MAX_SYNC_SESSION_BYTES_PER_USER,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !stored {
        return Err(StatusCode::INSUFFICIENT_STORAGE);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn sessions_list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<SinceParams>,
) -> Result<Json<SessionListResponse>, StatusCode> {
    let auth = validate_auth(&state, &headers).await?;
    let db = state.db.as_ref().ok_or(StatusCode::NOT_IMPLEMENTED)?;
    let rows = SyncSessionRow::list_since(db, &auth.user_id, params.since.unwrap_or(0))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let sessions = rows
        .into_iter()
        .map(|r| SessionBlob {
            session_id: r.session_id,
            encrypted_data: r.encrypted_data,
            nonce: r.nonce,
            version: r.version,
            updated_at: r.updated_at,
        })
        .collect();
    Ok(Json(SessionListResponse { sessions }))
}

async fn sessions_get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> Result<Json<SessionBlob>, StatusCode> {
    let auth = validate_auth(&state, &headers).await?;
    if !valid_session_id(&session_id) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let db = state.db.as_ref().ok_or(StatusCode::NOT_IMPLEMENTED)?;
    let row = SyncSessionRow::get(db, &auth.user_id, &session_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(SessionBlob {
        session_id: row.session_id,
        encrypted_data: row.encrypted_data,
        nonce: row.nonce,
        version: row.version,
        updated_at: row.updated_at,
    }))
}

async fn sessions_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let auth = validate_auth(&state, &headers).await?;
    if !valid_session_id(&session_id) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let db = state.db.as_ref().ok_or(StatusCode::NOT_IMPLEMENTED)?;
    SyncSessionRow::delete(db, &auth.user_id, &session_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Settings sync ───────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SettingsUpsertRequest {
    pub encrypted_data: String,
    pub nonce: String,
    pub version: i64,
}

#[derive(Serialize)]
pub struct SettingsBlob {
    pub encrypted_data: String,
    pub nonce: String,
    pub version: i64,
    pub updated_at: i64,
}

async fn settings_upsert(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<SettingsUpsertRequest>,
) -> Result<StatusCode, StatusCode> {
    let auth = validate_auth(&state, &headers).await?;
    if !valid_encrypted_blob(&body.encrypted_data, &body.nonce, body.version) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let db = state.db.as_ref().ok_or(StatusCode::NOT_IMPLEMENTED)?;
    SyncSettingsRow::upsert(
        db,
        &auth.user_id,
        &body.encrypted_data,
        &body.nonce,
        body.version,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn settings_get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Option<SettingsBlob>>, StatusCode> {
    let auth = validate_auth(&state, &headers).await?;
    let db = state.db.as_ref().ok_or(StatusCode::NOT_IMPLEMENTED)?;
    let row = SyncSettingsRow::get(db, &auth.user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(|r| SettingsBlob {
            encrypted_data: r.encrypted_data,
            nonce: r.nonce,
            version: r.version,
            updated_at: r.updated_at,
        });
    Ok(Json(row))
}

#[derive(Deserialize)]
pub struct SinceParams {
    pub since: Option<i64>,
}
