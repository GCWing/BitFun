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
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const PROVIDER_ID: &str = "opencode.commands";
const ECOSYSTEM_ID: &str = "opencode";
const MAX_COMMAND_FILES: usize = 2048;
const MAX_COMMAND_FILE_BYTES: usize = 256 * 1024;
const MAX_COMMAND_TEMPLATE_BYTES: usize = 8 * 1024 * 1024;
const MAX_CONFIG_FILE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct OpenCodeCommandProviderOptions {
    pub user_config_dir: PathBuf,
    pub legacy_user_config_dir: Option<PathBuf>,
    pub explicit_config_file: Option<PathBuf>,
    pub explicit_config_dir: Option<PathBuf>,
    pub project_config_enabled: bool,
}

impl OpenCodeCommandProviderOptions {
    pub fn from_environment() -> Self {
        let home = dirs::home_dir();
        let user_config_dir = opencode_user_config_dir(
            std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            home.clone(),
        );
        let legacy_user_config_dir = home.map(|home| home.join(".opencode"));
        Self {
            user_config_dir,
            legacy_user_config_dir,
            explicit_config_file: std::env::var_os("OPENCODE_CONFIG").map(PathBuf::from),
            explicit_config_dir: std::env::var_os("OPENCODE_CONFIG_DIR").map(PathBuf::from),
            project_config_enabled: !environment_truthy("OPENCODE_DISABLE_PROJECT_CONFIG"),
        }
    }
}

impl Default for OpenCodeCommandProviderOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

pub struct OpenCodeCommandProvider {
    options: OpenCodeCommandProviderOptions,
}

impl OpenCodeCommandProvider {
    pub fn new(options: OpenCodeCommandProviderOptions) -> Self {
        Self { options }
    }

    fn discover_layers(&self, context: &ExternalSourceContext) -> Vec<SourceLayer> {
        let mut layers = Vec::new();
        // Phase 1: global JSON configuration.
        push_config_file_layer(
            &mut layers,
            &self.options.user_config_dir.join("config.json"),
            ExternalSourceScope::UserGlobal,
            "OpenCode user configuration",
        );
        push_config_directory_layers(
            &mut layers,
            &self.options.user_config_dir,
            ExternalSourceScope::UserGlobal,
            "OpenCode user configuration",
        );
        // Phase 2: OPENCODE_CONFIG.
        if let Some(path) = &self.options.explicit_config_file {
            push_config_file_layer(
                &mut layers,
                path,
                ExternalSourceScope::UserGlobal,
                "OpenCode OPENCODE_CONFIG",
            );
        }
        // Phase 3: project JSON configuration, root first.
        if self.options.project_config_enabled {
            if let Some(workspace_root) = &context.workspace_root {
                let project_root = find_project_root(workspace_root);
                for directory in directories_between(&project_root, workspace_root) {
                    push_config_directory_layers(
                        &mut layers,
                        &directory,
                        ExternalSourceScope::Project,
                        "OpenCode project configuration",
                    );
                }
            }
        }
        // Phase 4: global command directories.
        push_command_directory_layer(
            &mut layers,
            &self.options.user_config_dir,
            ExternalSourceScope::UserGlobal,
            "OpenCode user command directory",
        );
        // Phase 5: project .opencode directories, nearest first. Since later
        // values win, the outer project directory wins a same-name tie.
        if self.options.project_config_enabled {
            if let Some(workspace_root) = &context.workspace_root {
                let project_root = find_project_root(workspace_root);
                for directory in directories_between(&project_root, workspace_root)
                    .into_iter()
                    .rev()
                {
                    push_directory_layers(
                        &mut layers,
                        &directory.join(".opencode"),
                        ExternalSourceScope::Project,
                        "OpenCode project command directory",
                    );
                }
            }
        }
        // Phase 6: ~/.opencode compatibility directory.
        if let Some(legacy) = &self.options.legacy_user_config_dir {
            if legacy != &self.options.user_config_dir {
                push_directory_layers(
                    &mut layers,
                    legacy,
                    ExternalSourceScope::UserGlobal,
                    "OpenCode legacy user configuration",
                );
            }
        }
        // Phase 7: OPENCODE_CONFIG_DIR.
        if let Some(directory) = &self.options.explicit_config_dir {
            push_directory_layers(
                &mut layers,
                directory,
                ExternalSourceScope::WorkspaceLocal,
                "OpenCode OPENCODE_CONFIG_DIR",
            );
        }
        deduplicate_layers_keep_last(layers)
    }
}

impl Default for OpenCodeCommandProvider {
    fn default() -> Self {
        Self::new(OpenCodeCommandProviderOptions::default())
    }
}

impl PromptCommandSourceProvider for OpenCodeCommandProvider {
    fn identity(&self) -> PromptCommandProviderIdentity {
        PromptCommandProviderIdentity::new(PROVIDER_ID, ECOSYSTEM_ID, "OpenCode")
            .expect("static OpenCode provider identity must be valid")
    }

    fn discover(
        &self,
        context: &ExternalSourceContext,
    ) -> Result<PromptCommandProviderSnapshot, ExternalSourceProviderError> {
        if context
            .workspace_root
            .as_ref()
            .is_some_and(|workspace_root| !workspace_root.is_absolute())
        {
            return Err(ExternalSourceProviderError::new(
                "opencode.command.workspace_invalid",
                "workspace root must be absolute",
                false,
            ));
        }

        let mut sources = Vec::new();
        let mut diagnostics = Vec::new();
        let mut command_candidates = Vec::new();
        let mut unavailable_command_ids = Vec::new();
        let mut provider_template_bytes = 0usize;

        for layer in self.discover_layers(context) {
            let parsed = match &layer.kind {
                SourceLayerKind::ConfigFile(path) => parse_config_file(path),
                SourceLayerKind::CommandDirectory(path) => parse_command_directory(path),
            };
            let source_key = source_key(&layer);
            let ParsedLayer {
                commands,
                unavailable_command_names,
                diagnostics: parsed_diagnostics,
                content_version,
                mut fatal,
            } = parsed;
            let mut layer_diagnostics = parsed_diagnostics
                .into_iter()
                .map(|diagnostic| ExternalSourceDiagnostic {
                    source: Some(source_key.clone()),
                    ..diagnostic
                })
                .collect::<Vec<_>>();
            let layer_template_bytes = commands
                .values()
                .map(|command| command.template.len())
                .sum::<usize>();
            if !fatal
                && provider_template_bytes.saturating_add(layer_template_bytes)
                    > MAX_COMMAND_TEMPLATE_BYTES
            {
                fatal = true;
                layer_diagnostics.push(ExternalSourceDiagnostic::warning(
                    "opencode.command.provider_template_bytes_limit",
                    "OpenCode command templates exceed the 8 MiB provider limit",
                    Some(source_key.clone()),
                ));
            } else if !fatal {
                provider_template_bytes += layer_template_bytes;
            }
            let mut has_restricted_commands = false;
            if !fatal {
                unavailable_command_ids.extend(unavailable_command_names.into_iter().filter_map(
                    |name| SourceQualifiedCommandId::new(source_key.clone(), name).ok(),
                ));
                for (name, input) in commands {
                    match command_definition(source_key.clone(), name.clone(), input) {
                        Ok(definition) => {
                            has_restricted_commands |= !matches!(
                                definition.availability,
                                PromptCommandAvailability::Available
                            );
                            command_candidates.push(definition);
                        }
                        Err(error) => {
                            if let Ok(command_id) =
                                SourceQualifiedCommandId::new(source_key.clone(), name)
                            {
                                unavailable_command_ids.push(command_id);
                            }
                            layer_diagnostics.push(ExternalSourceDiagnostic::warning(
                                error.code,
                                error.message,
                                Some(source_key.clone()),
                            ));
                        }
                    }
                }
            }
            let source_health = if fatal {
                ExternalSourceHealth::Unavailable
            } else if !layer_diagnostics.is_empty() {
                ExternalSourceHealth::Degraded
            } else if has_restricted_commands {
                ExternalSourceHealth::Partial
            } else {
                ExternalSourceHealth::Available
            };
            diagnostics.extend(layer_diagnostics.clone());
            sources.push(ExternalSourceRecord {
                key: source_key.clone(),
                ecosystem_id: EcosystemId::new(ECOSYSTEM_ID)
                    .expect("static ecosystem id must be valid"),
                display_name: layer.display_name,
                source_kind: layer.source_kind.to_string(),
                scope: layer.scope,
                location: layer.location.to_string_lossy().to_string(),
                execution_domain_id: context.execution_domain_id.clone(),
                health: source_health,
                content_version,
                diagnostics: layer_diagnostics,
            });
        }

        for diagnostic in &mut diagnostics {
            diagnostic.asset_kind = ExternalSourceAssetKind::Command;
        }
        for source in &mut sources {
            for diagnostic in &mut source.diagnostics {
                diagnostic.asset_kind = ExternalSourceAssetKind::Command;
            }
        }
        Ok(PromptCommandProviderSnapshot {
            provider: self.identity(),
            sources,
            commands: command_candidates,
            unavailable_command_ids,
            diagnostics,
        })
    }

    fn expand(
        &self,
        command: &PromptCommandDefinition,
        arguments: &str,
    ) -> Result<ExpandedPromptCommand, ExternalSourceProviderError> {
        if command.id.source.provider_id.as_str() != PROVIDER_ID {
            return Err(ExternalSourceProviderError::new(
                "opencode.command.identity_mismatch",
                "command is not owned by the OpenCode command provider",
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
                    "opencode.command.restricted",
                    reason.clone(),
                    false,
                ))
            }
            _ => Err(ExternalSourceProviderError::new(
                "opencode.command.availability_unknown",
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
                // Discovery preserves OpenCode's low-to-high source order. A
                // later candidate replaces an earlier same-name definition.
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
                let project_root = find_project_root(workspace_root);
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

#[derive(Debug)]
struct SourceLayer {
    kind: SourceLayerKind,
    location: PathBuf,
    scope: ExternalSourceScope,
    display_name: String,
    source_kind: &'static str,
}

#[derive(Debug)]
enum SourceLayerKind {
    ConfigFile(PathBuf),
    CommandDirectory(PathBuf),
}

fn push_directory_layers(
    layers: &mut Vec<SourceLayer>,
    directory: &Path,
    scope: ExternalSourceScope,
    display_name: &str,
) {
    push_config_directory_layers(layers, directory, scope, display_name);
    push_command_directory_layer(layers, directory, scope, display_name);
}

fn push_config_directory_layers(
    layers: &mut Vec<SourceLayer>,
    directory: &Path,
    scope: ExternalSourceScope,
    display_name: &str,
) {
    for name in ["opencode.json", "opencode.jsonc"] {
        push_config_file_layer(layers, &directory.join(name), scope, display_name);
    }
}

fn push_command_directory_layer(
    layers: &mut Vec<SourceLayer>,
    directory: &Path,
    scope: ExternalSourceScope,
    display_name: &str,
) {
    let command_roots = [directory.join("command"), directory.join("commands")];
    if command_roots
        .iter()
        .any(|path| match fs::symlink_metadata(path) {
            Ok(_) => true,
            Err(error) => error.kind() != std::io::ErrorKind::NotFound,
        })
    {
        layers.push(SourceLayer {
            kind: SourceLayerKind::CommandDirectory(directory.to_path_buf()),
            location: directory.to_path_buf(),
            scope,
            display_name: display_name.to_string(),
            source_kind: "opencode_command_directory",
        });
    }
}

fn push_config_file_layer(
    layers: &mut Vec<SourceLayer>,
    path: &Path,
    scope: ExternalSourceScope,
    display_name: &str,
) {
    if path.is_file() {
        layers.push(SourceLayer {
            kind: SourceLayerKind::ConfigFile(path.to_path_buf()),
            location: path.to_path_buf(),
            scope,
            display_name: display_name.to_string(),
            source_kind: "opencode_config",
        });
    }
}

fn source_key(layer: &SourceLayer) -> SourceKey {
    let mut hasher = Sha256::new();
    hasher.update(layer.source_kind.as_bytes());
    hasher.update([0]);
    let identity_path = dunce::canonicalize(&layer.location)
        .unwrap_or_else(|_| normalize_path_lexically(&layer.location));
    hasher.update(identity_path.to_string_lossy().as_bytes());
    let digest = hex::encode(hasher.finalize());
    SourceKey::new(
        PROVIDER_ID,
        format!("{}-{}", layer.source_kind, &digest[..24]),
    )
    .expect("hashed OpenCode source id must be valid")
}

fn deduplicate_layers_keep_last(layers: Vec<SourceLayer>) -> Vec<SourceLayer> {
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

#[derive(Debug, Default, Deserialize)]
struct OpenCodeConfigDocument {
    #[serde(default, rename = "command")]
    commands: BTreeMap<String, OpenCodeCommandInput>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct OpenCodeCommandInput {
    template: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    variant: Option<String>,
    #[serde(default)]
    subtask: Option<bool>,
}

struct ParsedLayer {
    commands: BTreeMap<String, OpenCodeCommandInput>,
    unavailable_command_names: BTreeSet<String>,
    diagnostics: Vec<ExternalSourceDiagnostic>,
    content_version: String,
    fatal: bool,
}

fn parse_config_file(path: &Path) -> ParsedLayer {
    match read_bounded_text(path, MAX_CONFIG_FILE_BYTES) {
        Ok(BoundedTextRead::Content(content)) => {
            let content_version = content_version([(path, content.as_bytes())]);
            match parse_config_document(&content) {
                Ok(document) => ParsedLayer {
                    commands: document.commands,
                    unavailable_command_names: BTreeSet::new(),
                    diagnostics: Vec::new(),
                    content_version,
                    fatal: false,
                },
                Err(error) => ParsedLayer {
                    commands: BTreeMap::new(),
                    unavailable_command_names: BTreeSet::new(),
                    diagnostics: vec![ExternalSourceDiagnostic::error(
                        "opencode.command.config_invalid",
                        format!("Failed to parse OpenCode command config: {error}"),
                        None,
                    )],
                    content_version,
                    fatal: true,
                },
            }
        }
        Ok(BoundedTextRead::TooLarge) => ParsedLayer {
            commands: BTreeMap::new(),
            unavailable_command_names: BTreeSet::new(),
            diagnostics: vec![ExternalSourceDiagnostic::error(
                "opencode.command.config_too_large",
                "OpenCode config exceeds the 1 MiB compatibility limit",
                None,
            )],
            content_version: "too-large".to_string(),
            fatal: true,
        },
        Ok(BoundedTextRead::InvalidUtf8) => ParsedLayer {
            commands: BTreeMap::new(),
            unavailable_command_names: BTreeSet::new(),
            diagnostics: vec![ExternalSourceDiagnostic::error(
                "opencode.command.config_invalid_utf8",
                "OpenCode command config must be valid UTF-8",
                None,
            )],
            content_version: "invalid-utf8".to_string(),
            fatal: true,
        },
        Err(error) => ParsedLayer {
            commands: BTreeMap::new(),
            unavailable_command_names: BTreeSet::new(),
            diagnostics: vec![ExternalSourceDiagnostic::error(
                "opencode.command.config_unreadable",
                format!("Failed to read OpenCode command config: {error}"),
                None,
            )],
            content_version: "unreadable".to_string(),
            fatal: true,
        },
    }
}

fn parse_command_directory(directory: &Path) -> ParsedLayer {
    let mut files = Vec::new();
    let mut scan_diagnostics = Vec::new();
    let mut scan_failed = false;
    for name in ["command", "commands"] {
        let root = directory.join(name);
        match fs::symlink_metadata(&root) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                scan_failed = true;
                scan_diagnostics.push(ExternalSourceDiagnostic::error(
                    "opencode.command.directory_unreadable",
                    format!("Failed to inspect an OpenCode command directory: {error}"),
                    None,
                ));
                break;
            }
            Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
                scan_failed = true;
                scan_diagnostics.push(ExternalSourceDiagnostic::error(
                    "opencode.command.directory_invalid",
                    "An OpenCode command directory path is not a regular directory",
                    None,
                ));
                break;
            }
            Ok(_) => {}
        }
        let remaining = MAX_COMMAND_FILES.saturating_sub(files.len());
        match collect_bounded_regular_files(
            &root,
            BoundedDirectoryWalkLimits::for_file_limit(remaining),
            |path| {
                path.extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
            },
        ) {
            Ok(mut discovered) => files.append(&mut discovered),
            Err(error) => {
                scan_failed = true;
                let code = if matches!(error, BoundedDirectoryWalkError::LimitExceeded(_)) {
                    "opencode.command.file_limit"
                } else {
                    "opencode.command.directory_unreadable"
                };
                scan_diagnostics.push(ExternalSourceDiagnostic::error(
                    code,
                    format!("Failed to safely scan an OpenCode command directory: {error}"),
                    None,
                ));
                break;
            }
        }
    }
    files.sort();

    let mut commands = BTreeMap::new();
    let mut unavailable_command_names = BTreeSet::new();
    let mut diagnostics = scan_diagnostics;
    let mut version_hasher = Sha256::new();
    let mut total_template_bytes = 0usize;
    let mut template_budget_exhausted = false;
    for path in &files {
        let Some(name) = command_name(directory, path) else {
            continue;
        };
        if template_budget_exhausted {
            unavailable_command_names.insert(name);
            continue;
        }
        let content = match read_bounded_text(path, MAX_COMMAND_FILE_BYTES) {
            Ok(BoundedTextRead::Content(content)) => content,
            Ok(BoundedTextRead::TooLarge) => {
                commands.remove(&name);
                unavailable_command_names.insert(name);
                diagnostics.push(ExternalSourceDiagnostic::warning(
                    "opencode.command.file_too_large",
                    "OpenCode command file exceeds the 256 KiB compatibility limit",
                    None,
                ));
                continue;
            }
            Ok(BoundedTextRead::InvalidUtf8) => {
                commands.remove(&name);
                unavailable_command_names.insert(name);
                diagnostics.push(ExternalSourceDiagnostic::warning(
                    "opencode.command.file_invalid_utf8",
                    "OpenCode command files must be valid UTF-8",
                    None,
                ));
                continue;
            }
            Err(error) => {
                commands.remove(&name);
                unavailable_command_names.insert(name);
                diagnostics.push(ExternalSourceDiagnostic::warning(
                    "opencode.command.file_unreadable",
                    format!("Failed to read command file: {error}"),
                    None,
                ));
                continue;
            }
        };
        version_hasher.update(path.to_string_lossy().as_bytes());
        version_hasher.update([0]);
        version_hasher.update(content.as_bytes());
        version_hasher.update([0]);
        if total_template_bytes.saturating_add(content.len()) > MAX_COMMAND_TEMPLATE_BYTES {
            commands.remove(&name);
            unavailable_command_names.insert(name);
            template_budget_exhausted = true;
            diagnostics.push(ExternalSourceDiagnostic::warning(
                "opencode.command.total_template_bytes_limit",
                "OpenCode command templates exceed the 8 MiB collection limit",
                None,
            ));
            continue;
        }
        total_template_bytes += content.len();
        match parse_markdown_command(&content) {
            Ok(input) => {
                unavailable_command_names.remove(&name);
                commands.insert(name, input);
            }
            Err(error) => {
                commands.remove(&name);
                unavailable_command_names.insert(name);
                diagnostics.push(ExternalSourceDiagnostic::warning(
                    "opencode.command.markdown_invalid",
                    format!("Failed to parse OpenCode command Markdown: {error}"),
                    None,
                ));
            }
        }
    }
    ParsedLayer {
        commands,
        unavailable_command_names,
        diagnostics,
        content_version: format!("sha256:{}", hex::encode(version_hasher.finalize())),
        fatal: scan_failed || template_budget_exhausted,
    }
}

fn command_name(directory: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(directory).ok()?;
    let mut components = relative.components();
    let first = components.next()?.as_os_str().to_str()?;
    if first != "command" && first != "commands" {
        return None;
    }
    let tail = components.collect::<PathBuf>();
    let mut name = tail.to_string_lossy().replace('\\', "/");
    if name.to_ascii_lowercase().ends_with(".md") {
        name.truncate(name.len() - 3);
    }
    (!name.is_empty()).then_some(name)
}

fn parse_markdown_command(content: &str) -> Result<OpenCodeCommandInput, String> {
    let (metadata, body) = if content.starts_with("---\n") || content.starts_with("---\r\n") {
        let (metadata, body) = FrontMatterMarkdown::load_str(content).or_else(|first_error| {
            let sanitized = sanitize_opencode_frontmatter(content);
            if sanitized == content {
                return Err(first_error);
            }
            FrontMatterMarkdown::load_str(&sanitized).map_err(|retry_error| {
                format!(
                    "{first_error}; OpenCode-compatible front matter retry failed: {retry_error}"
                )
            })
        })?;
        (Some(metadata), body)
    } else {
        (None, content.to_string())
    };
    let mut input = OpenCodeCommandInput {
        template: body.trim().to_string(),
        ..OpenCodeCommandInput::default()
    };
    if let Some(metadata) = metadata {
        let optional_string = |key: &str| -> Result<Option<String>, String> {
            match metadata.get(key) {
                None => Ok(None),
                Some(value) => value.as_str().map(str::to_string).map(Some).ok_or_else(|| {
                    format!("OpenCode command front matter field '{key}' must be a string")
                }),
            }
        };
        input.description = optional_string("description")?;
        input.agent = optional_string("agent")?;
        input.model = optional_string("model")?;
        input.variant = optional_string("variant")?;
        input.subtask = match metadata.get("subtask") {
            None => None,
            Some(value) => Some(value.as_bool().ok_or_else(|| {
                "OpenCode command front matter field 'subtask' must be a boolean".to_string()
            })?),
        };
    }
    if input.template.is_empty() {
        return Err("command template is empty".to_string());
    }
    Ok(input)
}

fn sanitize_opencode_frontmatter(content: &str) -> String {
    let Some(captures) = markdown_frontmatter_regex().captures(content) else {
        return content.to_string();
    };
    let Some(frontmatter) = captures.get(1) else {
        return content.to_string();
    };
    let mut changed = false;
    let sanitized = frontmatter
        .as_str()
        .lines()
        .flat_map(|line| {
            if line.trim().starts_with('#')
                || line.trim().is_empty()
                || line.chars().next().is_some_and(char::is_whitespace)
            {
                return vec![line.to_string()];
            }
            let Some(entry) = markdown_frontmatter_entry_regex().captures(line) else {
                return vec![line.to_string()];
            };
            let key = entry.get(1).map(|value| value.as_str()).unwrap_or_default();
            let value = entry
                .get(2)
                .map(|value| value.as_str().trim())
                .unwrap_or_default();
            if value.is_empty()
                || value == ">"
                || value == "|"
                || value.starts_with('"')
                || value.starts_with('\'')
                || !value.contains(':')
            {
                return vec![line.to_string()];
            }
            changed = true;
            vec![format!("{key}: |-"), format!("  {value}")]
        })
        .collect::<Vec<_>>()
        .join("\n");
    if !changed {
        return content.to_string();
    }
    let mut result = String::with_capacity(content.len() + sanitized.len());
    result.push_str(&content[..frontmatter.start()]);
    result.push_str(&sanitized);
    result.push_str(&content[frontmatter.end()..]);
    result
}

fn command_definition(
    source: SourceKey,
    name: String,
    input: OpenCodeCommandInput,
) -> Result<PromptCommandDefinition, ExternalSourceProviderError> {
    let content_version = command_content_version(&name, &input);
    let mut required_capabilities = Vec::new();
    if shell_regex().is_match(&input.template) {
        required_capabilities.push("command.shell".to_string());
    }
    if file_regex().is_match(&input.template) {
        required_capabilities.push("command.file_reference".to_string());
    }
    if input.agent.is_some() {
        required_capabilities.push("command.agent".to_string());
    }
    if input.model.is_some() {
        required_capabilities.push("command.model".to_string());
    }
    if input.variant.is_some() {
        required_capabilities.push("command.variant".to_string());
    }
    if input.subtask.is_some() {
        required_capabilities.push("command.subtask".to_string());
    }
    if config_variable_regex().is_match(&input.template) {
        required_capabilities.push("command.config_variable".to_string());
    }
    let availability = if required_capabilities.is_empty() {
        PromptCommandAvailability::Available
    } else {
        PromptCommandAvailability::Restricted {
            reason: format!(
                "OpenCode command requires capabilities not available in this release: {}",
                required_capabilities.join(", ")
            ),
            required_capabilities,
        }
    };
    let definition = PromptCommandDefinition {
        id: SourceQualifiedCommandId::new(source, name.clone()).map_err(|error| {
            ExternalSourceProviderError::new(
                "opencode.command.name_invalid",
                error.to_string(),
                false,
            )
        })?,
        name: name.clone(),
        description: input
            .description
            .unwrap_or_else(|| format!("OpenCode command /{name}")),
        template: input.template,
        availability,
        content_version,
    };
    definition.validate().map_err(|error| {
        ExternalSourceProviderError::new(
            "opencode.command.definition_invalid",
            error.to_string(),
            false,
        )
    })?;
    Ok(definition)
}

fn parse_config_document(input: &str) -> Result<OpenCodeConfigDocument, String> {
    let value = serde_json::from_str::<serde_json::Value>(&strip_jsonc(input))
        .map_err(|error| error.to_string())?;
    if value.get("commands").is_some() && value.get("command").is_none() {
        return Err("unsupported top-level 'commands'; OpenCode uses 'command'".to_string());
    }
    serde_json::from_value(value).map_err(|error| error.to_string())
}

fn command_content_version(name: &str, input: &OpenCodeCommandInput) -> String {
    let mut hasher = Sha256::new();
    for value in [
        Some(name),
        Some(input.template.as_str()),
        input.description.as_deref(),
        input.agent.as_deref(),
        input.model.as_deref(),
        input.variant.as_deref(),
    ] {
        match value {
            Some(value) => {
                hasher.update(value.len().to_le_bytes());
                hasher.update(value.as_bytes());
            }
            None => hasher.update(usize::MAX.to_le_bytes()),
        }
    }
    hasher.update([u8::from(input.subtask.unwrap_or(false))]);
    hasher.update([u8::from(input.subtask.is_some())]);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

pub(crate) fn opencode_user_config_dir(
    xdg_config_home: Option<PathBuf>,
    home: Option<PathBuf>,
) -> PathBuf {
    xdg_config_home
        .or_else(|| home.map(|home| home.join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("opencode")
}

fn environment_truthy(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .is_some_and(|value| matches!(value.to_ascii_lowercase().as_str(), "true" | "1"))
}

pub(crate) fn strip_jsonc(input: &str) -> String {
    let mut without_comments = String::with_capacity(input.len());
    let chars = input.chars().collect::<Vec<_>>();
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;
    while index < chars.len() {
        let current = chars[index];
        if in_string {
            without_comments.push(current);
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
            without_comments.push(current);
            index += 1;
            continue;
        }
        if current == '/' && chars.get(index + 1) == Some(&'/') {
            index += 2;
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            without_comments.push('\n');
            index += usize::from(index < chars.len());
            continue;
        }
        if current == '/' && chars.get(index + 1) == Some(&'*') {
            index += 2;
            while index + 1 < chars.len() && !(chars[index] == '*' && chars[index + 1] == '/') {
                if chars[index] == '\n' {
                    without_comments.push('\n');
                }
                index += 1;
            }
            index = (index + 2).min(chars.len());
            continue;
        }
        without_comments.push(current);
        index += 1;
    }

    let chars = without_comments.chars().collect::<Vec<_>>();
    let mut output = String::with_capacity(chars.len());
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
            continue;
        }
        if current == ',' {
            let mut lookahead = index + 1;
            while lookahead < chars.len() && chars[lookahead].is_whitespace() {
                lookahead += 1;
            }
            if matches!(chars.get(lookahead), Some('}') | Some(']')) {
                index += 1;
                continue;
            }
        }
        output.push(current);
        index += 1;
    }
    output
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
    let placeholders = placeholder_regex()
        .captures_iter(template)
        .filter_map(|capture| capture[1].parse::<usize>().ok())
        .collect::<Vec<_>>();
    let last = placeholders.iter().copied().max().unwrap_or(0);
    let with_positions =
        placeholder_regex().replace_all(template, |capture: &regex::Captures<'_>| {
            let position = capture[1].parse::<usize>().unwrap_or(0);
            let argument_index = position.saturating_sub(1);
            if argument_index >= args.len() {
                String::new()
            } else if position == last {
                args[argument_index..].join(" ")
            } else {
                args[argument_index].clone()
            }
        });
    let uses_arguments = template.contains("$ARGUMENTS");
    let mut expanded = with_positions.replace("$ARGUMENTS", arguments);
    if placeholders.is_empty() && !uses_arguments && !arguments.trim().is_empty() {
        expanded.push_str("\n\n");
        expanded.push_str(arguments);
    }
    expanded.trim().to_string()
}

fn argument_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"(?:\[Image\s+\d+\]|"[^"]*"|'[^']*'|[^\s"']+)"#)
            .expect("static OpenCode argument regex must compile")
    })
}

fn placeholder_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\$(\d+)").expect("static placeholder regex must compile"))
}

fn shell_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"!`[^`]+`").expect("static shell regex must compile"))
}

fn file_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?:^|[^\w`])@(\.?[^\s`,.]*(?:\.[^\s`,.]+)*)")
            .expect("static file reference regex must compile")
    })
}

fn config_variable_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX
        .get_or_init(|| Regex::new(r"\{(?:env|file):[^}]+\}").expect("valid config variable regex"))
}

fn markdown_frontmatter_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?s)^---\r?\n(.*?)\r?\n---")
            .expect("static Markdown front matter regex must compile")
    })
}

fn markdown_frontmatter_entry_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"^([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*(.*)$")
            .expect("static Markdown front matter entry regex must compile")
    })
}

fn content_version<'a>(entries: impl IntoIterator<Item = (&'a Path, &'a [u8])>) -> String {
    let mut hasher = Sha256::new();
    for (path, content) in entries {
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update(content);
        hasher.update([0]);
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
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

#[cfg(test)]
mod tests {
    use super::opencode_user_config_dir;
    use std::path::PathBuf;

    #[test]
    fn default_config_root_uses_xdg_semantics_on_every_platform() {
        assert_eq!(
            opencode_user_config_dir(None, Some(PathBuf::from("home"))),
            PathBuf::from("home/.config/opencode")
        );
        assert_eq!(
            opencode_user_config_dir(
                Some(PathBuf::from("custom-config")),
                Some(PathBuf::from("home"))
            ),
            PathBuf::from("custom-config/opencode")
        );
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
    if let Some(parent) = directory.parent() {
        add_nearest_existing_watch_root(roots, parent);
    }
    // Keep the desired root even before it exists. The host watches its nearest
    // existing parent non-recursively, then promotes this root to a recursive
    // watch after a creation event and a successful rescan.
    add_watch_root(roots, directory.to_path_buf(), true);
}
