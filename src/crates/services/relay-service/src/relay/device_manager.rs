//! Per-user online device registry for account-based device routing.
//!
//! This is a **parallel** pathway to `RoomManager`: the existing QR-pairing
//! flow keeps using rooms (1 desktop per room, unchanged). Account-logged-in
//! devices register here, scoped by `user_id`, and can route
//! `device_to_device` messages to each other. The relay never decrypts the
//! payloads — it only routes by `(user_id, target_device_id)`.
//!
//! The manager also supports HTTP RPC: a request can register a pending
//! response keyed by `correlation_id`, and when a `DeviceMessage` response
//! arrives via WS from a device, the pending future is resolved.

use dashmap::DashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, MutexGuard,
};
use tokio::sync::{mpsc, oneshot, watch, OwnedSemaphorePermit, Semaphore};
use tracing::{debug, info};

use crate::relay::room::{ConnId, OutboundMessage};

pub const MAX_PENDING_DEVICE_RPCS: usize = 1024;

/// An online device connection belonging to a user.
struct DeviceConn {
    #[allow(dead_code)]
    conn_id: ConnId,
    /// Bearer token that authenticated this exact socket. A device may have
    /// more than one still-valid token (for example while a replacement login
    /// is being verified), so logout must not disconnect the socket merely
    /// because its `(user_id, device_id)` matches another token.
    auth_token: String,
    device_name: String,
    tx: mpsc::Sender<OutboundMessage>,
    force_close_tx: watch::Sender<bool>,
}

/// A socket that passed the initial token lookup but has not yet completed the
/// post-registration revocation check. Pending sockets are deliberately absent
/// from presence and routing, and do not replace an active socket.
struct PendingDeviceConn {
    user_id: String,
    device_id: String,
    auth_token: String,
    device_name: String,
    tx: mpsc::Sender<OutboundMessage>,
    force_close_tx: watch::Sender<bool>,
}

/// Pending HTTP RPC response, keyed by correlation_id.
struct PendingRpc {
    tx: oneshot::Sender<RpcResponse>,
    user_id: String,
    target_device_id: String,
    _permit: OwnedSemaphorePermit,
}

/// The response payload from a device RPC call.
#[derive(Debug, Clone)]
pub struct RpcResponse {
    pub encrypted_data: String,
    pub nonce: String,
}

/// Tracks online devices grouped by `user_id` so that `device_to_device`
/// messages can be routed within an account without exposing other accounts.
pub struct DeviceManager {
    /// Serializes presence mutations with authoritative snapshot broadcasts.
    ///
    /// DashMap keeps individual registry operations safe, but a presence
    /// message is a compound operation: capture the current membership and
    /// enqueue that snapshot to every current member. Without this gate, an
    /// older logout/disconnect task can resume after a reconnect and publish
    /// a stale snapshot after the reconnect's newer one.
    presence_gate: Mutex<()>,
    /// Serializes registry ownership changes with their SQLite `online`
    /// projection across async route handlers. This is intentionally global:
    /// connects/logouts are rare, and one lock avoids an unbounded per-device
    /// lock registry while making the durable last-writer deterministic.
    presence_projection_gate: tokio::sync::Mutex<()>,
    /// user_id → (device_id → DeviceConn)
    users: DashMap<String, DashMap<String, DeviceConn>>,
    /// conn_id → (user_id, device_id) for cleanup on disconnect.
    conn_to_device: DashMap<ConnId, (String, String)>,
    /// conn_id → provisional device connection awaiting its final token check.
    pending_connections: DashMap<ConnId, PendingDeviceConn>,
    /// correlation_id → pending RPC response sender (for HTTP→WS→HTTP bridge).
    pending_rpcs: DashMap<String, PendingRpc>,
    pending_rpc_permits: Arc<Semaphore>,
    /// Starts the database-backed token revalidator exactly once, lazily from
    /// the first WebSocket handled inside a Tokio runtime.
    token_revalidator_started: AtomicBool,
}

impl DeviceManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            presence_gate: Mutex::new(()),
            presence_projection_gate: tokio::sync::Mutex::new(()),
            users: DashMap::new(),
            conn_to_device: DashMap::new(),
            pending_connections: DashMap::new(),
            pending_rpcs: DashMap::new(),
            pending_rpc_permits: Arc::new(Semaphore::new(MAX_PENDING_DEVICE_RPCS)),
            token_revalidator_started: AtomicBool::new(false),
        })
    }

    /// Register an online device under a user. Replaces any prior connection
    /// for the same `(user_id, device_id)` (reconnect). Returns the list of
    /// *other* online device ids in the account so the caller can push a
    /// presence update.
    pub fn register(
        &self,
        user_id: &str,
        device_id: &str,
        auth_token: &str,
        device_name: &str,
        conn_id: ConnId,
        tx: mpsc::Sender<OutboundMessage>,
        force_close_tx: watch::Sender<bool>,
    ) -> Vec<(String, String)> {
        let _presence_guard = self.lock_presence();
        // Remove any stale conn mapping for this conn first.
        if let Some((_, (old_user, old_device))) = self.conn_to_device.remove(&conn_id) {
            if let Some(user_devices) = self.users.get(&old_user) {
                // Only drop the device if this conn still owns it.
                if user_devices
                    .get(&old_device)
                    .map(|d| d.conn_id == conn_id)
                    .unwrap_or(false)
                {
                    user_devices.remove(&old_device);
                }
            }
        }

        let entry = self.users.entry(user_id.to_string()).or_default();
        // If this device already has a live conn, drop the stale conn→device
        // mapping so a later disconnect of the old socket cannot unregister
        // the replacement connection.
        let prior = entry
            .get(device_id)
            .map(|prior| (prior.conn_id, prior.force_close_tx.clone()));
        if let Some((prior_conn_id, prior_close_tx)) = prior {
            if prior_conn_id != conn_id {
                self.conn_to_device.remove(&prior_conn_id);
                let _ = prior_close_tx.send(true);
            }
        }
        let others: Vec<(String, String)> = entry
            .iter()
            .filter(|d| d.key() != device_id)
            .map(|d| (d.key().clone(), d.device_name.clone()))
            .collect();
        entry.insert(
            device_id.to_string(),
            DeviceConn {
                conn_id,
                auth_token: auth_token.to_string(),
                device_name: device_name.to_string(),
                tx,
                force_close_tx,
            },
        );
        self.conn_to_device
            .insert(conn_id, (user_id.to_string(), device_id.to_string()));

        info!(
            "Device {device_id} registered for user {user_id} ({} online)",
            entry.len()
        );
        others
    }

    /// Stage a connection while an async post-registration token check runs.
    /// It cannot receive routed messages or presence and cannot evict an
    /// already-authorized connection for the same physical device.
    pub fn register_pending(
        &self,
        user_id: &str,
        device_id: &str,
        auth_token: &str,
        device_name: &str,
        conn_id: ConnId,
        tx: mpsc::Sender<OutboundMessage>,
        force_close_tx: watch::Sender<bool>,
    ) {
        let _presence_guard = self.lock_presence();
        if let Some((_, previous)) = self.pending_connections.remove(&conn_id) {
            let _ = previous.force_close_tx.send(true);
        }
        self.pending_connections.insert(
            conn_id,
            PendingDeviceConn {
                user_id: user_id.to_string(),
                device_id: device_id.to_string(),
                auth_token: auth_token.to_string(),
                device_name: device_name.to_string(),
                tx,
                force_close_tx,
            },
        );
    }

    /// Promote the exact pending connection to the single active owner for
    /// `(user_id, device_id)`. Returns false if logout/delete already removed
    /// it while the final database lookup was in flight.
    pub fn activate_pending_with_initial_message(
        &self,
        user_id: &str,
        device_id: &str,
        auth_token: &str,
        conn_id: ConnId,
        initial_text: &str,
    ) -> bool {
        let _presence_guard = self.lock_presence();
        let pending = self
            .pending_connections
            .remove_if(&conn_id, |_, pending| {
                pending.user_id == user_id
                    && pending.device_id == device_id
                    && pending.auth_token == auth_token
            })
            .map(|(_, pending)| pending);
        let Some(pending) = pending else {
            return false;
        };

        // Queue AuthOk while the candidate is still invisible and while the
        // same presence gate excludes snapshot broadcasts. Once membership is
        // published below, every later DevicePresence is necessarily behind
        // AuthOk in this socket's FIFO queue.
        if pending
            .tx
            .try_send(OutboundMessage::text(initial_text))
            .is_err()
        {
            let _ = pending.force_close_tx.send(true);
            return false;
        }

        let entry = self.users.entry(user_id.to_string()).or_default();
        if let Some((prior_conn_id, prior_close_tx)) = entry
            .get(device_id)
            .map(|prior| (prior.conn_id, prior.force_close_tx.clone()))
        {
            if prior_conn_id != conn_id {
                self.conn_to_device.remove(&prior_conn_id);
                let _ = prior_close_tx.send(true);
            }
        }
        entry.insert(
            device_id.to_string(),
            DeviceConn {
                conn_id,
                auth_token: pending.auth_token,
                device_name: pending.device_name,
                tx: pending.tx,
                force_close_tx: pending.force_close_tx,
            },
        );
        self.conn_to_device
            .insert(conn_id, (user_id.to_string(), device_id.to_string()));
        info!(
            "Device {device_id} activated for user {user_id} ({} online)",
            entry.len()
        );
        true
    }

    /// Remove a device on disconnect. Returns the `(user_id, device_id)` that
    /// was removed, if any (for presence/DB cleanup by the caller).
    pub fn unregister(&self, conn_id: ConnId) -> Option<(String, String)> {
        let _presence_guard = self.lock_presence();
        let removed = self.conn_to_device.remove(&conn_id);
        if let Some((_, (user_id, device_id))) = &removed {
            if let Some(user_devices) = self.users.get(user_id) {
                // Only remove if this closing conn is still the active owner.
                // A newer reconnect may have already replaced the mapping.
                let still_owner = user_devices
                    .get(device_id)
                    .map(|d| d.conn_id == conn_id)
                    .unwrap_or(false);
                if still_owner {
                    user_devices.remove(device_id);
                    debug!("Device {device_id} disconnected from user {user_id}");
                    return Some((user_id.clone(), device_id.clone()));
                }
                debug!(
                    "Ignoring stale unregister for device {device_id} (conn {conn_id} superseded)"
                );
                return None;
            }
        }
        if let Some((_, pending)) = self.pending_connections.remove(&conn_id) {
            return Some((pending.user_id, pending.device_id));
        }
        removed.map(|(_, v)| v)
    }

    /// Reject one provisional AuthConnect without affecting an already-active
    /// socket that may legitimately use the same still-valid token.
    pub fn disconnect_pending(&self, conn_id: ConnId) -> bool {
        let _presence_guard = self.lock_presence();
        let Some((_, pending)) = self.pending_connections.remove(&conn_id) else {
            return false;
        };
        let _ = pending.force_close_tx.send(true);
        true
    }

    /// Immediately revoke a device's in-memory authorization and close its
    /// WebSocket through a dedicated control channel. The close signal cannot
    /// be starved by a saturated outbound data queue.
    pub fn disconnect_device(&self, user_id: &str, device_id: &str) -> bool {
        let _presence_guard = self.lock_presence();
        let active = self
            .users
            .get(user_id)
            .and_then(|devices| devices.remove(device_id).map(|(_, device)| device));
        if let Some(device) = active.as_ref() {
            self.conn_to_device.remove(&device.conn_id);
            let _ = device.force_close_tx.send(true);
        }
        let pending_ids: Vec<_> = self
            .pending_connections
            .iter()
            .filter(|entry| entry.user_id == user_id && entry.device_id == device_id)
            .map(|entry| *entry.key())
            .collect();
        let mut removed_pending = false;
        for pending_id in pending_ids {
            if let Some((_, pending)) = self.pending_connections.remove(&pending_id) {
                removed_pending = true;
                let _ = pending.force_close_tx.send(true);
            }
        }
        let removed = active.is_some() || removed_pending;
        if removed {
            debug!("Force-disconnected device {device_id}");
        }
        removed
    }

    /// Disconnect only when the current socket was authenticated by
    /// `auth_token`. Revoking a second token for the same machine must leave a
    /// still-valid replacement/previous socket untouched.
    pub fn disconnect_device_if_token(
        &self,
        user_id: &str,
        device_id: &str,
        auth_token: &str,
    ) -> bool {
        let _presence_guard = self.lock_presence();
        // `remove_if` keeps the token comparison and removal inside one shard
        // lock. A reconnect cannot replace socket A with socket B between a
        // successful comparison against A and the removal of the entry.
        let active = self.users.get(user_id).and_then(|devices| {
            devices
                .remove_if(device_id, |_, device| device.auth_token == auth_token)
                .map(|(_, device)| device)
        });
        if let Some(device) = active.as_ref() {
            self.conn_to_device.remove(&device.conn_id);
            let _ = device.force_close_tx.send(true);
        }
        let pending_ids: Vec<_> = self
            .pending_connections
            .iter()
            .filter(|entry| {
                entry.user_id == user_id
                    && entry.device_id == device_id
                    && entry.auth_token == auth_token
            })
            .map(|entry| *entry.key())
            .collect();
        let mut removed_pending = false;
        for pending_id in pending_ids {
            if let Some((_, pending)) = self.pending_connections.remove(&pending_id) {
                removed_pending = true;
                let _ = pending.force_close_tx.send(true);
            }
        }
        let removed = active.is_some() || removed_pending;
        if removed {
            debug!("Force-disconnected device {device_id} for its revoked token");
        }
        removed
    }

    /// Route a raw JSON text message to `target_device_id` within `user_id`.
    /// Returns false if the target is offline or its queue is full.
    pub fn route_message(&self, user_id: &str, target_device_id: &str, text: &str) -> bool {
        let Some(user_devices) = self.users.get(user_id) else {
            return false;
        };
        let Some(dev) = user_devices.get(target_device_id) else {
            return false;
        };
        match dev.tx.try_send(OutboundMessage::text(text)) {
            Ok(()) => true,
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                debug!("route_message: target {target_device_id} queue full, dropping");
                false
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => false,
        }
    }

    /// List currently online `(device_id, device_name)` for a user (for
    /// presence broadcasts).
    pub fn online_devices(&self, user_id: &str) -> Vec<(String, String)> {
        let _presence_guard = self.lock_presence();
        self.online_devices_unlocked(user_id)
    }

    fn online_devices_unlocked(&self, user_id: &str) -> Vec<(String, String)> {
        self.users
            .get(user_id)
            .map(|d| {
                d.iter()
                    .map(|e| (e.key().clone(), e.device_name.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Whether the in-memory routing registry currently has an owner for this
    /// device. The registry is the live-connection authority; durable `online`
    /// flags are projected from it.
    pub fn is_device_online(&self, user_id: &str, device_id: &str) -> bool {
        self.users
            .get(user_id)
            .is_some_and(|devices| devices.contains_key(device_id))
    }

    /// Snapshot all active socket credentials for the periodic revocation
    /// revalidator. Pending candidates are excluded because their activation
    /// path already performs two serialized database checks.
    pub fn active_device_credentials(&self) -> Vec<(ConnId, String, String, String)> {
        let _presence_guard = self.lock_presence();
        self.users
            .iter()
            .flat_map(|user| {
                let user_id = user.key().clone();
                user.value()
                    .iter()
                    .map(move |device| {
                        (
                            device.conn_id,
                            user_id.clone(),
                            device.key().clone(),
                            device.auth_token.clone(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Return true only for the first caller. The background worker is
    /// intentionally launched by the WebSocket entrypoint rather than the
    /// synchronous router builder, which may run before a Tokio runtime exists.
    pub fn claim_token_revalidator_start(&self) -> bool {
        self.token_revalidator_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub fn connection_count(&self) -> usize {
        self.conn_to_device.len() + self.pending_connections.len()
    }

    pub fn pending_rpc_count(&self) -> usize {
        self.pending_rpcs.len()
    }

    /// Look up the `(user_id, device_id)` owning a connection (for routing
    /// device-to-device messages from the sender's conn). Returns an owned
    /// copy because the registry guard is released on return.
    pub fn conn_mapping(&self, conn_id: ConnId) -> Option<(String, String)> {
        self.conn_to_device.get(&conn_id).map(|e| e.value().clone())
    }

    pub fn has_connection(&self, conn_id: ConnId) -> bool {
        self.conn_to_device.contains_key(&conn_id)
            || self.pending_connections.contains_key(&conn_id)
    }

    pub async fn lock_presence_projection(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.presence_projection_gate.lock().await
    }

    /// Build and broadcast one authoritative presence snapshot to every
    /// currently registered device. Membership cannot change between the
    /// snapshot and the queue writes, and delayed lifecycle tasks always
    /// rebuild from current state instead of publishing a captured old list.
    pub fn broadcast_current_presence<F>(&self, user_id: &str, build_message: F)
    where
        F: FnOnce(&[(String, String)]) -> Option<String>,
    {
        let _presence_guard = self.lock_presence();
        let devices = self.online_devices_unlocked(user_id);
        let Some(text) = build_message(&devices) else {
            return;
        };
        let Some(user_devices) = self.users.get(user_id) else {
            return;
        };
        for entry in user_devices.iter() {
            let tx = entry.tx.clone();
            let msg = OutboundMessage::text(&text);
            // best-effort; don't block the caller on a slow peer
            let _ = tx.try_send(msg);
        }
    }

    fn lock_presence(&self) -> MutexGuard<'_, ()> {
        self.presence_gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    // ── HTTP RPC bridge ────────────────────────────────────────────────

    /// Register a pending RPC response keyed by `correlation_id`.
    /// Returns the receiver end that the HTTP handler will await.
    pub fn register_rpc(
        &self,
        correlation_id: &str,
        user_id: &str,
        target_device_id: &str,
    ) -> Option<oneshot::Receiver<RpcResponse>> {
        let permit = Arc::clone(&self.pending_rpc_permits)
            .try_acquire_owned()
            .ok()?;
        let (tx, rx) = oneshot::channel();
        self.pending_rpcs.insert(
            correlation_id.to_string(),
            PendingRpc {
                tx,
                user_id: user_id.to_string(),
                target_device_id: target_device_id.to_string(),
                _permit: permit,
            },
        );
        Some(rx)
    }

    /// Resolve a pending RPC by `correlation_id` (called when a device
    /// sends back a DeviceMessage response via WS). Returns false if no
    /// pending RPC matches (e.g. it was a fire-and-forget WS message, not
    /// an HTTP-initiated RPC).
    pub fn resolve_rpc(
        &self,
        correlation_id: &str,
        user_id: &str,
        source_device_id: &str,
        response: RpcResponse,
    ) -> bool {
        let is_expected_source = self
            .pending_rpcs
            .get(correlation_id)
            .map(|pending| {
                pending.user_id == user_id && pending.target_device_id == source_device_id
            })
            .unwrap_or(false);
        if !is_expected_source {
            return false;
        }
        if let Some(entry) = self.pending_rpcs.remove(correlation_id) {
            let _ = entry.1.tx.send(response);
            true
        } else {
            false
        }
    }

    /// Cancel a pending RPC (called on timeout/error).
    pub fn cancel_rpc(&self, correlation_id: &str) {
        self.pending_rpcs.remove(correlation_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_tx() -> mpsc::Sender<OutboundMessage> {
        let (tx, _rx) = mpsc::channel(8);
        tx
    }

    #[test]
    fn stale_unregister_does_not_drop_replacement_connection() {
        let mgr = DeviceManager::new();
        let tx = dummy_tx();
        let (close_tx_1, mut close_rx_1) = watch::channel(false);

        mgr.register("user-1", "dev-1", "token-1", "A", 1, tx.clone(), close_tx_1);
        assert_eq!(mgr.online_devices("user-1").len(), 1);

        // Reconnect with a new conn id.
        let (close_tx_2, _close_rx_2) = watch::channel(false);
        mgr.register("user-1", "dev-1", "token-2", "A", 2, tx, close_tx_2);
        assert_eq!(mgr.online_devices("user-1").len(), 1);
        assert_eq!(mgr.conn_mapping(2), Some(("user-1".into(), "dev-1".into())));
        assert_eq!(mgr.conn_mapping(1), None);
        assert!(*close_rx_1.borrow_and_update());

        // Late disconnect of the old socket must not remove the new registration.
        assert_eq!(mgr.unregister(1), None);
        assert_eq!(mgr.online_devices("user-1").len(), 1);
        assert_eq!(mgr.conn_mapping(2), Some(("user-1".into(), "dev-1".into())));

        // Closing the active conn still cleans up.
        assert_eq!(mgr.unregister(2), Some(("user-1".into(), "dev-1".into())));
        assert!(mgr.online_devices("user-1").is_empty());
    }

    #[tokio::test]
    async fn disconnect_revokes_routing_before_socket_close_is_consumed() {
        let mgr = DeviceManager::new();
        let (tx, _rx) = mpsc::channel(8);
        let (close_tx, mut close_rx) = watch::channel(false);
        mgr.register("user-1", "dev-1", "token-1", "A", 1, tx, close_tx);

        assert!(mgr.disconnect_device("user-1", "dev-1"));
        assert!(mgr.conn_mapping(1).is_none());
        assert!(mgr.online_devices("user-1").is_empty());
        close_rx.changed().await.expect("close signal");
        assert!(*close_rx.borrow());
    }

    #[tokio::test]
    async fn token_scoped_disconnect_does_not_close_another_login_socket() {
        let mgr = DeviceManager::new();
        let (tx, _rx) = mpsc::channel(8);
        let (close_tx, mut close_rx) = watch::channel(false);
        mgr.register("user-1", "dev-1", "active-token", "A", 1, tx, close_tx);

        assert!(!mgr.disconnect_device_if_token("user-1", "dev-1", "candidate-token"));
        assert_eq!(mgr.conn_mapping(1), Some(("user-1".into(), "dev-1".into())));
        assert!(!*close_rx.borrow_and_update());

        assert!(mgr.disconnect_device_if_token("user-1", "dev-1", "active-token"));
        assert!(mgr.conn_mapping(1).is_none());
        close_rx.changed().await.expect("close signal");
        assert!(*close_rx.borrow());
    }

    #[tokio::test]
    async fn revoked_pending_candidate_is_hidden_and_preserves_active_owner() {
        let mgr = DeviceManager::new();
        let (active_tx, _active_rx) = mpsc::channel(8);
        let (active_close_tx, mut active_close_rx) = watch::channel(false);
        mgr.register(
            "user-1",
            "dev-1",
            "active-token",
            "Active",
            1,
            active_tx,
            active_close_tx,
        );

        let (candidate_tx, _candidate_rx) = mpsc::channel(8);
        let (candidate_close_tx, mut candidate_close_rx) = watch::channel(false);
        mgr.register_pending(
            "user-1",
            "dev-1",
            "revoked-token",
            "Candidate",
            2,
            candidate_tx,
            candidate_close_tx,
        );

        assert_eq!(mgr.connection_count(), 2);
        assert_eq!(
            mgr.online_devices("user-1"),
            vec![("dev-1".to_string(), "Active".to_string())]
        );
        assert_eq!(mgr.conn_mapping(1), Some(("user-1".into(), "dev-1".into())));
        assert!(mgr.conn_mapping(2).is_none());
        assert!(!*active_close_rx.borrow_and_update());

        assert!(mgr.disconnect_device_if_token("user-1", "dev-1", "revoked-token"));
        candidate_close_rx.changed().await.expect("candidate close");
        assert!(*candidate_close_rx.borrow());
        assert!(!*active_close_rx.borrow_and_update());
        assert_eq!(mgr.connection_count(), 1);
        assert_eq!(mgr.conn_mapping(1), Some(("user-1".into(), "dev-1".into())));
        assert!(!mgr.activate_pending_with_initial_message(
            "user-1",
            "dev-1",
            "revoked-token",
            2,
            "auth-ok",
        ));
    }

    #[tokio::test]
    async fn delayed_presence_broadcast_uses_current_membership() {
        let mgr = DeviceManager::new();
        let (old_tx, _old_rx) = mpsc::channel(8);
        let (old_close_tx, _old_close_rx) = watch::channel(false);
        mgr.register(
            "user-1",
            "old-device",
            "old-token",
            "Old",
            1,
            old_tx,
            old_close_tx,
        );

        let (current_tx, mut current_rx) = mpsc::channel(8);
        let (current_close_tx, _current_close_rx) = watch::channel(false);
        mgr.register(
            "user-1",
            "current-device",
            "current-token",
            "Current",
            2,
            current_tx,
            current_close_tx,
        );
        assert!(mgr.disconnect_device("user-1", "old-device"));

        // Model an older lifecycle task resuming after the replacement. It
        // must serialize the live registry, not a list captured before await.
        mgr.broadcast_current_presence("user-1", |devices| serde_json::to_string(devices).ok());
        let message = current_rx.recv().await.expect("presence snapshot");
        let devices: Vec<(String, String)> = serde_json::from_str(&message.text).unwrap();
        assert_eq!(
            devices,
            vec![("current-device".to_string(), "Current".to_string())]
        );
    }

    #[tokio::test]
    async fn rpc_response_must_come_from_the_expected_account_and_device() {
        let mgr = DeviceManager::new();
        let mut response_rx = mgr
            .register_rpc("corr-1", "user-1", "desktop-1")
            .expect("RPC registration");
        let response = RpcResponse {
            encrypted_data: "ciphertext".to_string(),
            nonce: "nonce".to_string(),
        };

        assert!(!mgr.resolve_rpc("corr-1", "user-2", "desktop-1", response.clone()));
        assert!(!mgr.resolve_rpc("corr-1", "user-1", "desktop-2", response.clone()));
        assert!(response_rx.try_recv().is_err());
        assert!(mgr.resolve_rpc("corr-1", "user-1", "desktop-1", response));
        assert_eq!(
            response_rx
                .await
                .expect("expected RPC response")
                .encrypted_data,
            "ciphertext"
        );
    }

    #[test]
    fn pending_device_rpcs_are_bounded_and_permits_are_reclaimed() {
        let mgr = DeviceManager::new();
        let mut receivers = Vec::new();
        for index in 0..MAX_PENDING_DEVICE_RPCS {
            receivers.push(
                mgr.register_rpc(&format!("corr-{index}"), "user-1", "desktop-1")
                    .expect("registration within global limit"),
            );
        }
        assert!(mgr
            .register_rpc("overflow", "user-1", "desktop-1")
            .is_none());

        mgr.cancel_rpc("corr-0");
        assert!(mgr
            .register_rpc("after-cancel", "user-1", "desktop-1")
            .is_some());
        drop(receivers);
    }
}
