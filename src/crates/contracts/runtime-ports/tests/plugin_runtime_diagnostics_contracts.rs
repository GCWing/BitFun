use bitfun_runtime_ports::{
    PermissionPromptDenyState, PermissionPromptDescriptor, PermissionPromptEffectKind,
    PluginArtifactRef, PluginAuditRef, PluginCapabilityRef, PluginConfigValidationIssue,
    PluginConfigValidationState, PluginConfigValidationStatus, PluginDiagnostic,
    PluginDiagnosticDetail, PluginDiagnosticSeverity, PluginManifestRef, PluginOwnerKind,
    PluginOwnerRef, PluginQuarantineClearCondition, PluginQuarantineReason, PluginQuarantineScope,
    PluginQuarantineState, PluginRiskLevel, PluginRollbackMode, PluginRollbackPolicy,
    PluginSourceKind, PluginSourceRef, PluginTargetRef, PluginTrustLevel,
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

fn quarantine_state() -> PluginQuarantineState {
    PluginQuarantineState {
        schema_version: 1,
        quarantine_id: "quarantine-1".to_string(),
        scope: PluginQuarantineScope::Plugin {
            project_domain_id: "project-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            plugin_id: "opencode.example".to_string(),
        },
        reason: PluginQuarantineReason::DeadlineExceeded,
        source: source_ref(),
        audit: audit_ref(),
        created_at_ms: 1_720_000_001,
        log_ref: Some(log_ref()),
        clears_when: vec![PluginQuarantineClearCondition::HostRestarted],
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
fn diagnostic_and_quarantine_state_are_auditable_projection_facts() {
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
        retryable: false,
    };
    let quarantine = quarantine_state();

    let diagnostic_json = serde_json::to_value(diagnostic).expect("serialize diagnostic");
    let quarantine_json = serde_json::to_value(quarantine).expect("serialize quarantine");

    assert_eq!(diagnostic_json["source"]["pluginId"], "opencode.example");
    assert_eq!(diagnostic_json["severity"], "error");
    assert_eq!(diagnostic_json["detail"]["kind"], "config_validation");
    assert_eq!(diagnostic_json["detail"]["validation"]["status"], "invalid");
    assert!(diagnostic_json.get("recoveryActions").is_none());
    assert_eq!(diagnostic_json["audit"]["eventId"], "event-1");
    assert_eq!(quarantine_json["schemaVersion"], 1);
    assert_eq!(quarantine_json["source"]["contentHash"], "sha256:abc123");
    assert_eq!(quarantine_json["audit"]["correlationId"], "corr-1");
    assert_eq!(quarantine_json["scope"]["kind"], "plugin");
    assert_eq!(quarantine_json["scope"]["projectDomainId"], "project-1");
    assert_eq!(quarantine_json["scope"]["workspaceId"], "workspace-1");
    assert_eq!(quarantine_json["reason"], "deadline_exceeded");
    assert_eq!(quarantine_json["logRef"]["artifactKind"], "host_log");
    assert!(quarantine_json.get("recoveryActions").is_none());
    assert_eq!(quarantine_json["diagnosticIds"][0], "diag-1");
}
