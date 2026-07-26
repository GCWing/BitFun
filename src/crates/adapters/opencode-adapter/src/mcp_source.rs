use bitfun_product_domains::external_sources::{
    EcosystemId, ExternalMcpDiscoveryInput, ExternalMcpProviderIdentity,
    ExternalMcpProviderSnapshot, ExternalMcpServerDefinition, ExternalMcpSourceProvider,
    ExternalMcpStaticStatus, ExternalMcpTransportKind, ExternalSourceAssetKind,
    ExternalSourceContext, ExternalSourceDiagnostic, ExternalSourceHealth,
    ExternalSourceProviderError, ExternalSourceRecord, ExternalSourceScope, ExternalWatchRoot,
    PreparedExternalMcpServer, PreparedExternalMcpTransport, SecretValue, SourceKey,
    SourceQualifiedMcpServerId,
};
use bitfun_static_hook_support::{read_bounded_text, BoundedTextRead};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const PROVIDER_ID: &str = "opencode.mcp";
const ECOSYSTEM_ID: &str = "opencode";
const MAX_CONFIG_FILE_BYTES: usize = 1024 * 1024;
const MAX_MCP_SERVERS: usize = 256;
const MAX_COMMAND_PARTS: usize = 256;
const MAX_MAP_ENTRIES: usize = 128;
const MAX_RUNTIME_TEXT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct OpenCodeMcpProviderOptions {
    pub user_config_dir: PathBuf,
    pub legacy_user_config_dir: Option<PathBuf>,
    pub explicit_config_file: Option<PathBuf>,
    pub explicit_config_dir: Option<PathBuf>,
    pub project_config_enabled: bool,
    /// Test/product-host override for an already-known project boundary.
    pub project_root_override: Option<PathBuf>,
}

impl OpenCodeMcpProviderOptions {
    pub fn from_environment() -> Self {
        let home = dirs::home_dir();
        let explicit_config_dir = std::env::var_os("OPENCODE_CONFIG_DIR").map(PathBuf::from);
        let user_config_dir = explicit_config_dir.clone().unwrap_or_else(|| {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| home.as_ref().map(|home| home.join(".config")))
                .unwrap_or_else(|| PathBuf::from(".config"))
                .join("opencode")
        });
        Self {
            user_config_dir,
            legacy_user_config_dir: explicit_config_dir
                .is_none()
                .then(|| home.map(|home| home.join(".opencode")))
                .flatten(),
            explicit_config_file: std::env::var_os("OPENCODE_CONFIG").map(PathBuf::from),
            explicit_config_dir,
            project_config_enabled: !environment_truthy("OPENCODE_DISABLE_PROJECT_CONFIG"),
            project_root_override: None,
        }
    }
}

impl Default for OpenCodeMcpProviderOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

pub struct OpenCodeMcpProvider {
    options: OpenCodeMcpProviderOptions,
}

impl OpenCodeMcpProvider {
    pub fn new(options: OpenCodeMcpProviderOptions) -> Self {
        Self { options }
    }

    fn project_root(&self, workspace_root: &Path) -> PathBuf {
        self.options
            .project_root_override
            .clone()
            .unwrap_or_else(|| find_project_root(workspace_root))
    }

    fn discover_layers(&self, context: &ExternalSourceContext) -> Vec<ConfigLayer> {
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
                    push_config_files(
                        &mut layers,
                        &directory.join(".opencode"),
                        ExternalSourceScope::Project,
                        "OpenCode project configuration",
                    );
                }
            }
        }
        if let Some(legacy) = &self.options.legacy_user_config_dir {
            if legacy != &self.options.user_config_dir {
                push_config_files(
                    &mut layers,
                    legacy,
                    ExternalSourceScope::UserGlobal,
                    "OpenCode legacy configuration",
                );
            }
        }
        if let Some(directory) = &self.options.explicit_config_dir {
            push_config_files(
                &mut layers,
                directory,
                ExternalSourceScope::UserGlobal,
                "OpenCode OPENCODE_CONFIG_DIR",
            );
        }
        deduplicate_layers_keep_last(layers)
    }

    fn materialize(
        &self,
        input: &ExternalMcpDiscoveryInput,
    ) -> Result<MaterializedMcpSnapshot, ExternalSourceProviderError> {
        if input
            .context
            .workspace_root
            .as_ref()
            .is_some_and(|workspace_root| !workspace_root.is_absolute())
        {
            return Err(ExternalSourceProviderError::new(
                "opencode.mcp.workspace_invalid",
                "workspace root must be absolute",
                false,
            ));
        }

        let provider = self.identity();
        let mut sources = Vec::new();
        let mut diagnostics = Vec::new();
        let mut merged_servers = BTreeMap::<String, Value>::new();
        let mut provenance = BTreeMap::<String, Vec<SourceKey>>::new();

        for layer in self.discover_layers(&input.context) {
            let key = source_key(&layer);
            let parsed = parse_config_layer(&input.revision_key, &layer.path);
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
                    .expect("static OpenCode ecosystem id must be valid"),
                display_name: layer.display_name.clone(),
                source_kind: "opencode_mcp_config".to_string(),
                scope: layer.scope,
                location: layer.path.to_string_lossy().to_string(),
                execution_domain_id: input.context.execution_domain_id.clone(),
                health,
                content_version: parsed.content_version,
                diagnostics: layer_diagnostics.clone(),
            });
            diagnostics.append(&mut layer_diagnostics);

            if parsed.fatal || input.suppressed_sources.contains(&key) {
                continue;
            }
            for (name, patch) in parsed.servers {
                if merged_servers.len() >= MAX_MCP_SERVERS && !merged_servers.contains_key(&name) {
                    diagnostics.push(
                        ExternalSourceDiagnostic::warning(
                            "opencode.mcp.server_limit",
                            format!(
                                "OpenCode MCP configuration exceeds the {MAX_MCP_SERVERS} server limit"
                            ),
                            Some(key.clone()),
                        )
                        .with_asset_kind(ExternalSourceAssetKind::Mcp),
                    );
                    continue;
                }
                let current = merged_servers
                    .entry(name.clone())
                    .or_insert_with(|| Value::Object(Map::new()));
                deep_merge(current, patch);
                let entries = provenance.entry(name).or_default();
                if entries.last() != Some(&key) {
                    entries.push(key.clone());
                }
            }
        }

        let mut servers = Vec::new();
        let mut prepared = BTreeMap::new();
        for (name, value) in merged_servers {
            let server_provenance = provenance.remove(&name).unwrap_or_default();
            let Some(effective_source) = server_provenance.last().cloned() else {
                continue;
            };
            match materialize_server(
                &input.context,
                &input.revision_key,
                effective_source,
                server_provenance,
                name,
                value,
            ) {
                Ok(server) => {
                    let stable_key = server.definition.id.stable_key();
                    prepared.insert(stable_key, server.prepared);
                    servers.push(server.definition);
                }
                Err(error) => diagnostics.push(
                    ExternalSourceDiagnostic::warning(error.code, error.message, None)
                        .with_asset_kind(ExternalSourceAssetKind::Mcp),
                ),
            }
        }
        servers.sort_by(|left, right| left.name.cmp(&right.name));

        let snapshot = ExternalMcpProviderSnapshot {
            provider,
            sources,
            servers,
            diagnostics,
        };
        snapshot.validate().map_err(|error| {
            ExternalSourceProviderError::new(
                "opencode.mcp.snapshot_invalid",
                error.to_string(),
                false,
            )
        })?;
        Ok(MaterializedMcpSnapshot { snapshot, prepared })
    }
}

impl Default for OpenCodeMcpProvider {
    fn default() -> Self {
        Self::new(OpenCodeMcpProviderOptions::default())
    }
}

impl ExternalMcpSourceProvider for OpenCodeMcpProvider {
    fn identity(&self) -> ExternalMcpProviderIdentity {
        ExternalMcpProviderIdentity::new(PROVIDER_ID, ECOSYSTEM_ID, "OpenCode")
            .expect("static OpenCode MCP provider identity must be valid")
    }

    fn discover(
        &self,
        input: &ExternalMcpDiscoveryInput,
    ) -> Result<ExternalMcpProviderSnapshot, ExternalSourceProviderError> {
        self.materialize(input)
            .map(|materialized| materialized.snapshot)
    }

    fn prepare_server(
        &self,
        input: &ExternalMcpDiscoveryInput,
        server_id: &SourceQualifiedMcpServerId,
        expected_behavior_version: &str,
    ) -> Result<PreparedExternalMcpServer, ExternalSourceProviderError> {
        if server_id.source.provider_id.as_str() != PROVIDER_ID {
            return Err(ExternalSourceProviderError::new(
                "opencode.mcp.identity_mismatch",
                "MCP server is not owned by the OpenCode MCP provider",
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
                ExternalSourceProviderError::new(
                    "opencode.mcp.stale_revision",
                    "MCP server is no longer available at the requested revision",
                    true,
                )
            })?;
        if definition.behavior_version != expected_behavior_version {
            return Err(ExternalSourceProviderError::new(
                "opencode.mcp.stale_revision",
                "MCP server behavior changed before activation",
                true,
            ));
        }
        if !definition.source_enabled
            || !matches!(definition.static_status, ExternalMcpStaticStatus::Ready)
        {
            return Err(ExternalSourceProviderError::new(
                "opencode.mcp.not_activatable",
                "MCP server is disabled or unsupported",
                false,
            ));
        }
        let prepared = materialized
            .prepared
            .get(&server_id.stable_key())
            .cloned()
            .ok_or_else(|| {
                ExternalSourceProviderError::new(
                    "opencode.mcp.preparation_missing",
                    "MCP runtime preparation is unavailable",
                    false,
                )
            })?;
        resolve_runtime_values(prepared, server_id.clone(), expected_behavior_version)
    }

    fn watch_roots(&self, context: &ExternalSourceContext) -> Vec<ExternalWatchRoot> {
        let mut roots = BTreeMap::new();
        add_directory_watch_roots(&mut roots, &self.options.user_config_dir);
        if let Some(legacy) = &self.options.legacy_user_config_dir {
            add_directory_watch_roots(&mut roots, legacy);
        }
        if let Some(path) = &self.options.explicit_config_file {
            if let Some(parent) = path.parent() {
                add_nearest_existing_watch_root(&mut roots, parent);
            }
        }
        if let Some(directory) = &self.options.explicit_config_dir {
            add_directory_watch_roots(&mut roots, directory);
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

struct MaterializedMcpSnapshot {
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
        oauth_enabled: bool,
    },
}

struct MaterializedServer {
    definition: ExternalMcpServerDefinition,
    prepared: PreparedTransportTemplate,
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
    let object = value.as_object().ok_or_else(|| {
        ExternalSourceProviderError::new(
            "opencode.mcp.server_invalid",
            format!("OpenCode MCP server '{name}' must be an object"),
            false,
        )
    })?;
    let source_enabled = object
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let server_type = object.get("type").and_then(Value::as_str);
    let inferred_type = server_type.or_else(|| {
        if object.contains_key("command") {
            Some("local")
        } else if object.contains_key("url") {
            Some("remote")
        } else {
            None
        }
    });
    let id = SourceQualifiedMcpServerId::new(effective_source, name.clone()).map_err(|error| {
        ExternalSourceProviderError::new(
            "opencode.mcp.name_invalid",
            format!("OpenCode MCP server name is invalid: {error}"),
            false,
        )
    })?;
    match inferred_type {
        Some("local") => materialize_local_server(
            context,
            id,
            provenance,
            name,
            object,
            source_enabled,
            behavior_version,
        ),
        Some("remote") => materialize_remote_server(
            id,
            provenance,
            name,
            object,
            source_enabled,
            behavior_version,
        ),
        _ => {
            let reason = "OpenCode MCP server type must be 'local' or 'remote'".to_string();
            Ok(MaterializedServer {
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
                    source_enabled,
                    behavior_version,
                    static_status: ExternalMcpStaticStatus::Unsupported { reason },
                },
                prepared: PreparedTransportTemplate::Local {
                    command: String::new(),
                    args: Vec::new(),
                    environment: BTreeMap::new(),
                    working_directory: None,
                },
            })
        }
    }
}

fn materialize_local_server(
    context: &ExternalSourceContext,
    id: SourceQualifiedMcpServerId,
    provenance: Vec<SourceKey>,
    name: String,
    object: &Map<String, Value>,
    source_enabled: bool,
    behavior_version: String,
) -> Result<MaterializedServer, ExternalSourceProviderError> {
    let command_parts = string_array(object.get("command"));
    let mut reason = command_parts
        .as_ref()
        .err()
        .cloned()
        .or_else(|| timeout_unsupported_reason(object))
        .or_else(|| unsupported_variable_reason(object));
    let command_parts = command_parts.unwrap_or_default();
    if command_parts.is_empty() {
        reason.get_or_insert_with(|| "Local MCP command must not be empty".to_string());
    }
    if command_parts.len() > MAX_COMMAND_PARTS {
        reason.get_or_insert_with(|| {
            format!("Local MCP command exceeds the {MAX_COMMAND_PARTS} part limit")
        });
    }
    let environment = string_map(object.get("environment"));
    if let Err(error) = &environment {
        reason.get_or_insert(error.clone());
    }
    let environment = environment.unwrap_or_default();
    let environment_reference_names =
        collect_environment_reference_names(environment.values().map(String::as_str));
    if let Err(error) = &environment_reference_names {
        reason.get_or_insert(error.clone());
    }
    let environment_reference_names = environment_reference_names.unwrap_or_default();
    let cwd = match object.get("cwd") {
        None => context
            .workspace_root
            .as_ref()
            .map(|path| normalize_path_lexically(path)),
        Some(Value::String(value)) => {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                Some(normalize_path_lexically(&path))
            } else if let Some(workspace_root) = &context.workspace_root {
                Some(normalize_path_lexically(&workspace_root.join(path)))
            } else {
                reason.get_or_insert_with(|| {
                    "Relative MCP working directory requires a workspace".to_string()
                });
                None
            }
        }
        Some(_) => {
            reason.get_or_insert_with(|| "MCP cwd must be a string".to_string());
            None
        }
    };
    let runtime_bytes = command_parts.iter().map(String::len).sum::<usize>()
        + environment
            .iter()
            .map(|(key, value)| key.len() + value.len())
            .sum::<usize>();
    if runtime_bytes > MAX_RUNTIME_TEXT_BYTES {
        reason.get_or_insert_with(|| {
            format!("Local MCP runtime values exceed the {MAX_RUNTIME_TEXT_BYTES} byte limit")
        });
    }
    let command = command_parts.first().cloned().unwrap_or_default();
    let args = command_parts.iter().skip(1).cloned().collect::<Vec<_>>();
    let static_status = if !source_enabled {
        ExternalMcpStaticStatus::DisabledBySource
    } else if let Some(reason) = reason {
        ExternalMcpStaticStatus::Unsupported { reason }
    } else {
        ExternalMcpStaticStatus::Ready
    };
    Ok(MaterializedServer {
        definition: ExternalMcpServerDefinition {
            id,
            provenance,
            name,
            transport: ExternalMcpTransportKind::LocalStdio,
            command_preview: Some(bitfun_static_hook_support::redacted_executable_preview(
                &command,
            )),
            argument_count: args.len(),
            working_directory: cwd
                .as_ref()
                .map(|directory| directory.to_string_lossy().to_string()),
            environment_keys: environment.keys().cloned().collect(),
            environment_reference_names,
            remote_url_preview: None,
            header_names: Vec::new(),
            source_enabled,
            behavior_version,
            static_status,
        },
        prepared: PreparedTransportTemplate::Local {
            command,
            args,
            environment,
            working_directory: cwd,
        },
    })
}

fn materialize_remote_server(
    id: SourceQualifiedMcpServerId,
    provenance: Vec<SourceKey>,
    name: String,
    object: &Map<String, Value>,
    source_enabled: bool,
    behavior_version: String,
) -> Result<MaterializedServer, ExternalSourceProviderError> {
    let raw_url = object
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let mut reason =
        timeout_unsupported_reason(object).or_else(|| unsupported_variable_reason(object));
    let preview_url = match sanitized_https_url(&raw_url) {
        Ok(url) => url,
        Err(error) => {
            reason.get_or_insert(error);
            "https://unsupported.invalid/mcp".to_string()
        }
    };
    let headers = string_map(object.get("headers"));
    if let Err(error) = &headers {
        reason.get_or_insert(error.clone());
    }
    let headers = headers.unwrap_or_default();
    let environment_reference_names =
        collect_environment_reference_names(headers.values().map(String::as_str));
    if let Err(error) = &environment_reference_names {
        reason.get_or_insert(error.clone());
    }
    let environment_reference_names = environment_reference_names.unwrap_or_default();
    let oauth_enabled = match object.get("oauth") {
        None => true,
        Some(Value::Bool(false)) => false,
        Some(Value::Object(oauth)) if oauth.is_empty() => true,
        Some(Value::Object(_)) => {
            reason.get_or_insert_with(|| {
                "Pre-registered OpenCode OAuth client configuration is not supported yet"
                    .to_string()
            });
            true
        }
        Some(_) => {
            reason
                .get_or_insert_with(|| "OpenCode MCP oauth must be an object or false".to_string());
            true
        }
    };
    let runtime_bytes = raw_url.len()
        + headers
            .iter()
            .map(|(key, value)| key.len() + value.len())
            .sum::<usize>();
    if runtime_bytes > MAX_RUNTIME_TEXT_BYTES {
        reason.get_or_insert_with(|| {
            format!("Remote MCP runtime values exceed the {MAX_RUNTIME_TEXT_BYTES} byte limit")
        });
    }
    let static_status = if !source_enabled {
        ExternalMcpStaticStatus::DisabledBySource
    } else if let Some(reason) = reason {
        ExternalMcpStaticStatus::Unsupported { reason }
    } else {
        ExternalMcpStaticStatus::Ready
    };
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
            remote_url_preview: Some(preview_url),
            header_names: headers.keys().cloned().collect(),
            source_enabled,
            behavior_version,
            static_status,
        },
        prepared: PreparedTransportTemplate::Remote {
            url: raw_url,
            headers,
            oauth_enabled,
        },
    })
}

fn resolve_runtime_values(
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
        } => {
            let command = expand_environment_references(&command)?;
            let args = args
                .iter()
                .map(|value| expand_environment_references(value))
                .collect::<Result<Vec<_>, _>>()?;
            let environment = environment
                .into_iter()
                .map(|(key, value)| {
                    expand_environment_references(&value)
                        .map(|value| (key, SecretValue::new(value)))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            let runtime_bytes = command.len()
                + args.iter().map(String::len).sum::<usize>()
                + environment
                    .iter()
                    .map(|(key, value)| key.len() + value.expose().len())
                    .sum::<usize>();
            if runtime_bytes > MAX_RUNTIME_TEXT_BYTES {
                return Err(ExternalSourceProviderError::new(
                    "opencode.mcp.runtime_too_large",
                    format!(
                        "Expanded MCP runtime values exceed the {MAX_RUNTIME_TEXT_BYTES} byte limit"
                    ),
                    false,
                ));
            }
            PreparedExternalMcpTransport::Local {
                command,
                args,
                environment,
                working_directory,
            }
        }
        PreparedTransportTemplate::Remote {
            url,
            headers,
            oauth_enabled,
        } => {
            let url = expand_environment_references(&url)?;
            sanitized_https_url(&url).map_err(|message| {
                ExternalSourceProviderError::new("opencode.mcp.url_invalid", message, false)
            })?;
            let headers = headers
                .into_iter()
                .map(|(key, value)| {
                    expand_environment_references(&value)
                        .map(|value| (key, SecretValue::new(value)))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            let runtime_bytes = url.len()
                + headers
                    .iter()
                    .map(|(key, value)| key.len() + value.expose().len())
                    .sum::<usize>();
            if runtime_bytes > MAX_RUNTIME_TEXT_BYTES {
                return Err(ExternalSourceProviderError::new(
                    "opencode.mcp.runtime_too_large",
                    format!(
                        "Expanded MCP runtime values exceed the {MAX_RUNTIME_TEXT_BYTES} byte limit"
                    ),
                    false,
                ));
            }
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

fn replace_environment_references(
    value: &str,
    mut resolve: impl FnMut(&str) -> Result<String, ExternalSourceProviderError>,
) -> Result<String, ExternalSourceProviderError> {
    let mut output = String::with_capacity(value.len());
    let mut remainder = value;
    while let Some(start) = remainder.find("{env:") {
        output.push_str(&remainder[..start]);
        let after_start = &remainder[start + 5..];
        let Some(end) = after_start.find('}') else {
            return Err(ExternalSourceProviderError::new(
                "opencode.mcp.variable_invalid",
                "OpenCode environment reference is not closed",
                false,
            ));
        };
        let name = &after_start[..end];
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(ExternalSourceProviderError::new(
                "opencode.mcp.variable_invalid",
                "OpenCode environment reference name is invalid",
                false,
            ));
        }
        let resolved = resolve(name)?;
        output.push_str(&resolved);
        remainder = &after_start[end + 1..];
    }
    output.push_str(remainder);
    Ok(output)
}

fn expand_environment_references(value: &str) -> Result<String, ExternalSourceProviderError> {
    replace_environment_references(value, |name| {
        std::env::var(name).map_err(|_| {
            ExternalSourceProviderError::new(
                "opencode.mcp.environment_missing",
                format!("Required environment variable '{name}' is not available"),
                true,
            )
        })
    })
}

fn collect_environment_reference_names<'a>(
    values: impl IntoIterator<Item = &'a str>,
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

fn timeout_unsupported_reason(object: &Map<String, Value>) -> Option<String> {
    match object.get("timeout") {
        None => None,
        Some(Value::Number(number)) if number.as_u64() == Some(5000) => None,
        Some(_) => {
            Some("Custom OpenCode MCP initialization timeout is not supported yet".to_string())
        }
    }
}

fn unsupported_variable_reason(object: &Map<String, Value>) -> Option<String> {
    let encoded = serde_json::to_string(object).ok()?;
    if encoded.contains("{file:") {
        return Some(
            "OpenCode file variable references are not supported for MCP servers".to_string(),
        );
    }
    let executable_reference =
        object
            .get("command")
            .and_then(Value::as_array)
            .is_some_and(|parts| {
                parts
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|part| part.contains("{env:"))
            });
    let address_or_cwd_reference = ["url", "cwd"].into_iter().any(|key| {
        object
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("{env:"))
    });
    (executable_reference || address_or_cwd_reference).then(|| {
        "Environment references are supported only in MCP environment and header values".to_string()
    })
}

fn string_array(value: Option<&Value>) -> Result<Vec<String>, String> {
    let values = value
        .and_then(Value::as_array)
        .ok_or_else(|| "Local MCP command must be an array of strings".to_string())?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| "Local MCP command must contain only strings".to_string())
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

fn sanitized_https_url(value: &str) -> Result<String, String> {
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

struct ConfigLayer {
    path: PathBuf,
    scope: ExternalSourceScope,
    display_name: String,
}

struct ParsedConfigLayer {
    servers: BTreeMap<String, Value>,
    diagnostics: Vec<ExternalSourceDiagnostic>,
    content_version: String,
    fatal: bool,
}

fn parse_config_layer(
    revision_key: &bitfun_product_domains::external_sources::ExternalMcpRevisionKey,
    path: &Path,
) -> ParsedConfigLayer {
    match read_bounded_text(path, MAX_CONFIG_FILE_BYTES) {
        Ok(BoundedTextRead::TooLarge) => {
            return ParsedConfigLayer {
                servers: BTreeMap::new(),
                diagnostics: vec![ExternalSourceDiagnostic::error(
                    "opencode.mcp.config_too_large",
                    "OpenCode config exceeds the 1 MiB compatibility limit",
                    None,
                )
                .with_asset_kind(ExternalSourceAssetKind::Mcp)],
                content_version: "too-large".to_string(),
                fatal: true,
            };
        }
        Ok(BoundedTextRead::InvalidUtf8) => {
            return ParsedConfigLayer {
                servers: BTreeMap::new(),
                diagnostics: vec![ExternalSourceDiagnostic::error(
                    "opencode.mcp.config_invalid_utf8",
                    "OpenCode config is not valid UTF-8",
                    None,
                )
                .with_asset_kind(ExternalSourceAssetKind::Mcp)],
                content_version: "invalid-utf8".to_string(),
                fatal: true,
            };
        }
        Ok(BoundedTextRead::Content(content)) => {
            let content_version = content_version(revision_key, path, content.as_bytes());
            let value = match serde_json::from_str::<Value>(&strip_jsonc(&content)) {
                Ok(value) => value,
                Err(error) => {
                    return ParsedConfigLayer {
                        servers: BTreeMap::new(),
                        diagnostics: vec![ExternalSourceDiagnostic::error(
                            "opencode.mcp.config_invalid",
                            format!("Failed to parse OpenCode MCP config: {error}"),
                            None,
                        )
                        .with_asset_kind(ExternalSourceAssetKind::Mcp)],
                        content_version,
                        fatal: true,
                    };
                }
            };
            let servers = match value.get("mcp") {
                None => BTreeMap::new(),
                Some(Value::Object(servers)) => servers
                    .iter()
                    .map(|(name, value)| (name.clone(), value.clone()))
                    .collect(),
                Some(_) => {
                    return ParsedConfigLayer {
                        servers: BTreeMap::new(),
                        diagnostics: vec![ExternalSourceDiagnostic::error(
                            "opencode.mcp.config_invalid",
                            "OpenCode top-level mcp field must be an object",
                            None,
                        )
                        .with_asset_kind(ExternalSourceAssetKind::Mcp)],
                        content_version,
                        fatal: true,
                    };
                }
            };
            return ParsedConfigLayer {
                servers,
                diagnostics: Vec::new(),
                content_version,
                fatal: false,
            };
        }
        Err(error) => {
            return ParsedConfigLayer {
                servers: BTreeMap::new(),
                diagnostics: vec![ExternalSourceDiagnostic::error(
                    "opencode.mcp.config_unreadable",
                    format!("Failed to read OpenCode MCP config: {error}"),
                    None,
                )
                .with_asset_kind(ExternalSourceAssetKind::Mcp)],
                content_version: "unreadable".to_string(),
                fatal: true,
            };
        }
    }
}

fn deep_merge(current: &mut Value, patch: Value) {
    match (current, patch) {
        (Value::Object(current), Value::Object(patch)) => {
            for (key, value) in patch {
                match current.get_mut(&key) {
                    Some(existing) => deep_merge(existing, value),
                    None => {
                        current.insert(key, value);
                    }
                }
            }
        }
        (current, patch) => *current = patch,
    }
}

fn behavior_version(
    revision_key: &bitfun_product_domains::external_sources::ExternalMcpRevisionKey,
    name: &str,
    value: &Value,
) -> String {
    let encoded = serde_json::to_vec(value).unwrap_or_default();
    revision_key.opaque_revision(
        "opencode.mcp.behavior.v1",
        [name.as_bytes(), encoded.as_slice()],
    )
}

fn content_version(
    revision_key: &bitfun_product_domains::external_sources::ExternalMcpRevisionKey,
    path: &Path,
    content: &[u8],
) -> String {
    revision_key.opaque_revision(
        "opencode.mcp.content.v1",
        [path.to_string_lossy().as_bytes(), content],
    )
}

fn source_key(layer: &ConfigLayer) -> SourceKey {
    let identity_path =
        dunce::canonicalize(&layer.path).unwrap_or_else(|_| normalize_path_lexically(&layer.path));
    let mut hasher = Sha256::new();
    hasher.update(b"opencode_mcp_config");
    hasher.update([0]);
    hasher.update(identity_path.to_string_lossy().as_bytes());
    let digest = hex::encode(hasher.finalize());
    SourceKey::new(
        PROVIDER_ID,
        format!("opencode_mcp_config-{}", &digest[..24]),
    )
    .expect("hashed OpenCode MCP source id must be valid")
}

fn push_config_files(
    layers: &mut Vec<ConfigLayer>,
    directory: &Path,
    scope: ExternalSourceScope,
    display_name: &str,
) {
    for name in ["opencode.json", "opencode.jsonc"] {
        push_config_file(layers, &directory.join(name), scope, display_name);
    }
}

fn push_config_file(
    layers: &mut Vec<ConfigLayer>,
    path: &Path,
    scope: ExternalSourceScope,
    display_name: &str,
) {
    let should_inspect = match fs::metadata(path) {
        Ok(metadata) => metadata.is_file(),
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    };
    if should_inspect {
        layers.push(ConfigLayer {
            path: path.to_path_buf(),
            scope,
            display_name: display_name.to_string(),
        });
    }
}

fn deduplicate_layers_keep_last(layers: Vec<ConfigLayer>) -> Vec<ConfigLayer> {
    let mut seen = BTreeSet::new();
    let mut layers = layers
        .into_iter()
        .rev()
        .filter(|layer| seen.insert(source_key(layer)))
        .collect::<Vec<_>>();
    layers.reverse();
    layers
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
            continue;
        }
        if current == '/' && chars.get(index + 1) == Some(&'/') {
            index += 2;
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            output.push('\n');
            index += usize::from(index < chars.len());
            continue;
        }
        if current == '/' && chars.get(index + 1) == Some(&'*') {
            index += 2;
            while index + 1 < chars.len() && !(chars[index] == '*' && chars[index + 1] == '/') {
                if chars[index] == '\n' {
                    output.push('\n');
                }
                index += 1;
            }
            index = (index + 2).min(chars.len());
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
    if let Some(parent) = directory.parent() {
        add_nearest_existing_watch_root(roots, parent);
    }
    add_watch_root(roots, directory.to_path_buf(), true);
}
