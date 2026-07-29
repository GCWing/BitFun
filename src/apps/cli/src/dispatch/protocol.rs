use serde::{Deserialize, Serialize};

pub(crate) const DISPATCH_PROTOCOL_VERSION: u32 = 1;

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
    pub(crate) pending_permissions: Vec<serde_json::Value>,
    pub(crate) cursor_reset: bool,
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
