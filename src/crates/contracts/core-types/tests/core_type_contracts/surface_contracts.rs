use bitfun_core_types::surface::{
    ApprovalSource, CapabilityRequest, CapabilityRequestKind, PermissionDecision, PermissionScope,
    RuntimeArtifactKind, RuntimeArtifactRef, SurfaceKind, ThreadEnvironment, ThreadEnvironmentKind,
};
use std::collections::BTreeMap;

#[test]
fn surface_contract_serializes_observational_runtime_facts() {
    let artifact = RuntimeArtifactRef {
        id: "artifact-1".to_string(),
        kind: RuntimeArtifactKind::TerminalSnapshot,
        session_id: Some("session-1".to_string()),
        thread_id: Some("thread-1".to_string()),
        turn_id: Some("turn-1".to_string()),
        producer_surface: Some(SurfaceKind::Cli),
        parent_artifact_id: None,
        metadata: BTreeMap::new(),
    };

    let json = serde_json::to_value(&artifact).expect("serialize artifact");

    assert_eq!(json["kind"], "terminal_snapshot");
    assert_eq!(json["sessionId"], "session-1");
    assert_eq!(json["producerSurface"], "cli");
    assert!(json.get("parentArtifactId").is_none());
}

#[test]
fn permission_and_capability_contracts_keep_source_identity() {
    let request = CapabilityRequest {
        request_id: "cap-1".to_string(),
        kind: CapabilityRequestKind::PermissionDecision,
        source: ApprovalSource {
            surface: SurfaceKind::Remote,
            thread_id: Some("thread-remote".to_string()),
            turn_id: Some("turn-remote".to_string()),
            subagent_thread_id: Some("child-1".to_string()),
        },
        artifact: None,
        permission: Some(PermissionScope {
            tool_id: Some("bash".to_string()),
            command_prefix: Some("git status".to_string()),
            path_pattern: Some("src/**".to_string()),
            agent_role: Some("reviewer".to_string()),
            surface: Some(SurfaceKind::Remote),
            thread_id: Some("thread-remote".to_string()),
        }),
        decision: Some(PermissionDecision::ApproveSession),
        metadata: BTreeMap::new(),
    };

    let json = serde_json::to_value(&request).expect("serialize request");

    assert_eq!(json["kind"], "permission_decision");
    assert_eq!(json["source"]["surface"], "remote");
    assert_eq!(json["source"]["subagentThreadId"], "child-1");
    assert_eq!(json["permission"]["commandPrefix"], "git status");
    assert_eq!(json["decision"], "approve_session");
}

#[test]
fn thread_environment_contract_does_not_require_surface_specific_fields() {
    let env = ThreadEnvironment {
        kind: ThreadEnvironmentKind::RemoteConnect,
        workspace_path: None,
        remote_connection_id: Some("paired-phone".to_string()),
        label: None,
        metadata: BTreeMap::new(),
    };

    let json = serde_json::to_value(&env).expect("serialize environment");

    assert_eq!(json["kind"], "remote_connect");
    assert_eq!(json["remoteConnectionId"], "paired-phone");
    assert!(json.get("workspacePath").is_none());
    assert!(json.get("label").is_none());
}
