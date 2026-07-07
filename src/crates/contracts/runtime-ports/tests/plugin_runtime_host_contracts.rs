use bitfun_runtime_ports::{
    PermissionPromptDenyState, PermissionPromptDescriptor, PermissionPromptEffectKind,
    PluginArtifactRef, PluginAuditRef, PluginCapabilityRef, PluginConfigValidationIssue,
    PluginConfigValidationState, PluginConfigValidationStatus, PluginDiagnostic,
    PluginDiagnosticDetail, PluginDiagnosticSeverity, PluginHostLifecycleEvent,
    PluginHostLifecyclePhase, PluginManifestRef, PluginOwnerKind, PluginOwnerRef,
    PluginQuarantineClearCondition, PluginQuarantineReason, PluginQuarantineScope,
    PluginQuarantineState, PluginRecoveryAction, PluginRecoveryActionKind,
    PluginRecoveryActionRequest, PluginRecoveryActionResult, PluginRecoveryActionStatus,
    PluginRiskLevel, PluginRollbackMode, PluginRollbackPolicy, PluginSourceKind, PluginSourceRef,
    PluginTargetRef, PluginTrustLevel,
};

fn manifest_ref() -> PluginManifestRef {
    PluginManifestRef {
        manifest_id: "manifest-1".to_string(),
        schema_version: "opencode.plugin.v1".to_string(),
        path: Some("opencode.json".to_string()),
    }
}

fn source_ref() -> PluginSourceRef {
    PluginSourceRef {
        plugin_id: "opencode.example".to_string(),
        source_kind: PluginSourceKind::OpenCodeCompatible,
        source: "file:///plugins/opencode-example".to_string(),
        version: Some("1.2.3".to_string()),
        content_hash: "sha256:abc123".to_string(),
        trust_level: PluginTrustLevel::Trusted,
        manifest: Some(manifest_ref()),
    }
}

fn owner_ref() -> PluginOwnerRef {
    PluginOwnerRef {
        kind: PluginOwnerKind::ExtensionContract,
        id: "extension.tool-provider".to_string(),
    }
}

fn capability_ref() -> PluginCapabilityRef {
    PluginCapabilityRef {
        capability_id: "tools.provider".to_string(),
        owner: owner_ref(),
    }
}

fn log_ref() -> PluginArtifactRef {
    PluginArtifactRef {
        artifact_id: "log-1".to_string(),
        artifact_kind: "host_log".to_string(),
        display_name: "Plugin host dispatch log".to_string(),
        uri: Some("bitfun://logs/plugin-host/diag-1".to_string()),
    }
}

fn target_ref() -> PluginTargetRef {
    PluginTargetRef {
        target_kind: "tool_provider".to_string(),
        target_id: "opencode.example.provider".to_string(),
        display_name: "OpenCode example provider".to_string(),
        artifact: Some(log_ref()),
    }
}

fn audit_ref() -> PluginAuditRef {
    PluginAuditRef {
        correlation_id: "corr-1".to_string(),
        event_id: Some("event-1".to_string()),
    }
}

fn retry_action() -> PluginRecoveryAction {
    PluginRecoveryAction {
        action_id: "retry-1".to_string(),
        kind: PluginRecoveryActionKind::Retry,
        target: target_ref(),
        audit: audit_ref(),
        artifact: None,
    }
}

fn quarantine_state() -> PluginQuarantineState {
    PluginQuarantineState {
        schema_version: 1,
        quarantine_id: "quarantine-1".to_string(),
        scope: PluginQuarantineScope::Plugin {
            plugin_id: "opencode.example".to_string(),
        },
        reason: PluginQuarantineReason::DeadlineExceeded,
        source: source_ref(),
        audit: audit_ref(),
        created_at_ms: 1_720_000_001,
        log_ref: Some(log_ref()),
        clears_when: vec![PluginQuarantineClearCondition::HostRestarted],
        recovery_actions: vec![retry_action()],
        diagnostic_ids: vec!["diag-1".to_string()],
    }
}

#[test]
fn permission_prompt_descriptor_contains_minimum_user_decision_facts() {
    let prompt = PermissionPromptDescriptor {
        descriptor_version: 1,
        prompt_id: "prompt-1".to_string(),
        plugin: source_ref(),
        requested_capability: capability_ref(),
        requested_effect: PermissionPromptEffectKind::ProviderCandidate,
        target: target_ref(),
        risk_level: PluginRiskLevel::Medium,
        owner: PluginOwnerRef {
            kind: PluginOwnerKind::ProductFeature,
            id: "tools".to_string(),
        },
        rollback: PluginRollbackPolicy {
            mode: PluginRollbackMode::DisablePlugin,
            reason_ref: Some("audit:event-1".to_string()),
        },
        deny_state: PermissionPromptDenyState::CandidateDiscarded,
        audit: audit_ref(),
    };

    let json = serde_json::to_value(prompt).expect("serialize prompt");

    assert_eq!(json["descriptorVersion"], 1);
    assert_eq!(json["plugin"]["pluginId"], "opencode.example");
    assert_eq!(json["plugin"]["contentHash"], "sha256:abc123");
    assert_eq!(json["plugin"]["manifest"]["path"], "opencode.json");
    assert_eq!(
        json["requestedCapability"]["capabilityId"],
        "tools.provider"
    );
    assert_eq!(json["requestedEffect"], "provider_candidate");
    assert_eq!(json["target"]["targetId"], "opencode.example.provider");
    assert_eq!(json["target"]["displayName"], "OpenCode example provider");
    assert_eq!(json["riskLevel"], "medium");
    assert_eq!(json["owner"]["kind"], "product_feature");
    assert_eq!(json["rollback"]["mode"], "disable_plugin");
    assert_eq!(json["denyState"], "candidate_discarded");
    assert_eq!(json["audit"]["correlationId"], "corr-1");
}

#[test]
fn diagnostic_and_quarantine_state_are_auditable_and_recoverable() {
    let diagnostic = PluginDiagnostic {
        diagnostic_id: "diag-1".to_string(),
        severity: PluginDiagnosticSeverity::Error,
        source: source_ref(),
        code: "config.missing_permission_gate".to_string(),
        message: "Command contribution must declare a permission gate".to_string(),
        detail: PluginDiagnosticDetail::ConfigValidation {
            manifest: manifest_ref(),
            validation: PluginConfigValidationState {
                status: PluginConfigValidationStatus::Invalid,
                issues: vec![PluginConfigValidationIssue {
                    field: "commands[0].permission".to_string(),
                    code: "missing_permission_gate".to_string(),
                    message: "Command contribution must declare a permission gate".to_string(),
                }],
            },
        },
        audit: audit_ref(),
        retryable: true,
        recovery_actions: vec![retry_action()],
    };
    let quarantine = quarantine_state();

    let diagnostic_json = serde_json::to_value(diagnostic).expect("serialize diagnostic");
    let quarantine_json = serde_json::to_value(quarantine).expect("serialize quarantine");

    assert_eq!(diagnostic_json["source"]["pluginId"], "opencode.example");
    assert_eq!(diagnostic_json["severity"], "error");
    assert_eq!(diagnostic_json["detail"]["kind"], "config_validation");
    assert_eq!(diagnostic_json["detail"]["validation"]["status"], "invalid");
    assert_eq!(
        diagnostic_json["recoveryActions"][0]["target"]["targetId"],
        "opencode.example.provider"
    );
    assert_eq!(diagnostic_json["audit"]["eventId"], "event-1");
    assert_eq!(quarantine_json["schemaVersion"], 1);
    assert_eq!(quarantine_json["source"]["contentHash"], "sha256:abc123");
    assert_eq!(quarantine_json["audit"]["correlationId"], "corr-1");
    assert_eq!(quarantine_json["scope"]["kind"], "plugin");
    assert_eq!(quarantine_json["reason"], "deadline_exceeded");
    assert_eq!(quarantine_json["logRef"]["artifactKind"], "host_log");
    assert_eq!(quarantine_json["recoveryActions"][0]["kind"], "retry");
    assert_eq!(quarantine_json["diagnosticIds"][0], "diag-1");
}

#[test]
fn recovery_action_request_and_result_are_typed_execution_contracts() {
    let request = PluginRecoveryActionRequest {
        request_id: "recovery-request-1".to_string(),
        source: source_ref(),
        action_id: "retry-1".to_string(),
        quarantine_id: "quarantine-1".to_string(),
        scope: PluginQuarantineScope::Plugin {
            plugin_id: "opencode.example".to_string(),
        },
        requested_by: PluginOwnerRef {
            kind: PluginOwnerKind::ProductFeature,
            id: "plugin-settings".to_string(),
        },
        authorization: audit_ref(),
        epochs: bitfun_runtime_ports::PluginRuntimeEpochs {
            project_epoch: 7,
            trust_epoch: 3,
            policy_epoch: 5,
            tool_registry_epoch: Some(11),
        },
        idempotency_key: "quarantine-1:retry".to_string(),
        requested_at_ms: 1_720_000_003,
    };
    let result = PluginRecoveryActionResult {
        request_id: "recovery-request-1".to_string(),
        action_id: "retry-1".to_string(),
        status: PluginRecoveryActionStatus::Accepted,
        diagnostic: None,
        quarantine: Some(quarantine_state()),
    };

    let request_json = serde_json::to_value(request).expect("serialize recovery request");
    let result_json = serde_json::to_value(result).expect("serialize recovery result");

    assert_eq!(request_json["source"]["pluginId"], "opencode.example");
    assert_eq!(request_json["actionId"], "retry-1");
    assert_eq!(request_json["quarantineId"], "quarantine-1");
    assert_eq!(request_json["scope"]["kind"], "plugin");
    assert_eq!(request_json["requestedBy"]["kind"], "product_feature");
    assert_eq!(request_json["authorization"]["correlationId"], "corr-1");
    assert_eq!(request_json["epochs"]["projectEpoch"], 7);
    assert_eq!(request_json["idempotencyKey"], "quarantine-1:retry");
    assert_eq!(result_json["status"], "accepted");
    assert_eq!(result_json["quarantine"]["quarantineId"], "quarantine-1");
}

#[test]
fn host_lifecycle_event_tracks_phase_source_and_epoch() {
    let event = PluginHostLifecycleEvent {
        event_id: "lifecycle-1".to_string(),
        phase: PluginHostLifecyclePhase::Dispatch,
        project_domain_id: "project-1".to_string(),
        source: source_ref(),
        observed_at_ms: 1_720_000_002,
        project_epoch: 7,
        diagnostic_id: Some("diag-1".to_string()),
    };

    let json = serde_json::to_value(event).expect("serialize lifecycle event");

    assert_eq!(json["phase"], "dispatch");
    assert_eq!(json["source"]["sourceKind"], "open_code_compatible");
    assert_eq!(
        json["source"]["manifest"]["schemaVersion"],
        "opencode.plugin.v1"
    );
    assert_eq!(json["projectEpoch"], 7);
    assert_eq!(json["diagnosticId"], "diag-1");
}
