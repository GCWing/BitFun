//! Room management for the relay server.
//!
//! Each room holds a single desktop participant connected via WebSocket.
//! Mobile clients interact through HTTP requests that the relay bridges
//! to the desktop via the WebSocket connection. The relay stores no
//! business data — it only routes messages.

use chrono::Utc;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, OwnedSemaphorePermit, Semaphore};
use tracing::{debug, info, warn};

pub type ConnId = u64;
pub const MAX_PENDING_REQUESTS: usize = 1024;

struct PendingRequest {
    tx: oneshot::Sender<ResponsePayload>,
    _permit: OwnedSemaphorePermit,
}

pub struct PendingRequestGuard {
    room_manager: Arc<RoomManager>,
    correlation_id: String,
}

impl Drop for PendingRequestGuard {
    fn drop(&mut self) {
        self.room_manager.cancel_pending(&self.correlation_id);
    }
}

#[derive(Debug, Clone)]
pub struct OutboundMessage {
    pub text: String,
}

/// Payload returned by the desktop in response to a bridged HTTP request.
#[derive(Debug, Clone)]
pub struct ResponsePayload {
    pub encrypted_data: String,
    pub nonce: String,
}

#[derive(Debug)]
pub struct DesktopConnection {
    pub conn_id: ConnId,
    #[allow(dead_code)]
    pub device_id: String,
    #[allow(dead_code)]
    pub public_key: String,
    pub tx: mpsc::Sender<OutboundMessage>,
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
    pub desktop: Option<DesktopConnection>,
}

impl RelayRoom {
    pub fn new(room_id: String) -> Self {
        let now = Utc::now().timestamp();
        Self {
            room_id,
            created_at: now,
            last_activity: now,
            desktop: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.desktop.is_none()
    }

    pub fn touch(&mut self) {
        self.last_activity = Utc::now().timestamp();
    }
}

pub async fn send_outbound_message(
    tx: &mpsc::Sender<OutboundMessage>,
    message: OutboundMessage,
) -> bool {
    match tx.send(message).await {
        Ok(()) => true,
        Err(_) => {
            debug!("Outbound websocket channel closed before message could be sent");
            false
        }
    }
}

pub struct RoomManager {
    rooms: DashMap<String, RelayRoom>,
    conn_to_room: DashMap<ConnId, String>,
    next_conn_id: std::sync::atomic::AtomicU64,
    pending_requests: DashMap<String, PendingRequest>,
    pending_permits: Arc<Semaphore>,
}

impl RoomManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            rooms: DashMap::new(),
            conn_to_room: DashMap::new(),
            next_conn_id: std::sync::atomic::AtomicU64::new(1),
            pending_requests: DashMap::new(),
            pending_permits: Arc::new(Semaphore::new(MAX_PENDING_REQUESTS)),
        })
    }

    pub fn next_conn_id(&self) -> ConnId {
        self.next_conn_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub fn create_room(
        &self,
        room_id: &str,
        conn_id: ConnId,
        device_id: &str,
        public_key: &str,
        tx: mpsc::Sender<OutboundMessage>,
    ) -> bool {
        if let Some((_, old_room_id)) = self.conn_to_room.remove(&conn_id) {
            let should_remove = if let Some(mut room) = self.rooms.get_mut(&old_room_id) {
                room.desktop = None;
                room.is_empty()
            } else {
                false
            };
            if should_remove {
                self.rooms.remove(&old_room_id);
            }
        }

        self.rooms.remove(room_id);

        let now = Utc::now().timestamp();
        let mut room = RelayRoom::new(room_id.to_string());
        room.desktop = Some(DesktopConnection {
            conn_id,
            device_id: device_id.to_string(),
            public_key: public_key.to_string(),
            tx,
            joined_at: now,
            last_heartbeat: now,
        });

        self.rooms.insert(room_id.to_string(), room);
        self.conn_to_room.insert(conn_id, room_id.to_string());

        info!("Room {room_id} created by desktop {device_id}");
        true
    }

    pub async fn send_to_desktop(&self, room_id: &str, message: &str) -> bool {
        let tx = if let Some(mut room) = self.rooms.get_mut(room_id) {
            room.touch();
            room.desktop.as_ref().map(|desktop| desktop.tx.clone())
        } else {
            None
        };

        if let Some(tx) = tx {
            send_outbound_message(
                &tx,
                OutboundMessage {
                    text: message.to_string(),
                },
            )
            .await
        } else {
            false
        }
    }

    #[allow(dead_code)]
    pub fn get_desktop_public_key(&self, room_id: &str) -> Option<String> {
        self.rooms
            .get(room_id)
            .and_then(|r| r.desktop.as_ref().map(|d| d.public_key.clone()))
    }

    pub fn try_register_pending(
        self: &Arc<Self>,
        correlation_id: String,
    ) -> Option<(PendingRequestGuard, oneshot::Receiver<ResponsePayload>)> {
        let permit = Arc::clone(&self.pending_permits).try_acquire_owned().ok()?;
        let (tx, rx) = oneshot::channel();
        let guard = PendingRequestGuard {
            room_manager: Arc::clone(self),
            correlation_id: correlation_id.clone(),
        };
        self.pending_requests.insert(
            correlation_id,
            PendingRequest {
                tx,
                _permit: permit,
            },
        );
        Some((guard, rx))
    }

    pub fn resolve_pending(&self, correlation_id: &str, payload: ResponsePayload) -> bool {
        if let Some((_, pending)) = self.pending_requests.remove(correlation_id) {
            pending.tx.send(payload).is_ok()
        } else {
            warn!("No pending request for correlation_id={correlation_id}");
            false
        }
    }

    pub fn cancel_pending(&self, correlation_id: &str) {
        self.pending_requests.remove(correlation_id);
    }

    pub fn on_disconnect(&self, conn_id: ConnId) {
        if let Some((_, room_id)) = self.conn_to_room.remove(&conn_id) {
            let should_remove = if let Some(mut room) = self.rooms.get_mut(&room_id) {
                if room.desktop.as_ref().is_some_and(|d| d.conn_id == conn_id) {
                    info!("Desktop disconnected from room {room_id}");
                    room.desktop = None;
                }
                room.is_empty()
            } else {
                false
            };
            if should_remove {
                self.rooms.remove(&room_id);
                debug!("Empty room {room_id} removed");
            }
        }
    }

    pub fn heartbeat(&self, conn_id: ConnId) -> bool {
        if let Some(room_id) = self.conn_to_room.get(&conn_id) {
            if let Some(mut room) = self.rooms.get_mut(room_id.value()) {
                let is_match = room.desktop.as_ref().is_some_and(|d| d.conn_id == conn_id);
                if is_match {
                    let now = Utc::now().timestamp();
                    room.last_activity = now;
                    if let Some(ref mut desktop) = room.desktop {
                        desktop.last_heartbeat = now;
                    }
                    return true;
                }
            }
        }
        false
    }

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
                if let Some(ref desktop) = room.desktop {
                    self.conn_to_room.remove(&desktop.conn_id);
                }
                info!("Stale room {room_id} cleaned up");
            }
        }

        stale_ids
    }

    pub fn room_exists(&self, room_id: &str) -> bool {
        self.rooms.contains_key(room_id)
    }

    pub fn has_desktop(&self, room_id: &str) -> bool {
        self.rooms.get(room_id).is_some_and(|r| r.desktop.is_some())
    }

    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }

    pub fn connection_count(&self) -> usize {
        self.conn_to_room.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn outbound_send_waits_for_bounded_queue_capacity() {
        let (tx, mut rx) = mpsc::channel(1);

        assert!(
            send_outbound_message(
                &tx,
                OutboundMessage {
                    text: "first".to_string(),
                },
            )
            .await
        );

        let blocked_send = tokio::spawn({
            let tx = tx.clone();
            async move {
                send_outbound_message(
                    &tx,
                    OutboundMessage {
                        text: "second".to_string(),
                    },
                )
                .await
            }
        });

        tokio::task::yield_now().await;
        assert!(
            !blocked_send.is_finished(),
            "bounded outbound send should apply backpressure instead of dropping"
        );

        assert_eq!(rx.recv().await.expect("first message").text, "first");
        assert!(timeout(Duration::from_secs(1), blocked_send)
            .await
            .expect("send should complete after capacity is released")
            .expect("send task should not panic"));
        assert_eq!(rx.recv().await.expect("second message").text, "second");
    }

    #[test]
    fn pending_registration_is_bounded() {
        let manager = RoomManager::new();
        let mut guards = Vec::new();

        for index in 0..MAX_PENDING_REQUESTS {
            let (guard, _rx) = manager
                .try_register_pending(format!("pending-{index}"))
                .expect("pending registration within limit");
            guards.push(guard);
        }

        assert!(manager
            .try_register_pending("overflow".to_string())
            .is_none());
        drop(guards.pop());
        assert!(manager
            .try_register_pending("after-cancel".to_string())
            .is_some());
    }
}
