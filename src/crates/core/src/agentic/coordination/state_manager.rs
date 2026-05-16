//! Session state manager
//!
//! Provides centralized management and synchronization of session state

use crate::agentic::core::{ProcessingPhase, SessionState};
use crate::agentic::events::{AgenticEvent, EventPriority, EventQueue};
use dashmap::DashMap;
use log::debug;
use std::sync::Arc;

/// Session state manager
pub struct SessionStateManager {
    /// Session states (by session ID)
    states: Arc<DashMap<String, SessionState>>,

    /// Event queue
    event_queue: Arc<EventQueue>,
}

impl SessionStateManager {
    pub fn new(event_queue: Arc<EventQueue>) -> Self {
        Self {
            states: Arc::new(DashMap::new()),
            event_queue,
        }
    }

    /// Initialize session state
    pub async fn initialize(&self, session_id: &str) {
        self.states
            .insert(session_id.to_string(), SessionState::Idle);
    }

    /// Get session state
    pub fn get_state(&self, session_id: &str) -> Option<SessionState> {
        self.states.get(session_id).map(|s| s.clone())
    }

    /// Update session state
    pub async fn update_state(&self, session_id: &str, new_state: SessionState) {
        // IMPORTANT: keep the DashMap guard scope short -- do NOT hold it across .await.
        let should_emit = if let Some(mut state) = self.states.get_mut(session_id) {
            *state = new_state.clone();
            true
        } else {
            false
        };
        // RefMut guard released here -- DashMap shard lock is free.

        if should_emit {
            self.emit_state_change_event(session_id, new_state).await;
        }
    }

    /// Set processing phase
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

    /// Set to idle
    pub async fn set_idle(&self, session_id: &str) {
        self.update_state(session_id, SessionState::Idle).await;
    }

    /// Set to error
    pub async fn set_error(&self, session_id: &str, error: String, recoverable: bool) {
        self.update_state(session_id, SessionState::Error { error, recoverable })
            .await;
    }

    /// Check if new dialog turn can be started
    /// Allows Idle state or recoverable error state (e.g., after cancellation)
    pub fn can_start_new_turn(&self, session_id: &str) -> bool {
        if let Some(state) = self.get_state(session_id) {
            matches!(
                state,
                SessionState::Idle
                    | SessionState::Error {
                        recoverable: true,
                        ..
                    }
            )
        } else {
            false
        }
    }

    /// Check if currently processing
    pub fn is_processing(&self, session_id: &str) -> bool {
        if let Some(state) = self.get_state(session_id) {
            matches!(state, SessionState::Processing { .. })
        } else {
            false
        }
    }

    /// Remove session state
    pub fn remove(&self, session_id: &str) {
        self.states.remove(session_id);
        debug!("Removed session state: session_id={}", session_id);
    }

    /// Emit state change event
    async fn emit_state_change_event(&self, session_id: &str, state: SessionState) {
        let event = AgenticEvent::SessionStateChanged {
            session_id: session_id.to_string(),
            new_state: crate::agentic::events::types::session_state_to_string(&state),
        };

        let _ = self
            .event_queue
            .enqueue(event, Some(EventPriority::High))
            .await;
    }

    /// Get all session states
    pub fn get_all_states(&self) -> Vec<(String, SessionState)> {
        self.states
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }
}
