//! Provider-neutral session state manager.
//!
//! This manager owns runtime session state transitions and emits the stable
//! product event projection without depending on `bitfun-core`.

use crate::event_queue::EventQueue;
use crate::session_state::{session_state_label_for_state, ProcessingPhase, SessionState};
use bitfun_events::{AgenticEvent, AgenticEventPriority as EventPriority};
use dashmap::DashMap;
use log::debug;
use std::sync::Arc;

pub struct SessionStateManager {
    states: Arc<DashMap<String, SessionState>>,
    event_queue: Arc<EventQueue>,
}

impl SessionStateManager {
    pub fn new(event_queue: Arc<EventQueue>) -> Self {
        Self {
            states: Arc::new(DashMap::new()),
            event_queue,
        }
    }

    pub async fn initialize(&self, session_id: &str) {
        self.states
            .insert(session_id.to_string(), SessionState::Idle);
    }

    pub fn get_state(&self, session_id: &str) -> Option<SessionState> {
        self.states.get(session_id).map(|state| state.clone())
    }

    pub async fn update_state(&self, session_id: &str, new_state: SessionState) {
        let should_emit = if let Some(mut state) = self.states.get_mut(session_id) {
            *state = new_state.clone();
            true
        } else {
            false
        };

        if should_emit {
            self.emit_state_change_event(session_id, new_state).await;
        }
    }

    pub async fn set_processing_phase(
        &self,
        session_id: &str,
        current_turn_id: String,
        phase: ProcessingPhase,
    ) {
        self.update_state(
            session_id,
            SessionState::Processing {
                current_turn_id,
                phase,
            },
        )
        .await;
    }

    pub async fn set_idle(&self, session_id: &str) {
        self.update_state(session_id, SessionState::Idle).await;
    }

    pub async fn set_error(&self, session_id: &str, error: String, recoverable: bool) {
        self.update_state(session_id, SessionState::Error { error, recoverable })
            .await;
    }

    pub fn can_start_new_turn(&self, session_id: &str) -> bool {
        matches!(
            self.get_state(session_id),
            Some(SessionState::Idle)
                | Some(SessionState::Error {
                    recoverable: true,
                    ..
                })
        )
    }

    pub fn is_processing(&self, session_id: &str) -> bool {
        matches!(
            self.get_state(session_id),
            Some(SessionState::Processing { .. })
        )
    }

    pub fn remove(&self, session_id: &str) {
        self.states.remove(session_id);
        debug!("Removed session state: session_id={}", session_id);
    }

    pub fn get_all_states(&self) -> Vec<(String, SessionState)> {
        self.states
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    async fn emit_state_change_event(&self, session_id: &str, state: SessionState) {
        let event = AgenticEvent::SessionStateChanged {
            session_id: session_id.to_string(),
            new_state: session_state_label_for_state(&state).to_string(),
        };

        let _ = self
            .event_queue
            .enqueue(event, Some(EventPriority::High))
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::SessionStateManager;
    use crate::event_queue::{EventQueue, EventQueueConfig};
    use crate::session_state::{ProcessingPhase, SessionState};
    use bitfun_events::{AgenticEvent, AgenticEventPriority as EventPriority};
    use std::sync::Arc;

    fn test_manager() -> (Arc<EventQueue>, SessionStateManager) {
        let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let manager = SessionStateManager::new(Arc::clone(&queue));
        (queue, manager)
    }

    #[tokio::test]
    async fn session_state_manager_emits_compatible_state_change_events() {
        let (queue, manager) = test_manager();

        manager.initialize("session-1").await;
        manager
            .set_processing_phase("session-1", "turn-1".to_string(), ProcessingPhase::Thinking)
            .await;

        let batch = queue.dequeue_batch(10).await;
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].priority, EventPriority::High);
        match &batch[0].event {
            AgenticEvent::SessionStateChanged {
                session_id,
                new_state,
            } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(new_state, "processing");
            }
            _ => panic!("expected session state event"),
        }
    }

    #[tokio::test]
    async fn session_state_manager_keeps_turn_start_guard_semantics() {
        let (_queue, manager) = test_manager();

        assert!(!manager.can_start_new_turn("session-1"));
        manager.initialize("session-1").await;
        assert!(manager.can_start_new_turn("session-1"));

        manager
            .set_processing_phase(
                "session-1",
                "turn-1".to_string(),
                ProcessingPhase::ToolCalling,
            )
            .await;
        assert!(manager.is_processing("session-1"));
        assert!(!manager.can_start_new_turn("session-1"));

        manager
            .set_error("session-1", "cancelled".to_string(), true)
            .await;
        assert!(manager.can_start_new_turn("session-1"));
        assert_eq!(
            manager.get_state("session-1"),
            Some(SessionState::Error {
                error: "cancelled".to_string(),
                recoverable: true,
            })
        );

        manager.remove("session-1");
        assert!(manager.get_all_states().is_empty());
    }
}
