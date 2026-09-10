use openbitfun_product_domains::legacy_migration::{
    MigrationDomainId, MigrationDomainResult, MigrationGroupId, MigrationOnboardingState,
    MigrationPromptChoice, MigrationSelection, MigratorHandoffRequest,
    MigratorProtocolCapabilities, MigratorProtocolCapability, MigratorRequestMode,
    CURRENT_MIGRATOR_PROTOCOL_VERSION,
};
use std::collections::BTreeSet;

#[test]
fn session_group_always_expands_coordination_database_dependency() {
    let selection = MigrationSelection {
        groups: BTreeSet::from([MigrationGroupId::WorkspacesSessionsAndTasks]),
    };

    assert_eq!(
        selection.expanded_domains(),
        vec![
            MigrationDomainId::WorkspaceSessions,
            MigrationDomainId::AgentCoordination,
            MigrationDomainId::CrossReferenceRepair,
        ]
    );
}

#[test]
fn persisted_onboarding_shape_accepts_old_payloads() {
    let state: MigrationOnboardingState =
        serde_json::from_str(r#"{"choice":"remind_later"}"#).expect("additive fields must default");

    assert_eq!(state.choice, MigrationPromptChoice::RemindLater);
    assert_eq!(state.format_version, 0);
    assert!(state.run_id.is_none());
    assert!(state.last_scanned_at_ms.is_none());
    assert!(state.last_report_run_id.is_none());
}

#[test]
fn persisted_domain_result_defaults_new_repair_lists() {
    let result: MigrationDomainResult = serde_json::from_value(serde_json::json!({
        "domain": "credentials",
        "state": "staged",
        "imported": 1
    }))
    .expect("new repair lists must remain additive");

    assert!(result.requires_reauthentication.is_empty());
    assert!(result.requires_relocation.is_empty());
}

#[test]
fn handoff_rejects_expired_or_future_protocol_requests() {
    let request: MigratorHandoffRequest = serde_json::from_value(serde_json::json!({
        "protocolVersion": CURRENT_MIGRATOR_PROTOCOL_VERSION,
        "mode": "onboarding",
        "runId": "run-1",
        "nonce": "nonce-1",
        "createdAtMs": 10,
        "expiresAtMs": 20
    }))
    .expect("request should accept additive defaults");

    let capabilities = MigratorProtocolCapabilities::current();
    assert!(capabilities.accepts_request(&request));
    assert!(!capabilities.accepts(
        CURRENT_MIGRATOR_PROTOCOL_VERSION + 1,
        MigratorRequestMode::Onboarding
    ));
    assert!(request.is_expired_at(21));
}

#[test]
fn handoff_capability_negotiation_is_additive_and_fail_closed() {
    let old_request: MigratorHandoffRequest = serde_json::from_value(serde_json::json!({
        "protocolVersion": CURRENT_MIGRATOR_PROTOCOL_VERSION,
        "mode": "execute"
    }))
    .expect("older requests must default the additive capability set");
    assert!(old_request.required_capabilities.is_empty());
    assert!(MigratorProtocolCapabilities::current().accepts_request(&old_request));

    let mut required = BTreeSet::new();
    required.insert(MigratorProtocolCapability::JournalRecovery);
    let request = MigratorHandoffRequest {
        required_capabilities: required,
        protocol_version: CURRENT_MIGRATOR_PROTOCOL_VERSION,
        mode: MigratorRequestMode::Execute,
        ..MigratorHandoffRequest::default()
    };
    assert!(MigratorProtocolCapabilities::current().accepts_request(&request));

    let mut limited = MigratorProtocolCapabilities::current();
    limited.capabilities.clear();
    assert!(!limited.accepts_request(&request));
}
