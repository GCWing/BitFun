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
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use toml::Value;

const PROVIDER_ID: &str = "codex.mcp";
const ECOSYSTEM_ID: &str = "codex";
const MAX_CONFIG_FILE_BYTES: usize = 1024 * 1024;
const MAX_MCP_SERVERS: usize = 256;
const MAX_COMMAND_PARTS: usize = 256;
const MAX_MAP_ENTRIES: usize = 128;
const MAX_RUNTIME_TEXT_BYTES: usize = 64 * 1024;

const SUPPORTED_FIELDS: &[&str] = &[
    "command",
    "args",
    "env",
    "env_vars",
    "cwd",
    "url",
    "bearer_token_env_var",
    "http_headers",
    "env_http_headers",
    "enabled",
    "required",
    "auth",
];

#[derive(Debug, Clone)]
pub struct CodexMcpProviderOptions {
    pub codex_home: PathBuf,
    pub project_root_override: Option<PathBuf>,
    pub project_config_enabled: bool,
}

impl CodexMcpProviderOptions {
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

impl Default for CodexMcpProviderOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

pub struct CodexMcpProvider {
    options: CodexMcpProviderOptions,
}

impl CodexMcpProvider {
    pub fn new(options: CodexMcpProviderOptions) -> Self {
        Self { options }
    }

    fn project_root(&self, workspace: &Path) -> PathBuf {
        self.options
            .project_root_override
            .clone()
            .unwrap_or_else(|| find_project_root(workspace))
    }

    fn layers(&self, context: &ExternalSourceContext) -> Vec<ConfigLayer> {
        let mut layers = Vec::new();
        push_layer(
            &mut layers,
            self.options.codex_home.join("config.toml"),
            ExternalSourceScope::UserGlobal,
            "Codex user MCP configuration",
        );
        if self.options.project_config_enabled {
            if let Some(workspace) = &context.workspace_root {
                for directory in directories_between(&self.project_root(workspace), workspace) {
                    push_layer(
                        &mut layers,
                        directory.join(".codex/config.toml"),
                        ExternalSourceScope::Project,
                        "Codex project MCP configuration",
                    );
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
            .is_some_and(|workspace| !workspace.is_absolute())
        {
            return Err(provider_error(
                "workspace_invalid",
                "workspace root must be absolute",
                false,
            ));
        }
        let mut sources = Vec::new();
        let mut diagnostics = Vec::new();
        let mut merged = BTreeMap::<String, Value>::new();
        let mut provenance = BTreeMap::<String, Vec<SourceKey>>::new();

        for layer in self.layers(&input.context) {
            let key = source_key(&layer.path);
            let allowed_root = layer.path.parent().unwrap_or(Path::new("."));
            let resolved_path = resolve_bounded_regular_file(&layer.path, allowed_root)
                .map_err(|error| bounded_file_error(error))?;
            let parsed = parse_layer(&resolved_path);
            let mut layer_diagnostics = parsed
                .diagnostics
                .into_iter()
                .map(|diagnostic| ExternalSourceDiagnostic {
                    source: Some(key.clone()),
                    ..diagnostic
                })
                .collect::<Vec<_>>();
            let health = if parsed.fatal {
                ExternalSourceHealth::Unavailable
            } else if layer_diagnostics.is_empty() {
                ExternalSourceHealth::Available
            } else {
                ExternalSourceHealth::Degraded
            };
            sources.push(ExternalSourceRecord {
                key: key.clone(),
                ecosystem_id: EcosystemId::new(ECOSYSTEM_ID)
                    .expect("static Codex ecosystem id must be valid"),
                display_name: layer.display_name.to_string(),
                source_kind: "codex_mcp_config".to_string(),
                scope: layer.scope,
                location: layer.path.to_string_lossy().to_string(),
                execution_domain_id: input.context.execution_domain_id.clone(),
                health,
                content_version: content_version(
                    &input.revision_key,
                    &layer.path,
                    parsed.raw.as_bytes(),
                ),
                diagnostics: layer_diagnostics.clone(),
            });
            diagnostics.append(&mut layer_diagnostics);
            if input.suppressed_sources.contains(&key) {
                continue;
            }
            if parsed.fatal {
                return Err(provider_error(
                    "overlay_invalid",
                    "A required Codex MCP configuration layer is invalid; no later-layer servers were activated",
                    false,
                ));
            }
            for (name, mut patch) in parsed.servers {
                if merged.len() >= MAX_MCP_SERVERS && !merged.contains_key(&name) {
                    diagnostics.push(
                        ExternalSourceDiagnostic::warning(
                            "codex.mcp.server_limit",
                            format!(
                                "Codex MCP configuration exceeds the {MAX_MCP_SERVERS} server limit"
                            ),
                            Some(key.clone()),
                        )
                        .with_asset_kind(ExternalSourceAssetKind::Mcp),
                    );
                    continue;
                }
                resolve_layer_relative_cwd(&mut patch, &layer.path);
                let current = merged
                    .entry(name.clone())
                    .or_insert_with(|| Value::Table(toml::map::Map::new()));
                deep_merge(current, patch);
                let sources = provenance.entry(name).or_default();
                if sources.last() != Some(&key) {
                    sources.push(key.clone());
                }
            }
        }

        let mut servers = Vec::new();
        let mut prepared = BTreeMap::new();
        for (name, value) in merged {
            let server_provenance = provenance.remove(&name).unwrap_or_default();
            let Some(effective_source) = server_provenance.last().cloned() else {
                continue;
            };
            let result = materialize_server(
                &input.context,
                &input.revision_key,
                effective_source,
                server_provenance,
                name,
                value,
            )?;
            diagnostics.extend(result.diagnostics);
            prepared.insert(result.definition.id.stable_key(), result.prepared);
            servers.push(result.definition);
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

impl Default for CodexMcpProvider {
    fn default() -> Self {
        Self::new(CodexMcpProviderOptions::default())
    }
}

impl ExternalMcpSourceProvider for CodexMcpProvider {
    fn identity(&self) -> ExternalMcpProviderIdentity {
        ExternalMcpProviderIdentity::new(PROVIDER_ID, ECOSYSTEM_ID, "Codex")
            .expect("static Codex MCP provider identity must be valid")
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
                "MCP server is not owned by the Codex MCP provider",
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
                "MCP server is disabled, unsupported, or invalid",
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

struct ConfigLayer {
    path: PathBuf,
    scope: ExternalSourceScope,
    display_name: &'static str,
}

struct ParsedLayer {
    raw: String,
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
        environment_refs: BTreeMap<String, String>,
        working_directory: Option<PathBuf>,
    },
    Remote {
        url: String,
        headers: BTreeMap<String, String>,
        header_refs: BTreeMap<String, String>,
        bearer_token_env_var: Option<String>,
        oauth_enabled: bool,
    },
}

struct MaterializedServer {
    definition: ExternalMcpServerDefinition,
    prepared: PreparedTransportTemplate,
    diagnostics: Vec<ExternalSourceDiagnostic>,
}

fn parse_layer(path: &Path) -> ParsedLayer {
    match read_bounded_text(path, MAX_CONFIG_FILE_BYTES) {
        Ok(BoundedTextRead::TooLarge) => ParsedLayer {
            raw: String::new(),
            servers: BTreeMap::new(),
            diagnostics: vec![mcp_error(
                "config_too_large",
                "Codex config exceeds the 1 MiB compatibility limit",
                None,
            )],
            fatal: true,
        },
        Ok(BoundedTextRead::InvalidUtf8) => ParsedLayer {
            raw: String::new(),
            servers: BTreeMap::new(),
            diagnostics: vec![mcp_error(
                "config_invalid_utf8",
                "Codex config is not valid UTF-8",
                None,
            )],
            fatal: true,
        },
        Ok(BoundedTextRead::Content(raw)) => match toml::from_str::<Value>(&raw) {
            Ok(value) => match value.get("mcp_servers") {
                None => ParsedLayer {
                    raw,
                    servers: BTreeMap::new(),
                    diagnostics: Vec::new(),
                    fatal: false,
                },
                Some(Value::Table(servers)) => ParsedLayer {
                    raw,
                    servers: servers
                        .iter()
                        .map(|(name, value)| (name.clone(), value.clone()))
                        .collect(),
                    diagnostics: Vec::new(),
                    fatal: false,
                },
                Some(_) => ParsedLayer {
                    raw,
                    servers: BTreeMap::new(),
                    diagnostics: vec![mcp_error(
                        "config_invalid",
                        "Codex mcp_servers must be a TOML table",
                        None,
                    )],
                    fatal: true,
                },
            },
            Err(error) => ParsedLayer {
                raw,
                servers: BTreeMap::new(),
                diagnostics: vec![mcp_error(
                    "config_invalid",
                    &format!("Failed to parse Codex MCP config: {error}"),
                    None,
                )],
                fatal: true,
            },
        },
        Err(error) => ParsedLayer {
            raw: String::new(),
            servers: BTreeMap::new(),
            diagnostics: vec![mcp_error(
                "config_unreadable",
                &format!("Failed to read Codex MCP config: {error}"),
                None,
            )],
            fatal: true,
        },
    }
}

fn bounded_file_error(error: BoundedFileResolveError) -> ExternalSourceProviderError {
    match error {
        BoundedFileResolveError::OutsideRoot => provider_error(
            "config_outside_root",
            "Codex MCP config resolves outside its allowed source root",
            false,
        ),
        BoundedFileResolveError::NotRegular => provider_error(
            "config_not_regular",
            "Codex MCP config is not a regular file",
            false,
        ),
        BoundedFileResolveError::Io(error) => provider_error(
            "config_unreadable",
            &format!("Failed to resolve Codex MCP config: {error}"),
            true,
        ),
    }
}

fn materialize_server(
    context: &ExternalSourceContext,
    revision_key: &bitfun_product_domains::external_sources::ExternalMcpRevisionKey,
    effective_source: SourceKey,
    provenance: Vec<SourceKey>,
    name: String,
    value: Value,
) -> Result<MaterializedServer, ExternalSourceProviderError> {
    let behavior_version = behavior_version(revision_key, &name, &value);
    let id = SourceQualifiedMcpServerId::new(effective_source.clone(), name.clone()).map_err(
        |error| {
            provider_error(
                "name_invalid",
                &format!("Codex MCP server name is invalid: {error}"),
                false,
            )
        },
    )?;
    let Some(object) = value.as_table() else {
        return Ok(unsupported_local(
            id,
            provenance,
            name,
            behavior_version,
            "Codex MCP server must be a TOML table",
        ));
    };
    let mut reasons = object
        .keys()
        .filter(|field| !SUPPORTED_FIELDS.contains(&field.as_str()))
        .map(|field| format!("Codex MCP field '{field}' is not supported"))
        .collect::<Vec<_>>();
    let enabled = match object.get("enabled") {
        None => true,
        Some(Value::Boolean(value)) => *value,
        Some(_) => {
            reasons.push("Codex MCP enabled must be a boolean".to_string());
            true
        }
    };
    let required = match object.get("required") {
        None => false,
        Some(Value::Boolean(value)) => *value,
        Some(_) => {
            reasons.push("Codex MCP required must be a boolean".to_string());
            false
        }
    };
    let diagnostics = required
        .then(|| {
            ExternalSourceDiagnostic::warning(
                "codex.mcp.required_not_imported",
                "BitFun does not adopt Codex required-startup failure semantics",
                Some(effective_source),
            )
            .with_asset_kind(ExternalSourceAssetKind::Mcp)
        })
        .into_iter()
        .collect::<Vec<_>>();
    match (object.get("command"), object.get("url")) {
        (Some(_), None) => materialize_local(
            context,
            id,
            provenance,
            name,
            object,
            enabled,
            behavior_version,
            reasons,
            diagnostics,
        ),
        (None, Some(_)) => materialize_remote(
            id,
            provenance,
            name,
            object,
            enabled,
            behavior_version,
            reasons,
            diagnostics,
        ),
        _ => {
            let mut result = unsupported_local(
                id,
                provenance,
                name,
                behavior_version,
                "Codex MCP server must define exactly one of command or url",
            );
            result.diagnostics = diagnostics;
            Ok(result)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn materialize_local(
    context: &ExternalSourceContext,
    id: SourceQualifiedMcpServerId,
    provenance: Vec<SourceKey>,
    name: String,
    object: &toml::map::Map<String, Value>,
    enabled: bool,
    behavior_version: String,
    mut reasons: Vec<String>,
    diagnostics: Vec<ExternalSourceDiagnostic>,
) -> Result<MaterializedServer, ExternalSourceProviderError> {
    for field in [
        "bearer_token_env_var",
        "http_headers",
        "env_http_headers",
        "auth",
    ] {
        if object.contains_key(field) {
            reasons.push(format!("Codex MCP field '{field}' is not valid for stdio"));
        }
    }
    let command = string_value(object.get("command"), "command", &mut reasons);
    let args = string_array(object.get("args"), "args", &mut reasons);
    if args.len() > MAX_COMMAND_PARTS {
        reasons.push(format!(
            "MCP args exceed the {MAX_COMMAND_PARTS} part limit"
        ));
    }
    let environment = string_map(object.get("env"), "env", &mut reasons);
    let environment_refs = environment_refs(object.get("env_vars"), &mut reasons);
    let cwd = string_value_optional(object.get("cwd"), "cwd", &mut reasons).map(PathBuf::from);
    let cwd = cwd.or_else(|| context.workspace_root.clone());
    enforce_size(
        command.len()
            + args.iter().map(String::len).sum::<usize>()
            + environment
                .iter()
                .map(|(key, value)| key.len() + value.len())
                .sum::<usize>(),
        &mut reasons,
    );
    let status = static_status(enabled, reasons);
    let environment_keys = environment
        .keys()
        .chain(environment_refs.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let environment_reference_names = environment_refs
        .values()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(MaterializedServer {
        definition: ExternalMcpServerDefinition {
            id,
            provenance,
            name,
            transport: ExternalMcpTransportKind::LocalStdio,
            command_preview: Some(redacted_executable_preview(&command)),
            argument_count: args.len(),
            working_directory: cwd.as_ref().map(|path| cwd_preview(path, context)),
            environment_keys,
            environment_reference_names,
            remote_url_preview: None,
            header_names: Vec::new(),
            source_enabled: enabled,
            behavior_version,
            static_status: status,
        },
        prepared: PreparedTransportTemplate::Local {
            command,
            args,
            environment,
            environment_refs,
            working_directory: cwd,
        },
        diagnostics,
    })
}

#[allow(clippy::too_many_arguments)]
fn materialize_remote(
    id: SourceQualifiedMcpServerId,
    provenance: Vec<SourceKey>,
    name: String,
    object: &toml::map::Map<String, Value>,
    enabled: bool,
    behavior_version: String,
    mut reasons: Vec<String>,
    diagnostics: Vec<ExternalSourceDiagnostic>,
) -> Result<MaterializedServer, ExternalSourceProviderError> {
    for field in ["args", "env", "env_vars", "cwd"] {
        if object.contains_key(field) {
            reasons.push(format!("Codex MCP field '{field}' is not valid for HTTP"));
        }
    }
    let url = string_value(object.get("url"), "url", &mut reasons);
    let preview = sanitized_https_origin(&url).unwrap_or_else(|error| {
        reasons.push(error);
        "https://unsupported.invalid/".to_string()
    });
    let headers = string_map(object.get("http_headers"), "http_headers", &mut reasons);
    let header_refs = string_map(
        object.get("env_http_headers"),
        "env_http_headers",
        &mut reasons,
    );
    let bearer_token_env_var = string_value_optional(
        object.get("bearer_token_env_var"),
        "bearer_token_env_var",
        &mut reasons,
    );
    if bearer_token_env_var
        .as_deref()
        .is_some_and(|name| !valid_environment_name(name))
        || header_refs
            .values()
            .any(|name| !valid_environment_name(name))
    {
        reasons.push("Codex MCP environment reference name is invalid".to_string());
    }
    let oauth_enabled = match object.get("auth") {
        None => true,
        Some(Value::String(value)) if value == "oauth" => true,
        Some(Value::String(value)) if value == "chatgpt" => {
            reasons.push("Codex ChatGPT MCP authentication is not supported".to_string());
            false
        }
        Some(Value::String(_)) => {
            reasons.push("Codex MCP auth must be 'oauth' or 'chatgpt'".to_string());
            false
        }
        Some(_) => {
            reasons.push("Codex MCP auth must be 'oauth' or 'chatgpt'".to_string());
            false
        }
    };
    enforce_size(
        url.len()
            + headers
                .iter()
                .map(|(key, value)| key.len() + value.len())
                .sum::<usize>(),
        &mut reasons,
    );
    let status = static_status(enabled, reasons);
    let mut header_names = headers
        .keys()
        .chain(header_refs.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    if bearer_token_env_var.is_some() {
        header_names.insert("Authorization".to_string());
    }
    let environment_reference_names = bearer_token_env_var
        .iter()
        .chain(header_refs.values())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(MaterializedServer {
        definition: ExternalMcpServerDefinition {
            id,
            provenance,
            name,
            transport: ExternalMcpTransportKind::StreamableHttp,
            command_preview: None,
            argument_count: 0,
            working_directory: None,
            environment_keys: Vec::new(),
            environment_reference_names,
            remote_url_preview: Some(preview),
            header_names: header_names.into_iter().collect(),
            source_enabled: enabled,
            behavior_version,
            static_status: status,
        },
        prepared: PreparedTransportTemplate::Remote {
            url,
            headers,
            header_refs,
            bearer_token_env_var,
            oauth_enabled,
        },
        diagnostics,
    })
}

fn unsupported_local(
    id: SourceQualifiedMcpServerId,
    provenance: Vec<SourceKey>,
    name: String,
    behavior_version: String,
    reason: &str,
) -> MaterializedServer {
    MaterializedServer {
        definition: ExternalMcpServerDefinition {
            id,
            provenance,
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
            environment_refs: BTreeMap::new(),
            working_directory: None,
        },
        diagnostics: Vec::new(),
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
            mut environment,
            environment_refs,
            working_directory,
        } => {
            for (key, reference) in environment_refs {
                environment.insert(key, resolve_environment(&reference)?);
            }
            let environment = environment
                .into_iter()
                .map(|(key, value)| (key, SecretValue::new(value)))
                .collect::<BTreeMap<_, _>>();
            ensure_runtime_size(
                command.len()
                    + args.iter().map(String::len).sum::<usize>()
                    + environment
                        .iter()
                        .map(|(key, value)| key.len() + value.expose().len())
                        .sum::<usize>(),
            )?;
            PreparedExternalMcpTransport::Local {
                command,
                args,
                environment,
                working_directory,
            }
        }
        PreparedTransportTemplate::Remote {
            url,
            mut headers,
            header_refs,
            bearer_token_env_var,
            oauth_enabled,
        } => {
            for (header, reference) in header_refs {
                headers.insert(header, resolve_environment(&reference)?);
            }
            if let Some(reference) = bearer_token_env_var {
                headers.insert(
                    "Authorization".to_string(),
                    format!("Bearer {}", resolve_environment(&reference)?),
                );
            }
            let headers = headers
                .into_iter()
                .map(|(key, value)| (key, SecretValue::new(value)))
                .collect::<BTreeMap<_, _>>();
            ensure_runtime_size(
                url.len()
                    + headers
                        .iter()
                        .map(|(key, value)| key.len() + value.expose().len())
                        .sum::<usize>(),
            )?;
            PreparedExternalMcpTransport::Remote {
                url,
                headers,
                oauth_enabled,
            }
        }
    };
    Ok(PreparedExternalMcpServer {
        id,
        behavior_version: behavior_version.to_string(),
        transport,
    })
}

fn string_value(value: Option<&Value>, field: &str, reasons: &mut Vec<String>) -> String {
    string_value_optional(value, field, reasons).unwrap_or_default()
}

fn string_value_optional(
    value: Option<&Value>,
    field: &str,
    reasons: &mut Vec<String>,
) -> Option<String> {
    match value {
        None => None,
        Some(Value::String(value)) if !value.is_empty() => Some(value.clone()),
        Some(_) => {
            reasons.push(format!("Codex MCP {field} must be a non-empty string"));
            None
        }
    }
}

fn string_array(value: Option<&Value>, field: &str, reasons: &mut Vec<String>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };
    let Some(values) = value.as_array() else {
        reasons.push(format!("Codex MCP {field} must be an array of strings"));
        return Vec::new();
    };
    let values = values
        .iter()
        .map(|value| value.as_str().map(str::to_string))
        .collect::<Option<Vec<_>>>();
    values.unwrap_or_else(|| {
        reasons.push(format!("Codex MCP {field} must contain only strings"));
        Vec::new()
    })
}

fn string_map(
    value: Option<&Value>,
    field: &str,
    reasons: &mut Vec<String>,
) -> BTreeMap<String, String> {
    let Some(value) = value else {
        return BTreeMap::new();
    };
    let Some(values) = value.as_table() else {
        reasons.push(format!("Codex MCP {field} must be a string map"));
        return BTreeMap::new();
    };
    if values.len() > MAX_MAP_ENTRIES {
        reasons.push(format!(
            "Codex MCP {field} exceeds the {MAX_MAP_ENTRIES} entry limit"
        ));
        return BTreeMap::new();
    }
    values
        .iter()
        .filter_map(|(key, value)| match value.as_str() {
            Some(value) => Some((key.clone(), value.to_string())),
            None => {
                reasons.push(format!("Codex MCP {field} values must be strings"));
                None
            }
        })
        .collect()
}

fn environment_refs(value: Option<&Value>, reasons: &mut Vec<String>) -> BTreeMap<String, String> {
    let Some(value) = value else {
        return BTreeMap::new();
    };
    let Some(values) = value.as_array() else {
        reasons.push("Codex MCP env_vars must be an array".to_string());
        return BTreeMap::new();
    };
    if values.len() > MAX_MAP_ENTRIES {
        reasons.push(format!(
            "Codex MCP env_vars exceeds the {MAX_MAP_ENTRIES} entry limit"
        ));
        return BTreeMap::new();
    }
    let mut result = BTreeMap::new();
    for value in values {
        let (name, source) = match value {
            Value::String(name) => (Some(name.as_str()), None),
            Value::Table(config) => (
                config.get("name").and_then(Value::as_str),
                config.get("source").and_then(Value::as_str),
            ),
            _ => (None, None),
        };
        let Some(name) = name.filter(|name| valid_environment_name(name)) else {
            reasons.push("Codex MCP env_vars entry is invalid".to_string());
            continue;
        };
        if !matches!(source, None | Some("local")) {
            reasons.push("Remote Codex MCP env_vars sources are not supported".to_string());
            continue;
        }
        result.insert(name.to_string(), name.to_string());
    }
    result
}

fn static_status(enabled: bool, reasons: Vec<String>) -> ExternalMcpStaticStatus {
    if !enabled {
        ExternalMcpStaticStatus::DisabledBySource
    } else if reasons.is_empty() {
        ExternalMcpStaticStatus::Ready
    } else {
        ExternalMcpStaticStatus::Unsupported {
            reason: reasons.join("; "),
        }
    }
}

fn resolve_environment(name: &str) -> Result<String, ExternalSourceProviderError> {
    std::env::var(name).map_err(|_| {
        provider_error(
            "environment_missing",
            &format!("Required environment variable '{name}' is not available"),
            true,
        )
    })
}

fn valid_environment_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_alphanumeric() && (index > 0 || !byte.is_ascii_digit())
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

fn enforce_size(bytes: usize, reasons: &mut Vec<String>) {
    if bytes > MAX_RUNTIME_TEXT_BYTES {
        reasons.push(format!(
            "MCP runtime values exceed the {MAX_RUNTIME_TEXT_BYTES} byte limit"
        ));
    }
}

fn ensure_runtime_size(bytes: usize) -> Result<(), ExternalSourceProviderError> {
    if bytes > MAX_RUNTIME_TEXT_BYTES {
        Err(provider_error(
            "runtime_too_large",
            &format!("Expanded MCP values exceed the {MAX_RUNTIME_TEXT_BYTES} byte limit"),
            false,
        ))
    } else {
        Ok(())
    }
}

fn resolve_layer_relative_cwd(value: &mut Value, config_file: &Path) {
    let Some(object) = value.as_table_mut() else {
        return;
    };
    let Some(Value::String(cwd)) = object.get_mut("cwd") else {
        return;
    };
    let path = PathBuf::from(&*cwd);
    if !path.is_absolute() {
        *cwd = normalize_path_lexically(&config_file.parent().unwrap_or(Path::new(".")).join(path))
            .to_string_lossy()
            .to_string();
    }
}

fn deep_merge(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Table(base), Value::Table(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(&key) {
                    Some(existing) => deep_merge(existing, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

fn behavior_version(
    revision_key: &bitfun_product_domains::external_sources::ExternalMcpRevisionKey,
    name: &str,
    value: &Value,
) -> String {
    let mut behavior = value.clone();
    if let Some(table) = behavior.as_table_mut() {
        if table.get("required").is_some_and(Value::is_bool) {
            table.remove("required");
        }
    }
    let encoded = toml::to_string(&behavior).unwrap_or_default();
    revision_key.opaque_revision(
        "codex.mcp.behavior.v1",
        [name.as_bytes(), encoded.as_bytes()],
    )
}

fn content_version(
    revision_key: &bitfun_product_domains::external_sources::ExternalMcpRevisionKey,
    path: &Path,
    content: &[u8],
) -> String {
    revision_key.opaque_revision(
        "codex.mcp.content.v1",
        [path.to_string_lossy().as_bytes(), content],
    )
}

fn source_key(path: &Path) -> SourceKey {
    let identity = dunce::canonicalize(path).unwrap_or_else(|_| normalize_path_lexically(path));
    SourceKey::new(
        PROVIDER_ID,
        format!(
            "codex_mcp-{}",
            &digest([identity.to_string_lossy().as_ref()])[..24]
        ),
    )
    .expect("hashed Codex MCP source id must be valid")
}

fn mcp_error(suffix: &str, message: &str, source: Option<SourceKey>) -> ExternalSourceDiagnostic {
    ExternalSourceDiagnostic::error(format!("codex.mcp.{suffix}"), message, source)
        .with_asset_kind(ExternalSourceAssetKind::Mcp)
}

fn provider_error(suffix: &str, message: &str, transient: bool) -> ExternalSourceProviderError {
    ExternalSourceProviderError::new(format!("codex.mcp.{suffix}"), message, transient)
}

fn push_layer(
    layers: &mut Vec<ConfigLayer>,
    path: PathBuf,
    scope: ExternalSourceScope,
    display_name: &'static str,
) {
    let should_inspect = match fs::metadata(&path) {
        Ok(metadata) => metadata.is_file(),
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    };
    if should_inspect {
        layers.push(ConfigLayer {
            path,
            scope,
            display_name,
        });
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

fn digest<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    hex::encode(hasher.finalize())
}
