//! Provider-neutral session state facts.

use bitfun_runtime_ports::DialogSessionStateFact;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

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

/// Timeout after which a `Processing` session is considered hung when its
/// `last_progress_at` marker has not advanced.
pub const DEFAULT_HUNG_TIMEOUT: Duration = Duration::from_secs(600);

/// Display/management session state (the seven-state projection).
///
/// This is a distinct layer from the runtime [`SessionState`]. The runtime
/// state owns execution-failure and retry semantics (`Error { recoverable }`,
/// consumed by `can_start_new_turn`), while this enum is the user-facing
/// projection used by the session sidebar, DAG member nodes, and Session tool
/// queries. The two layers do not conflict: `SessionState::Error` is preserved
/// unchanged and maps to `PendingAttention` here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionDisplayState {
    /// Zero messages (`dialog_turn_ids` is empty).
    Standby,
    /// A turn is actively executing.
    Processing,
    /// Has conversation history and is idle.
    Completed,
    /// Unresponsive beyond [`DEFAULT_HUNG_TIMEOUT`] while processing.
    Hung,
    /// Interrupted (reason captured on the session).
    Interrupted,
    /// Needs user attention (question mark).
    PendingAttention,
    /// Completed and already viewed (green dot cleared).
    Viewed,
}

impl SessionDisplayState {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Standby => "standby",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Hung => "hung",
            Self::Interrupted => "interrupted",
            Self::PendingAttention => "pending_attention",
            Self::Viewed => "viewed",
        }
    }
}

/// Derive the display state from runtime facts plus session lifecycle markers.
///
/// Precedence: `needs_attention` wins (pending user attention), then runtime
/// state drives the remaining projection. `Error` maps to `PendingAttention`
/// because a failed turn needs user handling; `Idle` distinguishes Standby
/// (zero messages) from Completed (has history) and honors the interrupt and
/// viewed markers.
#[allow(clippy::too_many_arguments)]
pub fn derive_display_state(
    state: &SessionState,
    turn_count: usize,
    interrupt_reason: Option<&str>,
    needs_attention: bool,
    viewed: bool,
    last_progress_at: Option<SystemTime>,
    now: SystemTime,
) -> SessionDisplayState {
    if needs_attention {
        return SessionDisplayState::PendingAttention;
    }
    match state {
        SessionState::Processing { .. } => {
            if interrupt_reason.is_some() {
                return SessionDisplayState::Interrupted;
            }
            if let Some(last_progress_at) = last_progress_at {
                if now.duration_since(last_progress_at).unwrap_or_default() >= DEFAULT_HUNG_TIMEOUT
                {
                    return SessionDisplayState::Hung;
                }
            }
            SessionDisplayState::Processing
        }
        SessionState::Error { .. } => SessionDisplayState::PendingAttention,
        SessionState::Idle => {
            if interrupt_reason.is_some() {
                SessionDisplayState::Interrupted
            } else if turn_count == 0 {
                SessionDisplayState::Standby
            } else if viewed {
                SessionDisplayState::Viewed
            } else {
                SessionDisplayState::Completed
            }
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
    use super::{
        derive_display_state, session_state_label_for_state, ProcessingPhase, SessionDisplayState,
        SessionState, DEFAULT_HUNG_TIMEOUT,
    };
    use serde_json::json;
    use std::time::{Duration, SystemTime};

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

    #[test]
    fn display_state_enumerates_all_seven_states() {
        let values = [
            SessionDisplayState::Standby,
            SessionDisplayState::Processing,
            SessionDisplayState::Completed,
            SessionDisplayState::Hung,
            SessionDisplayState::Interrupted,
            SessionDisplayState::PendingAttention,
            SessionDisplayState::Viewed,
        ];
        let labels: Vec<&str> = values.iter().map(|v| v.as_str()).collect();
        assert_eq!(
            labels,
            vec![
                "standby",
                "processing",
                "completed",
                "hung",
                "interrupted",
                "pending_attention",
                "viewed"
            ]
        );
    }

    #[test]
    fn display_state_zero_messages_is_standby_and_history_is_completed() {
        let now = SystemTime::now();
        assert_eq!(
            derive_display_state(&SessionState::Idle, 0, None, false, false, None, now),
            SessionDisplayState::Standby
        );
        assert_eq!(
            derive_display_state(&SessionState::Idle, 3, None, false, false, None, now),
            SessionDisplayState::Completed
        );
        assert_eq!(
            derive_display_state(&SessionState::Idle, 3, None, false, true, None, now),
            SessionDisplayState::Viewed
        );
    }

    #[test]
    fn display_state_error_and_attention_map_to_pending_attention() {
        let now = SystemTime::now();
        assert_eq!(
            derive_display_state(
                &SessionState::Error {
                    error: "boom".to_string(),
                    recoverable: true,
                },
                1,
                None,
                false,
                false,
                None,
                now
            ),
            SessionDisplayState::PendingAttention
        );
        assert_eq!(
            derive_display_state(&SessionState::Idle, 0, None, true, false, None, now),
            SessionDisplayState::PendingAttention
        );
    }

    #[test]
    fn display_state_distinguishes_hung_interrupted_processing() {
        let now = SystemTime::now();
        let processing = SessionState::Processing {
            current_turn_id: "turn-1".to_string(),
            phase: ProcessingPhase::ToolCalling,
        };

        // Fresh progress -> Processing.
        assert_eq!(
            derive_display_state(&processing, 1, None, false, false, Some(now), now),
            SessionDisplayState::Processing
        );

        // Interrupt reason wins while processing.
        assert_eq!(
            derive_display_state(
                &processing,
                1,
                Some("user cancelled"),
                false,
                false,
                Some(now),
                now
            ),
            SessionDisplayState::Interrupted
        );

        // Stale progress beyond the timeout -> Hung.
        let stale = now - DEFAULT_HUNG_TIMEOUT - Duration::from_secs(1);
        assert_eq!(
            derive_display_state(&processing, 1, None, false, false, Some(stale), now),
            SessionDisplayState::Hung
        );
    }
}
