//! Embedded mini relay server for LAN / ngrok modes.
//!
//! Runs inside the desktop process using axum + WebSocket.
//! Supports the same protocol as the standalone relay-server.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tokio::sync::mpsc;

type ConnId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageDirection {
    ToMobile,
    ToDesktop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferedMessage {
    pub seq: u64,
    pub timestamp: i64,
    pub direction: MessageDirection,
    pub encrypted_data: String,
    pub nonce: String,
}

struct Participant {
    conn_id: ConnId,
    device_id: String,
    device_type: String,
    public_key: String,
    tx: Option<mpsc::UnboundedSender<String>>,
    last_activity: i64,
}

struct Room {
    participants: Vec<Participant>,
    message_store: Vec<BufferedMessage>,
    next_seq: u64,
}

impl Room {
    fn buffer_message(&mut self, direction: MessageDirection, encrypted_data: String, nonce: String) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.message_store.push(BufferedMessage {
            seq,
            timestamp: chrono::Utc::now().timestamp(),
            direction,
            encrypted_data,
            nonce,
        });
        seq
    }
}

struct RelayState {
    rooms: DashMap<String, Room>,
    conn_to_room: DashMap<ConnId, String>,
    next_id: AtomicU64,
}

impl RelayState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            rooms: DashMap::new(),
            conn_to_room: DashMap::new(),
            next_id: AtomicU64::new(1),
        })
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Inbound {
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
        #[allow(dead_code)]
        room_id: String,
        encrypted_data: String,
        nonce: String,
    },
    Heartbeat,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Outbound {
    RoomCreated { room_id: String },
    PeerJoined { device_id: String, device_type: String, public_key: String },
    Relay { room_id: String, encrypted_data: String, nonce: String },
    PeerDisconnected { device_id: String },
    HeartbeatAck,
    Error { message: String },
}

/// Start the embedded relay and return a shutdown handle.
/// The server listens on `0.0.0.0:{port}`.
pub async fn start_embedded_relay(port: u16) -> anyhow::Result<EmbeddedRelayHandle> {
    let state = RelayState::new();
    let app_state = state.clone();

    let cleanup_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            let now = chrono::Utc::now().timestamp();
            let stale_ids: Vec<String> = cleanup_state.rooms
                .iter()
                .filter(|r| (now - r.participants.iter().map(|p| p.last_activity).max().unwrap_or(now)) > 300)
                .map(|r| r.key().clone())
                .collect();
                
            for id in stale_ids {
                if let Some((_, room)) = cleanup_state.rooms.remove(&id) {
                    for p in room.participants {
                        cleanup_state.conn_to_room.remove(&p.conn_id);
                    }
                }
            }
        }
    });

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/health", get(health))
        .route("/api/rooms/{room_id}/join", axum::routing::post(join_room_http))
        .route("/api/rooms/{room_id}/message", axum::routing::post(relay_message_http))
        .route("/api/rooms/{room_id}/poll", get(poll_messages_http))
        .route("/api/rooms/{room_id}/ack", axum::routing::post(ack_messages_http))
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .map_err(|e| anyhow::anyhow!("failed to bind embedded relay on port {port}: {e}"))?;

    info!("Embedded relay started on 0.0.0.0:{port}");

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async { let _ = shutdown_rx.await; })
            .await
            .ok();
    });

    // Brief wait for server to be ready
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    Ok(EmbeddedRelayHandle { _shutdown: Some(shutdown_tx) })
}

pub struct EmbeddedRelayHandle {
    _shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl EmbeddedRelayHandle {
    pub fn stop(&mut self) {
        if let Some(tx) = self._shutdown.take() {
            let _ = tx.send(());
            info!("Embedded relay stopped");
        }
    }
}

impl Drop for EmbeddedRelayHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "healthy"}))
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<RelayState>>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<RelayState>) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();

    let conn_id = state.next_id.fetch_add(1, Ordering::Relaxed);

    let write_task = tokio::spawn(async move {
        while let Some(text) = out_rx.recv().await {
            if ws_tx.send(Message::Text(text)).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = ws_rx.next().await {
        if let Message::Text(text) = msg {
            handle_msg(&text, conn_id, &state, &out_tx);
        }
    }

    on_disconnect(conn_id, &state);
    drop(out_tx);
    let _ = write_task.await;
    debug!("Embedded relay: conn {conn_id} closed");
}

fn handle_msg(
    text: &str,
    conn_id: ConnId,
    state: &Arc<RelayState>,
    out_tx: &mpsc::UnboundedSender<String>,
) {
    let msg: Inbound = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            send(&Some(out_tx.clone()), &Outbound::Error { message: format!("bad message: {e}") });
            return;
        }
    };

    match msg {
        Inbound::CreateRoom { room_id, device_id, device_type, public_key } => {
            let room_id = room_id.unwrap_or_else(gen_room_id);
            let mut room = Room { 
                participants: Vec::with_capacity(2),
                message_store: Vec::new(),
                next_seq: 1,
            };
            room.participants.push(Participant {
                conn_id, device_id, device_type, public_key, tx: Some(out_tx.clone()), last_activity: chrono::Utc::now().timestamp(),
            });
            state.rooms.insert(room_id.clone(), room);
            state.conn_to_room.insert(conn_id, room_id.clone());
            send(&Some(out_tx.clone()), &Outbound::RoomCreated { room_id });
        }

        Inbound::JoinRoom { room_id, device_id, device_type, public_key } => {
            let existing_peer = state.rooms.get(&room_id).and_then(|r| {
                r.participants.first().map(|p| (p.device_id.clone(), p.device_type.clone(), p.public_key.clone()))
            });

            let ok = if let Some(mut room) = state.rooms.get_mut(&room_id) {
                if room.participants.len() < 2 {
                    room.participants.push(Participant {
                        conn_id,
                        device_id: device_id.clone(),
                        device_type: device_type.clone(),
                        public_key: public_key.clone(),
                        tx: Some(out_tx.clone()),
                        last_activity: chrono::Utc::now().timestamp(),
                    });
                    state.conn_to_room.insert(conn_id, room_id.clone());
                    true
                } else {
                    false
                }
            } else {
                false
            };

            if ok {
                if let Some(room) = state.rooms.get(&room_id) {
                    for p in &room.participants {
                        if p.conn_id != conn_id {
                            send(&p.tx, &Outbound::PeerJoined {
                                device_id: device_id.clone(),
                                device_type: device_type.clone(),
                                public_key: public_key.clone(),
                            });
                        }
                    }
                }
                if let Some((pdid, pdt, ppk)) = existing_peer {
                    send(&Some(out_tx.clone()), &Outbound::PeerJoined {
                        device_id: pdid, device_type: pdt, public_key: ppk,
                    });
                }
            } else {
                send(&Some(out_tx.clone()), &Outbound::Error { message: format!("cannot join room {room_id}") });
            }
        }

        Inbound::Relay { room_id, encrypted_data, nonce } => {
            if let Some(rid) = state.conn_to_room.get(&conn_id) {
                if let Some(mut room) = state.rooms.get_mut(rid.value()) {
                    let sender_type = room.participants.iter()
                        .find(|p| p.conn_id == conn_id)
                        .map(|p| p.device_type.clone())
                        .unwrap_or_default();
                    
                    let direction = if sender_type == "desktop" {
                        MessageDirection::ToMobile
                    } else {
                        MessageDirection::ToDesktop
                    };
                    
                    room.buffer_message(direction, encrypted_data.clone(), nonce.clone());

                    let relay_json = serde_json::to_string(&Outbound::Relay {
                        room_id: room_id.clone(), encrypted_data, nonce,
                    }).unwrap_or_default();
                    for p in &room.participants {
                        if p.conn_id != conn_id {
                            if let Some(ref tx) = p.tx {
                                let _ = tx.send(relay_json.clone());
                            }
                        }
                    }
                }
            }
        }

        Inbound::Heartbeat => {
            if let Some(room_id) = state.conn_to_room.get(&conn_id) {
                if let Some(mut room) = state.rooms.get_mut(room_id.value()) {
                    if let Some(p) = room.participants.iter_mut().find(|p| p.conn_id == conn_id) {
                        p.last_activity = chrono::Utc::now().timestamp();
                    }
                }
                send(&Some(out_tx.clone()), &Outbound::HeartbeatAck);
            } else {
                send(&Some(out_tx.clone()), &Outbound::Error { message: "Room not found".into() });
            }
        }
    }
}

fn on_disconnect(conn_id: ConnId, state: &Arc<RelayState>) {
    if let Some((_, room_id)) = state.conn_to_room.remove(&conn_id) {
        let mut should_remove = false;
        if let Some(mut room) = state.rooms.get_mut(&room_id) {
            let removed = room.participants.iter().position(|p| p.conn_id == conn_id);
            if let Some(idx) = removed {
                let p = room.participants.remove(idx);
                let notif = serde_json::to_string(&Outbound::PeerDisconnected {
                    device_id: p.device_id,
                }).unwrap_or_default();
                for other in &room.participants {
                    if let Some(ref tx) = other.tx {
                        let _ = tx.send(notif.clone());
                    }
                }
            }
            should_remove = room.participants.is_empty();
        }
        if should_remove {
            state.rooms.remove(&room_id);
        }
    }
}

fn send(tx: &Option<mpsc::UnboundedSender<String>>, msg: &Outbound) {
    if let Some(tx) = tx {
        if let Ok(json) = serde_json::to_string(msg) {
            let _ = tx.send(json);
        }
    }
}

fn gen_room_id() -> String {
    let bytes: [u8; 6] = rand::random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ── HTTP Handlers ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct JoinRoomRequest {
    device_id: String,
    device_type: String,
    public_key: String,
}

async fn join_room_http(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(room_id): axum::extract::Path<String>,
    Json(body): Json<JoinRoomRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let conn_id = state.next_id.fetch_add(1, Ordering::Relaxed);
    
    let existing_peer = state.rooms.get(&room_id).and_then(|r| {
        r.participants.first().map(|p| (p.device_id.clone(), p.device_type.clone(), p.public_key.clone()))
    });

    let ok = if let Some(mut room) = state.rooms.get_mut(&room_id) {
        if room.participants.len() < 2 {
            room.participants.push(Participant {
                conn_id,
                device_id: body.device_id.clone(),
                device_type: body.device_type.clone(),
                public_key: body.public_key.clone(),
                tx: None, // HTTP client
                last_activity: chrono::Utc::now().timestamp(),
            });
            state.conn_to_room.insert(conn_id, room_id.clone());
            true
        } else {
            false
        }
    } else {
        false
    };

    if ok {
        if let Some(room) = state.rooms.get(&room_id) {
            for p in &room.participants {
                if p.conn_id != conn_id {
                    send(&p.tx, &Outbound::PeerJoined {
                        device_id: body.device_id.clone(),
                        device_type: body.device_type.clone(),
                        public_key: body.public_key.clone(),
                    });
                }
            }
        }
        
        if let Some((pdid, pdt, ppk)) = existing_peer {
            Ok(Json(serde_json::json!({
                "status": "joined",
                "peer": {
                    "device_id": pdid,
                    "device_type": pdt,
                    "public_key": ppk
                }
            })))
        } else {
            Ok(Json(serde_json::json!({
                "status": "joined",
                "peer": null
            })))
        }
    } else {
        Err(axum::http::StatusCode::BAD_REQUEST)
    }
}

#[derive(Deserialize)]
struct RelayMessageRequest {
    device_id: String,
    encrypted_data: String,
    nonce: String,
}

async fn relay_message_http(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(room_id): axum::extract::Path<String>,
    Json(body): Json<RelayMessageRequest>,
) -> axum::http::StatusCode {
    if let Some(mut room) = state.rooms.get_mut(&room_id) {
        let sender_conn_id = room.participants.iter()
            .find(|p| p.device_id == body.device_id)
            .map(|p| p.conn_id);
            
        if let Some(conn_id) = sender_conn_id {
            let sender_type = room.participants.iter()
                .find(|p| p.conn_id == conn_id)
                .map(|p| p.device_type.clone())
                .unwrap_or_default();
            
            let direction = if sender_type == "desktop" {
                MessageDirection::ToMobile
            } else {
                MessageDirection::ToDesktop
            };
            
            room.buffer_message(direction, body.encrypted_data.clone(), body.nonce.clone());

            let relay_json = serde_json::to_string(&Outbound::Relay {
                room_id: room_id.clone(), 
                encrypted_data: body.encrypted_data, 
                nonce: body.nonce,
            }).unwrap_or_default();
            
            for p in &room.participants {
                if p.conn_id != conn_id {
                    if let Some(ref tx) = p.tx {
                        let _ = tx.send(relay_json.clone());
                    }
                }
            }
            return axum::http::StatusCode::OK;
        }
    }
    axum::http::StatusCode::NOT_FOUND
}

#[derive(Deserialize)]
struct PollQuery {
    since_seq: Option<u64>,
    device_type: Option<String>,
}

#[derive(Serialize)]
struct PollResponse {
    messages: Vec<BufferedMessage>,
    peer_connected: bool,
}

async fn poll_messages_http(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(room_id): axum::extract::Path<String>,
    axum::extract::Query(query): axum::extract::Query<PollQuery>,
) -> Result<Json<PollResponse>, axum::http::StatusCode> {
    let since = query.since_seq.unwrap_or(0);
    let direction_str = query.device_type.as_deref().unwrap_or("mobile");
    let direction = match direction_str {
        "desktop" => MessageDirection::ToDesktop,
        _ => MessageDirection::ToMobile,
    };
    
    if let Some(mut room) = state.rooms.get_mut(&room_id) {
        if let Some(p) = room.participants.iter_mut().find(|p| p.device_type == direction_str) {
            p.last_activity = chrono::Utc::now().timestamp();
        }
        let peer_connected = room.participants.iter().any(|p| p.device_type != direction_str);
        let messages = room.message_store
            .iter()
            .filter(|m| m.direction == direction && m.seq > since)
            .cloned()
            .collect();
        Ok(Json(PollResponse { messages, peer_connected }))
    } else {
        Ok(Json(PollResponse { messages: vec![], peer_connected: false }))
    }
}

#[derive(Deserialize)]
struct AckRequest {
    ack_seq: u64,
    device_type: Option<String>,
}

async fn ack_messages_http(
    State(state): State<Arc<RelayState>>,
    axum::extract::Path(room_id): axum::extract::Path<String>,
    Json(body): Json<AckRequest>,
) -> axum::http::StatusCode {
    let direction_str = body.device_type.as_deref().unwrap_or("mobile");
    let direction = match direction_str {
        "desktop" => MessageDirection::ToDesktop,
        _ => MessageDirection::ToMobile,
    };
    
    if let Some(mut room) = state.rooms.get_mut(&room_id) {
        if let Some(p) = room.participants.iter_mut().find(|p| p.device_type == direction_str) {
            p.last_activity = chrono::Utc::now().timestamp();
        }
        room.message_store.retain(|m| !(m.direction == direction && m.seq <= body.ack_seq));
    }
    axum::http::StatusCode::OK
}
