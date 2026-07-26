use bitfun_codex_adapter::{CodexSubagentProvider, CodexSubagentProviderOptions};
use bitfun_product_domains::external_sources::{
    ExecutionDomainId, ExternalSourceContext, ExternalSourceScope, SourceKey,
};
use bitfun_product_domains::external_subagents::{
    ExternalSubagentCompatibilityState, ExternalSubagentDiscoveryInput,
    ExternalSubagentModelRequest, ExternalSubagentSourceProvider,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    codex_home: PathBuf,
    project: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let codex_home = temp.path().join("home/.codex");
        let project = temp.path().join("project");
        let workspace = project.join("packages/app");
        fs::create_dir_all(&codex_home).unwrap();
        fs::create_dir_all(project.join(".git")).unwrap();
        fs::create_dir_all(&workspace).unwrap();
        Self {
            _temp: temp,
            codex_home,
            project,
            workspace,
        }
    }

    fn provider(&self) -> CodexSubagentProvider {
        CodexSubagentProvider::new(CodexSubagentProviderOptions {
            codex_home: self.codex_home.clone(),
            project_root_override: Some(self.project.clone()),
            project_config_enabled: true,
        })
    }

    fn context(&self) -> ExternalSourceContext {
        ExternalSourceContext {
            workspace_root: Some(self.workspace.clone()),
            execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
        }
    }

    fn discover(
        &self,
        suppressed_sources: BTreeSet<SourceKey>,
    ) -> bitfun_product_domains::external_subagents::ExternalSubagentProviderSnapshot {
        self.provider()
            .discover(&ExternalSubagentDiscoveryInput {
                context: self.context(),
                suppressed_sources,
            })
            .unwrap()
    }
}

fn write(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

#[test]
fn malformed_required_user_layer_blocks_project_subagent_candidates() {
    let fixture = Fixture::new();
    write(fixture.codex_home.join("config.toml"), "[agents.invalid\n");
    write(
        fixture.project.join(".codex/agents/project.toml"),
        r#"name = "project"
developer_instructions = "Project-only instructions"
"#,
    );

    let error = fixture
        .provider()
        .discover(&ExternalSubagentDiscoveryInput {
            context: fixture.context(),
            suppressed_sources: BTreeSet::new(),
        })
        .unwrap_err();
    assert_eq!(error.code, "codex.agent.config_invalid");
}

#[test]
fn standalone_project_role_overlays_user_role_and_inherits_missing_description() {
    let fixture = Fixture::new();
    write(
        fixture.codex_home.join("config.toml"),
        r#"[agents.researcher]
description = "Research role from user config"
config_file = "./agents/researcher.toml"
nickname_candidates = ["Noether"]
"#,
    );
    write(
        fixture.codex_home.join("agents/researcher.toml"),
        r#"developer_instructions = "Research from user role"
model = "gpt-user"
"#,
    );
    write(
        fixture.project.join(".codex/agents/researcher.toml"),
        r#"name = "researcher"
nickname_candidates = ["Hypatia"]
developer_instructions = "Research from project role"
model = "gpt-project"
"#,
    );

    let snapshot = fixture.discover(BTreeSet::new());
    let definition = &snapshot.definitions[0];
    assert_eq!(definition.logical_id, "researcher");
    assert_eq!(definition.description, "Research role from user config");
    assert_eq!(definition.prompt.expose(), "Research from project role");
    assert_eq!(definition.provenance.len(), 2);
    assert_eq!(
        definition.requested_model,
        ExternalSubagentModelRequest::Exact {
            provider_hint: None,
            model_name: "gpt-project".to_string(),
        }
    );
    assert_eq!(
        definition.compatibility,
        ExternalSubagentCompatibilityState::Ready
    );
}

#[test]
fn nearest_project_config_can_inherit_lower_role_file_by_field() {
    let fixture = Fixture::new();
    write(
        fixture.codex_home.join("config.toml"),
        r#"[agents.reviewer]
description = "User description"
config_file = "./agents/reviewer.toml"
"#,
    );
    write(
        fixture.codex_home.join("agents/reviewer.toml"),
        r#"developer_instructions = "Inherited role instructions"
model = "gpt-inherited"
"#,
    );
    write(
        fixture.workspace.join(".codex/config.toml"),
        r#"[agents.reviewer]
nickname_candidates = ["Atlas"]
"#,
    );

    let snapshot = fixture.discover(BTreeSet::new());
    let definition = &snapshot.definitions[0];
    assert_eq!(definition.description, "User description");
    assert_eq!(definition.prompt.expose(), "Inherited role instructions");
    assert_eq!(definition.provenance.len(), 2);
    assert_eq!(
        definition.requested_model,
        ExternalSubagentModelRequest::Exact {
            provider_hint: None,
            model_name: "gpt-inherited".to_string(),
        }
    );
}

#[test]
fn partial_role_overlays_merge_behavior_fields_independently() {
    let fixture = Fixture::new();
    write(
        fixture.codex_home.join("config.toml"),
        r#"[agents.model_only]
description = "Model overlay"
config_file = "./agents/model_only.toml"

[agents.prompt_only]
description = "Prompt overlay"
config_file = "./agents/prompt_only.toml"
"#,
    );
    write(
        fixture.codex_home.join("agents/model_only.toml"),
        r#"developer_instructions = "Inherited prompt"
model = "gpt-inherited"
"#,
    );
    write(
        fixture.codex_home.join("agents/prompt_only.toml"),
        r#"developer_instructions = "Original prompt"
model = "gpt-inherited"
"#,
    );
    write(
        fixture.project.join(".codex/config.toml"),
        r#"[agents.model_only]
config_file = "./agents/model_only.toml"

[agents.prompt_only]
config_file = "./agents/prompt_only.toml"
"#,
    );
    write(
        fixture.project.join(".codex/agents/model_only.toml"),
        r#"model = "gpt-project""#,
    );
    write(
        fixture.project.join(".codex/agents/prompt_only.toml"),
        r#"developer_instructions = "Project prompt""#,
    );

    let snapshot = fixture.discover(BTreeSet::new());
    let model_only = snapshot
        .definitions
        .iter()
        .find(|definition| definition.logical_id == "model_only")
        .unwrap();
    assert_eq!(model_only.prompt.expose(), "Inherited prompt");
    assert_eq!(
        model_only.requested_model,
        ExternalSubagentModelRequest::Exact {
            provider_hint: None,
            model_name: "gpt-project".to_string(),
        }
    );

    let prompt_only = snapshot
        .definitions
        .iter()
        .find(|definition| definition.logical_id == "prompt_only")
        .unwrap();
    assert_eq!(prompt_only.prompt.expose(), "Project prompt");
    assert_eq!(
        prompt_only.requested_model,
        ExternalSubagentModelRequest::Exact {
            provider_hint: None,
            model_name: "gpt-inherited".to_string(),
        }
    );
}

#[test]
fn unsupported_role_behavior_blocks_without_exposing_instructions() {
    let fixture = Fixture::new();
    write(
        fixture.codex_home.join("agents/reviewer.toml"),
        r#"name = "reviewer"
description = "Review role"
developer_instructions = "SECRET-CODEX-ROLE-PROMPT"
model = "gpt-5"
model_reasoning_effort = "high"
sandbox_mode = "danger-full-access"
[mcp_servers.private]
command = "private-server"
"#,
    );

    let snapshot = fixture.discover(BTreeSet::new());
    let definition = &snapshot.definitions[0];
    assert_eq!(
        definition.compatibility,
        ExternalSubagentCompatibilityState::Blocked
    );
    assert!(definition
        .diagnostic_codes
        .contains(&"codex_agent_reasoning_not_imported".to_string()));
    assert!(definition
        .diagnostic_codes
        .contains(&"codex_agent_sandbox_not_imported".to_string()));
    assert!(definition
        .diagnostic_codes
        .contains(&"codex_agent_mcp_not_imported".to_string()));
    assert!(!format!("{snapshot:?}").contains("SECRET-CODEX-ROLE-PROMPT"));
}

#[test]
fn declared_role_wins_same_layer_duplicate_and_emits_diagnostic() {
    let fixture = Fixture::new();
    write(
        fixture.codex_home.join("config.toml"),
        r#"[agents.reviewer]
description = "Declared"
config_file = "./agents/declared.toml"
"#,
    );
    write(
        fixture.codex_home.join("agents/declared.toml"),
        r#"developer_instructions = "Declared instructions""#,
    );
    write(
        fixture.codex_home.join("agents/duplicate.toml"),
        r#"name = "reviewer"
description = "Discovered duplicate"
developer_instructions = "Duplicate instructions"
"#,
    );

    let snapshot = fixture.discover(BTreeSet::new());
    assert_eq!(snapshot.definitions.len(), 1);
    assert_eq!(snapshot.definitions[0].description, "Declared");
    assert_eq!(
        snapshot.definitions[0].prompt.expose(),
        "Declared instructions"
    );
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "codex.agent.duplicate_name"));
}

#[test]
fn provider_failures_keep_one_stable_diagnostic_namespace() {
    let fixture = Fixture::new();
    write(
        fixture.codex_home.join("config.toml"),
        r#"[agents.reviewer]
description = "Reviewer"
unknown_behavior = true
"#,
    );

    let snapshot = fixture.discover(BTreeSet::new());
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "codex.agent.role_invalid"));
    assert!(!snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code.starts_with("codex.agent.codex.agent.")));
}

#[test]
fn suppression_precedes_native_field_merge_and_display_changes_keep_behavior_version() {
    let fixture = Fixture::new();
    let user_config = fixture.codex_home.join("config.toml");
    write(
        &user_config,
        r#"[agents.reviewer]
description = "First description"
config_file = "./agents/reviewer.toml"
"#,
    );
    write(
        fixture.codex_home.join("agents/reviewer.toml"),
        r#"developer_instructions = "Stable instructions"
model = "gpt-stable"
"#,
    );
    write(
        fixture.project.join(".codex/config.toml"),
        r#"[agents.reviewer]
description = "Project description"
"#,
    );

    let first = fixture.discover(BTreeSet::new());
    let behavior = first.definitions[0].behavior_version.clone();
    let project_source = first
        .sources
        .iter()
        .find(|source| source.scope == ExternalSourceScope::Project)
        .unwrap()
        .key
        .clone();
    let suppressed = fixture.discover([project_source.clone()].into_iter().collect());
    assert_eq!(suppressed.definitions[0].description, "First description");
    assert_eq!(suppressed.definitions[0].provenance.len(), 1);
    assert_eq!(suppressed.definitions[0].behavior_version, behavior);

    write(
        &user_config,
        r#"[agents.reviewer]
description = "Updated display-only description"
config_file = "./agents/reviewer.toml"
"#,
    );
    let updated = fixture.discover([project_source].into_iter().collect());
    assert_eq!(updated.definitions[0].behavior_version, behavior);
}

#[test]
fn watch_roots_cover_codex_home_and_project_codex_directories() {
    let fixture = Fixture::new();
    let roots = fixture.provider().watch_roots(&fixture.context());
    assert!(roots
        .iter()
        .any(|root| root.path == fixture.codex_home && root.recursive));
    assert!(roots
        .iter()
        .any(|root| root.path == fixture.project.join(".codex") && root.recursive));
    assert!(roots
        .iter()
        .any(|root| root.path == fixture.workspace.join(".codex") && root.recursive));
}

#[test]
fn malformed_project_config_does_not_reactivate_a_user_role() {
    let fixture = Fixture::new();
    write(
        fixture.codex_home.join("agents/reviewer.toml"),
        r#"name = "reviewer"
description = "User reviewer"
developer_instructions = "Review as the user role"
"#,
    );
    write(
        fixture.project.join(".codex/config.toml"),
        "[agents.reviewer\n",
    );

    let result = fixture
        .provider()
        .discover(&ExternalSubagentDiscoveryInput {
            context: fixture.context(),
            suppressed_sources: BTreeSet::new(),
        });

    assert!(
        result.is_err(),
        "a broken higher native layer must fail closed"
    );
}

#[test]
fn invalid_declared_overlay_blocks_the_lower_role_with_the_same_name() {
    let fixture = Fixture::new();
    write(
        fixture.codex_home.join("agents/reviewer.toml"),
        r#"name = "reviewer"
description = "User reviewer"
developer_instructions = "Review as the user role"
"#,
    );
    write(
        fixture.project.join(".codex/config.toml"),
        r#"[agents.reviewer]
future_behavior = true
"#,
    );

    let snapshot = fixture.discover(BTreeSet::new());

    assert_eq!(snapshot.definitions.len(), 1);
    assert_eq!(
        snapshot.definitions[0].compatibility,
        ExternalSubagentCompatibilityState::Invalid,
    );
}

#[test]
fn codex_agent_controls_are_not_silently_ignored() {
    let fixture = Fixture::new();
    write(
        fixture.codex_home.join("config.toml"),
        r#"[agents]
enabled = false
default_subagent_model = "gpt-default"
default_subagent_reasoning_effort = "high"

[agents.reviewer]
description = "Reviewer"
config_file = "./agents/reviewer.toml"
"#,
    );
    write(
        fixture.codex_home.join("agents/reviewer.toml"),
        r#"developer_instructions = "Review carefully""#,
    );

    let snapshot = fixture.discover(BTreeSet::new());
    let definition = &snapshot.definitions[0];

    assert!(definition.disabled);
    assert_eq!(
        definition.requested_model,
        ExternalSubagentModelRequest::Exact {
            provider_hint: None,
            model_name: "gpt-default".to_string(),
        },
    );
    assert_eq!(
        definition.compatibility,
        ExternalSubagentCompatibilityState::Blocked,
    );
    assert!(definition
        .diagnostic_codes
        .contains(&"codex_agent_default_reasoning_not_imported".to_string()));
}

#[test]
fn declared_role_file_cannot_escape_its_codex_directory() {
    let fixture = Fixture::new();
    let outside = fixture.project.join("outside-role.toml");
    write(
        &outside,
        r#"developer_instructions = "Outside instructions""#,
    );
    write(
        fixture.project.join(".codex/config.toml"),
        r#"[agents.reviewer]
description = "Reviewer"
config_file = "../outside-role.toml"
"#,
    );

    let snapshot = fixture.discover(BTreeSet::new());

    assert_eq!(snapshot.definitions.len(), 1);
    assert_eq!(
        snapshot.definitions[0].compatibility,
        ExternalSubagentCompatibilityState::Invalid,
    );
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|item| item.code == "codex.agent.role_file_outside_root"));
}
