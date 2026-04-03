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
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::relay::room::{ConnId, OutboundMessage, ResponsePayload, RoomManager};
use crate::routes::api::AppState;

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
}

pub async fn websocket_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.max_message_size(64 * 1024 * 1024)
        .max_frame_size(64 * 1024 * 1024)
        .max_write_buffer_size(64 * 1024 * 1024)
        .on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<OutboundMessage>();

    let conn_id = state.room_manager.next_conn_id();
    info!("WebSocket connected: conn_id={conn_id}");

    let write_task = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if !msg.text.is_empty()
                && ws_sender.send(Message::Text(msg.text)).await.is_err() {
                    break;
                }
        }
    });

    while let Some(msg_result) = ws_receiver.next().await {
        match msg_result {
            Ok(Message::Text(text)) => {
                handle_text_message(&text, conn_id, &state.room_manager, &out_tx);
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
    drop(out_tx);
    let _ = write_task.await;
    info!("WebSocket disconnected: conn_id={conn_id}");
}

fn handle_text_message(
    text: &str,
    conn_id: ConnId,
    room_manager: &Arc<RoomManager>,
    out_tx: &mpsc::UnboundedSender<OutboundMessage>,
) {
    debug!(
        "Received from conn_id={conn_id}: {}",
        &text[..text.len().min(200)]
    );
    let msg: InboundMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            warn!("Invalid message from conn_id={conn_id}: {e}");
            send_json(
                out_tx,
                &OutboundProtocol::Error {
                    message: format!("invalid message format: {e}"),
                },
            );
            return;
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
            let ok = room_manager.create_room(
                &room_id,
                conn_id,
                &device_id,
                &public_key,
                out_tx.clone(),
            );
            if ok {
                send_json(out_tx, &OutboundProtocol::RoomCreated { room_id });
            } else {
                send_json(
                    out_tx,
                    &OutboundProtocol::Error {
                        message: "failed to create room".into(),
                    },
                );
            }
        }

        InboundMessage::RelayResponse {
            correlation_id,
            encrypted_data,
            nonce,
        } => {
            debug!("RelayResponse from desktop conn_id={conn_id} corr={correlation_id}");
            room_manager.resolve_pending(
                &correlation_id,
                ResponsePayload {
                    encrypted_data,
                    nonce,
                },
            );
        }

        InboundMessage::Heartbeat => {
            if room_manager.heartbeat(conn_id) {
                send_json(out_tx, &OutboundProtocol::HeartbeatAck);
            } else {
                send_json(
                    out_tx,
                    &OutboundProtocol::Error {
                        message: "Room not found or expired".into(),
                    },
                );
            }
        }
    }
}

fn send_json<T: Serialize>(tx: &mpsc::UnboundedSender<OutboundMessage>, msg: &T) {
    if let Ok(json) = serde_json::to_string(msg) {
        let _ = tx.send(OutboundMessage { text: json });
    }
}

fn generate_room_id() -> String {
    let bytes: [u8; 6] = rand::random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
