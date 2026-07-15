//! Device RPC endpoints — any authenticated client can route commands to
//! other same-account devices via HTTP (no WS needed). The relay acts as
//! a transparent router: it validates the account token, routes the opaque
//! encrypted payload to the target device's WS, waits for the response,
//! and returns it over HTTP.
//!
//! This enables mobile-web and desktop alike to browse other devices'
//! workspaces/sessions and dispatch tasks, without requiring a direct WS
//! connection or proxying through another desktop.

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::db::AuthToken;
use crate::routes::api::AppState;
use crate::routes::websocket::OutboundProtocol;

#[cfg(not(test))]
const RPC_TIMEOUT: Duration = Duration::from_secs(120);
#[cfg(test)]
const RPC_TIMEOUT: Duration = Duration::from_millis(100);

/// Validate bearer token, returns user_id.
async fn validate_user(state: &AppState, headers: &HeaderMap) -> Result<String, StatusCode> {
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
    Ok(auth.user_id)
}

pub fn device_router() -> Router<AppState> {
    Router::new()
        .route("/api/devices", get(list_devices))
        .route("/api/devices/{target_device_id}/rpc", post(device_rpc))
        .route("/api/devices/{target_device_id}", delete(delete_device))
}

// ── List devices ────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct DeviceListEntry {
    pub device_id: String,
    pub device_name: String,
    pub online: bool,
    pub last_seen_at: Option<i64>,
}

/// `GET /api/devices` — list all devices for the account (online + offline).
async fn list_devices(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<DeviceListEntry>>, StatusCode> {
    let user_id = validate_user(&state, &headers).await?;

    // Get online devices from DeviceManager (in-memory)
    let online = state.device_manager.online_devices(&user_id);
    let online_ids: std::collections::HashSet<String> =
        online.iter().map(|(id, _)| id.clone()).collect();

    // Get all registered devices from the DB (online + offline)
    let mut devices = Vec::new();
    if let Some(db) = &state.db {
        if let Ok(db_devices) = crate::db::DeviceRow::list_by_user(db, &user_id).await {
            for row in db_devices {
                let is_online = online_ids.contains(&row.device_id);
                devices.push(DeviceListEntry {
                    device_id: row.device_id,
                    device_name: row.device_name.unwrap_or_default(),
                    online: is_online,
                    last_seen_at: row.last_seen_at,
                });
            }
        }
    }

    // Also include any online-only devices not yet in the DB
    for (id, name) in &online {
        if !devices.iter().any(|d| &d.device_id == id) {
            devices.push(DeviceListEntry {
                device_id: id.clone(),
                device_name: name.clone(),
                online: true,
                last_seen_at: None,
            });
        }
    }

    Ok(Json(devices))
}

// ── Device RPC ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct DeviceRpcRequest {
    /// Opaque ciphertext encrypted client-side with the account master_key.
    /// The relay never decrypts this — it only routes.
    pub encrypted_data: String,
    pub nonce: String,
}

#[derive(Serialize)]
pub struct DeviceRpcResponse {
    pub encrypted_data: String,
    pub nonce: String,
}

/// `POST /api/devices/:target_device_id/rpc`
///
/// Routes an encrypted command to the target device via WS, waits for the
/// encrypted response, and returns it. The relay stays zero-knowledge — it
/// only sees opaque ciphertext and routes by device_id within the account.
async fn device_rpc(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(target_device_id): Path<String>,
    Json(body): Json<DeviceRpcRequest>,
) -> Result<Json<DeviceRpcResponse>, StatusCode> {
    let user_id = validate_user(&state, &headers).await?;

    // Check target device is online in this account
    let online = state.device_manager.online_devices(&user_id);
    if !online.iter().any(|(id, _)| id == &target_device_id) {
        return Err(StatusCode::NOT_FOUND);
    }

    // Generate a correlation_id for request-response matching
    let correlation_id = uuid::Uuid::new_v4().to_string();

    // Register a pending RPC response (the WS handler will resolve it when
    // the target device sends back a DeviceMessage with the same correlation_id)
    let rx = state.device_manager.register_rpc(&correlation_id);

    // Build the WS message to send to the target device.
    // The relay acts as a "virtual" source — the target device sees this
    // as an IncomingDeviceMessage from a special "rpc" source.
    let out_msg = OutboundProtocol::IncomingDeviceMessage {
        source_device_id: "rpc".to_string(), // indicates HTTP RPC origin
        correlation_id: correlation_id.clone(),
        encrypted_data: body.encrypted_data,
        nonce: body.nonce,
    };
    let json = serde_json::to_string(&out_msg).unwrap_or_default();

    if !state
        .device_manager
        .route_message(&user_id, &target_device_id, &json)
    {
        state.device_manager.cancel_rpc(&correlation_id);
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    // Wait for the response (the target device sends back a DeviceMessage
    // via WS, which the WS handler resolves via resolve_rpc)
    match tokio::time::timeout(RPC_TIMEOUT, rx).await {
        Ok(Ok(resp)) => Ok(Json(DeviceRpcResponse {
            encrypted_data: resp.encrypted_data,
            nonce: resp.nonce,
        })),
        _ => {
            state.device_manager.cancel_rpc(&correlation_id);
            Err(StatusCode::GATEWAY_TIMEOUT)
        }
    }
}

// ── Delete device ───────────────────────────────────────────────────────

/// `DELETE /api/devices/:target_device_id`
///
/// Removes a device from the account (DB row + any active WS session).
/// The caller cannot delete itself.
async fn delete_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(target_device_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let user_id = validate_user(&state, &headers).await?;

    // Get the caller's own device_id from the auth token.
    let db = state.db.as_ref().ok_or(StatusCode::NOT_IMPLEMENTED)?;
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|t| t.trim().to_string())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let auth = AuthToken::find(db, &token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Prevent self-deletion.
    if auth.device_id == target_device_id {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Remove from DB.
    let _ = crate::db::DeviceRow::delete(db, &target_device_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Revoke any auth tokens belonging to the removed device.
    let _ = crate::db::AuthToken::revoke_by_device(db, &target_device_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Disconnect active WS session if any.
    state.device_manager.disconnect_device(&user_id, &target_device_id);

    tracing::info!("Device {target_device_id} removed from account {user_id}");
    Ok(StatusCode::NO_CONTENT)
}
