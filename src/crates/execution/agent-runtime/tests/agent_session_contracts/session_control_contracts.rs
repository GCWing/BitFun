use bitfun_agent_runtime::session_control::{
    render_session_control_tool_use_message, resolve_session_control_cancel_route,
    session_control_cancel_result_message, session_control_created_result_message,
    validate_session_control_input, SessionControlAction, SessionControlAgentType,
    SessionControlCancelRoute, SessionControlInput, SessionControlValidationContext,
};
use serde_json::json;

fn base_input(action: SessionControlAction) -> SessionControlInput {
    SessionControlInput {
        action,
        workspace: None,
        session_id: None,
        session_name: None,
        agent_type: None,
    }
}

#[test]
fn validates_cancel_without_workspace_and_ignores_workspace_shape() {
    let mut input = base_input(SessionControlAction::Cancel);
    input.session_id = Some("worker_1".to_string());
    input.workspace = Some("not-an-absolute-path".to_string());

    let result = validate_session_control_input(&input, SessionControlValidationContext::default());

    assert!(result.result, "{:?}", result.message);
}

#[test]
fn rejects_current_session_mutation_when_context_matches() {
    let mut input = base_input(SessionControlAction::Delete);
    input.session_id = Some("session_a".to_string());

    let result = validate_session_control_input(
        &input,
        SessionControlValidationContext {
            current_session_id: Some("session_a"),
            has_workspace_root: true,
        },
    );

    assert!(!result.result);
    assert_eq!(
        result.message.as_deref(),
        Some("cannot delete the current session from SessionControl")
    );
}

#[test]
fn validates_create_requires_workspace_and_creator_session() {
    let mut input = base_input(SessionControlAction::Create);
    input.workspace = Some(std::env::temp_dir().to_string_lossy().to_string());
    input.agent_type = Some(SessionControlAgentType::Plan);

    let missing_creator =
        validate_session_control_input(&input, SessionControlValidationContext::default());

    assert!(!missing_creator.result);
    assert_eq!(
        missing_creator.message.as_deref(),
        Some("create requires a creator session in tool context")
    );

    let with_creator = validate_session_control_input(
        &input,
        SessionControlValidationContext {
            current_session_id: Some("session_a"),
            has_workspace_root: true,
        },
    );

    assert!(with_creator.result, "{:?}", with_creator.message);
}

#[test]
fn renders_tool_message_without_core_context() {
    assert_eq!(
        render_session_control_tool_use_message(&json!({
            "action": "cancel",
            "workspace": "/repo",
            "session_id": "worker_1",
        })),
        "Cancel active turn for session worker_1"
    );
}

#[test]
fn builds_stable_result_messages() {
    let workspace = std::env::temp_dir().to_string_lossy().to_string();
    assert_eq!(
        session_control_created_result_message("session_a", &workspace, "Plan"),
        format!("Created session 'session_a' in workspace '{workspace}' using agent type 'Plan'.")
    );
    assert_eq!(
        session_control_cancel_result_message("worker_1", &workspace, Some("turn_1")),
        format!("Cancellation requested for the active turn 'turn_1' in session 'worker_1' within workspace '{workspace}'. The session remains available for future work, and queued messages are not cleared.")
    );
}

#[test]
fn routes_cancel_through_scheduler_only_when_requester_and_scheduler_exist() {
    assert_eq!(
        resolve_session_control_cancel_route(Some("requester"), true),
        SessionControlCancelRoute::RequesterViaScheduler {
            requester_session_id: "requester".to_string()
        }
    );
    assert_eq!(
        resolve_session_control_cancel_route(Some("requester"), false),
        SessionControlCancelRoute::CoordinatorDirect
    );
    assert_eq!(
        resolve_session_control_cancel_route(None, true),
        SessionControlCancelRoute::CoordinatorDirect
    );
}
