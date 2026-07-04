//! Tool confirmation planning and failure mapping.

use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::oneshot;

pub const CONFIRMATION_NO_TIMEOUT_SECS: u64 = 365 * 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolConfirmationRequestFacts {
    pub confirm_before_run: bool,
    pub tool_needs_permission: bool,
    pub confirmation_timeout_secs: Option<u64>,
    pub now: SystemTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolConfirmationGateFacts {
    pub global_skip_tool_confirmation: bool,
    pub context_skip_tool_confirmation: bool,
    pub any_tool_needs_permission: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolConfirmationGatePlan {
    SkipByPolicy,
    SkipNoPermissionedTool,
    AwaitPermissionedTool,
}

impl ToolConfirmationGatePlan {
    pub const fn confirm_before_run(self) -> bool {
        matches!(self, Self::AwaitPermissionedTool)
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolConfirmationResponse {
    Confirmed,
    Rejected(String),
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
    pub rejection_instruction: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ToolConfirmationChannelStore {
    channels: Arc<DashMap<String, oneshot::Sender<ToolConfirmationResponse>>>,
}

impl ToolConfirmationChannelStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, tool_id: String) -> oneshot::Receiver<ToolConfirmationResponse> {
        let (sender, receiver) = oneshot::channel();
        self.channels.insert(tool_id, sender);
        receiver
    }

    pub fn confirm(&self, tool_id: &str) -> bool {
        self.send(tool_id, ToolConfirmationResponse::Confirmed)
    }

    pub fn reject(&self, tool_id: &str, reason: String) -> bool {
        self.send(tool_id, ToolConfirmationResponse::Rejected(reason))
    }

    pub fn cancel(&self, tool_id: &str) -> bool {
        self.channels.remove(tool_id).is_some()
    }

    pub fn has_pending(&self, tool_id: &str) -> bool {
        self.channels.contains_key(tool_id)
    }

    fn send(&self, tool_id: &str, response: ToolConfirmationResponse) -> bool {
        let Some((_, sender)) = self.channels.remove(tool_id) else {
            return false;
        };
        let _ = sender.send(response);
        true
    }
}

pub fn resolve_tool_confirmation_gate(
    facts: ToolConfirmationGateFacts,
) -> ToolConfirmationGatePlan {
    if facts.global_skip_tool_confirmation || facts.context_skip_tool_confirmation {
        return ToolConfirmationGatePlan::SkipByPolicy;
    }

    if facts.any_tool_needs_permission {
        ToolConfirmationGatePlan::AwaitPermissionedTool
    } else {
        ToolConfirmationGatePlan::SkipNoPermissionedTool
    }
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
        ToolConfirmationOutcome::Rejected { reason } => {
            let rejection_instruction = normalize_rejection_instruction(&reason);
            Some(ToolConfirmationFailure {
                kind: ConfirmationFailureKind::Rejected,
                state_reason: format!("User rejected: {reason}"),
                error_message: format!("Tool was rejected by user: {reason}"),
                rejection_instruction,
            })
        }
        ToolConfirmationOutcome::ChannelClosed => Some(ToolConfirmationFailure {
            kind: ConfirmationFailureKind::ChannelClosed,
            state_reason: "Confirmation channel closed".to_string(),
            error_message: "Confirmation channel closed".to_string(),
            rejection_instruction: None,
        }),
        ToolConfirmationOutcome::Timeout { tool_name } => Some(ToolConfirmationFailure {
            kind: ConfirmationFailureKind::Timeout,
            state_reason: "Confirmation timeout".to_string(),
            error_message: format!("Confirmation timeout: {tool_name}"),
            rejection_instruction: None,
        }),
    }
}

fn normalize_rejection_instruction(reason: &str) -> Option<String> {
    let trimmed = reason.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("User rejected") {
        None
    } else {
        Some(trimmed.to_string())
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

    #[tokio::test]
    async fn confirmation_channel_store_delivers_confirmation_once() {
        let store = ToolConfirmationChannelStore::new();
        let receiver = store.register("tool-1".to_string());

        assert!(store.has_pending("tool-1"));
        assert!(store.confirm("tool-1"));
        assert!(!store.has_pending("tool-1"));
        assert_eq!(
            receiver.await.expect("confirmation should be delivered"),
            ToolConfirmationResponse::Confirmed
        );
        assert!(!store.confirm("tool-1"));
    }

    #[tokio::test]
    async fn confirmation_channel_store_delivers_rejection_reason() {
        let store = ToolConfirmationChannelStore::new();
        let receiver = store.register("tool-1".to_string());

        assert!(store.reject("tool-1", "unsafe".to_string()));
        assert_eq!(
            receiver.await.expect("rejection should be delivered"),
            ToolConfirmationResponse::Rejected("unsafe".to_string())
        );
    }

    #[tokio::test]
    async fn confirmation_channel_store_cancel_closes_receiver() {
        let store = ToolConfirmationChannelStore::new();
        let receiver = store.register("tool-1".to_string());

        assert!(store.cancel("tool-1"));
        assert!(receiver.await.is_err());
    }
}
