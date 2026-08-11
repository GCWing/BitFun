use bitfun_runtime_ports::{
    AgenticDecisionIntent, AgenticDecisionLineage, AgenticDecisionPolicyResult,
    AgenticDecisionPortRequest, AgenticDecisionRouteClass, AgenticDecisionShadowStatus,
    AgenticDecisionSurface, AgenticDecisionTargetKind,
};

fn lineage() -> AgenticDecisionLineage {
    AgenticDecisionLineage {
        decision_id: "decision-1".to_string(),
        core_run_id: "run-1".to_string(),
        task_id: "task-1".to_string(),
        action_id: "action-1".to_string(),
        correlation_id: "corr-1".to_string(),
        session_id: Some("session-1".to_string()),
        turn_id: Some("turn-1".to_string()),
        idempotency_key: Some("idem-1".to_string()),
    }
}

fn request(
    surface: AgenticDecisionSurface,
    target_kind: AgenticDecisionTargetKind,
    route: &str,
) -> AgenticDecisionPortRequest {
    AgenticDecisionPortRequest {
        lineage: lineage(),
        surface,
        target_kind,
        route: route.to_string(),
        intent: AgenticDecisionIntent::Execute,
        policy_result: AgenticDecisionPolicyResult::Allow,
        bitfun_policy_available: true,
        approval_id: None,
        evidence_refs: vec!["contract:test".to_string()],
        execute_physical: false,
    }
}

#[test]
fn desktop_turn_projects_as_shadow_only_governed_runtime() {
    let projection = request(
        AgenticDecisionSurface::Desktop,
        AgenticDecisionTargetKind::SessionTurn,
        "start_dialog_turn",
    )
    .project_shadow();

    assert_eq!(
        projection.route_class,
        AgenticDecisionRouteClass::GovernedRuntime
    );
    assert_eq!(projection.status, AgenticDecisionShadowStatus::ShadowAllowed);
    assert_eq!(projection.denial_reason, None);
    assert!(!projection.execute_physical);
}

#[test]
fn direct_tool_route_fails_closed_without_physical_execution() {
    let projection = request(
        AgenticDecisionSurface::ToolApi,
        AgenticDecisionTargetKind::ToolCall,
        "execute_tool",
    )
    .project_shadow();

    assert_eq!(projection.route_class, AgenticDecisionRouteClass::DirectGap);
    assert_eq!(projection.status, AgenticDecisionShadowStatus::ShadowDenied);
    assert_eq!(
        projection.denial_reason.as_deref(),
        Some("route_gap:execute_tool")
    );
    assert!(!projection.execute_physical);
}

#[test]
fn mcp_app_tool_call_fails_closed_but_resource_read_is_not_tool_execution() {
    let tool_projection = request(
        AgenticDecisionSurface::McpApp,
        AgenticDecisionTargetKind::McpCall,
        "send_mcp_app_message.tools_call",
    )
    .project_shadow();

    assert_eq!(
        tool_projection.route_class,
        AgenticDecisionRouteClass::DirectGap
    );
    assert_eq!(
        tool_projection.denial_reason.as_deref(),
        Some("route_gap:send_mcp_app_message.tools_call")
    );

    let read_projection = request(
        AgenticDecisionSurface::McpApp,
        AgenticDecisionTargetKind::ReadOnly,
        "send_mcp_app_message.resources_read",
    )
    .project_shadow();

    assert_eq!(read_projection.route_class, AgenticDecisionRouteClass::ReadOnly);
    assert_eq!(read_projection.status, AgenticDecisionShadowStatus::ShadowAllowed);
}

#[test]
fn shadow_mode_denies_physical_execution_even_on_governed_routes() {
    let mut request = request(
        AgenticDecisionSurface::Cron,
        AgenticDecisionTargetKind::SessionTurn,
        "scheduled_job_turn",
    );
    request.execute_physical = true;

    let projection = request.project_shadow();

    assert_eq!(projection.status, AgenticDecisionShadowStatus::ShadowDenied);
    assert_eq!(
        projection.denial_reason.as_deref(),
        Some("shadow_mode_forbids_physical_execution")
    );
    assert!(!projection.execute_physical);
}

#[test]
fn session_bound_projection_requires_session_and_turn_lineage() {
    let mut request = request(
        AgenticDecisionSurface::AppServer,
        AgenticDecisionTargetKind::SessionTurn,
        "submit_dialog_turn",
    );
    request.lineage.turn_id = None;

    let projection = request.project_shadow();

    assert_eq!(projection.status, AgenticDecisionShadowStatus::ShadowDenied);
    assert_eq!(
        projection.denial_reason.as_deref(),
        Some("missing_session_turn_lineage")
    );
}
