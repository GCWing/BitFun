use crate::external_mcp::{
    prepared_mcp_config, reconcile_external_mcp_catalog, ActiveExternalMcpCandidate,
    ExternalMcpDecision, ExternalMcpDecisions, NativeMcpCandidate,
};
use bitfun_external_sources::ExternalMcpCoordinatorSnapshot;
use bitfun_product_domains::external_sources::{
    external_mcp_approval_key, external_mcp_conflict_key, EcosystemId, ExecutionDomainId,
    ExternalMcpActivationState, ExternalMcpServerDefinition, ExternalMcpStaticStatus,
    ExternalMcpTransportKind, ExternalSourceCatalogEntry, ExternalSourceHealth,
    ExternalSourceLifecycleState, ExternalSourceRecord, ExternalSourceScope,
    PreparedExternalMcpServer, PreparedExternalMcpTransport, SecretValue, SourceKey,
    SourceQualifiedMcpServerId,
};
use std::collections::{BTreeMap, BTreeSet};

fn active_ecosystems() -> &'static BTreeSet<EcosystemId> {
    static ECOSYSTEMS: std::sync::OnceLock<BTreeSet<EcosystemId>> = std::sync::OnceLock::new();
    ECOSYSTEMS.get_or_init(|| {
        BTreeSet::from([EcosystemId::new("opencode").expect("valid test ecosystem")])
    })
}

#[test]
fn unavailable_external_mcp_can_be_disabled_but_not_silently_reapproved() {
    let state = ExternalMcpActivationState::RuntimeUnavailable {
        reason: "failed".to_string(),
    };
    assert!(crate::external_sources::external_mcp_decision_allowed(
        &state, false
    ));
    assert!(!crate::external_sources::external_mcp_decision_allowed(
        &state, true
    ));
}

fn snapshot(behavior_version: &str) -> ExternalMcpCoordinatorSnapshot {
    let source = ExternalSourceRecord {
        key: SourceKey::new("opencode.mcp", "project").unwrap(),
        ecosystem_id: EcosystemId::new("opencode").unwrap(),
        display_name: "OpenCode project configuration".to_string(),
        source_kind: "opencode_mcp_config".to_string(),
        scope: ExternalSourceScope::Project,
        location: "/workspace/opencode.json".to_string(),
        execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
        health: ExternalSourceHealth::Available,
        content_version: "source-v1".to_string(),
        diagnostics: Vec::new(),
    };
    let definition = ExternalMcpServerDefinition {
        id: SourceQualifiedMcpServerId::new(source.key.clone(), "github").unwrap(),
        provenance: vec![source.key.clone()],
        name: "github".to_string(),
        transport: ExternalMcpTransportKind::LocalStdio,
        command_preview: Some("npx".to_string()),
        argument_count: 1,
        working_directory: Some("<workspace>".to_string()),
        environment_keys: vec!["GITHUB_TOKEN".to_string()],
        environment_reference_names: Vec::new(),
        remote_url_preview: None,
        header_names: Vec::new(),
        source_enabled: true,
        behavior_version: behavior_version.to_string(),
        static_status: ExternalMcpStaticStatus::Ready,
    };
    ExternalMcpCoordinatorSnapshot {
        generation: 1,
        discovery_pending: false,
        sources: vec![ExternalSourceCatalogEntry {
            stable_key: source.preference_key(),
            presentation_group_id: None,
            record: source,
            lifecycle: ExternalSourceLifecycleState::Available,
        }],
        servers: vec![definition],
        diagnostics: Vec::new(),
    }
}

fn decisions<'a>(
    server_decisions: &'a BTreeMap<String, ExternalMcpDecision>,
    conflict_choices: &'a BTreeMap<String, String>,
) -> ExternalMcpDecisions<'a> {
    ExternalMcpDecisions {
        active_ecosystems: active_ecosystems(),
        server_decisions,
        conflict_choices,
    }
}

#[test]
fn approval_is_once_per_behavior_and_a_changed_behavior_fails_closed() {
    let snapshot_v1 = snapshot("behavior-v1");
    let definition = &snapshot_v1.servers[0];
    let approval_v1 = external_mcp_approval_key(
        "local-user",
        "/workspace-a",
        &definition.id,
        &definition.behavior_version,
    );
    let mut server_decisions = BTreeMap::new();
    let conflict_choices = BTreeMap::new();

    let pending = reconcile_external_mcp_catalog(
        "local-user",
        "/workspace-a",
        &snapshot_v1,
        &[],
        decisions(&server_decisions, &conflict_choices),
    );
    assert_eq!(pending.approval_requests.len(), 1);
    assert_eq!(
        pending.entries[0].activation_state,
        ExternalMcpActivationState::ApprovalRequired
    );
    assert!(pending.active.is_empty());

    server_decisions.insert(
        approval_v1.clone(),
        ExternalMcpDecision {
            decision_key: approval_v1,
            approved: true,
        },
    );
    let active = reconcile_external_mcp_catalog(
        "local-user",
        "/workspace-a",
        &snapshot_v1,
        &[],
        decisions(&server_decisions, &conflict_choices),
    );
    assert_eq!(
        active.entries[0].activation_state,
        ExternalMcpActivationState::Active
    );
    assert_eq!(active.active.len(), 1);
    assert!(active.entries[0].runtime_id.is_some());
    let other_workspace = reconcile_external_mcp_catalog(
        "local-user",
        "/workspace-b",
        &snapshot_v1,
        &[],
        decisions(&server_decisions, &conflict_choices),
    );
    assert_ne!(
        active.entries[0].runtime_id, other_workspace.entries[0].runtime_id,
        "runtime instances must be workspace scoped"
    );
    assert_eq!(
        other_workspace.entries[0].activation_state,
        ExternalMcpActivationState::ApprovalRequired,
        "approval in one workspace must not authorize another workspace"
    );
    assert!(other_workspace.active.is_empty());

    let changed = reconcile_external_mcp_catalog(
        "local-user",
        "/workspace-a",
        &snapshot("behavior-v2"),
        &[],
        decisions(&server_decisions, &conflict_choices),
    );
    assert_eq!(
        changed.entries[0].activation_state,
        ExternalMcpActivationState::ConfigurationChanged
    );
    assert_eq!(changed.approval_requests.len(), 1);
    assert!(changed.active.is_empty());
}

#[test]
fn decline_is_not_reasked_until_behavior_changes() {
    let snapshot = snapshot("behavior-v1");
    let definition = &snapshot.servers[0];
    let decision_key = external_mcp_approval_key(
        "local-user",
        "/workspace-a",
        &definition.id,
        &definition.behavior_version,
    );
    let server_decisions = [(
        decision_key.clone(),
        ExternalMcpDecision {
            decision_key,
            approved: false,
        },
    )]
    .into_iter()
    .collect();
    let conflict_choices = BTreeMap::new();

    let state = reconcile_external_mcp_catalog(
        "local-user",
        "/workspace-a",
        &snapshot,
        &[],
        decisions(&server_decisions, &conflict_choices),
    );
    assert_eq!(
        state.entries[0].activation_state,
        ExternalMcpActivationState::Declined
    );
    assert!(state.approval_requests.is_empty());
}

#[test]
fn native_conflict_stays_active_by_default_but_is_not_recorded_as_a_choice() {
    let snapshot_v1 = snapshot("behavior-v1");
    let external = &snapshot_v1.servers[0];
    let native = NativeMcpCandidate {
        candidate_id: "native_mcp:bitfun-github".to_string(),
        server_id: "bitfun-github".to_string(),
        name: "github".to_string(),
        display_name: "BitFun: github".to_string(),
        behavior_version: "native-v1".to_string(),
        enabled: true,
    };
    let mut decisions_by_server = BTreeMap::new();
    let mut conflict_choices = BTreeMap::new();
    let pending = reconcile_external_mcp_catalog(
        "local-user",
        "/workspace-a",
        &snapshot_v1,
        std::slice::from_ref(&native),
        decisions(&decisions_by_server, &conflict_choices),
    );
    assert_eq!(
        pending.entries[0].activation_state,
        ExternalMcpActivationState::Conflict
    );
    assert_eq!(
        pending.conflicts[0].candidates[0].candidate_id,
        native.candidate_id
    );
    assert!(pending.conflicts[0].selected_candidate_id.is_none());
    assert!(pending.approval_requests.is_empty());
    assert!(pending.suppressed_native_server_ids.is_empty());

    let external_id = external.candidate_id();
    let conflict_key = external_mcp_conflict_key(
        "local-user",
        "/workspace-a",
        "github",
        [
            (
                native.candidate_id.as_str(),
                native.behavior_version.as_str(),
            ),
            (external_id.as_str(), external.behavior_version.as_str()),
        ],
    );
    conflict_choices.insert(conflict_key, external_id.clone());
    let external_approval_key = external_mcp_approval_key(
        "local-user",
        "/workspace-a",
        &external.id,
        &external.behavior_version,
    );
    decisions_by_server.insert(
        external_approval_key.clone(),
        ExternalMcpDecision {
            decision_key: external_approval_key,
            approved: true,
        },
    );
    let selected = reconcile_external_mcp_catalog(
        "local-user",
        "/workspace-a",
        &snapshot_v1,
        std::slice::from_ref(&native),
        decisions(&decisions_by_server, &conflict_choices),
    );
    assert_eq!(
        selected.entries[0].activation_state,
        ExternalMcpActivationState::Active
    );
    assert_eq!(selected.active.len(), 1);
    assert_eq!(
        selected.suppressed_native_server_ids,
        ["bitfun-github".to_string()].into_iter().collect()
    );

    let other_workspace = reconcile_external_mcp_catalog(
        "local-user",
        "/workspace-b",
        &snapshot_v1,
        std::slice::from_ref(&native),
        decisions(&decisions_by_server, &conflict_choices),
    );
    assert!(other_workspace.conflicts[0].selected_candidate_id.is_none());
    assert!(other_workspace.suppressed_native_server_ids.is_empty());

    let changed = reconcile_external_mcp_catalog(
        "local-user",
        "/workspace-a",
        &snapshot("behavior-v2"),
        std::slice::from_ref(&native),
        decisions(&decisions_by_server, &conflict_choices),
    );
    assert!(changed.conflicts[0].selected_candidate_id.is_none());
    assert_eq!(
        changed.suppressed_native_server_ids,
        ["bitfun-github".to_string()].into_iter().collect(),
        "a changed external selection must not silently fall back to the native server"
    );

    let removed_external = ExternalMcpCoordinatorSnapshot {
        generation: 2,
        discovery_pending: false,
        sources: Vec::new(),
        servers: Vec::new(),
        diagnostics: Vec::new(),
    };
    let removed = reconcile_external_mcp_catalog(
        "local-user",
        "/workspace-a",
        &removed_external,
        std::slice::from_ref(&native),
        decisions(&decisions_by_server, &conflict_choices),
    );
    assert_eq!(removed.conflicts.len(), 1);
    assert_eq!(removed.conflicts[0].candidates.len(), 1);
    assert!(removed.conflicts[0].selected_candidate_id.is_none());
    assert_eq!(
        removed.suppressed_native_server_ids,
        ["bitfun-github".to_string()].into_iter().collect(),
        "deleting the selected external server must require an explicit native re-selection"
    );
}

#[test]
fn prepared_local_external_mcp_keeps_cwd_and_does_not_inherit_bitfun_secrets() {
    let mut source_snapshot = snapshot("behavior-v1");
    let definition = source_snapshot.servers.remove(0);
    let candidate = ActiveExternalMcpCandidate {
        runtime_id: "external-mcp-runtime".to_string(),
        definition: definition.clone(),
    };
    let prepared = PreparedExternalMcpServer {
        id: definition.id,
        behavior_version: definition.behavior_version,
        transport: PreparedExternalMcpTransport::Local {
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "example-mcp".to_string()],
            environment: [("EXPLICIT_TOKEN".to_string(), SecretValue::new("secret"))]
                .into_iter()
                .collect(),
            working_directory: Some("D:/workspace/project".into()),
        },
    };

    let config = prepared_mcp_config(&candidate, prepared).expect("valid runtime config");

    assert_eq!(
        config.working_directory.as_deref(),
        Some("D:/workspace/project")
    );
    assert_eq!(config.inherit_parent_environment, Some(false));
    assert_eq!(
        config.env.get("EXPLICIT_TOKEN").map(String::as_str),
        Some("secret")
    );
}
