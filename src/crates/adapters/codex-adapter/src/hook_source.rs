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
    StaticHookHandlerFact, StaticHookHandlerRule, StaticHookParseIssue, StaticHookParseResult,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const PROVIDER_ID: &str = "codex.hooks";
const ECOSYSTEM_ID: &str = "codex";
const MAX_CONFIG_FILE_BYTES: usize = 1024 * 1024;
const MAX_HANDLERS: usize = 2048;
const MAX_PROJECT_ANCESTORS: usize = 32;
const HANDLER_RULES: &[StaticHookHandlerRule] = &[
    StaticHookHandlerRule::new("command", ExternalHookHandlerKind::Command, &["command"]),
    StaticHookHandlerRule::new("prompt", ExternalHookHandlerKind::Prompt, &[]),
    StaticHookHandlerRule::new("agent", ExternalHookHandlerKind::Agent, &[]),
];
const CODEX_HOOK_EVENTS: &[&str] = &[
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "PreCompact",
    "PostCompact",
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "SubagentStart",
    "SubagentStop",
    "Stop",
];

#[derive(Debug, Clone)]
pub struct CodexHookProviderOptions {
    pub codex_home: PathBuf,
    /// Current checkout boundary used to bound project ancestor discovery.
    pub project_root_override: Option<PathBuf>,
    /// Primary-checkout root used for linked-worktree Hook declarations.
    pub project_hooks_root_override: Option<PathBuf>,
    pub project_hooks_enabled: bool,
}

impl CodexHookProviderOptions {
    pub fn from_environment() -> Self {
        let codex_home = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".codex")))
            .unwrap_or_else(|| PathBuf::from(".codex"));
        Self {
            codex_home,
            project_root_override: None,
            project_hooks_root_override: None,
            project_hooks_enabled: true,
        }
    }
}

impl Default for CodexHookProviderOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

pub struct CodexHookProvider {
    options: CodexHookProviderOptions,
}

impl CodexHookProvider {
    pub fn new(options: CodexHookProviderOptions) -> Self {
        Self { options }
    }

    fn project_roots(
        &self,
        workspace_root: &Path,
        project_root_markers: &[String],
    ) -> Vec<PathBuf> {
        let boundary = self
            .options
            .project_root_override
            .as_deref()
            .unwrap_or(workspace_root);
        let project_root = if project_root_markers.is_empty() {
            workspace_root.to_path_buf()
        } else {
            bounded_project_ancestors(workspace_root, boundary, MAX_PROJECT_ANCESTORS)
                .into_iter()
                .rev()
                .find(|ancestor| {
                    project_root_markers
                        .iter()
                        .any(|marker| std::fs::metadata(ancestor.join(marker)).is_ok())
                })
                .unwrap_or_else(|| workspace_root.to_path_buf())
        };
        bounded_project_ancestors(workspace_root, &project_root, MAX_PROJECT_ANCESTORS)
    }

    fn layers(
        &self,
        context: &ExternalSourceContext,
        project_root_markers: &[String],
    ) -> Vec<ConfigLayer> {
        let mut layers = vec![
            ConfigLayer::json(
                self.options.codex_home.join("hooks.json"),
                "user-hooks-json".to_string(),
                "Codex user hooks".to_string(),
                "~/.codex/hooks.json".to_string(),
                ExternalSourceScope::UserGlobal,
            ),
            ConfigLayer::toml(
                self.options.codex_home.join("config.toml"),
                "user-config-toml".to_string(),
                "Codex user configuration".to_string(),
                "~/.codex/config.toml".to_string(),
                ExternalSourceScope::UserGlobal,
            ),
        ];
        if self.options.project_hooks_enabled {
            if let Some(workspace_root) = &context.workspace_root {
                let boundary = self
                    .options
                    .project_root_override
                    .as_deref()
                    .unwrap_or(workspace_root);
                for root in self.project_roots(workspace_root, project_root_markers) {
                    let relative = root.strip_prefix(boundary).unwrap_or(Path::new(""));
                    let config_dir = root.join(".codex");
                    let hooks_dir = self
                        .options
                        .project_hooks_root_override
                        .as_ref()
                        .map(|main| main.join(relative).join(".codex"))
                        .unwrap_or_else(|| config_dir.clone());
                    let hooks_path = hooks_dir.join("hooks.json");
                    // Codex preserves ordinary linked-worktree configuration,
                    // but replaces the complete project `[hooks]` table with
                    // the matching primary-checkout layer. Both Hook source
                    // representations therefore share the same Hook root.
                    let config_path = hooks_dir.join("config.toml");
                    layers.push(ConfigLayer::json(
                        hooks_path.clone(),
                        format!(
                            "project-hooks-json-{}",
                            short_hash(hooks_path.as_os_str().as_encoded_bytes())
                        ),
                        "Codex project hooks".to_string(),
                        relative
                            .join(".codex/hooks.json")
                            .to_string_lossy()
                            .to_string(),
                        ExternalSourceScope::Project,
                    ));
                    layers.push(ConfigLayer::toml(
                        config_path.clone(),
                        format!(
                            "project-config-toml-{}",
                            short_hash(config_path.as_os_str().as_encoded_bytes())
                        ),
                        "Codex project configuration".to_string(),
                        relative
                            .join(".codex/config.toml")
                            .to_string_lossy()
                            .to_string(),
                        ExternalSourceScope::Project,
                    ));
                }
            }
        }
        layers
    }
}

impl Default for CodexHookProvider {
    fn default() -> Self {
        Self::new(CodexHookProviderOptions::default())
    }
}

impl ExternalHookSourceProvider for CodexHookProvider {
    fn identity(&self) -> ExternalHookProviderIdentity {
        ExternalHookProviderIdentity::new(PROVIDER_ID, ECOSYSTEM_ID, "Codex Hooks")
            .expect("static Codex Hook provider identity must be valid")
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
                "codex.hook.workspace_invalid",
                "workspace root must be absolute",
                false,
            ));
        }
        let user_config_path = self.options.codex_home.join("config.toml");
        let user_config = load_config_file(&user_config_path)?;
        let (project_root_markers, project_root_markers_invalid) = match &user_config {
            Some(LoadedConfigFile::Content(bytes)) => match codex_toml_root(bytes) {
                Some(root) => match codex_project_root_markers(&root) {
                    Ok(Some(markers)) => (markers, false),
                    Ok(None) => (vec![".git".to_string()], false),
                    Err(()) => (Vec::new(), true),
                },
                None => (Vec::new(), false),
            },
            Some(LoadedConfigFile::TooLarge) => (Vec::new(), false),
            None => (vec![".git".to_string()], false),
        };
        let layers = self.layers(context, &project_root_markers);
        let paths = layers
            .iter()
            .map(|layer| layer.path.clone())
            .collect::<BTreeSet<_>>();
        let mut loaded_files = BTreeMap::new();
        if let Some(user_config) = user_config {
            loaded_files.insert(user_config_path.clone(), user_config);
        }
        for path in paths {
            if loaded_files.contains_key(&path) {
                continue;
            }
            if let Some(loaded) = load_config_file(&path)? {
                loaded_files.insert(path, loaded);
            }
        }

        let mut sources = Vec::new();
        let mut entries = Vec::new();
        let mut diagnostics = Vec::new();
        if project_root_markers_invalid {
            diagnostics.push(hook_warning(
                "codex.hook.project_root_markers_invalid",
                "Codex project_root_markers must be an array of strings; project Hook discovery was limited to the workspace",
                None,
            ));
        }
        let mut remaining_handlers = MAX_HANDLERS;
        for layer in layers {
            let bytes = match loaded_files.get(&layer.path) {
                Some(LoadedConfigFile::Content(bytes)) => Some(bytes.as_slice()),
                Some(LoadedConfigFile::TooLarge) => None,
                None => continue,
            };
            inspect_layer(
                &layer,
                bytes,
                &mut remaining_handlers,
                &mut sources,
                &mut entries,
                &mut diagnostics,
            )?;
        }
        if !sources.is_empty() {
            diagnostics.push(hook_info(
                "codex.hook.coverage_static_only",
                "Codex plugin, managed, and session-provided Hook sources are outside this static file catalog",
                None,
            ));
            diagnostics.push(hook_info(
                "codex.hook.activation_not_evaluated",
                "Static inspection does not evaluate Codex Hook trust or runtime activation",
                None,
            ));
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
                "codex.hook.snapshot_invalid",
                error.to_string(),
                false,
            )
        })?;
        Ok(snapshot)
    }
}

#[derive(Clone, Copy)]
enum ConfigFormat {
    Json,
    Toml,
}

enum LoadedConfigFile {
    Content(Vec<u8>),
    TooLarge,
}

fn load_config_file(path: &Path) -> Result<Option<LoadedConfigFile>, ExternalSourceProviderError> {
    match regular_file_exists(path) {
        Ok(false) => Ok(None),
        Ok(true) => match read_bounded_file(path, MAX_CONFIG_FILE_BYTES) {
            Ok(BoundedFileRead::Content(bytes)) => Ok(Some(LoadedConfigFile::Content(bytes))),
            Ok(BoundedFileRead::TooLarge) => Ok(Some(LoadedConfigFile::TooLarge)),
            Err(error) => Err(ExternalSourceProviderError::new(
                "codex.hook.config_unreadable",
                format!("Codex Hook configuration could not be read: {error}"),
                true,
            )),
        },
        Err(error) => Err(ExternalSourceProviderError::new(
            "codex.hook.config_metadata_failed",
            format!("Codex Hook configuration metadata is unavailable: {error}"),
            true,
        )),
    }
}

struct ConfigLayer {
    path: PathBuf,
    source_id: String,
    label: String,
    location_hint: String,
    scope: ExternalSourceScope,
    format: ConfigFormat,
}

impl ConfigLayer {
    fn json(
        path: PathBuf,
        source_id: String,
        label: String,
        location_hint: String,
        scope: ExternalSourceScope,
    ) -> Self {
        Self {
            path,
            source_id,
            label,
            location_hint,
            scope,
            format: ConfigFormat::Json,
        }
    }

    fn toml(
        path: PathBuf,
        source_id: String,
        label: String,
        location_hint: String,
        scope: ExternalSourceScope,
    ) -> Self {
        Self {
            path,
            source_id,
            label,
            location_hint,
            scope,
            format: ConfigFormat::Toml,
        }
    }
}

fn inspect_layer(
    layer: &ConfigLayer,
    bytes: Option<&[u8]>,
    remaining_handlers: &mut usize,
    sources: &mut Vec<ExternalHookSource>,
    entries: &mut Vec<ExternalHookCatalogEntry>,
    diagnostics: &mut Vec<ExternalSourceDiagnostic>,
) -> Result<(), ExternalSourceProviderError> {
    let source_key = SourceKey::new(PROVIDER_ID, &layer.source_id)
        .expect("static Codex Hook source identity must be valid");
    let bytes = match bytes {
        Some(bytes) => bytes,
        None => {
            let diagnostic = hook_warning(
                "codex.hook.config_too_large",
                "Codex Hook configuration exceeds the 1 MiB static inspection limit",
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
    let mut parsed = parse_hook_document(
        bytes,
        match layer.format {
            ConfigFormat::Json => StaticHookDocumentFormat::Json,
            ConfigFormat::Toml => StaticHookDocumentFormat::Toml,
        },
        HANDLER_RULES,
        *remaining_handlers,
    );
    validate_codex_document(bytes, layer.format, &mut parsed);
    let version = codex_redacted_content_version(&parsed);
    let mut source_diagnostics = Vec::new();
    *remaining_handlers = remaining_handlers.saturating_sub(parsed.inspected_handlers);
    for issue in parsed.issues {
        source_diagnostics.push(parse_issue_diagnostic(issue, &source_key));
    }
    for handler in parsed.handlers {
        let native_activation = codex_native_activation(&handler);
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

fn validate_codex_document(bytes: &[u8], format: ConfigFormat, parsed: &mut StaticHookParseResult) {
    // `disableAllHooks` belongs to Claude Code. Codex state and feature facts
    // are skipped because static discovery cannot determine activation.
    parsed.all_disabled = false;
    let root = match format {
        ConfigFormat::Json => serde_json::from_slice::<Value>(bytes).ok(),
        ConfigFormat::Toml => std::str::from_utf8(bytes)
            .ok()
            .and_then(|source| toml::from_str::<toml::Value>(source).ok())
            .and_then(|value| serde_json::to_value(value).ok()),
    };
    let Some(Value::Object(root)) = root else {
        return;
    };
    if matches!(format, ConfigFormat::Json)
        && root
            .keys()
            .any(|key| key != "description" && key != "hooks")
    {
        parsed.handlers.clear();
        parsed.issues.clear();
        parsed.issues.push(StaticHookParseIssue::DocumentInvalid);
        return;
    }
    let Some(Value::Object(events)) = root.get("hooks") else {
        return;
    };
    let has_unknown_event = events
        .keys()
        .any(|event| event != "state" && !CODEX_HOOK_EVENTS.contains(&event.as_str()));
    if has_unknown_event {
        parsed
            .handlers
            .retain(|handler| CODEX_HOOK_EVENTS.contains(&handler.native_event.as_str()));
        if !parsed
            .issues
            .contains(&StaticHookParseIssue::EventNameInvalid)
        {
            parsed.issues.push(StaticHookParseIssue::EventNameInvalid);
        }
    }
}

fn codex_toml_root(bytes: &[u8]) -> Option<toml::Value> {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|source| toml::from_str::<toml::Value>(source).ok())
}

fn codex_project_root_markers(root: &toml::Value) -> Result<Option<Vec<String>>, ()> {
    let Some(value) = root.get("project_root_markers") else {
        return Ok(None);
    };
    let Some(values) = value.as_array() else {
        return Err(());
    };
    values
        .iter()
        .map(|value| value.as_str().map(str::to_string).ok_or(()))
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn codex_native_activation(handler: &StaticHookHandlerFact) -> ExternalHookNativeActivation {
    if matches!(
        handler.handler_kind,
        ExternalHookHandlerKind::Prompt | ExternalHookHandlerKind::Agent
    ) {
        return ExternalHookNativeActivation::Unsupported;
    }
    // User state and feature flags may be overridden by unobserved session
    // flags, while project declarations are trust-gated. Static inspection
    // therefore cannot safely claim effective activation for command Hooks.
    ExternalHookNativeActivation::Unknown
}

fn codex_redacted_content_version(parsed: &StaticHookParseResult) -> String {
    let mut material = redacted_parse_content_version(parsed);
    for handler in &parsed.handlers {
        material.push(':');
        material.push_str(match codex_native_activation(handler) {
            ExternalHookNativeActivation::Disabled => "disabled",
            ExternalHookNativeActivation::Unsupported => "unsupported",
            _ => "unknown",
        });
    }
    content_hash(material.as_bytes())
}

fn parse_issue_diagnostic(
    issue: StaticHookParseIssue,
    source: &SourceKey,
) -> ExternalSourceDiagnostic {
    let (code, message) = match issue {
        StaticHookParseIssue::DocumentInvalid => (
            "codex.hook.config_parse_failed",
            "Codex Hook configuration could not be parsed",
        ),
        StaticHookParseIssue::EventNameInvalid => (
            "codex.hook.event_name_invalid",
            "Codex Hook event name is unsupported, invalid, or exceeds the inspection limit",
        ),
        StaticHookParseIssue::EventInvalid => (
            "codex.hook.event_invalid",
            "Codex Hook event must contain an array of matcher groups",
        ),
        StaticHookParseIssue::GroupInvalid => (
            "codex.hook.group_invalid",
            "Codex Hook matcher group must contain a hooks array",
        ),
        StaticHookParseIssue::HandlerInvalid => (
            "codex.hook.handler_invalid",
            "Codex Hook handler is missing a supported type or required field",
        ),
        StaticHookParseIssue::HandlerLimit => (
            "codex.hook.handler_limit",
            "Additional Codex Hook handlers were omitted after the 2048 item inspection limit",
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
            "codex-hook:{}",
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
            format!("{source_version}:{native_event}:{group_index}:{handler_index}").as_bytes(),
        ),
    }
}

fn source(
    layer: &ConfigLayer,
    key: SourceKey,
    health: ExternalSourceHealth,
    content_version: String,
    diagnostics: Vec<ExternalSourceDiagnostic>,
) -> ExternalHookSource {
    ExternalHookSource {
        key,
        ecosystem_id: EcosystemId::new(ECOSYSTEM_ID)
            .expect("static Codex ecosystem id must be valid"),
        display_name: layer.label.to_string(),
        source_kind: match layer.format {
            ConfigFormat::Json => ExternalHookSourceKind::HooksFile,
            ConfigFormat::Toml => ExternalHookSourceKind::InlineConfiguration,
        },
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

fn hook_info(code: &str, message: &str, source: Option<SourceKey>) -> ExternalSourceDiagnostic {
    ExternalSourceDiagnostic {
        severity: bitfun_product_domains::external_sources::ExternalSourceDiagnosticSeverity::Info,
        asset_kind: ExternalSourceAssetKind::Hook,
        code: code.to_string(),
        message: message.to_string(),
        source,
    }
}

fn content_hash(value: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(value)))
}

fn short_hash(value: &[u8]) -> String {
    hex::encode(Sha256::digest(value))[..24].to_string()
}
