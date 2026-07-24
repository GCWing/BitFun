use bitfun_claude_code_adapter::{ClaudeCodeSubagentProvider, ClaudeCodeSubagentProviderOptions};
use bitfun_product_domains::external_sources::{
    ExecutionDomainId, ExternalSourceContext, ExternalSourceScope,
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
    user_claude: PathBuf,
    project: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let user_claude = temp.path().join("home/.claude");
        let project = temp.path().join("project");
        let workspace = project.join("packages/app");
        fs::create_dir_all(&user_claude).unwrap();
        fs::create_dir_all(project.join(".git")).unwrap();
        fs::create_dir_all(&workspace).unwrap();
        Self {
            _temp: temp,
            user_claude,
            project,
            workspace,
        }
    }

    fn provider(&self) -> ClaudeCodeSubagentProvider {
        ClaudeCodeSubagentProvider::new(ClaudeCodeSubagentProviderOptions {
            user_claude_dir: self.user_claude.clone(),
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
        suppressed_sources: BTreeSet<bitfun_product_domains::external_sources::SourceKey>,
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
fn nearest_project_agent_overrides_user_agent_without_field_merge() {
    let fixture = Fixture::new();
    write(
        fixture.user_claude.join("agents/review.md"),
        "---\nname: review\ndescription: Personal\nmodel: inherit\ntools: [Read, Grep]\n---\nPersonal prompt",
    );
    write(
        fixture.project.join(".claude/agents/review.md"),
        "---\nname: review\ndescription: Project\nmodel: claude-sonnet-4\ntools: [Read]\n---\nProject prompt",
    );
    write(
        fixture.workspace.join(".claude/agents/review.md"),
        "---\nname: review\ndescription: Nearest\nmodel: claude-opus-4\ntools: [Read, Glob]\n---\nNearest prompt",
    );

    let snapshot = fixture.discover(BTreeSet::new());
    let definition = &snapshot.definitions[0];
    assert_eq!(definition.description, "Nearest");
    assert_eq!(definition.prompt.expose(), "Nearest prompt");
    assert_eq!(definition.provenance.len(), 3);
    assert_eq!(
        definition.requested_model,
        ExternalSubagentModelRequest::Exact {
            provider_hint: None,
            model_name: "claude-opus-4".to_string(),
        }
    );
    assert_eq!(
        definition
            .requested_tools
            .selectors
            .iter()
            .map(|selector| selector.source_name.as_str())
            .collect::<Vec<_>>(),
        vec!["Glob", "Read"]
    );
}

#[test]
fn unsupported_behavior_blocks_while_color_is_display_only_degradation() {
    let fixture = Fixture::new();
    write(
        fixture.user_claude.join("agents/blocked.md"),
        "---\nname: blocked\ndescription: Blocked\ntools: [Read]\npermissionMode: bypassPermissions\nmaxTurns: 3\nbackground: true\n---\nSECRET-BLOCKED-PROMPT",
    );
    write(
        fixture.user_claude.join("agents/colored.md"),
        "---\nname: colored\ndescription: Colored\ntools: [Read]\ncolor: blue\n---\nColored prompt",
    );

    let snapshot = fixture.discover(BTreeSet::new());
    let blocked = snapshot
        .definitions
        .iter()
        .find(|item| item.logical_id == "blocked")
        .unwrap();
    assert_eq!(
        blocked.compatibility,
        ExternalSubagentCompatibilityState::Blocked
    );
    assert!(blocked
        .diagnostic_codes
        .contains(&"claude_agent_permission_mode_not_imported".to_string()));
    assert!(blocked
        .diagnostic_codes
        .contains(&"claude_agent_max_turns_not_imported".to_string()));
    let colored = snapshot
        .definitions
        .iter()
        .find(|item| item.logical_id == "colored")
        .unwrap();
    assert_eq!(
        colored.compatibility,
        ExternalSubagentCompatibilityState::ReadyWithDegradation
    );
    assert!(!format!("{snapshot:?}").contains("SECRET-BLOCKED-PROMPT"));
}

#[test]
fn malformed_required_or_display_fields_are_invalid_instead_of_activatable() {
    let fixture = Fixture::new();
    write(
        fixture.user_claude.join("agents/missing-description.md"),
        "---\nname: missing-description\ntools: [Read]\n---\nPrompt",
    );
    write(
        fixture.user_claude.join("agents/invalid-color.md"),
        "---\nname: invalid-color\ndescription: Invalid color\ntools: [Read]\ncolor: 42\n---\nPrompt",
    );
    write(
        fixture.user_claude.join("agents/invalid-name.md"),
        "---\nname: Invalid Name\ndescription: Invalid name\ntools: [Read]\n---\nPrompt",
    );

    let snapshot = fixture.discover(BTreeSet::new());

    assert_eq!(snapshot.definitions.len(), 2);
    assert!(snapshot.definitions.iter().all(|definition| {
        definition.compatibility == ExternalSubagentCompatibilityState::Invalid
    }));
    assert!(snapshot.definitions.iter().any(|definition| definition
        .diagnostic_codes
        .contains(&"claude_agent_description_missing".to_string())));
    assert!(snapshot.definitions.iter().any(|definition| definition
        .diagnostic_codes
        .contains(&"claude_agent_color_invalid".to_string())));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "claude.agent.name_invalid"));
}

#[test]
fn manual_permission_alias_keeps_an_otherwise_supported_agent_ready() {
    let fixture = Fixture::new();
    write(
        fixture.user_claude.join("agents/manual.md"),
        "---\nname: manual\ndescription: Manual permission alias\ntools: [Read]\npermissionMode: manual\n---\nPrompt",
    );

    let snapshot = fixture.discover(BTreeSet::new());

    assert_eq!(snapshot.definitions.len(), 1);
    assert_eq!(
        snapshot.definitions[0].compatibility,
        ExternalSubagentCompatibilityState::Ready
    );
}

#[test]
fn same_layer_duplicate_name_is_invalid_instead_of_using_filesystem_order() {
    let fixture = Fixture::new();
    write(
        fixture.user_claude.join("agents/a.md"),
        "---\nname: review\ndescription: A\n---\nA prompt",
    );
    write(
        fixture.user_claude.join("agents/b.md"),
        "---\nname: review\ndescription: B\n---\nB prompt",
    );

    let snapshot = fixture.discover(BTreeSet::new());
    assert_eq!(snapshot.definitions.len(), 1);
    assert_eq!(
        snapshot.definitions[0].compatibility,
        ExternalSubagentCompatibilityState::Invalid
    );
    assert!(snapshot.definitions[0]
        .diagnostic_codes
        .contains(&"claude_agent_duplicate_name".to_string()));
}

#[test]
fn suppression_is_applied_before_native_resolution() {
    let fixture = Fixture::new();
    write(
        fixture.user_claude.join("agents/review.md"),
        "---\nname: review\ndescription: Personal\ntools: [Read]\n---\nPersonal prompt",
    );
    write(
        fixture.project.join(".claude/agents/review.md"),
        "---\nname: review\ndescription: Project\ntools: [Read]\n---\nProject prompt",
    );
    let first = fixture.discover(BTreeSet::new());
    let project_source = first
        .sources
        .iter()
        .find(|source| source.scope == ExternalSourceScope::Project)
        .unwrap()
        .key
        .clone();

    let suppressed = fixture.discover([project_source].into_iter().collect());
    assert_eq!(suppressed.definitions[0].description, "Personal");
    assert_eq!(suppressed.definitions[0].provenance.len(), 1);
}

#[test]
fn display_only_changes_do_not_invalidate_behavior_version() {
    let fixture = Fixture::new();
    let path = fixture.user_claude.join("agents/review.md");
    write(
        &path,
        "---\nname: review\ndescription: First\ntools: [Read]\ncolor: blue\n---\nReview prompt",
    );
    let first = fixture.discover(BTreeSet::new());
    let behavior = first.definitions[0].behavior_version.clone();
    write(
        &path,
        "---\nname: review\ndescription: Updated\ntools: [Read]\ncolor: red\n---\nReview prompt",
    );
    let updated = fixture.discover(BTreeSet::new());
    assert_eq!(updated.definitions[0].behavior_version, behavior);
    assert_eq!(updated.definitions[0].description, "Updated");
}

#[test]
fn malformed_project_overlay_does_not_reactivate_a_personal_agent() {
    let fixture = Fixture::new();
    write(
        fixture.user_claude.join("agents/review.md"),
        "---\nname: review\ndescription: Personal\n---\nPersonal prompt",
    );
    write(
        fixture.project.join(".claude/agents/review.md"),
        "---\nname: review\ndescription: [unterminated\n---\nProject prompt",
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
fn watch_roots_cover_only_known_claude_directories() {
    let fixture = Fixture::new();
    let roots = fixture.provider().watch_roots(&fixture.context());
    assert!(roots
        .iter()
        .any(|root| root.path == fixture.user_claude && root.recursive));
    assert!(roots
        .iter()
        .any(|root| root.path == fixture.project.join(".claude") && root.recursive));
    assert!(roots
        .iter()
        .any(|root| root.path == fixture.workspace.join(".claude") && root.recursive));
}
