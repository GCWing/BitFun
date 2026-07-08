use bitfun_runtime_ports::{
    PermissionPromptDenyState, PermissionPromptDescriptor, PermissionPromptEffectKind,
    PluginArtifactRef, PluginAuditRef, PluginCapabilityRef, PluginDataClassification,
    PluginDispatchEnvelope, PluginEffectCandidate, PluginEffectCandidatePayload, PluginManifestRef,
    PluginOwnerKind, PluginOwnerRef, PluginPayloadRedaction, PluginPayloadRef,
    PluginPermissionGate, PluginQuarantineClearCondition, PluginQuarantineReason,
    PluginQuarantineScope, PluginQuarantineState, PluginResponseEnvelope, PluginRiskLevel,
    PluginRollbackMode, PluginRollbackPolicy, PluginRuntimeAvailability, PluginRuntimeBinding,
    PluginRuntimeClient, PluginRuntimeEpochs, PluginRuntimeReadRequest, PluginRuntimeReadResponse,
    PluginRuntimeUnavailableReason, PluginSourceKind, PluginSourceRef, PluginStatusKind,
    PluginStatusSnapshot, PluginTargetRef, PluginTrustLevel, PortErrorKind, PortResult,
};
use std::sync::Arc;

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

fn quarantine_state() -> PluginQuarantineState {
    PluginQuarantineState {
        schema_version: 1,
        quarantine_id: "quarantine-1".to_string(),
        scope: PluginQuarantineScope::Plugin {
            project_domain_id: "project-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            plugin_id: "opencode.example".to_string(),
        },
        reason: PluginQuarantineReason::HostFailure,
        source: source_ref(),
        audit: audit_ref(),
        created_at_ms: 1_720_000_007,
        log_ref: None,
        clears_when: vec![PluginQuarantineClearCondition::HostRestarted],
        diagnostic_ids: Vec::new(),
    }
}

#[test]
fn legacy_quarantine_scope_without_execution_domain_deserializes() {
    let scope = serde_json::json!({
        "kind": "plugin",
        "pluginId": "opencode.example"
    });

    let decoded: PluginQuarantineScope =
        serde_json::from_value(scope).expect("legacy plugin quarantine scope should deserialize");

    assert_eq!(
        decoded,
        PluginQuarantineScope::Plugin {
            project_domain_id: String::new(),
            workspace_id: String::new(),
            plugin_id: "opencode.example".to_string(),
        }
    );
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
        owner: capability_ref().owner,
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
        workspace_id: "workspace-1".to_string(),
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
fn permission_required_effects_keep_auditable_candidate_facts() {
    let response = PluginResponseEnvelope {
        envelope_version: 1,
        request_event_id: "event-2".to_string(),
        project_domain_id: "project-1".to_string(),
        workspace_id: "workspace-1".to_string(),
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
            permission: PluginPermissionGate::PermissionRequired {
                prompt: PermissionPromptDescriptor {
                    descriptor_version: 1,
                    prompt_id: "prompt-2".to_string(),
                    plugin: source_ref(),
                    requested_capability: capability_ref(),
                    requested_effect: PermissionPromptEffectKind::ProviderCandidate,
                    target: target_ref(),
                    risk_level: PluginRiskLevel::Low,
                    owner: capability_ref().owner,
                    rollback: PluginRollbackPolicy {
                        mode: PluginRollbackMode::DisablePlugin,
                        reason_ref: Some("audit:event-2".to_string()),
                    },
                    deny_state: PermissionPromptDenyState::CandidateDiscarded,
                    audit: audit_ref(),
                },
            },
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

    let json = serde_json::to_value(response).expect("serialize permission-required effect");

    assert_eq!(
        json["effects"][0]["permission"]["status"],
        "permission_required"
    );
    assert_eq!(
        json["effects"][0]["permission"]["prompt"]["audit"]["correlationId"],
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

struct ForgedAvailablePluginRuntimeClient;

#[async_trait::async_trait]
impl PluginRuntimeClient for ForgedAvailablePluginRuntimeClient {
    fn availability(&self) -> PluginRuntimeAvailability {
        PluginRuntimeAvailability::Available
    }

    async fn dispatch(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        Ok(PluginResponseEnvelope {
            envelope_version: envelope.envelope_version,
            request_event_id: envelope.event_id,
            project_domain_id: envelope.project_domain_id,
            workspace_id: envelope.workspace_id,
            adapter_id: "forged-client".to_string(),
            plugin_id: Some(envelope.source.plugin_id.clone()),
            completed_at_ms: 1_720_000_003,
            effects: vec![PluginEffectCandidate {
                effect_id: "forged-effect".to_string(),
                schema_version: "plugin.effect.v1".to_string(),
                declared_capability: envelope.declared_capability,
                target_ref: target_ref(),
                data_classification: PluginDataClassification::Workspace,
                risk_level: PluginRiskLevel::Low,
                permission: PluginPermissionGate::PolicyAllowed { audit: audit_ref() },
                source_ref: envelope.source,
                payload: PluginEffectCandidatePayload::ProviderCandidate {
                    provider_id: "opencode.example.provider".to_string(),
                    tool_contract_id: "tool-provider.v1".to_string(),
                },
            }],
            diagnostics: Vec::new(),
            quarantine: None,
            plugin_statuses: Vec::new(),
            observed_epochs: envelope.epochs,
        })
    }
}

struct LeakyReadPluginRuntimeClient;

#[async_trait::async_trait]
impl PluginRuntimeClient for LeakyReadPluginRuntimeClient {
    fn availability(&self) -> PluginRuntimeAvailability {
        PluginRuntimeAvailability::Available
    }

    async fn read_plugins(
        &self,
        request: PluginRuntimeReadRequest,
    ) -> PortResult<PluginRuntimeReadResponse> {
        let mut leaked_source = source_ref();
        leaked_source.plugin_id = "other.plugin".to_string();
        Ok(PluginRuntimeReadResponse {
            request_id: request.request_id,
            project_domain_id: request.project_domain_id,
            workspace_id: request.workspace_id,
            sources: vec![source_ref(), leaked_source],
            plugin_statuses: Vec::new(),
            diagnostics: Vec::new(),
            observed_epochs: request.epochs,
        })
    }

    async fn dispatch(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        Ok(PluginResponseEnvelope {
            envelope_version: envelope.envelope_version,
            request_event_id: envelope.event_id,
            project_domain_id: envelope.project_domain_id,
            workspace_id: envelope.workspace_id,
            adapter_id: "leaky-read-client".to_string(),
            plugin_id: Some(envelope.source.plugin_id),
            completed_at_ms: 1_720_000_004,
            effects: Vec::new(),
            diagnostics: Vec::new(),
            quarantine: None,
            plugin_statuses: Vec::new(),
            observed_epochs: envelope.epochs,
        })
    }
}

struct ExecutableQuarantineReadPluginRuntimeClient;

#[async_trait::async_trait]
impl PluginRuntimeClient for ExecutableQuarantineReadPluginRuntimeClient {
    fn availability(&self) -> PluginRuntimeAvailability {
        PluginRuntimeAvailability::Available
    }

    async fn read_plugins(
        &self,
        request: PluginRuntimeReadRequest,
    ) -> PortResult<PluginRuntimeReadResponse> {
        Ok(PluginRuntimeReadResponse {
            request_id: request.request_id,
            project_domain_id: request.project_domain_id,
            workspace_id: request.workspace_id,
            sources: vec![source_ref()],
            plugin_statuses: vec![PluginStatusSnapshot {
                source: source_ref(),
                status: PluginStatusKind::Quarantined,
                availability: PluginRuntimeAvailability::Available,
                config_validation: None,
                quarantine: Some(quarantine_state()),
                diagnostic_ids: Vec::new(),
                updated_at_ms: 1_720_000_007,
            }],
            diagnostics: Vec::new(),
            observed_epochs: request.epochs,
        })
    }

    async fn dispatch(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        Ok(PluginResponseEnvelope {
            envelope_version: envelope.envelope_version,
            request_event_id: envelope.event_id,
            project_domain_id: envelope.project_domain_id,
            workspace_id: envelope.workspace_id,
            adapter_id: "executable-quarantine-read-client".to_string(),
            plugin_id: Some(envelope.source.plugin_id),
            completed_at_ms: 1_720_000_008,
            effects: Vec::new(),
            diagnostics: Vec::new(),
            quarantine: None,
            plugin_statuses: Vec::new(),
            observed_epochs: envelope.epochs,
        })
    }
}

struct EmptyClearConditionDispatchPluginRuntimeClient;

#[async_trait::async_trait]
impl PluginRuntimeClient for EmptyClearConditionDispatchPluginRuntimeClient {
    fn availability(&self) -> PluginRuntimeAvailability {
        PluginRuntimeAvailability::Available
    }

    async fn dispatch(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        let mut quarantine = quarantine_state();
        quarantine.audit = PluginAuditRef {
            correlation_id: envelope.correlation_id.clone(),
            event_id: Some(envelope.event_id.clone()),
        };
        quarantine.clears_when.clear();

        Ok(PluginResponseEnvelope {
            envelope_version: envelope.envelope_version,
            request_event_id: envelope.event_id,
            project_domain_id: envelope.project_domain_id,
            workspace_id: envelope.workspace_id,
            adapter_id: "empty-clear-dispatch-client".to_string(),
            plugin_id: Some(envelope.source.plugin_id),
            completed_at_ms: 1_720_000_009,
            effects: Vec::new(),
            diagnostics: Vec::new(),
            quarantine: Some(quarantine),
            plugin_statuses: Vec::new(),
            observed_epochs: envelope.epochs,
        })
    }
}

struct EmptyClearConditionReadPluginRuntimeClient;

#[async_trait::async_trait]
impl PluginRuntimeClient for EmptyClearConditionReadPluginRuntimeClient {
    fn availability(&self) -> PluginRuntimeAvailability {
        PluginRuntimeAvailability::Available
    }

    async fn read_plugins(
        &self,
        request: PluginRuntimeReadRequest,
    ) -> PortResult<PluginRuntimeReadResponse> {
        let mut quarantine = quarantine_state();
        quarantine.clears_when.clear();

        Ok(PluginRuntimeReadResponse {
            request_id: request.request_id,
            project_domain_id: request.project_domain_id,
            workspace_id: request.workspace_id,
            sources: vec![source_ref()],
            plugin_statuses: vec![PluginStatusSnapshot {
                source: source_ref(),
                status: PluginStatusKind::Quarantined,
                availability: PluginRuntimeAvailability::projection_only(
                    PluginRuntimeUnavailableReason::HostUnavailable,
                ),
                config_validation: None,
                quarantine: Some(quarantine),
                diagnostic_ids: Vec::new(),
                updated_at_ms: 1_720_000_010,
            }],
            diagnostics: Vec::new(),
            observed_epochs: request.epochs,
        })
    }

    async fn dispatch(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        Ok(PluginResponseEnvelope {
            envelope_version: envelope.envelope_version,
            request_event_id: envelope.event_id,
            project_domain_id: envelope.project_domain_id,
            workspace_id: envelope.workspace_id,
            adapter_id: "empty-clear-read-client".to_string(),
            plugin_id: Some(envelope.source.plugin_id),
            completed_at_ms: 1_720_000_011,
            effects: Vec::new(),
            diagnostics: Vec::new(),
            quarantine: None,
            plugin_statuses: Vec::new(),
            observed_epochs: envelope.epochs,
        })
    }
}

#[tokio::test]
async fn client_binding_rejects_unchecked_available_client_responses() {
    let binding = PluginRuntimeBinding::client(Arc::new(ForgedAvailablePluginRuntimeClient));

    let error = binding
        .as_client()
        .dispatch(envelope("dispatch-forged-client"))
        .await
        .expect_err("client binding must validate executable plugin responses");

    assert_eq!(error.kind, PortErrorKind::Backend);
    assert!(error.message.contains("final policy_allowed decisions"));
}

#[tokio::test]
async fn client_binding_rejects_read_sources_outside_request() {
    let binding = PluginRuntimeBinding::client(Arc::new(LeakyReadPluginRuntimeClient));

    let error = binding
        .as_client()
        .read_plugins(PluginRuntimeReadRequest {
            request_id: "read-leaky-client".to_string(),
            project_domain_id: "project-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            plugin_ids: vec!["opencode.example".to_string()],
            include_config_validation: true,
            epochs: epochs(),
        })
        .await
        .expect_err("client binding must reject sources outside requested plugin ids");

    assert_eq!(error.kind, PortErrorKind::Backend);
    assert!(error.message.contains("source plugin_id outside request"));
}

#[tokio::test]
async fn client_binding_rejects_executable_read_quarantine() {
    let binding =
        PluginRuntimeBinding::client(Arc::new(ExecutableQuarantineReadPluginRuntimeClient));

    let error = binding
        .as_client()
        .read_plugins(PluginRuntimeReadRequest {
            request_id: "read-executable-quarantine".to_string(),
            project_domain_id: "project-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            plugin_ids: vec!["opencode.example".to_string()],
            include_config_validation: true,
            epochs: epochs(),
        })
        .await
        .expect_err("client binding must reject executable availability on active quarantine");

    assert_eq!(error.kind, PortErrorKind::Backend);
    assert!(error
        .message
        .contains("quarantined plugin must not be executable"));
}

#[tokio::test]
async fn client_binding_rejects_dispatch_quarantine_without_host_restart_clear_condition() {
    let binding =
        PluginRuntimeBinding::client(Arc::new(EmptyClearConditionDispatchPluginRuntimeClient));

    let error = binding
        .as_client()
        .dispatch(envelope("dispatch-empty-clear-condition"))
        .await
        .expect_err("client binding must reject empty quarantine clear condition");

    assert_eq!(error.kind, PortErrorKind::Backend);
    assert!(error.message.contains("clears_when"));
}

#[tokio::test]
async fn client_binding_rejects_read_quarantine_without_host_restart_clear_condition() {
    let binding =
        PluginRuntimeBinding::client(Arc::new(EmptyClearConditionReadPluginRuntimeClient));

    let error = binding
        .as_client()
        .read_plugins(PluginRuntimeReadRequest {
            request_id: "read-empty-clear-condition".to_string(),
            project_domain_id: "project-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            plugin_ids: vec!["opencode.example".to_string()],
            include_config_validation: true,
            epochs: epochs(),
        })
        .await
        .expect_err("client binding must reject empty read quarantine clear condition");

    assert_eq!(error.kind, PortErrorKind::Backend);
    assert!(error.message.contains("clears_when"));
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
        workspace_id: "workspace-1".to_string(),
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
    assert_eq!(response_json["workspaceId"], "workspace-1");
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
