//! Shared read-only access to product agent events.

use std::sync::Arc;

use bitfun_events::AgenticEventEnvelope;
use tokio::sync::broadcast;

use crate::event_queue::{EventQueue, SessionEventReceiver};

/// Cloneable source for subscribing to the runtime's existing agent event queue.
///
/// This source is read-only: queue draining and task lifecycle remain owned by
/// the product host. Global receivers use the existing bounded broadcast;
/// session receivers use a scoped bounded channel owned by the same queue.
/// Neither path creates a forwarding task or a second event schema.
#[derive(Clone)]
pub struct AgentEventSource {
    queue: Arc<EventQueue>,
}

impl std::fmt::Debug for AgentEventSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentEventSource").finish_non_exhaustive()
    }
}

impl AgentEventSource {
    pub fn new(queue: Arc<EventQueue>) -> Self {
        Self { queue }
    }

    pub fn subscribe(&self) -> AgentEventReceiver {
        self.queue.subscribe()
    }

    pub fn subscribe_session(&self, session_id: &str) -> AgentSessionEventReceiver {
        self.queue.subscribe_session(session_id)
    }
}

/// Existing Tokio receiver types keep TUI `recv` and `try_recv` behavior
/// unchanged and avoid a second adapter layer.
pub type AgentEventReceiver = broadcast::Receiver<AgenticEventEnvelope>;

/// A bounded receiver that contains events for exactly one session and
/// releases its channel when the final subscriber is dropped.
pub type AgentSessionEventReceiver = SessionEventReceiver;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bitfun_events::AgenticEvent;
    use tokio::sync::broadcast::error::RecvError;

    use crate::event_queue::{EventQueue, EventQueueConfig};

    use super::AgentEventSource;

    #[test]
    fn source_construction_does_not_require_a_tokio_runtime() {
        let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let source = AgentEventSource::new(queue);
        let _receiver = source.subscribe();
    }

    #[tokio::test]
    async fn clones_preserve_broadcast_order() {
        let queue = Arc::new(EventQueue::new(EventQueueConfig {
            max_queue_size: 32,
            batch_size: 4,
        }));
        let source = AgentEventSource::new(queue.clone());
        let clone = source.clone();

        let mut first = source.subscribe();
        let mut second = clone.subscribe();
        for index in 0..32 {
            queue
                .enqueue(
                    AgenticEvent::SessionStateChanged {
                        session_id: "session".to_string(),
                        new_state: index.to_string(),
                    },
                    None,
                )
                .await
                .expect("event should enqueue");
        }

        for expected in 0..32 {
            let first = first.recv().await.expect("first event");
            let second = second.recv().await.expect("second event");
            assert_eq!(first.id, second.id);
            assert!(matches!(
                first.event,
                AgenticEvent::SessionStateChanged { ref new_state, .. }
                    if new_state == &expected.to_string()
            ));
        }
    }

    #[tokio::test]
    async fn source_retains_default_sized_burst() {
        let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let source = AgentEventSource::new(queue.clone());
        let mut receiver = source.subscribe();

        for index in 0..2048 {
            queue
                .enqueue(
                    AgenticEvent::SessionStateChanged {
                        session_id: "session".to_string(),
                        new_state: index.to_string(),
                    },
                    None,
                )
                .await
                .expect("event should enqueue");
        }

        for expected in 0..2048 {
            let envelope = receiver.recv().await.expect("burst event");
            assert!(matches!(
                envelope.event,
                AgenticEvent::SessionStateChanged { ref new_state, .. }
                    if new_state == &expected.to_string()
            ));
        }
    }

    #[tokio::test]
    async fn receiver_reports_lag_and_closed_source() {
        let queue = Arc::new(EventQueue::new(EventQueueConfig {
            max_queue_size: 1,
            batch_size: 1,
        }));
        let source = AgentEventSource::new(queue.clone());
        let mut receiver = source.subscribe();

        for index in 0..1025 {
            queue
                .enqueue(
                    AgenticEvent::SessionStateChanged {
                        session_id: "session".to_string(),
                        new_state: index.to_string(),
                    },
                    None,
                )
                .await
                .expect("event should enqueue");
        }
        assert_eq!(receiver.recv().await, Err(RecvError::Lagged(1)));

        drop(source);
        drop(queue);
        while receiver.recv().await.is_ok() {}
        assert_eq!(receiver.recv().await, Err(RecvError::Closed));
    }

    #[tokio::test]
    async fn scoped_receiver_ignores_other_session_backlog() {
        let queue = Arc::new(EventQueue::new(EventQueueConfig {
            max_queue_size: 1,
            batch_size: 1,
        }));
        let source = AgentEventSource::new(queue.clone());
        let mut receiver = source.subscribe_session("target-session");

        for index in 0..1025 {
            queue
                .enqueue(
                    AgenticEvent::SessionStateChanged {
                        session_id: "other-session".to_string(),
                        new_state: index.to_string(),
                    },
                    None,
                )
                .await
                .expect("unrelated event should enqueue");
        }
        queue
            .enqueue(
                AgenticEvent::SessionStateChanged {
                    session_id: "target-session".to_string(),
                    new_state: "ready".to_string(),
                },
                None,
            )
            .await
            .expect("target event should enqueue");

        let envelope = receiver.recv().await.expect("target event");
        assert!(matches!(
            envelope.event,
            AgenticEvent::SessionStateChanged { ref session_id, ref new_state }
                if session_id == "target-session" && new_state == "ready"
        ));
    }

    #[tokio::test]
    async fn scoped_receiver_has_an_independent_bounded_budget() {
        let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let source = AgentEventSource::new(queue.clone());
        let mut receiver = source.subscribe_session("target-session");

        for index in 0..1025 {
            queue
                .enqueue(
                    AgenticEvent::SessionStateChanged {
                        session_id: "target-session".to_string(),
                        new_state: index.to_string(),
                    },
                    None,
                )
                .await
                .expect("target event should enqueue");
        }

        assert_eq!(receiver.recv().await, Err(RecvError::Lagged(1)));
    }
}
