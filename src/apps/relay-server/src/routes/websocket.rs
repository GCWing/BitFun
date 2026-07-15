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
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, error::TrySendError};
use tracing::{debug, error, info, warn};

use crate::relay::room::{send_outbound_message, ConnId, OutboundMessage, ResponsePayload};
use crate::routes::api::AppState;

const OUTBOUND_QUEUE_CAPACITY: usize = 256;

fn truncate_preview(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }

    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
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

pub async fn websocket_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.max_message_size(64 * 1024 * 1024)
        .max_frame_size(64 * 1024 * 1024)
        .max_write_buffer_size(64 * 1024 * 1024)
        .on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (out_tx, mut out_rx) = mpsc::channel::<OutboundMessage>(OUTBOUND_QUEUE_CAPACITY);

    let conn_id = state.room_manager.next_conn_id();
    info!("WebSocket connected: conn_id={conn_id}");

    let write_task = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if !msg.text.is_empty()
                && ws_sender
                    .send(Message::Text(msg.text.into()))
                    .await
                    .is_err()
            {
                break;
            }
        }
    });

    while let Some(msg_result) = ws_receiver.next().await {
        match msg_result {
            Ok(Message::Text(text)) => {
                if !handle_text_message(&text, conn_id, &state, &out_tx).await {
                    break;
                }
            }
            Ok(Message::Ping(_)) => {}
            Ok(Message::Close(_)) => {
                info!("WebSocket close from conn_id={conn_id}");
                break;
            }
            Err(e) => {
                error!("WebSocket error conn_id={conn_id}: {e}");
                break;
            }
            _ => {}
        }
    }

    state.room_manager.on_disconnect(conn_id);
    if let Some((user_id, device_id)) = state.device_manager.unregister(conn_id) {
        // Best-effort: mark the device offline in the DB and notify peers.
        if let Some(db) = state.db.as_ref() {
            let _ = crate::db::DeviceRow::set_online(db, &device_id, false).await;
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
) -> bool {
    debug!(
        "Received from conn_id={conn_id}: {}",
        truncate_preview(text, 200)
    );
    let msg: InboundMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            warn!("Invalid message from conn_id={conn_id}: {e}");
            return send_json_best_effort(
                out_tx,
                &OutboundProtocol::Error {
                    message: format!("invalid message format: {e}"),
                },
            );
        }
    };

    match msg {
        InboundMessage::CreateRoom {
            room_id,
            device_id,
            device_type: _,
            public_key,
        } => {
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
            debug!("RelayResponse from desktop conn_id={conn_id} corr={correlation_id}");
            state.room_manager.resolve_pending(
                &correlation_id,
                ResponsePayload {
                    encrypted_data,
                    nonce,
                },
            );
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
            // Mark the device online in the DB and the in-memory registry.
            let _ = crate::db::DeviceRow::upsert(
                db,
                &auth.device_id,
                &auth.user_id,
                &device_name,
                None,
            )
            .await;
            let _ = crate::db::DeviceRow::set_online(db, &auth.device_id, true).await;
            let _others = state.device_manager.register(
                &auth.user_id,
                &auth.device_id,
                &device_name,
                conn_id,
                out_tx.clone(),
            );
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
            if state
                .device_manager
                .resolve_rpc(&correlation_id, rpc_response)
            {
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
        Ok(json) => send_outbound_message(tx, OutboundMessage { text: json }).await,
        Err(e) => {
            warn!("Failed to serialize outbound websocket message: {e}");
            false
        }
    }
}

fn send_json_best_effort<T: Serialize>(tx: &mpsc::Sender<OutboundMessage>, msg: &T) -> bool {
    match serde_json::to_string(msg) {
        Ok(json) => match tx.try_send(OutboundMessage { text: json }) {
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
    use super::{send_json_best_effort, truncate_preview, OutboundProtocol};
    use tokio::sync::mpsc;

    #[test]
    fn truncate_preview_respects_utf8_boundaries() {
        let text = format!("{}{}", "a".repeat(199), "你");

        assert_eq!(truncate_preview(&text, 200), "a".repeat(199));
    }

    #[test]
    fn best_effort_control_response_does_not_block_on_full_queue() {
        let (tx, _rx) = mpsc::channel(1);
        assert!(send_json_best_effort(&tx, &OutboundProtocol::HeartbeatAck));

        assert!(
            send_json_best_effort(&tx, &OutboundProtocol::HeartbeatAck),
            "full queue should drop best-effort control response without closing read loop"
        );
    }
}
