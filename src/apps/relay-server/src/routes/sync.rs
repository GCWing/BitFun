//! Token-authenticated sync endpoints for encrypted session/settings blobs.
//!
//! Each handler validates the `Authorization: Bearer <token>` header via a
//! shared helper (the relay stays zero-knowledge: it only stores/returns
//! AES-GCM ciphertext encrypted client-side with the account master key).

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::routing::{delete, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::db::{AuthToken, SyncSessionRow, SyncSettingsRow};
use crate::routes::api::AppState;

/// Validated principal extracted from the bearer token.
pub struct AuthUser {
    pub user_id: String,
    #[allow(dead_code)]
    pub device_id: String,
}

/// Validate the bearer token in `headers`; returns the owning user/device.
async fn validate_auth(state: &AppState, headers: &HeaderMap) -> Result<AuthUser, StatusCode> {
    let db = state.db.as_ref().ok_or(StatusCode::NOT_IMPLEMENTED)?;
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let auth = AuthToken::find(db, &token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    Ok(AuthUser {
        user_id: auth.user_id,
        device_id: auth.device_id,
    })
}

pub fn sync_router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/sync/sessions",
            post(sessions_upsert).get(sessions_list),
        )
        .route("/api/sync/sessions/{session_id}", delete(sessions_delete))
        .route(
            "/api/sync/settings",
            post(settings_upsert).get(settings_get),
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
    let db = state.db.as_ref().ok_or(StatusCode::NOT_IMPLEMENTED)?;
    SyncSessionRow::upsert(
        db,
        &auth.user_id,
        &body.session_id,
        &body.encrypted_data,
        &body.nonce,
        body.version,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
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

async fn sessions_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let auth = validate_auth(&state, &headers).await?;
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
