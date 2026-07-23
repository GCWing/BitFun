//! Provider-neutral session state facts.

use bitfun_runtime_ports::DialogSessionStateFact;
use serde::{Deserialize, Serialize};

/// Session state shared by runtime coordination and product event projection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionState {
    Idle,
    Processing {
        current_turn_id: String,
        phase: ProcessingPhase,
    },
    Error {
        error: String,
        recoverable: bool,
    },
}

impl SessionState {
    pub const fn dialog_state_fact(&self) -> DialogSessionStateFact {
        match self {
            Self::Idle => DialogSessionStateFact::Idle,
            Self::Processing { .. } => DialogSessionStateFact::Processing,
            Self::Error { .. } => DialogSessionStateFact::Error,
        }
    }
}

/// Runtime processing phase, aligned with the existing product event payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProcessingPhase {
    Starting,
    Compacting,
    Thinking,
    Streaming,
    ToolCalling,
    ToolConfirming,
}

pub fn session_state_label_for_state(state: &SessionState) -> &'static str {
    crate::events::session_state_label(state.dialog_state_fact())
}

#[cfg(test)]
mod tests {
    use super::{session_state_label_for_state, ProcessingPhase, SessionState};
    use serde_json::json;

    #[test]
    fn session_state_labels_match_existing_event_wire_values() {
        assert_eq!(session_state_label_for_state(&SessionState::Idle), "idle");
        assert_eq!(
            session_state_label_for_state(&SessionState::Processing {
                current_turn_id: "turn-1".to_string(),
                phase: ProcessingPhase::Thinking,
            }),
            "processing"
        );
        assert_eq!(
            session_state_label_for_state(&SessionState::Error {
                error: "boom".to_string(),
                recoverable: true,
            }),
            "error"
        );
    }

    #[test]
    fn processing_state_serialization_stays_compatible() {
        let state = SessionState::Processing {
            current_turn_id: "turn-1".to_string(),
            phase: ProcessingPhase::ToolCalling,
        };

        assert_eq!(
            serde_json::to_value(&state).expect("session state should serialize"),
            json!({
                "Processing": {
                    "current_turn_id": "turn-1",
                    "phase": "ToolCalling"
                }
            })
        );
    }
}
