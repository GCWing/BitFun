//! Product-facing MCP management use cases.
//!
//! This module owns MCP configuration/runtime projections without depending on
//! App Server wire types or a concrete product surface.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bitfun_product_domains::external_sources::{
    ExternalMcpActivationState, ExternalMcpCatalogEntry, ExternalMcpTransportKind,
    ExternalSourceCatalogSnapshot,
};

use super::{
    ConfigLocation, MCPServerConfig, MCPServerManager, MCPServerStatus, MCPServerTransport,
    MCPServerType, MCPService,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpManagementErrorKind {
    InvalidRequest,
    Unavailable,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpManagementError {
    pub kind: McpManagementErrorKind,
    pub message: String,
}

impl McpManagementError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            kind: McpManagementErrorKind::InvalidRequest,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            kind: McpManagementErrorKind::Internal,
            message: sanitize_error(message.into()),
        }
    }
}

impl std::fmt::Display for McpManagementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for McpManagementError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpManagementList {
    pub servers: Vec<McpManagementServer>,
    pub config_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpManagementServer {
    pub id: String,
    pub name: String,
    pub server_type: String,
    pub status: String,
    pub tool_count: usize,
    pub source_label: String,
    pub external: bool,
    pub detail: String,
    pub action: McpManagementAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpManagementAction {
    NativeToggle,
    ReadOnly {
        reason: String,
    },
    ExternalDecision {
        candidate_id: String,
        decision_key: String,
        approved: bool,
        expected_mcp_generation: u64,
        expected_preference_revision: u64,
    },
    ConflictChoice {
        conflict_key: String,
        candidate_id: String,
        approve_external: bool,
        expected_mcp_generation: u64,
        expected_preference_revision: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpManagementTransport {
    Stdio,
    Sse,
    StreamableHttp,
}

#[derive(Clone)]
pub struct McpManagementMutation {
    pub transport: McpManagementTransport,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub url: Option<String>,
    pub auto_start: bool,
    pub enabled: bool,
    pub oauth: Option<serde_json::Value>,
    pub xaa: Option<serde_json::Value>,
}

impl std::fmt::Debug for McpManagementMutation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpManagementMutation")
            .field("transport", &self.transport)
            .field("command_configured", &self.command.is_some())
            .field("argument_count", &self.args.len())
            .field("environment_keys", &self.env.keys().collect::<Vec<_>>())
            .field("header_names", &self.headers.keys().collect::<Vec<_>>())
            .field("url_configured", &self.url.is_some())
            .field("auto_start", &self.auto_start)
            .field("enabled", &self.enabled)
            .field("oauth_configured", &self.oauth.is_some())
            .field("xaa_configured", &self.xaa.is_some())
            .finish()
    }
}

#[derive(Clone)]
pub struct McpManagementService {
    service: Arc<MCPService>,
}

impl McpManagementService {
    pub fn new(service: Arc<MCPService>) -> Self {
        Self { service }
    }

    pub async fn list(&self, workspace: &Path) -> Result<McpManagementList, McpManagementError> {
        let external = crate::external_sources::external_source_snapshot(Some(workspace), false)
            .await
            .map_err(McpManagementError::internal)?;
        let tool_registry = crate::agentic::tools::registry::get_global_tool_registry();
        let tools = tool_registry.read().await.get_all_tools();
        let configs = self
            .service
            .config_service()
            .load_all_configs()
            .await
            .map_err(|error| McpManagementError::internal(error.to_string()))?;
        let manager = self.service.server_manager();
        let mut servers = Vec::new();

        for config in configs {
            let status = native_status(&config, manager.as_ref()).await;
            let prefix = format!("mcp_{}_", config.id);
            let tool_count = tools
                .iter()
                .filter(|tool| tool.name().starts_with(&prefix))
                .count();
            let native_id = crate::external_sources::native_mcp_candidate_id(&config.id);
            let conflict = external.mcp_conflicts.iter().find(|conflict| {
                conflict
                    .candidates
                    .iter()
                    .any(|candidate| candidate.candidate_id == native_id)
            });
            let action = match conflict {
                Some(conflict)
                    if conflict
                        .candidates
                        .iter()
                        .find(|candidate| candidate.candidate_id == native_id)
                        .is_some_and(|candidate| !candidate.available) =>
                {
                    let reason = conflict
                        .candidates
                        .iter()
                        .find(|candidate| candidate.candidate_id == native_id)
                        .and_then(|candidate| candidate.unavailable_reason.clone())
                        .unwrap_or_else(|| {
                            "Enable this BitFun server in its MCP configuration".to_string()
                        });
                    McpManagementAction::ReadOnly { reason }
                }
                Some(conflict) if conflict.selected_candidate_id.as_deref() != Some(&native_id) => {
                    McpManagementAction::ConflictChoice {
                        conflict_key: conflict.conflict_key.clone(),
                        candidate_id: native_id,
                        approve_external: false,
                        expected_mcp_generation: external.mcp_generation,
                        expected_preference_revision: external.preference_revision,
                    }
                }
                _ => McpManagementAction::NativeToggle,
            };
            servers.push(McpManagementServer {
                id: config.id.clone(),
                name: config.name.clone(),
                server_type: format!("{:?}", config.server_type).to_lowercase(),
                status,
                tool_count,
                source_label: "BitFun".to_string(),
                external: false,
                detail: native_detail(&config),
                action,
            });
        }

        for entry in &external.mcp_servers {
            let source_label = external
                .sources
                .iter()
                .find(|source| source.record.key == entry.definition.id.source)
                .map(|source| source.record.display_name.clone())
                .unwrap_or_else(|| "External AI app".to_string());
            let status = external_status(entry, manager.as_ref()).await;
            let tool_count = entry.runtime_id.as_deref().map_or(0, |runtime_id| {
                let prefix = format!("mcp_{runtime_id}_");
                tools
                    .iter()
                    .filter(|tool| tool.name().starts_with(&prefix))
                    .count()
            });
            servers.push(McpManagementServer {
                id: entry.candidate_id.clone(),
                name: entry.definition.name.clone(),
                server_type: "external".to_string(),
                status,
                tool_count,
                source_label,
                external: true,
                detail: external_detail(entry),
                action: external_action(entry, &external),
            });
        }

        if servers.is_empty() && external.discovery_pending {
            servers.push(McpManagementServer {
                id: "external-mcp-discovery-pending".to_string(),
                name: "External MCP servers".to_string(),
                server_type: "external".to_string(),
                status: "Checking".to_string(),
                tool_count: 0,
                source_label: "External AI applications".to_string(),
                external: true,
                detail: "BitFun is still checking compatible MCP settings".to_string(),
                action: McpManagementAction::ReadOnly {
                    reason: "Still checking; this list updates automatically".to_string(),
                },
            });
        }

        let config_path = crate::infrastructure::try_get_path_manager_arc()
            .ok()
            .map(|manager| manager.app_config_file().display().to_string());
        Ok(McpManagementList {
            servers,
            config_path,
        })
    }

    pub async fn toggle(&self, server_id: &str) -> Result<(), McpManagementError> {
        let manager = self.service.server_manager();
        match manager.get_server_status(server_id).await {
            Ok(MCPServerStatus::Connected) | Ok(MCPServerStatus::Healthy) => {
                manager.stop_server(server_id).await
            }
            _ => manager.start_server(server_id).await,
        }
        .map_err(|error| McpManagementError::internal(error.to_string()))
    }

    pub async fn add(
        &self,
        name: &str,
        mutation: McpManagementMutation,
    ) -> Result<(), McpManagementError> {
        let config = config_from_mutation(name, mutation)?;
        self.service
            .server_manager()
            .add_server(config)
            .await
            .map_err(|error| McpManagementError::internal(error.to_string()))
    }

    pub async fn delete(&self, server_id: &str) -> Result<(), McpManagementError> {
        self.service
            .config_service()
            .delete_server_config(server_id)
            .await
            .map_err(|error| McpManagementError::internal(error.to_string()))?;
        schedule_stop(self.service.server_manager(), server_id.to_string());
        Ok(())
    }
}

async fn native_status(config: &MCPServerConfig, manager: &MCPServerManager) -> String {
    if !config.enabled {
        return "Stopped".to_string();
    }
    match tokio::time::timeout(
        Duration::from_millis(30),
        manager.get_server_status(&config.id),
    )
    .await
    {
        Ok(Ok(value)) => format!("{value:?}"),
        _ => "Starting".to_string(),
    }
}

fn native_detail(config: &MCPServerConfig) -> String {
    let server_type = format!("{:?}", config.server_type).to_ascii_lowercase();
    let transport = config.resolved_transport().as_str();
    if config.server_type == MCPServerType::Local {
        format!(
            "type: {server_type}; transport: {transport}; command: {}; arguments: {}; environment variables set: {}",
            config.command.as_deref().unwrap_or("unknown"),
            config.args.len(),
            if config.env.is_empty() { "none" } else { "configured" }
        )
    } else {
        let origin = config
            .url
            .as_deref()
            .and_then(|value| reqwest::Url::parse(value).ok())
            .and_then(|url| {
                let host = url.host_str()?;
                Some(match url.port() {
                    Some(port) => format!("{}://{}:{}", url.scheme(), host, port),
                    None => format!("{}://{}", url.scheme(), host),
                })
            })
            .unwrap_or_else(|| "unknown".to_string());
        format!(
            "type: {server_type}; transport: {transport}; remote origin: {}; HTTP headers: {}",
            origin,
            if config.headers.is_empty() {
                "none"
            } else {
                "configured"
            }
        )
    }
}

fn external_action(
    entry: &ExternalMcpCatalogEntry,
    snapshot: &ExternalSourceCatalogSnapshot,
) -> McpManagementAction {
    match &entry.activation_state {
        ExternalMcpActivationState::ApprovalRequired
        | ExternalMcpActivationState::Declined
        | ExternalMcpActivationState::ConfigurationChanged => {
            McpManagementAction::ExternalDecision {
                candidate_id: entry.candidate_id.clone(),
                decision_key: entry.decision_key.clone(),
                approved: true,
                expected_mcp_generation: snapshot.mcp_generation,
                expected_preference_revision: snapshot.preference_revision,
            }
        }
        ExternalMcpActivationState::Starting
        | ExternalMcpActivationState::Active
        | ExternalMcpActivationState::RuntimeUnavailable { .. } => {
            McpManagementAction::ExternalDecision {
                candidate_id: entry.candidate_id.clone(),
                decision_key: entry.decision_key.clone(),
                approved: false,
                expected_mcp_generation: snapshot.mcp_generation,
                expected_preference_revision: snapshot.preference_revision,
            }
        }
        ExternalMcpActivationState::Conflict | ExternalMcpActivationState::Covered { .. } => {
            snapshot
                .mcp_conflicts
                .iter()
                .find(|conflict| {
                    conflict
                        .candidates
                        .iter()
                        .any(|candidate| candidate.candidate_id == entry.candidate_id)
                })
                .map(|conflict| McpManagementAction::ConflictChoice {
                    conflict_key: conflict.conflict_key.clone(),
                    candidate_id: entry.candidate_id.clone(),
                    approve_external: true,
                    expected_mcp_generation: snapshot.mcp_generation,
                    expected_preference_revision: snapshot.preference_revision,
                })
                .unwrap_or_else(|| McpManagementAction::ReadOnly {
                    reason: "Refresh to review the current conflict".to_string(),
                })
        }
        ExternalMcpActivationState::Unsupported { reason } => McpManagementAction::ReadOnly {
            reason: format!("Not supported: {reason}"),
        },
        ExternalMcpActivationState::SourceDisabled => McpManagementAction::ReadOnly {
            reason: "Enable this server in the source application".to_string(),
        },
        ExternalMcpActivationState::Removed => McpManagementAction::ReadOnly {
            reason: "Removed".to_string(),
        },
        _ => McpManagementAction::ReadOnly {
            reason: "This external MCP state is read-only".to_string(),
        },
    }
}

async fn external_status(entry: &ExternalMcpCatalogEntry, manager: &MCPServerManager) -> String {
    match &entry.activation_state {
        ExternalMcpActivationState::Active => match entry.runtime_id.as_deref() {
            Some(id) => {
                match tokio::time::timeout(Duration::from_millis(30), manager.get_server_status(id))
                    .await
                {
                    Ok(Ok(value)) => format!("{value:?}"),
                    Ok(Err(_)) => "Unavailable".to_string(),
                    Err(_) => "Starting".to_string(),
                }
            }
            None => "Enabled".to_string(),
        },
        ExternalMcpActivationState::ApprovalRequired => "Confirmation required".to_string(),
        ExternalMcpActivationState::Starting => "Starting".to_string(),
        ExternalMcpActivationState::Declined => "Kept disabled".to_string(),
        ExternalMcpActivationState::Conflict => "Choice required".to_string(),
        ExternalMcpActivationState::Covered { .. } => "Not selected".to_string(),
        ExternalMcpActivationState::SourceDisabled => "Source disabled".to_string(),
        ExternalMcpActivationState::ConfigurationChanged => "Changed; confirm again".to_string(),
        ExternalMcpActivationState::Unsupported { .. } => "Not supported".to_string(),
        ExternalMcpActivationState::RuntimeUnavailable { reason } => {
            format!("Unavailable - {reason}")
        }
        ExternalMcpActivationState::Removed => "Removed".to_string(),
        _ => "Unavailable".to_string(),
    }
}

fn external_detail(entry: &ExternalMcpCatalogEntry) -> String {
    let definition = &entry.definition;
    match definition.transport {
        ExternalMcpTransportKind::LocalStdio => format!(
            "source MCP configuration; local command: {}; arguments: {}; environment variables set: {}",
            definition.command_preview.as_deref().unwrap_or("unknown"),
            definition.argument_count,
            if definition.environment_keys.is_empty() { "none" } else { "configured" },
        ),
        ExternalMcpTransportKind::StreamableHttp => format!(
            "source MCP configuration; remote origin: {}; HTTP headers: {}",
            definition.remote_url_preview.as_deref().unwrap_or("unknown"),
            if definition.header_names.is_empty() { "none" } else { "configured" },
        ),
        _ => "unsupported external MCP transport".to_string(),
    }
}

fn config_from_mutation(
    name: &str,
    mutation: McpManagementMutation,
) -> Result<MCPServerConfig, McpManagementError> {
    let (server_type, transport) = match mutation.transport {
        McpManagementTransport::Stdio => (MCPServerType::Local, MCPServerTransport::Stdio),
        McpManagementTransport::Sse => (MCPServerType::Remote, MCPServerTransport::Sse),
        McpManagementTransport::StreamableHttp => {
            (MCPServerType::Remote, MCPServerTransport::StreamableHttp)
        }
    };
    let oauth = mutation
        .oauth
        .map(serde_json::from_value)
        .transpose()
        .map_err(|_| McpManagementError::invalid("MCP OAuth configuration is invalid"))?;
    let xaa = mutation
        .xaa
        .map(serde_json::from_value)
        .transpose()
        .map_err(|_| McpManagementError::invalid("MCP XAA configuration is invalid"))?;
    Ok(MCPServerConfig {
        id: name.to_string(),
        name: name.to_string(),
        server_type,
        transport: Some(transport),
        command: mutation.command,
        args: mutation.args,
        env: mutation.env,
        working_directory: None,
        inherit_parent_environment: None,
        headers: mutation.headers,
        url: mutation.url,
        auto_start: mutation.auto_start,
        enabled: mutation.enabled,
        location: ConfigLocation::User,
        capabilities: Vec::new(),
        settings: HashMap::new(),
        oauth,
        oauth_enabled: None,
        xaa,
        timeouts: Default::default(),
    })
}

fn schedule_stop(manager: Arc<MCPServerManager>, server_id: String) {
    tokio::spawn(async move {
        for attempt in 1..=20 {
            let result =
                tokio::time::timeout(Duration::from_millis(250), manager.stop_server(&server_id))
                    .await;
            match result {
                Ok(Ok(())) | Ok(Err(crate::util::errors::BitFunError::NotFound(_))) => return,
                Ok(Err(error)) => log::debug!(
                    "Best-effort MCP stop failed: id={} attempt={} error={}",
                    server_id,
                    attempt,
                    error
                ),
                Err(_) => log::debug!(
                    "Best-effort MCP stop timed out: id={} attempt={}",
                    server_id,
                    attempt
                ),
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        log::warn!("Best-effort MCP stop exhausted retries: id={}", server_id);
    });
}

fn sanitize_error(message: String) -> String {
    let mut result = message
        .chars()
        .filter(|character| !character.is_control())
        .take(500)
        .collect::<String>();
    if result.is_empty() {
        result.push_str("MCP management operation failed");
    }
    result
}
