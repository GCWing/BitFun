//! DeviceEvent fan-out to attached Peer Mode controllers.

use std::sync::{Arc, OnceLock};

use bitfun_core::agentic::events::EventQueue;
use bitfun_core::service::remote_connect::encryption::encrypt_to_base64;
use bitfun_core::service::remote_connect::remote_server::RemoteCommand;
use bitfun_events::project_agentic_frontend_event;
use tokio::sync::mpsc;

use super::control::attached_controllers;

static PEER_EVENT_FANOUT_TX: OnceLock<mpsc::UnboundedSender<(String, serde_json::Value)>> =
    OnceLock::new();

/// Start a second EventQueue subscriber that fans agentic events to controllers.
pub(crate) fn start_peer_event_fanout(event_queue: Arc<EventQueue>) {
    tokio::spawn(async move {
        let mut rx = event_queue.subscribe();
        loop {
            match rx.recv().await {
                Ok(envelope) => {
                    if attached_controllers().is_empty() {
                        continue;
                    }
                    if let Some(projected) = project_agentic_frontend_event(envelope.event) {
                        fanout_peer_device_event(projected.event_name, projected.payload);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!("CLI peer event fanout lagged by {skipped} events");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

/// Queue a DeviceEvent for sequential delivery to attached controllers.
pub(crate) fn fanout_peer_device_event(event: String, payload: serde_json::Value) {
    if attached_controllers().is_empty() {
        return;
    }
    let tx = PEER_EVENT_FANOUT_TX.get_or_init(|| {
        let (tx, mut rx) = mpsc::unbounded_channel::<(String, serde_json::Value)>();
        tokio::spawn(async move {
            while let Some((event, payload)) = rx.recv().await {
                fanout_peer_device_event_once(event, payload).await;
            }
        });
        tx
    });
    if let Err(e) = tx.send((event, payload)) {
        tracing::debug!("peer event fanout queue closed: {e}");
    }
}

async fn fanout_peer_device_event_once(event: String, payload: serde_json::Value) {
    let targets = attached_controllers();
    if targets.is_empty() {
        return;
    }

    let (session, relay_client) = match crate::account::peer_fanout_context().await {
        Ok(ctx) => ctx,
        Err(e) => {
            tracing::debug!("peer event fanout skipped: {e}");
            return;
        }
    };

    let envelope = match serde_json::to_string(&RemoteCommand::DeviceEvent {
        event: event.clone(),
        payload,
    }) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("peer event fanout serialize failed: {e}");
            return;
        }
    };
    let (encrypted_data, nonce) = match encrypt_to_base64(&session.master_key, &envelope) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("peer event fanout encrypt failed: {e}");
            return;
        }
    };

    for target in targets {
        let correlation_id = uuid::Uuid::new_v4().to_string();
        if let Err(e) = relay_client
            .send_device_message(&target, &correlation_id, &encrypted_data, &nonce)
            .await
        {
            tracing::debug!("peer event fanout to {target} failed: {e}");
        }
    }
}
