use bitfun_product_domains::external_sources::{
    external_tool_approval_key, external_tool_conflict_key, prompt_command_conflict_key,
    EcosystemId, ExecutionDomainId, ExpandedPromptCommand, ExternalSourceContext,
    ExternalSourceDiagnostic, ExternalSourceHealth, ExternalSourceProviderError,
    ExternalSourceRecord, ExternalSourceScope, ExternalToolCapability, ExternalToolDefinition,
    ExternalToolRuntimeKind, ExternalToolStaticStatus, ExternalWatchRoot,
    PromptCommandAvailability, PromptCommandDefinition, PromptCommandProviderIdentity,
    PromptCommandProviderSnapshot, PromptCommandSourceProvider, SourceKey,
    SourceQualifiedCommandId, SourceQualifiedToolId, SourceQualifiedToolTargetId,
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
