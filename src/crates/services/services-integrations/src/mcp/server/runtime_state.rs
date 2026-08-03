//! MCP server runtime state owner.
//!
//! This type groups reusable MCP runtime state that is independent from product
//! assembly side effects such as global tool registration, frontend events, and
//! OAuth callback UI.

use super::{
    mcp_server_is_running, resolve_mcp_local_command, MCPCatalogCache, MCPConnection,
    MCPConnectionPool, MCPReconnectTracker, MCPRuntimeError, MCPRuntimeResult, MCPServerConfig,
    MCPServerProcess, MCPServerRegistry, MCPServerStatus, MCPServerTransport, MCPServerType,
};
use crate::mcp::protocol::{MCPPrompt, MCPResource};
use log::info;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Result of starting one registered MCP process.
///
/// A newly started connection is returned without publishing it to the shared
/// connection pool. Product assembly can therefore complete its own
/// generation/retirement checks before making the connection visible.
pub enum MCPProcessStartOutcome {
    AlreadyRunning,
    Started { connection: Arc<MCPConnection> },
}

/// Host-owned paths required to start one MCP process.
///
/// Keeping the server kind in the context prevents callers from passing an
/// unrelated empty or placeholder path across the service boundary.
#[derive(Debug)]
pub enum MCPProcessStartContext {
    Local { managed_runtimes_dir: PathBuf },
    Remote { data_dir: PathBuf },
}

impl MCPProcessStartContext {
    fn server_type(&self) -> MCPServerType {
        match self {
            Self::Local { .. } => MCPServerType::Local,
            Self::Remote { .. } => MCPServerType::Remote,
        }
    }
}

impl fmt::Debug for MCPProcessStartOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRunning => formatter.write_str("AlreadyRunning"),
            Self::Started { .. } => formatter.write_str("Started"),
        }
    }
}

pub struct MCPServerRuntimeState {
    registry: MCPServerRegistry,
    connection_pool: MCPConnectionPool,
    reconnect_tracker: MCPReconnectTracker,
    catalog_cache: MCPCatalogCache,
}

impl MCPServerRuntimeState {
    pub fn new() -> Self {
        Self {
            registry: MCPServerRegistry::new(),
            connection_pool: MCPConnectionPool::new(),
            reconnect_tracker: MCPReconnectTracker::default(),
            catalog_cache: MCPCatalogCache::new(),
        }
    }

    pub async fn is_empty(&self) -> bool {
        self.registry.get_all_server_ids().await.is_empty()
    }

    pub async fn contains(&self, server_id: &str) -> bool {
        self.registry.contains(server_id).await
    }

    pub async fn register(&self, config: &MCPServerConfig) -> MCPRuntimeResult<()> {
        self.registry.register(config).await
    }

    pub async fn ensure_registered(&self, config: &MCPServerConfig) -> MCPRuntimeResult<bool> {
        self.registry.ensure_registered(config).await
    }

    pub async fn unregister(&self, server_id: &str) -> MCPRuntimeResult<()> {
        self.registry.unregister(server_id).await
    }

    pub async fn clear_registry(&self) -> MCPRuntimeResult<()> {
        self.registry.clear().await
    }

    async fn get_process(&self, server_id: &str) -> Option<Arc<RwLock<MCPServerProcess>>> {
        self.registry.get_process(server_id).await
    }

    pub async fn start_process(
        &self,
        config: &MCPServerConfig,
        context: MCPProcessStartContext,
    ) -> MCPRuntimeResult<MCPProcessStartOutcome> {
        if !config.enabled {
            return Err(MCPRuntimeError::configuration(format!(
                "MCP server is disabled: {}",
                config.id
            )));
        }
        if config.server_type != context.server_type() {
            let server_type = match config.server_type {
                MCPServerType::Local => "local",
                MCPServerType::Remote => "remote",
            };
            return Err(MCPRuntimeError::configuration(format!(
                "MCP process start context does not match server type '{server_type}'"
            )));
        }

        self.ensure_registered(config).await?;
        let process = self.get_process(&config.id).await.ok_or_else(|| {
            MCPRuntimeError::not_found(format!("MCP server not registered: {}", config.id))
        })?;
        let mut process = process.write().await;

        if mcp_server_is_running(process.status().await) {
            return Ok(MCPProcessStartOutcome::AlreadyRunning);
        }

        match (config.server_type, context) {
            (
                MCPServerType::Local,
                MCPProcessStartContext::Local {
                    managed_runtimes_dir,
                },
            ) => {
                let command = config.command.as_ref().ok_or_else(|| {
                    MCPRuntimeError::configuration("Missing command for local MCP server")
                })?;
                let resolved = resolve_mcp_local_command(command, managed_runtimes_dir)?;
                info!(
                    "Starting local MCP server: command={} source={} id={}",
                    resolved.command, resolved.source_label, config.id
                );
                process
                    .start_with_environment_policy_and_timeouts(
                        &resolved.command,
                        &config.args,
                        &config.env,
                        config.working_directory.as_deref().map(Path::new),
                        config.inherits_parent_environment(),
                        config.timeouts,
                    )
                    .await?;
            }
            (MCPServerType::Remote, MCPProcessStartContext::Remote { data_dir }) => {
                let transport = config.resolved_transport();
                if transport != MCPServerTransport::StreamableHttp {
                    return Err(MCPRuntimeError::not_implemented(format!(
                        "Remote MCP transport '{}' is not yet supported",
                        transport.as_str()
                    )));
                }
                if config.url.is_none() {
                    return Err(MCPRuntimeError::configuration(
                        "Missing URL for remote MCP server",
                    ));
                }
                info!(
                    "Connecting to remote MCP server: transport={} id={}",
                    transport.as_str(),
                    config.id
                );
                process.start_remote(data_dir, config).await?;
            }
            _ => unreachable!("MCP start context was validated before process registration"),
        }

        let connection = process.connection().ok_or_else(|| {
            MCPRuntimeError::mcp(format!(
                "MCP server '{}' started without a connection",
                config.id
            ))
        })?;
        Ok(MCPProcessStartOutcome::Started { connection })
    }

    pub async fn stop_process(&self, server_id: &str) -> MCPRuntimeResult<()> {
        let process = self.get_process(server_id).await.ok_or_else(|| {
            MCPRuntimeError::not_found(format!("MCP server not found: {server_id}"))
        })?;
        let stop_result = process.write().await.stop().await;

        self.remove_connection(server_id).await;
        self.remove_catalog(server_id).await;
        stop_result
    }

    pub async fn process_connection(&self, server_id: &str) -> Option<Arc<MCPConnection>> {
        let process = self.get_process(server_id).await?;
        let process = process.read().await;
        process.connection()
    }

    pub async fn process_status(&self, server_id: &str) -> MCPRuntimeResult<MCPServerStatus> {
        let process = self.get_process(server_id).await.ok_or_else(|| {
            MCPRuntimeError::not_found(format!("MCP server not found: {server_id}"))
        })?;
        let process = process.read().await;
        Ok(process.status().await)
    }

    pub async fn process_status_message(
        &self,
        server_id: &str,
    ) -> MCPRuntimeResult<Option<String>> {
        let process = self.get_process(server_id).await.ok_or_else(|| {
            MCPRuntimeError::not_found(format!("MCP server not found: {server_id}"))
        })?;
        let process = process.read().await;
        Ok(process.status_message().await)
    }

    pub async fn get_all_server_ids(&self) -> Vec<String> {
        self.registry.get_all_server_ids().await
    }

    async fn get_all_processes(&self) -> Vec<Arc<RwLock<MCPServerProcess>>> {
        self.registry.get_all_processes().await
    }

    pub async fn insert_runtime_config(&self, config: MCPServerConfig) -> MCPRuntimeResult<()> {
        self.registry.insert_runtime_config(config).await
    }

    pub async fn get_runtime_config(&self, server_id: &str) -> Option<MCPServerConfig> {
        self.registry.get_runtime_config(server_id).await
    }

    pub async fn remove_runtime_config(&self, server_id: &str) -> Option<MCPServerConfig> {
        self.registry.remove_runtime_config(server_id).await
    }

    pub async fn add_connection(&self, server_id: String, connection: Arc<MCPConnection>) {
        self.connection_pool
            .add_connection(server_id, connection)
            .await;
    }

    pub async fn get_connection(&self, server_id: &str) -> Option<Arc<MCPConnection>> {
        self.connection_pool.get_connection(server_id).await
    }

    pub async fn remove_connection(&self, server_id: &str) {
        self.connection_pool.remove_connection(server_id).await;
    }

    pub fn reconnect_poll_interval(&self) -> Duration {
        self.reconnect_tracker.poll_interval()
    }

    pub async fn has_pending_reconnects(&self) -> bool {
        self.reconnect_tracker.has_pending().await
    }

    pub async fn next_due_reconnect_attempt(&self, server_id: &str) -> Option<(u32, Duration)> {
        self.reconnect_tracker.next_due_attempt(server_id).await
    }

    pub async fn clear_reconnect_state(&self, server_id: &str) {
        self.reconnect_tracker.clear(server_id).await;
    }

    pub async fn clear_all_reconnect_state(&self) {
        self.reconnect_tracker.clear_all().await;
    }

    pub async fn refresh_resources(
        &self,
        server_id: &str,
        connection: Arc<MCPConnection>,
    ) -> MCPRuntimeResult<usize> {
        self.catalog_cache
            .refresh_resources(server_id, connection)
            .await
    }

    pub async fn refresh_prompts(
        &self,
        server_id: &str,
        connection: Arc<MCPConnection>,
    ) -> MCPRuntimeResult<usize> {
        self.catalog_cache
            .refresh_prompts(server_id, connection)
            .await
    }

    pub async fn warm_catalog(&self, server_id: &str, connection: Arc<MCPConnection>) {
        self.catalog_cache.warm(server_id, connection).await;
    }

    pub async fn get_cached_resources(&self, server_id: &str) -> Vec<MCPResource> {
        self.catalog_cache.get_resources(server_id).await
    }

    pub async fn get_cached_prompts(&self, server_id: &str) -> Vec<MCPPrompt> {
        self.catalog_cache.get_prompts(server_id).await
    }

    pub async fn remove_catalog(&self, server_id: &str) {
        self.catalog_cache.remove_server(server_id).await;
    }

    pub async fn clear_catalog(&self) {
        self.catalog_cache.clear().await;
    }

    pub async fn get_all_statuses(&self) -> Vec<(String, MCPServerStatus)> {
        let processes = self.get_all_processes().await;
        let mut statuses = Vec::new();

        for process in processes {
            let proc = process.read().await;
            let id = proc.id().to_string();
            let status = proc.status().await;
            statuses.push((id, status));
        }

        statuses
    }
}

impl Default for MCPServerRuntimeState {
    fn default() -> Self {
        Self::new()
    }
}
