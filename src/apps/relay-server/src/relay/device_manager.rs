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
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info};

use crate::relay::room::{ConnId, OutboundMessage};

/// An online device connection belonging to a user.
struct DeviceConn {
    #[allow(dead_code)]
    conn_id: ConnId,
    device_name: String,
    tx: mpsc::Sender<OutboundMessage>,
}

/// Pending HTTP RPC response, keyed by correlation_id.
struct PendingRpc {
    tx: oneshot::Sender<RpcResponse>,
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
}

impl DeviceManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            users: DashMap::new(),
            conn_to_device: DashMap::new(),
            pending_rpcs: DashMap::new(),
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
        if let Some(prior) = entry.get(device_id) {
            if prior.conn_id != conn_id {
                self.conn_to_device.remove(&prior.conn_id);
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

    /// Force-disconnect a specific device from the account by sending a
    /// close on its WS sender.  The actual cleanup is done by the WS read
    /// loop's `unregister` when it detects the broken pipe.
    pub fn disconnect_device(&self, user_id: &str, device_id: &str) {
        if let Some(user_devices) = self.users.get(user_id) {
            if let Some(dev) = user_devices.get(device_id) {
                let _ = dev.tx.try_send(OutboundMessage {
                    text: r#"{"type":"force_disconnect"}"#.to_string(),
                });
                debug!("Force-disconnect sent to device {device_id}");
            }
        }
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
        match dev.tx.try_send(OutboundMessage {
            text: text.to_string(),
        }) {
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
                let msg = OutboundMessage {
                    text: text.to_string(),
                };
                // best-effort; don't block the caller on a slow peer
                let _ = tx.try_send(msg);
            }
        }
    }

    // ── HTTP RPC bridge ────────────────────────────────────────────────

    /// Register a pending RPC response keyed by `correlation_id`.
    /// Returns the receiver end that the HTTP handler will await.
    pub fn register_rpc(&self, correlation_id: &str) -> oneshot::Receiver<RpcResponse> {
        let (tx, rx) = oneshot::channel();
        self.pending_rpcs
            .insert(correlation_id.to_string(), PendingRpc { tx });
        rx
    }

    /// Resolve a pending RPC by `correlation_id` (called when a device
    /// sends back a DeviceMessage response via WS). Returns false if no
    /// pending RPC matches (e.g. it was a fire-and-forget WS message, not
    /// an HTTP-initiated RPC).
    pub fn resolve_rpc(&self, correlation_id: &str, response: RpcResponse) -> bool {
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

        mgr.register("user-1", "dev-1", "A", 1, tx.clone());
        assert_eq!(mgr.online_devices("user-1").len(), 1);

        // Reconnect with a new conn id.
        mgr.register("user-1", "dev-1", "A", 2, tx);
        assert_eq!(mgr.online_devices("user-1").len(), 1);
        assert_eq!(mgr.conn_mapping(2), Some(("user-1".into(), "dev-1".into())));
        assert_eq!(mgr.conn_mapping(1), None);

        // Late disconnect of the old socket must not remove the new registration.
        assert_eq!(mgr.unregister(1), None);
        assert_eq!(mgr.online_devices("user-1").len(), 1);
        assert_eq!(mgr.conn_mapping(2), Some(("user-1".into(), "dev-1".into())));

        // Closing the active conn still cleans up.
        assert_eq!(
            mgr.unregister(2),
            Some(("user-1".into(), "dev-1".into()))
        );
        assert!(mgr.online_devices("user-1").is_empty());
    }
}
