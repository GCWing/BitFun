use bitfun_product_domains::external_sources::{
    EcosystemId, ExternalSourceAssetKind, ExternalSourceContext, ExternalSourceDiagnostic,
    ExternalSourceHealth, ExternalSourceProviderError, ExternalSourceRecord, ExternalSourceScope,
    ExternalWatchRoot, SourceKey,
};
use bitfun_product_domains::external_subagents::{
    external_subagent_candidate_id, ExternalSubagentBehaviorVersion,
    ExternalSubagentCompatibilityState, ExternalSubagentContributionId,
    ExternalSubagentContributionRole, ExternalSubagentDefinition, ExternalSubagentDiscoveryInput,
    ExternalSubagentLocalId, ExternalSubagentMode, ExternalSubagentModelRequest,
    ExternalSubagentProvenanceRef, ExternalSubagentProviderIdentity,
    ExternalSubagentProviderSnapshot, ExternalSubagentSourceProvider, ExternalSubagentToolRequest,
    ExternalSubagentToolSelector, SecretText,
};
use bitfun_services_core::markdown::FrontMatterMarkdown;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const PROVIDER_ID: &str = "opencode.agents";
const ECOSYSTEM_ID: &str = "opencode";
const MAX_CONFIG_FILE_BYTES: u64 = 1024 * 1024;
const MAX_AGENT_FILE_BYTES: u64 = 256 * 1024;
const MAX_AGENT_FILES: usize = 2048;
const MAX_TOTAL_PROMPT_BYTES: usize = 8 * 1024 * 1024;

const KNOWN_AGENT_FIELDS: &[&str] = &[
    "description",
    "prompt",
    "model",
    "variant",
    "temperature",
    "top_p",
    "tools",
    "disable",
    "mode",
    "hidden",
    "color",
    "steps",
    "maxSteps",
    "permission",
    "options",
];

const NATIVE_AGENT_IDS: &[&str] = &[
    "build",
    "plan",
    "general",
    "explore",
    "compaction",
    "title",
    "summary",
];

#[derive(Debug, Clone)]
pub struct OpenCodeSubagentProviderOptions {
    pub user_config_dir: PathBuf,
    pub legacy_user_config_dir: Option<PathBuf>,
    pub explicit_config_file: Option<PathBuf>,
    pub explicit_config_dir: Option<PathBuf>,
    pub project_config_enabled: bool,
    /// A test/product-host override for workspaces whose project boundary is
    /// already known. Normal environment discovery leaves this unset.
    pub project_root_override: Option<PathBuf>,
}

impl OpenCodeSubagentProviderOptions {
    pub fn from_environment() -> Self {
        let home = dirs::home_dir();
        let user_config_dir = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|home| home.join(".config")))
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("opencode");
        Self {
            user_config_dir,
            legacy_user_config_dir: home.map(|home| home.join(".opencode")),
            explicit_config_file: std::env::var_os("OPENCODE_CONFIG").map(PathBuf::from),
            explicit_config_dir: std::env::var_os("OPENCODE_CONFIG_DIR").map(PathBuf::from),
            project_config_enabled: !environment_truthy("OPENCODE_DISABLE_PROJECT_CONFIG"),
            project_root_override: None,
        }
    }
}

impl Default for OpenCodeSubagentProviderOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

pub struct OpenCodeSubagentProvider {
    options: OpenCodeSubagentProviderOptions,
}

impl OpenCodeSubagentProvider {
    pub fn new(options: OpenCodeSubagentProviderOptions) -> Self {
        Self { options }
    }

    fn project_root(&self, workspace_root: &Path) -> PathBuf {
        self.options
            .project_root_override
            .clone()
            .unwrap_or_else(|| find_project_root(workspace_root))
    }

    fn discover_layers(
        &self,
        context: &ExternalSourceContext,
    ) -> Result<Vec<AgentLayer>, ExternalSourceProviderError> {
        let mut layers = Vec::new();
        push_config_file(
            &mut layers,
            &self.options.user_config_dir.join("config.json"),
            ExternalSourceScope::UserGlobal,
            "OpenCode user configuration",
        );
        push_config_files(
            &mut layers,
            &self.options.user_config_dir,
            ExternalSourceScope::UserGlobal,
            "OpenCode user configuration",
        );
        if let Some(path) = &self.options.explicit_config_file {
            push_config_file(
                &mut layers,
                path,
                ExternalSourceScope::UserGlobal,
                "OpenCode OPENCODE_CONFIG",
            );
        }
        if self.options.project_config_enabled {
            if let Some(workspace_root) = &context.workspace_root {
                let project_root = self.project_root(workspace_root);
                for directory in directories_between(&project_root, workspace_root) {
                    push_config_files(
                        &mut layers,
                        &directory,
                        ExternalSourceScope::Project,
                        "OpenCode project configuration",
                    );
                }
            }
        }
        push_agent_files(
            &mut layers,
            &self.options.user_config_dir,
            ExternalSourceScope::UserGlobal,
            "OpenCode user agents",
        )?;
        if self.options.project_config_enabled {
            if let Some(workspace_root) = &context.workspace_root {
                let project_root = self.project_root(workspace_root);
                for directory in directories_between(&project_root, workspace_root) {
                    let opencode = directory.join(".opencode");
                    push_config_files(
                        &mut layers,
                        &opencode,
                        ExternalSourceScope::Project,
                        "OpenCode project agent configuration",
                    );
                    push_agent_files(
                        &mut layers,
                        &opencode,
                        ExternalSourceScope::Project,
                        "OpenCode project agents",
                    )?;
                }
            }
        }
        if let Some(legacy) = &self.options.legacy_user_config_dir {
            if legacy != &self.options.user_config_dir {
                push_config_files(
                    &mut layers,
                    legacy,
                    ExternalSourceScope::UserGlobal,
                    "OpenCode legacy user configuration",
                );
                push_agent_files(
                    &mut layers,
                    legacy,
                    ExternalSourceScope::UserGlobal,
                    "OpenCode legacy user agents",
                )?;
            }
        }
        if let Some(directory) = &self.options.explicit_config_dir {
            push_config_files(
                &mut layers,
                directory,
                ExternalSourceScope::WorkspaceLocal,
                "OpenCode OPENCODE_CONFIG_DIR",
            );
            push_agent_files(
                &mut layers,
                directory,
                ExternalSourceScope::WorkspaceLocal,
                "OpenCode explicit agents",
            )?;
        }
        Ok(deduplicate_layers_keep_last(layers))
    }
}

impl Default for OpenCodeSubagentProvider {
    fn default() -> Self {
        Self::new(OpenCodeSubagentProviderOptions::default())
    }
}

impl ExternalSubagentSourceProvider for OpenCodeSubagentProvider {
    fn identity(&self) -> ExternalSubagentProviderIdentity {
        ExternalSubagentProviderIdentity::new(PROVIDER_ID, ECOSYSTEM_ID, "OpenCode")
            .expect("static OpenCode subagent provider identity must be valid")
    }

    fn discover(
        &self,
        input: &ExternalSubagentDiscoveryInput,
    ) -> Result<ExternalSubagentProviderSnapshot, ExternalSourceProviderError> {
        if input
            .context
            .workspace_root
            .as_ref()
            .is_some_and(|workspace_root| !workspace_root.is_absolute())
        {
            return Err(ExternalSourceProviderError::new(
                "opencode.agent.workspace_invalid",
                "workspace root must be absolute",
                false,
            ));
        }

        let provider = self.identity();
        let mut sources = Vec::new();
        let mut diagnostics = Vec::new();
        let mut patches = BTreeMap::<String, Vec<AgentPatch>>::new();
        let mut ambient_permission_sources = Vec::new();
        let mut total_prompt_bytes = 0usize;

        for layer in self.discover_layers(&input.context)? {
            let source_key = source_key(&layer);
            let suppressed = input.suppressed_sources.contains(&source_key);
            if suppressed {
                sources.push(ExternalSourceRecord {
                    key: source_key,
                    ecosystem_id: EcosystemId::new(ECOSYSTEM_ID)
                        .expect("static OpenCode ecosystem id must be valid"),
                    display_name: layer.display_name.clone(),
                    source_kind: layer.source_kind().to_string(),
                    scope: layer.scope,
                    location: layer.path.to_string_lossy().to_string(),
                    execution_domain_id: input.context.execution_domain_id.clone(),
                    health: ExternalSourceHealth::Available,
                    content_version: digest([layer.path.to_string_lossy().as_ref()]),
                    diagnostics: Vec::new(),
                });
                continue;
            }
            let parsed = parse_layer(&layer)?;
            total_prompt_bytes = total_prompt_bytes.saturating_add(parsed.prompt_bytes);
            if total_prompt_bytes > MAX_TOTAL_PROMPT_BYTES {
                return Err(ExternalSourceProviderError::new(
                    "opencode.agent.total_prompt_bytes_limit",
                    "OpenCode agent prompts exceed the 8 MiB provider limit",
                    false,
                ));
            }
            let mut record = ExternalSourceRecord {
                key: source_key.clone(),
                ecosystem_id: EcosystemId::new(ECOSYSTEM_ID)
                    .expect("static OpenCode ecosystem id must be valid"),
                display_name: layer.display_name.clone(),
                source_kind: layer.source_kind().to_string(),
                scope: layer.scope,
                location: layer.path.to_string_lossy().to_string(),
                execution_domain_id: input.context.execution_domain_id.clone(),
                health: if parsed.diagnostics.is_empty() {
                    ExternalSourceHealth::Available
                } else {
                    ExternalSourceHealth::Partial
                },
                content_version: parsed.content_version,
                diagnostics: Vec::new(),
            };
            for diagnostic in parsed.diagnostics {
                let diagnostic = ExternalSourceDiagnostic {
                    asset_kind: ExternalSourceAssetKind::Subagent,
                    source: Some(source_key.clone()),
                    ..diagnostic
                };
                record.diagnostics.push(diagnostic.clone());
                diagnostics.push(diagnostic);
            }
            if parsed.ambient_permission {
                ambient_permission_sources.push(source_key.clone());
            }
            for mut patch in parsed.patches {
                patch.source = source_key.clone();
                patches
                    .entry(patch.logical_id.clone())
                    .or_default()
                    .push(patch);
            }
            sources.push(record);
        }

        let mut definitions = Vec::new();
        for (logical_id, contributions) in patches {
            definitions.push(materialize_definition(
                &provider,
                logical_id,
                contributions,
                &ambient_permission_sources,
            )?);
        }
        sources.sort_by(|left, right| left.key.cmp(&right.key));
        definitions.sort_by(|left, right| left.logical_id.cmp(&right.logical_id));
        for diagnostic in &mut diagnostics {
            diagnostic.asset_kind = ExternalSourceAssetKind::Subagent;
        }
        for source in &mut sources {
            for diagnostic in &mut source.diagnostics {
                diagnostic.asset_kind = ExternalSourceAssetKind::Subagent;
            }
        }
        let snapshot = ExternalSubagentProviderSnapshot {
            provider,
            sources,
            definitions,
            diagnostics,
        };
        snapshot.validate().map_err(|error| {
            ExternalSourceProviderError::new(
                "opencode.agent.snapshot_invalid",
                error.to_string(),
                false,
            )
        })?;
        Ok(snapshot)
    }

    fn watch_roots(&self, context: &ExternalSourceContext) -> Vec<ExternalWatchRoot> {
        let mut roots = BTreeMap::new();
        add_directory_watch_roots(&mut roots, &self.options.user_config_dir);
        if let Some(path) = &self.options.legacy_user_config_dir {
            add_directory_watch_roots(&mut roots, path);
        }
        if let Some(path) = &self.options.explicit_config_file {
            if let Some(parent) = path.parent() {
                add_nearest_existing_watch_root(&mut roots, parent);
            }
        }
        if let Some(path) = &self.options.explicit_config_dir {
            add_directory_watch_roots(&mut roots, path);
        }
        if self.options.project_config_enabled {
            if let Some(workspace_root) = &context.workspace_root {
                let project_root = self.project_root(workspace_root);
                for directory in directories_between(&project_root, workspace_root) {
                    add_watch_root(&mut roots, directory.clone(), false);
                    add_directory_watch_roots(&mut roots, &directory.join(".opencode"));
                }
            }
        }
        roots
            .into_iter()
            .map(|(path, recursive)| ExternalWatchRoot { path, recursive })
            .collect()
    }
}

#[derive(Debug, Clone)]
struct AgentLayer {
    path: PathBuf,
    scope: ExternalSourceScope,
    display_name: String,
    kind: AgentLayerKind,
}

impl AgentLayer {
    fn source_kind(&self) -> &'static str {
        match self.kind {
            AgentLayerKind::Config => "opencode_agent_config",
            AgentLayerKind::Markdown { legacy: false, .. } => "opencode_agent_markdown",
            AgentLayerKind::Markdown { legacy: true, .. } => "opencode_legacy_mode_markdown",
        }
    }
}

#[derive(Debug, Clone)]
enum AgentLayerKind {
    Config,
    Markdown { logical_id: String, legacy: bool },
}

#[derive(Debug)]
struct ParsedAgentLayer {
    patches: Vec<AgentPatch>,
    ambient_permission: bool,
    diagnostics: Vec<ExternalSourceDiagnostic>,
    content_version: String,
    prompt_bytes: usize,
}

#[derive(Debug, Clone)]
struct AgentPatch {
    source: SourceKey,
    logical_id: String,
    fields: Map<String, Value>,
    legacy: bool,
}

fn parse_layer(layer: &AgentLayer) -> Result<ParsedAgentLayer, ExternalSourceProviderError> {
    let metadata = fs::metadata(&layer.path).map_err(|error| {
        ExternalSourceProviderError::new(
            "opencode.agent.source_unreadable",
            format!("Failed to inspect OpenCode agent source: {error}"),
            true,
        )
    })?;
    let limit = match layer.kind {
        AgentLayerKind::Config => MAX_CONFIG_FILE_BYTES,
        AgentLayerKind::Markdown { .. } => MAX_AGENT_FILE_BYTES,
    };
    if metadata.len() > limit {
        return Err(ExternalSourceProviderError::new(
            "opencode.agent.source_too_large",
            "OpenCode agent source exceeds the compatibility size limit",
            false,
        ));
    }
    let content = fs::read_to_string(&layer.path).map_err(|error| {
        ExternalSourceProviderError::new(
            "opencode.agent.source_unreadable",
            format!("Failed to read OpenCode agent source: {error}"),
            true,
        )
    })?;
    let content_version = digest([layer.path.to_string_lossy().as_ref(), content.as_str()]);
    match &layer.kind {
        AgentLayerKind::Config => parse_config_layer(&content, content_version),
        AgentLayerKind::Markdown { logical_id, legacy } => {
            parse_markdown_layer(logical_id, *legacy, &content, content_version)
        }
    }
}

fn parse_config_layer(
    content: &str,
    content_version: String,
) -> Result<ParsedAgentLayer, ExternalSourceProviderError> {
    let value = serde_json::from_str::<Value>(&strip_jsonc(content)).map_err(|error| {
        ExternalSourceProviderError::new(
            "opencode.agent.config_invalid",
            format!("Failed to parse OpenCode agent config: {error}"),
            true,
        )
    })?;
    let Some(root) = value.as_object() else {
        return Err(ExternalSourceProviderError::new(
            "opencode.agent.config_invalid",
            "OpenCode configuration root must be an object",
            false,
        ));
    };
    let ambient_permission = root.contains_key("permission");
    let mut patches = Vec::new();
    let mut diagnostics = Vec::new();
    if let Some(agents) = root.get("agent") {
        if let Some(agents) = agents.as_object() {
            for (logical_id, value) in agents {
                patches.push(AgentPatch {
                    source: placeholder_source_key(),
                    logical_id: normalize_logical_id(logical_id),
                    fields: value.as_object().cloned().unwrap_or_else(|| {
                        let mut fields = Map::new();
                        fields.insert("__invalid_definition_type".to_string(), value.clone());
                        fields
                    }),
                    legacy: false,
                });
            }
        } else {
            diagnostics.push(ExternalSourceDiagnostic::error(
                "opencode.agent.map_invalid",
                "OpenCode 'agent' configuration must be an object",
                None,
            ));
        }
    }
    Ok(ParsedAgentLayer {
        prompt_bytes: patches
            .iter()
            .filter_map(|patch| patch.fields.get("prompt")?.as_str())
            .map(str::len)
            .sum(),
        patches,
        ambient_permission,
        diagnostics,
        content_version,
    })
}

fn parse_markdown_layer(
    logical_id: &str,
    legacy: bool,
    content: &str,
    content_version: String,
) -> Result<ParsedAgentLayer, ExternalSourceProviderError> {
    let (mut fields, body) = if content.starts_with("---\n") || content.starts_with("---\r\n") {
        let (metadata, body) = FrontMatterMarkdown::load_str(content).map_err(|error| {
            ExternalSourceProviderError::new(
                "opencode.agent.markdown_invalid",
                format!("Failed to parse OpenCode agent Markdown: {error}"),
                true,
            )
        })?;
        let value = serde_yaml::from_value::<Value>(metadata).map_err(|error| {
            ExternalSourceProviderError::new(
                "opencode.agent.markdown_invalid",
                format!("Failed to normalize OpenCode agent front matter: {error}"),
                false,
            )
        })?;
        let Some(fields) = value.as_object().cloned() else {
            return Err(ExternalSourceProviderError::new(
                "opencode.agent.markdown_invalid",
                "OpenCode agent front matter must be an object",
                false,
            ));
        };
        (fields, body)
    } else {
        (Map::new(), content.to_string())
    };
    fields.insert("prompt".to_string(), Value::String(body.trim().to_string()));
    if legacy {
        fields.insert("mode".to_string(), Value::String("primary".to_string()));
    }
    Ok(ParsedAgentLayer {
        prompt_bytes: body.len(),
        patches: vec![AgentPatch {
            source: placeholder_source_key(),
            logical_id: normalize_logical_id(logical_id),
            fields,
            legacy,
        }],
        ambient_permission: false,
        diagnostics: Vec::new(),
        content_version,
    })
}

fn materialize_definition(
    provider: &ExternalSubagentProviderIdentity,
    logical_id: String,
    contributions: Vec<AgentPatch>,
    ambient_permission_sources: &[SourceKey],
) -> Result<ExternalSubagentDefinition, ExternalSourceProviderError> {
    let local_id = ExternalSubagentLocalId::new(logical_id.clone()).map_err(|error| {
        ExternalSourceProviderError::new("opencode.agent.id_invalid", error.to_string(), false)
    })?;
    let mut effective = Value::Object(Map::new());
    let mut provenance = Vec::new();
    let mut legacy = false;
    for (index, contribution) in contributions.iter().enumerate() {
        deep_merge(&mut effective, Value::Object(contribution.fields.clone()));
        provenance.push(ExternalSubagentProvenanceRef {
            contribution_id: ExternalSubagentContributionId::new(
                contribution.source.clone(),
                local_id.clone(),
            ),
            role: if index == 0 {
                ExternalSubagentContributionRole::Base
            } else {
                ExternalSubagentContributionRole::Overlay
            },
        });
        legacy |= contribution.legacy;
    }
    for source in ambient_permission_sources {
        if !provenance
            .iter()
            .any(|item| &item.contribution_id.source == source)
        {
            provenance.push(ExternalSubagentProvenanceRef {
                contribution_id: ExternalSubagentContributionId::new(
                    source.clone(),
                    local_id.clone(),
                ),
                role: ExternalSubagentContributionRole::Overlay,
            });
        }
    }
    let fields = effective
        .as_object()
        .expect("agent merge remains an object");
    let mut invalid = Vec::new();
    let mut blocked = Vec::new();
    let mut degraded = Vec::new();

    if fields.contains_key("__invalid_definition_type") {
        invalid.push("opencode_agent_definition_type_invalid".to_string());
    }
    if fields.keys().any(|field| {
        field.as_str() != "__invalid_definition_type"
            && !KNOWN_AGENT_FIELDS.contains(&field.as_str())
    }) {
        blocked.push("opencode_unknown_agent_field".to_string());
    }
    if !ambient_permission_sources.is_empty() {
        blocked.push("opencode_ambient_permission_not_imported".to_string());
    }
    if fields.contains_key("permission") {
        blocked.push("opencode_agent_permission_not_imported".to_string());
    }
    if fields
        .get("options")
        .is_some_and(|value| !value.as_object().is_some_and(Map::is_empty))
    {
        blocked.push("opencode_agent_options_not_imported".to_string());
    }
    for field in [
        "variant",
        "temperature",
        "top_p",
        "steps",
        "maxSteps",
        "color",
    ] {
        if fields.contains_key(field) {
            degraded.push(format!("opencode_agent_{field}_not_imported"));
        }
    }

    let prompt = match fields.get("prompt") {
        Some(Value::String(value)) if !value.trim().is_empty() => value.clone(),
        Some(Value::String(_)) | None => {
            blocked.push("opencode_agent_prompt_not_imported".to_string());
            String::new()
        }
        Some(_) => {
            invalid.push("opencode_agent_prompt_type_invalid".to_string());
            String::new()
        }
    };
    if NATIVE_AGENT_IDS
        .iter()
        .any(|native_id| native_id.eq_ignore_ascii_case(&logical_id))
    {
        blocked.push("opencode_native_agent_overlay_not_imported".to_string());
    }
    if legacy {
        blocked.push("opencode_legacy_primary_mode_not_imported".to_string());
    }
    let description = string_field(fields, "description", &mut invalid)
        .unwrap_or_else(|| format!("OpenCode agent {logical_id}"));
    let display_name = logical_id.clone();
    let mode = match string_field(fields, "mode", &mut invalid).as_deref() {
        Some("subagent") => ExternalSubagentMode::Subagent,
        Some("all") | None => {
            degraded.push("opencode_primary_facet_not_imported".to_string());
            ExternalSubagentMode::All
        }
        Some("primary") => {
            blocked.push("opencode_primary_agent_not_imported".to_string());
            ExternalSubagentMode::Primary
        }
        Some(_) => {
            invalid.push("opencode_agent_mode_invalid".to_string());
            ExternalSubagentMode::Subagent
        }
    };
    let disabled = bool_field(fields, "disable", false, &mut invalid);
    let hidden = bool_field(fields, "hidden", false, &mut invalid);
    let requested_model = match fields.get("model") {
        None => ExternalSubagentModelRequest::Default,
        Some(Value::String(model)) if !model.trim().is_empty() => {
            let model = model.trim();
            let (provider_hint, model_name) = model
                .split_once('/')
                .map(|(provider, model_name)| (Some(provider.to_string()), model_name.to_string()))
                .unwrap_or_else(|| (None, model.to_string()));
            ExternalSubagentModelRequest::Exact {
                provider_hint,
                model_name,
            }
        }
        Some(_) => {
            invalid.push("opencode_agent_model_type_invalid".to_string());
            ExternalSubagentModelRequest::Default
        }
    };
    let requested_tools = tool_request(fields, &mut invalid, &mut blocked, &mut degraded);
    let compatibility = if !invalid.is_empty() {
        ExternalSubagentCompatibilityState::Invalid
    } else if !blocked.is_empty() {
        ExternalSubagentCompatibilityState::Blocked
    } else if !degraded.is_empty() {
        ExternalSubagentCompatibilityState::ReadyWithDegradation
    } else {
        ExternalSubagentCompatibilityState::Ready
    };
    let mut diagnostic_codes = invalid;
    diagnostic_codes.extend(blocked);
    diagnostic_codes.extend(degraded);
    diagnostic_codes.sort();
    diagnostic_codes.dedup();

    let behavior_diagnostic_codes = diagnostic_codes
        .iter()
        .filter(|code| code.as_str() != "opencode_agent_color_not_imported")
        .cloned()
        .collect::<Vec<_>>();
    let behavior_version = ExternalSubagentBehaviorVersion::new(format!(
        "sha256:{}",
        digest([
            logical_id.as_str(),
            prompt.as_str(),
            mode_label(mode),
            if disabled { "disabled" } else { "enabled" },
            if hidden { "hidden" } else { "visible" },
            &serde_json::to_string(&requested_model).expect("model request serializes"),
            &serde_json::to_string(&requested_tools).expect("tool request serializes"),
            &provenance
                .iter()
                .map(|item| item.contribution_id.stable_key())
                .collect::<Vec<_>>()
                .join("|"),
            &behavior_diagnostic_codes.join("|"),
        ])
    ))
    .expect("hashed behavior version must be valid");
    let candidate_id =
        external_subagent_candidate_id(&provider.provider_id, &logical_id, &provenance);
    let definition = ExternalSubagentDefinition {
        candidate_id,
        logical_id,
        provenance,
        display_name,
        description,
        prompt: SecretText::new(prompt),
        mode,
        disabled,
        hidden,
        requested_model,
        requested_tools,
        compatibility,
        diagnostic_codes,
        behavior_version,
    };
    definition.validate().map_err(|error| {
        ExternalSourceProviderError::new(
            "opencode.agent.definition_invalid",
            error.to_string(),
            false,
        )
    })?;
    Ok(definition)
}

fn string_field(
    fields: &Map<String, Value>,
    key: &str,
    invalid: &mut Vec<String>,
) -> Option<String> {
    match fields.get(key) {
        None => None,
        Some(Value::String(value)) => Some(value.clone()),
        Some(_) => {
            invalid.push(format!("opencode_agent_{key}_type_invalid"));
            None
        }
    }
}

fn bool_field(
    fields: &Map<String, Value>,
    key: &str,
    default: bool,
    invalid: &mut Vec<String>,
) -> bool {
    match fields.get(key) {
        None => default,
        Some(Value::Bool(value)) => *value,
        Some(_) => {
            invalid.push(format!("opencode_agent_{key}_type_invalid"));
            default
        }
    }
}

fn tool_request(
    fields: &Map<String, Value>,
    invalid: &mut Vec<String>,
    blocked: &mut Vec<String>,
    degraded: &mut Vec<String>,
) -> ExternalSubagentToolRequest {
    let Some(value) = fields.get("tools") else {
        if !fields.contains_key("permission") {
            degraded.push("opencode_default_permission_semantics_not_imported".to_string());
        }
        return ExternalSubagentToolRequest {
            selectors: [
                ("list", "LS"),
                ("read", "Read"),
                ("glob", "Glob"),
                ("grep", "Grep"),
            ]
            .into_iter()
            .map(|(source_name, canonical)| ExternalSubagentToolSelector {
                source_name: source_name.to_string(),
                canonical_host_name: Some(canonical.to_string()),
                allowed: true,
            })
            .collect(),
            uses_conservative_default: true,
        };
    };
    let Some(entries) = value.as_object() else {
        invalid.push("opencode_agent_tools_type_invalid".to_string());
        return ExternalSubagentToolRequest {
            selectors: Vec::new(),
            uses_conservative_default: false,
        };
    };
    let mut selectors = Vec::new();
    for (name, allowed) in entries {
        let Some(allowed) = allowed.as_bool() else {
            invalid.push("opencode_agent_tool_selector_type_invalid".to_string());
            continue;
        };
        if name
            .bytes()
            .any(|byte| matches!(byte, b'*' | b'?' | b'[' | b']'))
        {
            if allowed {
                blocked.push("opencode_agent_tool_pattern_not_imported".to_string());
            }
            continue;
        }
        let canonical = match name.to_ascii_lowercase().as_str() {
            "list" => Some("LS"),
            "read" => Some("Read"),
            "glob" => Some("Glob"),
            "grep" => Some("Grep"),
            _ => None,
        };
        selectors.push(ExternalSubagentToolSelector {
            source_name: name.clone(),
            canonical_host_name: canonical.map(str::to_string),
            allowed,
        });
    }
    ExternalSubagentToolRequest {
        selectors,
        uses_conservative_default: false,
    }
}

fn deep_merge(target: &mut Value, incoming: Value) {
    match (target, incoming) {
        (Value::Object(target), Value::Object(incoming)) => {
            for (key, value) in incoming {
                match target.get_mut(&key) {
                    Some(existing) => deep_merge(existing, value),
                    None => {
                        target.insert(key, value);
                    }
                }
            }
        }
        (target, incoming) => *target = incoming,
    }
}

fn mode_label(mode: ExternalSubagentMode) -> &'static str {
    match mode {
        ExternalSubagentMode::Subagent => "subagent",
        ExternalSubagentMode::All => "all",
        ExternalSubagentMode::Primary => "primary",
    }
}

fn push_config_files(
    layers: &mut Vec<AgentLayer>,
    directory: &Path,
    scope: ExternalSourceScope,
    display_name: &str,
) {
    for name in ["opencode.json", "opencode.jsonc"] {
        push_config_file(layers, &directory.join(name), scope, display_name);
    }
}

fn push_config_file(
    layers: &mut Vec<AgentLayer>,
    path: &Path,
    scope: ExternalSourceScope,
    display_name: &str,
) {
    if path.is_file() {
        layers.push(AgentLayer {
            path: path.to_path_buf(),
            scope,
            display_name: display_name.to_string(),
            kind: AgentLayerKind::Config,
        });
    }
}

fn push_agent_files(
    layers: &mut Vec<AgentLayer>,
    directory: &Path,
    scope: ExternalSourceScope,
    display_name: &str,
) -> Result<(), ExternalSourceProviderError> {
    let mut files = Vec::new();
    for (name, legacy) in [
        ("agent", false),
        ("agents", false),
        ("mode", true),
        ("modes", true),
    ] {
        let root = directory.join(name);
        collect_markdown_files(&root, &mut files)?;
        for path in files.drain(..) {
            let logical_id = markdown_logical_id(&root, &path).ok_or_else(|| {
                ExternalSourceProviderError::new(
                    "opencode.agent.markdown_id_invalid",
                    "OpenCode agent Markdown path cannot form an identifier",
                    false,
                )
            })?;
            layers.push(AgentLayer {
                path,
                scope,
                display_name: display_name.to_string(),
                kind: AgentLayerKind::Markdown { logical_id, legacy },
            });
        }
    }
    Ok(())
}

fn collect_markdown_files(
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), ExternalSourceProviderError> {
    let metadata = match fs::symlink_metadata(directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(ExternalSourceProviderError::new(
                "opencode.agent.directory_unreadable",
                format!("Failed to inspect OpenCode agent directory: {error}"),
                true,
            ));
        }
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Ok(());
    }
    let mut entries = fs::read_dir(directory)
        .map_err(|error| {
            ExternalSourceProviderError::new(
                "opencode.agent.directory_unreadable",
                format!("Failed to enumerate OpenCode agent directory: {error}"),
                true,
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            ExternalSourceProviderError::new(
                "opencode.agent.directory_unreadable",
                format!("Failed to read OpenCode agent directory entry: {error}"),
                true,
            )
        })?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        if files.len() >= MAX_AGENT_FILES {
            return Err(ExternalSourceProviderError::new(
                "opencode.agent.file_limit",
                format!("OpenCode agent directories exceed the {MAX_AGENT_FILES} file limit"),
                false,
            ));
        }
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            ExternalSourceProviderError::new(
                "opencode.agent.directory_unreadable",
                format!("Failed to inspect OpenCode agent directory entry: {error}"),
                true,
            )
        })?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_markdown_files(&path, files)?;
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        {
            files.push(path);
        }
    }
    Ok(())
}

fn markdown_logical_id(root: &Path, path: &Path) -> Option<String> {
    let mut name = path
        .strip_prefix(root)
        .ok()?
        .to_string_lossy()
        .replace('\\', "/");
    if name.to_ascii_lowercase().ends_with(".md") {
        name.truncate(name.len() - 3);
    }
    (!name.is_empty()).then(|| normalize_logical_id(&name))
}

fn normalize_logical_id(value: &str) -> String {
    value.trim().replace('\\', "/")
}

fn source_key(layer: &AgentLayer) -> SourceKey {
    let identity_path =
        dunce::canonicalize(&layer.path).unwrap_or_else(|_| normalize_path_lexically(&layer.path));
    let source_id = format!(
        "{}-{}",
        layer.source_kind(),
        &digest([
            layer.source_kind(),
            identity_path.to_string_lossy().as_ref()
        ])[..24]
    );
    SourceKey::new(PROVIDER_ID, source_id).expect("hashed OpenCode agent source id must be valid")
}

fn placeholder_source_key() -> SourceKey {
    SourceKey::new(PROVIDER_ID, "pending-source").expect("static placeholder source key")
}

fn deduplicate_layers_keep_last(layers: Vec<AgentLayer>) -> Vec<AgentLayer> {
    let mut seen = BTreeSet::new();
    let mut unique = layers
        .into_iter()
        .rev()
        .filter(|layer| seen.insert(source_key(layer)))
        .collect::<Vec<_>>();
    unique.reverse();
    unique
}

fn normalize_path_lexically(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn strip_jsonc(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let chars = input.chars().collect::<Vec<_>>();
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;
    while index < chars.len() {
        let current = chars[index];
        if in_string {
            output.push(current);
            if escaped {
                escaped = false;
            } else if current == '\\' {
                escaped = true;
            } else if current == '"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if current == '"' {
            in_string = true;
            output.push(current);
            index += 1;
        } else if current == '/' && chars.get(index + 1) == Some(&'/') {
            index += 2;
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
        } else if current == '/' && chars.get(index + 1) == Some(&'*') {
            index += 2;
            while index + 1 < chars.len() && !(chars[index] == '*' && chars[index + 1] == '/') {
                index += 1;
            }
            index = (index + 2).min(chars.len());
        } else {
            output.push(current);
            index += 1;
        }
    }
    output
}

fn digest(parts: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        let value = part.as_ref();
        hasher.update(value.len().to_le_bytes());
        hasher.update(value.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn environment_truthy(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .is_some_and(|value| matches!(value.to_ascii_lowercase().as_str(), "true" | "1"))
}

fn find_project_root(start: &Path) -> PathBuf {
    let start = if start.is_file() {
        start.parent().unwrap_or(start)
    } else {
        start
    };
    start
        .ancestors()
        .find(|path| path.join(".git").exists())
        .unwrap_or(start)
        .to_path_buf()
}

fn directories_between(root: &Path, opened: &Path) -> Vec<PathBuf> {
    let opened = if opened.is_file() {
        opened.parent().unwrap_or(opened)
    } else {
        opened
    };
    let mut directories = opened
        .ancestors()
        .take_while(|path| path.starts_with(root))
        .map(Path::to_path_buf)
        .collect::<Vec<_>>();
    directories.reverse();
    directories
}

fn nearest_existing_path(mut path: PathBuf) -> Option<PathBuf> {
    loop {
        if path.exists() {
            return Some(path);
        }
        if !path.pop() {
            return None;
        }
    }
}

fn add_watch_root(roots: &mut BTreeMap<PathBuf, bool>, path: PathBuf, recursive: bool) {
    roots
        .entry(path)
        .and_modify(|existing| *existing |= recursive)
        .or_insert(recursive);
}

fn add_nearest_existing_watch_root(roots: &mut BTreeMap<PathBuf, bool>, path: &Path) {
    if let Some(path) = nearest_existing_path(path.to_path_buf()) {
        add_watch_root(roots, path, false);
    }
}

fn add_directory_watch_roots(roots: &mut BTreeMap<PathBuf, bool>, directory: &Path) {
    if directory.exists() {
        add_watch_root(roots, directory.to_path_buf(), true);
    } else if let Some(parent) = directory.parent() {
        add_nearest_existing_watch_root(roots, parent);
    }
}
