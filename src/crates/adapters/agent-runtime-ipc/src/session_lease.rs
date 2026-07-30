use crate::{RuntimeIpcError, RuntimeIpcErrorCode};
use std::collections::HashMap;
use std::sync::Mutex;

/// Connection-to-Session controller leases; the Runtime remains Session owner.
#[derive(Default)]
pub(crate) struct RuntimeSessionLeases {
    state: Mutex<LeaseState>,
}

#[derive(Default)]
struct LeaseState {
    by_session: HashMap<String, String>,
    by_connection: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LeaseTransition {
    Unchanged,
    Claimed,
    Switched { previous_session_id: String },
}

impl RuntimeSessionLeases {
    pub(crate) fn switch(
        &self,
        connection_id: &str,
        session_id: &str,
    ) -> Result<LeaseTransition, RuntimeIpcError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = state.by_connection.get(connection_id).cloned();
        if previous.as_deref() == Some(session_id) {
            return Ok(LeaseTransition::Unchanged);
        }
        if state.by_session.contains_key(session_id) {
            return Err(error(
                RuntimeIpcErrorCode::SessionInUse,
                "session already has an active Shared TUI controller",
            ));
        }

        if let Some(previous_session_id) = previous {
            state.by_session.remove(&previous_session_id);
            state
                .by_session
                .insert(session_id.to_string(), connection_id.to_string());
            state
                .by_connection
                .insert(connection_id.to_string(), session_id.to_string());
            Ok(LeaseTransition::Switched {
                previous_session_id,
            })
        } else {
            state
                .by_session
                .insert(session_id.to_string(), connection_id.to_string());
            state
                .by_connection
                .insert(connection_id.to_string(), session_id.to_string());
            Ok(LeaseTransition::Claimed)
        }
    }

    pub(crate) fn rollback(&self, connection_id: &str, transition: LeaseTransition) {
        match transition {
            LeaseTransition::Unchanged => {}
            LeaseTransition::Claimed => {
                self.release_connection(connection_id);
            }
            LeaseTransition::Switched {
                previous_session_id,
            } => {
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if let Some(current_session_id) = state.by_connection.get(connection_id).cloned() {
                    state.by_session.remove(&current_session_id);
                }
                state
                    .by_session
                    .insert(previous_session_id.clone(), connection_id.to_string());
                state
                    .by_connection
                    .insert(connection_id.to_string(), previous_session_id);
            }
        }
    }

    pub(crate) fn validate(
        &self,
        connection_id: &str,
        session_id: &str,
    ) -> Result<(), RuntimeIpcError> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match state.by_connection.get(connection_id) {
            None => Err(error(
                RuntimeIpcErrorCode::ControllerRequired,
                "operation requires an attached Shared TUI session",
            )),
            Some(attached) if attached == session_id => Ok(()),
            Some(_) => Err(error(
                RuntimeIpcErrorCode::SessionMismatch,
                "operation targets a different session than this connection controls",
            )),
        }
    }

    pub(crate) fn validate_uncontrolled(&self, session_id: &str) -> Result<(), RuntimeIpcError> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.by_session.contains_key(session_id) {
            return Err(error(
                RuntimeIpcErrorCode::SessionInUse,
                "session already has an active Shared TUI controller",
            ));
        }
        Ok(())
    }

    pub(crate) fn release_connection(&self, connection_id: &str) -> Option<String> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let session_id = state.by_connection.remove(connection_id)?;
        state.by_session.remove(&session_id);
        Some(session_id)
    }

    pub(crate) fn attached_session(&self, connection_id: &str) -> Option<String> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .by_connection
            .get(connection_id)
            .cloned()
    }
}

fn error(code: RuntimeIpcErrorCode, message: &str) -> RuntimeIpcError {
    RuntimeIpcError {
        code,
        message: message.to_string(),
    }
}
