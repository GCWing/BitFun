use bitfun_product_domains::external_sources::{
    EcosystemId, ExternalMcpDiscoveryInput, ExternalMcpProviderIdentity,
    ExternalMcpProviderSnapshot, ExternalMcpServerDefinition, ExternalMcpSourceProvider,
    ExternalMcpStaticStatus, ExternalMcpTransportKind, ExternalSourceAssetKind,
    ExternalSourceContext, ExternalSourceDiagnostic, ExternalSourceHealth,
    ExternalSourceProviderError, ExternalSourceRecord, ExternalSourceScope, ExternalWatchRoot,
    PreparedExternalMcpServer, PreparedExternalMcpTransport, SecretValue, SourceKey,
    SourceQualifiedMcpServerId,
};
use bitfun_static_hook_support::{
    read_bounded_text, redacted_executable_preview, resolve_bounded_regular_file,
    BoundedFileResolveError, BoundedTextRead,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const PROVIDER_ID: &str = "claude-code.mcp";
const ECOSYSTEM_ID: &str = "claude-code";
const MAX_CONFIG_FILE_BYTES: usize = 1024 * 1024;
const MAX_MCP_SERVERS: usize = 256;
const MAX_COMMAND_PARTS: usize = 256;
const MAX_MAP_ENTRIES: usize = 128;
const MAX_RUNTIME_TEXT_BYTES: usize = 64 * 1024;
const STDIO_FIELDS: &[&str] = &["type", "command", "args", "env", "cwd"];
const HTTP_FIELDS: &[&str] = &["type", "url", "headers"];

#[derive(Debug, Clone)]
pub struct ClaudeCodeMcpProviderOptions {
    pub user_config_file: PathBuf,
    pub project_root_override: Option<PathBuf>,
    pub project_config_enabled: bool,
}

impl ClaudeCodeMcpProviderOptions {
    pub fn from_environment() -> Self {
        let user_config_file = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".claude.json");
        Self {
            user_config_file,
            project_root_override: None,
            project_config_enabled: true,
        }
    }
}

impl Default for ClaudeCodeMcpProviderOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

pub struct ClaudeCodeMcpProvider {
    options: ClaudeCodeMcpProviderOptions,
}

impl ClaudeCodeMcpProvider {
    pub fn new(options: ClaudeCodeMcpProviderOptions) -> Self {
        Self { options }
    }

    fn project_root(&self, workspace_root: &Path) -> PathBuf {
        self.options
            .project_root_override
            .clone()
            .unwrap_or_else(|| find_project_root(workspace_root))
    }

    fn layers(&self, context: &ExternalSourceContext) -> Vec<ConfigLayer> {
        let mut layers = Vec::new();
        if should_inspect(&self.options.user_config_file) {
            layers.push(ConfigLayer {
                path: self.options.user_config_file.clone(),
                selector: LayerSelector::User,
                scope: ExternalSourceScope::UserGlobal,
                display_name: "Claude Code user MCP configuration",
            });
        }
        if self.options.project_config_enabled {
            if let Some(workspace_root) = &context.workspace_root {
                let project_root = self.project_root(workspace_root);
                let project_file = project_root.join(".mcp.json");
                if should_inspect(&project_file) {
                    layers.push(ConfigLayer {
                        path: project_file,
                        selector: LayerSelector::Project,
                        scope: ExternalSourceScope::Project,
                        display_name: "Claude Code project MCP configuration",
                    });
                }
                if should_inspect(&self.options.user_config_file) {
                    layers.push(ConfigLayer {
                        path: self.options.user_config_file.clone(),
                        selector: LayerSelector::WorkspaceLocal(project_root),
                        scope: ExternalSourceScope::WorkspaceLocal,
                        display_name: "Claude Code workspace-local MCP configuration",
                    });
                }
            }
        }
        layers
    }

    fn materialize(
        &self,
        input: &ExternalMcpDiscoveryInput,
    ) -> Result<MaterializedSnapshot, ExternalSourceProviderError> {
        if input
            .context
            .workspace_root
            .as_ref()
            .is_some_and(|root| !root.is_absolute())
        {
            return Err(provider_error(
                "workspace_invalid",
                "workspace root must be absolute",
                false,
            ));
        }

        let mut sources = Vec::new();
        let mut diagnostics = Vec::new();
        let mut winners = BTreeMap::<String, (SourceKey, Value)>::new();
        let mut documents = BTreeMap::<PathBuf, ParsedDocument>::new();

        for layer in self.layers(&input.context) {
            let key = source_key(&layer);
            let allowed_root = layer.path.parent().unwrap_or(Path::new("."));
            let resolved_path = resolve_bounded_regular_file(&layer.path, allowed_root)
                .map_err(|error| bounded_file_error(error))?;
            let document = documents
                .entry(resolved_path.clone())
                .or_insert_with(|| parse_document(&resolved_path))
                .clone();
            let mut layer_diagnostics = document
                .diagnostics
                .iter()
                .cloned()
                .map(|diagnostic| ExternalSourceDiagnostic {
                    source: Some(key.clone()),
                    ..diagnostic
                })
                .collect::<Vec<_>>();
            let extracted = if document.fatal {
                ExtractedServers::default()
            } else {
                extract_servers(&document.value, &layer.selector)
            };
            layer_diagnostics.extend(extracted.diagnostics.into_iter().map(|diagnostic| {
                ExternalSourceDiagnostic {
                    source: Some(key.clone()),
                    ..diagnostic
                }
            }));
            let health = if document.fatal || extracted.fatal {
                ExternalSourceHealth::Unavailable
            } else if layer_diagnostics.is_empty() {
                ExternalSourceHealth::Available
            } else {
                ExternalSourceHealth::Degraded
            };
            sources.push(ExternalSourceRecord {
                key: key.clone(),
                ecosystem_id: EcosystemId::new(ECOSYSTEM_ID)
                    .expect("static Claude Code ecosystem id must be valid"),
                display_name: layer.display_name.to_string(),
                source_kind: layer.selector.source_kind().to_string(),
                scope: layer.scope,
                location: layer.path.to_string_lossy().to_string(),
                execution_domain_id: input.context.execution_domain_id.clone(),
                health,
                content_version: content_version(
                    &input.revision_key,
                    &layer.path,
                    layer.selector.source_kind(),
                    document.raw.as_bytes(),
                ),
                diagnostics: layer_diagnostics.clone(),
            });
            diagnostics.append(&mut layer_diagnostics);
            if input.suppressed_sources.contains(&key) {
                continue;
            }
            if document.fatal || extracted.fatal {
                if !winners.is_empty() {
                    return Err(provider_error(
                        "overlay_invalid",
                        "A higher-priority Claude Code MCP source is invalid; lower-priority servers were not reactivated",
                        false,
                    ));
                }
                continue;
            }
            for (name, value) in extracted.servers {
                if winners.len() >= MAX_MCP_SERVERS && !winners.contains_key(&name) {
                    diagnostics.push(
                        ExternalSourceDiagnostic::warning(
                            "claude.mcp.server_limit",
                            format!(
                                "Claude Code MCP configuration exceeds the {MAX_MCP_SERVERS} server limit"
                            ),
                            Some(key.clone()),
                        )
                        .with_asset_kind(ExternalSourceAssetKind::Mcp),
                    );
                    continue;
                }
                // Claude Code's native MCP scopes replace the whole entry.
                winners.insert(name, (key.clone(), value));
            }
        }

        let mut servers = Vec::new();
        let mut prepared = BTreeMap::new();
        for (name, (source, value)) in winners {
            let materialized =
                materialize_server(&input.context, &input.revision_key, source, name, value)?;
            prepared.insert(
                materialized.definition.id.stable_key(),
                materialized.prepared,
            );
            servers.push(materialized.definition);
        }
        servers.sort_by(|left, right| left.name.cmp(&right.name));
        let snapshot = ExternalMcpProviderSnapshot {
            provider: self.identity(),
            sources,
            servers,
            diagnostics,
        };
        snapshot
            .validate()
            .map_err(|error| provider_error("snapshot_invalid", &error.to_string(), false))?;
        Ok(MaterializedSnapshot { snapshot, prepared })
    }
}

impl Default for ClaudeCodeMcpProvider {
    fn default() -> Self {
        Self::new(ClaudeCodeMcpProviderOptions::default())
    }
}

impl ExternalMcpSourceProvider for ClaudeCodeMcpProvider {
    fn identity(&self) -> ExternalMcpProviderIdentity {
        ExternalMcpProviderIdentity::new(PROVIDER_ID, ECOSYSTEM_ID, "Claude Code")
            .expect("static Claude Code MCP provider identity must be valid")
    }

    fn discover(
        &self,
        input: &ExternalMcpDiscoveryInput,
    ) -> Result<ExternalMcpProviderSnapshot, ExternalSourceProviderError> {
        self.materialize(input).map(|result| result.snapshot)
    }

    fn prepare_server(
        &self,
        input: &ExternalMcpDiscoveryInput,
        server_id: &SourceQualifiedMcpServerId,
        expected_behavior_version: &str,
    ) -> Result<PreparedExternalMcpServer, ExternalSourceProviderError> {
        if server_id.source.provider_id.as_str() != PROVIDER_ID {
            return Err(provider_error(
                "identity_mismatch",
                "MCP server is not owned by the Claude Code MCP provider",
                false,
            ));
        }
        let materialized = self.materialize(input)?;
        let definition = materialized
            .snapshot
            .servers
            .iter()
            .find(|definition| &definition.id == server_id)
            .ok_or_else(|| {
                provider_error(
                    "stale_revision",
                    "MCP server is no longer available at the requested revision",
                    true,
                )
            })?;
        if definition.behavior_version != expected_behavior_version {
            return Err(provider_error(
                "stale_revision",
                "MCP server behavior changed before activation",
                true,
            ));
        }
        if !matches!(definition.static_status, ExternalMcpStaticStatus::Ready) {
            return Err(provider_error(
                "not_activatable",
                "MCP server is unsupported or invalid",
                false,
            ));
        }
        let template = materialized
            .prepared
            .get(&server_id.stable_key())
            .cloned()
            .ok_or_else(|| {
                provider_error(
                    "preparation_missing",
                    "MCP runtime preparation is unavailable",
                    false,
                )
            })?;
        prepare_transport(template, server_id.clone(), expected_behavior_version)
    }

    fn watch_roots(&self, context: &ExternalSourceContext) -> Vec<ExternalWatchRoot> {
        let mut roots = BTreeMap::new();
        if let Some(parent) = self.options.user_config_file.parent() {
            add_nearest_existing_watch_root(&mut roots, parent);
        }
        if self.options.project_config_enabled {
            if let Some(workspace_root) = &context.workspace_root {
                add_nearest_existing_watch_root(&mut roots, &self.project_root(workspace_root));
            }
        }
        roots
            .into_iter()
            .map(|(path, recursive)| ExternalWatchRoot { path, recursive })
            .collect()
    }
}

#[derive(Clone)]
struct ParsedDocument {
    raw: String,
    value: Value,
    diagnostics: Vec<ExternalSourceDiagnostic>,
    fatal: bool,
}

#[derive(Default)]
struct ExtractedServers {
    servers: BTreeMap<String, Value>,
    diagnostics: Vec<ExternalSourceDiagnostic>,
    fatal: bool,
}

struct MaterializedSnapshot {
    snapshot: ExternalMcpProviderSnapshot,
    prepared: BTreeMap<String, PreparedTransportTemplate>,
}

#[derive(Clone)]
enum PreparedTransportTemplate {
    Local {
        command: String,
        args: Vec<String>,
        environment: BTreeMap<String, String>,
        working_directory: Option<PathBuf>,
    },
    Remote {
        url: String,
        headers: BTreeMap<String, String>,
    },
}

struct MaterializedServer {
    definition: ExternalMcpServerDefinition,
    prepared: PreparedTransportTemplate,
}

enum LayerSelector {
    User,
    Project,
    WorkspaceLocal(PathBuf),
}

impl LayerSelector {
    fn source_kind(&self) -> &'static str {
        match self {
            Self::User => "claude_mcp_user_config",
            Self::Project => "claude_mcp_project_config",
            Self::WorkspaceLocal(_) => "claude_mcp_workspace_config",
        }
    }
}

struct ConfigLayer {
    path: PathBuf,
    selector: LayerSelector,
    scope: ExternalSourceScope,
    display_name: &'static str,
}

fn parse_document(path: &Path) -> ParsedDocument {
    match read_bounded_text(path, MAX_CONFIG_FILE_BYTES) {
        Ok(BoundedTextRead::TooLarge) => ParsedDocument {
            raw: String::new(),
            value: Value::Null,
            diagnostics: vec![diagnostic_error(
                "config_too_large",
                "Claude Code MCP config exceeds the 1 MiB compatibility limit",
            )],
            fatal: true,
        },
        Ok(BoundedTextRead::InvalidUtf8) => ParsedDocument {
            raw: String::new(),
            value: Value::Null,
            diagnostics: vec![diagnostic_error(
                "config_invalid_utf8",
                "Claude Code MCP config is not valid UTF-8",
            )],
            fatal: true,
        },
        Ok(BoundedTextRead::Content(raw)) => match serde_json::from_str::<Value>(&raw) {
            Ok(value) => ParsedDocument {
                raw,
                value,
                diagnostics: Vec::new(),
                fatal: false,
            },
            Err(error) => ParsedDocument {
                raw,
                value: Value::Null,
                diagnostics: vec![diagnostic_error(
                    "config_invalid",
                    &format!("Failed to parse Claude Code MCP config: {error}"),
                )],
                fatal: true,
            },
        },
        Err(error) => ParsedDocument {
            raw: String::new(),
            value: Value::Null,
            diagnostics: vec![diagnostic_error(
                "config_unreadable",
                &format!("Failed to read Claude Code MCP config: {error}"),
            )],
            fatal: true,
        },
    }
}

fn bounded_file_error(error: BoundedFileResolveError) -> ExternalSourceProviderError {
    match error {
        BoundedFileResolveError::OutsideRoot => provider_error(
            "config_outside_root",
            "Claude Code MCP config resolves outside its allowed source root",
            false,
        ),
        BoundedFileResolveError::NotRegular => provider_error(
            "config_not_regular",
            "Claude Code MCP config is not a regular file",
            false,
        ),
        BoundedFileResolveError::Io(error) => provider_error(
            "config_unreadable",
            &format!("Failed to resolve Claude Code MCP config: {error}"),
            true,
        ),
    }
}

fn extract_servers(value: &Value, selector: &LayerSelector) -> ExtractedServers {
    let selected = match selector {
        LayerSelector::User | LayerSelector::Project => value.get("mcpServers"),
        LayerSelector::WorkspaceLocal(workspace) => value
            .get("projects")
            .and_then(Value::as_object)
            .and_then(|projects| {
                projects.iter().find_map(|(path, value)| {
                    paths_equal(Path::new(path), workspace).then_some(value)
                })
            })
            .and_then(|project| project.get("mcpServers")),
    };
    match selected {
        None => ExtractedServers::default(),
        Some(Value::Object(servers)) => ExtractedServers {
            servers: servers
                .iter()
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect(),
            ..ExtractedServers::default()
        },
        Some(_) => ExtractedServers {
            diagnostics: vec![diagnostic_error(
                "config_invalid",
                "Claude Code mcpServers must be an object",
            )],
            fatal: true,
            ..ExtractedServers::default()
        },
    }
}

fn materialize_server(
    context: &ExternalSourceContext,
    revision_key: &bitfun_product_domains::external_sources::ExternalMcpRevisionKey,
    source: SourceKey,
    name: String,
    value: Value,
) -> Result<MaterializedServer, ExternalSourceProviderError> {
    let behavior_version = behavior_version(revision_key, &name, &value);
    let id = SourceQualifiedMcpServerId::new(source.clone(), name.clone()).map_err(|error| {
        provider_error(
            "name_invalid",
            &format!("Claude Code MCP server name is invalid: {error}"),
            false,
        )
    })?;
    let Some(object) = value.as_object() else {
        return Ok(unsupported_local(
            id,
            source,
            name,
            behavior_version,
            "Claude Code MCP server must be an object",
        ));
    };
    let server_type = match object.get("type") {
        Some(Value::String(value)) => Some(value.as_str()),
        Some(_) => None,
        None if object.contains_key("command") => Some("stdio"),
        None if object.contains_key("url") => Some("http"),
        None => None,
    };
    match server_type {
        Some("stdio") => materialize_local(context, id, source, name, object, behavior_version),
        Some("http" | "streamable-http") => {
            materialize_remote(id, source, name, object, behavior_version)
        }
        Some("sse") => Ok(unsupported_remote(
            id,
            source,
            name,
            behavior_version,
            object.get("url").and_then(Value::as_str),
            "Claude Code SSE MCP transport is not supported",
        )),
        _ => Ok(unsupported_local(
            id,
            source,
            name,
            behavior_version,
            "Claude Code MCP type must be stdio or HTTP",
        )),
    }
}

fn materialize_local(
    context: &ExternalSourceContext,
    id: SourceQualifiedMcpServerId,
    source: SourceKey,
    name: String,
    object: &Map<String, Value>,
    behavior_version: String,
) -> Result<MaterializedServer, ExternalSourceProviderError> {
    let command = object
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let args = string_array(object.get("args"));
    let environment = string_map(object.get("env"));
    let mut reason = unsupported_field_reason(object, STDIO_FIELDS);
    if let Err(error) = &args {
        reason.get_or_insert(error.clone());
    }
    if let Err(error) = &environment {
        reason.get_or_insert(error.clone());
    }
    let args = args.unwrap_or_default();
    let environment = environment.unwrap_or_default();
    if command.is_empty() {
        reason.get_or_insert_with(|| "Local MCP command must not be empty".to_string());
    }
    if command.contains("${") || args.iter().any(|arg| arg.contains("${")) {
        reason.get_or_insert_with(|| {
            "Environment references are supported only in MCP environment and header values"
                .to_string()
        });
    }
    if args.len() > MAX_COMMAND_PARTS {
        reason.get_or_insert_with(|| format!("MCP args exceed the {MAX_COMMAND_PARTS} part limit"));
    }
    let cwd_text = object.get("cwd").and_then(Value::as_str);
    if cwd_text.is_some_and(|cwd| cwd.contains("${")) {
        reason.get_or_insert_with(|| {
            "Environment references are supported only in MCP environment and header values"
                .to_string()
        });
    }
    let cwd = cwd_text.map(PathBuf::from).map(|path| {
        if path.is_absolute() {
            normalize_path_lexically(&path)
        } else {
            context
                .workspace_root
                .as_ref()
                .map(|root| normalize_path_lexically(&root.join(&path)))
                .unwrap_or(path)
        }
    });
    let references = collect_environment_reference_names(environment.values());
    if let Err(error) = &references {
        reason.get_or_insert(error.clone());
    }
    let references = references.unwrap_or_default();
    enforce_runtime_size(
        command.len()
            + args.iter().map(String::len).sum::<usize>()
            + environment
                .iter()
                .map(|(key, value)| key.len() + value.len())
                .sum::<usize>(),
        &mut reason,
    );
    let status = reason
        .map(|reason| ExternalMcpStaticStatus::Unsupported { reason })
        .unwrap_or(ExternalMcpStaticStatus::Ready);
    Ok(MaterializedServer {
        definition: ExternalMcpServerDefinition {
            id,
            provenance: vec![source],
            name,
            transport: ExternalMcpTransportKind::LocalStdio,
            command_preview: Some(redacted_executable_preview(&command)),
            argument_count: args.len(),
            working_directory: cwd.as_ref().map(|path| cwd_preview(path, context)),
            environment_keys: environment.keys().cloned().collect(),
            environment_reference_names: references,
            remote_url_preview: None,
            header_names: Vec::new(),
            source_enabled: true,
            behavior_version,
            static_status: status,
        },
        prepared: PreparedTransportTemplate::Local {
            command,
            args,
            environment,
            working_directory: cwd,
        },
    })
}

fn materialize_remote(
    id: SourceQualifiedMcpServerId,
    source: SourceKey,
    name: String,
    object: &Map<String, Value>,
    behavior_version: String,
) -> Result<MaterializedServer, ExternalSourceProviderError> {
    let url = object
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let headers = string_map(object.get("headers"));
    let mut reason = unsupported_field_reason(object, HTTP_FIELDS);
    if let Err(error) = &headers {
        reason.get_or_insert(error.clone());
    }
    let headers = headers.unwrap_or_default();
    if url.contains("${") {
        reason.get_or_insert_with(|| {
            "Environment references are supported only in MCP environment and header values"
                .to_string()
        });
    }
    let preview = sanitized_https_origin(&url).unwrap_or_else(|error| {
        reason.get_or_insert(error);
        "https://unsupported.invalid/".to_string()
    });
    let references = collect_environment_reference_names(headers.values());
    if let Err(error) = &references {
        reason.get_or_insert(error.clone());
    }
    let references = references.unwrap_or_default();
    enforce_runtime_size(
        url.len()
            + headers
                .iter()
                .map(|(key, value)| key.len() + value.len())
                .sum::<usize>(),
        &mut reason,
    );
    let status = reason
        .map(|reason| ExternalMcpStaticStatus::Unsupported { reason })
        .unwrap_or(ExternalMcpStaticStatus::Ready);
    Ok(MaterializedServer {
        definition: ExternalMcpServerDefinition {
            id,
            provenance: vec![source],
            name,
            transport: ExternalMcpTransportKind::StreamableHttp,
            command_preview: None,
            argument_count: 0,
            working_directory: None,
            environment_keys: Vec::new(),
            environment_reference_names: references,
            remote_url_preview: Some(preview),
            header_names: headers.keys().cloned().collect(),
            source_enabled: true,
            behavior_version,
            static_status: status,
        },
        prepared: PreparedTransportTemplate::Remote { url, headers },
    })
}

fn unsupported_field_reason(object: &Map<String, Value>, supported: &[&str]) -> Option<String> {
    let fields = object
        .keys()
        .filter(|field| !supported.contains(&field.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    (!fields.is_empty()).then(|| {
        format!(
            "Claude Code MCP fields are not supported: {}",
            fields.join(", ")
        )
    })
}

fn cwd_preview(path: &Path, context: &ExternalSourceContext) -> String {
    context
        .workspace_root
        .as_ref()
        .and_then(|workspace| path.strip_prefix(workspace).ok())
        .map(|relative| {
            if relative.as_os_str().is_empty() {
                ".".to_string()
            } else {
                format!("./{}", relative.to_string_lossy().replace('\\', "/"))
            }
        })
        .unwrap_or_else(|| "<configured>".to_string())
}

fn unsupported_local(
    id: SourceQualifiedMcpServerId,
    source: SourceKey,
    name: String,
    behavior_version: String,
    reason: &str,
) -> MaterializedServer {
    MaterializedServer {
        definition: ExternalMcpServerDefinition {
            id,
            provenance: vec![source],
            name,
            transport: ExternalMcpTransportKind::LocalStdio,
            command_preview: Some("unsupported".to_string()),
            argument_count: 0,
            working_directory: None,
            environment_keys: Vec::new(),
            environment_reference_names: Vec::new(),
            remote_url_preview: None,
            header_names: Vec::new(),
            source_enabled: true,
            behavior_version,
            static_status: ExternalMcpStaticStatus::Unsupported {
                reason: reason.to_string(),
            },
        },
        prepared: PreparedTransportTemplate::Local {
            command: String::new(),
            args: Vec::new(),
            environment: BTreeMap::new(),
            working_directory: None,
        },
    }
}

fn unsupported_remote(
    id: SourceQualifiedMcpServerId,
    source: SourceKey,
    name: String,
    behavior_version: String,
    raw_url: Option<&str>,
    reason: &str,
) -> MaterializedServer {
    MaterializedServer {
        definition: ExternalMcpServerDefinition {
            id,
            provenance: vec![source],
            name,
            transport: ExternalMcpTransportKind::StreamableHttp,
            command_preview: None,
            argument_count: 0,
            working_directory: None,
            environment_keys: Vec::new(),
            environment_reference_names: Vec::new(),
            remote_url_preview: Some(
                raw_url
                    .and_then(|url| sanitized_https_origin(url).ok())
                    .unwrap_or_else(|| "https://unsupported.invalid/".to_string()),
            ),
            header_names: Vec::new(),
            source_enabled: true,
            behavior_version,
            static_status: ExternalMcpStaticStatus::Unsupported {
                reason: reason.to_string(),
            },
        },
        prepared: PreparedTransportTemplate::Remote {
            url: String::new(),
            headers: BTreeMap::new(),
        },
    }
}

fn prepare_transport(
    template: PreparedTransportTemplate,
    id: SourceQualifiedMcpServerId,
    behavior_version: &str,
) -> Result<PreparedExternalMcpServer, ExternalSourceProviderError> {
    let transport = match template {
        PreparedTransportTemplate::Local {
            command,
            args,
            environment,
            working_directory,
        } => PreparedExternalMcpTransport::Local {
            command,
            args,
            environment: environment
                .into_iter()
                .map(|(key, value)| {
                    expand_environment_references(&value)
                        .map(|value| (key, SecretValue::new(value)))
                })
                .collect::<Result<_, _>>()?,
            working_directory,
        },
        PreparedTransportTemplate::Remote { url, headers } => {
            let headers = headers
                .into_iter()
                .map(|(key, value)| {
                    expand_environment_references(&value)
                        .map(|value| (key, SecretValue::new(value)))
                })
                .collect::<Result<_, _>>()?;
            PreparedExternalMcpTransport::Remote {
                url,
                headers,
                oauth_enabled: false,
            }
        }
    };
    Ok(PreparedExternalMcpServer {
        id,
        behavior_version: behavior_version.to_string(),
        transport,
    })
}

fn string_array(value: Option<&Value>) -> Result<Vec<String>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| "MCP args must be an array of strings".to_string())?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| "MCP args must contain only strings".to_string())
        })
        .collect()
}

fn string_map(value: Option<&Value>) -> Result<BTreeMap<String, String>, String> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    let object = value
        .as_object()
        .ok_or_else(|| "MCP environment or headers must be an object".to_string())?;
    if object.len() > MAX_MAP_ENTRIES {
        return Err(format!(
            "MCP environment or headers exceed the {MAX_MAP_ENTRIES} entry limit"
        ));
    }
    object
        .iter()
        .map(|(key, value)| {
            value
                .as_str()
                .map(|value| (key.clone(), value.to_string()))
                .ok_or_else(|| "MCP environment and header values must be strings".to_string())
        })
        .collect()
}

fn collect_environment_reference_names<'a>(
    values: impl IntoIterator<Item = &'a String>,
) -> Result<Vec<String>, String> {
    let mut names = BTreeSet::new();
    for value in values {
        replace_environment_references(value, |name| {
            names.insert(name.to_string());
            Ok(String::new())
        })
        .map_err(|error| error.message)?;
    }
    if names.len() > MAX_MAP_ENTRIES {
        return Err(format!(
            "MCP environment references exceed the {MAX_MAP_ENTRIES} entry limit"
        ));
    }
    Ok(names.into_iter().collect())
}

fn replace_environment_references(
    value: &str,
    mut resolve: impl FnMut(&str) -> Result<String, ExternalSourceProviderError>,
) -> Result<String, ExternalSourceProviderError> {
    let mut output = String::with_capacity(value.len());
    let mut remainder = value;
    while let Some(start) = remainder.find("${") {
        output.push_str(&remainder[..start]);
        let after = &remainder[start + 2..];
        let Some(end) = after.find('}') else {
            return Err(provider_error(
                "variable_invalid",
                "Claude Code environment reference is not closed",
                false,
            ));
        };
        let name = &after[..end];
        if name.is_empty()
            || !name.bytes().enumerate().all(|(index, byte)| {
                byte == b'_'
                    || byte.is_ascii_alphanumeric() && (index > 0 || !byte.is_ascii_digit())
            })
        {
            return Err(provider_error(
                "variable_invalid",
                "Claude Code environment reference name is invalid",
                false,
            ));
        }
        output.push_str(&resolve(name)?);
        remainder = &after[end + 1..];
    }
    output.push_str(remainder);
    Ok(output)
}

fn expand_environment_references(value: &str) -> Result<String, ExternalSourceProviderError> {
    replace_environment_references(value, |name| {
        std::env::var(name).map_err(|_| {
            provider_error(
                "environment_missing",
                &format!("Required environment variable '{name}' is not available"),
                true,
            )
        })
    })
}

fn sanitized_https_origin(value: &str) -> Result<String, String> {
    let mut url = url::Url::parse(value).map_err(|_| "Remote MCP URL is invalid".to_string())?;
    if url.scheme() != "https" {
        return Err("Remote MCP URL must use HTTPS".to_string());
    }
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_path("/");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string())
}

fn provider_error(suffix: &str, message: &str, transient: bool) -> ExternalSourceProviderError {
    ExternalSourceProviderError::new(format!("claude.mcp.{suffix}"), message, transient)
}

fn diagnostic_error(suffix: &str, message: &str) -> ExternalSourceDiagnostic {
    ExternalSourceDiagnostic::error(format!("claude.mcp.{suffix}"), message, None)
        .with_asset_kind(ExternalSourceAssetKind::Mcp)
}

fn behavior_version(
    revision_key: &bitfun_product_domains::external_sources::ExternalMcpRevisionKey,
    name: &str,
    value: &Value,
) -> String {
    let encoded = serde_json::to_vec(value).unwrap_or_default();
    revision_key.opaque_revision(
        "claude.mcp.behavior.v1",
        [name.as_bytes(), encoded.as_slice()],
    )
}

fn content_version(
    revision_key: &bitfun_product_domains::external_sources::ExternalMcpRevisionKey,
    path: &Path,
    kind: &str,
    content: &[u8],
) -> String {
    revision_key.opaque_revision(
        "claude.mcp.content.v1",
        [path.to_string_lossy().as_bytes(), kind.as_bytes(), content],
    )
}

fn source_key(layer: &ConfigLayer) -> SourceKey {
    let path =
        dunce::canonicalize(&layer.path).unwrap_or_else(|_| normalize_path_lexically(&layer.path));
    let mut hasher = Sha256::new();
    hasher.update(layer.selector.source_kind().as_bytes());
    hasher.update([0]);
    hasher.update(path.to_string_lossy().as_bytes());
    SourceKey::new(
        PROVIDER_ID,
        format!("claude_mcp-{}", &hex::encode(hasher.finalize())[..24]),
    )
    .expect("hashed Claude Code MCP source id must be valid")
}

fn should_inspect(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(metadata) => metadata.is_file(),
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    }
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    let left = dunce::canonicalize(left).unwrap_or_else(|_| normalize_path_lexically(left));
    let right = dunce::canonicalize(right).unwrap_or_else(|_| normalize_path_lexically(right));
    if cfg!(windows) {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    }
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

fn enforce_runtime_size(bytes: usize, reason: &mut Option<String>) {
    if bytes > MAX_RUNTIME_TEXT_BYTES {
        reason.get_or_insert_with(|| {
            format!("MCP runtime values exceed the {MAX_RUNTIME_TEXT_BYTES} byte limit")
        });
    }
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

fn add_nearest_existing_watch_root(roots: &mut BTreeMap<PathBuf, bool>, path: &Path) {
    if let Some(path) = nearest_existing_path(path.to_path_buf()) {
        roots.entry(path).or_insert(false);
    }
}
