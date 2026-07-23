//! Room management for the relay server.
//!
//! Each room holds a single desktop participant connected via WebSocket.
//! Mobile clients interact through HTTP requests that the relay bridges
//! to the desktop via the WebSocket connection. The relay stores no
//! business data — it only routes messages.

use chrono::Utc;
use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, OwnedSemaphorePermit, Semaphore};
use tracing::{debug, info, warn};

pub type ConnId = u64;
pub const MAX_PENDING_REQUESTS: usize = 1024;
pub const MAX_PENDING_REQUESTS_PER_ROOM: usize = 128;
pub const MAX_ACTIVE_ROOMS: usize = 10_000;

/// Room IDs cross an untrusted WebSocket boundary and later become asset
/// namespace names. Keep them to one portable path segment so they can never
/// influence filesystem traversal in a disk-backed relay host.
pub fn is_valid_room_id(room_id: &str) -> bool {
    !room_id.is_empty()
        && room_id.len() <= 128
        && !matches!(room_id, "_store" | "page-data" | "pages")
        && room_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

struct PendingRequest {
    tx: oneshot::Sender<ResponsePayload>,
    room_id: String,
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

impl OutboundMessage {
    pub fn text(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
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
    pending_room_counts: DashMap<String, usize>,
}

impl RoomManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            rooms: DashMap::new(),
            conn_to_room: DashMap::new(),
            next_conn_id: std::sync::atomic::AtomicU64::new(1),
            pending_requests: DashMap::new(),
            pending_permits: Arc::new(Semaphore::new(MAX_PENDING_REQUESTS)),
            pending_room_counts: DashMap::new(),
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
        if !is_valid_room_id(room_id) {
            warn!("Rejected invalid room id");
            return false;
        }
        if self.rooms.len() >= MAX_ACTIVE_ROOMS && !self.rooms.contains_key(room_id) {
            warn!("Rejected room creation because the active-room limit was reached");
            return false;
        }
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

        // Room ids are bearer secrets embedded in pairing QR codes. A second
        // socket must never be able to evict the desktop that currently owns
        // one by guessing or observing the id. Re-sending create_room from the
        // same socket is harmless and remains supported.
        match self.rooms.entry(room_id.to_string()) {
            Entry::Occupied(mut existing) => {
                let owned_by_other_connection = existing
                    .get()
                    .desktop
                    .as_ref()
                    .is_some_and(|desktop| desktop.conn_id != conn_id);
                if owned_by_other_connection {
                    warn!("Rejected attempt to replace an active relay room");
                    return false;
                }
                existing.insert(room);
            }
            Entry::Vacant(vacant) => {
                vacant.insert(room);
            }
        }

        if let Some(previous_room_id) = self
            .conn_to_room
            .insert(conn_id, room_id.to_string())
            .filter(|previous| previous != room_id)
        {
            let should_remove =
                if let Some(mut previous_room) = self.rooms.get_mut(&previous_room_id) {
                    if previous_room
                        .desktop
                        .as_ref()
                        .is_some_and(|desktop| desktop.conn_id == conn_id)
                    {
                        previous_room.desktop = None;
                    }
                    previous_room.is_empty()
                } else {
                    false
                };
            if should_remove {
                self.rooms.remove(&previous_room_id);
            }
        }

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
            send_outbound_message(&tx, OutboundMessage::text(message)).await
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
        room_id: &str,
        correlation_id: String,
    ) -> Option<(PendingRequestGuard, oneshot::Receiver<ResponsePayload>)> {
        let permit = Arc::clone(&self.pending_permits).try_acquire_owned().ok()?;
        if !self.try_acquire_room_pending(room_id) {
            drop(permit);
            return None;
        }

        let (tx, rx) = oneshot::channel();
        let guard = PendingRequestGuard {
            room_manager: Arc::clone(self),
            correlation_id: correlation_id.clone(),
        };
        if let Some(previous) = self.pending_requests.insert(
            correlation_id,
            PendingRequest {
                tx,
                room_id: room_id.to_string(),
                _permit: permit,
            },
        ) {
            self.release_room_pending(&previous.room_id);
        }
        Some((guard, rx))
    }

    /// Resolve a pending response only when it originates from the desktop
    /// socket that currently owns the associated room. Correlation ids are not
    /// authorization secrets and must not be sufficient on their own.
    pub fn resolve_pending_from_conn(
        &self,
        conn_id: ConnId,
        correlation_id: &str,
        payload: ResponsePayload,
    ) -> bool {
        let expected_room_id = self
            .pending_requests
            .get(correlation_id)
            .map(|pending| pending.room_id.clone());
        let owns_expected_room = expected_room_id.as_ref().is_some_and(|expected| {
            self.conn_to_room
                .get(&conn_id)
                .is_some_and(|actual| actual.value() == expected)
                && self.rooms.get(expected).is_some_and(|room| {
                    room.desktop
                        .as_ref()
                        .is_some_and(|desktop| desktop.conn_id == conn_id)
                })
        });
        if !owns_expected_room {
            warn!("Rejected relay response from a socket that does not own the pending room");
            return false;
        }

        if let Some((_, pending)) = self.pending_requests.remove(correlation_id) {
            self.release_room_pending(&pending.room_id);
            pending.tx.send(payload).is_ok()
        } else {
            warn!("No pending request for correlation_id={correlation_id}");
            false
        }
    }

    pub fn cancel_pending(&self, correlation_id: &str) {
        if let Some((_, pending)) = self.pending_requests.remove(correlation_id) {
            self.release_room_pending(&pending.room_id);
        }
    }

    fn try_acquire_room_pending(&self, room_id: &str) -> bool {
        let mut count = self
            .pending_room_counts
            .entry(room_id.to_string())
            .or_insert(0);
        if *count >= MAX_PENDING_REQUESTS_PER_ROOM {
            return false;
        }
        *count += 1;
        true
    }

    fn release_room_pending(&self, room_id: &str) {
        if let Entry::Occupied(mut entry) = self.pending_room_counts.entry(room_id.to_string()) {
            let should_remove = {
                let count = entry.get_mut();
                *count = count.saturating_sub(1);
                *count == 0
            };
            if should_remove {
                entry.remove();
            }
        }
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
        if ttl_secs == 0 {
            return Vec::new();
        }
        let now = Utc::now().timestamp();
        let stale_ids: Vec<String> = self
            .rooms
            .iter()
            .filter(|r| now.saturating_sub(r.last_activity) as u64 > ttl_secs)
            .map(|r| r.room_id.clone())
            .collect();

        for room_id in &stale_ids {
            let pending_ids: Vec<String> = self
                .pending_requests
                .iter()
                .filter(|pending| pending.room_id == *room_id)
                .map(|pending| pending.key().clone())
                .collect();
            for correlation_id in pending_ids {
                self.cancel_pending(&correlation_id);
            }
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

    pub fn pending_request_count(&self) -> usize {
        self.pending_requests.len()
    }

    pub fn has_connection(&self, conn_id: ConnId) -> bool {
        self.conn_to_room.contains_key(&conn_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn outbound_send_waits_for_bounded_queue_capacity() {
        let (tx, mut rx) = mpsc::channel(1);

        assert!(send_outbound_message(&tx, OutboundMessage::text("first"),).await);

        let blocked_send = tokio::spawn({
            let tx = tx.clone();
            async move { send_outbound_message(&tx, OutboundMessage::text("second")).await }
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
            let room_id = format!("room-{index}");
            let (guard, _rx) = manager
                .try_register_pending(&room_id, format!("pending-{index}"))
                .expect("pending registration within limit");
            guards.push(guard);
        }

        assert!(manager
            .try_register_pending("overflow-room", "overflow".to_string())
            .is_none());
        drop(guards.pop());
        assert!(manager
            .try_register_pending("after-cancel-room", "after-cancel".to_string())
            .is_some());
    }

    #[test]
    fn pending_registration_is_bounded_per_room_without_starving_other_rooms() {
        let manager = RoomManager::new();
        let mut guards = Vec::new();

        for index in 0..MAX_PENDING_REQUESTS_PER_ROOM {
            let (guard, _rx) = manager
                .try_register_pending("room-a", format!("room-a-{index}"))
                .expect("room-a pending registration within per-room limit");
            guards.push(guard);
        }

        assert!(manager
            .try_register_pending("room-a", "room-a-overflow".to_string())
            .is_none());
        assert!(manager
            .try_register_pending("room-b", "room-b-still-healthy".to_string())
            .is_some());
    }

    #[test]
    fn pending_room_counts_are_reclaimed_after_cancel_and_resolve() {
        let manager = RoomManager::new();

        let (_guard, _rx) = manager
            .try_register_pending("room-a", "pending-a".to_string())
            .expect("pending registration");
        assert!(manager.pending_room_counts.contains_key("room-a"));

        manager.cancel_pending("pending-a");
        assert!(!manager.pending_room_counts.contains_key("room-a"));

        let (_guard, _rx) = manager
            .try_register_pending("room-b", "pending-b".to_string())
            .expect("pending registration");
        let (tx, _rx) = mpsc::channel(1);
        assert!(manager.create_room("room-b", 2, "desktop-b", "public-key", tx));
        assert!(manager.resolve_pending_from_conn(
            2,
            "pending-b",
            ResponsePayload {
                encrypted_data: "encrypted".to_string(),
                nonce: "nonce".to_string(),
            },
        ));
        assert!(!manager.pending_room_counts.contains_key("room-b"));
    }

    #[test]
    fn active_room_cannot_be_replaced_by_another_connection() {
        let manager = RoomManager::new();
        let (tx_a, _rx_a) = mpsc::channel(1);
        let (tx_b, _rx_b) = mpsc::channel(1);

        assert!(manager.create_room("room-a", 1, "desktop-a", "key-a", tx_a));
        assert!(!manager.create_room("room-a", 2, "desktop-b", "key-b", tx_b));
        assert!(manager.heartbeat(1));
        assert!(!manager.heartbeat(2));
    }

    #[test]
    fn pending_response_must_come_from_owning_room_connection() {
        let manager = RoomManager::new();
        let (tx_a, _rx_a) = mpsc::channel(1);
        let (tx_b, _rx_b) = mpsc::channel(1);
        assert!(manager.create_room("room-a", 1, "desktop-a", "key-a", tx_a));
        assert!(manager.create_room("room-b", 2, "desktop-b", "key-b", tx_b));
        let (_guard, mut rx) = manager
            .try_register_pending("room-a", "pending-a".to_string())
            .expect("pending registration");
        let response = ResponsePayload {
            encrypted_data: "encrypted".to_string(),
            nonce: "nonce".to_string(),
        };

        assert!(!manager.resolve_pending_from_conn(2, "pending-a", response.clone()));
        assert!(rx.try_recv().is_err());
        assert!(manager.resolve_pending_from_conn(1, "pending-a", response));
    }

    #[test]
    fn room_ids_are_single_portable_path_segments() {
        for valid in ["room-a", "ROOM_1", "0123456789abcdef"] {
            assert!(is_valid_room_id(valid), "room id should be valid: {valid}");
        }
        for invalid in [
            "",
            "../room",
            "/tmp/room",
            "room/child",
            "room\\child",
            "_store",
            "page-data",
            "pages",
        ] {
            assert!(
                !is_valid_room_id(invalid),
                "room id should be rejected: {invalid}"
            );
        }
    }
}
