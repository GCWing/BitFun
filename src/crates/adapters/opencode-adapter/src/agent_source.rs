use crate::local_source_paths::{
    local_source_plan, local_source_watch_roots, LocalConfigDirectory, LocalConfigDirectoryKind,
    LocalConfigDocument, LocalConfigDocumentKind, LocalConfigDocumentSource, LocalSourcePlanItem,
    OpenCodeLocalConfigOptions,
};
use openbitfun_product_domains::external_sources::{
    EcosystemId, ExternalSourceAssetKind, ExternalSourceContext, ExternalSourceDiagnostic,
    ExternalSourceHealth, ExternalSourceProviderError, ExternalSourceRecord, ExternalSourceScope,
    ExternalWatchRoot, SourceKey,
};
use openbitfun_product_domains::external_subagents::{
    external_subagent_candidate_id, ExternalSubagentBehaviorVersion,
    ExternalSubagentCompatibilityState, ExternalSubagentContributionId,
    ExternalSubagentContributionRole, ExternalSubagentDefinition, ExternalSubagentDiscoveryInput,
    ExternalSubagentLocalId, ExternalSubagentMode, ExternalSubagentModelProfileRequest,
    ExternalSubagentModelRequest, ExternalSubagentProvenanceRef, ExternalSubagentProviderIdentity,
    ExternalSubagentProviderSnapshot, ExternalSubagentSourceProvider,
    ExternalSubagentToolCapability, ExternalSubagentToolRequest, ExternalSubagentToolSelector,
    SecretText,
};
use openbitfun_product_domains::tool_permissions::{
    wildcard_matches, PermissionConstraintLayer, PermissionEffect,
    PermissionResourceCaseSensitivity, PermissionRule,
};
use openbitfun_services_core::{jsonc::strip_jsonc, markdown::FrontMatterMarkdown};
use openbitfun_static_hook_support::common_external_subagent_tool_capability;
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

const V1_CONFIG_KEYS: &[&str] = &[
    "logLevel",
    "server",
    "command",
    "reference",
    "snapshot",
    "plugin",
    "autoshare",
    "disabled_providers",
    "enabled_providers",
    "small_model",
    "mode",
    "agent",
    "provider",
    "permission",
    "tools",
    "attachment",
    "layout",
];

const KNOWN_AGENT_FIELDS: &[&str] = &[
    "description",
    "prompt",
    "system",
    "model",
    "variant",
    "temperature",
    "top_p",
    "tools",
    "disable",
    "disabled",
    "mode",
    "hidden",
    "color",
    "steps",
    "maxSteps",
    "permission",
    "permissions",
    "request",
    "options",
];

const CURRENT_MARKDOWN_AGENT_FIELDS: &[&str] = &[
    "model",
    "variant",
    "request",
    "system",
    "description",
    "mode",
    "hidden",
    "color",
    "steps",
    "disabled",
    "permissions",
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
    pub config: OpenCodeLocalConfigOptions,
    /// A test/product-host override for workspaces whose project boundary is
    /// already known. Normal environment discovery leaves this unset.
    pub project_root_override: Option<PathBuf>,
}

impl OpenCodeSubagentProviderOptions {
    pub fn from_environment() -> Self {
        Self {
            config: OpenCodeLocalConfigOptions::from_environment(),
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

    fn home_dir(&self) -> Option<&Path> {
        self.options
            .config
            .legacy_user_config_dir
            .as_deref()
            .and_then(Path::parent)
    }

    fn discover_layers(
        &self,
        context: &ExternalSourceContext,
    ) -> Result<Vec<AgentLayer>, ExternalSourceProviderError> {
        let mut layers = Vec::new();
        for item in local_source_plan(
            &self.options.config,
            context.workspace_root.as_deref(),
            self.options.project_root_override.as_deref(),
        ) {
            match item {
                LocalSourcePlanItem::Config(document) => {
                    push_config_document(&mut layers, document)
                }
                LocalSourcePlanItem::Directory(directory) => {
                    push_agent_directory_layer(&mut layers, &directory)?
                }
            }
        }
        Ok(deduplicate_layers_keep_last(layers))
    }
}

fn push_agent_directory_layer(
    layers: &mut Vec<AgentLayer>,
    directory: &LocalConfigDirectory,
) -> Result<(), ExternalSourceProviderError> {
    let agent_name = match directory.kind {
        LocalConfigDirectoryKind::User => "OpenCode user agents",
        LocalConfigDirectoryKind::Project => "OpenCode project agents",
        LocalConfigDirectoryKind::Legacy => "OpenCode legacy user agents",
        LocalConfigDirectoryKind::Explicit => "OpenCode explicit agents",
    };
    push_agent_files(layers, &directory.path, directory.scope, agent_name)
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
                self.home_dir(),
                input.context.workspace_root.as_deref(),
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
        local_source_watch_roots(
            &self.options.config,
            context.workspace_root.as_deref(),
            self.options.project_root_override.as_deref(),
        )
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
        match &self.kind {
            AgentLayerKind::Config(_) => "opencode_agent_config",
            AgentLayerKind::Markdown { legacy: false, .. } => "opencode_agent_markdown",
            AgentLayerKind::Markdown { legacy: true, .. } => "opencode_legacy_mode_markdown",
        }
    }
}

#[derive(Debug, Clone)]
enum AgentLayerKind {
    Config(LocalConfigDocument),
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
    disabled_is_tombstone: bool,
}

fn parse_layer(layer: &AgentLayer) -> Result<ParsedAgentLayer, ExternalSourceProviderError> {
    let (content, identity) = match &layer.kind {
        AgentLayerKind::Config(document) => {
            let content = match document
                .read_bounded(MAX_CONFIG_FILE_BYTES as usize)
                .map_err(|error| {
                    ExternalSourceProviderError::new(
                        "opencode.agent.source_unreadable",
                        format!("Failed to read OpenCode agent source: {error}"),
                        true,
                    )
                })? {
                openbitfun_services_core::bounded_fs::BoundedTextRead::Content(content) => content,
                openbitfun_services_core::bounded_fs::BoundedTextRead::TooLarge => {
                    return Err(ExternalSourceProviderError::new(
                        "opencode.agent.source_too_large",
                        "OpenCode agent source exceeds the compatibility size limit",
                        false,
                    ));
                }
                openbitfun_services_core::bounded_fs::BoundedTextRead::InvalidUtf8 => {
                    return Err(ExternalSourceProviderError::new(
                        "opencode.agent.source_unreadable",
                        "OpenCode agent source must be valid UTF-8",
                        false,
                    ));
                }
            };
            (content, document.identity())
        }
        AgentLayerKind::Markdown { .. } => {
            let metadata = fs::metadata(&layer.path).map_err(|error| {
                ExternalSourceProviderError::new(
                    "opencode.agent.source_unreadable",
                    format!("Failed to inspect OpenCode agent source: {error}"),
                    true,
                )
            })?;
            if metadata.len() > MAX_AGENT_FILE_BYTES {
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
            (content, layer.path.to_string_lossy().into_owned())
        }
    };
    let content_version = digest([identity.as_str(), content.as_str()]);
    match &layer.kind {
        AgentLayerKind::Config(_) => parse_config_layer(&content, content_version),
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
    let legacy_document = root
        .keys()
        .any(|key| V1_CONFIG_KEYS.contains(&key.as_str()));
    let ambient_permission = if legacy_document {
        root.contains_key("permission") || root.contains_key("tools")
    } else {
        root.contains_key("permissions")
    };
    let mut patches = Vec::new();
    let mut diagnostics = Vec::new();
    let collection_names: &[&str] = if legacy_document {
        &["agent", "mode"]
    } else {
        &["agents"]
    };
    for &collection_name in collection_names {
        let Some(agents) = root.get(collection_name) else {
            continue;
        };
        if let Some(agents) = agents.as_object() {
            for (logical_id, value) in agents {
                let mut fields = value.as_object().cloned().unwrap_or_else(|| {
                    let mut fields = Map::new();
                    fields.insert("__invalid_definition_type".to_string(), value.clone());
                    fields
                });
                if legacy_document {
                    migrate_v1_agent_fields(&mut fields);
                    if collection_name == "mode" {
                        fields.insert("mode".to_string(), Value::String("primary".to_string()));
                    }
                }
                patches.push(AgentPatch {
                    source: placeholder_source_key(),
                    logical_id: normalize_logical_id(logical_id),
                    fields,
                    legacy: false,
                    disabled_is_tombstone: !legacy_document,
                });
            }
        } else {
            diagnostics.push(ExternalSourceDiagnostic::error(
                "opencode.agent.map_invalid",
                format!("OpenCode '{collection_name}' configuration must be an object"),
                None,
            ));
        }
    }
    Ok(ParsedAgentLayer {
        prompt_bytes: patches
            .iter()
            .filter_map(|patch| {
                patch
                    .fields
                    .get("system")
                    .or_else(|| patch.fields.get("prompt"))?
                    .as_str()
            })
            .map(str::len)
            .sum(),
        patches,
        ambient_permission,
        diagnostics,
        content_version,
    })
}

fn migrate_v1_agent_fields(fields: &mut Map<String, Value>) {
    if !fields.contains_key("system") {
        if let Some(prompt) = fields.remove("prompt") {
            fields.insert("system".to_string(), prompt);
        }
    }
    if !fields.contains_key("disabled") {
        if let Some(disabled) = fields.remove("disable") {
            fields.insert("disabled".to_string(), disabled);
        }
    }
    if !fields.contains_key("permissions") {
        if let Some(permission) = fields
            .get("permission")
            .and_then(migrate_order_independent_v1_permissions)
        {
            fields.remove("permission");
            fields.insert("permissions".to_string(), permission);
        }
    }
}

fn migrate_order_independent_v1_permissions(value: &Value) -> Option<Value> {
    if let Value::String(effect) = value {
        return permission_effect_is_valid(effect)
            .then(|| Value::Array(vec![current_permission_rule("*", "*", effect)]));
    }
    let entries = value.as_object()?;
    if entries.contains_key("*") && entries.len() > 1 {
        return None;
    }
    let mut rules = Vec::<(String, String)>::new();
    for (source_action, value) in entries {
        if source_action != "*"
            && source_action
                .bytes()
                .any(|byte| matches!(byte, b'*' | b'?' | b'[' | b']'))
        {
            return None;
        }
        let action = canonical_permission_action(source_action)?;
        let effect = value
            .as_str()
            .filter(|effect| permission_effect_is_valid(effect))?;
        if let Some((_, existing_effect)) = rules
            .iter()
            .find(|(existing_action, _)| existing_action == action)
        {
            if existing_effect != effect {
                return None;
            }
            continue;
        }
        rules.push((action.to_string(), effect.to_string()));
    }
    Some(Value::Array(
        rules
            .into_iter()
            .map(|(action, effect)| current_permission_rule(&action, "*", &effect))
            .collect(),
    ))
}

fn permission_effect_is_valid(effect: &str) -> bool {
    matches!(effect, "allow" | "ask" | "deny")
}

fn current_permission_rule(action: &str, resource: &str, effect: &str) -> Value {
    Value::Object(Map::from_iter([
        ("action".to_string(), Value::String(action.to_string())),
        ("resource".to_string(), Value::String(resource.to_string())),
        ("effect".to_string(), Value::String(effect.to_string())),
    ]))
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
    let legacy_schema = fields
        .keys()
        .any(|key| !CURRENT_MARKDOWN_AGENT_FIELDS.contains(&key.as_str()));
    let prompt_field = if legacy_schema { "prompt" } else { "system" };
    fields.insert(
        prompt_field.to_string(),
        Value::String(body.trim().to_string()),
    );
    if legacy_schema {
        migrate_v1_agent_fields(&mut fields);
    }
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
            disabled_is_tombstone: !legacy_schema,
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
    home_dir: Option<&Path>,
    workspace_root: Option<&Path>,
) -> Result<ExternalSubagentDefinition, ExternalSourceProviderError> {
    let local_id = ExternalSubagentLocalId::new(logical_id.clone()).map_err(|error| {
        ExternalSourceProviderError::new("opencode.agent.id_invalid", error.to_string(), false)
    })?;
    let mut effective = Value::Object(Map::new());
    let mut provenance = Vec::new();
    let mut legacy = false;
    let mut removed = false;
    for (index, contribution) in contributions.iter().enumerate() {
        let removes_agent = contribution.disabled_is_tombstone
            && contribution
                .fields
                .get("disabled")
                .is_some_and(|value| value == &Value::Bool(true));
        if removes_agent || removed {
            effective = Value::Object(Map::new());
        }
        deep_merge(&mut effective, Value::Object(contribution.fields.clone()));
        removed = removes_agent;
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
    if fields
        .get("options")
        .is_some_and(|value| !value.as_object().is_some_and(Map::is_empty))
    {
        blocked.push("opencode_agent_options_not_imported".to_string());
    }
    if fields
        .get("request")
        .is_some_and(|value| !value.as_object().is_some_and(Map::is_empty))
    {
        blocked.push("opencode_agent_request_not_imported".to_string());
    }
    for field in ["temperature", "top_p", "steps", "maxSteps", "color"] {
        if fields.contains_key(field) {
            degraded.push(format!("opencode_agent_{field}_not_imported"));
        }
    }

    if fields.contains_key("prompt") && fields.contains_key("system") {
        blocked.push("opencode_agent_prompt_versions_conflict".to_string());
    }
    let prompt = match fields.get("system").or_else(|| fields.get("prompt")) {
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
        Some("all") | None => ExternalSubagentMode::All,
        Some("primary") => ExternalSubagentMode::Primary,
        Some(_) => {
            invalid.push("opencode_agent_mode_invalid".to_string());
            ExternalSubagentMode::Subagent
        }
    };
    if fields.contains_key("disable") && fields.contains_key("disabled") {
        blocked.push("opencode_agent_disabled_versions_conflict".to_string());
    }
    let disabled = if fields.contains_key("disabled") {
        bool_field(fields, "disabled", false, &mut invalid)
    } else {
        bool_field(fields, "disable", false, &mut invalid)
    };
    let hidden = bool_field(fields, "hidden", false, &mut invalid);
    let requested_model = match fields.get("model") {
        None => ExternalSubagentModelRequest::Default,
        Some(Value::String(model)) if !model.trim().is_empty() => {
            let model = model.trim();
            let (provider_hint, model_name) = model
                .split_once('/')
                .map(|(provider, model_name)| (Some(provider.to_string()), model_name.to_string()))
                .unwrap_or_else(|| (None, model.to_string()));
            ExternalSubagentModelRequest::Reference {
                provider_hint,
                model_name,
            }
        }
        Some(_) => {
            invalid.push("opencode_agent_model_type_invalid".to_string());
            ExternalSubagentModelRequest::Default
        }
    };
    let has_explicit_model = matches!(
        &requested_model,
        ExternalSubagentModelRequest::Reference { .. }
    );
    let requested_model_profile = match fields.get("variant") {
        None => None,
        Some(Value::String(_)) if !has_explicit_model => None,
        Some(Value::String(value)) => {
            let profile = ExternalSubagentModelProfileRequest::NamedVariant {
                name: value.clone(),
            };
            if profile.validate().is_ok() {
                Some(profile)
            } else {
                invalid.push("opencode_agent_variant_invalid".to_string());
                None
            }
        }
        Some(_) => {
            invalid.push("opencode_agent_variant_type_invalid".to_string());
            None
        }
    };
    // OpenCode applies an agent variant only when that agent declares a model
    // and the active model matches it. A variant on a default-model agent is
    // inert, so importing it as an active profile would add behavior that the
    // source does not have.
    let requested_tools = tool_request(fields, &mut invalid, &mut blocked, &mut degraded);
    let permission_constraints = permission_constraints(
        fields,
        &requested_tools,
        home_dir,
        workspace_root,
        &mut invalid,
        &mut blocked,
        &mut degraded,
    );
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
            &serde_json::to_string(&requested_model_profile)
                .expect("model profile request serializes"),
            &serde_json::to_string(&requested_tools).expect("tool request serializes"),
            &serde_json::to_string(&permission_constraints)
                .expect("permission constraints serialize"),
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
        requested_model_profile,
        requested_tools,
        permission_constraints,
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
        degraded.push("opencode_default_permission_semantics_not_imported".to_string());
        return ExternalSubagentToolRequest {
            selectors: [
                ("list", ExternalSubagentToolCapability::DirectoryList),
                ("read", ExternalSubagentToolCapability::ReadFile),
                ("glob", ExternalSubagentToolCapability::GlobFiles),
                ("grep", ExternalSubagentToolCapability::SearchText),
            ]
            .into_iter()
            .map(
                |(source_name, canonical_capability)| ExternalSubagentToolSelector {
                    source_name: source_name.to_string(),
                    canonical_capability: Some(canonical_capability),
                    allowed: true,
                },
            )
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
        if name.eq_ignore_ascii_case("task") {
            if allowed {
                blocked.push("opencode_agent_task_tool_not_imported".to_string());
            }
            continue;
        }
        let canonical_capability = common_external_subagent_tool_capability(name);
        selectors.push(ExternalSubagentToolSelector {
            source_name: name.clone(),
            canonical_capability,
            allowed,
        });
    }
    ExternalSubagentToolRequest {
        selectors,
        uses_conservative_default: false,
    }
}

fn permission_constraints(
    fields: &Map<String, Value>,
    requested_tools: &ExternalSubagentToolRequest,
    home_dir: Option<&Path>,
    workspace_root: Option<&Path>,
    invalid: &mut Vec<String>,
    blocked: &mut Vec<String>,
    degraded: &mut Vec<String>,
) -> PermissionConstraintLayer {
    if fields.contains_key("permission") && fields.contains_key("permissions") {
        blocked.push("opencode_agent_permission_versions_conflict".to_string());
        return PermissionConstraintLayer::default();
    }
    if let Some(value) = fields.get("permissions") {
        return current_permission_constraints(
            value,
            requested_tools,
            home_dir,
            workspace_root,
            invalid,
            blocked,
        );
    }
    let Some(value) = fields.get("permission") else {
        return PermissionConstraintLayer::default();
    };
    let mut rules = Vec::new();
    match value {
        Value::String(_) => {
            if let Some(effect) = permission_effect(value, invalid) {
                validate_permission_action_enforcement(
                    "*",
                    effect,
                    requested_tools,
                    blocked,
                    degraded,
                );
                rules.push(PermissionRule::new("*", "*", effect));
            }
        }
        Value::Object(entries) => {
            for (source_action, value) in entries {
                if source_action
                    .bytes()
                    .any(|byte| matches!(byte, b'*' | b'?' | b'[' | b']'))
                {
                    blocked.push(
                        "opencode_agent_legacy_permission_action_pattern_not_imported".to_string(),
                    );
                    continue;
                }
                let Some(effect) = permission_effect(value, invalid) else {
                    if value.is_object() {
                        blocked.push(
                            "opencode_agent_permission_resource_patterns_not_imported".to_string(),
                        );
                    }
                    continue;
                };
                let Some(action) = canonical_permission_action(source_action) else {
                    validate_permission_action_enforcement(
                        source_action,
                        effect,
                        requested_tools,
                        blocked,
                        degraded,
                    );
                    continue;
                };
                validate_permission_action_enforcement(
                    source_action,
                    effect,
                    requested_tools,
                    blocked,
                    degraded,
                );
                let rule = PermissionRule::new(action, "*", effect);
                if rules.iter().any(|existing| {
                    existing.action == rule.action && existing.effect != rule.effect
                }) {
                    blocked.push("opencode_agent_permission_alias_effect_conflict".to_string());
                    continue;
                }
                if !rules.contains(&rule) {
                    rules.push(rule);
                }
            }
        }
        _ => invalid.push("opencode_agent_permission_type_invalid".to_string()),
    }
    PermissionConstraintLayer::new(rules)
}

fn current_permission_constraints(
    value: &Value,
    requested_tools: &ExternalSubagentToolRequest,
    home_dir: Option<&Path>,
    workspace_root: Option<&Path>,
    invalid: &mut Vec<String>,
    blocked: &mut Vec<String>,
) -> PermissionConstraintLayer {
    let Some(entries) = value.as_array() else {
        invalid.push("opencode_agent_permissions_type_invalid".to_string());
        return PermissionConstraintLayer::default();
    };
    let mut rules = Vec::new();
    for entry in entries {
        let Some(rule) = entry.as_object() else {
            invalid.push("opencode_agent_permission_rule_type_invalid".to_string());
            continue;
        };
        if rule
            .keys()
            .any(|key| !matches!(key.as_str(), "action" | "resource" | "effect"))
        {
            blocked.push("opencode_agent_permission_rule_field_not_imported".to_string());
            continue;
        }
        let Some(source_action) = required_permission_rule_string(rule, "action", invalid) else {
            continue;
        };
        let Some(resource) = required_permission_rule_string(rule, "resource", invalid) else {
            continue;
        };
        let Some(effect_value) = rule.get("effect") else {
            invalid.push("opencode_agent_permission_rule_effect_missing".to_string());
            continue;
        };
        let Some(effect) = permission_effect(effect_value, invalid) else {
            continue;
        };
        let action = imported_permission_action(source_action);
        let resource = match translate_current_permission_resource(
            source_action,
            &action,
            resource,
            requested_tools,
            home_dir,
            workspace_root,
        ) {
            Ok(resource) => resource,
            Err(code) => {
                blocked.push(code.to_string());
                resource.to_string()
            }
        };
        validate_current_permission_action_enforcement(
            source_action,
            &action,
            effect,
            requested_tools,
            blocked,
        );
        rules.push(PermissionRule::new(action, resource, effect));
    }
    PermissionConstraintLayer::new(rules)
}

fn translate_current_permission_resource(
    source_action: &str,
    action: &str,
    resource: &str,
    requested_tools: &ExternalSubagentToolRequest,
    home_dir: Option<&Path>,
    workspace_root: Option<&Path>,
) -> Result<String, &'static str> {
    let path_resource =
        permission_action_uses_workspace_paths(source_action, action, requested_tools);
    if !path_resource && source_action != "external_directory" {
        return Ok(resource.to_string());
    }

    let translated = if let Some(home_suffix) = permission_home_suffix(resource) {
        let Some(home_dir) = home_dir else {
            return Err("opencode_agent_permission_home_unavailable");
        };
        let home_dir = canonical_permission_root(home_dir);
        format!("{home_dir}{home_suffix}")
    } else {
        resource.to_string()
    };

    if !path_resource
        || translated.chars().all(|character| character == '*')
        || Path::new(&translated).is_absolute()
    {
        return Ok(translated);
    }
    if translated.starts_with('*') || translated.starts_with('?') {
        return Err("opencode_agent_permission_resource_domain_ambiguous");
    }
    let Some(workspace_root) = workspace_root else {
        return Err("opencode_agent_permission_workspace_unavailable");
    };
    let workspace_root = canonical_permission_root(workspace_root);
    if translated == "." {
        return Ok(workspace_root);
    }
    Ok(format!(
        "{}/{}",
        workspace_root.trim_end_matches('/'),
        translated.replace('\\', "/")
    ))
}

fn permission_action_uses_workspace_paths(
    source_action: &str,
    action: &str,
    requested_tools: &ExternalSubagentToolRequest,
) -> bool {
    if matches!(action, "read" | "edit") {
        return true;
    }
    let matched_actions = requested_tools
        .selectors
        .iter()
        .filter(|selector| selector.allowed)
        .filter_map(|selector| permission_action_for_tool_capability(selector.canonical_capability))
        .filter(|host_action| {
            wildcard_matches(
                host_action,
                source_action,
                PermissionResourceCaseSensitivity::Sensitive,
            )
        })
        .collect::<BTreeSet<_>>();
    !matched_actions.is_empty()
        && matched_actions
            .iter()
            .all(|action| matches!(*action, "read" | "edit"))
}

fn permission_home_suffix(resource: &str) -> Option<&str> {
    if resource == "~" || resource == "$HOME" {
        Some("")
    } else if resource.starts_with("~/") {
        Some(&resource[1..])
    } else if resource.starts_with("$HOME/") || resource.starts_with("$HOME\\") {
        Some(&resource[5..])
    } else {
        None
    }
}

fn canonical_permission_root(path: &Path) -> String {
    dunce::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
}

fn validate_current_permission_action_enforcement(
    source_action: &str,
    rule_action: &str,
    effect: PermissionEffect,
    requested_tools: &ExternalSubagentToolRequest,
    blocked: &mut Vec<String>,
) {
    if effect == PermissionEffect::Allow {
        return;
    }
    let reaches_unenforced_tool = requested_tools.selectors.iter().any(|selector| {
        if !selector.allowed {
            return false;
        }
        let host_action = permission_action_for_tool_capability(selector.canonical_capability);
        let rule_reaches_tool = wildcard_matches(
            &selector.source_name,
            source_action,
            PermissionResourceCaseSensitivity::Sensitive,
        ) || host_action.is_some_and(|action| {
            wildcard_matches(
                action,
                rule_action,
                PermissionResourceCaseSensitivity::Sensitive,
            )
        });
        rule_reaches_tool
            && match host_action {
                Some(action) => !wildcard_matches(
                    action,
                    rule_action,
                    PermissionResourceCaseSensitivity::Sensitive,
                ),
                None => true,
            }
    });
    if reaches_unenforced_tool {
        blocked.push("opencode_agent_permission_action_not_enforceable".to_string());
    }
}

fn validate_permission_action_enforcement(
    source_action: &str,
    effect: PermissionEffect,
    requested_tools: &ExternalSubagentToolRequest,
    blocked: &mut Vec<String>,
    degraded: &mut Vec<String>,
) {
    let canonical_action = canonical_permission_action(source_action);
    let selected_unknown_tool = canonical_action.is_none()
        && requested_tools
            .selectors
            .iter()
            .any(|selector| selector.allowed && selector.canonical_capability.is_none());
    let selected_named_tool_is_unenforced = source_action != "*"
        && requested_tools.selectors.iter().any(|selector| {
            let host_action = permission_action_for_tool_capability(selector.canonical_capability);
            let source_action_matches = wildcard_matches(
                &selector.source_name,
                source_action,
                PermissionResourceCaseSensitivity::Sensitive,
            ) || host_action.is_some_and(|action| {
                wildcard_matches(
                    action,
                    source_action,
                    PermissionResourceCaseSensitivity::Sensitive,
                )
            });
            selector.allowed
                && source_action_matches
                && (canonical_action.is_none() || host_action != canonical_action)
        });
    let wildcard_reaches_unenforced_tool = source_action == "*"
        && requested_tools.selectors.iter().any(|selector| {
            selector.allowed
                && permission_action_for_tool_capability(selector.canonical_capability).is_none()
        });

    if effect != PermissionEffect::Allow
        && (selected_unknown_tool
            || selected_named_tool_is_unenforced
            || wildcard_reaches_unenforced_tool)
    {
        blocked.push("opencode_agent_permission_action_not_enforceable".to_string());
    } else if canonical_action.is_none() {
        degraded.push("opencode_agent_permission_action_not_imported".to_string());
    }
}

fn permission_action_for_tool_capability(
    capability: Option<ExternalSubagentToolCapability>,
) -> Option<&'static str> {
    match capability {
        Some(ExternalSubagentToolCapability::ReadFile) => Some("read"),
        Some(
            ExternalSubagentToolCapability::WriteFile | ExternalSubagentToolCapability::EditFile,
        ) => Some("edit"),
        Some(ExternalSubagentToolCapability::ExecuteCommand) => Some("bash"),
        _ => None,
    }
}

fn required_permission_rule_string<'a>(
    rule: &'a Map<String, Value>,
    field: &str,
    invalid: &mut Vec<String>,
) -> Option<&'a str> {
    match rule.get(field) {
        Some(Value::String(value)) if !value.trim().is_empty() && value.trim() == value => {
            Some(value)
        }
        Some(Value::String(_)) => {
            invalid.push(format!("opencode_agent_permission_rule_{field}_invalid"));
            None
        }
        Some(_) => {
            invalid.push(format!(
                "opencode_agent_permission_rule_{field}_type_invalid"
            ));
            None
        }
        None => {
            invalid.push(format!("opencode_agent_permission_rule_{field}_missing"));
            None
        }
    }
}

fn permission_effect(value: &Value, invalid: &mut Vec<String>) -> Option<PermissionEffect> {
    match value.as_str() {
        Some("allow") => Some(PermissionEffect::Allow),
        Some("ask") => Some(PermissionEffect::Ask),
        Some("deny") => Some(PermissionEffect::Deny),
        Some(_) => {
            invalid.push("opencode_agent_permission_effect_invalid".to_string());
            None
        }
        None if value.is_object() => None,
        None => {
            invalid.push("opencode_agent_permission_effect_type_invalid".to_string());
            None
        }
    }
}

fn canonical_permission_action(source_action: &str) -> Option<&'static str> {
    let normalized;
    let source_action = if cfg!(windows) {
        normalized = source_action.to_ascii_lowercase();
        normalized.as_str()
    } else {
        source_action
    };
    match source_action {
        "*" => Some("*"),
        "write" | "edit" | "patch" | "apply_patch" => Some("edit"),
        "read" => Some("read"),
        "bash" => Some("bash"),
        "task" => Some("task"),
        "skill" => Some("skill"),
        "webfetch" => Some("webfetch"),
        "websearch" => Some("websearch"),
        "git" => Some("git"),
        "external_directory" => Some("external_directory"),
        _ => None,
    }
}

fn imported_permission_action(source_action: &str) -> String {
    canonical_permission_action(source_action)
        .map(str::to_string)
        .unwrap_or_else(|| {
            if cfg!(windows) {
                source_action.to_ascii_lowercase()
            } else {
                source_action.to_string()
            }
        })
}

fn deep_merge(target: &mut Value, incoming: Value) {
    match (target, incoming) {
        (Value::Object(target), Value::Object(incoming)) => {
            for (key, value) in incoming {
                match target.get_mut(&key) {
                    Some(existing) if key == "permissions" => match (existing, value) {
                        (Value::Array(existing), Value::Array(mut incoming)) => {
                            existing.append(&mut incoming);
                        }
                        (existing, incoming) => *existing = incoming,
                    },
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

fn push_config_document(layers: &mut Vec<AgentLayer>, document: LocalConfigDocument) {
    let display_name = match document.kind {
        LocalConfigDocumentKind::User => "OpenCode user configuration",
        LocalConfigDocumentKind::ExplicitFile => "OpenCode OPENCODE_CONFIG",
        LocalConfigDocumentKind::Project
        | LocalConfigDocumentKind::Directory(LocalConfigDirectoryKind::Project) => {
            "OpenCode project agent configuration"
        }
        LocalConfigDocumentKind::Directory(LocalConfigDirectoryKind::Legacy) => {
            "OpenCode legacy user configuration"
        }
        LocalConfigDocumentKind::Directory(LocalConfigDirectoryKind::Explicit) => {
            "OpenCode OPENCODE_CONFIG_DIR"
        }
        LocalConfigDocumentKind::Directory(LocalConfigDirectoryKind::User) => {
            "OpenCode user configuration"
        }
        LocalConfigDocumentKind::Inline => "OpenCode OPENCODE_CONFIG_CONTENT",
    };
    let path = match &document.source {
        LocalConfigDocumentSource::File(path) => path.clone(),
        LocalConfigDocumentSource::Inline(_) => PathBuf::from(document.location()),
    };
    layers.push(AgentLayer {
        path,
        scope: document.scope,
        display_name: display_name.to_string(),
        kind: AgentLayerKind::Config(document),
    });
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
    let identity = match &layer.kind {
        AgentLayerKind::Config(document) => document.identity(),
        AgentLayerKind::Markdown { .. } => dunce::canonicalize(&layer.path)
            .unwrap_or_else(|_| normalize_path_lexically(&layer.path))
            .to_string_lossy()
            .into_owned(),
    };
    let source_id = format!(
        "{}-{}",
        layer.source_kind(),
        &digest([layer.source_kind(), identity.as_str()])[..24]
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

fn digest(parts: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        let value = part.as_ref();
        hasher.update(value.len().to_le_bytes());
        hasher.update(value.as_bytes());
    }
    hex::encode(hasher.finalize())
}
