//! WebSocket handler for the relay server.
//!
//! Each connected client sends/receives JSON messages following the relay protocol.
//! The server never decrypts application data — it only handles room management
//! and forwards encrypted payloads between paired devices.
//! Desktop→mobile messages are also buffered for later polling.

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

use crate::relay::room::{ConnId, OutboundMessage, RoomManager};
use crate::routes::api::AppState;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
pub enum InboundMessage {
    CreateRoom {
        room_id: Option<String>,
        device_id: String,
        device_type: String,
        public_key: String,
    },
    JoinRoom {
        room_id: String,
        device_id: String,
        device_type: String,
        public_key: String,
    },
    Relay {
        room_id: String,
        encrypted_data: String,
        nonce: String,
    },
    Heartbeat,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
pub enum OutboundProtocol {
    RoomCreated { room_id: String },
    PeerJoined { device_id: String, device_type: String, public_key: String },
    Relay { room_id: String, encrypted_data: String, nonce: String },
    HeartbeatAck,
    PeerDisconnected { device_id: String },
    Error { message: String },
}

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<OutboundMessage>();

    let conn_id = state.room_manager.next_conn_id();
    info!("WebSocket connected: conn_id={conn_id}");

    let write_task = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if !msg.text.is_empty() {
                if ws_sender.send(Message::Text(msg.text)).await.is_err() {
                    break;
                }
            }
        }
    });

    while let Some(msg_result) = ws_receiver.next().await {
        match msg_result {
            Ok(Message::Text(text)) => {
                handle_text_message(&text, conn_id, &state.room_manager, &out_tx);
            }
            Ok(Message::Ping(_)) => {
                // Axum auto-replies Pong for Ping frames
            }
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
    debug!("Received from conn_id={conn_id}: {}", &text[..text.len().min(200)]);
    let msg: InboundMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            warn!("Invalid message from conn_id={conn_id}: {e}");
            send_json(out_tx, &OutboundProtocol::Error {
                message: format!("invalid message format: {e}"),
            });
            return;
        }
    };

    match msg {
        InboundMessage::CreateRoom {
            room_id,
            device_id,
            device_type,
            public_key,
        } => {
            let room_id = room_id.unwrap_or_else(generate_room_id);
            let ok = room_manager.create_room(
                &room_id, conn_id, &device_id, &device_type, &public_key, Some(out_tx.clone()),
            );
            if ok {
                send_json(out_tx, &OutboundProtocol::RoomCreated { room_id });
            } else {
                send_json(out_tx, &OutboundProtocol::Error {
                    message: "failed to create room".into(),
                });
            }
        }

        InboundMessage::JoinRoom {
            room_id,
            device_id,
            device_type,
            public_key,
        } => {
            let existing_peer = room_manager.get_peer_info(&room_id, conn_id);

            let ok = room_manager.join_room(
                &room_id, conn_id, &device_id, &device_type, &public_key, Some(out_tx.clone()),
            );

            if ok {
                let joiner_notification = serde_json::to_string(&OutboundProtocol::PeerJoined {
                    device_id: device_id.clone(),
                    device_type: device_type.clone(),
                    public_key: public_key.clone(),
                }).unwrap_or_default();
                room_manager.send_to_others_in_room(&room_id, conn_id, &joiner_notification);

                if let Some((peer_did, peer_dt, peer_pk)) = existing_peer {
                    send_json(out_tx, &OutboundProtocol::PeerJoined {
                        device_id: peer_did,
                        device_type: peer_dt,
                        public_key: peer_pk,
                    });
                } else {
                    warn!("No existing peer found for room {room_id} to send back to joiner");
                }
            } else {
                send_json(out_tx, &OutboundProtocol::Error {
                    message: format!("failed to join room {room_id}"),
                });
            }
        }

        InboundMessage::Relay {
            room_id: _,
            encrypted_data,
            nonce,
        } => {
            debug!("Relay message from conn_id={conn_id} data_len={}", encrypted_data.len());
            if room_manager.relay_message(conn_id, &encrypted_data, &nonce) {
                debug!("Relay message forwarded from conn_id={conn_id}");
            } else {
                warn!("Relay failed for conn_id={conn_id}: no peer found");
            }
        }

        InboundMessage::Heartbeat => {
            if room_manager.heartbeat(conn_id) {
                send_json(out_tx, &OutboundProtocol::HeartbeatAck);
            } else {
                send_json(out_tx, &OutboundProtocol::Error {
                    message: "Room not found or expired".into(),
                });
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
