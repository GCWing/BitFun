use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
};

use crate::daemon::protocol::{ClientCapabilities, NotificationEnvelope, ServerMessage};

use super::notifications::{
    notification_for_event, should_send_notification, ServiceNotificationEvent,
};

#[derive(Default)]
pub(super) struct ConnectionRegistry {
    next_id: AtomicU64,
    connections: std::sync::Mutex<HashMap<u64, ConnectionState>>,
}

#[derive(Clone)]
struct ConnectionState {
    sender: mpsc::Sender<ServerMessage>,
    capabilities: ClientCapabilities,
    initialized: bool,
}

impl ConnectionRegistry {
    pub(super) fn register(&self, sender: mpsc::Sender<ServerMessage>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let mut connections = match self.connections.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        connections.insert(
            id,
            ConnectionState {
                sender,
                capabilities: ClientCapabilities::default(),
                initialized: false,
            },
        );
        id
    }

    pub(super) fn unregister(&self, connection_id: u64) {
        let mut connections = match self.connections.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        connections.remove(&connection_id);
    }

    pub(super) fn set_capabilities(&self, connection_id: u64, capabilities: ClientCapabilities) {
        let mut connections = match self.connections.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(connection) = connections.get_mut(&connection_id) {
            connection.capabilities = capabilities;
        }
    }

    pub(super) fn mark_initialized(&self, connection_id: u64) {
        let mut connections = match self.connections.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(connection) = connections.get_mut(&connection_id) {
            connection.initialized = true;
        }
    }

    pub(super) fn broadcast(&self, event: ServiceNotificationEvent) {
        let notification = match notification_for_event(event) {
            Some(notification) => notification,
            None => return,
        };
        let recipients = {
            let connections = match self.connections.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            connections
                .values()
                .filter(|connection| {
                    should_send_notification(
                        connection.initialized,
                        &connection.capabilities,
                        &notification,
                    )
                })
                .map(|connection| connection.sender.clone())
                .collect::<Vec<_>>()
        };
        for sender in recipients {
            let _ = sender.send(ServerMessage::Notification(NotificationEnvelope::new(
                notification.clone(),
            )));
        }
    }
}
