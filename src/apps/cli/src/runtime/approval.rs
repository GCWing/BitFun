use bitfun_agent_runtime::sdk::{PermissionRequest, AUTO_APPROVE_ASK_CONTEXT_KEY};
use bitfun_agent_runtime::user_questions::USER_INPUT_AVAILABLE_CONTEXT_KEY;
use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CliApprovalPolicy {
    /// Inherit the persisted user interaction preference.
    Ask,
    /// Explicitly disable Auto mode for this invocation/session.
    DisableAuto,
    Reject,
    Auto,
}

/// Build invocation-scoped approval metadata consumed by the shared Runtime.
///
/// Headless entrypoints must use this helper instead of mutating persisted
/// confirmation settings or defining a parallel permission mechanism.
pub(crate) fn approval_metadata(approval_policy: CliApprovalPolicy) -> Map<String, Value> {
    let mut metadata = Map::new();
    if matches!(
        approval_policy,
        CliApprovalPolicy::Reject | CliApprovalPolicy::Auto
    ) {
        metadata.insert(
            USER_INPUT_AVAILABLE_CONTEXT_KEY.to_string(),
            Value::Bool(false),
        );
    }
    let auto_approve_ask = match approval_policy {
        CliApprovalPolicy::Ask => None,
        CliApprovalPolicy::DisableAuto | CliApprovalPolicy::Reject => Some(false),
        CliApprovalPolicy::Auto => Some(true),
    };
    if let Some(auto_approve_ask) = auto_approve_ask {
        metadata.insert(
            AUTO_APPROVE_ASK_CONTEXT_KEY.to_string(),
            Value::Bool(auto_approve_ask),
        );
    }
    metadata
}

pub(crate) fn permission_request_targets_session(
    request: &PermissionRequest,
    session_id: &str,
) -> bool {
    request.session_id == session_id
        || request
            .delegation
            .as_ref()
            .is_some_and(|delegation| delegation.parent_session_id == session_id)
}

#[cfg(test)]
mod tests {
    use super::{approval_metadata, permission_request_targets_session, CliApprovalPolicy};
    use bitfun_agent_runtime::sdk::{
        PermissionDelegationContext, PermissionRequest, PermissionRequestSource,
        PermissionRequestSourceKind, AUTO_APPROVE_ASK_CONTEXT_KEY,
    };
    use bitfun_agent_runtime::user_questions::USER_INPUT_AVAILABLE_CONTEXT_KEY;
    use serde_json::Map;

    fn request() -> PermissionRequest {
        PermissionRequest {
            request_id: "request-1".to_string(),
            round_id: "synthetic:request-1".to_string(),
            order: 0,
            tool_call_id: Some("child-tool".to_string()),
            project_path: None,
            project_id: "project-1".to_string(),
            session_id: "child-session".to_string(),
            agent_id: "Explore".to_string(),
            action: "edit".to_string(),
            resources: vec!["src/main.rs".to_string()],
            save_resources: Vec::new(),
            source: PermissionRequestSource {
                kind: PermissionRequestSourceKind::ToolCall,
                identity: "Write".to_string(),
            },
            delegation: Some(PermissionDelegationContext {
                parent_session_id: "parent-session".to_string(),
                parent_dialog_turn_id: Some("parent-turn".to_string()),
                parent_tool_call_id: "parent-task".to_string(),
                subagent_type: "Explore".to_string(),
            }),
            display_metadata: Map::new(),
        }
    }

    #[test]
    fn permission_requests_target_their_execution_and_parent_interaction_sessions() {
        let request = request();

        assert!(permission_request_targets_session(
            &request,
            "child-session"
        ));
        assert!(permission_request_targets_session(
            &request,
            "parent-session"
        ));
        assert!(!permission_request_targets_session(
            &request,
            "unrelated-session"
        ));
    }

    #[test]
    fn headless_approval_metadata_is_invocation_scoped() {
        let auto = approval_metadata(CliApprovalPolicy::Auto);
        assert_eq!(
            auto.get(USER_INPUT_AVAILABLE_CONTEXT_KEY),
            Some(&serde_json::Value::Bool(false))
        );
        assert_eq!(
            auto.get(AUTO_APPROVE_ASK_CONTEXT_KEY),
            Some(&serde_json::Value::Bool(true))
        );

        let reject = approval_metadata(CliApprovalPolicy::Reject);
        assert_eq!(
            reject.get(USER_INPUT_AVAILABLE_CONTEXT_KEY),
            Some(&serde_json::Value::Bool(false))
        );
        assert_eq!(
            reject.get(AUTO_APPROVE_ASK_CONTEXT_KEY),
            Some(&serde_json::Value::Bool(false))
        );

        assert!(approval_metadata(CliApprovalPolicy::Ask).is_empty());
        assert_eq!(
            approval_metadata(CliApprovalPolicy::DisableAuto).get(AUTO_APPROVE_ASK_CONTEXT_KEY),
            Some(&serde_json::Value::Bool(false))
        );
    }
}
