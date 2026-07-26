use bitfun_opencode_adapter::{OpenCodeHookProvider, OpenCodeHookProviderOptions};
use bitfun_product_domains::external_hook_catalog::{
    ExternalHookHandlerKind, ExternalHookProjectionStatus, ExternalHookSourceProvider,
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
fn discovers_static_project_hooks_without_loading_the_plugin() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    let plugins = workspace.join(".opencode/plugins");
    fs::create_dir_all(&plugins).unwrap();
    fs::create_dir_all(&user).unwrap();
    fs::write(
        plugins.join("audit.ts"),
        r#"
export const AuditPlugin = async () => ({
  "tool.execute.before": async () => {},
  "tool.execute.after": async () => {},
  "session.compacted": async () => {},
})
"#,
    )
    .unwrap();

    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: true,
        project_root_override: Some(workspace.clone()),
    });
    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.sources.len(), 1);
    assert_eq!(snapshot.entries.len(), 3);
    assert!(snapshot
        .entries
        .iter()
        .all(|entry| { entry.handler_kind == ExternalHookHandlerKind::Function }));
    assert!(snapshot.entries.iter().any(|entry| {
        entry.mapping.as_ref().map(|mapping| mapping.hook_point)
            == Some(ExternalHookPoint::ToolAfter)
    }));
    assert!(snapshot.entries.iter().any(|entry| {
        entry.mapping.as_ref().map(|mapping| mapping.hook_point)
            == Some(ExternalHookPoint::ToolBefore)
    }));
    assert!(snapshot.entries.iter().any(|entry| {
        entry.native_event == "session.compacted"
            && entry.projection_status == ExternalHookProjectionStatus::Opaque
    }));
}

#[test]
fn discovers_hooks_from_every_named_plugin_export_in_module_order() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user.join("plugins")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        user.join("plugins/multiple.ts"),
        r#"
export const First = async () => ({ "chat.message": async () => {} })
export const Second = async () => ({ "tool.execute.after": async () => {} })
"#,
    )
    .unwrap();
    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: false,
        project_root_override: Some(workspace.clone()),
    });

    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(
        snapshot
            .entries
            .iter()
            .map(|entry| entry.native_event.as_str())
            .collect::<Vec<_>>(),
        vec!["chat.message", "tool.execute.after"]
    );
}

#[test]
fn preserves_each_binding_from_a_destructured_plugin_export() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user.join("plugins")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        user.join("plugins/destructured.ts"),
        r#"
const plugins = {
  First: async () => ({ "tool.execute.before": async () => {} }),
  Second: async () => ({ "tool.execute.after": async () => {} }),
}
export const { First, Second } = plugins
"#,
    )
    .unwrap();
    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: false,
        project_root_override: Some(workspace.clone()),
    });

    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.entries.len(), 2);
    assert!(snapshot
        .entries
        .iter()
        .all(|entry| entry.native_event == "<dynamic>"));
    assert_ne!(
        snapshot.entries[0].stable_key,
        snapshot.entries[1].stable_key
    );
}

#[test]
fn type_only_exports_do_not_create_dynamic_hook_entries() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user.join("plugins")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        user.join("plugins/types.ts"),
        r#"
type PluginShape = { name: string }
export type { PluginShape }
"#,
    )
    .unwrap();
    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: false,
        project_root_override: Some(workspace.clone()),
    });

    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.sources.len(), 1);
    assert!(snapshot.entries.is_empty());
}

#[test]
fn type_only_export_all_and_ambient_exports_do_not_create_hook_entries() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user.join("plugins")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        user.join("plugins/ambient.ts"),
        r#"
export type * from "./types"
export declare const Plugin: unknown
export declare function Factory(): unknown
export declare class PluginClass {}
"#,
    )
    .unwrap();
    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: false,
        project_root_override: Some(workspace.clone()),
    });

    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.sources.len(), 1);
    assert!(snapshot.entries.is_empty());
}

#[test]
fn declaration_files_are_not_discovered_as_runtime_plugins() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user.join("plugins")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        user.join("plugins/plugin.d.ts"),
        r#"
export function Plugin(): unknown
export class PluginClass {}
"#,
    )
    .unwrap();
    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: false,
        project_root_override: Some(workspace.clone()),
    });

    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert!(snapshot.sources.is_empty());
    assert!(snapshot.entries.is_empty());
}

#[test]
fn value_export_all_is_preserved_as_an_opaque_runtime_export() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user.join("plugins")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        user.join("plugins/reexport.ts"),
        r#"export * from "./plugins""#,
    )
    .unwrap();
    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: false,
        project_root_override: Some(workspace.clone()),
    });

    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.entries.len(), 1);
    assert_eq!(snapshot.entries[0].native_event, "<dynamic>");
    assert_eq!(
        snapshot.entries[0].projection_status,
        ExternalHookProjectionStatus::Opaque
    );
}

#[test]
fn preserves_same_event_registered_by_distinct_named_plugin_exports() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user.join("plugins")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        user.join("plugins/multiple.ts"),
        r#"
export const First = async () => ({ "tool.execute.before": async () => {} })
export const Second = async () => ({ "tool.execute.before": async () => {} })
"#,
    )
    .unwrap();
    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: false,
        project_root_override: Some(workspace.clone()),
    });

    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.entries.len(), 2);
    assert!(snapshot
        .entries
        .iter()
        .all(|entry| entry.native_event == "tool.execute.before"));
    assert_ne!(
        snapshot.entries[0].stable_key,
        snapshot.entries[1].stable_key
    );
}

#[test]
fn preserves_each_opaque_named_function_and_specifier_export() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user.join("plugins")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        user.join("plugins/functions.ts"),
        r#"
export async function First() { return { "tool.execute.before": async () => {} } }
export async function Second() { return { "tool.execute.after": async () => {} } }
"#,
    )
    .unwrap();
    fs::write(
        user.join("plugins/specifiers.ts"),
        r#"
const Third = async () => ({ "tool.execute.before": async () => {} })
const Fourth = async () => ({ "tool.execute.after": async () => {} })
export { Third, Fourth }
"#,
    )
    .unwrap();
    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: false,
        project_root_override: Some(workspace.clone()),
    });

    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.entries.len(), 4);
    assert!(snapshot
        .entries
        .iter()
        .all(|entry| entry.native_event == "<dynamic>"));
    assert_eq!(
        snapshot
            .entries
            .iter()
            .map(|entry| entry.stable_key.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        4
    );
}

#[test]
fn changing_only_plugin_handler_bodies_does_not_change_catalog_versions() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user.join("plugins")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    let plugin = user.join("plugins/audit.ts");
    let write = |secret: &str| {
        fs::write(
            &plugin,
            format!(r#"export const Audit = async () => ({{ "tool.execute.before": async () => {{ const token = "{secret}" }} }})"#),
        )
        .unwrap();
    };
    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: false,
        project_root_override: Some(workspace.clone()),
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
fn npm_packages_are_declared_only_and_malformed_plugins_are_isolated() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user.join("plugins")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        user.join("config.json"),
        r#"{"plugin":["@example/audit-plugin"]}"#,
    )
    .unwrap();
    fs::write(user.join("plugins/broken.ts"), "export const =").unwrap();

    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: true,
        project_root_override: Some(workspace.clone()),
    });
    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert!(snapshot.entries.is_empty());
    assert_eq!(snapshot.sources.len(), 2);
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.hook.package_declared_only"));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.hook.plugin_parse_failed"));
}

#[test]
fn current_jsonc_tuple_declarations_and_both_local_plugin_directories_are_visible() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user.join("plugin")).unwrap();
    fs::create_dir_all(user.join("plugins")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        user.join("opencode.jsonc"),
        r#"{
          // OpenCode accepts JSONC and tuple plugin specs.
          "plugin": [["@example/audit-plugin", {"mode":"safe"}],],
        }"#,
    )
    .unwrap();
    for directory in ["plugin", "plugins"] {
        fs::write(
            user.join(directory).join(format!("{directory}.ts")),
            r#"export const Audit = async () => ({ "tool.execute.before": async () => {} })"#,
        )
        .unwrap();
    }

    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: false,
        project_root_override: Some(workspace.clone()),
    });
    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.entries.len(), 2);
    assert_eq!(
        snapshot
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "opencode.hook.package_declared_only")
            .count(),
        1
    );
}

#[test]
fn project_root_opencode_config_declares_plugins_without_scanning_root_plugin_dirs() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user.clone()).unwrap();
    fs::create_dir_all(workspace.join("plugins")).unwrap();
    fs::write(
        workspace.join("opencode.json"),
        r#"{"plugin":["@example/project-plugin"]}"#,
    )
    .unwrap();
    fs::write(
        workspace.join("plugins/not-an-opencode-plugin.ts"),
        r#"export const WrongRoot = async () => ({ "tool.execute.before": async () => {} })"#,
    )
    .unwrap();

    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: true,
        project_root_override: Some(workspace.clone()),
    });
    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert!(snapshot.entries.is_empty());
    assert_eq!(snapshot.sources.len(), 1);
    assert_eq!(
        snapshot.sources[0].location_hint,
        "OpenCode project configuration/opencode.json"
    );
}

#[test]
fn computed_hook_registration_is_reported_as_opaque_instead_of_guessed() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user.join("plugins")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        user.join("plugins/dynamic.ts"),
        r#"
const eventName = "tool.execute.before"
export const Dynamic = async () => ({ [eventName]: async () => {} })
"#,
    )
    .unwrap();

    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: false,
        project_root_override: Some(workspace.clone()),
    });
    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.entries.len(), 1);
    assert_eq!(
        snapshot.entries[0].projection_status,
        ExternalHookProjectionStatus::Opaque
    );
    assert_eq!(snapshot.entries[0].native_event, "<dynamic>");
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.hook.registration_opaque"));
}

#[test]
fn total_hook_entries_are_bounded_across_many_plugins() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    let plugins = user.join("plugins");
    fs::create_dir_all(&plugins).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    let source = r#"
export const Many = async () => ({
  auth: async () => {},
  "chat.headers": async () => {},
  "chat.message": async () => {},
  "chat.params": async () => {},
  "command.execute.before": async () => {},
  config: async () => {},
  dispose: async () => {},
  event: async () => {},
  "experimental.chat.messages.transform": async () => {},
  "experimental.chat.system.transform": async () => {},
  "experimental.compaction.autocontinue": async () => {},
  "experimental.provider.small_model": async () => {},
  "experimental.session.compacting": async () => {},
  "experimental.text.complete": async () => {},
  "permission.ask": async () => {},
  provider: async () => {},
  "shell.env": async () => {},
  "tool.execute.after": async () => {},
  "tool.execute.before": async () => {},
  "tool.definition": async () => {},
})
"#;
    for index in 0..103 {
        fs::write(plugins.join(format!("many-{index:03}.ts")), source).unwrap();
    }

    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: false,
        project_root_override: Some(workspace.clone()),
    });
    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.entries.len(), 2048);
    assert_eq!(
        snapshot
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "opencode.hook.entry_limit")
            .count(),
        1
    );
}

#[test]
fn regex_literals_with_comment_markers_do_not_corrupt_static_parsing() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user.join("plugins")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        user.join("plugins/regex.ts"),
        r#"
const endpoint = /https?:\/\//
export const Audit = async () => ({ "tool.execute.before": async () => endpoint })
"#,
    )
    .unwrap();
    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: false,
        project_root_override: Some(workspace.clone()),
    });

    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.entries.len(), 1);
    assert_eq!(snapshot.entries[0].native_event, "tool.execute.before");
    assert!(!snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.hook.plugin_parse_failed"));
}

#[test]
fn inserting_another_static_event_does_not_churn_existing_entry_identity() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user.join("plugins")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    let plugin = user.join("plugins/audit.ts");
    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: false,
        project_root_override: Some(workspace.clone()),
    });
    fs::write(
        &plugin,
        r#"export const Audit = async () => ({ "tool.execute.before": async () => {} })"#,
    )
    .unwrap();
    let first = provider.discover(&context(&workspace)).unwrap();
    fs::write(
        &plugin,
        r#"export const Audit = async () => ({ "chat.message": async () => {}, "tool.execute.before": async () => {} })"#,
    )
    .unwrap();
    let second = provider.discover(&context(&workspace)).unwrap();
    let first_entry = &first.entries[0];
    let second_entry = second
        .entries
        .iter()
        .find(|entry| entry.native_event == "tool.execute.before")
        .unwrap();

    assert_eq!(first_entry.stable_key, second_entry.stable_key);
    assert_eq!(first_entry.content_version, second_entry.content_version);
}

#[test]
fn inserting_a_static_event_does_not_churn_opaque_entry_identity() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    fs::create_dir_all(user.join("plugins")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    let plugin = user.join("plugins/audit.ts");
    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: false,
        project_root_override: Some(workspace.clone()),
    });
    fs::write(
        &plugin,
        r#"export const Audit = async () => ({ "session.compacted": async () => {} })"#,
    )
    .unwrap();
    let first = provider.discover(&context(&workspace)).unwrap();
    fs::write(
        &plugin,
        r#"export const Audit = async () => ({ "tool.execute.before": async () => {}, "session.compacted": async () => {} })"#,
    )
    .unwrap();
    let second = provider.discover(&context(&workspace)).unwrap();
    let first_entry = &first.entries[0];
    let second_entry = second
        .entries
        .iter()
        .find(|entry| entry.native_event == "session.compacted")
        .unwrap();

    assert_eq!(first_entry.stable_key, second_entry.stable_key);
    assert_eq!(first_entry.content_version, second_entry.content_version);
}

#[test]
fn project_hook_discovery_includes_bounded_ancestor_directories() {
    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let project = root.path().join("project");
    let workspace = project.join("packages/app");
    for directory in [
        project.join(".opencode/plugins"),
        workspace.join(".opencode/plugins"),
    ] {
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("audit.ts"),
            r#"export const Audit = async () => ({ "tool.execute.before": async () => {} })"#,
        )
        .unwrap();
    }
    fs::create_dir_all(&user).unwrap();
    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: true,
        project_root_override: Some(project),
    });

    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.entries.len(), 2);
    assert_eq!(snapshot.sources.len(), 2);
}

#[cfg(unix)]
#[test]
fn plugin_symlinks_follow_native_file_discovery() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let user = root.path().join("user");
    let workspace = root.path().join("workspace");
    let plugins = user.join("plugins");
    fs::create_dir_all(&plugins).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    let target = root.path().join("shared.ts");
    fs::write(
        &target,
        r#"export const Audit = async () => ({ "tool.execute.after": async () => {} })"#,
    )
    .unwrap();
    symlink(&target, plugins.join("linked.ts")).unwrap();
    let provider = OpenCodeHookProvider::new(OpenCodeHookProviderOptions {
        user_config_dir: user,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: false,
        project_root_override: Some(workspace.clone()),
    });

    let snapshot = provider.discover(&context(&workspace)).unwrap();

    assert_eq!(snapshot.entries.len(), 1);
    assert_eq!(snapshot.entries[0].native_event, "tool.execute.after");
}
