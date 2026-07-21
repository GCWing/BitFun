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
use std::time::{Duration, Instant};
use tokio::sync::{
    mpsc::{self, error::TrySendError},
    watch,
};
use tracing::{debug, error, info, warn};

use crate::relay::room::{send_outbound_message, ConnId, OutboundMessage, ResponsePayload};
use crate::routes::api::AppState;

const OUTBOUND_QUEUE_CAPACITY: usize = 256;
const MAX_WS_MESSAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_ENCRYPTED_PAYLOAD_BYTES: usize = 48 * 1024 * 1024;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_DEVICE_NAME_BYTES: usize = 256;
const MAX_PUBLIC_KEY_BYTES: usize = 512;
const MAX_NONCE_BYTES: usize = 256;
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
const MAX_MESSAGES_PER_WINDOW: u32 = 600;

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
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (out_tx, mut out_rx) = mpsc::channel::<OutboundMessage>(OUTBOUND_QUEUE_CAPACITY);
    let (force_close_tx, mut force_close_rx) = watch::channel(false);

    let conn_id = state.room_manager.next_conn_id();
    let mut rate_limiter = ConnectionRateLimiter::new();
    let mut token_expiry_task: Option<tokio::task::JoinHandle<()>> = None;
    info!("WebSocket connected: conn_id={conn_id}");

    let mut writer_force_close_rx = force_close_rx.clone();
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
                        break;
                    }
                }
            }
        }
    });

    loop {
        let msg_result = tokio::select! {
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
    if let Some((user_id, device_id)) = state.device_manager.unregister(conn_id) {
        // Best-effort: mark the device offline in the DB and notify peers.
        if let Some(db) = state.db.as_ref() {
            let _ = crate::db::DeviceRow::set_online(db, &user_id, &device_id, false).await;
        }
        let remaining = state.device_manager.online_devices(&user_id);
        let presence = build_presence(&remaining);
        state.device_manager.broadcast_except(
            &user_id,
            &device_id,
            &serde_json::to_string(&OutboundProtocol::DevicePresence { devices: presence })
                .unwrap_or_default(),
        );
    }
    drop(out_tx);
    let _ = write_task.await;
    info!("WebSocket disconnected: conn_id={conn_id}");
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
                || state.device_manager.conn_mapping(conn_id).is_some()
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
            // Mark the device online in the DB and the in-memory registry.
            let _ = crate::db::DeviceRow::upsert(
                db,
                &auth.device_id,
                &auth.user_id,
                &device_name,
                None,
            )
            .await;
            let _ =
                crate::db::DeviceRow::set_online(db, &auth.user_id, &auth.device_id, true).await;
            let _others = state.device_manager.register(
                &auth.user_id,
                &auth.device_id,
                &device_name,
                conn_id,
                out_tx.clone(),
                force_close_tx.clone(),
            );
            let expires_in = auth
                .expires_at
                .saturating_sub(chrono::Utc::now().timestamp())
                .max(0) as u64;
            let expiry_close_tx = force_close_tx.clone();
            *token_expiry_task = Some(tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(expires_in)).await;
                let _ = expiry_close_tx.send(true);
            }));
            send_json(
                out_tx,
                &OutboundProtocol::AuthOk {
                    user_id: auth.user_id.clone(),
                    device_id: auth.device_id.clone(),
                },
            )
            .await;
            // Full presence (including self) so clients can treat the snapshot
            // as authoritative rather than an incremental patch.
            let all_online = state.device_manager.online_devices(&auth.user_id);
            let presence = build_presence(&all_online);
            send_json_best_effort(
                out_tx,
                &OutboundProtocol::DevicePresence {
                    devices: presence.clone(),
                },
            );
            state.device_manager.broadcast_except(
                &auth.user_id,
                &auth.device_id,
                &serde_json::to_string(&OutboundProtocol::DevicePresence { devices: presence })
                    .unwrap_or_default(),
            );
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
        is_valid_display_text, is_valid_encrypted_payload, is_valid_identifier,
        is_websocket_origin_allowed, send_json_best_effort, ConnectionRateLimiter,
        OutboundProtocol, MAX_MESSAGES_PER_WINDOW,
    };
    use axum::http::{header, HeaderMap};
    use tokio::sync::mpsc;

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
    fn websocket_message_rate_is_bounded() {
        let mut limiter = ConnectionRateLimiter::new();
        for _ in 0..MAX_MESSAGES_PER_WINDOW {
            assert!(limiter.allow());
        }
        assert!(!limiter.allow());
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
}
