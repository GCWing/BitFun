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

const MAX_DEVICE_ID_BYTES: usize = 128;
const MAX_ENCRYPTED_PAYLOAD_BYTES: usize = 48 * 1024 * 1024;
const MAX_NONCE_BYTES: usize = 256;

fn is_valid_device_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_DEVICE_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn is_valid_encrypted_payload(encrypted_data: &str, nonce: &str) -> bool {
    !encrypted_data.is_empty()
        && encrypted_data.len() <= MAX_ENCRYPTED_PAYLOAD_BYTES
        && !nonce.is_empty()
        && nonce.len() <= MAX_NONCE_BYTES
        && !nonce.chars().any(char::is_control)
}

/// Validate bearer token and return its account principal and capability kind.
async fn validate_user(state: &AppState, headers: &HeaderMap) -> Result<AuthToken, StatusCode> {
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
    if !auth.can_control_devices() {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(auth)
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
    let auth = validate_user(&state, &headers).await?;
    let user_id = auth.user_id;

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
    let auth = validate_user(&state, &headers).await?;
    let user_id = auth.user_id;

    if !is_valid_device_id(&target_device_id)
        || !is_valid_encrypted_payload(&body.encrypted_data, &body.nonce)
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check target device is online in this account
    let online = state.device_manager.online_devices(&user_id);
    if !online.iter().any(|(id, _)| id == &target_device_id) {
        return Err(StatusCode::NOT_FOUND);
    }

    // Generate a correlation_id for request-response matching
    let correlation_id = uuid::Uuid::new_v4().to_string();

    // Register a pending RPC response (the WS handler will resolve it when
    // the target device sends back a DeviceMessage with the same correlation_id)
    let rx = state
        .device_manager
        .register_rpc(&correlation_id, &user_id, &target_device_id)
        .ok_or(StatusCode::TOO_MANY_REQUESTS)?;

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
/// Deleting the caller's own device revokes its current token as well.
async fn delete_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(target_device_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let auth = validate_user(&state, &headers).await?;
    if !auth.is_device_token() {
        return Err(StatusCode::FORBIDDEN);
    }
    let user_id = auth.user_id;

    if !is_valid_device_id(&target_device_id) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let db = state.db.as_ref().ok_or(StatusCode::NOT_IMPLEMENTED)?;

    // Revoke the target's auth tokens before removing its device row. The DB
    // helper also scopes the deletion to this account and performs both writes
    // atomically so a guessed device id cannot affect another account.
    let deleted = crate::db::DeviceRow::delete_for_user(db, &user_id, &target_device_id)
        .await
        .map_err(|error| {
            tracing::error!(
                user_id = %user_id,
                target_device_id = %target_device_id,
                %error,
                "Failed to delete account device"
            );
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if !deleted {
        return Err(StatusCode::NOT_FOUND);
    }

    // Disconnect active WS session if any.
    state
        .device_manager
        .disconnect_device(&user_id, &target_device_id);

    let online = state.device_manager.online_devices(&user_id);
    let presence = online
        .iter()
        .map(
            |(device_id, device_name)| crate::routes::websocket::DevicePresenceEntry {
                device_id: device_id.clone(),
                device_name: device_name.clone(),
            },
        )
        .collect();
    let presence_json =
        serde_json::to_string(&OutboundProtocol::DevicePresence { devices: presence })
            .unwrap_or_default();
    state
        .device_manager
        .broadcast_except(&user_id, &target_device_id, &presence_json);

    tracing::info!("Device {target_device_id} removed from account {user_id}");
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{connect, AuthToken, DbPool, DeviceRow, UserRow};
    use crate::relay::RoomManager;
    use crate::MemoryAssetStore;
    use axum::body::Body;
    use axum::http::Request;
    use std::sync::Arc;
    use tower::ServiceExt;

    struct TestContext {
        app: axum::Router,
        db: Arc<DbPool>,
        owner_token: String,
        delegated_token: String,
        target_token: String,
        other_token: String,
    }

    async fn setup_app() -> TestContext {
        let db = Arc::new(connect(":memory:").await.unwrap());
        UserRow::create(&db, "owner", "alice", "s", "ks", "{}", "hash", "wmk")
            .await
            .unwrap();
        UserRow::create(&db, "other", "bob", "s", "ks", "{}", "hash", "wmk")
            .await
            .unwrap();
        DeviceRow::upsert(&db, "owner-device", "owner", "Owner", None)
            .await
            .unwrap();
        DeviceRow::upsert(&db, "target-device", "owner", "Target", None)
            .await
            .unwrap();
        DeviceRow::upsert(&db, "other-device", "other", "Other", None)
            .await
            .unwrap();

        let owner_token = AuthToken::create(&db, "owner", "owner-device")
            .await
            .unwrap()
            .token;
        let delegated_token = AuthToken::create_delegated(&db, "owner", "owner-device")
            .await
            .unwrap()
            .token;
        let target_token = AuthToken::create(&db, "owner", "target-device")
            .await
            .unwrap()
            .token;
        let other_token = AuthToken::create(&db, "other", "other-device")
            .await
            .unwrap()
            .token;
        let app = crate::build_relay_router(
            RoomManager::new(),
            Arc::new(MemoryAssetStore::new()),
            std::time::Instant::now(),
            Some(db.clone()),
            "test",
        );

        TestContext {
            app,
            db,
            owner_token,
            delegated_token,
            target_token,
            other_token,
        }
    }

    async fn delete(app: &axum::Router, token: &str, device_id: &str) -> StatusCode {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/devices/{device_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
    }

    async fn list(app: &axum::Router, token: &str) -> StatusCode {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/devices")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn deleting_owned_device_revokes_token_before_device_row() {
        let ctx = setup_app().await;

        let status = delete(&ctx.app, &ctx.owner_token, "target-device").await;

        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(AuthToken::find(&ctx.db, &ctx.target_token)
            .await
            .unwrap()
            .is_none());
        let devices = DeviceRow::list_by_user(&ctx.db, "owner").await.unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device_id, "owner-device");
    }

    #[tokio::test]
    async fn deleting_current_device_revokes_the_callers_token() {
        let ctx = setup_app().await;

        assert_eq!(
            delete(&ctx.app, &ctx.owner_token, "owner-device").await,
            StatusCode::NO_CONTENT
        );
        assert!(AuthToken::find(&ctx.db, &ctx.owner_token)
            .await
            .unwrap()
            .is_none());
        let devices = DeviceRow::list_by_user(&ctx.db, "owner").await.unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device_id, "target-device");
    }

    #[tokio::test]
    async fn deleting_another_accounts_device_is_rejected() {
        let ctx = setup_app().await;

        assert_eq!(
            delete(&ctx.app, &ctx.owner_token, "other-device").await,
            StatusCode::NOT_FOUND
        );
        assert!(AuthToken::find(&ctx.db, &ctx.owner_token)
            .await
            .unwrap()
            .is_some());
        assert!(AuthToken::find(&ctx.db, &ctx.other_token)
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn delegated_control_token_cannot_delete_its_parent_device() {
        let ctx = setup_app().await;

        assert_eq!(list(&ctx.app, &ctx.delegated_token).await, StatusCode::OK);
        assert_eq!(
            delete(&ctx.app, &ctx.delegated_token, "owner-device").await,
            StatusCode::FORBIDDEN
        );
        assert!(AuthToken::find(&ctx.db, &ctx.owner_token)
            .await
            .unwrap()
            .is_some());
        assert!(DeviceRow::list_by_user(&ctx.db, "owner")
            .await
            .unwrap()
            .iter()
            .any(|device| device.device_id == "owner-device"));
    }
}
