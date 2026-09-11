//! Cordis Hook bridge declarations are inspected without composing a profile.
use openbitfun_product_domains::external_hook_catalog::*;
use openbitfun_product_domains::external_sources::*;
use openbitfun_static_hook_support::{
    parse_hook_document, read_bounded_file, regular_file_exists, BoundedFileRead,
    StaticHookCatalog, StaticHookDocumentFormat, StaticHookHandlerRule,
};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

const MAX_BYTES: usize = 1024 * 1024;
const RULES: &[StaticHookHandlerRule] = &[
    StaticHookHandlerRule::new("command", ExternalHookHandlerKind::Command, &["command"]),
    StaticHookHandlerRule::new("http", ExternalHookHandlerKind::Http, &["url"]),
    StaticHookHandlerRule::new("mcp_tool", ExternalHookHandlerKind::McpTool, &[]),
    StaticHookHandlerRule::new("prompt", ExternalHookHandlerKind::Prompt, &[]),
    StaticHookHandlerRule::new("agent", ExternalHookHandlerKind::Agent, &[]),
];

#[derive(Debug, Clone)]
pub struct DshHookProviderOptions {
    pub dsh_home: PathBuf,
}
impl Default for DshHookProviderOptions {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let dsh_home = std::env::var("DSH_HOME")
            .ok()
            .filter(|value| !value.is_empty())
            .map(|value| {
                if value == "~" {
                    home.clone()
                } else if let Some(suffix) = value.strip_prefix("~/") {
                    home.join(suffix)
                } else {
                    PathBuf::from(value)
                }
            })
            .unwrap_or_else(|| home.join(".dsh"));
        Self { dsh_home }
    }
}

#[derive(Default)]
pub struct DshHookProvider {
    options: DshHookProviderOptions,
}
impl DshHookProvider {
    pub fn new(options: DshHookProviderOptions) -> Self {
        Self { options }
    }
}

fn read(path: &Path) -> Result<Option<Vec<u8>>, ExternalSourceProviderError> {
    let error = || {
        ExternalSourceProviderError::new(
            "dsh.hook.read_failed",
            "Could not read DeepSeek Harness Hook sources",
            true,
        )
    };
    if !regular_file_exists(path).map_err(|_| error())? {
        return Ok(None);
    }
    match read_bounded_file(path, MAX_BYTES).map_err(|_| error())? {
        BoundedFileRead::Content(bytes) => Ok(Some(bytes)),
        BoundedFileRead::TooLarge => Err(ExternalSourceProviderError::new(
            "dsh.hook.file_limit",
            "DeepSeek Harness source exceeds the static discovery size limit",
            false,
        )),
    }
}

impl ExternalHookSourceProvider for DshHookProvider {
    fn identity(&self) -> ExternalHookProviderIdentity {
        ExternalHookProviderIdentity::new(
            "deepseek-harness.hooks",
            "deepseek-harness",
            "DeepSeek Harness Hooks",
        )
        .expect("static provider")
    }

    fn discover(
        &self,
        context: &ExternalSourceContext,
    ) -> Result<ExternalHookProviderSnapshot, ExternalSourceProviderError> {
        if context
            .workspace_root
            .as_ref()
            .is_some_and(|path| !path.is_absolute())
        {
            return Err(ExternalSourceProviderError::new(
                "dsh.hook.workspace_invalid",
                "Workspace root must be absolute",
                false,
            ));
        }
        let mut catalog = StaticHookCatalog::new(self.identity());
        let mut configs = vec![(
            self.options.dsh_home.join("cordis.patch.yml"),
            ExternalSourceScope::UserGlobal,
        )];
        let profiles = self.options.dsh_home.join("profiles");
        match std::fs::read_dir(&profiles) {
            Ok(entries) => {
                let mut roots = Vec::new();
                for entry in entries.take(129) {
                    let entry = entry.map_err(|_| {
                        ExternalSourceProviderError::new(
                            "dsh.hook.profiles_unreadable",
                            "Could not list DeepSeek Harness profiles",
                            true,
                        )
                    })?;
                    if entry.file_name() != "node_modules" {
                        roots.push(entry.path());
                    }
                }
                if roots.len() > 128 {
                    return Err(ExternalSourceProviderError::new(
                        "dsh.hook.profile_limit",
                        "DeepSeek Harness profile limit reached",
                        false,
                    ));
                }
                roots.sort();
                for root in roots {
                    if !root.is_dir() {
                        continue;
                    }
                    for filename in ["cordis.yml", "cordis.patch.yml"] {
                        configs.push((root.join(filename), ExternalSourceScope::UserGlobal));
                    }
                    let manifest = root.join("package.json");
                    if let Some(bytes) = read(&manifest)? {
                        if serde_json::from_slice::<Value>(&bytes)
                            .ok()
                            .and_then(|value| value.pointer("/dsh/profile/bundles").cloned())
                            .and_then(|value| value.as_array().cloned())
                            .is_some_and(|bundles| !bundles.is_empty())
                        {
                            let index = catalog.source(
                                &manifest.to_string_lossy(),
                                &manifest.to_string_lossy(),
                                ExternalHookSourceKind::PackageDeclaration,
                                ExternalSourceScope::UserGlobal,
                            )?;
                            catalog.warning(index, "dsh.hook.bundles_opaque", "Profile bundle and native Cordis registrations require composition; only explicit local Hook bridge declarations are inspected");
                        }
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                return Err(ExternalSourceProviderError::new(
                    "dsh.hook.profiles_unreadable",
                    "Could not list DeepSeek Harness profiles",
                    true,
                ))
            }
        }
        // Cordis composition files in the selected workspace are discoverable
        // sources, not evidence that a native profile has loaded them.
        if let Some(root) = &context.workspace_root {
            for filename in ["cordis.yml", "cordis.patch.yml"] {
                configs.push((root.join(filename), ExternalSourceScope::Project));
            }
        }
        let mut seen = BTreeSet::new();
        for (path, scope) in configs {
            let Some(bytes) = read(&path)? else {
                continue;
            };
            if !seen.insert(path.clone()) {
                continue;
            }
            let index = catalog.source(
                &path.to_string_lossy(),
                &path.to_string_lossy(),
                ExternalHookSourceKind::Settings,
                scope,
            )?;
            let value: Value = match serde_yaml::from_slice(&bytes) {
                Ok(value) => value,
                Err(_) => {
                    catalog.warning(
                        index,
                        "dsh.hook.config_invalid",
                        "Cordis configuration is not valid YAML",
                    );
                    continue;
                }
            };
            let Some(rows) = value.as_array() else {
                catalog.warning(
                    index,
                    "dsh.hook.config_invalid",
                    "Cordis configuration must be a list",
                );
                continue;
            };
            inspect_rows(
                rows,
                false,
                0,
                &mut 0usize,
                index,
                &path,
                scope,
                context,
                &mut catalog,
            )?;
        }
        catalog.finish()
    }
}

#[allow(clippy::too_many_arguments)]
fn inspect_rows(
    rows: &[Value],
    parent_disabled: bool,
    depth: usize,
    inspected: &mut usize,
    config_index: usize,
    config_path: &Path,
    scope: ExternalSourceScope,
    context: &ExternalSourceContext,
    catalog: &mut StaticHookCatalog,
) -> Result<(), ExternalSourceProviderError> {
    if depth > 16 {
        catalog.warning(
            config_index,
            "dsh.hook.depth_limit",
            "Cordis group depth limit reached",
        );
        return Ok(());
    }
    for row in rows {
        *inspected += 1;
        if *inspected > 2048 {
            catalog.warning(
                config_index,
                "dsh.hook.row_limit",
                "Cordis row limit reached",
            );
            break;
        }
        let disabled =
            parent_disabled || row.get("disabled").and_then(Value::as_bool) == Some(true);
        if let Some(insert) = row.get("insert").and_then(Value::as_array) {
            inspect_rows(
                insert,
                disabled,
                depth + 1,
                inspected,
                config_index,
                config_path,
                scope,
                context,
                catalog,
            )?;
        }
        if row.get("group").and_then(Value::as_bool) == Some(true) {
            if let Some(group) = row.get("config").and_then(Value::as_array) {
                inspect_rows(
                    group,
                    disabled,
                    depth + 1,
                    inspected,
                    config_index,
                    config_path,
                    scope,
                    context,
                    catalog,
                )?;
            }
        }
        let name = row.get("name").and_then(Value::as_str).unwrap_or_default();
        let dialect = match name {
            "@deepseek-ai/dsh-hooks-claude-code" | "dsh-hooks-claude-code" => "claude-code",
            "@deepseek-ai/dsh-hooks-codex" | "dsh-hooks-codex" => "codex",
            _ => {
                if row.pointer("/config/configPath").is_some() && name.is_empty() {
                    catalog.warning(config_index, "dsh.hook.patch_opaque", "A patch references a configuration file without a statically identified bridge; profile composition is required");
                }
                continue;
            }
        };
        let Some(reference) = row
            .pointer("/config/configPath")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        else {
            catalog.warning(
                config_index,
                "dsh.hook.config_path_invalid",
                "DeepSeek Harness Hook bridge requires a static configPath",
            );
            continue;
        };
        if reference.contains(['$', '\n', '\r'])
            || reference.contains("://")
            || reference.starts_with('~')
        {
            catalog.warning(
                config_index,
                "dsh.hook.config_path_opaque",
                "Hook configPath cannot be resolved statically",
            );
            continue;
        }
        let path = if Path::new(reference).is_absolute() {
            PathBuf::from(reference)
        } else if let Some(cwd) = &context.workspace_root {
            cwd.join(reference)
        } else {
            catalog.warning(
                config_index,
                "dsh.hook.cwd_required",
                "Relative Hook configPath requires a selected launch workspace",
            );
            continue;
        };
        let identity = format!(
            "{}:{inspected}:{dialect}:{}",
            config_path.display(),
            path.display()
        );
        let index = catalog.source(
            &identity,
            &path.to_string_lossy(),
            ExternalHookSourceKind::HooksFile,
            scope,
        )?;
        let Some(bytes) = read(&path)? else {
            catalog.warning(
                index,
                "dsh.hook.config_missing",
                "Referenced Hook configuration is missing",
            );
            continue;
        };
        let mut document = match serde_json::from_slice::<Value>(&bytes) {
            Ok(Value::Object(root)) => {
                if root.get("hooks").is_some_and(Value::is_object) {
                    Value::Object(root)
                } else {
                    serde_json::json!({"hooks": root})
                }
            }
            _ => {
                catalog.warning(
                    index,
                    "dsh.hook.config_invalid",
                    "Hook configuration must be a JSON object",
                );
                continue;
            }
        };
        if let Some(events) = document.get_mut("hooks").and_then(Value::as_object_mut) {
            for groups in events.values_mut().filter_map(Value::as_array_mut) {
                for group in groups {
                    if let Some(handlers) = group.get_mut("hooks").and_then(Value::as_array_mut) {
                        for handler in handlers.iter_mut().filter_map(Value::as_object_mut) {
                            if !handler.get("type").is_some_and(Value::is_string) {
                                handler.insert("type".into(), Value::String("command".into()));
                            }
                        }
                    }
                }
            }
        }
        let normalized = serde_json::to_vec(&document).expect("JSON document");
        let parsed = parse_hook_document(&normalized, StaticHookDocumentFormat::Json, RULES, 2048);
        if !parsed.issues.is_empty() {
            catalog.warning(
                index,
                "dsh.hook.handlers_invalid",
                "Hook configuration contains malformed or unsupported declarations",
            );
        }
        for handler in parsed.handlers {
            let supported_event = if dialect == "claude-code" {
                matches!(
                    handler.native_event.as_str(),
                    "SessionStart"
                        | "UserPromptSubmit"
                        | "PreToolUse"
                        | "PostToolUse"
                        | "Stop"
                        | "SubagentStart"
                        | "SubagentStop"
                )
            } else {
                matches!(
                    handler.native_event.as_str(),
                    "SessionStart" | "UserPromptSubmit" | "PreToolUse" | "PostToolUse" | "Stop"
                )
            };
            let async_codex = dialect == "codex"
                && document
                    .get("hooks")
                    .and_then(|events| events.get(&handler.native_event))
                    .and_then(Value::as_array)
                    .and_then(|groups| groups.get(handler.group_index))
                    .and_then(|group| group.get("hooks"))
                    .and_then(Value::as_array)
                    .and_then(|handlers| handlers.get(handler.handler_index))
                    .and_then(|handler| handler.get("async"))
                    .and_then(Value::as_bool)
                    == Some(true);
            let activation = if disabled {
                ExternalHookNativeActivation::Disabled
            } else if handler.handler_kind != ExternalHookHandlerKind::Command
                || !supported_event
                || async_codex
            {
                ExternalHookNativeActivation::Unsupported
            } else {
                ExternalHookNativeActivation::Unknown
            };
            let matcher = if matches!(handler.native_event.as_str(), "UserPromptSubmit" | "Stop") {
                ExternalHookMatcherSummary::Any
            } else {
                handler.matcher
            };
            catalog.event(
                index,
                &handler.native_event,
                handler.handler_kind,
                matcher,
                ExternalHookProjectionStatus::NativeOnly,
                activation,
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(path: impl AsRef<Path>, text: &str) {
        std::fs::create_dir_all(path.as_ref().parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    #[test]
    fn discovers_profile_bridge_references_bare_maps_and_native_limits() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project = temp.path().join("project");
        write(home.join("profiles/dev/cordis.patch.yml"), "- insert:\n  - name: '@deepseek-ai/dsh-hooks-codex'\n    config:\n      configPath: hooks.json\n");
        write(
            project.join("hooks.json"),
            r#"{"PreToolUse":[{"hooks":[{"command":"SECRET"},{"type":"command","async":true,"command":"async-secret"}]}],"PreCompact":[{"hooks":[{"type":"command","command":"secret"}]}]}"#,
        );
        let provider = DshHookProvider::new(DshHookProviderOptions { dsh_home: home });
        let context = ExternalSourceContext {
            workspace_root: Some(project.clone()),
            execution_domain_id: ExecutionDomainId::new("fixture").unwrap(),
        };
        let snapshot = provider.discover(&context).unwrap();
        assert_eq!(snapshot.entries.len(), 3);
        assert_eq!(snapshot.entries.iter().filter(|entry| entry.native_activation == ExternalHookNativeActivation::Unsupported).count(), 2);
        assert!(snapshot.entries.iter().all(|entry| entry.projection_status
            == ExternalHookProjectionStatus::NativeOnly
            && entry.mapping.is_none()));
        let encoded = serde_json::to_string(&snapshot).unwrap();
        assert!(!encoded.contains("SECRET"));
        write(
            project.join("hooks.json"),
            r#"{"PreToolUse":[{"hooks":[{"command":"DIFFERENT"},{"type":"command","async":true,"command":"changed"}]}],"PreCompact":[{"hooks":[{"type":"command","command":"changed"}]}]}"#,
        );
        assert_eq!(snapshot, provider.discover(&context).unwrap());
    }
    #[test]
    fn disabled_groups_preserve_discovered_handlers_without_activation() {
        let temp = tempfile::tempdir().unwrap();
        write(temp.path().join("cordis.patch.yml"), "- group: true\n  disabled: true\n  config:\n  - name: '@deepseek-ai/dsh-hooks-claude-code'\n    config:\n      configPath: hooks.json\n");
        write(
            temp.path().join("hooks.json"),
            r#"{"hooks":{"SubagentStart":[{"hooks":[{"type":"command","command":"never-run"}]}]}}"#,
        );
        let snapshot = DshHookProvider::new(DshHookProviderOptions {
            dsh_home: temp.path().into(),
        })
        .discover(&ExternalSourceContext {
            workspace_root: Some(temp.path().into()),
            execution_domain_id: ExecutionDomainId::new("fixture").unwrap(),
        })
        .unwrap();
        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(
            snapshot.entries[0].native_activation,
            ExternalHookNativeActivation::Disabled
        );
    }
    #[test]
    fn missing_or_unresolved_references_have_diagnostics() {
        let temp = tempfile::tempdir().unwrap();
        write(
            temp.path().join("cordis.patch.yml"),
            "- name: '@deepseek-ai/dsh-hooks-codex'\n  config:\n    configPath: missing.json\n",
        );
        let provider = DshHookProvider::new(DshHookProviderOptions {
            dsh_home: temp.path().into(),
        });
        let mut context = ExternalSourceContext {
            workspace_root: None,
            execution_domain_id: ExecutionDomainId::new("fixture").unwrap(),
        };
        let snapshot = provider.discover(&context).unwrap();
        assert!(snapshot
            .sources
            .iter()
            .flat_map(|source| &source.diagnostics)
            .any(|diagnostic| diagnostic.code == "dsh.hook.cwd_required"));
        context.workspace_root = Some(temp.path().into());
        let snapshot = provider.discover(&context).unwrap();
        assert!(snapshot
            .sources
            .iter()
            .flat_map(|source| &source.diagnostics)
            .any(|diagnostic| diagnostic.code == "dsh.hook.config_missing"));
    }
}
