//! REST API routes for the relay server.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::relay::room::{BufferedMessage, MessageDirection};
use crate::relay::RoomManager;

#[derive(Clone)]
pub struct AppState {
    pub room_manager: Arc<RoomManager>,
    pub start_time: std::time::Instant,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub rooms: usize,
    pub connections: usize,
}

pub async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
        rooms: state.room_manager.room_count(),
        connections: state.room_manager.connection_count(),
    })
}

#[derive(Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    pub protocol_version: u8,
}

pub async fn server_info() -> Json<ServerInfo> {
    Json(ServerInfo {
        name: "BitFun Relay Server".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        protocol_version: 1,
    })
}

#[derive(Deserialize)]
pub struct JoinRoomRequest {
    pub device_id: String,
    pub device_type: String,
    pub public_key: String,
}

/// `POST /api/rooms/:room_id/join`
pub async fn join_room(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(body): Json<JoinRoomRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let conn_id = state.room_manager.next_conn_id();
    let existing_peer = state.room_manager.get_peer_info(&room_id, conn_id);

    let ok = state.room_manager.join_room(
        &room_id,
        conn_id,
        &body.device_id,
        &body.device_type,
        &body.public_key,
        None, // HTTP client, no websocket tx
    );

    if ok {
        let joiner_notification = serde_json::to_string(&crate::routes::websocket::OutboundProtocol::PeerJoined {
            device_id: body.device_id.clone(),
            device_type: body.device_type.clone(),
            public_key: body.public_key.clone(),
        }).unwrap_or_default();
        state.room_manager.send_to_others_in_room(&room_id, conn_id, &joiner_notification);

        if let Some((peer_did, peer_dt, peer_pk)) = existing_peer {
            Ok(Json(serde_json::json!({
                "status": "joined",
                "peer": {
                    "device_id": peer_did,
                    "device_type": peer_dt,
                    "public_key": peer_pk
                }
            })))
        } else {
            Ok(Json(serde_json::json!({
                "status": "joined",
                "peer": null
            })))
        }
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

#[derive(Deserialize)]
pub struct RelayMessageRequest {
    pub device_id: String,
    pub encrypted_data: String,
    pub nonce: String,
}

/// `POST /api/rooms/:room_id/message`
pub async fn relay_message(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(body): Json<RelayMessageRequest>,
) -> StatusCode {
    // Find conn_id by device_id in the room
    if let Some(conn_id) = state.room_manager.get_conn_id_by_device(&room_id, &body.device_id) {
        if state.room_manager.relay_message(conn_id, &body.encrypted_data, &body.nonce) {
            StatusCode::OK
        } else {
            StatusCode::NOT_FOUND
        }
    } else {
        StatusCode::UNAUTHORIZED
    }
}

#[derive(Deserialize)]
pub struct PollQuery {
    pub since_seq: Option<u64>,
    pub device_type: Option<String>,
}

#[derive(Serialize)]
pub struct PollResponse {
    pub messages: Vec<BufferedMessage>,
    pub peer_connected: bool,
}

/// `GET /api/rooms/:room_id/poll?since_seq=0&device_type=mobile`
pub async fn poll_messages(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Query(query): Query<PollQuery>,
) -> Result<Json<PollResponse>, StatusCode> {
    let since = query.since_seq.unwrap_or(0);
    let direction = match query.device_type.as_deref() {
        Some("desktop") => MessageDirection::ToDesktop,
        _ => MessageDirection::ToMobile,
    };
    
    let peer_connected = state.room_manager.has_peer(&room_id, query.device_type.as_deref().unwrap_or("mobile"));
    let messages = state.room_manager.poll_messages(&room_id, direction, since);
    
    Ok(Json(PollResponse { messages, peer_connected }))
}

#[derive(Deserialize)]
pub struct AckRequest {
    pub ack_seq: u64,
    pub device_type: Option<String>,
}

/// `POST /api/rooms/:room_id/ack`
pub async fn ack_messages(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(body): Json<AckRequest>,
) -> StatusCode {
    let direction = match body.device_type.as_deref() {
        Some("desktop") => MessageDirection::ToDesktop,
        _ => MessageDirection::ToMobile,
    };
    state
        .room_manager
        .ack_messages(&room_id, direction, body.ack_seq);
    StatusCode::OK
}
