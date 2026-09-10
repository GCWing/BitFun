//! Device RPC endpoints — any authenticated client can route commands to
//! other same-account devices via HTTP (no WS needed). The relay acts as
//! a transparent router: it validates the account token, routes the opaque
//! encrypted payload to the target device's WS, waits for the response,
//! and returns it over HTTP.
//!
//! This enables mobile-web and desktop alike to browse other devices'
//! workspaces/sessions and dispatch tasks, without requiring a direct WS
//! connection or proxying through another desktop.

use axum::extract::{DefaultBodyLimit, Path, State};
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

/// Body ceiling for `/api/devices/:id/rpc`.
///
/// Without this the route inherits Axum's 2 MB default, which rejects the body
/// before `is_valid_encrypted_payload` ever runs — so a mobile client sending a
/// photo got a bare 413 while the payload validator above claimed to allow
/// 48 MB. Derived from that constant so the two cannot drift apart again; the
/// slack covers the JSON envelope and the base64 nonce.
const RPC_BODY_LIMIT_BYTES: usize = MAX_ENCRYPTED_PAYLOAD_BYTES + 64 * 1024;

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
        && encrypted_data.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=' | b'-' | b'_')
        })
        && !nonce.is_empty()
        && nonce.len() <= MAX_NONCE_BYTES
        && !nonce.chars().any(char::is_control)
}

/// Validate bearer token and return its account principal and capability kind.
pub(crate) async fn validate_user(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthToken, StatusCode> {
    let db = state.db.as_ref();
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
        .route("/api/devices/{target_device_id}/key", get(device_key))
        .route(
            "/api/devices/{target_device_id}/rpc",
            post(device_rpc).layer(DefaultBodyLimit::max(RPC_BODY_LIMIT_BYTES)),
        )
        .route(
            "/api/devices/{target_device_id}/messages",
            post(device_message).layer(DefaultBodyLimit::max(RPC_BODY_LIMIT_BYTES)),
        )
        .route("/api/devices/{target_device_id}", delete(delete_device))
}

#[derive(Serialize)]
pub struct DeviceKeyResponse {
    pub device_id: String,
    pub public_key: String,
}

/// Resolve even hidden controller keys, scoped to the authenticated account.
pub async fn device_key(
    State(state): State<AppState>,
    Path(target_device_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<DeviceKeyResponse>, StatusCode> {
    let auth = validate_user(&state, &headers).await?;
    if !is_valid_device_id(&target_device_id) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let db = state.db.as_ref();
    let public_key = sqlx::query_scalar::<_, Option<String>>(
        "SELECT public_key FROM devices WHERE user_id = ?1 AND device_id = ?2
         UNION ALL SELECT k.public_key FROM delegated_device_keys k JOIN auth_tokens t ON t.token = k.token
         WHERE t.user_id = ?1 AND k.controller_id = ?2 AND t.expires_at > unixepoch() LIMIT 1",
    )
    .bind(&auth.user_id)
    .bind(&target_device_id)
    .fetch_optional(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .flatten()
    .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(DeviceKeyResponse {
        device_id: target_device_id,
        public_key,
    }))
}

// ── List devices ────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct DeviceListEntry {
    pub device_id: String,
    pub device_name: String,
    pub device_kind: Option<String>,
    pub online: bool,
    pub last_seen_at: Option<i64>,
}

/// `GET /api/devices` — list the account's remote-control targets.
///
/// Phones and watches register device rows too (they need one to hold an auth
/// token), but they cannot host a session, so they are never listed here.
async fn list_devices(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<DeviceListEntry>>, StatusCode> {
    let auth = validate_user(&state, &headers).await?;
    let user_id = auth.user_id.clone();

    // Get online devices from DeviceManager (in-memory)
    let online = state.device_manager.online_devices(&user_id);
    let online_ids: std::collections::HashSet<String> =
        online.iter().map(|(id, _)| id.clone()).collect();

    // Get all registered devices from the DB (online + offline)
    let mut devices = Vec::new();
    let mut hidden_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    {
        let db = &state.db;
        if let Ok(db_devices) = crate::db::DeviceRow::list_by_user(db, &user_id).await {
            for row in db_devices {
                if !crate::db::device_kind_is_desktop(row.device_kind.as_deref()) {
                    hidden_ids.insert(row.device_id);
                    continue;
                }
                let is_online = online_ids.contains(&row.device_id);
                devices.push(DeviceListEntry {
                    device_id: row.device_id,
                    device_name: row.device_name.unwrap_or_default(),
                    device_kind: row.device_kind,
                    online: is_online,
                    last_seen_at: row.last_seen_at,
                });
            }
        }
    }

    // Also include any online-only devices not yet in the DB. The in-memory
    // registry does not carry a kind, so these are treated like a NULL row —
    // except for ids the DB just told us to hide, which must stay hidden.
    for (id, name) in &online {
        if hidden_ids.contains(id) {
            continue;
        }
        if !devices.iter().any(|d| &d.device_id == id) {
            devices.push(DeviceListEntry {
                device_id: id.clone(),
                device_name: name.clone(),
                device_kind: None,
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
    /// Opaque ciphertext encrypted client-side with the device-pair key.
    /// The relay never decrypts this — it only routes.
    pub encrypted_data: String,
    pub nonce: String,
}

#[derive(Serialize)]
pub struct DeviceRpcResponse {
    pub encrypted_data: String,
    pub nonce: String,
}

#[derive(Deserialize)]
struct DeviceMessageRequest {
    correlation_id: String,
    encrypted_data: String,
    nonce: String,
}

/// Device responses use the same authenticated, memory-admitted HTTP ingress
/// as requests. WebSocket ingress is reserved for small control messages.
async fn device_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(target_device_id): Path<String>,
    Json(body): Json<DeviceMessageRequest>,
) -> Result<StatusCode, StatusCode> {
    let auth = validate_user(&state, &headers).await?;
    if !auth.is_device_token() {
        return Err(StatusCode::FORBIDDEN);
    }
    if !is_valid_device_id(&target_device_id)
        || !is_valid_device_id(&body.correlation_id)
        || !is_valid_encrypted_payload(&body.encrypted_data, &body.nonce)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let response = crate::relay::device_manager::RpcResponse::try_new(
        body.encrypted_data.clone(),
        body.nonce.clone(),
    )
    .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    if state.device_manager.resolve_rpc(
        &body.correlation_id,
        &auth.user_id,
        &auth.device_id,
        response,
    ) {
        return Ok(StatusCode::NO_CONTENT);
    }
    let message = OutboundProtocol::IncomingDeviceMessage {
        source_device_id: auth.device_id,
        correlation_id: body.correlation_id,
        encrypted_data: body.encrypted_data,
        nonce: body.nonce,
    };
    let json = serde_json::to_string(&message).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !state
        .device_manager
        .route_message(&auth.user_id, &target_device_id, &json)
    {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
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
) -> Result<axum::response::Response, StatusCode> {
    let auth = validate_user(&state, &headers).await?;
    let db = state.db.as_ref();
    let source_device_id = auth
        .routing_device_id(db)
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?;
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
    // Retain the authenticated controller identity so the receiver can resolve
    // its same-account public key. Correlation still resolves the HTTP response.
    let out_msg = OutboundProtocol::IncomingDeviceMessage {
        source_device_id,
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
        Ok(Ok(resp)) => resp.into_http_response(),
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
    let user_id = auth.user_id.clone();

    if !is_valid_device_id(&target_device_id) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let db = state.db.as_ref();
    let _presence_projection_guard = state.device_manager.lock_presence_projection().await;
    let current_auth = crate::db::AuthToken::find(db, &auth.token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !current_auth.is_device_token()
        || current_auth.user_id != user_id
        || current_auth.device_id != auth.device_id
    {
        return Err(StatusCode::UNAUTHORIZED);
    }

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
    drop(_presence_projection_guard);

    state
        .device_manager
        .broadcast_current_presence(&user_id, |devices| {
            let devices = devices
                .iter()
                .map(
                    |(device_id, device_name)| crate::routes::websocket::DevicePresenceEntry {
                        device_id: device_id.clone(),
                        device_name: device_name.clone(),
                    },
                )
                .collect();
            serde_json::to_string(&OutboundProtocol::DevicePresence { devices }).ok()
        });

    tracing::info!("Device {target_device_id} removed from account {user_id}");
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{connect, AuthToken, DbPool, DeviceRow, UserRow};
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

    #[tokio::test]
    async fn delegated_device_keys_are_scoped_and_follow_parent_revocation() {
        let ctx = setup_app().await;
        let controller = AuthToken::create_keyed_delegated(
            &ctx.db,
            "owner",
            "owner-device",
            "controller-public-key",
        )
        .await
        .unwrap();
        let id = controller.routing_device_id(&ctx.db).await.unwrap();
        assert_ne!(id, "owner-device");
        let second =
            AuthToken::create_keyed_delegated(&ctx.db, "owner", "owner-device", "other-public-key")
                .await
                .unwrap();
        assert_ne!(id, second.routing_device_id(&ctx.db).await.unwrap());
        for (token, expected) in [
            (&ctx.owner_token, StatusCode::OK),
            (&ctx.other_token, StatusCode::NOT_FOUND),
        ] {
            let response = ctx
                .app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/api/devices/{id}/key"))
                        .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
        }
        AuthToken::revoke_by_device(&ctx.db, "owner", "owner-device")
            .await
            .unwrap();
        assert!(controller.routing_device_id(&ctx.db).await.is_err());
        assert!(second.routing_device_id(&ctx.db).await.is_err());
    }

    #[tokio::test]
    async fn http_device_response_requires_expected_principal_and_accepts_large_payload() {
        let ctx = setup_app().await;
        let state = AppState {
            start_time: std::time::Instant::now(),
            asset_store: Arc::new(MemoryAssetStore::new()),
            db: ctx.db.clone(),
            page_data: None,
            page_access_manager: Arc::new(crate::routes::pages::PageAccessManager::new()),
            page_upload_manager: Arc::new(crate::routes::pages::PageUploadManager::new()),
            page_execution_guard: Arc::new(crate::page_execution::PageExecutionGuard::new()),
            login_rate_limiter: Arc::new(crate::routes::auth::LoginRateLimiter::new()),
            device_manager: crate::relay::DeviceManager::new(),
            cors_allow_origins: Arc::new(Vec::new()),
            page_browser_auth: None,
        };
        let app = device_router()
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                crate::admission::admit,
            ))
            .with_state(state.clone());
        let mut pending = state
            .device_manager
            .register_rpc("correlation", "owner", "target-device")
            .unwrap();
        let ciphertext = "a".repeat(256 * 1024);
        let body = serde_json::json!({
            "correlation_id": "correlation", "encrypted_data": ciphertext, "nonce": "nonce"
        })
        .to_string();
        for (token, expected) in [
            (&ctx.owner_token, StatusCode::NOT_FOUND),
            (&ctx.other_token, StatusCode::NOT_FOUND),
            (&ctx.delegated_token, StatusCode::FORBIDDEN),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/devices/owner-device/messages")
                        .header(header::AUTHORIZATION, format!("Bearer {token}"))
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(body.clone()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
            assert!(pending.try_recv().is_err());
        }
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/devices/owner-device/messages")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", ctx.target_token),
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let response = pending.await.unwrap().into_http_response().unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["encrypted_data"], ciphertext);
        assert_eq!(value["nonce"], "nonce");
    }

    #[test]
    fn encrypted_payload_cannot_expand_json_with_escape_characters() {
        assert!(!is_valid_encrypted_payload("\"\\", "nonce"));
        assert!(is_valid_encrypted_payload("ab+/=_-09", "nonce"));
    }

    async fn setup_app() -> TestContext {
        let db = Arc::new(connect(":memory:").await.unwrap());
        UserRow::create(&db, "owner", "alice").await.unwrap();
        UserRow::create(&db, "other", "bob").await.unwrap();
        DeviceRow::upsert(&db, "owner-device", "owner", "Owner", None, None)
            .await
            .unwrap();
        DeviceRow::upsert(&db, "target-device", "owner", "Target", None, None)
            .await
            .unwrap();
        DeviceRow::upsert(&db, "other-device", "other", "Other", None, None)
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
            Arc::new(MemoryAssetStore::new()),
            std::time::Instant::now(),
            db.clone(),
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

    async fn listed_device_ids(app: &axum::Router, token: &str) -> Vec<String> {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/devices")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let entries: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        entries
            .into_iter()
            .map(|entry| entry["device_id"].as_str().unwrap().to_string())
            .collect()
    }

    async fn rpc(
        app: &axum::Router,
        token: &str,
        device_id: &str,
        payload_bytes: usize,
    ) -> StatusCode {
        let body = serde_json::json!({
            "encrypted_data": "A".repeat(payload_bytes),
            "nonce": "AAAA",
        })
        .to_string();
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/devices/{device_id}/rpc"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn device_list_hides_mobile_devices_and_keeps_unlabeled_rows() {
        let ctx = setup_app().await;
        DeviceRow::upsert(
            &ctx.db,
            "phone",
            "owner",
            "HarmonyOS Phone",
            Some("mobile"),
            None,
        )
        .await
        .unwrap();
        DeviceRow::upsert(&ctx.db, "mac", "owner", "MacBook", Some("desktop"), None)
            .await
            .unwrap();

        let ids = listed_device_ids(&ctx.app, &ctx.owner_token).await;

        assert!(!ids.contains(&"phone".to_string()));
        assert!(ids.contains(&"mac".to_string()));
        // owner-device and target-device were registered before the kind
        // existed; a NULL kind must still be offered as a control target.
        assert!(ids.contains(&"owner-device".to_string()));
        assert!(ids.contains(&"target-device".to_string()));
    }

    #[tokio::test]
    async fn a_login_without_a_kind_does_not_erase_a_known_one() {
        let ctx = setup_app().await;
        DeviceRow::upsert(
            &ctx.db,
            "phone",
            "owner",
            "HarmonyOS Phone",
            Some("mobile"),
            None,
        )
        .await
        .unwrap();

        // An older client build logs in again and reports no kind.
        DeviceRow::upsert(&ctx.db, "phone", "owner", "HarmonyOS Phone", None, None)
            .await
            .unwrap();

        let ids = listed_device_ids(&ctx.app, &ctx.owner_token).await;
        assert!(!ids.contains(&"phone".to_string()));
    }

    #[tokio::test]
    async fn device_rpc_accepts_payloads_above_the_default_body_limit() {
        let ctx = setup_app().await;

        // A phone sending a photo lands here: base64 of the image, encrypted and
        // base64ed again, clears Axum's 2 MB default by itself. NOT_FOUND means
        // the body was read and the offline target was the only complaint.
        let status = rpc(&ctx.app, &ctx.owner_token, "target-device", 3 * 1024 * 1024).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn device_rpc_still_rejects_payloads_past_the_route_limit() {
        let ctx = setup_app().await;

        let status = rpc(
            &ctx.app,
            &ctx.owner_token,
            "target-device",
            RPC_BODY_LIMIT_BYTES + 1,
        )
        .await;

        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
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
