use crate::local_source_paths::{
    local_source_plan, local_source_watch_roots, normalize_path_lexically,
    LocalConfigDirectoryKind, LocalConfigDocument, LocalConfigDocumentKind, LocalSourcePlanItem,
    OpenCodeLocalConfigOptions,
};
use openbitfun_product_domains::external_sources::{
    EcosystemId, ExternalMcpDiscoveryInput, ExternalMcpProviderIdentity,
    ExternalMcpProviderSnapshot, ExternalMcpServerDefinition, ExternalMcpSourceProvider,
    ExternalMcpStaticStatus, ExternalMcpTimeouts, ExternalMcpTransportKind,
    ExternalSourceAssetKind, ExternalSourceContext, ExternalSourceDiagnostic, ExternalSourceHealth,
    ExternalSourceProviderError, ExternalSourceRecord, ExternalSourceScope, ExternalWatchRoot,
    PreparedExternalMcpImportServer, PreparedExternalMcpImportTransport, PreparedExternalMcpServer,
    PreparedExternalMcpTransport, SecretValue, SourceKey, SourceQualifiedMcpServerId,
    MAX_EXTERNAL_MCP_TIMEOUT_MS,
};
use openbitfun_services_core::jsonc::strip_jsonc;
use openbitfun_static_hook_support::BoundedTextRead;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const PROVIDER_ID: &str = "opencode.mcp";
const ECOSYSTEM_ID: &str = "opencode";
const MAX_CONFIG_FILE_BYTES: usize = 1024 * 1024;
const MAX_MCP_SERVERS: usize = 256;
const MAX_COMMAND_PARTS: usize = 256;
const MAX_MAP_ENTRIES: usize = 128;
const MAX_RUNTIME_TEXT_BYTES: usize = 64 * 1024;
const LOCAL_FIELDS: &[&str] = &[
    "type",
    "command",
    "environment",
    "enabled",
    "timeout",
    "cwd",
];
const REMOTE_FIELDS: &[&str] = &["type", "url", "headers", "oauth", "enabled", "timeout"];

#[derive(Debug, Clone)]
pub struct OpenCodeMcpProviderOptions {
    pub config: OpenCodeLocalConfigOptions,
    /// Test/product-host override for an already-known project boundary.
    pub project_root_override: Option<PathBuf>,
}

impl OpenCodeMcpProviderOptions {
    pub fn from_environment() -> Self {
        Self {
            config: OpenCodeLocalConfigOptions::from_environment(),
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

    fn discover_layers(&self, context: &ExternalSourceContext) -> Vec<ConfigLayer> {
        local_source_plan(
            &self.options.config,
            context.workspace_root.as_deref(),
            self.options.project_root_override.as_deref(),
        )
        .into_iter()
        .filter_map(|item| match item {
            LocalSourcePlanItem::Config(document) => Some(ConfigLayer::new(document)),
            LocalSourcePlanItem::Directory(_) => None,
        })
        .collect()
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
            let parsed = parse_config_layer(&input.revision_key, &layer.document);
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
                location: layer.document.location(),
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

    fn current_preparation(
        &self,
        input: &ExternalMcpDiscoveryInput,
        server_id: &SourceQualifiedMcpServerId,
        expected_behavior_version: &str,
    ) -> Result<(ExternalMcpServerDefinition, PreparedTransportTemplate), ExternalSourceProviderError>
    {
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
            .cloned()
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
                "MCP server behavior changed before preparation",
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
                    "MCP preparation is unavailable",
                    false,
                )
            })?;
        Ok((definition, prepared))
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
        let (definition, prepared) =
            self.current_preparation(input, server_id, expected_behavior_version)?;
        resolve_runtime_values(
            prepared,
            server_id.clone(),
            expected_behavior_version,
            definition.timeouts,
        )
    }

    fn prepare_import(
        &self,
        input: &ExternalMcpDiscoveryInput,
        server_id: &SourceQualifiedMcpServerId,
        expected_behavior_version: &str,
    ) -> Result<PreparedExternalMcpImportServer, ExternalSourceProviderError> {
        let (definition, prepared) =
            self.current_preparation(input, server_id, expected_behavior_version)?;
        prepare_import_projection(definition, prepared)
    }

    fn watch_roots(&self, context: &ExternalSourceContext) -> Vec<ExternalWatchRoot> {
        local_source_watch_roots(
            &self.options.config,
            context.workspace_root.as_deref(),
            self.options.project_root_override.as_deref(),
        )
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
        working_directory_explicit: bool,
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
    revision_key: &openbitfun_product_domains::external_sources::ExternalMcpRevisionKey,
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
                    timeouts: ExternalMcpTimeouts::default(),
                    source_enabled,
                    behavior_version,
                    static_status: ExternalMcpStaticStatus::Unsupported { reason },
                },
                prepared: PreparedTransportTemplate::Local {
                    command: String::new(),
                    args: Vec::new(),
                    environment: BTreeMap::new(),
                    working_directory: None,
                    working_directory_explicit: false,
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
    let mut reason = unsupported_field_reason(object, LOCAL_FIELDS)
        .or_else(|| command_parts.as_ref().err().cloned())
        .or_else(|| unsupported_variable_reason(object));
    let timeouts = timeout_overrides(object).unwrap_or_else(|error| {
        reason.get_or_insert(error);
        ExternalMcpTimeouts::default()
    });
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
    let working_directory_explicit = object.contains_key("cwd");
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
            command_preview: Some(openbitfun_static_hook_support::redacted_executable_preview(
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
            timeouts,
            source_enabled,
            behavior_version,
            static_status,
        },
        prepared: PreparedTransportTemplate::Local {
            command,
            args,
            environment,
            working_directory: cwd,
            working_directory_explicit,
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
    let mut reason = unsupported_field_reason(object, REMOTE_FIELDS)
        .or_else(|| unsupported_variable_reason(object));
    let timeouts = timeout_overrides(object).unwrap_or_else(|error| {
        reason.get_or_insert(error);
        ExternalMcpTimeouts::default()
    });
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
            timeouts,
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
    timeouts: ExternalMcpTimeouts,
) -> Result<PreparedExternalMcpServer, ExternalSourceProviderError> {
    let transport = match template {
        PreparedTransportTemplate::Local {
            command,
            args,
            environment,
            working_directory,
            working_directory_explicit: _,
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
        timeouts,
        transport,
    })
}

fn prepare_import_projection(
    definition: ExternalMcpServerDefinition,
    template: PreparedTransportTemplate,
) -> Result<PreparedExternalMcpImportServer, ExternalSourceProviderError> {
    if !definition.timeouts.is_empty() {
        return Err(ExternalSourceProviderError::new(
            "external_mcp.import_setup_required",
            "MCP timeout overrides cannot be imported into native configuration",
            false,
        ));
    }
    let transport = match template {
        PreparedTransportTemplate::Local {
            command,
            args,
            environment,
            working_directory: _,
            working_directory_explicit,
        } if environment.is_empty() && !working_directory_explicit => {
            PreparedExternalMcpImportTransport::Local { command, args }
        }
        PreparedTransportTemplate::Remote {
            url,
            headers,
            oauth_enabled,
        } if headers.is_empty() && oauth_enabled => {
            PreparedExternalMcpImportTransport::Remote { url }
        }
        _ => {
            return Err(ExternalSourceProviderError::new(
                "external_mcp.import_setup_required",
                "MCP declaration contains fields that cannot be imported safely",
                false,
            ));
        }
    };
    let prepared = PreparedExternalMcpImportServer {
        id: definition.id,
        behavior_version: definition.behavior_version,
        transport,
    };
    prepared.validate().map_err(|_| {
        ExternalSourceProviderError::new(
            "external_mcp.import_setup_required",
            "MCP declaration contains fields that cannot be imported safely",
            false,
        )
    })?;
    Ok(prepared)
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

fn timeout_overrides(object: &Map<String, Value>) -> Result<ExternalMcpTimeouts, String> {
    match object.get("timeout") {
        None => Ok(ExternalMcpTimeouts::default()),
        Some(Value::Number(number)) => number
            .as_u64()
            .filter(|timeout| (1..=MAX_EXTERNAL_MCP_TIMEOUT_MS).contains(timeout))
            .map(|timeout| ExternalMcpTimeouts {
                startup_ms: Some(timeout),
                catalog_ms: Some(timeout),
                execution_ms: Some(timeout),
            })
            .ok_or_else(|| {
                format!(
                    "OpenCode MCP timeout must be an integer from 1 to {MAX_EXTERNAL_MCP_TIMEOUT_MS} milliseconds"
                )
            }),
        Some(_) => Err(format!(
            "OpenCode MCP timeout must be an integer from 1 to {MAX_EXTERNAL_MCP_TIMEOUT_MS} milliseconds"
        )),
    }
}

fn unsupported_field_reason(object: &Map<String, Value>, supported: &[&str]) -> Option<String> {
    let fields = object
        .keys()
        .filter(|field| !supported.contains(&field.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    (!fields.is_empty()).then(|| {
        format!(
            "OpenCode MCP fields are not supported: {}",
            fields.join(", ")
        )
    })
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
    document: LocalConfigDocument,
    scope: ExternalSourceScope,
    display_name: String,
}

impl ConfigLayer {
    fn new(document: LocalConfigDocument) -> Self {
        let display_name = match document.kind {
            LocalConfigDocumentKind::User => "OpenCode user configuration",
            LocalConfigDocumentKind::ExplicitFile => "OpenCode OPENCODE_CONFIG",
            LocalConfigDocumentKind::Project
            | LocalConfigDocumentKind::Directory(LocalConfigDirectoryKind::Project) => {
                "OpenCode project configuration"
            }
            LocalConfigDocumentKind::Directory(LocalConfigDirectoryKind::Legacy) => {
                "OpenCode legacy configuration"
            }
            LocalConfigDocumentKind::Directory(LocalConfigDirectoryKind::Explicit) => {
                "OpenCode OPENCODE_CONFIG_DIR"
            }
            LocalConfigDocumentKind::Directory(LocalConfigDirectoryKind::User) => {
                "OpenCode user configuration"
            }
            LocalConfigDocumentKind::Inline => "OpenCode OPENCODE_CONFIG_CONTENT",
        };
        Self {
            scope: document.scope,
            document,
            display_name: display_name.to_string(),
        }
    }
}

struct ParsedConfigLayer {
    servers: BTreeMap<String, Value>,
    diagnostics: Vec<ExternalSourceDiagnostic>,
    content_version: String,
    fatal: bool,
}

fn parse_config_layer(
    revision_key: &openbitfun_product_domains::external_sources::ExternalMcpRevisionKey,
    document: &LocalConfigDocument,
) -> ParsedConfigLayer {
    match document.read_bounded(MAX_CONFIG_FILE_BYTES) {
        Ok(BoundedTextRead::TooLarge) => ParsedConfigLayer {
            servers: BTreeMap::new(),
            diagnostics: vec![ExternalSourceDiagnostic::error(
                "opencode.mcp.config_too_large",
                "OpenCode config exceeds the 1 MiB compatibility limit",
                None,
            )
            .with_asset_kind(ExternalSourceAssetKind::Mcp)],
            content_version: "too-large".to_string(),
            fatal: true,
        },
        Ok(BoundedTextRead::InvalidUtf8) => ParsedConfigLayer {
            servers: BTreeMap::new(),
            diagnostics: vec![ExternalSourceDiagnostic::error(
                "opencode.mcp.config_invalid_utf8",
                "OpenCode config is not valid UTF-8",
                None,
            )
            .with_asset_kind(ExternalSourceAssetKind::Mcp)],
            content_version: "invalid-utf8".to_string(),
            fatal: true,
        },
        Ok(BoundedTextRead::Content(content)) => {
            let content_version = content_version(
                revision_key,
                document.identity().as_bytes(),
                content.as_bytes(),
            );
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
            ParsedConfigLayer {
                servers,
                diagnostics: Vec::new(),
                content_version,
                fatal: false,
            }
        }
        Err(error) => ParsedConfigLayer {
            servers: BTreeMap::new(),
            diagnostics: vec![ExternalSourceDiagnostic::error(
                "opencode.mcp.config_unreadable",
                format!("Failed to read OpenCode MCP config: {error}"),
                None,
            )
            .with_asset_kind(ExternalSourceAssetKind::Mcp)],
            content_version: "unreadable".to_string(),
            fatal: true,
        },
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
    revision_key: &openbitfun_product_domains::external_sources::ExternalMcpRevisionKey,
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
    revision_key: &openbitfun_product_domains::external_sources::ExternalMcpRevisionKey,
    identity: &[u8],
    content: &[u8],
) -> String {
    revision_key.opaque_revision("opencode.mcp.content.v1", [identity, content])
}

fn source_key(layer: &ConfigLayer) -> SourceKey {
    let mut hasher = Sha256::new();
    hasher.update(b"opencode_mcp_config");
    hasher.update([0]);
    hasher.update(layer.document.identity().as_bytes());
    let digest = hex::encode(hasher.finalize());
    SourceKey::new(
        PROVIDER_ID,
        format!("opencode_mcp_config-{}", &digest[..24]),
    )
    .expect("hashed OpenCode MCP source id must be valid")
}
