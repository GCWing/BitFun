//! WebSocket handler for the relay server.
//!
//! Only desktop clients connect via WebSocket. Mobile clients use HTTP.
//! The relay bridges HTTP requests to the desktop via WebSocket using
//! correlation IDs for request-response matching.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{
    mpsc::{self, error::TrySendError},
    watch,
};
use tracing::{debug, error, info, warn};

use crate::relay::room::{send_outbound_message, ConnId, OutboundMessage, ResponsePayload};
use crate::routes::api::AppState;

const OUTBOUND_QUEUE_CAPACITY: usize = i32::MAX as usize;
const MAX_WS_MESSAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_ENCRYPTED_PAYLOAD_BYTES: usize = 48 * 1024 * 1024;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_DEVICE_NAME_BYTES: usize = 256;
const MAX_PUBLIC_KEY_BYTES: usize = 512;
const MAX_NONCE_BYTES: usize = 256;
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
const MAX_MESSAGES_PER_WINDOW: u32 = i32::MAX as u32;
const DEVICE_TOKEN_REVALIDATION_INTERVAL: Duration = Duration::from_secs(5);

struct ConnectionRateLimiter {
    window_started: Instant,
    message_count: u32,
}

impl ConnectionRateLimiter {
    fn new() -> Self {
        Self {
            window_started: Instant::now(),
            message_count: 0,
        }
    }

    fn allow(&mut self) -> bool {
        if self.window_started.elapsed() >= RATE_LIMIT_WINDOW {
            self.window_started = Instant::now();
            self.message_count = 0;
        }
        if self.message_count >= MAX_MESSAGES_PER_WINDOW {
            return false;
        }
        self.message_count += 1;
        true
    }
}

/// Messages received from the desktop via WebSocket.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InboundMessage {
    CreateRoom {
        room_id: Option<String>,
        device_id: String,
        #[allow(dead_code)]
        device_type: String,
        public_key: String,
    },
    /// Desktop responds to a bridged HTTP request.
    RelayResponse {
        correlation_id: String,
        encrypted_data: String,
        nonce: String,
    },
    Heartbeat,
    /// Account-authenticated connect (parallel to CreateRoom for the device
    /// routing pathway). Validates the token and registers the device.
    AuthConnect {
        token: String,
        device_name: String,
    },
    /// Route an encrypted payload to another device in the same account.
    DeviceMessage {
        target_device_id: String,
        correlation_id: String,
        encrypted_data: String,
        nonce: String,
    },
}

/// Messages sent to the desktop via WebSocket.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutboundProtocol {
    RoomCreated {
        room_id: String,
    },
    /// Mobile pairing request forwarded to desktop.
    PairRequest {
        correlation_id: String,
        public_key: String,
        device_id: String,
        device_name: String,
    },
    /// Encrypted command from mobile forwarded to desktop.
    Command {
        correlation_id: String,
        encrypted_data: String,
        nonce: String,
    },
    HeartbeatAck,
    Error {
        message: String,
    },
    /// Result of an `AuthConnect`: the validated user_id + this device's id.
    AuthOk {
        user_id: String,
        device_id: String,
    },
    AuthError {
        message: String,
    },
    /// A device-to-device message routed from another device in the account.
    IncomingDeviceMessage {
        source_device_id: String,
        correlation_id: String,
        encrypted_data: String,
        nonce: String,
    },
    /// Current online devices in the account (presence broadcast).
    DevicePresence {
        devices: Vec<DevicePresenceEntry>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct DevicePresenceEntry {
    pub device_id: String,
    pub device_name: String,
}

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    if !is_websocket_origin_allowed(&headers, &state.cors_allow_origins) {
        warn!("Rejected WebSocket connection from a disallowed browser origin");
        return StatusCode::FORBIDDEN.into_response();
    }
    ws.max_message_size(MAX_WS_MESSAGE_BYTES)
        .max_frame_size(MAX_WS_MESSAGE_BYTES)
        .max_write_buffer_size(MAX_WS_MESSAGE_BYTES)
        .on_upgrade(move |socket| handle_socket(socket, state))
}

fn is_websocket_origin_allowed(headers: &HeaderMap, allowed_origins: &[String]) -> bool {
    let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
    else {
        // Native clients do not send Origin and authenticate at the protocol
        // layer. Origin is a browser boundary, not a replacement for auth.
        return true;
    };
    if origin.eq_ignore_ascii_case("null") {
        return false;
    }
    let Some(origin) = crate::normalized_browser_origin(origin) else {
        return false;
    };
    if allowed_origins
        .iter()
        .any(|allowed| allowed == "*" || allowed.eq_ignore_ascii_case(&origin))
    {
        return true;
    }

    let Some(host) = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
    else {
        return false;
    };
    let origin_authority = origin
        .split_once("://")
        .map(|(_, authority)| authority)
        .and_then(|authority| authority.split('/').next());
    origin_authority.is_some_and(|authority| authority.eq_ignore_ascii_case(host))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    ensure_device_token_revalidator(&state);
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (out_tx, mut out_rx) = mpsc::channel::<OutboundMessage>(OUTBOUND_QUEUE_CAPACITY);
    let (force_close_tx, mut force_close_rx) = watch::channel(false);

    let conn_id = state.room_manager.next_conn_id();
    let mut rate_limiter = ConnectionRateLimiter::new();
    let mut token_expiry_task: Option<tokio::task::JoinHandle<()>> = None;
    info!("WebSocket connected: conn_id={conn_id}");

    let mut writer_force_close_rx = force_close_rx.clone();
    let writer_failure_close_tx = force_close_tx.clone();
    let write_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                changed = writer_force_close_rx.changed() => {
                    if changed.is_ok() && *writer_force_close_rx.borrow() {
                        info!("Closing revoked WebSocket connection");
                        let _ = ws_sender.send(Message::Close(None)).await;
                    }
                    break;
                }
                msg = out_rx.recv() => {
                    let Some(msg) = msg else { break };
                    if !msg.text.is_empty()
                        && ws_sender
                            .send(Message::Text(msg.text.into()))
                            .await
                            .is_err()
                    {
                        // The read half can remain open after a write-half
                        // failure. Wake the owner loop so it promptly removes
                        // routing/presence instead of leaving a half-open
                        // device online until token expiry.
                        let _ = writer_failure_close_tx.send(true);
                        break;
                    }
                }
            }
        }
    });

    loop {
        let msg_result = tokio::select! {
            biased;
            changed = force_close_rx.changed() => {
                if changed.is_ok() && *force_close_rx.borrow() {
                    info!("Revoked WebSocket connection: conn_id={conn_id}");
                }
                break;
            }
            msg = ws_receiver.next() => {
                let Some(msg) = msg else { break };
                msg
            }
        };
        if !rate_limiter.allow() {
            warn!("WebSocket rate limit exceeded: conn_id={conn_id}");
            let _ = send_json_best_effort(
                &out_tx,
                &OutboundProtocol::Error {
                    message: "message rate limit exceeded".into(),
                },
            );
            break;
        }
        match msg_result {
            Ok(Message::Text(text)) => {
                if !handle_text_message(
                    &text,
                    conn_id,
                    &state,
                    &out_tx,
                    &force_close_tx,
                    &mut token_expiry_task,
                )
                .await
                {
                    break;
                }
            }
            Ok(Message::Ping(_)) => {}
            Ok(Message::Close(_)) => {
                info!("WebSocket close from conn_id={conn_id}");
                break;
            }
            Ok(Message::Binary(_)) => {
                warn!("Rejected binary WebSocket message: conn_id={conn_id}");
                let _ = send_json_best_effort(
                    &out_tx,
                    &OutboundProtocol::Error {
                        message: "binary messages are not supported".into(),
                    },
                );
                break;
            }
            Err(e) => {
                error!("WebSocket error conn_id={conn_id}: {e}");
                break;
            }
            _ => {}
        }
    }

    if let Some(task) = token_expiry_task {
        task.abort();
    }

    state.room_manager.on_disconnect(conn_id);
    let _presence_projection_guard = state.device_manager.lock_presence_projection().await;
    if let Some((user_id, device_id)) = state.device_manager.unregister(conn_id) {
        // Best-effort: mark the device offline in the DB and notify peers.
        if !state.device_manager.is_device_online(&user_id, &device_id) {
            if let Some(db) = state.db.as_ref() {
                let _ = crate::db::DeviceRow::set_online(db, &user_id, &device_id, false).await;
            }
        }
        state
            .device_manager
            .broadcast_current_presence(&user_id, |devices| {
                serde_json::to_string(&OutboundProtocol::DevicePresence {
                    devices: build_presence(devices),
                })
                .ok()
            });
    }
    drop(_presence_projection_guard);
    drop(out_tx);
    let _ = write_task.await;
    info!("WebSocket disconnected: conn_id={conn_id}");
}

fn ensure_device_token_revalidator(state: &AppState) {
    let Some(db) = state.db.clone() else {
        return;
    };
    if !state.device_manager.claim_token_revalidator_start() {
        return;
    }
    let device_manager = Arc::clone(&state.device_manager);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(DEVICE_TOKEN_REVALIDATION_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Consume the immediate first tick. AuthConnect performs its own two
        // checks; this worker is for later out-of-process revocation.
        interval.tick().await;
        loop {
            interval.tick().await;
            if let Err(error) = revalidate_active_device_tokens_once(&db, &device_manager).await {
                warn!(%error, "Failed to revalidate active device tokens");
            }
        }
    });
}

async fn revalidate_active_device_tokens_once(
    db: &crate::db::DbPool,
    device_manager: &crate::relay::DeviceManager,
) -> anyhow::Result<usize> {
    let active = device_manager.active_device_credentials();
    if active.is_empty() {
        return Ok(0);
    }
    let tokens = active
        .iter()
        .map(|(_, _, _, token)| token.clone())
        .collect::<Vec<_>>();
    let valid = crate::db::AuthToken::find_valid_device_tokens(db, &tokens)
        .await?
        .into_iter()
        .map(|token| (token.user_id, token.device_id, token.token))
        .collect::<HashSet<_>>();
    let revoked = active
        .into_iter()
        .filter(|(_, user_id, device_id, token)| {
            !valid.contains(&(user_id.clone(), device_id.clone(), token.clone()))
        })
        .collect::<Vec<_>>();
    if revoked.is_empty() {
        return Ok(0);
    }

    // A batch snapshot can race a same-process login/logout. Re-check each
    // missing exact token under the lifecycle gate before removing its socket.
    let _presence_projection_guard = device_manager.lock_presence_projection().await;
    let mut affected_users = HashSet::new();
    let mut disconnected = 0;
    for (_conn_id, user_id, device_id, token) in revoked {
        match registered_device_token_is_current(db, &token, &user_id, &device_id).await {
            Ok(true) => continue,
            Err(error) => {
                warn!(%error, %user_id, %device_id, "Failed exact token revalidation");
                continue;
            }
            Ok(false) => {}
        }
        if device_manager.disconnect_device_if_token(&user_id, &device_id, &token) {
            disconnected += 1;
            affected_users.insert(user_id.clone());
            if !device_manager.is_device_online(&user_id, &device_id) {
                if let Err(error) =
                    crate::db::DeviceRow::set_online(db, &user_id, &device_id, false).await
                {
                    warn!(%error, %user_id, %device_id, "Failed to project revoked device offline");
                }
            }
        }
    }
    for user_id in affected_users {
        device_manager.broadcast_current_presence(&user_id, |devices| {
            serde_json::to_string(&OutboundProtocol::DevicePresence {
                devices: build_presence(devices),
            })
            .ok()
        });
    }
    Ok(disconnected)
}

async fn handle_text_message(
    text: &str,
    conn_id: ConnId,
    state: &AppState,
    out_tx: &mpsc::Sender<OutboundMessage>,
    force_close_tx: &watch::Sender<bool>,
    token_expiry_task: &mut Option<tokio::task::JoinHandle<()>>,
) -> bool {
    if text.len() > MAX_WS_MESSAGE_BYTES {
        return reject_protocol(out_tx, "message is too large");
    }
    let msg: InboundMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            warn!("Invalid message from conn_id={conn_id}: {e}");
            return send_json_best_effort(
                out_tx,
                &OutboundProtocol::Error {
                    message: "invalid message format".into(),
                },
            );
        }
    };
    let message_type = match &msg {
        InboundMessage::CreateRoom { .. } => "create_room",
        InboundMessage::RelayResponse { .. } => "relay_response",
        InboundMessage::Heartbeat => "heartbeat",
        InboundMessage::AuthConnect { .. } => "auth_connect",
        InboundMessage::DeviceMessage { .. } => "device_message",
    };
    debug!(
        "Received WebSocket message: conn_id={conn_id} type={message_type} bytes={}",
        text.len()
    );

    match msg {
        InboundMessage::CreateRoom {
            room_id,
            device_id,
            device_type,
            public_key,
        } => {
            if state.device_manager.conn_mapping(conn_id).is_some() {
                return reject_protocol(
                    out_tx,
                    "an authenticated device connection cannot create a pairing room",
                );
            }
            if !is_valid_identifier(&device_id)
                || !is_valid_display_text(&device_type, 32)
                || !is_valid_display_text(&public_key, MAX_PUBLIC_KEY_BYTES)
                || room_id
                    .as_deref()
                    .is_some_and(|value| !crate::relay::room::is_valid_room_id(value))
            {
                return reject_protocol(out_tx, "invalid room parameters");
            }
            let room_id = room_id.unwrap_or_else(generate_room_id);
            let ok = state.room_manager.create_room(
                &room_id,
                conn_id,
                &device_id,
                &public_key,
                out_tx.clone(),
            );
            if ok {
                send_json(out_tx, &OutboundProtocol::RoomCreated { room_id }).await
            } else {
                send_json(
                    out_tx,
                    &OutboundProtocol::Error {
                        message: "failed to create room".into(),
                    },
                )
                .await
            }
        }

        InboundMessage::RelayResponse {
            correlation_id,
            encrypted_data,
            nonce,
        } => {
            if !is_valid_identifier(&correlation_id)
                || !is_valid_encrypted_payload(&encrypted_data, &nonce)
            {
                return reject_protocol(out_tx, "invalid relay response");
            }
            debug!("RelayResponse from desktop conn_id={conn_id} corr={correlation_id}");
            if !state.room_manager.resolve_pending_from_conn(
                conn_id,
                &correlation_id,
                ResponsePayload {
                    encrypted_data,
                    nonce,
                },
            ) {
                return reject_protocol(out_tx, "relay response does not match this room");
            }
            true
        }

        InboundMessage::Heartbeat => {
            // Account-authenticated device connections have no room; treat
            // heartbeat as a keepalive ack when the conn is registered.
            if state.room_manager.heartbeat(conn_id)
                || state.device_manager.conn_mapping(conn_id).is_some()
            {
                send_json_best_effort(out_tx, &OutboundProtocol::HeartbeatAck)
            } else {
                send_json_best_effort(
                    out_tx,
                    &OutboundProtocol::Error {
                        message: "Room not found or expired".into(),
                    },
                )
            }
        }

        InboundMessage::AuthConnect { token, device_name } => {
            if state.room_manager.has_connection(conn_id)
                || state.device_manager.has_connection(conn_id)
            {
                return reject_protocol(out_tx, "connection is already authenticated");
            }
            if !crate::db::is_valid_auth_token(&token)
                || !is_valid_display_text(&device_name, MAX_DEVICE_NAME_BYTES)
            {
                return reject_protocol(out_tx, "invalid authentication parameters");
            }
            let Some(db) = state.db.as_ref() else {
                return send_json_best_effort(
                    out_tx,
                    &OutboundProtocol::AuthError {
                        message: "account features disabled".into(),
                    },
                );
            };
            let auth = match crate::db::AuthToken::find(db, &token).await {
                Ok(Some(a)) => a,
                _ => {
                    return send_json_best_effort(
                        out_tx,
                        &OutboundProtocol::AuthError {
                            message: "invalid or expired token".into(),
                        },
                    )
                }
            };
            if !auth.is_device_token() {
                return send_json_best_effort(
                    out_tx,
                    &OutboundProtocol::AuthError {
                        message: "token is not valid for a device connection".into(),
                    },
                );
            }
            // Make the candidate visible to token-scoped logout without
            // exposing it to presence/routing or replacing the current active
            // socket until the mandatory final token check succeeds.
            state.device_manager.register_pending(
                &auth.user_id,
                &auth.device_id,
                &token,
                &device_name,
                conn_id,
                out_tx.clone(),
                force_close_tx.clone(),
            );

            let activated = match activate_pending_device_if_authorized(
                db,
                &state.device_manager,
                &token,
                &auth.user_id,
                &auth.device_id,
                &device_name,
                conn_id,
            )
            .await
            {
                Ok(activated) => activated,
                Err(error) => {
                    warn!(%error, "Failed to activate authenticated device");
                    false
                }
            };
            if !activated {
                return false;
            }
            let expires_in = auth
                .expires_at
                .saturating_sub(chrono::Utc::now().timestamp())
                .max(0) as u64;
            let expiry_close_tx = force_close_tx.clone();
            *token_expiry_task = Some(tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(expires_in)).await;
                let _ = expiry_close_tx.send(true);
            }));
            // Full presence (including self) so clients can treat the snapshot
            // as authoritative rather than an incremental patch. Snapshot and
            // enqueue are serialized with membership mutations.
            state
                .device_manager
                .broadcast_current_presence(&auth.user_id, |devices| {
                    serde_json::to_string(&OutboundProtocol::DevicePresence {
                        devices: build_presence(devices),
                    })
                    .ok()
                });
            true
        }

        InboundMessage::DeviceMessage {
            target_device_id,
            correlation_id,
            encrypted_data,
            nonce,
        } => {
            if !is_valid_identifier(&target_device_id)
                || !is_valid_identifier(&correlation_id)
                || !is_valid_encrypted_payload(&encrypted_data, &nonce)
            {
                return reject_protocol(out_tx, "invalid device message");
            }
            // Look up the sender's (user_id, device_id) from the conn map.
            let sender = state.device_manager.conn_mapping(conn_id);
            let Some((user_id, source_device_id)) = sender else {
                return send_json_best_effort(
                    out_tx,
                    &OutboundProtocol::Error {
                        message: "not authenticated (send AuthConnect first)".into(),
                    },
                );
            };

            // First check: is this a response to a pending HTTP RPC?
            // If so, resolve the pending future and don't forward via WS.
            let rpc_response = crate::relay::device_manager::RpcResponse {
                encrypted_data: encrypted_data.clone(),
                nonce: nonce.clone(),
            };
            if state.device_manager.resolve_rpc(
                &correlation_id,
                &user_id,
                &source_device_id,
                rpc_response,
            ) {
                // HTTP RPC resolved — the HTTP caller gets the response.
                return true;
            }

            // Normal WS-to-WS device routing
            let out_msg = OutboundProtocol::IncomingDeviceMessage {
                source_device_id,
                correlation_id,
                encrypted_data,
                nonce,
            };
            let json = serde_json::to_string(&out_msg).unwrap_or_default();
            if !state
                .device_manager
                .route_message(&user_id, &target_device_id, &json)
            {
                return send_json_best_effort(
                    out_tx,
                    &OutboundProtocol::Error {
                        message: format!("target device {target_device_id} offline"),
                    },
                );
            }
            true
        }
    }
}

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

fn reject_protocol(tx: &mpsc::Sender<OutboundMessage>, message: &str) -> bool {
    let _ = send_json_best_effort(
        tx,
        &OutboundProtocol::Error {
            message: message.to_string(),
        },
    );
    false
}

fn build_presence(devices: &[(String, String)]) -> Vec<DevicePresenceEntry> {
    devices
        .iter()
        .map(|(id, name)| DevicePresenceEntry {
            device_id: id.clone(),
            device_name: name.clone(),
        })
        .collect()
}

async fn registered_device_token_is_current(
    db: &crate::db::DbPool,
    token: &str,
    expected_user_id: &str,
    expected_device_id: &str,
) -> anyhow::Result<bool> {
    Ok(crate::db::AuthToken::find(db, token)
        .await?
        .is_some_and(|current| {
            current.is_device_token()
                && current.user_id == expected_user_id
                && current.device_id == expected_device_id
        }))
}

async fn project_device_offline_if_unowned(
    db: &crate::db::DbPool,
    device_manager: &crate::relay::DeviceManager,
    user_id: &str,
    device_id: &str,
) -> anyhow::Result<()> {
    if !device_manager.is_device_online(user_id, device_id) {
        crate::db::DeviceRow::set_online(db, user_id, device_id, false).await?;
    }
    Ok(())
}

async fn reconcile_device_after_token_disconnect(
    db: &crate::db::DbPool,
    device_manager: &crate::relay::DeviceManager,
    user_id: &str,
    device_id: &str,
) -> anyhow::Result<()> {
    let projection_result =
        project_device_offline_if_unowned(db, device_manager, user_id, device_id).await;
    device_manager.broadcast_current_presence(user_id, |devices| {
        serde_json::to_string(&OutboundProtocol::DevicePresence {
            devices: build_presence(devices),
        })
        .ok()
    });
    projection_result
}

/// Complete the activation after the durable device projection has been
/// written. The caller must hold `presence_projection_gate`, keeping the
/// second token check, in-memory promotion, and any rollback atomic with
/// logout, device deletion, and socket cleanup.
async fn complete_pending_device_activation_if_authorized(
    db: &crate::db::DbPool,
    device_manager: &crate::relay::DeviceManager,
    token: &str,
    expected_user_id: &str,
    expected_device_id: &str,
    conn_id: ConnId,
) -> anyhow::Result<bool> {
    // The durable writes can wait on SQLite. Re-check wall-clock expiry
    // immediately before the synchronous promotion so a token that expired
    // during that wait never becomes routable or receives AuthOk.
    let still_current =
        match registered_device_token_is_current(db, token, expected_user_id, expected_device_id)
            .await
        {
            Ok(current) => current,
            Err(error) => {
                device_manager.disconnect_pending(conn_id);
                let _ = project_device_offline_if_unowned(
                    db,
                    device_manager,
                    expected_user_id,
                    expected_device_id,
                )
                .await;
                return Err(error);
            }
        };
    if !still_current {
        device_manager.disconnect_device_if_token(expected_user_id, expected_device_id, token);
        reconcile_device_after_token_disconnect(
            db,
            device_manager,
            expected_user_id,
            expected_device_id,
        )
        .await?;
        return Ok(false);
    }

    // Pending remains invisible until every durable write succeeds. Promotion
    // is synchronous under the same lifecycle gate, so clients never route to
    // a socket whose AuthConnect may still fail on database projection.
    let auth_ok = serde_json::to_string(&OutboundProtocol::AuthOk {
        user_id: expected_user_id.to_string(),
        device_id: expected_device_id.to_string(),
    })?;
    if !device_manager.activate_pending_with_initial_message(
        expected_user_id,
        expected_device_id,
        token,
        conn_id,
        &auth_ok,
    ) {
        project_device_offline_if_unowned(db, device_manager, expected_user_id, expected_device_id)
            .await?;
        return Ok(false);
    }
    Ok(true)
}

async fn activate_pending_device_if_authorized(
    db: &crate::db::DbPool,
    device_manager: &crate::relay::DeviceManager,
    token: &str,
    expected_user_id: &str,
    expected_device_id: &str,
    device_name: &str,
    conn_id: ConnId,
) -> anyhow::Result<bool> {
    // Serialize the final token lookup, durable device update, activation, and
    // online projection with logout/delete/socket cleanup. No persistent side
    // effect occurs before this lookup, so a deleted device cannot be revived
    // by an AuthConnect request that passed only the earlier lookup.
    let _presence_projection_guard = device_manager.lock_presence_projection().await;
    if !registered_device_token_is_current(db, token, expected_user_id, expected_device_id).await? {
        device_manager.disconnect_device_if_token(expected_user_id, expected_device_id, token);
        reconcile_device_after_token_disconnect(
            db,
            device_manager,
            expected_user_id,
            expected_device_id,
        )
        .await?;
        return Ok(false);
    }

    if let Err(error) =
        crate::db::DeviceRow::upsert(db, expected_device_id, expected_user_id, device_name, None)
            .await
    {
        device_manager.disconnect_pending(conn_id);
        return Err(error);
    }
    if let Err(error) =
        crate::db::DeviceRow::set_online(db, expected_user_id, expected_device_id, true).await
    {
        device_manager.disconnect_pending(conn_id);
        return Err(error);
    }
    complete_pending_device_activation_if_authorized(
        db,
        device_manager,
        token,
        expected_user_id,
        expected_device_id,
        conn_id,
    )
    .await
}

async fn send_json<T: Serialize>(tx: &mpsc::Sender<OutboundMessage>, msg: &T) -> bool {
    match serde_json::to_string(msg) {
        Ok(json) => send_outbound_message(tx, OutboundMessage::text(json)).await,
        Err(e) => {
            warn!("Failed to serialize outbound websocket message: {e}");
            false
        }
    }
}

fn send_json_best_effort<T: Serialize>(tx: &mpsc::Sender<OutboundMessage>, msg: &T) -> bool {
    match serde_json::to_string(msg) {
        Ok(json) => match tx.try_send(OutboundMessage::text(json)) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                warn!("Outbound websocket queue is full; dropping best-effort control response");
                true
            }
            Err(TrySendError::Closed(_)) => false,
        },
        Err(e) => {
            warn!("Failed to serialize outbound websocket message: {e}");
            false
        }
    }
}

fn generate_room_id() -> String {
    let bytes: [u8; 6] = rand::random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        activate_pending_device_if_authorized, complete_pending_device_activation_if_authorized,
        is_valid_display_text, is_valid_encrypted_payload, is_valid_identifier,
        is_websocket_origin_allowed, registered_device_token_is_current,
        revalidate_active_device_tokens_once, send_json_best_effort, ConnectionRateLimiter,
        OutboundProtocol, MAX_MESSAGES_PER_WINDOW,
    };
    use crate::db::{connect, AuthToken, DeviceRow, UserRow};
    use crate::relay::DeviceManager;
    use axum::http::{header, HeaderMap};
    use tokio::sync::{mpsc, watch};

    #[test]
    fn best_effort_control_response_does_not_block_on_full_queue() {
        let (tx, _rx) = mpsc::channel(1);
        assert!(send_json_best_effort(&tx, &OutboundProtocol::HeartbeatAck));

        assert!(
            send_json_best_effort(&tx, &OutboundProtocol::HeartbeatAck),
            "full queue should drop best-effort control response without closing read loop"
        );
    }

    #[test]
    fn protocol_fields_are_bounded() {
        assert!(is_valid_identifier("device-1"));
        assert!(!is_valid_identifier("../device"));
        assert!(!is_valid_identifier(&"x".repeat(129)));
        assert!(is_valid_display_text("MacBook Pro", 256));
        assert!(!is_valid_display_text("bad\nname", 256));
        assert!(is_valid_encrypted_payload("ciphertext", "nonce"));
        assert!(!is_valid_encrypted_payload("", "nonce"));
    }

    #[test]
    fn websocket_message_rate_is_effectively_unbounded() {
        assert_eq!(MAX_MESSAGES_PER_WINDOW, i32::MAX as u32);
        let mut limiter = ConnectionRateLimiter::new();
        for _ in 0..64 {
            assert!(limiter.allow());
        }
    }

    #[test]
    fn websocket_browser_origin_is_same_origin_or_explicitly_allowed() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "relay.example.com".parse().unwrap());
        headers.insert(header::ORIGIN, "https://relay.example.com".parse().unwrap());
        assert!(is_websocket_origin_allowed(&headers, &[]));

        headers.insert(header::ORIGIN, "https://app.example.com".parse().unwrap());
        assert!(!is_websocket_origin_allowed(&headers, &[]));
        assert!(is_websocket_origin_allowed(
            &headers,
            &["https://app.example.com".to_string()]
        ));

        headers.insert(header::ORIGIN, "null".parse().unwrap());
        assert!(!is_websocket_origin_allowed(&headers, &["*".to_string()]));

        for invalid in [
            "file://relay.example.com",
            "https://relay.example.com/path",
            "https://user@relay.example.com",
        ] {
            headers.insert(header::ORIGIN, invalid.parse().unwrap());
            assert!(!is_websocket_origin_allowed(&headers, &[]));
        }
    }

    #[tokio::test]
    async fn token_revoked_between_initial_validation_and_activation_is_rejected() {
        let db = connect(":memory:").await.unwrap();
        UserRow::create(&db, "owner", "alice", "s", "ks", "{}", "hash", "wmk")
            .await
            .unwrap();
        DeviceRow::upsert(&db, "device-a", "owner", "Device A", None)
            .await
            .unwrap();
        let token = AuthToken::create(&db, "owner", "device-a")
            .await
            .unwrap()
            .token;

        // This is the first AuthConnect lookup. Pause the conceptual request
        // after it, then let logout delete the row before registration's
        // mandatory second lookup.
        assert!(AuthToken::find(&db, &token).await.unwrap().is_some());
        sqlx::query("DELETE FROM auth_tokens WHERE token = ?")
            .bind(&token)
            .execute(&db)
            .await
            .unwrap();

        let manager = DeviceManager::new();
        let (tx, _rx) = mpsc::channel(4);
        let (close_tx, mut close_rx) = watch::channel(false);
        manager.register_pending("owner", "device-a", &token, "Device A", 1, tx, close_tx);
        assert!(manager.online_devices("owner").is_empty());
        assert!(manager.conn_mapping(1).is_none());

        assert!(
            !registered_device_token_is_current(&db, &token, "owner", "device-a")
                .await
                .unwrap()
        );
        assert!(!activate_pending_device_if_authorized(
            &db, &manager, &token, "owner", "device-a", "Device A", 1,
        )
        .await
        .unwrap());
        close_rx.changed().await.unwrap();
        assert!(*close_rx.borrow());
        assert!(manager.conn_mapping(1).is_none());
    }

    #[tokio::test]
    async fn expired_token_disconnects_active_and_pending_without_ghost_online_projection() {
        let db = connect(":memory:").await.unwrap();
        UserRow::create(&db, "owner", "alice", "s", "ks", "{}", "hash", "wmk")
            .await
            .unwrap();
        DeviceRow::upsert(&db, "device-a", "owner", "Device A", None)
            .await
            .unwrap();
        DeviceRow::set_online(&db, "owner", "device-a", true)
            .await
            .unwrap();
        let token = AuthToken::create(&db, "owner", "device-a")
            .await
            .unwrap()
            .token;

        let manager = DeviceManager::new();
        let (active_tx, _active_rx) = mpsc::channel(4);
        let (active_close_tx, mut active_close_rx) = watch::channel(false);
        manager.register(
            "owner",
            "device-a",
            &token,
            "Device A",
            1,
            active_tx,
            active_close_tx,
        );
        let (pending_tx, _pending_rx) = mpsc::channel(4);
        let (pending_close_tx, mut pending_close_rx) = watch::channel(false);
        manager.register_pending(
            "owner",
            "device-a",
            &token,
            "Device A",
            2,
            pending_tx,
            pending_close_tx,
        );
        let (peer_tx, mut peer_rx) = mpsc::channel(4);
        let (peer_close_tx, _peer_close_rx) = watch::channel(false);
        manager.register(
            "owner",
            "device-c",
            "independent-token",
            "Device C",
            3,
            peer_tx,
            peer_close_tx,
        );

        sqlx::query("UPDATE auth_tokens SET expires_at = ? WHERE token = ?")
            .bind(chrono::Utc::now().timestamp())
            .bind(&token)
            .execute(&db)
            .await
            .unwrap();
        assert!(!activate_pending_device_if_authorized(
            &db, &manager, &token, "owner", "device-a", "Device A", 2,
        )
        .await
        .unwrap());

        active_close_rx.changed().await.unwrap();
        pending_close_rx.changed().await.unwrap();
        assert!(*active_close_rx.borrow());
        assert!(*pending_close_rx.borrow());
        assert!(manager.conn_mapping(1).is_none());
        assert!(manager.conn_mapping(2).is_none());
        assert_eq!(
            manager.conn_mapping(3),
            Some(("owner".into(), "device-c".into()))
        );
        assert!(!manager.route_message("owner", "device-a", "opaque"));
        let presence: serde_json::Value =
            serde_json::from_str(&peer_rx.recv().await.unwrap().text).unwrap();
        assert_eq!(presence["type"], "device_presence");
        assert_eq!(presence["devices"].as_array().unwrap().len(), 1);
        assert_eq!(presence["devices"][0]["device_id"], "device-c");
        let rows = DeviceRow::list_by_user(&db, "owner").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].online, 0);
    }

    #[tokio::test]
    async fn external_token_revocation_reaper_disconnects_idle_device_and_updates_presence() {
        let db = connect(":memory:").await.unwrap();
        UserRow::create(&db, "owner", "alice", "s", "ks", "{}", "hash", "wmk")
            .await
            .unwrap();
        DeviceRow::upsert(&db, "device-a", "owner", "Device A", None)
            .await
            .unwrap();
        DeviceRow::upsert(&db, "device-c", "owner", "Device C", None)
            .await
            .unwrap();
        DeviceRow::set_online(&db, "owner", "device-a", true)
            .await
            .unwrap();
        DeviceRow::set_online(&db, "owner", "device-c", true)
            .await
            .unwrap();
        let revoked_token = AuthToken::create(&db, "owner", "device-a")
            .await
            .unwrap()
            .token;
        let peer_token = AuthToken::create(&db, "owner", "device-c")
            .await
            .unwrap()
            .token;

        let manager = DeviceManager::new();
        let (revoked_tx, _revoked_rx) = mpsc::channel(4);
        let (revoked_close_tx, mut revoked_close_rx) = watch::channel(false);
        manager.register(
            "owner",
            "device-a",
            &revoked_token,
            "Device A",
            1,
            revoked_tx,
            revoked_close_tx,
        );
        let (peer_tx, mut peer_rx) = mpsc::channel(4);
        let (peer_close_tx, _peer_close_rx) = watch::channel(false);
        manager.register(
            "owner",
            "device-c",
            &peer_token,
            "Device C",
            2,
            peer_tx,
            peer_close_tx,
        );

        // Simulate reset-password/delete-user tooling mutating the shared DB
        // from outside the running relay process while both sockets are idle.
        sqlx::query("DELETE FROM auth_tokens WHERE token = ?")
            .bind(&revoked_token)
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(
            revalidate_active_device_tokens_once(&db, &manager)
                .await
                .unwrap(),
            1
        );

        revoked_close_rx.changed().await.unwrap();
        assert!(*revoked_close_rx.borrow());
        assert!(manager.conn_mapping(1).is_none());
        assert_eq!(
            manager.conn_mapping(2),
            Some(("owner".into(), "device-c".into()))
        );
        let presence: serde_json::Value =
            serde_json::from_str(&peer_rx.recv().await.unwrap().text).unwrap();
        assert_eq!(presence["devices"].as_array().unwrap().len(), 1);
        assert_eq!(presence["devices"][0]["device_id"], "device-c");
        let rows = DeviceRow::list_by_user(&db, "owner").await.unwrap();
        assert_eq!(
            rows.iter()
                .find(|row| row.device_id == "device-a")
                .unwrap()
                .online,
            0
        );
        assert_eq!(
            rows.iter()
                .find(|row| row.device_id == "device-c")
                .unwrap()
                .online,
            1
        );
    }

    #[tokio::test]
    async fn token_expiring_during_durable_projection_never_receives_auth_ok() {
        let db = connect(":memory:").await.unwrap();
        UserRow::create(&db, "owner", "alice", "s", "ks", "{}", "hash", "wmk")
            .await
            .unwrap();
        DeviceRow::upsert(&db, "device-a", "owner", "Device A", None)
            .await
            .unwrap();
        let token = AuthToken::create(&db, "owner", "device-a")
            .await
            .unwrap()
            .token;
        let manager = DeviceManager::new();
        let (tx, mut rx) = mpsc::channel(4);
        let (close_tx, mut close_rx) = watch::channel(false);
        manager.register_pending("owner", "device-a", &token, "Device A", 1, tx, close_tx);

        let _projection_guard = manager.lock_presence_projection().await;
        assert!(
            registered_device_token_is_current(&db, &token, "owner", "device-a")
                .await
                .unwrap()
        );
        DeviceRow::upsert(&db, "device-a", "owner", "Device A", None)
            .await
            .unwrap();
        DeviceRow::set_online(&db, "owner", "device-a", true)
            .await
            .unwrap();
        sqlx::query("UPDATE auth_tokens SET expires_at = ? WHERE token = ?")
            .bind(chrono::Utc::now().timestamp())
            .bind(&token)
            .execute(&db)
            .await
            .unwrap();

        assert!(!complete_pending_device_activation_if_authorized(
            &db, &manager, &token, "owner", "device-a", 1,
        )
        .await
        .unwrap());
        close_rx.changed().await.unwrap();
        assert!(*close_rx.borrow());
        assert!(rx.try_recv().is_err(), "AuthOk must not be queued");
        assert!(manager.conn_mapping(1).is_none());
        let rows = DeviceRow::list_by_user(&db, "owner").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].online, 0);
    }

    #[tokio::test]
    async fn stale_auth_connect_cannot_recreate_a_deleted_device() {
        let db = connect(":memory:").await.unwrap();
        UserRow::create(&db, "owner", "alice", "s", "ks", "{}", "hash", "wmk")
            .await
            .unwrap();
        DeviceRow::upsert(&db, "device-a", "owner", "Device A", None)
            .await
            .unwrap();
        let token = AuthToken::create(&db, "owner", "device-a")
            .await
            .unwrap()
            .token;
        assert!(AuthToken::find(&db, &token).await.unwrap().is_some());

        assert!(DeviceRow::delete_for_user(&db, "owner", "device-a")
            .await
            .unwrap());
        let manager = DeviceManager::new();
        let (tx, _rx) = mpsc::channel(4);
        let (close_tx, mut close_rx) = watch::channel(false);
        manager.register_pending("owner", "device-a", &token, "Device A", 1, tx, close_tx);

        assert!(!activate_pending_device_if_authorized(
            &db, &manager, &token, "owner", "device-a", "Device A", 1,
        )
        .await
        .unwrap());
        close_rx.changed().await.unwrap();
        assert!(*close_rx.borrow());
        assert!(DeviceRow::list_by_user(&db, "owner")
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn pending_device_becomes_routable_only_after_durable_projection() {
        let db = connect(":memory:").await.unwrap();
        UserRow::create(&db, "owner", "alice", "s", "ks", "{}", "hash", "wmk")
            .await
            .unwrap();
        DeviceRow::upsert(&db, "device-a", "owner", "Device A", None)
            .await
            .unwrap();
        let token = AuthToken::create(&db, "owner", "device-a")
            .await
            .unwrap()
            .token;
        let manager = DeviceManager::new();
        let (tx, mut rx) = mpsc::channel(4);
        let (close_tx, _close_rx) = watch::channel(false);
        manager.register_pending("owner", "device-a", &token, "Device A", 1, tx, close_tx);

        assert!(manager.online_devices("owner").is_empty());
        assert!(!manager.route_message("owner", "device-a", "opaque"));
        assert!(activate_pending_device_if_authorized(
            &db, &manager, &token, "owner", "device-a", "Device A", 1,
        )
        .await
        .unwrap());

        assert_eq!(
            manager.conn_mapping(1),
            Some(("owner".into(), "device-a".into()))
        );
        assert_eq!(manager.online_devices("owner").len(), 1);
        let rows = DeviceRow::list_by_user(&db, "owner").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].online, 1);

        manager.broadcast_current_presence("owner", |devices| {
            serde_json::to_string(&OutboundProtocol::DevicePresence {
                devices: super::build_presence(devices),
            })
            .ok()
        });
        let auth_ok: serde_json::Value =
            serde_json::from_str(&rx.recv().await.unwrap().text).unwrap();
        let presence: serde_json::Value =
            serde_json::from_str(&rx.recv().await.unwrap().text).unwrap();
        assert_eq!(auth_ok["type"], "auth_ok");
        assert_eq!(presence["type"], "device_presence");
    }
}
