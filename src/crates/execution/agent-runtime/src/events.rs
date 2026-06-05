//! Runtime event owner facts.

use crate::scheduler::{TurnOutcome, TurnOutcomeStatus};
use bitfun_runtime_ports::{DialogSessionStateFact, DialogTurnOutcomeKind};
use std::fmt;

/// Why a dialog execution or model round finished.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishReason {
    /// Normal completion
    Complete,
    /// Need to execute tools
    ToolCalls,
    /// User cancelled
    Cancelled,
    /// Error
    Error,
}

impl FinishReason {
    pub const fn as_str(&self) -> &'static str {
        match self {
            FinishReason::Complete => "complete",
            FinishReason::ToolCalls => "tool_calls",
            FinishReason::Cancelled => "cancelled",
            FinishReason::Error => "error",
        }
    }
}

impl fmt::Display for FinishReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub const fn session_state_label(state: DialogSessionStateFact) -> &'static str {
    match state {
        DialogSessionStateFact::Missing => "missing",
        DialogSessionStateFact::Idle => "idle",
        DialogSessionStateFact::Processing => "processing",
        DialogSessionStateFact::Error => "error",
    }
}

pub const fn turn_outcome_status_kind(status: TurnOutcomeStatus) -> DialogTurnOutcomeKind {
    match status {
        TurnOutcomeStatus::Completed => DialogTurnOutcomeKind::Completed,
        TurnOutcomeStatus::Cancelled => DialogTurnOutcomeKind::Cancelled,
        TurnOutcomeStatus::Failed => DialogTurnOutcomeKind::Failed,
    }
}

pub fn turn_outcome_kind(outcome: &TurnOutcome) -> DialogTurnOutcomeKind {
    turn_outcome_status_kind(outcome.status())
}
