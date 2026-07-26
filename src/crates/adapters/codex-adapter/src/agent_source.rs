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
    SecretText,
};
use bitfun_static_hook_support::{
    collect_bounded_regular_files, read_bounded_text, resolve_bounded_regular_file,
    BoundedDirectoryWalkError, BoundedDirectoryWalkLimits, BoundedFileResolveError,
    BoundedTextRead,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use toml::Value;

const PROVIDER_ID: &str = "codex.agents";
const ECOSYSTEM_ID: &str = "codex";
const MAX_CONFIG_FILE_BYTES: usize = 1024 * 1024;
const MAX_AGENT_FILE_BYTES: usize = 256 * 1024;
const MAX_AGENT_FILES: usize = 2048;
const MAX_TOTAL_PROMPT_BYTES: usize = 8 * 1024 * 1024;

const AGENTS_CONTROL_FIELDS: &[&str] = &[
    "enabled",
    "max_concurrent_threads_per_session",
    "max_threads",
    "max_depth",
    "default_subagent_model",
    "default_subagent_reasoning_effort",
    "job_max_runtime_seconds",
    "interrupt_message",
];

const DISPLAY_FIELDS: &[&str] = &["name", "description", "nickname_candidates"];
const SUPPORTED_BEHAVIOR_FIELDS: &[&str] = &["developer_instructions", "model"];

#[derive(Debug, Clone)]
pub struct CodexSubagentProviderOptions {
    pub codex_home: PathBuf,
    pub project_root_override: Option<PathBuf>,
    pub project_config_enabled: bool,
}

impl CodexSubagentProviderOptions {
    pub fn from_environment() -> Self {
        let codex_home = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".codex")
            });
        Self {
            codex_home,
            project_root_override: None,
            project_config_enabled: true,
        }
    }
}

impl Default for CodexSubagentProviderOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

pub struct CodexSubagentProvider {
    options: CodexSubagentProviderOptions,
}

impl CodexSubagentProvider {
    pub fn new(options: CodexSubagentProviderOptions) -> Self {
        Self { options }
    }

    fn project_root(&self, workspace: &Path) -> PathBuf {
        self.options
            .project_root_override
            .clone()
            .unwrap_or_else(|| find_project_root(workspace))
    }

    fn layers(&self, context: &ExternalSourceContext) -> Vec<NativeLayer> {
        let mut layers = vec![NativeLayer {
            config_directory: self.options.codex_home.clone(),
            scope: ExternalSourceScope::UserGlobal,
            rank: 0,
            display_name: "Codex user agent configuration",
        }];
        if self.options.project_config_enabled {
            if let Some(workspace) = &context.workspace_root {
                for (index, directory) in
                    directories_between(&self.project_root(workspace), workspace)
                        .into_iter()
                        .enumerate()
                {
                    layers.push(NativeLayer {
                        config_directory: directory.join(".codex"),
                        scope: ExternalSourceScope::Project,
                        rank: index + 1,
                        display_name: "Codex project agent configuration",
                    });
                }
            }
        }
        layers
    }
}

impl Default for CodexSubagentProvider {
    fn default() -> Self {
        Self::new(CodexSubagentProviderOptions::default())
    }
}

impl ExternalSubagentSourceProvider for CodexSubagentProvider {
    fn identity(&self) -> ExternalSubagentProviderIdentity {
        ExternalSubagentProviderIdentity::new(PROVIDER_ID, ECOSYSTEM_ID, "Codex")
            .expect("static Codex subagent provider identity is valid")
    }

    fn discover(
        &self,
        input: &ExternalSubagentDiscoveryInput,
    ) -> Result<ExternalSubagentProviderSnapshot, ExternalSourceProviderError> {
        if input
            .context
            .workspace_root
            .as_ref()
            .is_some_and(|workspace| !workspace.is_absolute())
        {
            return Err(provider_error(
                "workspace_invalid",
                "workspace root must be absolute",
                false,
            ));
        }

        let provider = self.identity();
        let mut sources = Vec::new();
        let mut diagnostics = Vec::new();
        let mut contributions = BTreeMap::<String, Vec<RoleContribution>>::new();
        let mut effective_controls = BTreeMap::<String, Value>::new();
        let mut total_prompt_bytes = 0usize;

        for layer in self.layers(&input.context) {
            let layer_result = discover_layer(&layer, &input.context, &input.suppressed_sources)?;
            for mut source in layer_result.sources {
                let suppressed = input.suppressed_sources.contains(&source.record.key);
                source.record.diagnostics.extend(source.diagnostics.clone());
                diagnostics.extend(source.diagnostics.clone());
                if !suppressed {
                    effective_controls.extend(source.controls.clone());
                    for contribution in source.contributions {
                        total_prompt_bytes =
                            total_prompt_bytes.saturating_add(contribution.behavior.prompt.len());
                        if total_prompt_bytes > MAX_TOTAL_PROMPT_BYTES {
                            return Err(provider_error(
                                "total_prompt_bytes_limit",
                                "Codex agent instructions exceed the 8 MiB provider limit",
                                false,
                            ));
                        }
                        contributions
                            .entry(contribution.logical_id.clone())
                            .or_default()
                            .push(contribution);
                    }
                }
                sources.push(source.record);
            }
        }

        let mut definitions = Vec::new();
        for (logical_id, mut items) in contributions {
            items.sort_by(|left, right| (left.rank, &left.path).cmp(&(right.rank, &right.path)));
            definitions.push(materialize_definition(
                &provider,
                logical_id,
                items,
                &effective_controls,
            )?);
        }
        sources.sort_by(|left, right| left.key.cmp(&right.key));
        sources.dedup_by(|left, right| left.key == right.key);
        definitions.sort_by(|left, right| left.logical_id.cmp(&right.logical_id));
        diagnostics.sort_by(|left, right| {
            (&left.code, &left.message, &left.source).cmp(&(
                &right.code,
                &right.message,
                &right.source,
            ))
        });
        diagnostics.dedup();
        let snapshot = ExternalSubagentProviderSnapshot {
            provider,
            sources,
            definitions,
            diagnostics,
        };
        snapshot
            .validate()
            .map_err(|error| provider_error("snapshot_invalid", &error.to_string(), false))?;
        Ok(snapshot)
    }

    fn watch_roots(&self, context: &ExternalSourceContext) -> Vec<ExternalWatchRoot> {
        let mut roots = BTreeMap::from([(self.options.codex_home.clone(), true)]);
        if self.options.project_config_enabled {
            if let Some(workspace) = &context.workspace_root {
                for directory in directories_between(&self.project_root(workspace), workspace) {
                    roots.insert(directory.join(".codex"), true);
                }
            }
        }
        roots
            .into_iter()
            .map(|(path, recursive)| ExternalWatchRoot { path, recursive })
            .collect()
    }
}

struct NativeLayer {
    config_directory: PathBuf,
    scope: ExternalSourceScope,
    rank: usize,
    display_name: &'static str,
}

struct LayerDiscovery {
    sources: Vec<DiscoveredSource>,
}

struct DiscoveredSource {
    record: ExternalSourceRecord,
    contributions: Vec<RoleContribution>,
    controls: BTreeMap<String, Value>,
    diagnostics: Vec<ExternalSourceDiagnostic>,
}

#[derive(Clone)]
struct RoleContribution {
    source: SourceKey,
    path: PathBuf,
    rank: usize,
    logical_id: String,
    description: Option<String>,
    behavior: RoleBehavior,
    invalid_codes: Vec<String>,
}

#[derive(Clone, Default)]
struct RoleBehavior {
    prompt: String,
    model: Option<String>,
    behavior_fields: BTreeMap<String, Value>,
}

fn discover_layer(
    layer: &NativeLayer,
    context: &ExternalSourceContext,
    suppressed_sources: &BTreeSet<SourceKey>,
) -> Result<LayerDiscovery, ExternalSourceProviderError> {
    let mut sources = Vec::new();
    let mut layer_names = BTreeSet::new();
    let mut declared_files = BTreeSet::new();
    let config_file = layer.config_directory.join("config.toml");

    if should_inspect(&config_file) {
        let source = source_key(&config_file, "codex_agent_config");
        let resolved_config = resolve_bounded_regular_file(&config_file, &layer.config_directory)
            .map_err(role_file_error)?;
        match parse_toml_file(&resolved_config, MAX_CONFIG_FILE_BYTES) {
            Ok((raw, value)) => {
                let mut contributions = Vec::new();
                let mut controls = BTreeMap::new();
                let mut source_diagnostics = Vec::new();
                let mut referenced_content = Vec::new();
                if let Some(agents) = value.get("agents") {
                    match agents.as_table() {
                        Some(agents) => {
                            for (name, value) in agents.iter().filter(|(name, value)| {
                                AGENTS_CONTROL_FIELDS.contains(&name.as_str()) || !value.is_table()
                            }) {
                                controls.insert(name.clone(), value.clone());
                            }
                            for (declared_name, role) in agents.iter().filter(|(name, value)| {
                                !AGENTS_CONTROL_FIELDS.contains(&name.as_str()) && value.is_table()
                            }) {
                                match parse_declared_role(
                                    declared_name,
                                    role,
                                    &config_file,
                                    source.clone(),
                                    layer.rank,
                                ) {
                                    Ok((contribution, referenced_file, referenced_raw)) => {
                                        if !layer_names.insert(contribution.logical_id.clone()) {
                                            source_diagnostics.push(agent_warning(
                                                "duplicate_name",
                                                format!(
                                                    "Duplicate Codex agent role '{}' in one config layer was ignored",
                                                    contribution.logical_id
                                                ),
                                                Some(source.clone()),
                                            ));
                                            continue;
                                        }
                                        if let Some(file) = referenced_file {
                                            declared_files.insert(normalize_path_lexically(&file));
                                        }
                                        referenced_content.push(referenced_raw);
                                        contributions.push(contribution);
                                    }
                                    Err(error) => {
                                        let logical_id = normalize_logical_id(declared_name);
                                        if validate_logical_id(&logical_id).is_ok()
                                            && layer_names.insert(logical_id.clone())
                                        {
                                            contributions.push(RoleContribution {
                                                source: source.clone(),
                                                path: config_file.clone(),
                                                rank: layer.rank,
                                                logical_id,
                                                description: None,
                                                behavior: RoleBehavior::default(),
                                                invalid_codes: vec![
                                                    "codex_agent_role_invalid".to_string()
                                                ],
                                            });
                                        }
                                        source_diagnostics.push(agent_error(
                                            error.code,
                                            error.message,
                                            Some(source.clone()),
                                        ));
                                    }
                                }
                            }
                        }
                        None => source_diagnostics.push(agent_error(
                            "agents_table_invalid",
                            "Codex agents config must be a TOML table",
                            Some(source.clone()),
                        )),
                    }
                }
                let mut version_parts = vec![raw.as_str()];
                version_parts.extend(referenced_content.iter().map(String::as_str));
                let health = if source_diagnostics.iter().any(|item| {
                    item.severity
                        == bitfun_product_domains::external_sources::ExternalSourceDiagnosticSeverity::Error
                }) {
                    ExternalSourceHealth::Partial
                } else if source_diagnostics.is_empty() {
                    ExternalSourceHealth::Available
                } else {
                    ExternalSourceHealth::Degraded
                };
                sources.push(DiscoveredSource {
                    record: source_record(
                        &config_file,
                        source,
                        layer,
                        context,
                        health,
                        version_parts,
                    ),
                    contributions,
                    controls,
                    diagnostics: source_diagnostics,
                });
            }
            Err(error) => {
                if !suppressed_sources.contains(&source) {
                    return Err(provider_error(
                        "config_invalid",
                        &format!(
                            "Required Codex agent config could not be loaded: {}",
                            error.message
                        ),
                        false,
                    ));
                }
                let diagnostic = agent_error(error.code, error.message, Some(source.clone()));
                sources.push(DiscoveredSource {
                    record: source_record(
                        &config_file,
                        source,
                        layer,
                        context,
                        ExternalSourceHealth::Unavailable,
                        ["invalid"],
                    ),
                    contributions: Vec::new(),
                    controls: BTreeMap::new(),
                    diagnostics: vec![diagnostic],
                });
            }
        }
    }

    let mut files = Vec::new();
    collect_agent_files(&layer.config_directory.join("agents"), &mut files)?;
    for path in files {
        if declared_files.contains(&normalize_path_lexically(&path)) {
            continue;
        }
        let source = source_key(&path, "codex_agent_file");
        match parse_standalone_role(&path, source.clone(), layer.rank) {
            Ok((raw, contribution)) => {
                if !layer_names.insert(contribution.logical_id.clone()) {
                    let diagnostic = agent_warning(
                        "duplicate_name",
                        format!(
                            "Duplicate Codex agent role '{}' in one config layer was ignored",
                            contribution.logical_id
                        ),
                        Some(source.clone()),
                    );
                    sources.push(DiscoveredSource {
                        record: source_record(
                            &path,
                            source,
                            layer,
                            context,
                            ExternalSourceHealth::Degraded,
                            [raw.as_str()],
                        ),
                        contributions: Vec::new(),
                        controls: BTreeMap::new(),
                        diagnostics: vec![diagnostic],
                    });
                    continue;
                }
                sources.push(DiscoveredSource {
                    record: source_record(
                        &path,
                        source,
                        layer,
                        context,
                        ExternalSourceHealth::Available,
                        [raw.as_str()],
                    ),
                    contributions: vec![contribution],
                    controls: BTreeMap::new(),
                    diagnostics: Vec::new(),
                });
            }
            Err(error) => {
                let diagnostic = agent_error(error.code, error.message, Some(source.clone()));
                sources.push(DiscoveredSource {
                    record: source_record(
                        &path,
                        source,
                        layer,
                        context,
                        ExternalSourceHealth::Unavailable,
                        ["invalid"],
                    ),
                    contributions: Vec::new(),
                    controls: BTreeMap::new(),
                    diagnostics: vec![diagnostic],
                });
            }
        }
    }
    Ok(LayerDiscovery { sources })
}

fn parse_declared_role(
    declared_name: &str,
    value: &Value,
    declaring_file: &Path,
    source: SourceKey,
    rank: usize,
) -> Result<(RoleContribution, Option<PathBuf>, String), ExternalSourceProviderError> {
    let fields = value.as_table().ok_or_else(|| {
        provider_error(
            "role_invalid",
            "Codex agent role declaration must be a TOML table",
            false,
        )
    })?;
    if fields.keys().any(|key| {
        !matches!(
            key.as_str(),
            "description" | "config_file" | "nickname_candidates"
        )
    }) {
        return Err(provider_error(
            "role_invalid",
            "Codex agent role declaration contains unknown fields",
            false,
        ));
    }
    let declared_description = optional_string(fields.get("description"), "description")?;
    let config_path = optional_string(fields.get("config_file"), "config_file")?.map(|path| {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            normalize_path_lexically(&path)
        } else {
            normalize_path_lexically(&declaring_file.parent().unwrap_or(Path::new(".")).join(path))
        }
    });
    let mut logical_id = normalize_logical_id(declared_name);
    let mut description = declared_description;
    let mut behavior = RoleBehavior::default();
    let mut raw = String::new();
    if let Some(path) = &config_path {
        let allowed_root = declaring_file.parent().unwrap_or(Path::new("."));
        let resolved_path =
            resolve_bounded_regular_file(path, allowed_root).map_err(role_file_error)?;
        let (role_raw, role_value) = parse_toml_file(&resolved_path, MAX_AGENT_FILE_BYTES)?;
        raw = role_raw;
        let table = role_value.as_table().ok_or_else(|| {
            provider_error(
                "role_file_invalid",
                "Codex agent role file must be a table",
                false,
            )
        })?;
        if let Some(name) = optional_string(table.get("name"), "name")? {
            logical_id = normalize_logical_id(&name);
        }
        if let Some(file_description) = optional_string(table.get("description"), "description")? {
            description = Some(file_description);
        }
        behavior = parse_role_behavior(table)?;
    }
    validate_logical_id(&logical_id)?;
    Ok((
        RoleContribution {
            source,
            path: declaring_file.to_path_buf(),
            rank,
            logical_id,
            description,
            behavior,
            invalid_codes: Vec::new(),
        },
        config_path,
        raw,
    ))
}

fn parse_standalone_role(
    path: &Path,
    source: SourceKey,
    rank: usize,
) -> Result<(String, RoleContribution), ExternalSourceProviderError> {
    let (raw, value) = parse_toml_file(path, MAX_AGENT_FILE_BYTES)?;
    let fields = value.as_table().ok_or_else(|| {
        provider_error(
            "role_file_invalid",
            "Codex agent role file must be a table",
            false,
        )
    })?;
    let logical_id = optional_string(fields.get("name"), "name")?
        .map(|name| normalize_logical_id(&name))
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            provider_error(
                "name_invalid",
                "Standalone Codex agent role must define a non-empty name",
                false,
            )
        })?;
    validate_logical_id(&logical_id)?;
    let description = optional_string(fields.get("description"), "description")?;
    let behavior = parse_role_behavior(fields)?;
    if behavior.prompt.trim().is_empty() {
        return Err(provider_error(
            "instructions_missing",
            "Standalone Codex agent role must define developer_instructions",
            false,
        ));
    }
    Ok((
        raw,
        RoleContribution {
            source,
            path: path.to_path_buf(),
            rank,
            logical_id,
            description,
            behavior,
            invalid_codes: Vec::new(),
        },
    ))
}

fn parse_role_behavior(
    fields: &toml::map::Map<String, Value>,
) -> Result<RoleBehavior, ExternalSourceProviderError> {
    let prompt = optional_string(
        fields.get("developer_instructions"),
        "developer_instructions",
    )?
    .unwrap_or_default();
    let model = optional_string(fields.get("model"), "model")?;
    let behavior_fields = fields
        .iter()
        .filter(|(key, _)| !DISPLAY_FIELDS.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    Ok(RoleBehavior {
        prompt,
        model,
        behavior_fields,
    })
}

fn materialize_definition(
    provider: &ExternalSubagentProviderIdentity,
    logical_id: String,
    contributions: Vec<RoleContribution>,
    controls: &BTreeMap<String, Value>,
) -> Result<ExternalSubagentDefinition, ExternalSourceProviderError> {
    let local_id = ExternalSubagentLocalId::new(logical_id.clone())
        .map_err(|error| provider_error("id_invalid", &error.to_string(), false))?;
    let mut effective_description = None;
    let mut effective_behavior = RoleBehavior::default();
    let mut behavior_present = false;
    for contribution in &contributions {
        if contribution.description.is_some() {
            effective_description.clone_from(&contribution.description);
        }
        if contribution
            .behavior
            .behavior_fields
            .contains_key("developer_instructions")
        {
            effective_behavior
                .prompt
                .clone_from(&contribution.behavior.prompt);
        }
        if contribution.behavior.behavior_fields.contains_key("model") {
            effective_behavior
                .model
                .clone_from(&contribution.behavior.model);
        }
        if !contribution.behavior.behavior_fields.is_empty() {
            effective_behavior
                .behavior_fields
                .extend(contribution.behavior.behavior_fields.clone());
            behavior_present = true;
        }
    }
    let provenance = contributions
        .iter()
        .enumerate()
        .map(|(index, contribution)| ExternalSubagentProvenanceRef {
            contribution_id: ExternalSubagentContributionId::new(
                contribution.source.clone(),
                local_id.clone(),
            ),
            role: if index == 0 {
                ExternalSubagentContributionRole::Base
            } else {
                ExternalSubagentContributionRole::Overlay
            },
        })
        .collect::<Vec<_>>();
    let mut invalid = contributions
        .iter()
        .flat_map(|contribution| contribution.invalid_codes.iter().cloned())
        .collect::<Vec<_>>();
    let mut blocked = Vec::new();
    if effective_description.is_none() {
        invalid.push("codex_agent_description_missing".to_string());
    }
    if !behavior_present || effective_behavior.prompt.trim().is_empty() {
        blocked.push("codex_agent_developer_instructions_missing".to_string());
    }
    for key in effective_behavior.behavior_fields.keys() {
        if SUPPORTED_BEHAVIOR_FIELDS.contains(&key.as_str()) {
            continue;
        }
        let code = match key.as_str() {
            "model_reasoning_effort" | "model_reasoning_summary" | "model_verbosity" => {
                "codex_agent_reasoning_not_imported"
            }
            "sandbox_mode" | "sandbox_workspace_write" | "permissions" => {
                "codex_agent_sandbox_not_imported"
            }
            "approval_policy" | "default_permissions" => "codex_agent_approval_policy_not_imported",
            "mcp_servers" => "codex_agent_mcp_not_imported",
            "skills" => "codex_agent_skills_not_imported",
            _ => "codex_agent_unknown_field",
        };
        blocked.push(code.to_string());
    }
    let default_model = controls
        .get("default_subagent_model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let requested_model = match effective_behavior.model.as_deref().or(default_model) {
        None | Some("") => ExternalSubagentModelRequest::Default,
        Some(model) => ExternalSubagentModelRequest::Exact {
            provider_hint: None,
            model_name: model.trim().to_string(),
        },
    };
    let disabled = match controls.get("enabled") {
        None | Some(Value::Boolean(true)) => false,
        Some(Value::Boolean(false)) => true,
        Some(_) => {
            blocked.push("codex_agent_control_invalid".to_string());
            false
        }
    };
    for key in controls.keys() {
        match key.as_str() {
            "enabled" | "default_subagent_model" | "job_max_runtime_seconds" => {}
            "default_subagent_reasoning_effort" => {
                blocked.push("codex_agent_default_reasoning_not_imported".to_string());
            }
            "max_concurrent_threads_per_session" | "max_threads" | "max_depth" => {
                blocked.push("codex_agent_concurrency_not_imported".to_string());
            }
            "interrupt_message" => {
                blocked.push("codex_agent_interrupt_behavior_not_imported".to_string());
            }
            _ => blocked.push("codex_agent_unknown_control".to_string()),
        }
    }
    if controls
        .get("default_subagent_model")
        .is_some_and(|value| !matches!(value, Value::String(model) if !model.trim().is_empty()))
    {
        blocked.push("codex_agent_control_invalid".to_string());
    }
    let compatibility = if !invalid.is_empty() {
        ExternalSubagentCompatibilityState::Invalid
    } else if !blocked.is_empty() {
        ExternalSubagentCompatibilityState::Blocked
    } else {
        ExternalSubagentCompatibilityState::Ready
    };
    let mut diagnostic_codes = invalid;
    diagnostic_codes.extend(blocked);
    diagnostic_codes.sort();
    diagnostic_codes.dedup();
    let requested_tools = ExternalSubagentToolRequest {
        selectors: Vec::new(),
        uses_conservative_default: true,
    };
    let behavior_version = ExternalSubagentBehaviorVersion::new(format!(
        "sha256:{}",
        digest([
            logical_id.as_str(),
            effective_behavior.prompt.as_str(),
            &serde_json::to_string(&requested_model).unwrap_or_default(),
            &serde_json::to_string(controls).unwrap_or_default(),
            &diagnostic_codes.join("|"),
        ])
    ))
    .expect("hashed Codex agent behavior version is valid");
    let candidate_id =
        external_subagent_candidate_id(&provider.provider_id, &logical_id, &provenance);
    let definition = ExternalSubagentDefinition {
        candidate_id,
        logical_id: logical_id.clone(),
        provenance,
        display_name: logical_id,
        description: effective_description.unwrap_or_default(),
        prompt: SecretText::new(effective_behavior.prompt),
        mode: ExternalSubagentMode::Subagent,
        disabled,
        hidden: false,
        requested_model,
        requested_tools,
        compatibility,
        diagnostic_codes,
        behavior_version,
    };
    definition
        .validate()
        .map_err(|error| provider_error("definition_invalid", &error.to_string(), false))?;
    Ok(definition)
}

fn parse_toml_file(
    path: &Path,
    max_bytes: usize,
) -> Result<(String, Value), ExternalSourceProviderError> {
    let raw = match read_bounded_text(path, max_bytes).map_err(|error| {
        provider_error(
            "source_unreadable",
            &format!("Failed to read Codex configuration: {error}"),
            true,
        )
    })? {
        BoundedTextRead::Content(raw) => raw,
        BoundedTextRead::TooLarge => {
            return Err(provider_error(
                "source_too_large",
                "Codex configuration exceeds its compatibility size limit",
                false,
            ));
        }
        BoundedTextRead::InvalidUtf8 => {
            return Err(provider_error(
                "source_invalid_utf8",
                "Codex configuration is not valid UTF-8",
                false,
            ));
        }
    };
    let value = toml::from_str(&raw).map_err(|error| {
        provider_error(
            "source_invalid",
            &format!("Failed to parse Codex TOML: {error}"),
            false,
        )
    })?;
    Ok((raw, value))
}

fn collect_agent_files(
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), ExternalSourceProviderError> {
    *files = collect_bounded_regular_files(
        directory,
        BoundedDirectoryWalkLimits::for_file_limit(MAX_AGENT_FILES),
        |path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
        },
    )
    .map_err(|error| match error {
        BoundedDirectoryWalkError::LimitExceeded(_) => provider_error(
            "file_limit",
            &format!(
                "Codex agent directory traversal exceeds the bounded {MAX_AGENT_FILES}-file budget"
            ),
            false,
        ),
        BoundedDirectoryWalkError::Io(error) => provider_error(
            "directory_unreadable",
            &format!("Failed to enumerate a Codex agent directory: {error}"),
            true,
        ),
    })?;
    Ok(())
}

fn optional_string(
    value: Option<&Value>,
    field: &str,
) -> Result<Option<String>, ExternalSourceProviderError> {
    match value {
        None => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => {
            Ok(Some(value.trim().to_string()))
        }
        Some(Value::String(_)) => Err(provider_error(
            "field_invalid",
            &format!("Codex agent field '{field}' must not be blank"),
            false,
        )),
        Some(_) => Err(provider_error(
            "field_invalid",
            &format!("Codex agent field '{field}' must be a string"),
            false,
        )),
    }
}

fn source_record<'a>(
    path: &Path,
    key: SourceKey,
    layer: &NativeLayer,
    context: &ExternalSourceContext,
    health: ExternalSourceHealth,
    contents: impl IntoIterator<Item = &'a str>,
) -> ExternalSourceRecord {
    ExternalSourceRecord {
        key,
        ecosystem_id: EcosystemId::new(ECOSYSTEM_ID).expect("static Codex ecosystem id is valid"),
        display_name: layer.display_name.to_string(),
        source_kind: "codex_agent_toml".to_string(),
        scope: layer.scope,
        location: path.to_string_lossy().to_string(),
        execution_domain_id: context.execution_domain_id.clone(),
        health,
        content_version: format!("sha256:{}", digest(contents)),
        diagnostics: Vec::new(),
    }
}

fn source_key(path: &Path, kind: &str) -> SourceKey {
    let identity = dunce::canonicalize(path).unwrap_or_else(|_| normalize_path_lexically(path));
    SourceKey::new(
        PROVIDER_ID,
        format!(
            "codex_agent-{}",
            &digest([kind, identity.to_string_lossy().as_ref()])[..24]
        ),
    )
    .expect("hashed Codex agent source id is valid")
}

fn should_inspect(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(metadata) => metadata.is_file(),
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    }
}

fn provider_error(suffix: &str, message: &str, transient: bool) -> ExternalSourceProviderError {
    ExternalSourceProviderError::new(format!("codex.agent.{suffix}"), message, transient)
}

fn role_file_error(error: BoundedFileResolveError) -> ExternalSourceProviderError {
    match error {
        BoundedFileResolveError::OutsideRoot => provider_error(
            "role_file_outside_root",
            "Codex agent role file resolves outside its allowed .codex source root",
            false,
        ),
        BoundedFileResolveError::NotRegular => provider_error(
            "role_file_not_regular",
            "Codex agent role file is not a regular file",
            false,
        ),
        BoundedFileResolveError::Io(error) => provider_error(
            "role_file_unreadable",
            &format!("Failed to resolve Codex agent role file: {error}"),
            true,
        ),
    }
}

fn agent_error(
    suffix: impl AsRef<str>,
    message: impl Into<String>,
    source: Option<SourceKey>,
) -> ExternalSourceDiagnostic {
    let suffix = suffix.as_ref();
    let code = if suffix.starts_with("codex.agent.") {
        suffix.to_string()
    } else {
        format!("codex.agent.{suffix}")
    };
    ExternalSourceDiagnostic::error(code, message, source)
        .with_asset_kind(ExternalSourceAssetKind::Subagent)
}

fn agent_warning(
    suffix: impl AsRef<str>,
    message: impl Into<String>,
    source: Option<SourceKey>,
) -> ExternalSourceDiagnostic {
    ExternalSourceDiagnostic::warning(format!("codex.agent.{}", suffix.as_ref()), message, source)
        .with_asset_kind(ExternalSourceAssetKind::Subagent)
}

fn validate_logical_id(logical_id: &str) -> Result<(), ExternalSourceProviderError> {
    ExternalSubagentLocalId::new(logical_id.to_string())
        .map(|_| ())
        .map_err(|error| provider_error("name_invalid", &error.to_string(), false))
}

fn normalize_logical_id(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "-")
}

fn digest<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    hex::encode(hasher.finalize())
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

fn normalize_path_lexically(path: &Path) -> PathBuf {
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
