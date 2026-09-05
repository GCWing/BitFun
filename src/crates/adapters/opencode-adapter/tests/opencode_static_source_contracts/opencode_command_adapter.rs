use openbitfun_opencode_adapter::{OpenCodeCommandProvider, OpenCodeCommandProviderOptions};
use openbitfun_product_domains::external_sources::{
    EcosystemId, ExecutionDomainId, ExternalSourceContext, ExternalSourceHealth,
    PromptCommandAvailability, PromptCommandDefinition, PromptCommandExecutionTarget,
    PromptCommandProviderSnapshot, PromptCommandShellPreference, PromptCommandSourceProvider,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    user_config: PathBuf,
    legacy_user_config: PathBuf,
    project: PathBuf,
    opened_directory: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        let user_config = temp.path().join("xdg/opencode");
        let legacy_user_config = temp.path().join("home/.opencode");
        let project = temp.path().join("project");
        let opened_directory = project.join("packages/app");
        fs::create_dir_all(&user_config).unwrap();
        fs::create_dir_all(&legacy_user_config).unwrap();
        fs::create_dir_all(project.join(".git")).unwrap();
        fs::create_dir_all(&opened_directory).unwrap();
        Self {
            _temp: temp,
            user_config,
            legacy_user_config,
            project,
            opened_directory,
        }
    }

    fn provider(&self) -> OpenCodeCommandProvider {
        self.provider_with_project_config(true)
    }

    fn provider_with_project_config(
        &self,
        project_config_enabled: bool,
    ) -> OpenCodeCommandProvider {
        OpenCodeCommandProvider::new(OpenCodeCommandProviderOptions {
            user_config_dir: self.user_config.clone(),
            legacy_user_config_dir: Some(self.legacy_user_config.clone()),
            explicit_config_file: None,
            explicit_config_dir: None,
            inline_config_content: None,
            project_config_enabled,
        })
    }

    fn context(&self) -> ExternalSourceContext {
        ExternalSourceContext {
            workspace_root: Some(self.opened_directory.clone()),
            execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
        }
    }
}

fn write(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn markdown(description: &str, template: &str) -> String {
    format!("---\ndescription: {description}\n---\n{template}\n")
}

fn resolve_all(
    provider: &OpenCodeCommandProvider,
    snapshot: &PromptCommandProviderSnapshot,
) -> Vec<PromptCommandDefinition> {
    provider
        .resolve_commands(
            snapshot,
            &snapshot
                .sources
                .iter()
                .map(|source| source.key.clone())
                .collect::<BTreeSet<_>>(),
        )
        .unwrap()
}

#[test]
fn discovers_global_project_and_nested_command_directories_in_opencode_order() {
    let fixture = Fixture::new();
    write(
        fixture.user_config.join("opencode.jsonc"),
        r#"{
          // OpenCode accepts JSONC and trailing commas.
          "command": {
            "review": { "template": "global config $ARGUMENTS", "description": "global config" },
            "from-json": { "template": "json $ARGUMENTS", },
          },
        }"#,
    );
    write(
        fixture.user_config.join("command/review.md"),
        &markdown("global markdown", "global markdown $ARGUMENTS"),
    );
    write(
        fixture.legacy_user_config.join("commands/nested/legacy.md"),
        &markdown("legacy nested", "legacy $ARGUMENTS"),
    );
    write(
        fixture.project.join("opencode.json"),
        r#"{"command":{"review":{"template":"project config $ARGUMENTS","description":"project config"}}}"#,
    );
    write(
        fixture.project.join(".opencode/command/review.md"),
        &markdown("project directory", "project directory $ARGUMENTS"),
    );
    write(
        fixture.opened_directory.join("opencode.jsonc"),
        r#"{"command":{"review":{"template":"closer config $ARGUMENTS","description":"closer config"}}}"#,
    );
    write(
        fixture
            .opened_directory
            .join(".opencode/commands/review.md"),
        &markdown("closest directory", "closest directory $ARGUMENTS"),
    );

    let provider = fixture.provider();
    let snapshot = provider
        .discover(&fixture.context())
        .expect("discover OpenCode commands");
    let resolved = resolve_all(&provider, &snapshot);
    let review = resolved
        .iter()
        .find(|command| command.name == "review")
        .unwrap();

    assert_eq!(review.description, "project directory");
    assert_eq!(review.template, "project directory $ARGUMENTS");
    assert!(resolved.iter().any(|command| command.name == "from-json"));
    assert!(resolved
        .iter()
        .any(|command| command.name == "nested/legacy"));
    assert!(snapshot.sources.len() >= 6);
    assert!(snapshot
        .sources
        .iter()
        .all(|source| source.ecosystem_id.as_str() == "opencode"));
}

#[test]
fn mirrors_current_opencode_command_precedence_phases() {
    let fixture = Fixture::new();
    let explicit = fixture._temp.path().join("explicit");
    let provider = OpenCodeCommandProvider::new(OpenCodeCommandProviderOptions {
        user_config_dir: fixture.user_config.clone(),
        legacy_user_config_dir: Some(fixture.legacy_user_config.clone()),
        explicit_config_file: None,
        explicit_config_dir: Some(explicit.clone()),
        inline_config_content: None,
        project_config_enabled: true,
    });
    let winner = || {
        let snapshot = provider.discover(&fixture.context()).unwrap();
        resolve_all(&provider, &snapshot)
            .into_iter()
            .find(|command| command.name == "review")
            .unwrap()
            .template
    };

    write(
        fixture.user_config.join("opencode.json"),
        r#"{"command":{"review":{"template":"global json"}}}"#,
    );
    write(
        fixture.project.join("opencode.json"),
        r#"{"command":{"review":{"template":"project json"}}}"#,
    );
    assert_eq!(winner(), "project json");

    write(
        fixture.user_config.join("commands/review.md"),
        &markdown("global directory", "global directory"),
    );
    assert_eq!(winner(), "global directory");

    write(
        fixture
            .opened_directory
            .join(".opencode/commands/review.md"),
        &markdown("closest project directory", "closest project directory"),
    );
    assert_eq!(winner(), "closest project directory");

    write(
        fixture.project.join(".opencode/commands/review.md"),
        &markdown("outer project directory", "outer project directory"),
    );
    assert_eq!(winner(), "outer project directory");

    write(
        fixture.legacy_user_config.join("commands/review.md"),
        &markdown("legacy directory", "legacy directory"),
    );
    assert_eq!(winner(), "legacy directory");

    write(
        explicit.join("commands/review.md"),
        &markdown("explicit directory", "explicit directory"),
    );
    assert_eq!(winner(), "explicit directory");
}

#[test]
fn discovers_user_global_commands_without_an_open_workspace() {
    let fixture = Fixture::new();
    write(
        fixture.user_config.join("command/global.md"),
        &markdown("global command", "global $ARGUMENTS"),
    );
    write(
        fixture.user_config.join("config.json"),
        r#"{"command":{"from-config-json":{"template":"legacy global"}}}"#,
    );
    write(
        fixture.project.join(".opencode/command/project.md"),
        &markdown("project command", "project $ARGUMENTS"),
    );
    let context = ExternalSourceContext {
        workspace_root: None,
        execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
    };

    let snapshot = fixture.provider().discover(&context).unwrap();

    assert!(snapshot
        .commands
        .iter()
        .any(|command| command.name == "global"));
    assert!(snapshot
        .commands
        .iter()
        .any(|command| command.name == "from-config-json"));
    assert!(!snapshot
        .commands
        .iter()
        .any(|command| command.name == "project"));
}

#[test]
fn suppressing_an_opencode_winner_reveals_the_next_ecosystem_source() {
    let fixture = Fixture::new();
    write(
        fixture.user_config.join("commands/review.md"),
        &markdown("global review", "global"),
    );
    write(
        fixture.project.join(".opencode/commands/review.md"),
        &markdown("project review", "project"),
    );
    let provider = fixture.provider();
    let snapshot = provider.discover(&fixture.context()).unwrap();
    let mut enabled = snapshot
        .sources
        .iter()
        .map(|source| source.key.clone())
        .collect::<BTreeSet<_>>();

    let resolved = provider.resolve_commands(&snapshot, &enabled).unwrap();
    assert_eq!(resolved[0].template, "project");
    let project_source = snapshot
        .commands
        .iter()
        .find(|command| command.template == "project")
        .unwrap()
        .id
        .source
        .clone();
    enabled.remove(&project_source);

    let fallback = provider.resolve_commands(&snapshot, &enabled).unwrap();
    assert_eq!(fallback[0].template, "global");
    enabled.insert(project_source);
    assert_eq!(
        provider.resolve_commands(&snapshot, &enabled).unwrap()[0].template,
        "project"
    );
}

#[test]
fn disabled_project_config_excludes_project_files_directories_and_watch_roots() {
    let fixture = Fixture::new();
    write(
        fixture.user_config.join("commands/global.md"),
        &markdown("global", "global"),
    );
    write(
        fixture.project.join("opencode.json"),
        r#"{"command":{"project-json":{"template":"project"}}}"#,
    );
    write(
        fixture.project.join(".opencode/commands/project-dir.md"),
        &markdown("project", "project"),
    );
    let provider = fixture.provider_with_project_config(false);

    let snapshot = provider.discover(&fixture.context()).unwrap();

    assert_eq!(snapshot.commands.len(), 1);
    assert_eq!(snapshot.commands[0].name, "global");
    assert!(!provider
        .watch_roots(&fixture.context())
        .iter()
        .any(|root| root.path.starts_with(&fixture.project)));
}

#[test]
fn invalid_source_is_diagnostic_and_does_not_remove_other_valid_sources() {
    let fixture = Fixture::new();
    write(
        fixture.user_config.join("commands/global.md"),
        &markdown("valid global", "global prompt"),
    );
    write(
        fixture.project.join("opencode.jsonc"),
        "{ this is invalid jsonc",
    );

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    assert!(snapshot
        .commands
        .iter()
        .any(|command| command.name == "global"));
    assert!(snapshot
        .sources
        .iter()
        .any(|source| source.health == ExternalSourceHealth::Unavailable));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.command.config_invalid"));
}

#[test]
fn supported_subagent_delegation_is_typed_and_other_unsupported_features_stay_restricted() {
    let fixture = Fixture::new();
    write(
        fixture.user_config.join("opencode.json"),
        r#"{
          "shell": "pwsh",
          "command": {
            "shell": {"template":"Run !`git status` and review @src/main.rs"},
            "file": {"template":"Review @src/main.rs, @src/main.rs, and @docs/guide.md with $ARGUMENTS"},
            "dynamic-file": {"template":"Review @src/$1.rs"},
            "unsafe-file": {"template":"Review @/etc/passwd"},
            "config-var": {"template":"Review {env:HOME}"},
            "agent": {"template":"Delegate this", "agent":"explore"},
            "agent-subtask": {"template":"Delegate this", "agent":"explore", "subtask":true},
            "agent-shell": {"template":"Run !`git status` then delegate", "agent":"explore"},
            "primary-agent": {"template":"Stay primary", "agent":"build", "subtask":false},
            "subtask": {"template":"Delegate this", "subtask":true},
            "model": {"template":"Delegate this", "agent":"explore", "model":"provider/model"},
            "variant": {"template":"Delegate this", "agent":"explore", "variant":"high"}
          }
        }"#,
    );

    let provider = fixture.provider();
    let snapshot = provider.discover(&fixture.context()).unwrap();
    assert!(snapshot
        .sources
        .iter()
        .any(|source| source.health == ExternalSourceHealth::Partial));
    for name in [
        "dynamic-file",
        "unsafe-file",
        "config-var",
        "agent-shell",
        "primary-agent",
        "subtask",
        "model",
        "variant",
    ] {
        let command = snapshot
            .commands
            .iter()
            .find(|command| command.name == name)
            .unwrap();
        assert!(matches!(
            command.availability,
            PromptCommandAvailability::Restricted { .. }
        ));
        assert!(provider.expand(&fixture.context(), command, "").is_err());
    }
    let agent_shell = snapshot
        .commands
        .iter()
        .find(|command| command.name == "agent-shell")
        .unwrap();
    let PromptCommandAvailability::Restricted {
        required_capabilities,
        ..
    } = &agent_shell.availability
    else {
        panic!("shell-backed subagent delegation must be restricted")
    };
    assert!(required_capabilities.contains(&"command.external_subagent.shell".to_string()));
    for name in ["agent", "agent-subtask"] {
        let command = snapshot
            .commands
            .iter()
            .find(|command| command.name == name)
            .unwrap();
        assert!(matches!(
            command.availability,
            PromptCommandAvailability::Available
        ));
        assert_eq!(
            command.execution_target,
            PromptCommandExecutionTarget::FreshExternalSubagent {
                ecosystem_id: EcosystemId::new("opencode").unwrap(),
                logical_id: "explore".to_string(),
            }
        );
        assert_eq!(
            provider
                .expand(&fixture.context(), command, "")
                .unwrap()
                .content,
            "Delegate this"
        );
    }
    let shell = snapshot
        .commands
        .iter()
        .find(|command| command.name == "shell")
        .unwrap();
    assert!(matches!(
        shell.availability,
        PromptCommandAvailability::Available
    ));
    assert_eq!(
        shell.shell_preference,
        Some(PromptCommandShellPreference::Preferred {
            executable: "pwsh".to_string()
        })
    );
    let shell_expansion = provider
        .expand(&fixture.context(), shell, "")
        .unwrap()
        .shell
        .unwrap();
    assert_eq!(shell_expansion.working_directory, fixture.project);
    assert_eq!(shell_expansion.invocations[0].command, "git status");
    assert!(shell_expansion.invocations[0].can_remember);
    let dynamic_file = snapshot
        .commands
        .iter()
        .find(|command| command.name == "dynamic-file")
        .unwrap();
    let PromptCommandAvailability::Restricted {
        required_capabilities,
        ..
    } = &dynamic_file.availability
    else {
        panic!("dynamic file references must be restricted")
    };
    assert!(required_capabilities.contains(&"command.file_reference.dynamic".to_string()));
    let unsafe_file = snapshot
        .commands
        .iter()
        .find(|command| command.name == "unsafe-file")
        .unwrap();
    let PromptCommandAvailability::Restricted {
        required_capabilities,
        ..
    } = &unsafe_file.availability
    else {
        panic!("unsafe file references must be restricted")
    };
    assert!(required_capabilities.contains(&"command.file_reference.unsafe_path".to_string()));

    let file_command = snapshot
        .commands
        .iter()
        .find(|command| command.name == "file")
        .unwrap();
    assert!(matches!(
        file_command.availability,
        PromptCommandAvailability::Available
    ));
    let expansion = provider
        .expand(
            &fixture.context(),
            file_command,
            "@arguments/are-not-files.md",
        )
        .unwrap();
    assert_eq!(
        expansion.workspace_file_references,
        ["src/main.rs", "docs/guide.md"]
    );
    assert_eq!(
        expansion.content,
        "Review @src/main.rs, @src/main.rs, and @docs/guide.md with @arguments/are-not-files.md"
    );
}

#[test]
fn empty_configured_shell_uses_the_host_default() {
    let fixture = Fixture::new();
    write(
        fixture.user_config.join("opencode.json"),
        r#"{
          "shell": "  ",
          "command": {
            "status": {"template":"Run !`git status`"}
          }
        }"#,
    );

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();
    let command = snapshot
        .commands
        .iter()
        .find(|command| command.name == "status")
        .unwrap();

    assert!(matches!(
        command.availability,
        PromptCommandAvailability::Available
    ));
    assert_eq!(
        command.shell_preference,
        Some(PromptCommandShellPreference::HostDefault)
    );
}

#[test]
fn plural_command_config_is_diagnostic_instead_of_silently_empty() {
    let fixture = Fixture::new();
    write(
        fixture.user_config.join("opencode.json"),
        r#"{"commands":{"review":{"template":"wrong field"}}}"#,
    );

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    assert!(snapshot.commands.is_empty());
    assert!(snapshot
        .sources
        .iter()
        .any(|source| source.health == ExternalSourceHealth::Unavailable));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.message.contains("OpenCode uses 'command'") }));
}

#[test]
fn one_invalid_definition_does_not_fail_the_provider() {
    let fixture = Fixture::new();
    write(
        fixture.user_config.join("commands/global.md"),
        &markdown("valid global", "global prompt"),
    );
    let invalid_name = "x".repeat(200);
    write(
        fixture.project.join("opencode.json"),
        &format!(r#"{{"command":{{"{invalid_name}":{{"template":"invalid"}}}}}}"#),
    );

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    assert!(snapshot
        .commands
        .iter()
        .any(|command| command.name == "global"));
    assert!(snapshot
        .sources
        .iter()
        .any(|source| source.health == ExternalSourceHealth::Degraded));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.command.name_invalid"));
}

#[test]
fn invalid_known_markdown_metadata_is_not_silently_dropped() {
    let fixture = Fixture::new();
    write(
        fixture.user_config.join("commands/review.md"),
        "---\ndescription:\n  - not-a-string\n---\nReview this change\n",
    );

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    assert!(snapshot.commands.is_empty());
    assert_eq!(snapshot.unavailable_command_ids.len(), 1);
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.command.markdown_invalid"));
}

#[test]
fn markdown_frontmatter_retries_opencode_unquoted_colon_compatibility() {
    let fixture = Fixture::new();
    write(
        fixture.user_config.join("commands/review.md"),
        "---\ndescription: Review: focused changes\n---\nReview this change\n",
    );

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();
    let command = snapshot
        .commands
        .iter()
        .find(|command| command.name == "review")
        .unwrap();

    assert_eq!(command.description, "Review: focused changes");
    assert!(matches!(
        command.availability,
        PromptCommandAvailability::Available
    ));
}

#[test]
fn expands_arguments_and_positions_using_the_frozen_opencode_semantics() {
    let fixture = Fixture::new();
    let provider = fixture.provider();
    write(
        fixture.user_config.join("opencode.json"),
        r#"{
          "command": {
            "all": {"template":"Review $ARGUMENTS"},
            "positions": {"template":"First=$1 Rest=$2"},
            "append": {"template":"Review this change"}
          }
        }"#,
    );
    let snapshot = provider.discover(&fixture.context()).unwrap();

    let expand = |name: &str, arguments: &str| {
        let command = snapshot
            .commands
            .iter()
            .find(|item| item.name == name)
            .unwrap();
        provider
            .expand(&fixture.context(), command, arguments)
            .unwrap()
            .content
    };
    assert_eq!(
        expand("all", "src/lib.rs carefully"),
        "Review src/lib.rs carefully"
    );
    assert_eq!(
        expand("positions", "\"hello world\" second third"),
        "First=hello world Rest=second third"
    );
    assert_eq!(
        expand("append", "with tests"),
        "Review this change\n\nwith tests"
    );
}

#[test]
fn rejects_argument_expansion_before_repeated_placeholders_can_overallocate() {
    let fixture = Fixture::new();
    write(
        fixture.user_config.join("commands/large.md"),
        &"$ARGUMENTS".repeat(1024),
    );
    let provider = fixture.provider();
    let snapshot = provider.discover(&fixture.context()).unwrap();
    let command = snapshot
        .commands
        .iter()
        .find(|command| command.name == "large")
        .unwrap();

    let error = provider
        .expand(&fixture.context(), command, &"x".repeat(2048))
        .unwrap_err();

    assert_eq!(error.code, "opencode.command.expansion_too_large");
}

#[test]
fn deleting_the_winning_file_reveals_the_next_opencode_source() {
    let fixture = Fixture::new();
    let global = fixture.user_config.join("commands/review.md");
    let project = fixture.project.join(".opencode/commands/review.md");
    write(&global, &markdown("global", "global"));
    write(&project, &markdown("project", "project"));
    let provider = fixture.provider();

    let initial = provider.discover(&fixture.context()).unwrap();
    let initial = resolve_all(&provider, &initial);
    assert_eq!(
        initial
            .iter()
            .find(|item| item.name == "review")
            .unwrap()
            .template,
        "project"
    );
    fs::remove_file(project).unwrap();
    let refreshed = provider.discover(&fixture.context()).unwrap();
    let refreshed = resolve_all(&provider, &refreshed);
    assert_eq!(
        refreshed
            .iter()
            .find(|item| item.name == "review")
            .unwrap()
            .template,
        "global"
    );
}

#[test]
fn invalid_higher_priority_directory_candidate_does_not_expose_a_lower_duplicate() {
    let fixture = Fixture::new();
    write(
        fixture.user_config.join("command/review.md"),
        &markdown("lower", "lower"),
    );
    write(
        fixture.user_config.join("commands/review.md"),
        "---\ndescription: invalid\n",
    );

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    snapshot
        .validate()
        .expect("snapshot identities remain unique");
    assert!(!snapshot
        .commands
        .iter()
        .any(|command| command.name == "review"));
    assert!(snapshot
        .unavailable_command_ids
        .iter()
        .any(|command_id| command_id.local_id.as_str() == "review"));
}

#[test]
fn semantically_invalid_known_command_is_marked_unavailable() {
    let fixture = Fixture::new();
    write(
        fixture.user_config.join("opencode.json"),
        r#"{"command":{"review":{"template":""}}}"#,
    );

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    assert!(snapshot.commands.is_empty());
    assert!(snapshot
        .unavailable_command_ids
        .iter()
        .any(|command_id| command_id.local_id.as_str() == "review"));
}

#[test]
fn malformed_skills_field_does_not_hide_other_valid_command_contributions() {
    let fixture = Fixture::new();
    write(
        fixture.user_config.join("opencode.json"),
        r#"{"skills":["valid",42],"command":{"review":{"template":"review this"}}}"#,
    );

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    assert!(snapshot
        .commands
        .iter()
        .any(|command| command.id.local_id.as_str() == "review"));
}

#[test]
fn invalid_command_directory_shape_is_unavailable_not_a_stable_empty_source() {
    let fixture = Fixture::new();
    write(fixture.user_config.join("command"), "not a directory");

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    assert!(snapshot
        .sources
        .iter()
        .any(|source| source.health == ExternalSourceHealth::Unavailable));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.command.directory_invalid"));
}

#[test]
fn explicit_paths_aliasing_default_paths_do_not_duplicate_sources_or_commands() {
    let fixture = Fixture::new();
    let config = fixture.user_config.join("opencode.json");
    write(&config, r#"{"command":{"review":{"template":"review"}}}"#);
    write(
        fixture.user_config.join("commands/other.md"),
        &markdown("other", "other"),
    );
    let provider = OpenCodeCommandProvider::new(OpenCodeCommandProviderOptions {
        user_config_dir: fixture.user_config.clone(),
        legacy_user_config_dir: Some(fixture.legacy_user_config.clone()),
        explicit_config_file: Some(config),
        explicit_config_dir: Some(fixture.user_config.clone()),
        inline_config_content: None,
        project_config_enabled: true,
    });

    let snapshot = provider.discover(&fixture.context()).unwrap();

    snapshot.validate().expect("snapshot must be unique");
    let source_keys = snapshot
        .sources
        .iter()
        .map(|source| source.key.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(source_keys.len(), snapshot.sources.len());
    let command_ids = snapshot
        .commands
        .iter()
        .map(|command| command.id.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(command_ids.len(), snapshot.commands.len());
}

#[test]
fn oversized_config_is_bounded_and_reported() {
    let fixture = Fixture::new();
    fs::write(
        fixture.user_config.join("opencode.json"),
        vec![b' '; 1024 * 1024 + 1],
    )
    .unwrap();

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.command.config_too_large"));
}

#[test]
fn oversized_command_directory_is_not_partially_published() {
    let fixture = Fixture::new();
    let directory = fixture.user_config.join("commands");
    fs::create_dir_all(&directory).unwrap();
    for index in 0..=2048 {
        fs::write(directory.join(format!("command-{index:04}.md")), "prompt").unwrap();
    }

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    assert!(snapshot.commands.is_empty());
    assert!(snapshot
        .sources
        .iter()
        .any(|source| source.health == ExternalSourceHealth::Unavailable));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.command.file_limit"));
}

#[test]
fn command_directory_template_bytes_are_bounded_as_a_collection() {
    let fixture = Fixture::new();
    let directory = fixture.user_config.join("commands");
    fs::create_dir_all(&directory).unwrap();
    let body = "x".repeat(220 * 1024);
    for index in 0..40 {
        fs::write(directory.join(format!("large-{index:02}.md")), &body).unwrap();
    }

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();
    let published_bytes = snapshot
        .commands
        .iter()
        .map(|command| command.template.len())
        .sum::<usize>();

    assert!(published_bytes <= 8 * 1024 * 1024);
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == "opencode.command.total_template_bytes_limit" }));
}

#[test]
fn template_bytes_are_bounded_across_all_discovered_layers() {
    let fixture = Fixture::new();
    let body = "x".repeat(220 * 1024);
    for (root, prefix) in [
        (fixture.user_config.join("commands"), "global"),
        (fixture.project.join(".opencode/commands"), "project"),
    ] {
        fs::create_dir_all(&root).unwrap();
        for index in 0..24 {
            fs::write(root.join(format!("{prefix}-{index:02}.md")), &body).unwrap();
        }
    }

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();
    let published_bytes = snapshot
        .commands
        .iter()
        .map(|command| command.template.len())
        .sum::<usize>();

    assert!(published_bytes <= 8 * 1024 * 1024);
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == "opencode.command.provider_template_bytes_limit" }));
}

#[test]
fn watch_roots_cover_global_and_project_creation_paths() {
    let fixture = Fixture::new();
    let roots = fixture.provider().watch_roots(&fixture.context());

    assert!(roots
        .iter()
        .any(|root| root.path == fixture.user_config && root.recursive));
    assert!(roots
        .iter()
        .any(|root| root.path == fixture.project && !root.recursive));
    assert!(roots
        .iter()
        .any(|root| { root.path == fixture.project.join(".opencode") && root.recursive }));
}
