use bitfun_product_domains::external_hook_catalog::{
    ExternalHookCatalogEntry, ExternalHookHandlerKind, ExternalHookMapping,
    ExternalHookMatcherSummary, ExternalHookNativeActivation, ExternalHookProjectionStatus,
    ExternalHookProviderIdentity, ExternalHookProviderSnapshot, ExternalHookSource,
    ExternalHookSourceKind, ExternalHookSourceProvider,
};
use bitfun_product_domains::external_hook_contributions::ExternalHookPoint;
use bitfun_product_domains::external_sources::{
    EcosystemId, ExternalSourceAssetKind, ExternalSourceContext, ExternalSourceDiagnostic,
    ExternalSourceHealth, ExternalSourceProviderError, ExternalSourceScope, SourceKey,
};
use bitfun_static_hook_support::{
    bounded_project_ancestors, parse_hook_document, read_bounded_file,
    redacted_parse_content_version, regular_file_exists, BoundedFileRead, StaticHookDocumentFormat,
    StaticHookHandlerRule, StaticHookParseIssue,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const PROVIDER_ID: &str = "claude-code.hooks";
const ECOSYSTEM_ID: &str = "claude-code";
const MAX_SETTINGS_FILE_BYTES: usize = 1024 * 1024;
const MAX_HANDLERS: usize = 2048;
const MAX_PROJECT_ANCESTORS: usize = 32;
const HANDLER_RULES: &[StaticHookHandlerRule] = &[
    StaticHookHandlerRule::new("command", ExternalHookHandlerKind::Command, &["command"]),
    StaticHookHandlerRule::new("http", ExternalHookHandlerKind::Http, &["url"]),
    StaticHookHandlerRule::new(
        "mcp_tool",
        ExternalHookHandlerKind::McpTool,
        &["server", "tool"],
    ),
    StaticHookHandlerRule::new("prompt", ExternalHookHandlerKind::Prompt, &["prompt"]),
    StaticHookHandlerRule::new("agent", ExternalHookHandlerKind::Agent, &["prompt"]),
];

/// Reads Claude Code's static switch without applying settings precedence or
/// attempting to reproduce managed/runtime policy.
fn parse_disable_all_hooks(bytes: &[u8]) -> Result<Option<bool>, ()> {
    let Value::Object(root) = serde_json::from_slice::<Value>(bytes).map_err(|_| ())? else {
        return Err(());
    };
    Ok(root.get("disableAllHooks").and_then(Value::as_bool))
}

#[derive(Debug, Clone)]
pub struct ClaudeCodeHookProviderOptions {
    pub user_settings_file: PathBuf,
    pub project_root_override: Option<PathBuf>,
    pub project_settings_enabled: bool,
}

impl ClaudeCodeHookProviderOptions {
    pub fn from_environment() -> Self {
        let user_settings_file = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".claude/settings.json");
        Self {
            user_settings_file,
            project_root_override: None,
            project_settings_enabled: true,
        }
    }
}

impl Default for ClaudeCodeHookProviderOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

pub struct ClaudeCodeHookProvider {
    options: ClaudeCodeHookProviderOptions,
}

impl ClaudeCodeHookProvider {
    pub fn new(options: ClaudeCodeHookProviderOptions) -> Self {
        Self { options }
    }

    fn project_roots(&self, workspace_root: &Path) -> Vec<PathBuf> {
        let boundary = self
            .options
            .project_root_override
            .as_deref()
            .unwrap_or(workspace_root);
        bounded_project_ancestors(workspace_root, boundary, MAX_PROJECT_ANCESTORS)
    }

    fn layers(&self, context: &ExternalSourceContext) -> Vec<SettingsLayer> {
        let mut layers = vec![SettingsLayer {
            path: self.options.user_settings_file.clone(),
            source_id: "user-settings".to_string(),
            label: "Claude Code user settings".to_string(),
            location_hint: "~/.claude/settings.json".to_string(),
            scope: ExternalSourceScope::UserGlobal,
        }];
        if self.options.project_settings_enabled {
            if let Some(workspace_root) = &context.workspace_root {
                let boundary = self
                    .options
                    .project_root_override
                    .as_deref()
                    .unwrap_or(workspace_root);
                for root in self.project_roots(workspace_root) {
                    let project_root = root.join(".claude");
                    let relative = root.strip_prefix(boundary).unwrap_or(Path::new(""));
                    for (name, local) in [("settings.json", false), ("settings.local.json", true)] {
                        let path = project_root.join(name);
                        let relative_path = relative.join(".claude").join(name);
                        layers.push(SettingsLayer {
                            source_id: format!(
                                "project-{}",
                                short_hash(path.as_os_str().as_encoded_bytes())
                            ),
                            path,
                            label: if local {
                                "Claude Code local project settings".to_string()
                            } else {
                                "Claude Code project settings".to_string()
                            },
                            location_hint: relative_path.to_string_lossy().to_string(),
                            scope: if local {
                                ExternalSourceScope::WorkspaceLocal
                            } else {
                                ExternalSourceScope::Project
                            },
                        });
                    }
                }
            }
        }
        layers
    }
}

impl Default for ClaudeCodeHookProvider {
    fn default() -> Self {
        Self::new(ClaudeCodeHookProviderOptions::default())
    }
}

impl ExternalHookSourceProvider for ClaudeCodeHookProvider {
    fn identity(&self) -> ExternalHookProviderIdentity {
        ExternalHookProviderIdentity::new(PROVIDER_ID, ECOSYSTEM_ID, "Claude Code Hooks")
            .expect("static Claude Code Hook provider identity must be valid")
    }

    fn discover(
        &self,
        context: &ExternalSourceContext,
    ) -> Result<ExternalHookProviderSnapshot, ExternalSourceProviderError> {
        if context
            .workspace_root
            .as_ref()
            .is_some_and(|workspace_root| !workspace_root.is_absolute())
        {
            return Err(ExternalSourceProviderError::new(
                "claude.hook.workspace_invalid",
                "workspace root must be absolute",
                false,
            ));
        }

        let mut loaded_layers = Vec::new();
        let mut activation_reliable = true;
        let mut effective_disabled = None;
        for layer in self.layers(context) {
            match regular_file_exists(&layer.path) {
                Ok(false) => continue,
                Ok(true) => {}
                Err(error) => {
                    return Err(ExternalSourceProviderError::new(
                        "claude.hook.settings_metadata_failed",
                        format!("Claude Code Hook settings metadata is unavailable: {error}"),
                        true,
                    ));
                }
            }
            let bytes = match read_bounded_file(&layer.path, MAX_SETTINGS_FILE_BYTES) {
                Ok(BoundedFileRead::Content(bytes)) => Some(bytes),
                Ok(BoundedFileRead::TooLarge) => {
                    activation_reliable = false;
                    None
                }
                Err(error) => {
                    return Err(ExternalSourceProviderError::new(
                        "claude.hook.settings_unreadable",
                        format!("Claude Code Hook settings could not be read: {error}"),
                        true,
                    ));
                }
            };
            if let Some(bytes) = &bytes {
                match parse_disable_all_hooks(bytes) {
                    Ok(Some(disabled)) => effective_disabled = Some(disabled),
                    Ok(None) => {}
                    Err(()) => activation_reliable = false,
                }
            }
            loaded_layers.push((layer, bytes));
        }
        let native_activation = if activation_reliable && effective_disabled == Some(true) {
            ExternalHookNativeActivation::Disabled
        } else {
            ExternalHookNativeActivation::Unknown
        };

        let mut sources = Vec::new();
        let mut entries = Vec::new();
        let mut diagnostics = Vec::new();
        let mut remaining_handlers = MAX_HANDLERS;
        for (layer, bytes) in loaded_layers {
            inspect_settings_layer(
                &layer,
                bytes.as_deref(),
                native_activation,
                &mut remaining_handlers,
                &mut sources,
                &mut entries,
                &mut diagnostics,
            )?;
        }
        diagnostics
            .sort_by(|left, right| (&left.code, &left.message).cmp(&(&right.code, &right.message)));
        let snapshot = ExternalHookProviderSnapshot {
            provider: self.identity(),
            sources,
            entries,
            diagnostics,
        };
        snapshot.validate().map_err(|error| {
            ExternalSourceProviderError::new(
                "claude.hook.snapshot_invalid",
                error.to_string(),
                false,
            )
        })?;
        Ok(snapshot)
    }
}

struct SettingsLayer {
    path: PathBuf,
    source_id: String,
    label: String,
    location_hint: String,
    scope: ExternalSourceScope,
}

fn inspect_settings_layer(
    layer: &SettingsLayer,
    bytes: Option<&[u8]>,
    native_activation: ExternalHookNativeActivation,
    remaining_handlers: &mut usize,
    sources: &mut Vec<ExternalHookSource>,
    entries: &mut Vec<ExternalHookCatalogEntry>,
    diagnostics: &mut Vec<ExternalSourceDiagnostic>,
) -> Result<(), ExternalSourceProviderError> {
    let source_key = SourceKey::new(PROVIDER_ID, &layer.source_id)
        .expect("static Claude Code Hook source identity must be valid");
    let bytes = match bytes {
        Some(bytes) => bytes,
        None => {
            let diagnostic = hook_warning(
                "claude.hook.settings_too_large",
                "Claude Code settings exceed the 1 MiB static inspection limit",
                Some(source_key.clone()),
            );
            sources.push(source(
                layer,
                source_key,
                ExternalSourceHealth::Unavailable,
                "unavailable:too-large".to_string(),
                vec![diagnostic.clone()],
            ));
            diagnostics.push(diagnostic);
            return Ok(());
        }
    };
    let parsed = parse_hook_document(
        bytes,
        StaticHookDocumentFormat::Json,
        HANDLER_RULES,
        *remaining_handlers,
    );
    let version = redacted_parse_content_version(&parsed);
    let mut source_diagnostics = Vec::new();
    if parsed.all_disabled {
        source_diagnostics.push(hook_warning(
            "claude.hook.all_disabled",
            "Claude Code declares disableAllHooks for this settings layer; entries remain visible for inspection",
            Some(source_key.clone()),
        ));
    }
    *remaining_handlers = remaining_handlers.saturating_sub(parsed.inspected_handlers);
    for issue in parsed.issues {
        source_diagnostics.push(parse_issue_diagnostic(issue, &source_key));
    }
    for handler in parsed.handlers {
        entries.push(entry(
            &source_key,
            &handler.native_event,
            handler.matcher,
            handler.handler_kind,
            handler.group_index,
            handler.handler_index,
            native_activation,
            &version,
        ));
    }
    let health = if source_diagnostics.is_empty() {
        ExternalSourceHealth::Available
    } else {
        ExternalSourceHealth::Degraded
    };
    diagnostics.extend(source_diagnostics.clone());
    sources.push(source(
        layer,
        source_key,
        health,
        version,
        source_diagnostics,
    ));
    Ok(())
}

fn parse_issue_diagnostic(
    issue: StaticHookParseIssue,
    source: &SourceKey,
) -> ExternalSourceDiagnostic {
    let (code, message) = match issue {
        StaticHookParseIssue::DocumentInvalid => (
            "claude.hook.settings_parse_failed",
            "Claude Code settings are not a valid JSON object",
        ),
        StaticHookParseIssue::EventNameInvalid => (
            "claude.hook.event_name_invalid",
            "Claude Code Hook event name is invalid or exceeds the inspection limit",
        ),
        StaticHookParseIssue::EventInvalid => (
            "claude.hook.event_invalid",
            "Claude Code Hook event must contain an array of matcher groups",
        ),
        StaticHookParseIssue::GroupInvalid => (
            "claude.hook.group_invalid",
            "Claude Code Hook matcher group must contain a hooks array",
        ),
        StaticHookParseIssue::HandlerInvalid => (
            "claude.hook.handler_invalid",
            "Claude Code Hook handler is missing a supported type or required field",
        ),
        StaticHookParseIssue::HandlerLimit => (
            "claude.hook.handler_limit",
            "Additional Claude Code Hook handlers were omitted after the 2048 item inspection limit",
        ),
    };
    hook_warning(code, message, Some(source.clone()))
}

#[allow(clippy::too_many_arguments)]
fn entry(
    source: &SourceKey,
    native_event: &str,
    matcher: ExternalHookMatcherSummary,
    handler_kind: ExternalHookHandlerKind,
    group_index: usize,
    handler_index: usize,
    native_activation: ExternalHookNativeActivation,
    source_version: &str,
) -> ExternalHookCatalogEntry {
    let mapping = match native_event {
        "PreToolUse" => Some(ExternalHookMapping {
            hook_point: ExternalHookPoint::ToolBefore,
        }),
        "PostToolUse" => Some(ExternalHookMapping {
            hook_point: ExternalHookPoint::ToolAfter,
        }),
        _ => None,
    };
    ExternalHookCatalogEntry {
        stable_key: format!(
            "claude-hook:{}",
            short_hash(
                format!(
                    "{}:{native_event}:{group_index}:{handler_index}",
                    source.stable_key()
                )
                .as_bytes()
            )
        ),
        source: source.clone(),
        native_event: native_event.to_string(),
        matcher,
        handler_kind,
        projection_status: if mapping.is_some() {
            ExternalHookProjectionStatus::Mapped
        } else {
            ExternalHookProjectionStatus::NativeOnly
        },
        native_activation,
        mapping,
        content_version: content_hash(
            format!(
                "{source_version}:{native_event}:{group_index}:{handler_index}:{}",
                match native_activation {
                    ExternalHookNativeActivation::Disabled => "disabled",
                    ExternalHookNativeActivation::Unsupported => "unsupported",
                    ExternalHookNativeActivation::Unknown => "unknown",
                    _ => "unknown",
                }
            )
            .as_bytes(),
        ),
    }
}

fn source(
    layer: &SettingsLayer,
    key: SourceKey,
    health: ExternalSourceHealth,
    content_version: String,
    diagnostics: Vec<ExternalSourceDiagnostic>,
) -> ExternalHookSource {
    ExternalHookSource {
        key,
        ecosystem_id: EcosystemId::new(ECOSYSTEM_ID)
            .expect("static Claude Code ecosystem id must be valid"),
        display_name: layer.label.to_string(),
        source_kind: ExternalHookSourceKind::Settings,
        scope: layer.scope,
        location_hint: layer.location_hint.to_string(),
        health,
        content_version,
        diagnostics,
    }
}

fn hook_warning(code: &str, message: &str, source: Option<SourceKey>) -> ExternalSourceDiagnostic {
    ExternalSourceDiagnostic::warning(code, message, source)
        .with_asset_kind(ExternalSourceAssetKind::Hook)
}

fn content_hash(value: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(value)))
}

fn short_hash(value: &[u8]) -> String {
    hex::encode(Sha256::digest(value))[..24].to_string()
}
