use bitfun_opencode_adapter::{OpenCodeSubagentProvider, OpenCodeSubagentProviderOptions};
use bitfun_product_domains::external_sources::{
    ExecutionDomainId, ExternalSourceContext, ExternalSourceScope,
};
use bitfun_product_domains::external_subagents::{
    ExternalSubagentCompatibilityState, ExternalSubagentDiscoveryInput, ExternalSubagentMode,
    ExternalSubagentModelRequest, ExternalSubagentSourceProvider,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn context(workspace: PathBuf) -> ExternalSourceContext {
    ExternalSourceContext {
        workspace_root: Some(workspace),
        execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
    }
}

fn provider(temp: &TempDir, workspace: &std::path::Path) -> OpenCodeSubagentProvider {
    OpenCodeSubagentProvider::new(OpenCodeSubagentProviderOptions {
        user_config_dir: temp.path().join("user"),
        legacy_user_config_dir: Some(temp.path().join("legacy")),
        explicit_config_file: None,
        explicit_config_dir: None,
        project_config_enabled: true,
        project_root_override: Some(workspace.to_path_buf()),
    })
}

fn discover(
    provider: &OpenCodeSubagentProvider,
    workspace: PathBuf,
    suppressed_sources: BTreeSet<bitfun_product_domains::external_sources::SourceKey>,
) -> bitfun_product_domains::external_subagents::ExternalSubagentProviderSnapshot {
    provider
        .discover(&ExternalSubagentDiscoveryInput {
            context: context(workspace),
            suppressed_sources,
        })
        .expect("discover OpenCode agents")
}

#[test]
fn global_and_project_agent_fields_deep_merge_with_ordered_provenance() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "review": {
              "description": "Global review",
              "prompt": "Review using the global policy",
              "mode": "subagent",
              "model": "openrouter/anthropic/claude-sonnet-4",
              "tools": { "read": true, "grep": false }
            }
          }
        }"#,
    )
    .unwrap();
    fs::write(
        workspace.join("opencode.jsonc"),
        r#"{
          // Project copy must not alter execution behavior.
          "agent": { "review": { "description": "Project review", "color": "blue" } }
        }"#,
    )
    .unwrap();

    let provider = provider(&temp, &workspace);
    let first = discover(&provider, workspace.clone(), BTreeSet::new());
    let definition = &first.definitions[0];
    assert_eq!(definition.logical_id, "review");
    assert_eq!(definition.description, "Project review");
    assert_eq!(definition.prompt.expose(), "Review using the global policy");
    assert_eq!(definition.provenance.len(), 2);
    assert_eq!(definition.mode, ExternalSubagentMode::Subagent);
    assert_eq!(
        definition.requested_model,
        ExternalSubagentModelRequest::Exact {
            provider_hint: Some("openrouter".to_string()),
            model_name: "anthropic/claude-sonnet-4".to_string(),
        }
    );
    assert_eq!(definition.requested_tools.selectors.len(), 2);
    assert_eq!(
        definition.compatibility,
        ExternalSubagentCompatibilityState::ReadyWithDegradation
    );
    let behavior = definition.behavior_version.clone();

    fs::write(
        workspace.join("opencode.jsonc"),
        r#"{ "agent": { "review": { "description": "Project review updated", "color": "red" } } }"#,
    )
    .unwrap();
    let updated = discover(&provider, workspace.clone(), BTreeSet::new());
    assert_eq!(updated.definitions[0].behavior_version, behavior);
    assert_eq!(updated.definitions[0].description, "Project review updated");
    fs::write(
        workspace.join("opencode.jsonc"),
        r#"{ "agent": { "review": { "description": "Project review updated" } } }"#,
    )
    .unwrap();
    let color_removed = discover(&provider, workspace.clone(), BTreeSet::new());
    assert_eq!(color_removed.definitions[0].behavior_version, behavior);

    let project_source = updated
        .sources
        .iter()
        .find(|source| source.scope == ExternalSourceScope::Project)
        .unwrap()
        .key
        .clone();
    let without_project = discover(&provider, workspace, [project_source].into_iter().collect());
    assert_eq!(without_project.definitions[0].description, "Global review");
    assert_eq!(without_project.definitions[0].provenance.len(), 1);
}

#[test]
fn suppressed_agent_source_remains_discoverable_for_reenable() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "review": {
              "description": "Review agent",
              "prompt": "Review the change",
              "mode": "subagent"
            }
          }
        }"#,
    )
    .unwrap();

    let provider = provider(&temp, &workspace);
    let initial = discover(&provider, workspace.clone(), BTreeSet::new());
    let source_key = initial.sources[0].key.clone();
    fs::write(temp.path().join("user/opencode.json"), "{ invalid").unwrap();
    let suppressed = discover(
        &provider,
        workspace,
        [source_key.clone()].into_iter().collect(),
    );

    assert!(suppressed.definitions.is_empty());
    assert_eq!(suppressed.sources.len(), 1);
    assert_eq!(suppressed.sources[0].key, source_key);
}

#[test]
fn safe_subset_is_fail_closed_and_default_tools_are_explicit() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "permission": { "bash": "deny" },
          "agent": {
            "defaulted": { "prompt": "Use safe defaults", "mode": "subagent" },
            "unsafe": {
              "prompt": "do-not-leak-this-prompt",
              "mode": "subagent",
              "permission": { "bash": "allow" },
              "options": { "providerSecret": "do-not-leak-value" },
              "futureField": "do-not-leak-unknown"
            },
            "wrongType": { "prompt": 42, "mode": "subagent" },
            "primaryOnly": { "prompt": "Primary", "mode": "primary" },
            "sampling": { "prompt": "Sampling", "temperature": 0.2, "color": "blue" }
          }
        }"#,
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    let find = |id: &str| {
        snapshot
            .definitions
            .iter()
            .find(|item| item.logical_id == id)
            .unwrap()
    };
    let defaulted = find("defaulted");
    assert_eq!(
        defaulted
            .requested_tools
            .selectors
            .iter()
            .map(|item| item.canonical_host_name.as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec!["LS", "Read", "Glob", "Grep"]
    );
    assert!(defaulted.requested_tools.uses_conservative_default);
    assert_eq!(
        defaulted.compatibility,
        ExternalSubagentCompatibilityState::Blocked,
        "ambient permission blocks every agent from this source"
    );
    assert_eq!(
        find("unsafe").compatibility,
        ExternalSubagentCompatibilityState::Blocked
    );
    assert_eq!(
        find("wrongType").compatibility,
        ExternalSubagentCompatibilityState::Invalid
    );
    assert_eq!(
        find("primaryOnly").compatibility,
        ExternalSubagentCompatibilityState::Blocked
    );
    assert_eq!(
        find("sampling").compatibility,
        ExternalSubagentCompatibilityState::Blocked
    );
    let debug = format!("{snapshot:?}");
    assert!(!debug.contains("do-not-leak-this-prompt"));
    assert!(!debug.contains("do-not-leak-value"));
    assert!(!debug.contains("do-not-leak-unknown"));
}

#[test]
fn markdown_agent_directories_are_supported_and_legacy_modes_are_visible_but_blocked() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user/agents/review")).unwrap();
    fs::create_dir_all(temp.path().join("user/mode")).unwrap();
    fs::write(
        temp.path().join("user/agents/review/security.md"),
        "---\ndescription: Security review\nmode: subagent\ntools:\n  read: true\n---\nReview security boundaries.",
    )
    .unwrap();
    fs::write(
        temp.path().join("user/mode/legacy.md"),
        "---\ndescription: Legacy primary mode\n---\nAct as a primary agent.",
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    let markdown = snapshot
        .definitions
        .iter()
        .find(|item| item.logical_id == "review/security")
        .unwrap();
    assert_eq!(markdown.description, "Security review");
    assert_eq!(markdown.prompt.expose(), "Review security boundaries.");
    assert_eq!(
        markdown.compatibility,
        ExternalSubagentCompatibilityState::Ready
    );

    let legacy = snapshot
        .definitions
        .iter()
        .find(|item| item.logical_id == "legacy")
        .unwrap();
    assert_eq!(
        legacy.compatibility,
        ExternalSubagentCompatibilityState::Blocked
    );
    assert!(legacy
        .diagnostic_codes
        .contains(&"opencode_legacy_primary_mode_not_imported".to_string()));
}

#[test]
fn missing_prompt_and_native_overlays_are_blocked_not_invalid() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(temp.path().join("user")).unwrap();
    fs::write(
        temp.path().join("user/opencode.json"),
        r#"{
          "agent": {
            "missing": { "description": "Relies on OpenCode defaults" },
            "defaulted": { "prompt": "Use conservative defaults", "mode": "subagent" },
            "Build": { "prompt": "Overlay native build", "mode": "subagent" }
          }
        }"#,
    )
    .unwrap();

    let snapshot = discover(&provider(&temp, &workspace), workspace, BTreeSet::new());
    for id in ["missing", "Build"] {
        assert_eq!(
            snapshot
                .definitions
                .iter()
                .find(|item| item.logical_id == id)
                .unwrap()
                .compatibility,
            ExternalSubagentCompatibilityState::Blocked
        );
    }
    let defaulted = snapshot
        .definitions
        .iter()
        .find(|item| item.logical_id == "defaulted")
        .unwrap();
    assert_eq!(
        defaulted.compatibility,
        ExternalSubagentCompatibilityState::ReadyWithDegradation
    );
    assert_eq!(defaulted.requested_tools.selectors.len(), 4);
}
