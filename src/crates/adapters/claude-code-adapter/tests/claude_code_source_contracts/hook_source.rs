use openbitfun_claude_code_adapter::{ClaudeCodeHookProvider, ClaudeCodeHookProviderOptions};
use openbitfun_product_domains::external_hook_catalog::{
    ExternalHookHandlerKind, ExternalHookNativeActivation, ExternalHookProjectionStatus,
    ExternalHookSourceProvider,
};
use openbitfun_product_domains::external_hook_contributions::ExternalHookPoint;
use openbitfun_product_domains::external_sources::{ExecutionDomainId, ExternalSourceContext};
use std::fs;
use tempfile::tempdir;

fn context(workspace: &std::path::Path) -> ExternalSourceContext {
    ExternalSourceContext {
        workspace_root: Some(workspace.to_path_buf()),
        execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
    }
}

#[test]
fn prepares_only_the_supported_synchronous_claude_command_subset() {
    let root = tempdir().unwrap();
    let user_settings = root.path().join("home/.claude/settings.json");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user_settings.parent().unwrap()).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(user_settings.parent().unwrap().join("hooks")).unwrap();
    fs::write(
        user_settings.parent().unwrap().join("hooks/check.py"),
        b"print('claude')",
    )
    .unwrap();
    fs::write(
        &user_settings,
        r#"{"hooks":{"PreToolUse":[
          {"matcher":"Bash","hooks":[{"command":"python .claude/hooks/check.py","timeoutSec":12,"statusMessage":"Checking"}]},
          {"if":"private","hooks":[{"type":"command","command":"conditional"}]},
          {"hooks":[{"type":"command","command":"later","asyncRewake":true},{"type":"http","url":"https://private"}]}
        ],"Notification":[{"hooks":[{"type":"command","command":"notify"}]}]}}"#,
    )
    .unwrap();
    let provider = ClaudeCodeHookProvider::new(ClaudeCodeHookProviderOptions {
        user_settings_file: user_settings,
        project_root_override: Some(workspace.clone()),
        project_settings_enabled: true,
    });
    let ctx = context(&workspace);
    let catalog = provider.discover(&ctx).unwrap();
    let source = catalog.sources[0].clone();
    let prepared = provider
        .prepare_import(&ctx, &source.key, &source.content_version)
        .unwrap();

    assert_eq!(prepared.handlers.len(), 1);
    assert!(prepared.handlers[0]
        .command
        .contains("__OPENBITFUN_MANAGED_HOOK_ROOT__/hooks/check.py"));
    assert_eq!(prepared.handlers[0].timeout_seconds, Some(12));
    assert_eq!(
        prepared.handlers[0].status_message.as_deref(),
        Some("Checking")
    );
    assert_eq!(prepared.assets.len(), 1);
    assert!(prepared
        .skipped
        .iter()
        .any(|item| item.reason_code == "unsupported_group_field" && item.count == 1));
    assert!(prepared
        .skipped
        .iter()
        .any(|item| item.reason_code == "unsupported_behavior_field" && item.count == 1));
    assert!(prepared
        .skipped
        .iter()
        .any(|item| item.reason_code == "unsupported_handler_type" && item.count == 1));
    assert!(prepared
        .skipped
        .iter()
        .any(|item| item.reason_code == "unsupported_event" && item.count == 1));
}

#[test]
fn disable_all_hooks_yields_a_skipped_only_preview() {
    let root = tempdir().unwrap();
    let user_settings = root.path().join("settings.json");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        &user_settings,
        r#"{"disableAllHooks":true,"hooks":{"PreToolUse":[{"hooks":[{"command":"private"}]}]}}"#,
    )
    .unwrap();
    let provider = ClaudeCodeHookProvider::new(ClaudeCodeHookProviderOptions {
        user_settings_file: user_settings,
        project_root_override: Some(workspace.clone()),
        project_settings_enabled: false,
    });
    let ctx = context(&workspace);
    let catalog = provider.discover(&ctx).unwrap();
    let source = &catalog.sources[0];
    let prepared = provider
        .prepare_import(&ctx, &source.key, &source.content_version)
        .unwrap();

    assert!(prepared.handlers.is_empty());
    assert_eq!(prepared.skipped[0].reason_code, "all_disabled");
    assert_eq!(prepared.skipped[0].count, 1);
}

#[test]
fn layered_disable_is_respected_when_preparing_a_project_source() {
    let root = tempdir().unwrap();
    let user_settings = root.path().join("home/.claude/settings.json");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user_settings.parent().unwrap()).unwrap();
    fs::create_dir_all(workspace.join(".claude")).unwrap();
    fs::write(&user_settings, r#"{"disableAllHooks":true}"#).unwrap();
    fs::write(
        workspace.join(".claude/settings.json"),
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"command":"private"}]}]}}"#,
    )
    .unwrap();
    let provider = ClaudeCodeHookProvider::new(ClaudeCodeHookProviderOptions {
        user_settings_file: user_settings,
        project_root_override: Some(workspace.clone()),
        project_settings_enabled: true,
    });
    let ctx = context(&workspace);
    let catalog = provider.discover(&ctx).unwrap();
    let source = catalog
        .sources
        .iter()
        .find(|source| source.location_hint.replace('\\', "/") == ".claude/settings.json")
        .unwrap();
    let prepared = provider
        .prepare_import(&ctx, &source.key, &source.content_version)
        .unwrap();

    assert!(prepared.handlers.is_empty());
    assert_eq!(prepared.skipped[0].reason_code, "all_disabled");
}

#[test]
fn narrower_project_activation_cannot_enable_a_disabled_user_import() {
    let root = tempdir().unwrap();
    let user_settings = root.path().join("home/.claude/settings.json");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user_settings.parent().unwrap()).unwrap();
    fs::create_dir_all(workspace.join(".claude")).unwrap();
    fs::write(
        &user_settings,
        r#"{"disableAllHooks":true,"hooks":{"PreToolUse":[{"hooks":[{"command":"user-command"}]}]}}"#,
    )
    .unwrap();
    fs::write(
        workspace.join(".claude/settings.local.json"),
        r#"{"disableAllHooks":false}"#,
    )
    .unwrap();
    let provider = ClaudeCodeHookProvider::new(ClaudeCodeHookProviderOptions {
        user_settings_file: user_settings,
        project_root_override: Some(workspace.clone()),
        project_settings_enabled: true,
    });
    let ctx = context(&workspace);
    let catalog = provider.discover(&ctx).unwrap();
    let source = catalog
        .sources
        .iter()
        .find(|source| {
            source.scope
                == openbitfun_product_domains::external_sources::ExternalSourceScope::UserGlobal
        })
        .unwrap();

    let prepared = provider
        .prepare_import(&ctx, &source.key, &source.content_version)
        .unwrap();

    assert!(prepared.handlers.is_empty());
    assert_eq!(prepared.skipped[0].reason_code, "all_disabled");
}

#[test]
fn skipped_handlers_do_not_leave_assets_in_another_valid_handler_plan() {
    let root = tempdir().unwrap();
    let user_settings = root.path().join("home/.claude/settings.json");
    let workspace = root.path().join("workspace");
    let hooks = user_settings.parent().unwrap().join("hooks");
    fs::create_dir_all(&hooks).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(hooks.join("valid.py"), b"print('valid')").unwrap();
    fs::write(hooks.join("skipped.py"), b"print('must not copy')").unwrap();
    fs::write(
        &user_settings,
        r#"{"hooks":{"PreToolUse":[{"hooks":[
          {"command":"python .claude/hooks/valid.py"},
          {"command":"python .claude/hooks/skipped.py","timeoutSec":0}
        ]}]}}"#,
    )
    .unwrap();
    let provider = ClaudeCodeHookProvider::new(ClaudeCodeHookProviderOptions {
        user_settings_file: user_settings,
        project_root_override: Some(workspace.clone()),
        project_settings_enabled: false,
    });
    let ctx = context(&workspace);
    let catalog = provider.discover(&ctx).unwrap();
    let source = &catalog.sources[0];

    let prepared = provider
        .prepare_import(&ctx, &source.key, &source.content_version)
        .unwrap();

    assert_eq!(prepared.handlers.len(), 1);
    assert_eq!(prepared.assets.len(), 1);
    assert_eq!(
        prepared.assets[0].relative_path,
        std::path::PathBuf::from("hooks/valid.py")
    );
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
