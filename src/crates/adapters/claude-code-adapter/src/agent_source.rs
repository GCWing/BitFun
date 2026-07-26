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
use bitfun_static_hook_support::{
    collect_bounded_regular_files, read_bounded_text, BoundedDirectoryWalkError,
    BoundedDirectoryWalkLimits, BoundedTextRead,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

const PROVIDER_ID: &str = "claude-code.agents";
const ECOSYSTEM_ID: &str = "claude-code";
const MAX_AGENT_FILES: usize = 2048;
const MAX_AGENT_FILE_BYTES: usize = 256 * 1024;
const MAX_TOTAL_PROMPT_BYTES: usize = 8 * 1024 * 1024;

const KNOWN_FIELDS: &[&str] = &[
    "name",
    "description",
    "tools",
    "disallowedTools",
    "model",
    "permissionMode",
    "maxTurns",
    "skills",
    "mcpServers",
    "hooks",
    "memory",
    "background",
    "effort",
    "isolation",
    "color",
    "initialPrompt",
];

#[derive(Debug, Clone)]
pub struct ClaudeCodeSubagentProviderOptions {
    pub user_claude_dir: PathBuf,
    pub project_root_override: Option<PathBuf>,
    pub project_config_enabled: bool,
}

impl ClaudeCodeSubagentProviderOptions {
    pub fn from_environment() -> Self {
        Self {
            user_claude_dir: dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".claude"),
            project_root_override: None,
            project_config_enabled: true,
        }
    }
}

impl Default for ClaudeCodeSubagentProviderOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

pub struct ClaudeCodeSubagentProvider {
    options: ClaudeCodeSubagentProviderOptions,
}

impl ClaudeCodeSubagentProvider {
    pub fn new(options: ClaudeCodeSubagentProviderOptions) -> Self {
        Self { options }
    }

    fn project_root(&self, workspace: &Path) -> PathBuf {
        self.options
            .project_root_override
            .clone()
            .unwrap_or_else(|| find_project_root(workspace))
    }

    fn files(
        &self,
        context: &ExternalSourceContext,
    ) -> Result<Vec<AgentFile>, ExternalSourceProviderError> {
        let mut files = Vec::new();
        let mut rank = 0usize;
        // Claude project agents override personal agents, so personal is the
        // lowest native rank.
        collect_agent_files(
            &self.options.user_claude_dir.join("agents"),
            ExternalSourceScope::UserGlobal,
            "Claude Code personal agent",
            rank,
            &mut files,
        )?;
        rank += 1;
        if self.options.project_config_enabled {
            if let Some(workspace) = &context.workspace_root {
                let project_root = self.project_root(workspace);
                for directory in directories_between(&project_root, workspace) {
                    collect_agent_files(
                        &directory.join(".claude/agents"),
                        ExternalSourceScope::Project,
                        "Claude Code project agent",
                        rank,
                        &mut files,
                    )?;
                    rank += 1;
                }
            }
        }
        files.sort_by(|left, right| (left.rank, &left.path).cmp(&(right.rank, &right.path)));
        files.dedup_by(|left, right| left.path == right.path);
        Ok(files)
    }
}

impl Default for ClaudeCodeSubagentProvider {
    fn default() -> Self {
        Self::new(ClaudeCodeSubagentProviderOptions::default())
    }
}

impl ExternalSubagentSourceProvider for ClaudeCodeSubagentProvider {
    fn identity(&self) -> ExternalSubagentProviderIdentity {
        ExternalSubagentProviderIdentity::new(PROVIDER_ID, ECOSYSTEM_ID, "Claude Code")
            .expect("static Claude Code subagent provider identity is valid")
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
            return Err(ExternalSourceProviderError::new(
                "claude.agent.workspace_invalid",
                "workspace root must be absolute",
                false,
            ));
        }
        let provider = self.identity();
        let mut sources = Vec::new();
        let mut diagnostics = Vec::new();
        let mut contributions = BTreeMap::<String, Vec<AgentContribution>>::new();
        let mut total_prompt_bytes = 0usize;

        for file in self.files(&input.context)? {
            let source_key = source_key(&file.path);
            if input.suppressed_sources.contains(&source_key) {
                sources.push(source_record(
                    &file,
                    source_key,
                    &input.context,
                    ExternalSourceHealth::Available,
                    digest([file.path.to_string_lossy().as_ref()]),
                    Vec::new(),
                ));
                continue;
            }
            match parse_agent_file(&file, source_key.clone()) {
                Ok(contribution) => {
                    total_prompt_bytes =
                        total_prompt_bytes.saturating_add(contribution.prompt.len());
                    if total_prompt_bytes > MAX_TOTAL_PROMPT_BYTES {
                        return Err(ExternalSourceProviderError::new(
                            "claude.agent.total_prompt_bytes_limit",
                            "Claude Code agent prompts exceed the 8 MiB provider limit",
                            false,
                        ));
                    }
                    let content_version = digest([
                        file.path.to_string_lossy().as_ref(),
                        contribution.prompt.as_str(),
                        &serde_json::to_string(&contribution.fields).unwrap_or_default(),
                    ]);
                    sources.push(source_record(
                        &file,
                        source_key,
                        &input.context,
                        ExternalSourceHealth::Available,
                        content_version,
                        Vec::new(),
                    ));
                    contributions
                        .entry(contribution.logical_id.clone())
                        .or_default()
                        .push(contribution);
                }
                Err(error) => {
                    if file.rank > 0 && !contributions.is_empty() {
                        return Err(ExternalSourceProviderError::new(
                            "claude.agent.overlay_invalid",
                            "A higher-priority Claude Code agent source is invalid; lower-priority agents were not reactivated",
                            false,
                        ));
                    }
                    let diagnostic =
                        agent_error(error.code, error.message, Some(source_key.clone()));
                    sources.push(source_record(
                        &file,
                        source_key,
                        &input.context,
                        ExternalSourceHealth::Unavailable,
                        digest([file.path.to_string_lossy().as_ref(), "invalid"]),
                        vec![diagnostic.clone()],
                    ));
                    diagnostics.push(diagnostic);
                }
            }
        }

        let mut definitions = Vec::new();
        for (logical_id, mut items) in contributions {
            items.sort_by(|left, right| (left.rank, &left.path).cmp(&(right.rank, &right.path)));
            definitions.push(materialize_definition(&provider, logical_id, items)?);
        }
        sources.sort_by(|left, right| left.key.cmp(&right.key));
        definitions.sort_by(|left, right| left.logical_id.cmp(&right.logical_id));
        diagnostics
            .sort_by(|left, right| (&left.code, &left.message).cmp(&(&right.code, &right.message)));
        let snapshot = ExternalSubagentProviderSnapshot {
            provider,
            sources,
            definitions,
            diagnostics,
        };
        snapshot.validate().map_err(|error| {
            ExternalSourceProviderError::new(
                "claude.agent.snapshot_invalid",
                error.to_string(),
                false,
            )
        })?;
        Ok(snapshot)
    }

    fn watch_roots(&self, context: &ExternalSourceContext) -> Vec<ExternalWatchRoot> {
        let mut roots = BTreeMap::from([(self.options.user_claude_dir.clone(), true)]);
        if self.options.project_config_enabled {
            if let Some(workspace) = &context.workspace_root {
                let project_root = self.project_root(workspace);
                for directory in directories_between(&project_root, workspace) {
                    roots.insert(directory.join(".claude"), true);
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
struct AgentFile {
    path: PathBuf,
    scope: ExternalSourceScope,
    display_name: String,
    rank: usize,
}

#[derive(Debug, Clone)]
struct AgentContribution {
    source: SourceKey,
    path: PathBuf,
    rank: usize,
    logical_id: String,
    fields: Map<String, Value>,
    prompt: String,
}

fn collect_agent_files(
    directory: &Path,
    scope: ExternalSourceScope,
    display_name: &str,
    rank: usize,
    files: &mut Vec<AgentFile>,
) -> Result<(), ExternalSourceProviderError> {
    let paths = collect_bounded_regular_files(
        directory,
        BoundedDirectoryWalkLimits::for_file_limit(MAX_AGENT_FILES),
        |path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        },
    )
    .map_err(|error| match error {
        BoundedDirectoryWalkError::LimitExceeded(_) => ExternalSourceProviderError::new(
            "claude.agent.file_limit",
            format!(
                "Claude Code agent directory traversal exceeds the bounded {MAX_AGENT_FILES}-file budget"
            ),
            false,
        ),
        BoundedDirectoryWalkError::Io(error) => ExternalSourceProviderError::new(
            "claude.agent.directory_unreadable",
            format!("Failed to enumerate a Claude Code agent directory: {error}"),
            true,
        ),
    })?;
    files.extend(paths.into_iter().map(|path| AgentFile {
        path,
        scope,
        display_name: display_name.to_string(),
        rank,
    }));
    Ok(())
}

fn parse_agent_file(
    file: &AgentFile,
    source: SourceKey,
) -> Result<AgentContribution, ExternalSourceProviderError> {
    let content = match read_bounded_text(&file.path, MAX_AGENT_FILE_BYTES).map_err(|error| {
        ExternalSourceProviderError::new(
            "claude.agent.source_unreadable",
            format!("Failed to read a Claude Code agent file: {error}"),
            true,
        )
    })? {
        BoundedTextRead::Content(content) => content,
        BoundedTextRead::TooLarge => {
            return Err(ExternalSourceProviderError::new(
                "claude.agent.source_too_large",
                "Claude Code agent exceeds the 256 KiB compatibility limit",
                false,
            ));
        }
        BoundedTextRead::InvalidUtf8 => {
            return Err(ExternalSourceProviderError::new(
                "claude.agent.source_invalid_utf8",
                "Claude Code agent is not valid UTF-8",
                false,
            ));
        }
    };
    let (metadata, body) = FrontMatterMarkdown::load_str(&content).map_err(|error| {
        ExternalSourceProviderError::new(
            "claude.agent.markdown_invalid",
            format!("Failed to parse Claude Code agent Markdown: {error}"),
            false,
        )
    })?;
    let value = serde_yaml::from_value::<Value>(metadata).map_err(|error| {
        ExternalSourceProviderError::new(
            "claude.agent.markdown_invalid",
            format!("Failed to normalize Claude Code agent front matter: {error}"),
            false,
        )
    })?;
    let fields = value.as_object().cloned().ok_or_else(|| {
        ExternalSourceProviderError::new(
            "claude.agent.markdown_invalid",
            "Claude Code agent front matter must be an object",
            false,
        )
    })?;
    let logical_id = fields
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| valid_agent_name(name))
        .map(str::to_string)
        .ok_or_else(|| {
            ExternalSourceProviderError::new(
                "claude.agent.name_invalid",
                "Claude Code agent name must use lowercase letters and hyphens",
                false,
            )
        })?;
    let prompt = body.trim().to_string();
    if prompt.is_empty() {
        return Err(ExternalSourceProviderError::new(
            "claude.agent.prompt_invalid",
            "Claude Code agent prompt is empty",
            false,
        ));
    }
    Ok(AgentContribution {
        source,
        path: file.path.clone(),
        rank: file.rank,
        logical_id,
        fields,
        prompt,
    })
}

fn materialize_definition(
    provider: &ExternalSubagentProviderIdentity,
    logical_id: String,
    contributions: Vec<AgentContribution>,
) -> Result<ExternalSubagentDefinition, ExternalSourceProviderError> {
    let local_id = ExternalSubagentLocalId::new(logical_id.clone()).map_err(|error| {
        ExternalSourceProviderError::new("claude.agent.id_invalid", error.to_string(), false)
    })?;
    let mut duplicate_rank = false;
    let mut seen_ranks = BTreeSet::new();
    for contribution in &contributions {
        duplicate_rank |= !seen_ranks.insert(contribution.rank);
    }
    let winner = contributions
        .last()
        .expect("materialization requires at least one contribution");
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
    let fields = &winner.fields;
    let mut invalid = Vec::new();
    let mut blocked = Vec::new();
    let mut degraded = Vec::new();
    if duplicate_rank {
        invalid.push("claude_agent_duplicate_name".to_string());
    }
    if fields
        .keys()
        .any(|field| !KNOWN_FIELDS.contains(&field.as_str()))
    {
        blocked.push("claude_agent_unknown_field".to_string());
    }
    match fields.get("color") {
        None => {}
        Some(Value::String(value))
            if matches!(
                value.as_str(),
                "red" | "blue" | "green" | "yellow" | "purple" | "orange" | "pink" | "cyan"
            ) =>
        {
            degraded.push("claude_agent_color_not_imported".to_string());
        }
        Some(_) => invalid.push("claude_agent_color_invalid".to_string()),
    }
    match fields.get("permissionMode") {
        None => {}
        Some(Value::String(value)) if matches!(value.as_str(), "default" | "manual") => {}
        Some(Value::String(_)) => {
            blocked.push("claude_agent_permission_mode_not_imported".to_string())
        }
        Some(_) => invalid.push("claude_agent_permission_mode_type_invalid".to_string()),
    }
    for (field, code) in [
        ("maxTurns", "claude_agent_max_turns_not_imported"),
        ("skills", "claude_agent_skills_not_imported"),
        ("mcpServers", "claude_agent_mcp_servers_not_imported"),
        ("hooks", "claude_agent_hooks_not_imported"),
        ("memory", "claude_agent_memory_not_imported"),
        ("background", "claude_agent_background_not_imported"),
        ("effort", "claude_agent_effort_not_imported"),
        ("isolation", "claude_agent_isolation_not_imported"),
        ("initialPrompt", "claude_agent_initial_prompt_not_imported"),
    ] {
        if fields.contains_key(field) {
            blocked.push(code.to_string());
        }
    }
    let description = match fields.get("description") {
        None => {
            invalid.push("claude_agent_description_missing".to_string());
            format!("Claude Code agent {logical_id}")
        }
        Some(Value::String(value)) if !value.trim().is_empty() => value.clone(),
        Some(Value::String(_)) => {
            invalid.push("claude_agent_description_invalid".to_string());
            format!("Claude Code agent {logical_id}")
        }
        Some(_) => {
            invalid.push("claude_agent_description_type_invalid".to_string());
            format!("Claude Code agent {logical_id}")
        }
    };
    let requested_model = match fields.get("model") {
        None => ExternalSubagentModelRequest::Default,
        Some(Value::String(value)) if value == "inherit" => ExternalSubagentModelRequest::Default,
        Some(Value::String(value)) if !value.trim().is_empty() => {
            ExternalSubagentModelRequest::Exact {
                provider_hint: None,
                model_name: value.trim().to_string(),
            }
        }
        Some(_) => {
            invalid.push("claude_agent_model_type_invalid".to_string());
            ExternalSubagentModelRequest::Default
        }
    };
    let requested_tools = tool_request(fields, &mut invalid, &mut degraded);
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
    let behavior_diagnostics = diagnostic_codes
        .iter()
        .filter(|code| code.as_str() != "claude_agent_color_not_imported")
        .cloned()
        .collect::<Vec<_>>();
    let behavior_version = ExternalSubagentBehaviorVersion::new(format!(
        "sha256:{}",
        digest([
            logical_id.as_str(),
            winner.prompt.as_str(),
            &serde_json::to_string(&requested_model).unwrap_or_default(),
            &serde_json::to_string(&requested_tools).unwrap_or_default(),
            &provenance
                .iter()
                .map(|item| item.contribution_id.stable_key())
                .collect::<Vec<_>>()
                .join("|"),
            &behavior_diagnostics.join("|"),
        ])
    ))
    .expect("hashed Claude Code agent behavior version is valid");
    let candidate_id =
        external_subagent_candidate_id(&provider.provider_id, &logical_id, &provenance);
    let definition = ExternalSubagentDefinition {
        candidate_id,
        logical_id: logical_id.clone(),
        provenance,
        display_name: logical_id,
        description,
        prompt: SecretText::new(winner.prompt.clone()),
        mode: ExternalSubagentMode::Subagent,
        disabled: false,
        hidden: false,
        requested_model,
        requested_tools,
        compatibility,
        diagnostic_codes,
        behavior_version,
    };
    definition.validate().map_err(|error| {
        ExternalSourceProviderError::new(
            "claude.agent.definition_invalid",
            error.to_string(),
            false,
        )
    })?;
    Ok(definition)
}

fn tool_request(
    fields: &Map<String, Value>,
    invalid: &mut Vec<String>,
    degraded: &mut Vec<String>,
) -> ExternalSubagentToolRequest {
    let mut selectors = BTreeMap::<String, bool>::new();
    match fields.get("tools") {
        None => {
            degraded.push("claude_agent_default_tools_not_imported".to_string());
            for name in ["Glob", "Grep", "LS", "Read"] {
                selectors.insert(name.to_string(), true);
            }
        }
        Some(value) => match string_list(value) {
            Some(values) => {
                for name in values {
                    selectors.insert(name, true);
                }
            }
            None => invalid.push("claude_agent_tools_type_invalid".to_string()),
        },
    }
    if let Some(value) = fields.get("disallowedTools") {
        match string_list(value) {
            Some(values) => {
                for name in values {
                    selectors.insert(name, false);
                }
            }
            None => invalid.push("claude_agent_disallowed_tools_type_invalid".to_string()),
        }
    }
    let uses_conservative_default = !fields.contains_key("tools");
    ExternalSubagentToolRequest {
        selectors: selectors
            .into_iter()
            .map(|(source_name, allowed)| ExternalSubagentToolSelector {
                canonical_host_name: canonical_tool_name(&source_name).map(str::to_string),
                source_name,
                allowed,
            })
            .collect(),
        uses_conservative_default,
    }
}

fn string_list(value: &Value) -> Option<Vec<String>> {
    match value {
        Value::String(value) => Some(
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect(),
        ),
        Value::Array(values) => values
            .iter()
            .map(|value| value.as_str().map(str::to_string))
            .collect(),
        _ => None,
    }
}

fn canonical_tool_name(name: &str) -> Option<&'static str> {
    match name.to_ascii_lowercase().as_str() {
        "ls" | "list" => Some("LS"),
        "read" => Some("Read"),
        "glob" => Some("Glob"),
        "grep" => Some("Grep"),
        _ => None,
    }
}

fn source_record(
    file: &AgentFile,
    key: SourceKey,
    context: &ExternalSourceContext,
    health: ExternalSourceHealth,
    content_version: String,
    diagnostics: Vec<ExternalSourceDiagnostic>,
) -> ExternalSourceRecord {
    ExternalSourceRecord {
        key,
        ecosystem_id: EcosystemId::new(ECOSYSTEM_ID)
            .expect("static Claude Code ecosystem id is valid"),
        display_name: file.display_name.clone(),
        source_kind: "claude_agent_markdown".to_string(),
        scope: file.scope,
        location: file.path.to_string_lossy().to_string(),
        execution_domain_id: context.execution_domain_id.clone(),
        health,
        content_version: format!("sha256:{content_version}"),
        diagnostics,
    }
}

fn source_key(path: &Path) -> SourceKey {
    let identity = dunce::canonicalize(path).unwrap_or_else(|_| normalize_path_lexically(path));
    SourceKey::new(
        PROVIDER_ID,
        format!(
            "claude_agent_markdown-{}",
            &digest([identity.to_string_lossy().as_ref()])[..24]
        ),
    )
    .expect("hashed Claude Code agent source id is valid")
}

fn agent_error(
    code: impl Into<String>,
    message: impl Into<String>,
    source: Option<SourceKey>,
) -> ExternalSourceDiagnostic {
    ExternalSourceDiagnostic::error(code, message, source)
        .with_asset_kind(ExternalSourceAssetKind::Subagent)
}

fn valid_agent_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
        && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value.as_bytes().last().is_some_and(u8::is_ascii_lowercase)
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
