use bitfun_product_domains::external_integration_policy::{
    evaluate_external_integration_policy, external_integration_policy_snapshot,
    ExternalEcosystemPolicy, ExternalEcosystemPolicyOverride, ExternalIntegrationAccess,
    ExternalIntegrationCapabilityDescriptor, ExternalIntegrationEcosystemDescriptor,
    ExternalIntegrationMode, ExternalIntegrationPolicyDocument, ExternalIntegrationPolicyOverride,
    ExternalIntegrationPolicyStatus,
};
use bitfun_product_domains::external_source_control::{
    ExternalSourceControlActionV1, ExternalSourceControlRequestV1, ExternalSourceControlSnapshotV1,
    ExternalSourceDesiredState, ExternalSourceDiscoveryState, ExternalSourceOperationStage,
    ExternalSourceRecoveryActionV1, ExternalSourceReviewState, EXTERNAL_SOURCE_CONTROL_SCHEMA_V1,
};
use bitfun_product_domains::external_sources::{
    external_mcp_approval_key, external_mcp_conflict_key, external_tool_approval_key,
    external_tool_conflict_key, prompt_command_conflict_key, EcosystemId, ExecutionDomainId,
    ExpandedPromptCommand, ExternalIntegrationCapabilityId, ExternalMcpActivationState,
    ExternalMcpApprovalRequest, ExternalMcpCatalogEntry, ExternalMcpConflict,
    ExternalMcpConflictCandidate, ExternalMcpDiscoveryInput, ExternalMcpProviderIdentity,
    ExternalMcpProviderSnapshot, ExternalMcpServerDefinition, ExternalMcpStaticStatus,
    ExternalMcpTransportKind, ExternalSourceAssetKind, ExternalSourceCatalogEntry,
    ExternalSourceCatalogSnapshot, ExternalSourceContext, ExternalSourceDiagnostic,
    ExternalSourceHealth, ExternalSourceHostCapabilities, ExternalSourceLifecycleState,
    ExternalSourceOperationError, ExternalSourceOperationErrorCode, ExternalSourceProviderError,
    ExternalSourcePublicSnapshot, ExternalSourceRecord, ExternalSourceScope,
    ExternalToolCapability, ExternalToolDefinition, ExternalToolRuntimeKind,
    ExternalToolStaticStatus, ExternalWatchRoot, PreparedExternalMcpServer,
    PreparedExternalMcpTransport, PromptCommandAvailability, PromptCommandCatalogEntry,
    PromptCommandDefinition, PromptCommandProviderIdentity, PromptCommandProviderSnapshot,
    PromptCommandSourceProvider, SecretValue, SourceKey, SourceQualifiedCommandId,
    SourceQualifiedMcpServerId, SourceQualifiedToolId, SourceQualifiedToolTargetId,
};
use bitfun_product_domains::external_subagents::{
    external_subagent_approval_key, external_subagent_candidate_id, external_subagent_conflict_key,
    ExternalSubagentBehaviorVersion, ExternalSubagentCandidateId,
    ExternalSubagentCompatibilityState, ExternalSubagentContributionId,
    ExternalSubagentContributionRole, ExternalSubagentDefinition, ExternalSubagentDiscoveryInput,
    ExternalSubagentLocalId, ExternalSubagentMode, ExternalSubagentModelRequest,
    ExternalSubagentProvenanceRef, ExternalSubagentProviderIdentity,
    ExternalSubagentProviderSnapshot, ExternalSubagentToolRequest, ExternalSubagentToolSelector,
    SecretText,
};
use std::path::PathBuf;

fn source(provider_id: &str, ecosystem_id: &str, source_id: &str) -> ExternalSourceRecord {
    ExternalSourceRecord {
        key: SourceKey::new(provider_id, source_id).expect("valid source key"),
        ecosystem_id: EcosystemId::new(ecosystem_id).expect("valid ecosystem id"),
        display_name: format!("{provider_id} commands"),
        source_kind: "prompt_commands".to_string(),
        scope: ExternalSourceScope::Project,
        location: format!("/workspace/{provider_id}"),
        execution_domain_id: ExecutionDomainId::new("local-user").expect("valid domain"),
        health: ExternalSourceHealth::Available,
        content_version: format!("{provider_id}-v1"),
        diagnostics: Vec::new(),
    }
}

fn command(provider_id: &str, source_id: &str, precedence: i32) -> PromptCommandDefinition {
    PromptCommandDefinition {
        id: SourceQualifiedCommandId::new(
            SourceKey::new(provider_id, source_id).unwrap(),
            "review",
        )
        .unwrap(),
        name: "review".to_string(),
        description: format!("Review from {provider_id}"),
        template: format!("{provider_id}: $ARGUMENTS"),
        availability: PromptCommandAvailability::Available,
        content_version: format!("command-v{precedence}"),
    }
}

fn context() -> ExternalSourceContext {
    ExternalSourceContext {
        workspace_root: Some(PathBuf::from("/workspace")),
        execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
    }
}

#[test]
fn opaque_ids_are_validated_without_closing_the_ecosystem_set() {
    assert_eq!(
        EcosystemId::new("future.product/v2")
            .expect("future ecosystem ids remain open")
            .as_str(),
        "future.product/v2"
    );
    assert!(EcosystemId::new("  ").is_err());
    assert!(ExecutionDomainId::new("domain\nwith-control").is_err());
}

#[test]
fn source_and_command_identity_remain_provider_qualified() {
    let left = SourceQualifiedCommandId::new(
        SourceKey::new("adapter-a", "project-commands").unwrap(),
        "review",
    )
    .unwrap();
    let right = SourceQualifiedCommandId::new(
        SourceKey::new("adapter-b", "project-commands").unwrap(),
        "review",
    )
    .unwrap();

    assert_ne!(left, right);
    assert_ne!(left.stable_key(), right.stable_key());
}

#[test]
fn presentation_group_id_is_optional_and_uses_the_camel_case_wire_name() {
    let mut entry = ExternalSourceCatalogEntry {
        stable_key: "opencode.commands:project".to_string(),
        presentation_group_id: None,
        record: source("opencode.commands", "opencode", "project"),
        lifecycle: ExternalSourceLifecycleState::Available,
    };

    let legacy_value = serde_json::to_value(&entry).unwrap();
    assert!(legacy_value.get("presentationGroupId").is_none());
    let legacy_entry: ExternalSourceCatalogEntry = serde_json::from_value(legacy_value).unwrap();
    assert!(legacy_entry.presentation_group_id.is_none());

    entry.presentation_group_id = Some("external-source:[\"source\"]".to_string());
    let current_value = serde_json::to_value(&entry).unwrap();
    assert_eq!(
        current_value["presentationGroupId"],
        "external-source:[\"source\"]"
    );
}

#[test]
fn conflict_fingerprint_is_order_independent_and_changes_with_content() {
    let first = prompt_command_conflict_key("local-user", "review", [("a", "v1"), ("b", "v2")]);
    let reordered = prompt_command_conflict_key("local-user", "REVIEW", [("b", "v2"), ("a", "v1")]);
    let updated = prompt_command_conflict_key("local-user", "review", [("a", "v1"), ("b", "v3")]);
    let remote = prompt_command_conflict_key("remote-user", "review", [("a", "v1"), ("b", "v2")]);

    assert_eq!(first, reordered);
    assert_ne!(first, updated);
    assert_ne!(first, remote);
}

#[test]
fn prompt_commands_use_a_typed_contract_instead_of_an_arbitrary_asset_payload() {
    let command = PromptCommandDefinition {
        id: SourceQualifiedCommandId::new(
            SourceKey::new("example-provider", "project-commands").unwrap(),
            "review",
        )
        .unwrap(),
        name: "review".to_string(),
        description: "Review the current change".to_string(),
        template: "Review $ARGUMENTS".to_string(),
        availability: PromptCommandAvailability::Restricted {
            reason: "Shell expansion is not supported yet".to_string(),
            required_capabilities: vec!["command.shell".to_string()],
        },
        content_version: "sha256:command-v1".to_string(),
    };

    let encoded = serde_json::to_value(&command).expect("serialize command contract");
    assert_eq!(encoded["name"], "review");
    assert_eq!(encoded["availability"]["state"], "restricted");
    assert!(encoded.get("payload").is_none());
}

struct FakeProvider {
    identity: PromptCommandProviderIdentity,
    snapshot: PromptCommandProviderSnapshot,
}

impl FakeProvider {
    fn new(provider_id: &str, ecosystem_id: &str, source_id: &str, precedence: i32) -> Self {
        let identity = PromptCommandProviderIdentity::new(
            provider_id,
            ecosystem_id,
            format!("{provider_id} display"),
        )
        .unwrap();
        Self {
            identity: identity.clone(),
            snapshot: PromptCommandProviderSnapshot {
                provider: identity,
                sources: vec![source(provider_id, ecosystem_id, source_id)],
                commands: vec![command(provider_id, source_id, precedence)],
                unavailable_command_ids: Vec::new(),
                diagnostics: Vec::new(),
            },
        }
    }
}

impl PromptCommandSourceProvider for FakeProvider {
    fn identity(&self) -> PromptCommandProviderIdentity {
        self.identity.clone()
    }

    fn discover(
        &self,
        _context: &ExternalSourceContext,
    ) -> Result<PromptCommandProviderSnapshot, ExternalSourceProviderError> {
        Ok(self.snapshot.clone())
    }

    fn expand(
        &self,
        command: &PromptCommandDefinition,
        arguments: &str,
    ) -> Result<ExpandedPromptCommand, ExternalSourceProviderError> {
        Ok(ExpandedPromptCommand {
            content: command.template.replace("$ARGUMENTS", arguments),
        })
    }

    fn watch_roots(&self, context: &ExternalSourceContext) -> Vec<ExternalWatchRoot> {
        vec![ExternalWatchRoot {
            path: context.workspace_root.clone().unwrap(),
            recursive: true,
        }]
    }
}

#[test]
fn capability_provider_contract_does_not_require_core_or_an_ecosystem_enum() {
    let provider: Box<dyn PromptCommandSourceProvider> = Box::new(FakeProvider::new(
        "fake-provider",
        "fake.ecosystem",
        "project-commands",
        1,
    ));

    let snapshot = provider.discover(&context()).expect("discover fake source");
    assert_eq!(snapshot.provider.ecosystem_id.as_str(), "fake.ecosystem");
    assert_eq!(provider.watch_roots(&context()).len(), 1);
}

#[test]
fn persisted_source_preference_keys_round_trip_without_path_guessing() {
    let record = source(
        "provider.with.dots",
        "fake.ecosystem",
        "project/source:agents",
    );
    assert_eq!(
        ExternalSourceRecord::source_key_from_preference_key(&record.preference_key()),
        Some(record.key)
    );
    assert!(ExternalSourceRecord::source_key_from_preference_key("malformed").is_none());
}

#[test]
fn external_subagent_identity_preserves_ordered_provenance_and_separate_revisions() {
    let provider =
        ExternalSubagentProviderIdentity::new("fake.agents", "fake.ecosystem", "Fake Agents")
            .unwrap();
    let first = ExternalSubagentContributionId::new(
        SourceKey::new("fake.agents", "global-config").unwrap(),
        ExternalSubagentLocalId::new("review").unwrap(),
    );
    let second = ExternalSubagentContributionId::new(
        SourceKey::new("fake.agents", "project-config").unwrap(),
        ExternalSubagentLocalId::new("review").unwrap(),
    );
    let provenance = vec![
        ExternalSubagentProvenanceRef {
            contribution_id: first,
            role: ExternalSubagentContributionRole::Base,
        },
        ExternalSubagentProvenanceRef {
            contribution_id: second,
            role: ExternalSubagentContributionRole::Overlay,
        },
    ];
    let candidate_id = external_subagent_candidate_id(&provider.provider_id, "review", &provenance);
    let reversed = external_subagent_candidate_id(
        &provider.provider_id,
        "review",
        &provenance.iter().cloned().rev().collect::<Vec<_>>(),
    );
    assert_ne!(
        candidate_id, reversed,
        "provenance order changes behavior identity"
    );

    let definition = ExternalSubagentDefinition {
        candidate_id,
        logical_id: "review".to_string(),
        provenance,
        display_name: "Review".to_string(),
        description: "Reviews a change".to_string(),
        prompt: SecretText::new("Review carefully"),
        mode: ExternalSubagentMode::Subagent,
        disabled: false,
        hidden: false,
        requested_model: ExternalSubagentModelRequest::Default,
        requested_tools: ExternalSubagentToolRequest {
            selectors: vec![ExternalSubagentToolSelector {
                source_name: "read".to_string(),
                canonical_host_name: Some("Read".to_string()),
                allowed: true,
            }],
            uses_conservative_default: false,
        },
        compatibility: ExternalSubagentCompatibilityState::Ready,
        diagnostic_codes: Vec::new(),
        behavior_version: ExternalSubagentBehaviorVersion::new("behavior-v1").unwrap(),
    };
    assert_eq!(definition.prompt.expose(), "Review carefully");
    assert!(!format!("{definition:?}").contains("Review carefully"));

    let mut invalid_model = definition.clone();
    invalid_model.requested_model = ExternalSubagentModelRequest::Exact {
        provider_hint: Some("fake\nprovider".to_string()),
        model_name: "model".to_string(),
    };
    assert!(invalid_model.validate().is_err());

    let mut invalid_tool = definition.clone();
    invalid_tool.requested_tools.selectors[0].source_name = "read\nsecret".to_string();
    assert!(invalid_tool.validate().is_err());

    let mut invalid_diagnostic = definition.clone();
    invalid_diagnostic.diagnostic_codes = vec!["provider.invalid:raw-source-key".to_string()];
    assert!(invalid_diagnostic.validate().is_err());

    let mut excessive_tools = definition.clone();
    excessive_tools.requested_tools.selectors = (0..257)
        .map(|index| ExternalSubagentToolSelector {
            source_name: format!("tool-{index}"),
            canonical_host_name: None,
            allowed: true,
        })
        .collect();
    assert!(excessive_tools.validate().is_err());

    let snapshot = ExternalSubagentProviderSnapshot {
        provider,
        sources: vec![
            source("fake.agents", "fake.ecosystem", "global-config"),
            source("fake.agents", "fake.ecosystem", "project-config"),
        ],
        definitions: vec![definition],
        diagnostics: Vec::new(),
    };
    snapshot
        .validate()
        .expect("valid external subagent provider snapshot");

    let source_key = snapshot.sources[0].key.clone();
    let mut valid_diagnostic = snapshot.clone();
    valid_diagnostic.diagnostics.push(
        ExternalSourceDiagnostic::warning(
            "fake.agent.degraded",
            "An optional field is not supported",
            Some(source_key),
        )
        .with_asset_kind(ExternalSourceAssetKind::Subagent),
    );
    valid_diagnostic
        .validate()
        .expect("bounded provider diagnostics with a known source are valid");

    let mut valid_source_diagnostic = snapshot.clone();
    let valid_source_key = valid_source_diagnostic.sources[0].key.clone();
    valid_source_diagnostic.sources[0].diagnostics.push(
        ExternalSourceDiagnostic::warning(
            "fake.agent.source_degraded",
            "This source has a recoverable warning",
            Some(valid_source_key),
        )
        .with_asset_kind(ExternalSourceAssetKind::Subagent),
    );
    valid_source_diagnostic
        .validate()
        .expect("source-owned diagnostics use the same provider contract");

    let mut invalid_provider_diagnostic = snapshot.clone();
    invalid_provider_diagnostic.diagnostics.push(
        ExternalSourceDiagnostic::warning(
            "fake.agent:raw-source",
            "Invalid diagnostic code",
            Some(SourceKey::new("other.agents", "project").unwrap()),
        )
        .with_asset_kind(ExternalSourceAssetKind::Command),
    );
    assert!(invalid_provider_diagnostic.validate().is_err());

    let mut wrong_provider_diagnostic = snapshot.clone();
    wrong_provider_diagnostic.diagnostics.push(
        ExternalSourceDiagnostic::warning(
            "fake.agent.invalid_source",
            "Unknown provider source",
            Some(SourceKey::new("other.agents", "project").unwrap()),
        )
        .with_asset_kind(ExternalSourceAssetKind::Subagent),
    );
    assert!(wrong_provider_diagnostic.validate().is_err());

    let mut unknown_source_diagnostic = snapshot.clone();
    unknown_source_diagnostic.diagnostics.push(
        ExternalSourceDiagnostic::warning(
            "fake.agent.unknown_source",
            "Unknown source",
            Some(SourceKey::new("fake.agents", "missing").unwrap()),
        )
        .with_asset_kind(ExternalSourceAssetKind::Subagent),
    );
    assert!(unknown_source_diagnostic.validate().is_err());

    let mut invalid_diagnostic_message = snapshot.clone();
    invalid_diagnostic_message.diagnostics.push(
        ExternalSourceDiagnostic::warning("fake.agent.invalid_message", "invalid\nmessage", None)
            .with_asset_kind(ExternalSourceAssetKind::Subagent),
    );
    assert!(invalid_diagnostic_message.validate().is_err());

    let mut wrong_asset_kind = snapshot.clone();
    wrong_asset_kind.diagnostics.push(
        ExternalSourceDiagnostic::warning(
            "fake.agent.wrong_kind",
            "Diagnostic belongs to another asset kind",
            None,
        )
        .with_asset_kind(ExternalSourceAssetKind::Tool),
    );
    assert!(wrong_asset_kind.validate().is_err());

    let mut excessive_sources = snapshot.clone();
    excessive_sources.sources = vec![snapshot.sources[0].clone(); 1025];
    assert!(excessive_sources.validate().is_err());

    let mut excessive_definitions = snapshot.clone();
    excessive_definitions.definitions = vec![snapshot.definitions[0].clone(); 1025];
    assert!(excessive_definitions.validate().is_err());

    let mut excessive_diagnostics = snapshot.clone();
    excessive_diagnostics.diagnostics = vec![
        ExternalSourceDiagnostic::warning(
            "fake.agent.degraded",
            "An optional field is not supported",
            None,
        )
        .with_asset_kind(ExternalSourceAssetKind::Subagent);
        1025
    ];
    assert!(excessive_diagnostics.validate().is_err());

    let mut excessive_provenance = snapshot.clone();
    excessive_provenance.definitions[0].provenance =
        vec![snapshot.definitions[0].provenance[0].clone(); 257];
    assert!(excessive_provenance.validate().is_err());

    let input = ExternalSubagentDiscoveryInput {
        context: context(),
        suppressed_sources: [SourceKey::new("fake.agents", "suppressed").unwrap()]
            .into_iter()
            .collect(),
    };
    assert_eq!(input.suppressed_sources.len(), 1);
}

#[test]
fn external_subagent_decision_keys_bind_behavior_but_not_catalog_copy() {
    let candidate = ExternalSubagentCandidateId::new("candidate-v1").unwrap();
    let behavior = ExternalSubagentBehaviorVersion::new("behavior-v1").unwrap();
    let approval = external_subagent_approval_key(&candidate, &behavior, "envelope-v1");
    let same = external_subagent_approval_key(&candidate, &behavior, "envelope-v1");
    let changed = external_subagent_approval_key(
        &candidate,
        &ExternalSubagentBehaviorVersion::new("behavior-v2").unwrap(),
        "envelope-v1",
    );
    assert_eq!(approval, same);
    assert_ne!(approval, changed);

    let first = external_subagent_conflict_key(
        "local-user",
        "/workspace",
        "review",
        [("local", "v1"), (candidate.as_str(), behavior.as_str())],
    );
    let reordered = external_subagent_conflict_key(
        "local-user",
        "/workspace",
        "REVIEW",
        [(candidate.as_str(), behavior.as_str()), ("local", "v1")],
    );
    assert_eq!(first, reordered);
}

#[test]
fn diagnostics_remain_source_qualified() {
    let diagnostic = ExternalSourceDiagnostic::warning(
        "fake.warning",
        "A non-blocking fake diagnostic",
        Some(SourceKey::new("fake", "source").unwrap()),
    );
    assert_eq!(diagnostic.source.unwrap().provider_id.as_str(), "fake");
}

#[test]
fn provider_snapshot_rejects_duplicate_sources_and_commands() {
    let provider = FakeProvider::new("fake", "fake.ecosystem", "project", 1);
    let mut duplicate_source = provider.snapshot.clone();
    duplicate_source
        .sources
        .push(duplicate_source.sources[0].clone());
    assert!(duplicate_source.validate().is_err());

    let mut duplicate_command = provider.snapshot;
    duplicate_command
        .commands
        .push(duplicate_command.commands[0].clone());
    assert!(duplicate_command.validate().is_err());
}

#[test]
fn unavailable_command_must_be_unique_absent_and_source_qualified() {
    let provider = FakeProvider::new("fake", "fake.ecosystem", "project", 1);
    let mut invalid = provider.snapshot;
    invalid
        .unavailable_command_ids
        .push(invalid.commands[0].id.clone());
    assert!(invalid.validate().is_err());
}

#[test]
fn standalone_tool_contract_separates_static_preview_from_executable_source() {
    let target = SourceQualifiedToolTargetId::new(
        SourceKey::new("opencode.tools", "project-tools").unwrap(),
        "weather.js",
    )
    .unwrap();
    let tool = ExternalToolDefinition {
        id: SourceQualifiedToolId::new(target, "default").unwrap(),
        name: "weather".to_string(),
        description_preview: "Get the weather for a location".to_string(),
        module_path: "/workspace/.opencode/tools/weather.js".to_string(),
        working_directory: "/workspace".to_string(),
        runtime_kind: ExternalToolRuntimeKind::JavaScript,
        capabilities: vec![
            ExternalToolCapability::FileSystem,
            ExternalToolCapability::Network,
            ExternalToolCapability::Process,
        ],
        content_version: "sha256:v1".to_string(),
        static_status: ExternalToolStaticStatus::Ready,
    };

    let encoded = serde_json::to_value(&tool).expect("serialize tool preview");
    assert_eq!(encoded["name"], "weather");
    assert_eq!(encoded["runtimeKind"], "java_script");
    assert!(encoded.get("moduleSource").is_none());
    assert!(encoded.get("payload").is_none());
    tool.validate().expect("valid standalone tool preview");
}

#[test]
fn legacy_public_snapshot_downprojects_new_tool_review_variants() {
    let snapshot: ExternalSourcePublicSnapshot = serde_json::from_value(serde_json::json!({
        "generation": 1,
        "discoveryPending": false,
        "sources": [],
        "commands": [],
        "tools": [{
            "definition": {
                "id": {
                    "target": {
                        "source": { "providerId": "opencode.tools", "sourceId": "project" },
                        "localId": "weather.js"
                    },
                    "exportId": "default"
                },
                "name": "weather",
                "descriptionPreview": "Get weather",
                "modulePath": "<workspace>/.opencode/tools/weather.js",
                "workingDirectory": "<workspace>",
                "runtimeKind": "java_script",
                "capabilities": [],
                "contentVersion": "sha256:v1",
                "staticStatus": { "state": "ready" }
            },
            "approvalKey": "approval-v1",
            "decisionKey": "decision-v1",
            "activation": { "state": "declined" }
        }]
    }))
    .expect("new public snapshot");

    let legacy =
        serde_json::to_value(snapshot.into_legacy_v0_compatible()).expect("legacy public snapshot");
    assert_eq!(legacy["tools"][0]["activation"]["state"], "disabled");
}

#[test]
fn standalone_tool_contract_rejects_names_that_are_not_model_callable() {
    let target = SourceQualifiedToolTargetId::new(
        SourceKey::new("fake.tools", "project-tools").unwrap(),
        "unsafe.js",
    )
    .unwrap();
    let mut tool = ExternalToolDefinition {
        id: SourceQualifiedToolId::new(target, "default").unwrap(),
        name: "unsafe tool".to_string(),
        description_preview: String::new(),
        module_path: "/workspace/unsafe.js".to_string(),
        working_directory: "/workspace".to_string(),
        runtime_kind: ExternalToolRuntimeKind::JavaScript,
        capabilities: vec![ExternalToolCapability::FileSystem],
        content_version: "sha256:v1".to_string(),
        static_status: ExternalToolStaticStatus::Ready,
    };

    assert!(tool.validate().is_err());
    tool.name = "safe_tool-1".to_string();
    tool.validate()
        .expect("portable tool name should be accepted");
}

#[test]
fn tool_approval_is_stable_for_safe_updates_but_changes_with_capabilities_or_domain() {
    let target = SourceQualifiedToolTargetId::new(
        SourceKey::new("opencode.tools", "project-tools").unwrap(),
        "weather.js",
    )
    .unwrap();
    let first = external_tool_approval_key(
        "local-user",
        &target,
        ExternalToolRuntimeKind::JavaScript,
        [
            ExternalToolCapability::FileSystem,
            ExternalToolCapability::Network,
        ],
    );
    let reordered = external_tool_approval_key(
        "local-user",
        &target,
        ExternalToolRuntimeKind::JavaScript,
        [
            ExternalToolCapability::Network,
            ExternalToolCapability::FileSystem,
        ],
    );
    let expanded = external_tool_approval_key(
        "local-user",
        &target,
        ExternalToolRuntimeKind::JavaScript,
        [
            ExternalToolCapability::FileSystem,
            ExternalToolCapability::Network,
            ExternalToolCapability::Process,
        ],
    );
    let remote = external_tool_approval_key(
        "remote-user",
        &target,
        ExternalToolRuntimeKind::JavaScript,
        [
            ExternalToolCapability::FileSystem,
            ExternalToolCapability::Network,
        ],
    );

    assert_eq!(first, reordered);
    assert_ne!(first, expanded);
    assert_ne!(first, remote);
}

#[test]
fn tool_conflict_choice_is_invalidated_when_name_or_candidate_changes() {
    let first = external_tool_conflict_key(
        "local-user",
        "weather",
        [
            ("builtin:weather", "builtin-v1"),
            ("opencode:weather", "tool-v1"),
        ],
    );
    let reordered = external_tool_conflict_key(
        "local-user",
        "WEATHER",
        [
            ("opencode:weather", "tool-v1"),
            ("builtin:weather", "builtin-v1"),
        ],
    );
    let updated = external_tool_conflict_key(
        "local-user",
        "weather",
        [
            ("builtin:weather", "builtin-v1"),
            ("opencode:weather", "tool-v2"),
        ],
    );

    assert_ne!(first, reordered);
    assert_ne!(first, updated);
}

#[test]
fn external_mcp_contract_keeps_runtime_secrets_out_of_static_snapshots() {
    let source = source("opencode.mcp", "opencode", "project-config");
    let definition = ExternalMcpServerDefinition {
        id: SourceQualifiedMcpServerId::new(source.key.clone(), "github").unwrap(),
        provenance: vec![source.key.clone()],
        name: "github".to_string(),
        transport: ExternalMcpTransportKind::StreamableHttp,
        command_preview: None,
        argument_count: 0,
        working_directory: None,
        environment_keys: Vec::new(),
        environment_reference_names: Vec::new(),
        remote_url_preview: Some("https://mcp.example.com/mcp".to_string()),
        header_names: vec!["Authorization".to_string()],
        source_enabled: true,
        behavior_version: "sha256:behavior-v1".to_string(),
        static_status: ExternalMcpStaticStatus::Ready,
    };
    let provider =
        ExternalMcpProviderIdentity::new("opencode.mcp", "opencode", "OpenCode MCP servers")
            .unwrap();
    let snapshot = ExternalMcpProviderSnapshot {
        provider,
        sources: vec![source],
        servers: vec![definition.clone()],
        diagnostics: Vec::new(),
    };

    snapshot.validate().expect("valid MCP provider snapshot");
    let encoded = serde_json::to_string(&snapshot).expect("serialize MCP snapshot");
    assert!(encoded.contains("Authorization"));
    assert!(!encoded.contains("Bearer secret"));
    assert!(encoded.contains("mcp.example.com"));

    let prepared = PreparedExternalMcpServer {
        id: definition.id,
        behavior_version: definition.behavior_version,
        transport: PreparedExternalMcpTransport::Remote {
            url: "https://mcp.example.com/mcp?token=url-secret".to_string(),
            headers: [(
                "Authorization".to_string(),
                SecretValue::new("Bearer secret"),
            )]
            .into_iter()
            .collect(),
            oauth_enabled: true,
        },
    };
    assert_eq!(
        prepared.transport.remote_headers().unwrap()["Authorization"].expose(),
        "Bearer secret"
    );
    assert!(!format!("{prepared:?}").contains("Bearer secret"));
    assert!(!format!("{prepared:?}").contains("url-secret"));
}

#[test]
fn external_mcp_snapshot_rejects_cross_provider_and_duplicate_servers() {
    let provider =
        ExternalMcpProviderIdentity::new("opencode.mcp", "opencode", "OpenCode MCP").unwrap();
    let source = source("opencode.mcp", "opencode", "project-config");
    let definition = ExternalMcpServerDefinition {
        id: SourceQualifiedMcpServerId::new(source.key.clone(), "github").unwrap(),
        provenance: vec![source.key.clone()],
        name: "github".to_string(),
        transport: ExternalMcpTransportKind::LocalStdio,
        command_preview: Some("npx".to_string()),
        argument_count: 2,
        working_directory: Some("/workspace".to_string()),
        environment_keys: vec!["GITHUB_TOKEN".to_string()],
        environment_reference_names: Vec::new(),
        remote_url_preview: None,
        header_names: Vec::new(),
        source_enabled: true,
        behavior_version: "sha256:behavior-v1".to_string(),
        static_status: ExternalMcpStaticStatus::Ready,
    };
    let snapshot = ExternalMcpProviderSnapshot {
        provider,
        sources: vec![source],
        servers: vec![definition.clone(), definition],
        diagnostics: Vec::new(),
    };

    assert!(snapshot.validate().is_err());

    let input = ExternalMcpDiscoveryInput {
        context: context(),
        suppressed_sources: [SourceKey::new("opencode.mcp", "suppressed").unwrap()]
            .into_iter()
            .collect(),
    };
    assert_eq!(input.suppressed_sources.len(), 1);
}

#[test]
fn external_mcp_decisions_change_only_with_behavior_domain_or_conflict_participants() {
    let id = SourceQualifiedMcpServerId::new(
        SourceKey::new("opencode.mcp", "project-config").unwrap(),
        "github",
    )
    .unwrap();
    let first = external_mcp_approval_key("local-user", "/workspace-a", &id, "behavior-v1");
    let same = external_mcp_approval_key("local-user", "/workspace-a", &id, "behavior-v1");
    let updated = external_mcp_approval_key("local-user", "/workspace-a", &id, "behavior-v2");
    let other_workspace =
        external_mcp_approval_key("local-user", "/workspace-b", &id, "behavior-v1");
    let remote = external_mcp_approval_key("remote-user", "/workspace-a", &id, "behavior-v1");
    assert_eq!(first, same);
    assert_ne!(first, updated);
    assert_ne!(first, other_workspace);
    assert_ne!(first, remote);

    let stable_id = id.stable_key();
    let conflict = external_mcp_conflict_key(
        "local-user",
        "/workspace-a",
        "github",
        [
            ("bitfun:github", "native-v1"),
            (stable_id.as_str(), "behavior-v1"),
        ],
    );
    let reordered = external_mcp_conflict_key(
        "local-user",
        "/workspace-a",
        "GITHUB",
        [
            (stable_id.as_str(), "behavior-v1"),
            ("bitfun:github", "native-v1"),
        ],
    );
    let participant_updated = external_mcp_conflict_key(
        "local-user",
        "/workspace-a",
        "github",
        [
            ("bitfun:github", "native-v1"),
            (stable_id.as_str(), "behavior-v2"),
        ],
    );
    assert_eq!(conflict, reordered);
    assert_ne!(conflict, participant_updated);
    assert_ne!(
        conflict,
        external_mcp_conflict_key(
            "local-user",
            "/workspace-b",
            "github",
            [
                ("bitfun:github", "native-v1"),
                (stable_id.as_str(), "behavior-v1"),
            ],
        )
    );
}

#[test]
fn external_mcp_product_view_is_version_guarded_and_contains_only_disclosed_fields() {
    let source = source("opencode.mcp", "opencode", "project-config");
    let definition = ExternalMcpServerDefinition {
        id: SourceQualifiedMcpServerId::new(source.key.clone(), "github").unwrap(),
        provenance: vec![source.key],
        name: "github".to_string(),
        transport: ExternalMcpTransportKind::LocalStdio,
        command_preview: Some("npx".to_string()),
        argument_count: 2,
        working_directory: Some("<workspace>".to_string()),
        environment_keys: vec!["GITHUB_TOKEN".to_string()],
        environment_reference_names: Vec::new(),
        remote_url_preview: None,
        header_names: Vec::new(),
        source_enabled: true,
        behavior_version: "sha256:behavior-v1".to_string(),
        static_status: ExternalMcpStaticStatus::Ready,
    };
    let entry = ExternalMcpCatalogEntry {
        candidate_id: definition.candidate_id(),
        definition: definition.clone(),
        approval_key: "external_mcp_approval:local-user:v1".to_string(),
        decision_key: "external_mcp_approval:local-user:v1".to_string(),
        runtime_id: None,
        activation_state: ExternalMcpActivationState::ApprovalRequired,
    };
    let request = ExternalMcpApprovalRequest {
        candidate_id: entry.candidate_id.clone(),
        approval_key: entry.approval_key.clone(),
        decision_key: entry.decision_key.clone(),
        definition,
    };
    let conflict = ExternalMcpConflict {
        conflict_key: "external_mcp:local-user:github:v1".to_string(),
        server_name: "github".to_string(),
        candidates: vec![
            ExternalMcpConflictCandidate {
                candidate_id: "native_mcp:github".to_string(),
                display_name: "BitFun: github".to_string(),
                external: false,
                source: None,
                behavior_version: "native-v1".to_string(),
                available: true,
                unavailable_reason: None,
            },
            ExternalMcpConflictCandidate {
                candidate_id: entry.candidate_id.clone(),
                display_name: "OpenCode: github".to_string(),
                external: true,
                source: Some(entry.definition.id.source.clone()),
                behavior_version: entry.definition.behavior_version.clone(),
                available: true,
                unavailable_reason: None,
            },
        ],
        selected_candidate_id: None,
    };

    let encoded = serde_json::to_string(&(entry, request, conflict)).unwrap();
    assert!(encoded.contains("GITHUB_TOKEN"));
    assert!(!encoded.contains("Bearer secret"));
    assert!(encoded.contains("approval_required"));
}

fn external_capability(value: &str) -> ExternalIntegrationCapabilityId {
    ExternalIntegrationCapabilityId::new(value).expect("valid external capability id")
}

const TEST_ECOSYSTEM_ID: &str = "test-ecosystem";
const EXTERNAL_CAPABILITY_COMMAND: &str = "command";
const EXTERNAL_CAPABILITY_TOOL: &str = "tool";
const EXTERNAL_CAPABILITY_SUBAGENT: &str = "subagent";
const EXTERNAL_CAPABILITY_MCP: &str = "mcp";

fn test_external_integration_ecosystems() -> Vec<ExternalIntegrationEcosystemDescriptor> {
    let capability =
        |id, recommended_access, safety_ceiling| ExternalIntegrationCapabilityDescriptor {
            capability_id: external_capability(id),
            recommended_access,
            safety_ceiling,
        };
    vec![ExternalIntegrationEcosystemDescriptor {
        ecosystem_id: EcosystemId::new(TEST_ECOSYSTEM_ID).unwrap(),
        display_name: "Test ecosystem".to_string(),
        adapter_revision: "1".to_string(),
        capabilities: vec![
            capability(
                EXTERNAL_CAPABILITY_COMMAND,
                ExternalIntegrationAccess::Auto,
                ExternalIntegrationAccess::Auto,
            ),
            capability(
                EXTERNAL_CAPABILITY_TOOL,
                ExternalIntegrationAccess::AskBeforeUse,
                ExternalIntegrationAccess::AskBeforeUse,
            ),
            capability(
                EXTERNAL_CAPABILITY_SUBAGENT,
                ExternalIntegrationAccess::AskBeforeUse,
                ExternalIntegrationAccess::AskBeforeUse,
            ),
            capability(
                EXTERNAL_CAPABILITY_MCP,
                ExternalIntegrationAccess::AskBeforeUse,
                ExternalIntegrationAccess::AskBeforeUse,
            ),
        ],
    }]
}

#[test]
fn recommended_external_integration_policy_is_low_friction_and_fail_closed() {
    let effective = evaluate_external_integration_policy(
        &ExternalIntegrationPolicyDocument::default(),
        Some("workspace-a"),
        &test_external_integration_ecosystems(),
    )
    .expect("default policy evaluates");
    let opencode = effective
        .ecosystems
        .get(&EcosystemId::new(TEST_ECOSYSTEM_ID).unwrap())
        .expect("test ecosystem is registered");

    assert_eq!(opencode.mode, ExternalIntegrationMode::Recommended);
    assert_eq!(
        opencode.capabilities[&external_capability(EXTERNAL_CAPABILITY_COMMAND)],
        ExternalIntegrationAccess::Auto
    );
    for capability in [
        EXTERNAL_CAPABILITY_TOOL,
        EXTERNAL_CAPABILITY_SUBAGENT,
        EXTERNAL_CAPABILITY_MCP,
    ] {
        assert_eq!(
            opencode.capabilities[&external_capability(capability)],
            ExternalIntegrationAccess::AskBeforeUse
        );
    }
}

#[test]
fn workspace_policy_overrides_only_the_fields_the_user_changed() {
    let ecosystem = EcosystemId::new(TEST_ECOSYSTEM_ID).unwrap();
    let mut document = ExternalIntegrationPolicyDocument::default();
    document.user_defaults.ecosystems.insert(
        ecosystem.clone(),
        ExternalEcosystemPolicy {
            mode: ExternalIntegrationMode::DiscoverOnly,
            ..ExternalEcosystemPolicy::default()
        },
    );
    document.workspace_overrides.insert(
        "workspace-a".to_string(),
        ExternalIntegrationPolicyOverride {
            ecosystems: [(
                ecosystem.clone(),
                ExternalEcosystemPolicyOverride {
                    mode: Some(ExternalIntegrationMode::Custom),
                    capability_overrides: [(
                        external_capability(EXTERNAL_CAPABILITY_COMMAND),
                        ExternalIntegrationAccess::Auto,
                    )]
                    .into_iter()
                    .collect(),
                    ..ExternalEcosystemPolicyOverride::default()
                },
            )]
            .into_iter()
            .collect(),
            ..ExternalIntegrationPolicyOverride::default()
        },
    );

    let effective = evaluate_external_integration_policy(
        &document,
        Some("workspace-a"),
        &test_external_integration_ecosystems(),
    )
    .unwrap();
    let opencode = &effective.ecosystems[&ecosystem];
    assert_eq!(opencode.mode, ExternalIntegrationMode::Custom);
    assert_eq!(
        opencode.capabilities[&external_capability(EXTERNAL_CAPABILITY_COMMAND)],
        ExternalIntegrationAccess::Auto
    );
    assert_eq!(
        opencode.capabilities[&external_capability(EXTERNAL_CAPABILITY_MCP)],
        ExternalIntegrationAccess::DiscoverOnly
    );

    let inherited = evaluate_external_integration_policy(
        &document,
        Some("workspace-b"),
        &test_external_integration_ecosystems(),
    )
    .unwrap();
    assert_eq!(
        inherited.ecosystems[&ecosystem].mode,
        ExternalIntegrationMode::DiscoverOnly
    );
}

#[test]
fn high_risk_auto_access_is_limited_by_the_capability_owner() {
    let ecosystem = EcosystemId::new(TEST_ECOSYSTEM_ID).unwrap();
    let mcp = external_capability(EXTERNAL_CAPABILITY_MCP);
    let mut document = ExternalIntegrationPolicyDocument::default();
    document.user_defaults.ecosystems.insert(
        ecosystem.clone(),
        ExternalEcosystemPolicy {
            mode: ExternalIntegrationMode::Custom,
            capability_overrides: [(mcp.clone(), ExternalIntegrationAccess::Auto)]
                .into_iter()
                .collect(),
            ..ExternalEcosystemPolicy::default()
        },
    );

    let effective = evaluate_external_integration_policy(
        &document,
        None,
        &test_external_integration_ecosystems(),
    )
    .unwrap();
    let opencode = &effective.ecosystems[&ecosystem];
    assert_eq!(
        opencode.capabilities[&mcp],
        ExternalIntegrationAccess::AskBeforeUse
    );
    assert!(opencode.policy_limited_capabilities.contains(&mcp));
}

#[test]
fn future_policy_values_and_minor_fields_survive_read_modify_write() {
    let raw = serde_json::json!({
        "schemaMajor": 1,
        "userDefaults": {
            "enabled": true,
            "ecosystems": {
                "opencode": {
                    "mode": "future_mode",
                    "capabilityOverrides": {
                        "future-capability": "future_access"
                    },
                    "futureEcosystemField": { "enabled": true }
                }
            },
            "futureSettingsField": "preserve-me"
        },
        "workspaceOverrides": {},
        "futureDocumentField": [1, 2, 3]
    });
    let mut document: ExternalIntegrationPolicyDocument =
        serde_json::from_value(raw.clone()).expect("future minor data remains readable");
    document.user_defaults.enabled = false;
    let encoded = serde_json::to_value(&document).expect("policy remains serializable");

    assert_eq!(
        encoded["userDefaults"]["ecosystems"]["opencode"]["mode"],
        "future_mode"
    );
    assert_eq!(
        encoded["userDefaults"]["ecosystems"]["opencode"]["capabilityOverrides"]
            ["future-capability"],
        "future_access"
    );
    assert_eq!(
        encoded["userDefaults"]["ecosystems"]["opencode"]["futureEcosystemField"],
        raw["userDefaults"]["ecosystems"]["opencode"]["futureEcosystemField"]
    );
    assert_eq!(
        encoded["userDefaults"]["futureSettingsField"],
        "preserve-me"
    );
    assert_eq!(encoded["futureDocumentField"], raw["futureDocumentField"]);

    let effective = evaluate_external_integration_policy(
        &document,
        None,
        &test_external_integration_ecosystems(),
    )
    .unwrap();
    assert!(!effective.enabled);
}

#[test]
fn incompatible_policy_schema_major_is_rejected_without_downgrade() {
    let document = ExternalIntegrationPolicyDocument {
        schema_major: 2,
        ..ExternalIntegrationPolicyDocument::default()
    };
    let error = evaluate_external_integration_policy(
        &document,
        None,
        &test_external_integration_ecosystems(),
    )
    .expect_err("future major schemas must fail closed");
    assert!(error.to_string().contains("schema major: 2"));
}

#[test]
fn incompatible_policy_schema_has_a_safe_read_only_public_snapshot() {
    let raw = serde_json::json!({
        "schemaMajor": 2,
        "userDefaults": {
            "enabled": true,
            "futureSecretHostField": "persistence-only"
        },
        "futureDocumentField": { "keep": true }
    });
    let document: ExternalIntegrationPolicyDocument = serde_json::from_value(raw).unwrap();
    let snapshot = external_integration_policy_snapshot(
        &document,
        Some("workspace-a"),
        test_external_integration_ecosystems(),
    )
    .expect("incompatible schemas remain inspectable through a safe snapshot");

    assert_eq!(
        snapshot.status,
        ExternalIntegrationPolicyStatus::IncompatibleSchema
    );
    assert!(!snapshot.global_effective.enabled);
    assert!(!snapshot.effective.enabled);
    assert!(snapshot
        .effective
        .ecosystems
        .values()
        .all(|ecosystem| ecosystem
            .capabilities
            .values()
            .all(|access| { matches!(access, ExternalIntegrationAccess::Disabled) })));

    let public = serde_json::to_string(&snapshot).unwrap();
    assert!(!public.contains("futureSecretHostField"));
    assert!(!public.contains("futureDocumentField"));

    let persisted = serde_json::to_string(&document).unwrap();
    assert!(persisted.contains("futureSecretHostField"));
    assert!(persisted.contains("futureDocumentField"));
}

#[test]
fn integration_registry_rejects_ambiguous_or_unsafe_descriptors() {
    let mut duplicate_ecosystem = test_external_integration_ecosystems();
    duplicate_ecosystem.push(duplicate_ecosystem[0].clone());
    let duplicate_error = evaluate_external_integration_policy(
        &ExternalIntegrationPolicyDocument::default(),
        None,
        &duplicate_ecosystem,
    )
    .expect_err("duplicate ecosystem registrations must fail closed");
    assert!(duplicate_error.to_string().contains("duplicate ecosystem"));

    let mut unsafe_recommendation = test_external_integration_ecosystems();
    unsafe_recommendation[0].capabilities[1].recommended_access = ExternalIntegrationAccess::Auto;
    let unsafe_error = evaluate_external_integration_policy(
        &ExternalIntegrationPolicyDocument::default(),
        None,
        &unsafe_recommendation,
    )
    .expect_err("registry defaults cannot exceed their safety ceiling");
    assert!(unsafe_error
        .to_string()
        .contains("exceeds the safety ceiling"));
}

#[test]
fn public_snapshot_never_exposes_executable_prompt_templates() {
    let snapshot = ExternalSourceCatalogSnapshot {
        generation: 1,
        discovery_pending: false,
        sources: Vec::new(),
        commands: vec![PromptCommandCatalogEntry {
            definition: command("opencode", "project-commands", 1),
        }],
        command_conflicts: Vec::new(),
        tools: Vec::new(),
        tool_approval_requests: Vec::new(),
        tool_conflicts: Vec::new(),
        mcp_generation: 0,
        mcp_servers: Vec::new(),
        mcp_approval_requests: Vec::new(),
        mcp_conflicts: Vec::new(),
        subagent_generation: 0,
        preference_revision: 0,
        subagents: Vec::new(),
        subagent_conflicts: Vec::new(),
        pending_subagent_approvals: Vec::new(),
        integration_policy: Default::default(),
        diagnostics: Vec::new(),
    };

    let public = ExternalSourcePublicSnapshot::from(snapshot);
    let encoded = serde_json::to_value(public).expect("serialize public projection");

    assert_eq!(encoded["commands"][0]["definition"]["name"], "review");
    assert!(encoded["commands"][0]["definition"]
        .get("template")
        .is_none());
}

#[test]
fn control_projection_keeps_lifecycle_facts_orthogonal() {
    let catalog = ExternalSourceCatalogSnapshot {
        generation: 7,
        discovery_pending: false,
        sources: vec![ExternalSourceCatalogEntry {
            stable_key: "opencode.commands:project".to_string(),
            presentation_group_id: None,
            record: source("opencode.commands", "opencode", "project"),
            lifecycle: ExternalSourceLifecycleState::UsingLastValidVersion,
        }],
        commands: vec![PromptCommandCatalogEntry {
            definition: command("opencode.commands", "project", 1),
        }],
        command_conflicts: Vec::new(),
        tools: Vec::new(),
        tool_approval_requests: Vec::new(),
        tool_conflicts: Vec::new(),
        mcp_generation: 2,
        mcp_servers: Vec::new(),
        mcp_approval_requests: Vec::new(),
        mcp_conflicts: Vec::new(),
        subagent_generation: 3,
        preference_revision: 11,
        subagents: Vec::new(),
        subagent_conflicts: Vec::new(),
        pending_subagent_approvals: Vec::new(),
        integration_policy: Default::default(),
        diagnostics: Vec::new(),
    };

    let control = ExternalSourceControlSnapshotV1::from_catalog(
        &catalog,
        ExecutionDomainId::new("local-user").unwrap(),
        false,
        ExternalSourceHostCapabilities::read_write(),
    );

    assert_eq!(control.schema_version, EXTERNAL_SOURCE_CONTROL_SCHEMA_V1);
    assert_eq!(control.refresh_generation, 7);
    assert_eq!(control.preference_revision, 11);
    assert_eq!(control.sources.len(), 1);
    assert_eq!(
        control.sources[0].discovery,
        ExternalSourceDiscoveryState::LastKnownGood
    );
    assert_eq!(
        control.sources[0].desired,
        ExternalSourceDesiredState::Enabled
    );
    assert_eq!(
        control.sources[0].review,
        ExternalSourceReviewState::NotRequired
    );
    assert_eq!(control.capabilities.len(), 4);
}

#[test]
fn control_projection_does_not_infer_review_facts_from_runtime_activation() {
    let record = source("opencode.mcp", "opencode", "project-config");
    let definition = ExternalMcpServerDefinition {
        id: SourceQualifiedMcpServerId::new(record.key.clone(), "docs").unwrap(),
        provenance: vec![record.key.clone()],
        name: "docs".to_string(),
        transport: ExternalMcpTransportKind::StreamableHttp,
        command_preview: None,
        argument_count: 0,
        working_directory: None,
        environment_keys: Vec::new(),
        environment_reference_names: Vec::new(),
        remote_url_preview: Some("https://mcp.example.com".to_string()),
        header_names: Vec::new(),
        source_enabled: true,
        behavior_version: "behavior-v1".to_string(),
        static_status: ExternalMcpStaticStatus::Ready,
    };
    let catalog = ExternalSourceCatalogSnapshot {
        generation: 1,
        discovery_pending: false,
        sources: vec![ExternalSourceCatalogEntry {
            stable_key: "opencode.mcp:project-config".to_string(),
            presentation_group_id: None,
            record: record.clone(),
            lifecycle: ExternalSourceLifecycleState::Available,
        }],
        commands: Vec::new(),
        command_conflicts: Vec::new(),
        tools: Vec::new(),
        tool_approval_requests: Vec::new(),
        tool_conflicts: Vec::new(),
        mcp_generation: 1,
        mcp_servers: vec![ExternalMcpCatalogEntry {
            candidate_id: "external_mcp:docs".to_string(),
            definition,
            approval_key: "approval-v1".to_string(),
            decision_key: "decision-v1".to_string(),
            runtime_id: None,
            activation_state: ExternalMcpActivationState::Declined,
        }],
        mcp_approval_requests: Vec::new(),
        mcp_conflicts: Vec::new(),
        subagent_generation: 1,
        preference_revision: 1,
        subagents: Vec::new(),
        subagent_conflicts: Vec::new(),
        pending_subagent_approvals: Vec::new(),
        integration_policy: Default::default(),
        diagnostics: Vec::new(),
    };

    let control = ExternalSourceControlSnapshotV1::from_catalog(
        &catalog,
        ExecutionDomainId::new("local-user").unwrap(),
        false,
        ExternalSourceHostCapabilities::read_write(),
    );

    assert_eq!(
        control.sources[0].review,
        ExternalSourceReviewState::NotRequired
    );
}

#[test]
fn desktop_local_host_capability_is_additive_on_the_wire() {
    let portable = serde_json::to_value(ExternalSourceHostCapabilities::read_write()).unwrap();
    let read_only =
        serde_json::to_value(ExternalSourceHostCapabilities::read_only_projection()).unwrap();
    let desktop = serde_json::to_value(ExternalSourceHostCapabilities::local_desktop()).unwrap();

    assert!(portable.get("canRevealSourceLocation").is_none());
    assert!(read_only.get("canRevealSourceLocation").is_none());
    assert_eq!(desktop["canRevealSourceLocation"], true);

    let legacy: ExternalSourceHostCapabilities = serde_json::from_value(serde_json::json!({
        "canRefresh": true,
        "canMutatePolicy": true,
        "canManageSources": true,
        "canApproveRuntime": true,
        "canExecuteExternalAssets": true,
        "canSetSafeMode": true
    }))
    .unwrap();
    assert!(!legacy.can_reveal_source_location);
}

#[test]
fn operation_error_round_trip_preserves_typed_recovery_without_message_parsing() {
    let error = ExternalSourceOperationError::new(
        ExternalSourceOperationErrorCode::StaleRevision,
        "refresh required",
        true,
    )
    .with_stage(ExternalSourceOperationStage::ApplyPreference)
    .with_causation_id("refresh-generation-7")
    .with_recovery_action(ExternalSourceRecoveryActionV1::Refresh);

    let encoded = error.encode();
    assert_eq!(ExternalSourceOperationError::decode(&encoded), Some(error));
    assert!(!encoded.contains("metadata"));
}

#[test]
fn control_action_uses_one_camel_case_dto_across_product_surfaces() {
    let request = ExternalSourceControlRequestV1 {
        schema_version: EXTERNAL_SOURCE_CONTROL_SCHEMA_V1,
        operation_id: "surface-operation-1".to_string(),
        expected_preference_revision: Some(8),
        action: ExternalSourceControlActionV1::SetSourceEnabled {
            source_key: "opencode.commands:project".to_string(),
            enabled: false,
        },
    };

    let encoded = serde_json::to_value(&request).expect("serialize control request");
    assert_eq!(encoded["schemaVersion"], 1);
    assert_eq!(encoded["operationId"], "surface-operation-1");
    assert_eq!(encoded["expectedPreferenceRevision"], 8);
    assert_eq!(encoded["action"]["type"], "set_source_enabled");
    assert_eq!(encoded["action"]["sourceKey"], "opencode.commands:project");
    assert!(encoded["action"].get("source_key").is_none());
    assert_eq!(
        serde_json::from_value::<ExternalSourceControlRequestV1>(encoded)
            .expect("deserialize the shared control request"),
        request
    );
}

#[test]
fn legacy_operation_errors_decode_with_empty_extension_fields() {
    let decoded = ExternalSourceOperationError::decode(
        r#"{"code":"unavailable","detail":"retry","retryable":true}"#,
    )
    .expect("legacy operation error remains readable");

    assert_eq!(decoded.code, ExternalSourceOperationErrorCode::Unavailable);
    assert!(decoded.stage.is_none());
    assert!(decoded.causation_id.is_none());
    assert!(decoded.recovery_actions.is_empty());
}

#[test]
fn decoded_operation_errors_bound_untrusted_extension_fields() {
    let oversized = "x".repeat(5000);
    let encoded = serde_json::json!({
        "code": "stale_revision",
        "detail": oversized,
        "retryable": true,
        "correlationId": "forged\nreference",
        "recoveryActions": [
            { "type": "refresh" },
            { "type": "refresh" },
            { "type": "retry" }
        ]
    })
    .to_string();

    let decoded = ExternalSourceOperationError::decode(&encoded).unwrap();
    assert_eq!(decoded.detail.chars().count(), 4096);
    assert!(decoded.correlation_id.is_none());
    assert_eq!(
        decoded.recovery_actions,
        vec![
            ExternalSourceRecoveryActionV1::Refresh,
            ExternalSourceRecoveryActionV1::Retry,
        ]
    );
}
