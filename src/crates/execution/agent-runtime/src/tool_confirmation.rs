//! Tool confirmation planning and failure mapping.

use std::time::{Duration, SystemTime};

pub const CONFIRMATION_NO_TIMEOUT_SECS: u64 = 365 * 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolConfirmationRequestFacts {
    pub confirm_before_run: bool,
    pub tool_needs_permission: bool,
    pub confirmation_timeout_secs: Option<u64>,
    pub now: SystemTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolConfirmationPlan {
    Skip,
    Await {
        timeout_at: SystemTime,
        timeout_secs: Option<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolConfirmationOutcome {
    Confirmed,
    Rejected { reason: String },
    ChannelClosed,
    Timeout { tool_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolConfirmationWaitResult {
    Confirmed,
    Rejected(String),
    ChannelClosed,
    TimedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationFailureKind {
    Rejected,
    ChannelClosed,
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolConfirmationFailure {
    pub kind: ConfirmationFailureKind,
    pub state_reason: String,
    pub error_message: String,
}

pub fn resolve_tool_confirmation_plan(
    request: ToolConfirmationRequestFacts,
) -> ToolConfirmationPlan {
    if !(request.confirm_before_run && request.tool_needs_permission) {
        return ToolConfirmationPlan::Skip;
    }

    let timeout_secs = request
        .confirmation_timeout_secs
        .unwrap_or(CONFIRMATION_NO_TIMEOUT_SECS);

    ToolConfirmationPlan::Await {
        timeout_at: request.now + Duration::from_secs(timeout_secs),
        timeout_secs: request.confirmation_timeout_secs,
    }
}

pub fn resolve_confirmation_failure(
    outcome: ToolConfirmationOutcome,
) -> Option<ToolConfirmationFailure> {
    match outcome {
        ToolConfirmationOutcome::Confirmed => None,
        ToolConfirmationOutcome::Rejected { reason } => Some(ToolConfirmationFailure {
            kind: ConfirmationFailureKind::Rejected,
            state_reason: format!("User rejected: {reason}"),
            error_message: format!("Tool was rejected by user: {reason}"),
        }),
        ToolConfirmationOutcome::ChannelClosed => Some(ToolConfirmationFailure {
            kind: ConfirmationFailureKind::ChannelClosed,
            state_reason: "Confirmation channel closed".to_string(),
            error_message: "Confirmation channel closed".to_string(),
        }),
        ToolConfirmationOutcome::Timeout { tool_name } => Some(ToolConfirmationFailure {
            kind: ConfirmationFailureKind::Timeout,
            state_reason: "Confirmation timeout".to_string(),
            error_message: format!("Confirmation timeout: {tool_name}"),
        }),
    }
}

pub fn resolve_confirmation_wait_result(
    result: ToolConfirmationWaitResult,
    tool_name: &str,
) -> ToolConfirmationOutcome {
    match result {
        ToolConfirmationWaitResult::Confirmed => ToolConfirmationOutcome::Confirmed,
        ToolConfirmationWaitResult::Rejected(reason) => {
            ToolConfirmationOutcome::Rejected { reason }
        }
        ToolConfirmationWaitResult::ChannelClosed => ToolConfirmationOutcome::ChannelClosed,
        ToolConfirmationWaitResult::TimedOut => ToolConfirmationOutcome::Timeout {
            tool_name: tool_name.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_confirmation_wait_timeout_to_tool_named_outcome() {
        let outcome =
            resolve_confirmation_wait_result(ToolConfirmationWaitResult::TimedOut, "Bash");

        assert_eq!(
            outcome,
            ToolConfirmationOutcome::Timeout {
                tool_name: "Bash".to_string()
            }
        );
        let failure = resolve_confirmation_failure(outcome).unwrap();
        assert_eq!(failure.kind, ConfirmationFailureKind::Timeout);
        assert_eq!(failure.error_message, "Confirmation timeout: Bash");
    }

    #[test]
    fn preserves_rejection_reason_in_confirmation_failure() {
        let outcome = resolve_confirmation_wait_result(
            ToolConfirmationWaitResult::Rejected("no".to_string()),
            "Edit",
        );

        let failure = resolve_confirmation_failure(outcome).unwrap();
        assert_eq!(failure.kind, ConfirmationFailureKind::Rejected);
        assert_eq!(failure.error_message, "Tool was rejected by user: no");
    }
}
