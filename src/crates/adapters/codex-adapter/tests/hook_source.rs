use bitfun_codex_adapter::{CodexHookProvider, CodexHookProviderOptions};
use bitfun_product_domains::external_hook_catalog::{
    ExternalHookHandlerKind, ExternalHookNativeActivation, ExternalHookProjectionStatus,
    ExternalHookSourceProvider,
};
use bitfun_product_domains::external_hook_contributions::ExternalHookPoint;
use bitfun_product_domains::external_sources::{ExecutionDomainId, ExternalSourceContext};
use std::fs;
use tempfile::tempdir;

fn context(workspace: &std::path::Path) -> ExternalSourceContext {
    ExternalSourceContext {
        workspace_root: Some(workspace.to_path_buf()),
        execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
    }
}

#[test]
fn discovers_hooks_json_and_inline_toml_without_exposing_handler_content() {
    let root = tempdir().unwrap();
    let codex_home = root.path().join("home/.codex");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(&codex_home).unwrap();
    fs::create_dir_all(workspace.join(".codex")).unwrap();
    fs::write(
        codex_home.join("hooks.json"),
        r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"secret command"}]}],"SessionStart":[{"hooks":[{"type":"prompt","prompt":"private prompt"}]}]}}"#,
    )
    .unwrap();
    fs::write(
        codex_home.join("config.toml"),
        r#"
[[hooks.PostToolUse]]
matcher = "Edit|Write"

[[hooks.PostToolUse.hooks]]
type = "agent"
prompt = "private agent prompt"
"#,
    )
    .unwrap();
    fs::write(
        workspace.join(".codex/hooks.json"),
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"project secret"}]}]}}"#,
    )
    .unwrap();

    let provider = CodexHookProvider::new(CodexHookProviderOptions {
        codex_home,
        project_root_override: Some(workspace.clone()),
        project_hooks_root_override: None,
        project_hooks_enabled: true,
    });
    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.sources.len(), 3);
    assert_eq!(snapshot.entries.len(), 4);
    assert!(snapshot.entries.iter().any(|entry| {
        entry.handler_kind == ExternalHookHandlerKind::Command
            && entry.mapping.as_ref().map(|mapping| mapping.hook_point)
                == Some(ExternalHookPoint::ToolBefore)
    }));
    assert!(snapshot.entries.iter().any(|entry| {
        entry.handler_kind == ExternalHookHandlerKind::Agent
            && entry.mapping.as_ref().map(|mapping| mapping.hook_point)
                == Some(ExternalHookPoint::ToolAfter)
    }));
    assert!(snapshot.entries.iter().any(|entry| {
        entry.native_event == "SessionStart"
            && entry.projection_status == ExternalHookProjectionStatus::NativeOnly
    }));

    let serialized = serde_json::to_string(&snapshot).unwrap();
    assert!(!serialized.contains("secret command"));
    assert!(!serialized.contains("private prompt"));
    assert!(!serialized.contains("private agent prompt"));
    assert!(!serialized.contains("project secret"));
}

#[test]
fn invalid_files_and_handlers_are_partial_diagnostics() {
    let root = tempdir().unwrap();
    let codex_home = root.path().join("home/.codex");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(&codex_home).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(codex_home.join("hooks.json"), "{ invalid").unwrap();
    fs::write(
        codex_home.join("config.toml"),
        r#"
[[hooks.PreToolUse]]
[[hooks.PreToolUse.hooks]]
type = "http"
url = "https://unsupported.example"
"#,
    )
    .unwrap();

    let provider = CodexHookProvider::new(CodexHookProviderOptions {
        codex_home,
        project_root_override: Some(workspace.clone()),
        project_hooks_root_override: None,
        project_hooks_enabled: false,
    });
    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert!(snapshot.entries.is_empty());
    assert_eq!(snapshot.sources.len(), 2);
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "codex.hook.config_parse_failed"));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "codex.hook.handler_invalid"));
}

#[test]
fn user_feature_gate_does_not_claim_effective_activation_without_session_layers() {
    let root = tempdir().unwrap();
    let codex_home = root.path().join("home/.codex");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(&codex_home).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        codex_home.join("config.toml"),
        r#"
[features]
hooks = false

[[hooks.PreToolUse]]
[[hooks.PreToolUse.hooks]]
type = "command"
command = "never expose this"
"#,
    )
    .unwrap();

    let provider = CodexHookProvider::new(CodexHookProviderOptions {
        codex_home,
        project_root_override: Some(workspace.clone()),
        project_hooks_root_override: None,
        project_hooks_enabled: false,
    });
    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.entries.len(), 1);
    assert_eq!(
        snapshot.entries[0].native_activation,
        ExternalHookNativeActivation::Unknown
    );
    assert!(!snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "codex.hook.all_disabled"));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "codex.hook.coverage_static_only"));
}

#[test]
fn rejects_unknown_hooks_json_root_fields_and_omits_unknown_events() {
    let root = tempdir().unwrap();
    let codex_home = root.path().join("home/.codex");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(&codex_home).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        codex_home.join("hooks.json"),
        r#"{"hooks":{"PreToolUes":[{"hooks":[{"type":"command","command":"typo"}]}]},"unexpected":true}"#,
    )
    .unwrap();
    fs::write(
        codex_home.join("config.toml"),
        r#"
[[hooks.PreToolUes]]
[[hooks.PreToolUes.hooks]]
type = "command"
command = "typo"
"#,
    )
    .unwrap();
    let provider = CodexHookProvider::new(CodexHookProviderOptions {
        codex_home,
        project_root_override: Some(workspace.clone()),
        project_hooks_root_override: None,
        project_hooks_enabled: false,
    });

    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert!(snapshot.entries.is_empty());
    assert!(snapshot.diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_str(),
            "codex.hook.config_parse_failed" | "codex.hook.event_name_invalid"
        )
    }));
}

#[test]
fn user_state_does_not_claim_effective_activation_without_session_layers() {
    let root = tempdir().unwrap();
    let codex_home = root.path().join("home/.codex");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(&codex_home).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    let config_path = codex_home.join("config.toml");
    let state_key = format!("{}:pre_tool_use:0:0", config_path.display()).replace('\\', "\\\\");
    fs::write(
        &config_path,
        format!(
            r#"
[[hooks.PreToolUse]]
[[hooks.PreToolUse.hooks]]
type = "command"
command = "private"

[hooks.state."{state_key}"]
enabled = false
trusted_hash = "must-not-be-exposed"
"#
        ),
    )
    .unwrap();
    let provider = CodexHookProvider::new(CodexHookProviderOptions {
        codex_home,
        project_root_override: Some(workspace.clone()),
        project_hooks_root_override: None,
        project_hooks_enabled: false,
    });

    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(
        snapshot.entries[0].native_activation,
        ExternalHookNativeActivation::Unknown
    );
    assert!(!serde_json::to_string(&snapshot)
        .unwrap()
        .contains("must-not-be-exposed"));
}

#[test]
fn user_state_does_not_claim_project_hook_activation_without_session_layers() {
    let root = tempdir().unwrap();
    let codex_home = root.path().join("home/.codex");
    let workspace = root.path().join("workspace");
    let project_codex = workspace.join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    fs::create_dir_all(&project_codex).unwrap();
    let project_hooks = project_codex.join("hooks.json");
    fs::write(
        &project_hooks,
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"private"}]}]}}"#,
    )
    .unwrap();
    let state_key = format!("{}:pre_tool_use:0:0", project_hooks.display()).replace('\\', "\\\\");
    fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
[hooks.state."{state_key}"]
enabled = false
"#
        ),
    )
    .unwrap();
    let provider = CodexHookProvider::new(CodexHookProviderOptions {
        codex_home,
        project_root_override: Some(workspace.clone()),
        project_hooks_root_override: None,
        project_hooks_enabled: true,
    });

    let snapshot = provider.discover(&context(&workspace)).unwrap();
    let project_entry = snapshot
        .entries
        .iter()
        .find(|entry| {
            entry
                .source
                .source_id
                .as_str()
                .starts_with("project-hooks-json-")
        })
        .unwrap();
    assert_eq!(
        project_entry.native_activation,
        ExternalHookNativeActivation::Unknown
    );
}

#[test]
fn changing_only_a_command_secret_does_not_change_catalog_versions() {
    let root = tempdir().unwrap();
    let codex_home = root.path().join("home/.codex");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(&codex_home).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    let hooks = codex_home.join("hooks.json");
    let write = |secret: &str| {
        fs::write(
            &hooks,
            format!(r#"{{"hooks":{{"PreToolUse":[{{"hooks":[{{"type":"command","command":"{secret}"}}]}}]}}}}"#),
        )
        .unwrap();
    };
    let provider = CodexHookProvider::new(CodexHookProviderOptions {
        codex_home,
        project_root_override: Some(workspace.clone()),
        project_hooks_root_override: None,
        project_hooks_enabled: false,
    });
    write("token-one");
    let first = provider.discover(&context(&workspace)).unwrap();
    write("token-two");
    let second = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(
        first.sources[0].content_version,
        second.sources[0].content_version
    );
    assert_eq!(
        first.entries[0].content_version,
        second.entries[0].content_version
    );
}

#[test]
fn linked_worktree_hooks_use_primary_checkout_across_nested_project_layers() {
    let root = tempdir().unwrap();
    let codex_home = root.path().join("home/.codex");
    let linked = root.path().join("worktrees/feature");
    let workspace = linked.join("packages/app");
    let primary = root.path().join("repo");
    fs::create_dir_all(&codex_home).unwrap();
    fs::create_dir_all(&linked).unwrap();
    fs::write(
        linked.join(".git"),
        "gitdir: ../../repo/.git/worktrees/feature",
    )
    .unwrap();
    for directory in [primary.join(".codex"), primary.join("packages/app/.codex")] {
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("hooks.json"),
            r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"private"}]}]}}"#,
        )
        .unwrap();
    }
    fs::create_dir_all(&workspace).unwrap();
    let provider = CodexHookProvider::new(CodexHookProviderOptions {
        codex_home,
        project_root_override: Some(linked),
        project_hooks_root_override: Some(primary),
        project_hooks_enabled: true,
    });

    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.sources.len(), 2);
    assert_eq!(snapshot.entries.len(), 2);
    assert!(snapshot
        .sources
        .iter()
        .any(|source| source.location_hint == ".codex/hooks.json"));
    assert!(snapshot.sources.iter().any(|source| source
        .location_hint
        .replace('\\', "/")
        .ends_with("packages/app/.codex/hooks.json")));
}

#[test]
fn linked_worktree_inline_hooks_use_primary_checkout_and_ignore_project_state() {
    let root = tempdir().unwrap();
    let codex_home = root.path().join("home/.codex");
    let linked = root.path().join("worktrees/feature");
    let primary = root.path().join("repo");
    fs::create_dir_all(&codex_home).unwrap();
    fs::create_dir_all(linked.join(".codex")).unwrap();
    fs::create_dir_all(primary.join(".codex")).unwrap();
    fs::write(
        linked.join(".git"),
        "gitdir: ../../repo/.git/worktrees/feature",
    )
    .unwrap();
    let primary_config = primary.join(".codex/config.toml");
    let state_key = format!("{}:pre_tool_use:0:0", primary_config.display()).replace('\\', "\\\\");
    fs::write(
        &primary_config,
        format!(
            r#"
[[hooks.PreToolUse]]
[[hooks.PreToolUse.hooks]]
type = "command"
command = "primary private"

[hooks.state."{state_key}"]
enabled = false
"#
        ),
    )
    .unwrap();
    fs::write(
        linked.join(".codex/config.toml"),
        r#"
[features]
hooks = false

[[hooks.PostToolUse]]
[[hooks.PostToolUse.hooks]]
type = "command"
command = "linked private"
"#,
    )
    .unwrap();
    let provider = CodexHookProvider::new(CodexHookProviderOptions {
        codex_home,
        project_root_override: Some(linked.clone()),
        project_hooks_root_override: Some(primary),
        project_hooks_enabled: true,
    });

    let snapshot = provider.discover(&context(&linked)).unwrap();

    assert_eq!(snapshot.entries.len(), 1);
    assert_eq!(snapshot.entries[0].native_event, "PreToolUse");
    assert_eq!(
        snapshot.entries[0].native_activation,
        ExternalHookNativeActivation::Unknown
    );
    assert!(!serde_json::to_string(&snapshot)
        .unwrap()
        .contains("linked private"));
}

#[test]
fn custom_project_root_markers_bound_codex_ancestor_layers() {
    let root = tempdir().unwrap();
    let codex_home = root.path().join("home/.codex");
    let project = root.path().join("project");
    let package = project.join("packages");
    let workspace = package.join("app");
    fs::create_dir_all(&codex_home).unwrap();
    fs::write(
        codex_home.join("config.toml"),
        r#"project_root_markers = ["package.json"]"#,
    )
    .unwrap();
    fs::create_dir_all(project.join(".codex")).unwrap();
    fs::create_dir_all(package.join(".codex")).unwrap();
    fs::create_dir_all(workspace.join(".codex")).unwrap();
    fs::write(package.join("package.json"), "{}").unwrap();
    for (directory, event) in [
        (project.join(".codex"), "SessionStart"),
        (package.join(".codex"), "PreToolUse"),
        (workspace.join(".codex"), "PostToolUse"),
    ] {
        fs::write(
            directory.join("hooks.json"),
            format!(
                r#"{{"hooks":{{"{event}":[{{"hooks":[{{"type":"command","command":"private"}}]}}]}}}}"#
            ),
        )
        .unwrap();
    }
    let provider = CodexHookProvider::new(CodexHookProviderOptions {
        codex_home,
        project_root_override: Some(project),
        project_hooks_root_override: None,
        project_hooks_enabled: true,
    });

    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.entries.len(), 2);
    assert!(snapshot
        .entries
        .iter()
        .any(|entry| entry.native_event == "PreToolUse"));
    assert!(snapshot
        .entries
        .iter()
        .any(|entry| entry.native_event == "PostToolUse"));
    assert!(!snapshot
        .entries
        .iter()
        .any(|entry| entry.native_event == "SessionStart"));
}

#[test]
fn empty_project_root_markers_limit_codex_hooks_to_workspace() {
    let root = tempdir().unwrap();
    let codex_home = root.path().join("home/.codex");
    let project = root.path().join("project");
    let workspace = project.join("packages/app");
    fs::create_dir_all(&codex_home).unwrap();
    fs::write(codex_home.join("config.toml"), "project_root_markers = []").unwrap();
    fs::create_dir_all(project.join(".codex")).unwrap();
    fs::create_dir_all(workspace.join(".codex")).unwrap();
    fs::write(
        project.join(".codex/hooks.json"),
        r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"private"}]}]}}"#,
    )
    .unwrap();
    fs::write(
        workspace.join(".codex/hooks.json"),
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"private"}]}]}}"#,
    )
    .unwrap();
    let provider = CodexHookProvider::new(CodexHookProviderOptions {
        codex_home,
        project_root_override: Some(project),
        project_hooks_root_override: None,
        project_hooks_enabled: true,
    });

    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.entries.len(), 1);
    assert_eq!(snapshot.entries[0].native_event, "PreToolUse");
}
