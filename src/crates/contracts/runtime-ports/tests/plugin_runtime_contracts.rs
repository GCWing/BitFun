use bitfun_runtime_ports::{
    PermissionPromptDenyState, PermissionPromptDescriptor, PermissionPromptEffectKind,
    PluginArtifactRef, PluginAuditRef, PluginCapabilityRef, PluginDataClassification,
    PluginDispatchEnvelope, PluginEffectCandidate, PluginEffectCandidatePayload, PluginManifestRef,
    PluginOwnerKind, PluginOwnerRef, PluginPayloadRedaction, PluginPayloadRef,
    PluginPermissionGate, PluginResponseEnvelope, PluginRiskLevel, PluginRollbackMode,
    PluginRollbackPolicy, PluginRuntimeAvailability, PluginRuntimeBinding, PluginRuntimeEpochs,
    PluginRuntimeReadRequest, PluginRuntimeReadResponse, PluginRuntimeUnavailableReason,
    PluginSourceKind, PluginSourceRef, PluginStatusKind, PluginStatusSnapshot, PluginTargetRef,
    PluginTrustLevel, PortErrorKind,
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

fn capability_ref() -> PluginCapabilityRef {
    PluginCapabilityRef {
        capability_id: "tools.provider".to_string(),
        owner: PluginOwnerRef {
            kind: PluginOwnerKind::ExtensionContract,
            id: "extension.tool-provider".to_string(),
        },
    }
}

fn artifact_ref() -> PluginArtifactRef {
    PluginArtifactRef {
        artifact_id: "artifact-provider-1".to_string(),
        artifact_kind: "tool_provider_manifest".to_string(),
        display_name: "OpenCode provider manifest".to_string(),
        uri: Some("bitfun://artifacts/provider-manifest".to_string()),
    }
}

fn target_ref() -> PluginTargetRef {
    PluginTargetRef {
        target_kind: "tool_provider".to_string(),
        target_id: "opencode.example.provider".to_string(),
        display_name: "OpenCode example provider".to_string(),
        artifact: Some(artifact_ref()),
    }
}

fn audit_ref() -> PluginAuditRef {
    PluginAuditRef {
        correlation_id: "corr-1".to_string(),
        event_id: Some("event-1".to_string()),
    }
}

fn epochs() -> PluginRuntimeEpochs {
    PluginRuntimeEpochs {
        project_epoch: 7,
        trust_epoch: 3,
        policy_epoch: 5,
        tool_registry_epoch: Some(11),
    }
}

fn envelope(id: &str) -> PluginDispatchEnvelope {
    PluginDispatchEnvelope {
        envelope_version: 1,
        event_id: id.to_string(),
        event_type: "agent.turn.completed".to_string(),
        event_version: "2026-07-07".to_string(),
        project_domain_id: "project-1".to_string(),
        workspace_id: "workspace-1".to_string(),
        extension_point_id: "command.palette".to_string(),
        source: source_ref(),
        declared_capability: capability_ref(),
        correlation_id: "corr-1".to_string(),
        causation_id: None,
        idempotency_key: format!("{id}:command.palette"),
        deadline_ms: 30_000,
        epochs: epochs(),
        payload_ref: Some(PluginPayloadRef {
            payload_id: "payload-1".to_string(),
            schema_version: "agent.turn.completed.v1".to_string(),
            data_classification: PluginDataClassification::Workspace,
            redaction: PluginPayloadRedaction::Partial,
            uri: Some("bitfun://payloads/payload-1".to_string()),
        }),
    }
}

fn permission_prompt() -> PermissionPromptDescriptor {
    PermissionPromptDescriptor {
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
    }
}

#[test]
fn dispatch_envelope_serializes_typed_host_boundary_without_raw_payload() {
    let json = serde_json::to_value(envelope("event-1")).expect("serialize dispatch envelope");

    assert_eq!(json["envelopeVersion"], 1);
    assert_eq!(json["eventId"], "event-1");
    assert_eq!(json["extensionPointId"], "command.palette");
    assert_eq!(json["source"]["sourceKind"], "open_code_compatible");
    assert_eq!(
        json["source"]["manifest"]["schemaVersion"],
        "opencode.plugin.v1"
    );
    assert_eq!(json["declaredCapability"]["capabilityId"], "tools.provider");
    assert_eq!(json["epochs"]["policyEpoch"], 5);
    assert!(
        json.get("payload").is_none(),
        "raw payload must not be a stable host ABI"
    );
    assert_eq!(
        json["payloadRef"]["dataClassification"], "workspace",
        "payloads crossing the host boundary must carry classification"
    );

    let roundtrip: PluginDispatchEnvelope =
        serde_json::from_value(json).expect("dispatch envelope should round-trip");
    assert_eq!(roundtrip.source.plugin_id, "opencode.example");
    assert_eq!(
        roundtrip.payload_ref.expect("payload ref").payload_id,
        "payload-1"
    );
}

#[test]
fn response_envelope_carries_effect_candidates_and_observed_epochs() {
    let response = PluginResponseEnvelope {
        envelope_version: 1,
        request_event_id: "event-1".to_string(),
        project_domain_id: "project-1".to_string(),
        adapter_id: "opencode-compatible".to_string(),
        plugin_id: Some("opencode.example".to_string()),
        completed_at_ms: 1_720_000_001,
        effects: vec![PluginEffectCandidate {
            effect_id: "effect-1".to_string(),
            schema_version: "plugin.effect.v1".to_string(),
            declared_capability: capability_ref(),
            target_ref: target_ref(),
            data_classification: PluginDataClassification::Workspace,
            risk_level: PluginRiskLevel::Medium,
            permission: PluginPermissionGate::PermissionRequired {
                prompt: permission_prompt(),
            },
            source_ref: source_ref(),
            payload: PluginEffectCandidatePayload::ProviderCandidate {
                provider_id: "opencode.example.provider".to_string(),
                tool_contract_id: "tool-provider.v1".to_string(),
            },
        }],
        diagnostics: Vec::new(),
        quarantine: None,
        plugin_statuses: vec![PluginStatusSnapshot {
            source: source_ref(),
            status: PluginStatusKind::Enabled,
            availability: PluginRuntimeAvailability::Available,
            config_validation: None,
            quarantine: None,
            diagnostic_ids: Vec::new(),
            updated_at_ms: 1_720_000_001,
        }],
        observed_epochs: epochs(),
    };

    let json = serde_json::to_value(response).expect("serialize response envelope");

    assert_eq!(json["requestEventId"], "event-1");
    assert_eq!(json["effects"][0]["payload"]["kind"], "provider_candidate");
    assert_eq!(
        json["effects"][0]["permission"]["prompt"]["requestedEffect"],
        "provider_candidate"
    );
    assert_eq!(
        json["effects"][0]["permission"]["status"],
        "permission_required"
    );
    assert_eq!(
        json["effects"][0]["targetRef"]["artifact"]["displayName"],
        "OpenCode provider manifest"
    );
    assert_eq!(json["pluginStatuses"][0]["status"], "enabled");
    assert_eq!(
        json["pluginStatuses"][0]["availability"]["status"],
        "available"
    );
    assert_eq!(json["observedEpochs"]["toolRegistryEpoch"], 11);
    assert!(
        json.get("accepted").is_none(),
        "host responses must return typed candidates"
    );

    let roundtrip: PluginResponseEnvelope =
        serde_json::from_value(json).expect("response envelope should round-trip");
    assert_eq!(roundtrip.effects.len(), 1);
}

#[test]
fn policy_allowed_effects_keep_auditable_permission_facts() {
    let response = PluginResponseEnvelope {
        envelope_version: 1,
        request_event_id: "event-2".to_string(),
        project_domain_id: "project-1".to_string(),
        adapter_id: "opencode-compatible".to_string(),
        plugin_id: Some("opencode.example".to_string()),
        completed_at_ms: 1_720_000_002,
        effects: vec![PluginEffectCandidate {
            effect_id: "effect-2".to_string(),
            schema_version: "plugin.effect.v1".to_string(),
            declared_capability: capability_ref(),
            target_ref: target_ref(),
            data_classification: PluginDataClassification::Workspace,
            risk_level: PluginRiskLevel::Low,
            permission: PluginPermissionGate::PolicyAllowed { audit: audit_ref() },
            source_ref: source_ref(),
            payload: PluginEffectCandidatePayload::ProviderCandidate {
                provider_id: "opencode.example.provider".to_string(),
                tool_contract_id: "tool-provider.v1".to_string(),
            },
        }],
        diagnostics: Vec::new(),
        quarantine: None,
        plugin_statuses: Vec::new(),
        observed_epochs: epochs(),
    };

    let json = serde_json::to_value(response).expect("serialize policy-allowed effect");

    assert_eq!(json["effects"][0]["permission"]["status"], "policy_allowed");
    assert_eq!(
        json["effects"][0]["permission"]["audit"]["correlationId"],
        "corr-1"
    );
    assert!(
        json["effects"][0]["payload"]
            .get("materializeWhen")
            .is_none(),
        "materialization is derived from the permission gate instead of a free payload flag"
    );
    assert!(
        serde_json::from_value::<PluginPermissionGate>(serde_json::json!({
            "status": "not_required"
        }))
        .is_err(),
        "permission gates must not accept unaudited no-op states"
    );
}

#[test]
fn read_plugins_contract_supports_discovery_status_and_config_projection() {
    let request = PluginRuntimeReadRequest {
        request_id: "read-1".to_string(),
        project_domain_id: "project-1".to_string(),
        workspace_id: "workspace-1".to_string(),
        plugin_ids: vec!["opencode.example".to_string()],
        include_config_validation: true,
        epochs: epochs(),
    };
    let response = PluginRuntimeReadResponse {
        request_id: "read-1".to_string(),
        project_domain_id: "project-1".to_string(),
        sources: vec![source_ref()],
        plugin_statuses: vec![PluginStatusSnapshot {
            source: source_ref(),
            status: PluginStatusKind::Enabled,
            availability: PluginRuntimeAvailability::Available,
            config_validation: None,
            quarantine: None,
            diagnostic_ids: Vec::new(),
            updated_at_ms: 1_720_000_002,
        }],
        diagnostics: Vec::new(),
        observed_epochs: epochs(),
    };

    let request_json = serde_json::to_value(request).expect("serialize read request");
    let response_json = serde_json::to_value(response).expect("serialize read response");

    assert_eq!(request_json["includeConfigValidation"], true);
    assert_eq!(request_json["pluginIds"][0], "opencode.example");
    assert_eq!(response_json["sources"][0]["pluginId"], "opencode.example");
    assert_eq!(response_json["pluginStatuses"][0]["status"], "enabled");
    assert_eq!(response_json["observedEpochs"]["trustEpoch"], 3);
}

#[tokio::test]
async fn disabled_plugin_runtime_binding_reports_not_available() {
    let binding = PluginRuntimeBinding::disabled(PluginRuntimeUnavailableReason::NotBuilt);

    assert_eq!(
        binding.availability(),
        PluginRuntimeAvailability::Disabled {
            reason: PluginRuntimeUnavailableReason::NotBuilt
        }
    );

    let error = binding
        .as_client()
        .dispatch(envelope("dispatch-1"))
        .await
        .expect_err("disabled binding must not accept plugin dispatches");

    assert_eq!(error.kind, PortErrorKind::NotAvailable);
    assert!(error.message.contains("plugin runtime is disabled"));

    let read_error = binding
        .as_client()
        .read_plugins(PluginRuntimeReadRequest {
            request_id: "read-disabled".to_string(),
            project_domain_id: "project-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            plugin_ids: Vec::new(),
            include_config_validation: true,
            epochs: epochs(),
        })
        .await
        .expect_err("disabled binding must not expose plugin discovery/status reads");

    assert_eq!(read_error.kind, PortErrorKind::NotAvailable);
}

#[tokio::test]
async fn projection_only_plugin_runtime_rejects_dispatch_without_host() {
    let binding =
        PluginRuntimeBinding::projection_only(PluginRuntimeUnavailableReason::UnsupportedProfile);

    assert_eq!(
        binding.availability(),
        PluginRuntimeAvailability::ProjectionOnly {
            reason: PluginRuntimeUnavailableReason::UnsupportedProfile
        }
    );

    let error = binding
        .as_client()
        .dispatch(envelope("dispatch-2"))
        .await
        .expect_err("projection-only binding must not pretend to deliver plugin dispatches");

    assert_eq!(error.kind, PortErrorKind::NotAvailable);
    assert!(error.message.contains("projection-only"));
}
