use openbitfun_claude_code_adapter::{ClaudeCodeCommandProvider, ClaudeCodeCommandProviderOptions};
use openbitfun_product_domains::external_sources::{
    ExecutionDomainId, ExternalSourceContext, PromptCommandAvailability,
    PromptCommandProviderSnapshot, PromptCommandShellPreference, PromptCommandSourceProvider,
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

    fn provider(&self) -> ClaudeCodeCommandProvider {
        ClaudeCodeCommandProvider::new(ClaudeCodeCommandProviderOptions {
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
}

fn write(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn resolve(
    provider: &ClaudeCodeCommandProvider,
    snapshot: &PromptCommandProviderSnapshot,
) -> Vec<openbitfun_product_domains::external_sources::PromptCommandDefinition> {
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
fn personal_command_overrides_nested_project_command_with_native_provenance() {
    let fixture = Fixture::new();
    write(
        fixture.project.join(".claude/commands/review.md"),
        "---\ndescription: Project review\n---\nProject $ARGUMENTS",
    );
    write(
        fixture.workspace.join(".claude/commands/review.md"),
        "---\ndescription: Nested review\n---\nNested $ARGUMENTS",
    );
    write(
        fixture.user_claude.join("commands/review.md"),
        "---\ndescription: Personal review\n---\nPersonal $ARGUMENTS",
    );

    let provider = fixture.provider();
    let snapshot = provider.discover(&fixture.context()).unwrap();
    let commands = resolve(&provider, &snapshot);
    let review = commands.iter().find(|item| item.name == "review").unwrap();

    assert_eq!(review.description, "Personal review");
    assert_eq!(review.template, "Personal $ARGUMENTS");
    assert_eq!(snapshot.sources.len(), 3);
    assert!(snapshot
        .sources
        .iter()
        .all(|source| source.ecosystem_id.as_str() == "claude-code"));
}

#[test]
fn skill_with_same_name_shadows_legacy_command_without_reading_skill_body() {
    let fixture = Fixture::new();
    write(
        fixture.user_claude.join("commands/deploy.md"),
        "Deploy $ARGUMENTS",
    );
    write(
        fixture.user_claude.join("skills/deploy/SKILL.md"),
        "SECRET-SKILL-BODY",
    );

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    assert!(snapshot.commands.iter().all(|item| item.name != "deploy"));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "claude.command.shadowed_by_skill"));
    assert!(!format!("{snapshot:?}").contains("SECRET-SKILL-BODY"));
}

#[test]
fn oversized_skill_index_fails_closed_before_legacy_commands_can_activate() {
    let fixture = Fixture::new();
    write(
        fixture.user_claude.join("commands/deploy.md"),
        "Deploy $ARGUMENTS",
    );
    for index in 0..2_049 {
        fs::create_dir_all(
            fixture
                .user_claude
                .join("skills")
                .join(format!("skill-{index:04}")),
        )
        .unwrap();
    }

    let error = fixture
        .provider()
        .discover(&fixture.context())
        .expect_err("an oversized Skill index must fail closed");

    assert_eq!(error.code, "claude.command.skill_index_limit_exceeded");
}

#[test]
fn nested_command_uses_claude_codes_native_namespace() {
    let fixture = Fixture::new();
    write(
        fixture
            .project
            .join(".claude/commands/frontend/component.md"),
        "Review a frontend component",
    );

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    assert_eq!(snapshot.commands[0].name, "frontend:component");
    assert!(snapshot
        .commands
        .iter()
        .all(|command| command.name != "frontend/component"));
}

#[test]
fn allowed_tools_hints_are_accepted_without_changing_command_behavior() {
    let fixture = Fixture::new();
    let path = fixture.user_claude.join("commands/review.md");
    write(
        &path,
        "---\ndescription: Review\nallowed-tools: Read, Bash(git:*)\n---\nReview $ARGUMENTS",
    );

    let provider = fixture.provider();
    let first = provider.discover(&fixture.context()).unwrap();
    let first_command = &first.commands[0];
    assert!(matches!(
        first_command.availability,
        PromptCommandAvailability::Available
    ));
    let behavior_version = first_command.content_version.clone();
    assert_eq!(
        provider
            .expand(&fixture.context(), first_command, "this change")
            .unwrap()
            .content,
        "Review this change"
    );

    write(
        &path,
        "---\ndescription: Review\nallowed-tools: [\"Read\", \"Grep\", \"Bash\"]\n---\nReview $ARGUMENTS",
    );
    let updated = provider.discover(&fixture.context()).unwrap();
    assert!(matches!(
        updated.commands[0].availability,
        PromptCommandAvailability::Available
    ));
    assert_eq!(updated.commands[0].content_version, behavior_version);
}

#[test]
fn malformed_allowed_tools_hints_fail_closed() {
    let fixture = Fixture::new();
    write(
        fixture.user_claude.join("commands/scalar.md"),
        "---\nallowed-tools: 42\n---\nReview the change",
    );
    write(
        fixture.user_claude.join("commands/list.md"),
        "---\nallowed-tools: [Read, false]\n---\nReview the change",
    );

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    assert!(snapshot.commands.is_empty());
    assert_eq!(snapshot.unavailable_command_ids.len(), 2);
    let invalid_metadata = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "claude.command.markdown_invalid")
        .collect::<Vec<_>>();
    assert_eq!(invalid_metadata.len(), 2);
    assert!(invalid_metadata.iter().all(|diagnostic| diagnostic
        .message
        .contains("allowed-tools must be a string or string list")));
}

#[test]
fn dynamic_and_behavioral_commands_are_visible_but_restricted() {
    let fixture = Fixture::new();
    write(
        fixture.user_claude.join("commands/shell.md"),
        "---\ndescription: Shell\nallowed-tools: Bash\ndisallowed-tools: Write\nmodel: sonnet\n---\nInspect !`git status`, @README.md, and ${CLAUDE_SESSION_ID}",
    );

    let provider = fixture.provider();
    let snapshot = provider.discover(&fixture.context()).unwrap();
    let command = &snapshot.commands[0];
    let PromptCommandAvailability::Restricted {
        required_capabilities,
        ..
    } = &command.availability
    else {
        panic!("dynamic Claude command must be restricted")
    };
    assert!(!required_capabilities.contains(&"command.shell".to_string()));
    assert!(!required_capabilities.contains(&"command.file_reference".to_string()));
    assert!(required_capabilities.contains(&"command.model".to_string()));
    assert!(!required_capabilities.contains(&"command.allowed_tools".to_string()));
    assert!(required_capabilities.contains(&"command.disallowed_tools".to_string()));
    assert!(required_capabilities.contains(&"command.dynamic_variable".to_string()));
    assert!(provider.expand(&fixture.context(), command, "now").is_err());
}

#[test]
fn shell_directives_are_reviewable_with_claude_shell_and_project_root_semantics() {
    let fixture = Fixture::new();
    write(
        fixture.user_claude.join("commands/static.md"),
        "Inspect !`git status` and !`cat @README.md`",
    );
    write(
        fixture.user_claude.join("commands/dynamic.md"),
        "---\ndescription: Dynamic\nshell: powershell\n---\nInspect !`Write-Output $ARGUMENTS`",
    );

    let provider = fixture.provider();
    let snapshot = provider.discover(&fixture.context()).unwrap();
    let static_command = snapshot
        .commands
        .iter()
        .find(|command| command.name == "static")
        .unwrap();
    assert!(matches!(
        static_command.availability,
        PromptCommandAvailability::Available
    ));
    assert_eq!(
        static_command.shell_preference,
        Some(PromptCommandShellPreference::Required {
            executable: "bash".to_string()
        })
    );
    let static_expansion = provider
        .expand(&fixture.context(), static_command, "")
        .unwrap();
    assert!(static_expansion.workspace_file_references.is_empty());
    let static_shell = static_expansion.shell.unwrap();
    assert_eq!(static_shell.working_directory, fixture.project);
    assert_eq!(static_shell.invocations.len(), 2);
    assert!(static_shell
        .invocations
        .iter()
        .all(|invocation| invocation.can_remember));

    let dynamic_command = snapshot
        .commands
        .iter()
        .find(|command| command.name == "dynamic")
        .unwrap();
    assert_eq!(
        dynamic_command.shell_preference,
        Some(PromptCommandShellPreference::RequiredOneOf {
            executables: vec!["pwsh".to_string(), "powershell".to_string()]
        })
    );
    let dynamic_shell = provider
        .expand(&fixture.context(), dynamic_command, "hello world")
        .unwrap()
        .shell
        .unwrap();
    assert_eq!(
        dynamic_shell.invocations[0].command,
        "Write-Output hello world"
    );
    assert!(!dynamic_shell.invocations[0].can_remember);
}

#[test]
fn literal_file_references_are_prepared_without_scanning_arguments() {
    let fixture = Fixture::new();
    write(
        fixture.user_claude.join("commands/review.md"),
        "Review @src/lib.rs, @src/lib.rs, and @docs/guide.md for $ARGUMENTS",
    );

    let provider = fixture.provider();
    let snapshot = provider.discover(&fixture.context()).unwrap();
    let command = &snapshot.commands[0];
    assert!(matches!(
        command.availability,
        PromptCommandAvailability::Available
    ));

    let expansion = provider
        .expand(&fixture.context(), command, "@arguments/are-not-files.md")
        .unwrap();
    assert_eq!(
        expansion.workspace_file_references,
        ["src/lib.rs", "docs/guide.md"]
    );
    assert_eq!(
        expansion.content,
        "Review @src/lib.rs, @src/lib.rs, and @docs/guide.md for @arguments/are-not-files.md"
    );
}

#[test]
fn dynamic_and_unsafe_file_references_are_visible_but_restricted() {
    let fixture = Fixture::new();
    write(
        fixture.user_claude.join("commands/dynamic.md"),
        "Review @src/$1.rs",
    );
    write(
        fixture.user_claude.join("commands/unsafe.md"),
        "Review @/etc/passwd",
    );

    let provider = fixture.provider();
    let snapshot = provider.discover(&fixture.context()).unwrap();
    for (name, capability) in [
        ("dynamic", "command.file_reference.dynamic"),
        ("unsafe", "command.file_reference.unsafe_path"),
    ] {
        let command = snapshot
            .commands
            .iter()
            .find(|command| command.name == name)
            .unwrap();
        let PromptCommandAvailability::Restricted {
            required_capabilities,
            ..
        } = &command.availability
        else {
            panic!("{name} file reference must be restricted")
        };
        assert!(required_capabilities.contains(&capability.to_string()));
        assert!(provider.expand(&fixture.context(), command, "").is_err());
    }
}

#[test]
fn safe_arguments_expand_and_description_only_changes_keep_behavior_version() {
    let fixture = Fixture::new();
    let path = fixture.user_claude.join("commands/review.md");
    write(
        &path,
        "---\ndescription: First description\nargument-hint: path\n---\nReview $0 then $ARGUMENTS[1] and $ARGUMENTS",
    );
    let provider = fixture.provider();
    let first = provider.discover(&fixture.context()).unwrap();
    let first_command = &first.commands[0];
    let version = first_command.content_version.clone();
    assert_eq!(
        provider
            .expand(&fixture.context(), first_command, "src/lib.rs carefully")
            .unwrap()
            .content,
        "Review src/lib.rs then carefully and src/lib.rs carefully"
    );

    write(
        &path,
        "---\ndescription: Updated description\nargument-hint: file\n---\nReview $0 then $ARGUMENTS[1] and $ARGUMENTS",
    );
    let updated = provider.discover(&fixture.context()).unwrap();
    assert_eq!(updated.commands[0].content_version, version);
    assert_eq!(updated.commands[0].description, "Updated description");
}

#[test]
fn rejects_argument_expansion_before_repeated_placeholders_can_overallocate() {
    let fixture = Fixture::new();
    write(
        fixture.user_claude.join("commands/large.md"),
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

    assert_eq!(error.code, "claude.command.expansion_too_large");
}

#[test]
fn arguments_without_a_placeholder_use_claude_codes_arguments_section() {
    let fixture = Fixture::new();
    write(
        fixture.user_claude.join("commands/summarize.md"),
        "Summarize this change",
    );

    let provider = fixture.provider();
    let snapshot = provider.discover(&fixture.context()).unwrap();

    assert_eq!(
        provider
            .expand(&fixture.context(), &snapshot.commands[0], "focus on auth",)
            .unwrap()
            .content,
        "Summarize this change\n\nARGUMENTS: focus on auth"
    );
}

#[test]
fn missing_and_escaped_argument_placeholders_remain_literal() {
    let fixture = Fixture::new();
    write(
        fixture.user_claude.join("commands/literal.md"),
        r"Use $0, keep $ARGUMENTS[3], and show \$ARGUMENTS plus \$1",
    );

    let provider = fixture.provider();
    let snapshot = provider.discover(&fixture.context()).unwrap();

    assert_eq!(
        provider
            .expand(&fixture.context(), &snapshot.commands[0], "alpha beta")
            .unwrap()
            .content,
        "Use alpha, keep $ARGUMENTS[3], and show $ARGUMENTS plus $1"
    );
}

#[test]
fn case_insensitive_duplicate_in_one_layer_is_invalid_and_deterministic() {
    let fixture = Fixture::new();
    write(fixture.user_claude.join("commands/Review.md"), "First");
    write(fixture.user_claude.join("commands/review.md"), "Second");

    if fs::read_dir(fixture.user_claude.join("commands"))
        .unwrap()
        .filter_map(Result::ok)
        .count()
        < 2
    {
        // Windows' default case-insensitive filesystem cannot represent this
        // upstream ambiguity. Linux CI exercises the duplicate-name branch.
        return;
    }

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    assert!(snapshot.commands.is_empty());
    assert_eq!(snapshot.unavailable_command_ids.len(), 1);
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "claude.command.duplicate_name"));
}

#[test]
fn invalid_higher_layer_command_masks_lower_layer_until_source_is_disabled() {
    let fixture = Fixture::new();
    write(
        fixture.project.join(".claude/commands/review.md"),
        "Project review",
    );
    write(
        fixture.user_claude.join("commands/review.md"),
        "---\ndescription: Invalid personal review\n---\n   ",
    );

    let provider = fixture.provider();
    let snapshot = provider.discover(&fixture.context()).unwrap();
    assert!(resolve(&provider, &snapshot).is_empty());

    let enabled_without_personal = snapshot
        .sources
        .iter()
        .filter(|source| {
            source.scope
                != openbitfun_product_domains::external_sources::ExternalSourceScope::UserGlobal
        })
        .map(|source| source.key.clone())
        .collect::<BTreeSet<_>>();
    let fallback = provider
        .resolve_commands(&snapshot, &enabled_without_personal)
        .unwrap();
    assert_eq!(fallback.len(), 1);
    assert_eq!(fallback[0].template, "Project review");
}

#[test]
fn malformed_higher_layer_frontmatter_masks_lower_layer_command() {
    let fixture = Fixture::new();
    write(
        fixture.project.join(".claude/commands/review.md"),
        "Project review",
    );
    write(
        fixture.user_claude.join("commands/review.md"),
        "---\nallowed-tools: [Read\n---\nPersonal review",
    );

    let provider = fixture.provider();
    let snapshot = provider.discover(&fixture.context()).unwrap();

    assert!(resolve(&provider, &snapshot).is_empty());
    assert_eq!(snapshot.unavailable_command_ids.len(), 1);
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "claude.command.markdown_invalid"));
}

#[test]
fn watch_roots_are_bounded_to_user_and_project_claude_directories() {
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
