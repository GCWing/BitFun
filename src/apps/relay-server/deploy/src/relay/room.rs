//! Room management for the relay server.
//!
//! Each room holds at most 2 participants (desktop + mobile).
//! Messages are relayed without decryption (E2E encrypted between clients).
//! Desktop→mobile messages are buffered so that the mobile client can poll
//! for missed messages via the HTTP API.

use chrono::Utc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

pub type ConnId = u64;

#[derive(Debug, Clone)]
pub struct OutboundMessage {
    pub text: String,
}

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

#[derive(Debug)]
pub struct Participant {
    pub conn_id: ConnId,
    pub device_id: String,
    pub device_type: String,
    pub public_key: String,
    pub tx: Option<mpsc::UnboundedSender<OutboundMessage>>,
    #[allow(dead_code)]
    pub joined_at: i64,
    pub last_heartbeat: i64,
}

#[derive(Debug)]
pub struct RelayRoom {
    pub room_id: String,
    #[allow(dead_code)]
    pub created_at: i64,
    pub last_activity: i64,
    pub participants: Vec<Participant>,
    pub message_store: Vec<BufferedMessage>,
    pub next_seq: u64,
}

impl RelayRoom {
    pub fn new(room_id: String) -> Self {
        let now = Utc::now().timestamp();
        Self {
            room_id,
            created_at: now,
            last_activity: now,
            participants: Vec::with_capacity(2),
            message_store: Vec::new(),
            next_seq: 1,
        }
    }

    pub fn add_participant(&mut self, participant: Participant) -> bool {
        if self.participants.len() >= 2 {
            return false;
        }
        self.participants.push(participant);
        self.touch();
        true
    }

    pub fn remove_participant(&mut self, conn_id: ConnId) -> Option<Participant> {
        if let Some(idx) = self.participants.iter().position(|p| p.conn_id == conn_id) {
            Some(self.participants.remove(idx))
        } else {
            None
        }
    }

    pub fn relay_to_peer(&self, sender_conn_id: ConnId, message: &str) -> bool {
        for p in &self.participants {
            if p.conn_id != sender_conn_id {
                if let Some(ref tx) = p.tx {
                    let _ = tx.send(OutboundMessage {
                        text: message.to_string(),
                    });
                }
                return true;
            }
        }
        false
    }

    #[allow(dead_code)]
    pub fn send_to(&self, conn_id: ConnId, message: &str) {
        for p in &self.participants {
            if p.conn_id == conn_id {
                if let Some(ref tx) = p.tx {
                    let _ = tx.send(OutboundMessage {
                        text: message.to_string(),
                    });
                }
                return;
            }
        }
    }

    pub fn broadcast(&self, message: &str) {
        for p in &self.participants {
            if let Some(ref tx) = p.tx {
                let _ = tx.send(OutboundMessage {
                    text: message.to_string(),
                });
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.participants.is_empty()
    }

    #[allow(dead_code)]
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    pub fn update_heartbeat(&mut self, conn_id: ConnId) {
        let now = Utc::now().timestamp();
        for p in &mut self.participants {
            if p.conn_id == conn_id {
                p.last_heartbeat = now;
                break;
            }
        }
        // Do not update room's last_activity here, so that if the other peer is inactive,
        // we can still detect it.
    }

    fn touch(&mut self) {
        self.last_activity = Utc::now().timestamp();
    }

    /// Buffer an encrypted message for later polling by the target device.
    pub fn buffer_message(
        &mut self,
        direction: MessageDirection,
        encrypted_data: String,
        nonce: String,
    ) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.message_store.push(BufferedMessage {
            seq,
            timestamp: Utc::now().timestamp(),
            direction,
            encrypted_data,
            nonce,
        });
        self.touch();
        seq
    }

    /// Return buffered messages for a given direction with seq > since_seq.
    pub fn poll_messages(
        &self,
        direction: MessageDirection,
        since_seq: u64,
    ) -> Vec<BufferedMessage> {
        self.message_store
            .iter()
            .filter(|m| m.direction == direction && m.seq > since_seq)
            .cloned()
            .collect()
    }

    /// Remove buffered messages with seq <= ack_seq for a given direction.
    pub fn ack_messages(&mut self, direction: MessageDirection, ack_seq: u64) {
        self.message_store
            .retain(|m| !(m.direction == direction && m.seq <= ack_seq));
    }

    /// Get the device_type of the sender identified by conn_id.
    pub fn sender_device_type(&self, conn_id: ConnId) -> Option<&str> {
        self.participants
            .iter()
            .find(|p| p.conn_id == conn_id)
            .map(|p| p.device_type.as_str())
    }
}

pub struct RoomManager {
    rooms: DashMap<String, RelayRoom>,
    conn_to_room: DashMap<ConnId, String>,
    next_conn_id: std::sync::atomic::AtomicU64,
}

impl RoomManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            rooms: DashMap::new(),
            conn_to_room: DashMap::new(),
            next_conn_id: std::sync::atomic::AtomicU64::new(1),
        })
    }

    pub fn next_conn_id(&self) -> ConnId {
        self.next_conn_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    /// If conn_id is already in a room, remove it from that room first.
    fn leave_current_room(&self, conn_id: ConnId) {
        if let Some((_, old_room_id)) = self.conn_to_room.remove(&conn_id) {
            let mut should_remove = false;
            if let Some(mut room) = self.rooms.get_mut(&old_room_id) {
                room.remove_participant(conn_id);
                should_remove = room.is_empty();
            }
            if should_remove {
                self.rooms.remove(&old_room_id);
                debug!("Cleaned up old room {old_room_id} after conn moved");
            }
        }
    }

    pub fn create_room(
        &self,
        room_id: &str,
        conn_id: ConnId,
        device_id: &str,
        device_type: &str,
        public_key: &str,
        tx: Option<mpsc::UnboundedSender<OutboundMessage>>,
    ) -> bool {
        if self.rooms.contains_key(room_id) {
            warn!("Room {room_id} already exists");
            return false;
        }

        self.leave_current_room(conn_id);

        let now = Utc::now().timestamp();
        let mut room = RelayRoom::new(room_id.to_string());
        room.add_participant(Participant {
            conn_id,
            device_id: device_id.to_string(),
            device_type: device_type.to_string(),
            public_key: public_key.to_string(),
            tx,
            joined_at: now,
            last_heartbeat: now,
        });

        self.rooms.insert(room_id.to_string(), room);
        self.conn_to_room.insert(conn_id, room_id.to_string());

        info!("Room {room_id} created by {device_id} ({device_type})");
        true
    }

    pub fn join_room(
        &self,
        room_id: &str,
        conn_id: ConnId,
        device_id: &str,
        device_type: &str,
        public_key: &str,
        tx: Option<mpsc::UnboundedSender<OutboundMessage>>,
    ) -> bool {
        self.leave_current_room(conn_id);

        let mut room_ref = match self.rooms.get_mut(room_id) {
            Some(r) => r,
            None => {
                warn!("Room {room_id} not found");
                return false;
            }
        };

        let now = Utc::now().timestamp();
        let ok = room_ref.add_participant(Participant {
            conn_id,
            device_id: device_id.to_string(),
            device_type: device_type.to_string(),
            public_key: public_key.to_string(),
            tx,
            joined_at: now,
            last_heartbeat: now,
        });

        if ok {
            drop(room_ref);
            self.conn_to_room.insert(conn_id, room_id.to_string());
            info!("Device {device_id} ({device_type}) joined room {room_id}");
        } else {
            warn!("Room {room_id} is full");
        }

        ok
    }

    /// Relay a message to the peer. If the sender is desktop, also buffer for mobile polling.
    pub fn relay_message(&self, conn_id: ConnId, encrypted_data: &str, nonce: &str) -> bool {
        if let Some(room_id) = self.conn_to_room.get(&conn_id) {
            if let Some(mut room) = self.rooms.get_mut(room_id.value()) {
                let sender_type = room
                    .sender_device_type(conn_id)
                    .unwrap_or("unknown")
                    .to_string();

                let direction = if sender_type == "desktop" {
                    MessageDirection::ToMobile
                } else {
                    MessageDirection::ToDesktop
                };
                room.buffer_message(
                    direction,
                    encrypted_data.to_string(),
                    nonce.to_string(),
                );

                let relay_json = serde_json::json!({
                    "type": "relay",
                    "room_id": room_id.value(),
                    "encrypted_data": encrypted_data,
                    "nonce": nonce,
                })
                .to_string();

                return room.relay_to_peer(conn_id, &relay_json);
            }
        }
        false
    }

    pub fn on_disconnect(&self, conn_id: ConnId) {
        if let Some((_, room_id)) = self.conn_to_room.remove(&conn_id) {
            let mut should_remove = false;

            if let Some(mut room) = self.rooms.get_mut(&room_id) {
                if let Some(removed) = room.remove_participant(conn_id) {
                    info!(
                        "Device {} disconnected from room {}",
                        removed.device_id, room_id
                    );

                    let notification = serde_json::json!({
                        "type": "peer_disconnected",
                        "device_id": removed.device_id,
                    })
                    .to_string();
                    room.broadcast(&notification);
                }
                should_remove = room.is_empty();
            }

            if should_remove {
                self.rooms.remove(&room_id);
                debug!("Empty room {room_id} removed");
            }
        }
    }

    pub fn heartbeat(&self, conn_id: ConnId) -> bool {
        if let Some(room_id) = self.conn_to_room.get(&conn_id) {
            if let Some(mut room) = self.rooms.get_mut(room_id.value()) {
                room.update_heartbeat(conn_id);
                return true;
            }
        }
        false
    }

    /// Returns (device_id, device_type, public_key) of the peer.
    pub fn get_peer_info(
        &self,
        room_id: &str,
        conn_id: ConnId,
    ) -> Option<(String, String, String)> {
        if let Some(room) = self.rooms.get(room_id) {
            for p in &room.participants {
                if p.conn_id != conn_id {
                    return Some((
                        p.device_id.clone(),
                        p.device_type.clone(),
                        p.public_key.clone(),
                    ));
                }
            }
        }
        None
    }

    /// Find conn_id by device_id in a specific room
    pub fn get_conn_id_by_device(&self, room_id: &str, device_id: &str) -> Option<ConnId> {
        if let Some(room) = self.rooms.get(room_id) {
            for p in &room.participants {
                if p.device_id == device_id {
                    return Some(p.conn_id);
                }
            }
        }
        None
    }

    /// Check if the room has a peer of the opposite device type
    pub fn has_peer(&self, room_id: &str, my_device_type: &str) -> bool {
        if let Some(room) = self.rooms.get(room_id) {
            room.participants.iter().any(|p| p.device_type != my_device_type)
        } else {
            false
        }
    }

    /// Clean up stale rooms based on last_activity rather than created_at.
    /// Returns the list of room IDs that were removed.
    pub fn cleanup_stale_rooms(&self, ttl_secs: u64) -> Vec<String> {
        let now = Utc::now().timestamp();
        let stale_ids: Vec<String> = self
            .rooms
            .iter()
            .filter(|r| (now - r.last_activity) as u64 > ttl_secs)
            .map(|r| r.room_id.clone())
            .collect();

        for room_id in &stale_ids {
            if let Some((_, room)) = self.rooms.remove(room_id) {
                for p in &room.participants {
                    self.conn_to_room.remove(&p.conn_id);
                }
                info!("Stale room {room_id} cleaned up");
            }
        }

        stale_ids
    }

    pub fn send_to_others_in_room(&self, room_id: &str, exclude_conn_id: ConnId, message: &str) {
        if let Some(room) = self.rooms.get(room_id) {
            for p in &room.participants {
                if p.conn_id != exclude_conn_id {
                    if let Some(ref tx) = p.tx {
                        let _ = tx.send(OutboundMessage {
                            text: message.to_string(),
                        });
                    }
                }
            }
        }
    }

    /// Poll buffered messages for a specific room and direction.
    pub fn poll_messages(
        &self,
        room_id: &str,
        direction: MessageDirection,
        since_seq: u64,
    ) -> Vec<BufferedMessage> {
        if let Some(mut room) = self.rooms.get_mut(room_id) {
            room.last_activity = Utc::now().timestamp();
            room.poll_messages(direction, since_seq)
        } else {
            Vec::new()
        }
    }

    /// Acknowledge receipt of messages up to ack_seq.
    pub fn ack_messages(&self, room_id: &str, direction: MessageDirection, ack_seq: u64) {
        if let Some(mut room) = self.rooms.get_mut(room_id) {
            room.last_activity = Utc::now().timestamp();
            room.ack_messages(direction, ack_seq);
        }
    }

    pub fn room_exists(&self, room_id: &str) -> bool {
        self.rooms.contains_key(room_id)
    }

    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }

    pub fn connection_count(&self) -> usize {
        self.conn_to_room.len()
    }
}
