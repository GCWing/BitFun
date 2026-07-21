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
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, watch, OwnedSemaphorePermit, Semaphore};
use tracing::{debug, info};

use crate::relay::room::{ConnId, OutboundMessage};

pub const MAX_PENDING_DEVICE_RPCS: usize = 1024;

/// An online device connection belonging to a user.
struct DeviceConn {
    #[allow(dead_code)]
    conn_id: ConnId,
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
    /// user_id → (device_id → DeviceConn)
    users: DashMap<String, DashMap<String, DeviceConn>>,
    /// conn_id → (user_id, device_id) for cleanup on disconnect.
    conn_to_device: DashMap<ConnId, (String, String)>,
    /// correlation_id → pending RPC response sender (for HTTP→WS→HTTP bridge).
    pending_rpcs: DashMap<String, PendingRpc>,
    pending_rpc_permits: Arc<Semaphore>,
}

impl DeviceManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            users: DashMap::new(),
            conn_to_device: DashMap::new(),
            pending_rpcs: DashMap::new(),
            pending_rpc_permits: Arc::new(Semaphore::new(MAX_PENDING_DEVICE_RPCS)),
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
        device_name: &str,
        conn_id: ConnId,
        tx: mpsc::Sender<OutboundMessage>,
        force_close_tx: watch::Sender<bool>,
    ) -> Vec<(String, String)> {
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

    /// Remove a device on disconnect. Returns the `(user_id, device_id)` that
    /// was removed, if any (for presence/DB cleanup by the caller).
    pub fn unregister(&self, conn_id: ConnId) -> Option<(String, String)> {
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
        removed.map(|(_, v)| v)
    }

    /// Immediately revoke a device's in-memory authorization and close its
    /// WebSocket through a dedicated control channel. The close signal cannot
    /// be starved by a saturated outbound data queue.
    pub fn disconnect_device(&self, user_id: &str, device_id: &str) -> bool {
        let removed = self
            .users
            .get(user_id)
            .and_then(|devices| devices.remove(device_id).map(|(_, device)| device));
        let Some(device) = removed else {
            return false;
        };

        self.conn_to_device.remove(&device.conn_id);
        let _ = device.force_close_tx.send(true);
        debug!("Force-disconnected device {device_id}");
        true
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
        self.users
            .get(user_id)
            .map(|d| {
                d.iter()
                    .map(|e| (e.key().clone(), e.device_name.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn connection_count(&self) -> usize {
        self.conn_to_device.len()
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

    /// Broadcast a message to *all* online devices of a user except the sender.
    pub fn broadcast_except(&self, user_id: &str, exclude_device_id: &str, text: &str) {
        let Some(user_devices) = self.users.get(user_id) else {
            return;
        };
        for entry in user_devices.iter() {
            if entry.key() != exclude_device_id {
                let tx = entry.tx.clone();
                let msg = OutboundMessage::text(text);
                // best-effort; don't block the caller on a slow peer
                let _ = tx.try_send(msg);
            }
        }
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

        mgr.register("user-1", "dev-1", "A", 1, tx.clone(), close_tx_1);
        assert_eq!(mgr.online_devices("user-1").len(), 1);

        // Reconnect with a new conn id.
        let (close_tx_2, _close_rx_2) = watch::channel(false);
        mgr.register("user-1", "dev-1", "A", 2, tx, close_tx_2);
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
        mgr.register("user-1", "dev-1", "A", 1, tx, close_tx);

        assert!(mgr.disconnect_device("user-1", "dev-1"));
        assert!(mgr.conn_mapping(1).is_none());
        assert!(mgr.online_devices("user-1").is_empty());
        close_rx.changed().await.expect("close signal");
        assert!(*close_rx.borrow());
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
