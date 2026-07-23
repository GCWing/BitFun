use bitfun_claude_code_adapter::{ClaudeCodeHookProvider, ClaudeCodeHookProviderOptions};
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
fn discovers_user_project_and_local_settings_with_native_handler_kinds() {
    let root = tempdir().unwrap();
    let user_settings = root.path().join("home/.claude/settings.json");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user_settings.parent().unwrap()).unwrap();
    fs::create_dir_all(workspace.join(".claude")).unwrap();
    fs::write(
        &user_settings,
        r#"{
          "hooks": {
            "PreToolUse": [{"matcher":"Bash|Edit","hooks":[{"type":"command","command":"secret-command --token abc"}]}],
            "SessionStart": [{"hooks":[{"type":"http","url":"https://secret.example/hook"}]}]
          }
        }"#,
    )
    .unwrap();
    fs::write(
        workspace.join(".claude/settings.json"),
        r#"{"hooks":{"PostToolUse":[{"matcher":"mcp__.*","hooks":[{"type":"mcp_tool","server":"private","tool":"audit"}]}],"Stop":[{"hooks":[{"type":"prompt","prompt":"private prompt"}]}]}}"#,
    )
    .unwrap();
    fs::write(
        workspace.join(".claude/settings.local.json"),
        r#"{"hooks":{"PermissionRequest":[{"hooks":[{"type":"agent","prompt":"private agent task"}]}]}}"#,
    )
    .unwrap();

    let provider = ClaudeCodeHookProvider::new(ClaudeCodeHookProviderOptions {
        user_settings_file: user_settings,
        project_root_override: Some(workspace.clone()),
        project_settings_enabled: true,
    });
    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.sources.len(), 3);
    assert_eq!(snapshot.entries.len(), 5);
    assert!(snapshot.entries.iter().any(|entry| {
        entry.handler_kind == ExternalHookHandlerKind::Command
            && entry.mapping.as_ref().map(|mapping| mapping.hook_point)
                == Some(ExternalHookPoint::ToolBefore)
    }));
    assert!(snapshot.entries.iter().any(|entry| {
        entry.handler_kind == ExternalHookHandlerKind::McpTool
            && entry.mapping.as_ref().map(|mapping| mapping.hook_point)
                == Some(ExternalHookPoint::ToolAfter)
    }));
    assert!(snapshot.entries.iter().any(|entry| {
        entry.native_event == "SessionStart"
            && entry.projection_status == ExternalHookProjectionStatus::NativeOnly
    }));

    let serialized = serde_json::to_string(&snapshot).unwrap();
    assert!(!serialized.contains("secret-command"));
    assert!(!serialized.contains("secret.example"));
    assert!(!serialized.contains("private prompt"));
    assert!(!serialized.contains("private agent task"));
}

#[test]
fn malformed_handlers_are_isolated_and_disable_all_hooks_is_visible() {
    let root = tempdir().unwrap();
    let user_settings = root.path().join("settings.json");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        &user_settings,
        r#"{
          "disableAllHooks": true,
          "hooks": {
            "PreToolUse": [{"hooks":[{"command":"missing type"},{"type":"command","command":"valid"}]}]
          }
        }"#,
    )
    .unwrap();

    let provider = ClaudeCodeHookProvider::new(ClaudeCodeHookProviderOptions {
        user_settings_file: user_settings,
        project_root_override: Some(workspace.clone()),
        project_settings_enabled: false,
    });
    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.entries.len(), 1);
    assert_eq!(
        snapshot.entries[0].native_activation,
        ExternalHookNativeActivation::Disabled
    );
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "claude.hook.handler_invalid"));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "claude.hook.all_disabled"));
}

#[test]
fn changing_only_handler_secrets_does_not_change_catalog_versions() {
    let root = tempdir().unwrap();
    let user_settings = root.path().join("settings.json");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let write = |secret: &str| {
        fs::write(
            &user_settings,
            format!(r#"{{"hooks":{{"PreToolUse":[{{"hooks":[{{"type":"command","command":"{secret}"}}]}}]}}}}"#),
        )
        .unwrap();
    };
    let provider = ClaudeCodeHookProvider::new(ClaudeCodeHookProviderOptions {
        user_settings_file: user_settings.clone(),
        project_root_override: Some(workspace.clone()),
        project_settings_enabled: false,
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
fn layered_disable_applies_to_user_and_nested_project_hooks() {
    let root = tempdir().unwrap();
    let user_settings = root.path().join("home/.claude/settings.json");
    let project = root.path().join("project");
    let workspace = project.join("packages/app");
    fs::create_dir_all(user_settings.parent().unwrap()).unwrap();
    for directory in [project.join(".claude"), workspace.join(".claude")] {
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("settings.json"),
            r#"{"hooks":{"PostToolUse":[{"hooks":[{"type":"command","command":"private"}]}]}}"#,
        )
        .unwrap();
    }
    fs::write(
        &user_settings,
        r#"{"disableAllHooks":true,"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"private"}]}]}}"#,
    )
    .unwrap();
    let provider = ClaudeCodeHookProvider::new(ClaudeCodeHookProviderOptions {
        user_settings_file: user_settings,
        project_root_override: Some(project),
        project_settings_enabled: true,
    });

    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.sources.len(), 3);
    assert_eq!(snapshot.entries.len(), 3);
    assert!(snapshot
        .entries
        .iter()
        .all(|entry| { entry.native_activation == ExternalHookNativeActivation::Disabled }));
}

#[test]
fn layered_activation_changes_update_every_affected_entry_version() {
    let root = tempdir().unwrap();
    let user_settings = root.path().join("home/.claude/settings.json");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user_settings.parent().unwrap()).unwrap();
    fs::create_dir_all(workspace.join(".claude")).unwrap();
    fs::write(
        workspace.join(".claude/settings.json"),
        r#"{"hooks":{"PostToolUse":[{"hooks":[{"type":"command","command":"private"}]}]}}"#,
    )
    .unwrap();
    let provider = ClaudeCodeHookProvider::new(ClaudeCodeHookProviderOptions {
        user_settings_file: user_settings.clone(),
        project_root_override: Some(workspace.clone()),
        project_settings_enabled: true,
    });
    fs::write(&user_settings, r#"{"disableAllHooks":false}"#).unwrap();
    let enabled = provider.discover(&context(&workspace)).unwrap();
    fs::write(&user_settings, r#"{"disableAllHooks":true}"#).unwrap();
    let disabled = provider.discover(&context(&workspace)).unwrap();
    let enabled_project = enabled
        .entries
        .iter()
        .find(|entry| entry.native_event == "PostToolUse")
        .unwrap();
    let disabled_project = disabled
        .entries
        .iter()
        .find(|entry| entry.native_event == "PostToolUse")
        .unwrap();

    assert_eq!(
        enabled_project.native_activation,
        ExternalHookNativeActivation::Unknown
    );
    assert_eq!(
        disabled_project.native_activation,
        ExternalHookNativeActivation::Disabled
    );
    assert_ne!(
        enabled_project.content_version,
        disabled_project.content_version
    );
}
