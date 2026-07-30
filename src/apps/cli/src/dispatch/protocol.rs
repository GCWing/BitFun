use serde::{Deserialize, Serialize};

use bitfun_agent_runtime::sdk::{PermissionReply, PermissionRequest};
use bitfun_services_core::dispatch_workspace::{WorkspaceResultSummary, WorkspaceSnapshotMetadata};

pub(crate) const DISPATCH_PROTOCOL_VERSION: u32 = 2;
pub(crate) const MAX_DISPATCH_TEXT_BYTES: usize = 32 * 1024;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DispatchProbeRequest {
    #[serde(default)]
    pub(crate) workspace_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DispatchWorkspaceProbe {
    pub(crate) path: String,
    pub(crate) exists: bool,
    pub(crate) is_directory: bool,
    pub(crate) is_git_repository: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dirty: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ahead: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) behind: Option<u64>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DispatchProbeResponse {
    pub(crate) protocol_version: u32,
    pub(crate) cli_version: String,
    pub(crate) os: String,
    pub(crate) arch: String,
    pub(crate) capabilities: Vec<String>,
    pub(crate) model_configured: bool,
    pub(crate) available_models: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) default_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) model_diagnostic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) workspace: Option<DispatchWorkspaceProbe>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DispatchApprovalPolicy {
    Auto,
    RejectAndReport,
    Remote,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DispatchSubmitRequest {
    pub(crate) protocol_version: u32,
    pub(crate) job_id: String,
    pub(crate) session_id: String,
    pub(crate) workspace_path: String,
    pub(crate) agent_type: String,
    pub(crate) prompt: String,
    pub(crate) approval_policy: DispatchApprovalPolicy,
    #[serde(default)]
    pub(crate) model: Option<String>,
    #[serde(default)]
    pub(crate) title: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DispatchJobState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl DispatchJobState {
    pub(crate) const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DispatchSubmitResponse {
    pub(crate) accepted: bool,
    pub(crate) job_id: String,
    pub(crate) session_id: String,
    pub(crate) state: DispatchJobState,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DispatchStatusRequest {
    pub(crate) job_id: String,
    #[serde(default)]
    pub(crate) cursor: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DispatchAnswerRequest {
    pub(crate) job_id: String,
    pub(crate) request_id: String,
    #[serde(flatten)]
    pub(crate) reply: PermissionReply,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DispatchAnswerResponse {
    pub(crate) resolved: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DispatchAppendRequest {
    pub(crate) job_id: String,
    pub(crate) message_id: String,
    pub(crate) content: String,
    #[serde(default)]
    pub(crate) display_content: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DispatchAppendResponse {
    pub(crate) accepted: bool,
    pub(crate) message_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DispatchWorkspaceBeginRequest {
    pub(crate) protocol_version: u32,
    pub(crate) job_id: String,
    pub(crate) metadata: WorkspaceSnapshotMetadata,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DispatchWorkspaceBeginResponse {
    pub(crate) accepted: bool,
    pub(crate) offset: u64,
    pub(crate) upload_path: String,
    pub(crate) committed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) workspace_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DispatchWorkspaceChunkRequest {
    pub(crate) job_id: String,
    pub(crate) offset: u64,
    pub(crate) data_base64: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DispatchWorkspaceChunkResponse {
    pub(crate) accepted: bool,
    pub(crate) offset: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DispatchWorkspaceCommitRequest {
    pub(crate) job_id: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DispatchWorkspaceCommitResponse {
    pub(crate) committed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) workspace_path: Option<String>,
    pub(crate) metadata: WorkspaceSnapshotMetadata,
}

/// Ask the target to diff its terminal tree against the delivered snapshot.
///
/// Read-only on the target: it builds a bundle and reports what changed. The
/// controller decides whether to fetch it, and applying it locally is a
/// separate step the user confirms.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DispatchWorkspaceResultRequest {
    pub(crate) job_id: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DispatchWorkspaceResultResponse {
    /// Absolute path of the bundle on the target, for the controller to fetch.
    pub(crate) bundle_path: String,
    pub(crate) workspace_path: String,
    pub(crate) summary: WorkspaceResultSummary,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DispatchCancelRequest {
    pub(crate) job_id: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DispatchListRequest {}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum DispatchEvent {
    Audit {
        timestamp: String,
        action: String,
        details: serde_json::Value,
    },
    JobState {
        timestamp: String,
        state: DispatchJobState,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    AgentEvent {
        timestamp: String,
        event: serde_json::Value,
        #[serde(rename = "frontendEventName", skip_serializing_if = "Option::is_none")]
        frontend_event_name: Option<String>,
        #[serde(rename = "frontendPayload", skip_serializing_if = "Option::is_none")]
        frontend_payload: Option<serde_json::Value>,
    },
    PermissionRejected {
        timestamp: String,
        request: serde_json::Value,
        reason: String,
    },
}

impl DispatchEvent {
    pub(crate) fn approval_policy_selected(policy: DispatchApprovalPolicy) -> Self {
        Self::Audit {
            timestamp: chrono::Utc::now().to_rfc3339(),
            action: "approvalPolicySelected".to_string(),
            details: serde_json::json!({ "approvalPolicy": policy }),
        }
    }

    pub(crate) fn cancel_requested() -> Self {
        Self::Audit {
            timestamp: chrono::Utc::now().to_rfc3339(),
            action: "cancelRequested".to_string(),
            details: serde_json::json!({}),
        }
    }

    pub(crate) fn permission_pending(request_id: &str) -> Self {
        Self::Audit {
            timestamp: chrono::Utc::now().to_rfc3339(),
            action: "permissionPending".to_string(),
            details: serde_json::json!({ "requestId": request_id }),
        }
    }

    pub(crate) fn permission_resolved(request_id: &str) -> Self {
        Self::Audit {
            timestamp: chrono::Utc::now().to_rfc3339(),
            action: "permissionResolved".to_string(),
            details: serde_json::json!({ "requestId": request_id }),
        }
    }

    pub(crate) fn message_appended(message_id: &str) -> Self {
        Self::Audit {
            timestamp: chrono::Utc::now().to_rfc3339(),
            action: "messageAppended".to_string(),
            details: serde_json::json!({ "messageId": message_id }),
        }
    }

    pub(crate) fn oversized_event_omitted(encoded_bytes: usize, max_bytes: usize) -> Self {
        Self::Audit {
            timestamp: chrono::Utc::now().to_rfc3339(),
            action: "eventOmitted".to_string(),
            details: serde_json::json!({
                "reason": "eventTooLarge",
                "encodedBytes": encoded_bytes,
                "maxBytes": max_bytes,
            }),
        }
    }

    pub(crate) fn job_state(state: DispatchJobState, message: impl Into<Option<String>>) -> Self {
        Self::JobState {
            timestamp: chrono::Utc::now().to_rfc3339(),
            state,
            message: message.into(),
        }
    }

    pub(crate) fn agent_event(
        event: serde_json::Value,
        frontend_projection: Option<(String, serde_json::Value)>,
    ) -> Self {
        let (frontend_event_name, frontend_payload) = frontend_projection
            .map(|(name, payload)| (Some(name), Some(payload)))
            .unwrap_or((None, None));
        Self::AgentEvent {
            timestamp: chrono::Utc::now().to_rfc3339(),
            event,
            frontend_event_name,
            frontend_payload,
        }
    }

    pub(crate) fn permission_rejected(
        request: serde_json::Value,
        reason: impl Into<String>,
    ) -> Self {
        Self::PermissionRejected {
            timestamp: chrono::Utc::now().to_rfc3339(),
            request,
            reason: reason.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DispatchStatusResponse {
    pub(crate) state: DispatchJobState,
    pub(crate) cursor: u64,
    pub(crate) events: Vec<DispatchEvent>,
    pub(crate) pending_permissions: Vec<PermissionRequest>,
    pub(crate) cursor_reset: bool,
    pub(crate) history_truncated: bool,
    pub(crate) event_log_complete: bool,
    pub(crate) omitted_event_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_error: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DispatchCancelResponse {
    pub(crate) cancelled: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DispatchJobListEntry {
    pub(crate) job_id: String,
    pub(crate) session_id: String,
    pub(crate) state: DispatchJobState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) started_at: Option<String>,
    pub(crate) workspace_path: String,
    pub(crate) title: String,
    pub(crate) agent_type: String,
    pub(crate) approval_policy: DispatchApprovalPolicy,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) model: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_names_are_camel_case_and_policy_values_are_explicit() {
        let event = DispatchEvent::AgentEvent {
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            event: serde_json::json!({"id": "event-1"}),
            frontend_event_name: Some("agentic://text-chunk".to_string()),
            frontend_payload: Some(serde_json::json!({"sessionId": "session-1"})),
        };
        let value = serde_json::to_value(event).expect("serialize event");
        assert_eq!(value["type"], "agentEvent");
        assert_eq!(value["frontendEventName"], "agentic://text-chunk");
        assert_eq!(value["frontendPayload"]["sessionId"], "session-1");

        assert_eq!(
            serde_json::to_value(DispatchApprovalPolicy::RejectAndReport)
                .expect("serialize policy"),
            "reject-and-report"
        );
        assert_eq!(
            serde_json::to_value(DispatchJobState::Running).expect("serialize state"),
            "running"
        );
    }
}
