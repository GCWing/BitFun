use bitfun_product_domains::external_sources::{
    EcosystemId, ExpandedPromptCommand, ExternalSourceAssetKind, ExternalSourceContext,
    ExternalSourceDiagnostic, ExternalSourceHealth, ExternalSourceProviderError,
    ExternalSourceRecord, ExternalSourceScope, ExternalWatchRoot, PromptCommandAvailability,
    PromptCommandDefinition, PromptCommandProviderIdentity, PromptCommandProviderSnapshot,
    PromptCommandSourceProvider, SourceKey, SourceQualifiedCommandId,
};
use bitfun_services_core::markdown::FrontMatterMarkdown;
use bitfun_static_hook_support::{
    collect_bounded_regular_files, read_bounded_text, BoundedDirectoryWalkError,
    BoundedDirectoryWalkLimits, BoundedTextRead,
};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

const PROVIDER_ID: &str = "claude-code.commands";
const ECOSYSTEM_ID: &str = "claude-code";
const MAX_COMMAND_FILES: usize = 2048;
const MAX_COMMAND_FILE_BYTES: usize = 256 * 1024;
const MAX_TOTAL_TEMPLATE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ClaudeCodeCommandProviderOptions {
    pub user_claude_dir: PathBuf,
    pub project_root_override: Option<PathBuf>,
    pub project_config_enabled: bool,
}

impl ClaudeCodeCommandProviderOptions {
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

impl Default for ClaudeCodeCommandProviderOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

pub struct ClaudeCodeCommandProvider {
    options: ClaudeCodeCommandProviderOptions,
}

impl ClaudeCodeCommandProvider {
    pub fn new(options: ClaudeCodeCommandProviderOptions) -> Self {
        Self { options }
    }

    fn project_root(&self, workspace: &Path) -> PathBuf {
        self.options
            .project_root_override
            .clone()
            .unwrap_or_else(|| find_project_root(workspace))
    }

    fn layers(&self, context: &ExternalSourceContext) -> Vec<CommandLayer> {
        let mut layers = Vec::new();
        if self.options.project_config_enabled {
            if let Some(workspace) = &context.workspace_root {
                let project_root = self.project_root(workspace);
                for directory in directories_between(&project_root, workspace) {
                    push_layer(
                        &mut layers,
                        directory.join(".claude"),
                        ExternalSourceScope::Project,
                        "Claude Code project commands",
                    );
                }
            }
        }
        // Claude's personal Skill/Command scope has higher precedence than
        // project scope. Keep it last because resolve_commands is last-wins.
        push_layer(
            &mut layers,
            self.options.user_claude_dir.clone(),
            ExternalSourceScope::UserGlobal,
            "Claude Code personal commands",
        );
        deduplicate_layers(layers)
    }
}

impl Default for ClaudeCodeCommandProvider {
    fn default() -> Self {
        Self::new(ClaudeCodeCommandProviderOptions::default())
    }
}

impl PromptCommandSourceProvider for ClaudeCodeCommandProvider {
    fn identity(&self) -> PromptCommandProviderIdentity {
        PromptCommandProviderIdentity::new(PROVIDER_ID, ECOSYSTEM_ID, "Claude Code")
            .expect("static Claude Code command provider identity is valid")
    }

    fn discover(
        &self,
        context: &ExternalSourceContext,
    ) -> Result<PromptCommandProviderSnapshot, ExternalSourceProviderError> {
        if context
            .workspace_root
            .as_ref()
            .is_some_and(|workspace| !workspace.is_absolute())
        {
            return Err(ExternalSourceProviderError::new(
                "claude.command.workspace_invalid",
                "workspace root must be absolute",
                false,
            ));
        }

        let layers = self.layers(context);
        let skill_names = collect_effective_skill_names(&layers)?;
        let mut sources = Vec::new();
        let mut commands = Vec::new();
        let mut unavailable_command_ids = Vec::new();
        let mut diagnostics = Vec::new();
        let mut total_template_bytes = 0usize;

        for layer in layers {
            if !should_inspect_directory(&layer.commands_dir)? {
                continue;
            }
            let source_key = source_key(&layer);
            let parsed = parse_layer(&layer, &source_key)?;
            let mut source_diagnostics = parsed.diagnostics;
            let mut source_commands = Vec::new();
            let mut has_restricted = false;
            for input in parsed.commands {
                if skill_names.contains(&input.name.to_ascii_lowercase()) {
                    source_diagnostics.push(command_warning(
                        "claude.command.shadowed_by_skill",
                        format!(
                            "Claude Code Skill '{}' shadows the legacy command with the same name",
                            input.name
                        ),
                        Some(source_key.clone()),
                    ));
                    continue;
                }
                total_template_bytes = total_template_bytes.saturating_add(input.template.len());
                if total_template_bytes > MAX_TOTAL_TEMPLATE_BYTES {
                    return Err(ExternalSourceProviderError::new(
                        "claude.command.total_template_bytes_limit",
                        "Claude Code command templates exceed the 8 MiB provider limit",
                        false,
                    ));
                }
                let definition = command_definition(source_key.clone(), input)?;
                has_restricted |= !matches!(
                    definition.availability,
                    PromptCommandAvailability::Available
                );
                source_commands.push(definition);
            }
            unavailable_command_ids.extend(
                parsed.unavailable_names.into_iter().filter_map(|name| {
                    SourceQualifiedCommandId::new(source_key.clone(), name).ok()
                }),
            );
            let health = if !source_diagnostics.is_empty() {
                ExternalSourceHealth::Degraded
            } else if has_restricted {
                ExternalSourceHealth::Partial
            } else {
                ExternalSourceHealth::Available
            };
            diagnostics.extend(source_diagnostics.clone());
            sources.push(ExternalSourceRecord {
                key: source_key,
                ecosystem_id: EcosystemId::new(ECOSYSTEM_ID)
                    .expect("static Claude Code ecosystem id is valid"),
                display_name: layer.display_name,
                source_kind: "claude_command_directory".to_string(),
                scope: layer.scope,
                location: layer.commands_dir.to_string_lossy().to_string(),
                execution_domain_id: context.execution_domain_id.clone(),
                health,
                content_version: parsed.content_version,
                diagnostics: source_diagnostics,
            });
            commands.extend(source_commands);
        }

        // Each layer is internally sorted by parse_layer. Preserve layer order
        // here because resolve_commands applies Claude's native last-wins
        // precedence to the ordered contributions.
        unavailable_command_ids.sort();
        unavailable_command_ids.dedup();
        diagnostics
            .sort_by(|left, right| (&left.code, &left.message).cmp(&(&right.code, &right.message)));
        let snapshot = PromptCommandProviderSnapshot {
            provider: self.identity(),
            sources,
            commands,
            unavailable_command_ids,
            diagnostics,
        };
        snapshot.validate().map_err(|error| {
            ExternalSourceProviderError::new(
                "claude.command.snapshot_invalid",
                error.to_string(),
                false,
            )
        })?;
        Ok(snapshot)
    }

    fn expand(
        &self,
        command: &PromptCommandDefinition,
        arguments: &str,
    ) -> Result<ExpandedPromptCommand, ExternalSourceProviderError> {
        if command.id.source.provider_id.as_str() != PROVIDER_ID {
            return Err(ExternalSourceProviderError::new(
                "claude.command.identity_mismatch",
                "command is not owned by the Claude Code command provider",
                false,
            ));
        }
        match &command.availability {
            PromptCommandAvailability::Available => Ok(ExpandedPromptCommand {
                content: expand_template(&command.template, arguments),
            }),
            PromptCommandAvailability::Restricted { reason, .. }
            | PromptCommandAvailability::Invalid { reason } => {
                Err(ExternalSourceProviderError::new(
                    "claude.command.restricted",
                    reason.clone(),
                    false,
                ))
            }
            _ => Err(ExternalSourceProviderError::new(
                "claude.command.availability_unknown",
                "command availability is not supported by this adapter version",
                false,
            )),
        }
    }

    fn resolve_commands(
        &self,
        snapshot: &PromptCommandProviderSnapshot,
        enabled_sources: &BTreeSet<SourceKey>,
    ) -> Result<Vec<PromptCommandDefinition>, ExternalSourceProviderError> {
        let mut effective = BTreeMap::<String, Option<PromptCommandDefinition>>::new();
        for source in snapshot
            .sources
            .iter()
            .filter(|source| enabled_sources.contains(&source.key))
        {
            for command in snapshot
                .commands
                .iter()
                .filter(|command| command.id.source == source.key)
            {
                effective.insert(command.name.to_ascii_lowercase(), Some(command.clone()));
            }
            for unavailable in snapshot
                .unavailable_command_ids
                .iter()
                .filter(|command| command.source == source.key)
            {
                let retained = snapshot
                    .commands
                    .iter()
                    .any(|command| command.id == *unavailable);
                if !retained {
                    effective.insert(unavailable.local_id.as_str().to_ascii_lowercase(), None);
                }
            }
        }
        Ok(effective.into_values().flatten().collect())
    }

    fn watch_roots(&self, context: &ExternalSourceContext) -> Vec<ExternalWatchRoot> {
        let mut roots = BTreeMap::new();
        roots.insert(self.options.user_claude_dir.clone(), true);
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
struct CommandLayer {
    claude_dir: PathBuf,
    commands_dir: PathBuf,
    scope: ExternalSourceScope,
    display_name: String,
}

#[derive(Debug)]
struct ParsedLayer {
    commands: Vec<ClaudeCommandInput>,
    unavailable_names: Vec<String>,
    diagnostics: Vec<ExternalSourceDiagnostic>,
    content_version: String,
}

#[derive(Debug)]
struct ClaudeCommandInput {
    name: String,
    description: String,
    template: String,
    unsupported_fields: Vec<String>,
}

fn push_layer(
    layers: &mut Vec<CommandLayer>,
    claude_dir: PathBuf,
    scope: ExternalSourceScope,
    display_name: &str,
) {
    layers.push(CommandLayer {
        commands_dir: claude_dir.join("commands"),
        claude_dir,
        scope,
        display_name: display_name.to_string(),
    });
}

fn deduplicate_layers(layers: Vec<CommandLayer>) -> Vec<CommandLayer> {
    let mut seen = BTreeSet::new();
    layers
        .into_iter()
        .filter(|layer| {
            let path = dunce::canonicalize(&layer.claude_dir)
                .unwrap_or_else(|_| normalize_path_lexically(&layer.claude_dir));
            seen.insert(path)
        })
        .collect()
}

fn collect_effective_skill_names(
    layers: &[CommandLayer],
) -> Result<BTreeSet<String>, ExternalSourceProviderError> {
    let mut names = BTreeSet::new();
    for layer in layers {
        let root = layer.claude_dir.join("skills");
        let metadata = match fs::symlink_metadata(&root) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(ExternalSourceProviderError::new(
                    "claude.command.skill_index_unreadable",
                    format!("Failed to inspect a Claude Code Skill directory: {error}"),
                    true,
                ));
            }
        };
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            continue;
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(&root).map_err(|error| {
            ExternalSourceProviderError::new(
                "claude.command.skill_index_unreadable",
                format!("Failed to enumerate a Claude Code Skill directory: {error}"),
                true,
            )
        })? {
            let entry = entry.map_err(|error| {
                ExternalSourceProviderError::new(
                    "claude.command.skill_index_unreadable",
                    format!("Failed to read a Claude Code Skill directory entry: {error}"),
                    true,
                )
            })?;
            if entries.len() == MAX_COMMAND_FILES {
                return Err(ExternalSourceProviderError::new(
                    "claude.command.skill_index_limit_exceeded",
                    format!(
                        "Claude Code Skill discovery exceeds the {MAX_COMMAND_FILES}-entry safety limit"
                    ),
                    false,
                ));
            }
            entries.push(entry);
        }
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            let path = entry.path();
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) && path.join("SKILL.md").is_file()
            {
                if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                    names.insert(name.to_ascii_lowercase());
                }
            }
        }
    }
    Ok(names)
}

fn should_inspect_directory(path: &Path) -> Result<bool, ExternalSourceProviderError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.is_dir() && !metadata.file_type().is_symlink()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(ExternalSourceProviderError::new(
            "claude.command.directory_unreadable",
            format!("Failed to inspect a Claude Code command directory: {error}"),
            true,
        )),
    }
}

fn parse_layer(
    layer: &CommandLayer,
    source: &SourceKey,
) -> Result<ParsedLayer, ExternalSourceProviderError> {
    let mut files = Vec::new();
    collect_markdown_files(&layer.commands_dir, &mut files)?;
    files.sort();
    let mut version_hasher = Sha256::new();
    let mut by_name = BTreeMap::<String, Vec<(String, ClaudeCommandInput)>>::new();
    let mut unavailable_names = BTreeSet::new();
    let mut diagnostics = Vec::new();
    for path in files {
        let name = command_name(&layer.commands_dir, &path).ok_or_else(|| {
            ExternalSourceProviderError::new(
                "claude.command.name_invalid",
                "Claude Code command path cannot form a valid command name",
                false,
            )
        })?;
        let content = match read_bounded_text(&path, MAX_COMMAND_FILE_BYTES) {
            Ok(BoundedTextRead::Content(content)) => content,
            Ok(BoundedTextRead::TooLarge) => {
                unavailable_names.insert(name.clone());
                diagnostics.push(command_warning(
                    "claude.command.source_too_large",
                    "Claude Code command exceeds the 256 KiB compatibility limit",
                    Some(source.clone()),
                ));
                continue;
            }
            Ok(BoundedTextRead::InvalidUtf8) => {
                unavailable_names.insert(name.clone());
                diagnostics.push(command_warning(
                    "claude.command.source_invalid_utf8",
                    "Claude Code command is not valid UTF-8",
                    Some(source.clone()),
                ));
                continue;
            }
            Err(error) => {
                unavailable_names.insert(name.clone());
                diagnostics.push(command_warning(
                    "claude.command.source_unreadable",
                    format!("Failed to read a Claude Code command file: {error}"),
                    Some(source.clone()),
                ));
                continue;
            }
        };
        version_hasher.update(path.to_string_lossy().as_bytes());
        version_hasher.update([0]);
        version_hasher.update(content.as_bytes());
        version_hasher.update([0]);
        match parse_markdown_command(&name, &content) {
            Ok(input) => by_name
                .entry(name.to_ascii_lowercase())
                .or_default()
                .push((name, input)),
            Err(message) => {
                unavailable_names.insert(name);
                diagnostics.push(command_warning(
                    "claude.command.markdown_invalid",
                    message,
                    Some(source.clone()),
                ));
            }
        }
    }

    let mut commands = Vec::new();
    for (normalized, mut entries) in by_name {
        if entries.len() > 1 {
            unavailable_names.insert(normalized.clone());
            diagnostics.push(command_warning(
                "claude.command.duplicate_name",
                format!(
                    "Claude Code command name '{normalized}' is duplicated in one source layer"
                ),
                Some(source.clone()),
            ));
            continue;
        }
        commands.push(entries.remove(0).1);
    }
    commands.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(ParsedLayer {
        commands,
        unavailable_names: unavailable_names.into_iter().collect(),
        diagnostics,
        content_version: format!("sha256:{}", hex::encode(version_hasher.finalize())),
    })
}

fn collect_markdown_files(
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), ExternalSourceProviderError> {
    *files = collect_bounded_regular_files(
        directory,
        BoundedDirectoryWalkLimits::for_file_limit(MAX_COMMAND_FILES),
        |path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        },
    )
    .map_err(|error| match error {
        BoundedDirectoryWalkError::LimitExceeded(_) => ExternalSourceProviderError::new(
            "claude.command.file_limit",
            format!(
                "Claude Code command directory traversal exceeds the bounded {MAX_COMMAND_FILES}-file budget"
            ),
            false,
        ),
        BoundedDirectoryWalkError::Io(error) => ExternalSourceProviderError::new(
            "claude.command.directory_unreadable",
            format!("Failed to enumerate a Claude Code command directory: {error}"),
            true,
        ),
    })?;
    Ok(())
}

fn command_name(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let mut name = relative.to_string_lossy().replace('\\', "/");
    if name.to_ascii_lowercase().ends_with(".md") {
        name.truncate(name.len() - 3);
    }
    let parts = name.split('/').collect::<Vec<_>>();
    if name.is_empty()
        || name.len() > 255
        || parts.iter().any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
    {
        return None;
    }
    Some(parts.join(":"))
}

fn parse_markdown_command(name: &str, content: &str) -> Result<ClaudeCommandInput, String> {
    let (metadata, body) = if content.starts_with("---\n") || content.starts_with("---\r\n") {
        let (metadata, body) = FrontMatterMarkdown::load_str(content)
            .map_err(|error| format!("Failed to parse Claude Code command Markdown: {error}"))?;
        (Some(metadata), body)
    } else {
        (None, content.to_string())
    };
    let template = body.trim().to_string();
    if template.is_empty() {
        return Err("Claude Code command template is empty".to_string());
    }
    let mut description = String::new();
    let mut unsupported_fields = Vec::new();
    if let Some(metadata) = metadata {
        let mapping = metadata
            .as_mapping()
            .ok_or_else(|| "Claude Code command front matter must be an object".to_string())?;
        for (key, value) in mapping {
            let Some(key) = key.as_str() else {
                return Err("Claude Code command front matter keys must be strings".to_string());
            };
            match key {
                "description" => {
                    description = value
                        .as_str()
                        .ok_or_else(|| {
                            "Claude Code command description must be a string".to_string()
                        })?
                        .to_string();
                }
                // Display-only in Claude Code. It is intentionally not part of
                // BitFun's executable behavior version.
                "argument-hint" => {
                    if value.as_str().is_none() {
                        return Err(
                            "Claude Code command argument-hint must be a string".to_string()
                        );
                    }
                }
                other => unsupported_fields.push(other.to_string()),
            }
        }
    }
    Ok(ClaudeCommandInput {
        name: name.to_string(),
        description,
        template,
        unsupported_fields,
    })
}

fn command_definition(
    source: SourceKey,
    input: ClaudeCommandInput,
) -> Result<PromptCommandDefinition, ExternalSourceProviderError> {
    let mut required_capabilities = Vec::new();
    if shell_regex().is_match(&input.template) {
        required_capabilities.push("command.shell".to_string());
    }
    if file_regex().is_match(&input.template) {
        required_capabilities.push("command.file_reference".to_string());
    }
    if dynamic_variable_regex().is_match(&input.template) {
        required_capabilities.push("command.dynamic_variable".to_string());
    }
    for field in input.unsupported_fields {
        let capability = match field.as_str() {
            "model" => "command.model".to_string(),
            "allowed-tools" => "command.allowed_tools".to_string(),
            "disallowed-tools" => "command.disallowed_tools".to_string(),
            "context" | "fork" => "command.context".to_string(),
            "hooks" => "command.hooks".to_string(),
            "agent" => "command.agent".to_string(),
            "effort" => "command.effort".to_string(),
            "background" => "command.background".to_string(),
            other => format!("command.field.{other}"),
        };
        required_capabilities.push(capability);
    }
    required_capabilities.sort();
    required_capabilities.dedup();
    let availability = if required_capabilities.is_empty() {
        PromptCommandAvailability::Available
    } else {
        PromptCommandAvailability::Restricted {
            reason: format!(
                "Claude Code command requires unsupported behavior: {}",
                required_capabilities.join(", ")
            ),
            required_capabilities: required_capabilities.clone(),
        }
    };
    let availability_label = serde_json::to_string(&availability).unwrap_or_default();
    let content_version = digest([
        input.name.as_str(),
        input.template.as_str(),
        availability_label.as_str(),
    ]);
    let definition = PromptCommandDefinition {
        id: SourceQualifiedCommandId::new(source, input.name.clone()).map_err(|error| {
            ExternalSourceProviderError::new("claude.command.id_invalid", error.to_string(), false)
        })?,
        name: input.name,
        description: input.description,
        template: input.template,
        availability,
        content_version: format!("sha256:{content_version}"),
    };
    definition.validate().map_err(|error| {
        ExternalSourceProviderError::new(
            "claude.command.definition_invalid",
            error.to_string(),
            false,
        )
    })?;
    Ok(definition)
}

fn expand_template(template: &str, arguments: &str) -> String {
    let args = argument_regex()
        .find_iter(arguments)
        .map(|item| {
            let value = item.as_str();
            if value.len() >= 2
                && ((value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\'')))
            {
                value[1..value.len() - 1].to_string()
            } else {
                value.to_string()
            }
        })
        .collect::<Vec<_>>();
    let with_positions =
        placeholder_regex().replace_all(template, |capture: &regex::Captures<'_>| {
            let position = capture
                .get(1)
                .or_else(|| capture.get(2))
                .and_then(|value| value.as_str().parse::<usize>().ok())
                .unwrap_or(usize::MAX);
            args.get(position).cloned().unwrap_or_default()
        });
    let uses_arguments = template.contains("$ARGUMENTS");
    let uses_positions = placeholder_regex().is_match(template);
    let mut expanded = with_positions.replace("$ARGUMENTS", arguments);
    if !uses_arguments && !uses_positions && !arguments.trim().is_empty() {
        expanded.push_str("\n\nARGUMENTS: ");
        expanded.push_str(arguments);
    }
    expanded.trim().to_string()
}

fn argument_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"(?:\[Image\s+\d+\]|"[^"]*"|'[^']*'|[^\s"']+)"#)
            .expect("static Claude Code argument regex compiles")
    })
}

fn placeholder_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"\$(?:ARGUMENTS\[(\d+)\]|(\d+))").expect("static placeholder regex compiles")
    })
}

fn shell_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"!`[^`]+`").expect("static shell regex compiles"))
}

fn file_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?:^|[^\w`])@(\.?[^\s`,.]*(?:\.[^\s`,.]+)*)")
            .expect("static file regex compiles")
    })
}

fn dynamic_variable_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"\$\{(?:CLAUDE_SESSION_ID|CLAUDE_EFFORT|CLAUDE_SKILL_DIR|CLAUDE_PROJECT_DIR)\}")
            .expect("static Claude Code dynamic variable regex compiles")
    })
}

fn source_key(layer: &CommandLayer) -> SourceKey {
    let identity = dunce::canonicalize(&layer.commands_dir)
        .unwrap_or_else(|_| normalize_path_lexically(&layer.commands_dir));
    SourceKey::new(
        PROVIDER_ID,
        format!(
            "claude_command_directory-{}",
            &digest([identity.to_string_lossy().as_ref()])[..24]
        ),
    )
    .expect("hashed Claude Code command source id is valid")
}

fn command_warning(
    code: impl Into<String>,
    message: impl Into<String>,
    source: Option<SourceKey>,
) -> ExternalSourceDiagnostic {
    ExternalSourceDiagnostic::warning(code, message, source)
        .with_asset_kind(ExternalSourceAssetKind::Command)
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
