use bitfun_agent_runtime::sdk::PermissionRequest;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CliApprovalPolicy {
    /// Inherit the persisted user interaction preference.
    Ask,
    /// Explicitly disable Auto mode for this invocation/session.
    DisableAuto,
    Reject,
    Auto,
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
    use super::permission_request_targets_session;
    use bitfun_agent_runtime::sdk::{
        PermissionDelegationContext, PermissionRequest, PermissionRequestSource,
        PermissionRequestSourceKind,
    };
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
}
