//! MCP server manager
//!
//! Manages the lifecycle of all MCP servers.

use super::connection::{MCPConnection, MCPConnectionPool};
use super::{MCPServerConfig, MCPServerRegistry, MCPServerStatus};
use crate::service::mcp::adapter::tool::MCPToolAdapter;
use crate::service::mcp::config::MCPConfigService;
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, error, info, warn};
use std::sync::Arc;

/// MCP server manager.
pub struct MCPServerManager {
    registry: Arc<MCPServerRegistry>,
    connection_pool: Arc<MCPConnectionPool>,
    config_service: Arc<MCPConfigService>,
}

impl MCPServerManager {
    /// Creates a new server manager.
    pub fn new(config_service: Arc<MCPConfigService>) -> Self {
        Self {
            registry: Arc::new(MCPServerRegistry::new()),
            connection_pool: Arc::new(MCPConnectionPool::new()),
            config_service,
        }
    }

    /// Initializes all servers.
    pub async fn initialize_all(&self) -> BitFunResult<()> {
        info!("Initializing all MCP servers");

        let existing_server_ids = self.registry.get_all_server_ids().await;
        if !existing_server_ids.is_empty() {
            info!(
                "Refreshing MCP servers: shutting down existing servers before applying config: count={}",
                existing_server_ids.len()
            );
            self.shutdown().await?;
        }

        let configs = self.config_service.load_all_configs().await?;
        info!("Loaded {} MCP server configs", configs.len());

        if configs.is_empty() {
            warn!("No MCP server configurations found");
            return Ok(());
        }

        let mut registered_count = 0;
        for config in &configs {
            if config.enabled {
                match self.registry.register(config).await {
                    Ok(_) => {
                        registered_count += 1;
                        debug!(
                            "Registered MCP server: name={} id={}",
                            config.name, config.id
                        );
                    }
                    Err(e) => {
                        error!(
                            "Failed to register MCP server: name={} id={} error={}",
                            config.name, config.id, e
                        );
                        return Err(e);
                    }
                }
            }
        }
        info!("Registered {} MCP servers", registered_count);

        let mut started_count = 0;
        let mut failed_count = 0;
        for config in configs {
            if config.enabled && config.auto_start {
                info!(
                    "Auto-starting MCP server: name={} id={}",
                    config.name, config.id
                );
                match self.start_server(&config.id).await {
                    Ok(_) => {
                        started_count += 1;
                        info!("MCP server started successfully: name={}", config.name);
                    }
                    Err(e) => {
                        failed_count += 1;
                        error!(
                            "Failed to auto-start MCP server: name={} id={} error={}",
                            config.name, config.id, e
                        );
                    }
                }
            }
        }

        info!(
            "MCP server initialization completed: started={} failed={}",
            started_count, failed_count
        );
        Ok(())
    }

    /// Initializes servers without shutting down existing ones.
    ///
    /// This is safe to call multiple times (e.g., from multiple frontend windows).
    pub async fn initialize_non_destructive(&self) -> BitFunResult<()> {
        info!("Initializing MCP servers (non-destructive)");

        let configs = self.config_service.load_all_configs().await?;
        if configs.is_empty() {
            return Ok(());
        }

        for config in &configs {
            if !config.enabled {
                continue;
            }
            if !self.registry.contains(&config.id).await {
                if let Err(e) = self.registry.register(config).await {
                    warn!(
                        "Failed to register MCP server during non-destructive init: name={} id={} error={}",
                        config.name, config.id, e
                    );
                }
            }
        }

        for config in configs {
            if !(config.enabled && config.auto_start) {
                continue;
            }

            // Start only when not already running.
            if let Ok(status) = self.get_server_status(&config.id).await {
                if matches!(status, MCPServerStatus::Connected | MCPServerStatus::Healthy) {
                    continue;
                }
            }

            let _ = self.start_server(&config.id).await;
        }

        Ok(())
    }

    /// Ensures a server is registered in the registry if it exists in config.
    ///
    /// This is useful after config changes (e.g. importing MCP servers) where the registry
    /// hasn't been re-initialized yet.
    pub async fn ensure_registered(&self, server_id: &str) -> BitFunResult<()> {
        if self.registry.contains(server_id).await {
            return Ok(());
        }

        let Some(config) = self.config_service.get_server_config(server_id).await? else {
            return Err(BitFunError::NotFound(format!(
                "MCP server config not found: {}",
                server_id
            )));
        };

        if !config.enabled {
            return Ok(());
        }

        self.registry.register(&config).await?;
        Ok(())
    }

    /// Starts a server.
    pub async fn start_server(&self, server_id: &str) -> BitFunResult<()> {
        info!("Starting MCP server: id={}", server_id);

        let config = self
            .config_service
            .get_server_config(server_id)
            .await?
            .ok_or_else(|| {
                error!("MCP server config not found: id={}", server_id);
                BitFunError::NotFound(format!("MCP server config not found: {}", server_id))
            })?;

        if !config.enabled {
            warn!("MCP server is disabled: id={}", server_id);
            return Err(BitFunError::Configuration(format!(
                "MCP server is disabled: {}",
                server_id
            )));
        }

        if !self.registry.contains(server_id).await {
            self.registry.register(&config).await?;
        }

        let process = self.registry.get_process(server_id).await.ok_or_else(|| {
            error!("MCP server not registered: id={}", server_id);
            BitFunError::NotFound(format!("MCP server not registered: {}", server_id))
        })?;

        let mut proc = process.write().await;

        let status = proc.status().await;
        if matches!(
            status,
            MCPServerStatus::Connected | MCPServerStatus::Healthy
        ) {
            warn!("MCP server already running: id={}", server_id);
            return Ok(());
        }

        match config.server_type {
            super::MCPServerType::Local => {
                let command = config.command.as_ref().ok_or_else(|| {
                    error!("Missing command for local MCP server: id={}", server_id);
                    BitFunError::Configuration("Missing command for local MCP server".to_string())
                })?;

                info!(
                    "Starting local MCP server: command={} id={}",
                    command, server_id
                );

                proc.start(command, &config.args, &config.env)
                    .await
                    .map_err(|e| {
                        error!(
                            "Failed to start local MCP server process: id={} error={}",
                            server_id, e
                        );
                        e
                    })?;
            }
            super::MCPServerType::Remote => {
                let url = config.url.as_ref().ok_or_else(|| {
                    error!("Missing URL for remote MCP server: id={}", server_id);
                    BitFunError::Configuration("Missing URL for remote MCP server".to_string())
                })?;

                info!(
                    "Connecting to remote MCP server: url={} id={}",
                    url, server_id
                );

                proc.start_remote(url, &config.env, &config.headers)
                    .await
                    .map_err(|e| {
                    error!(
                        "Failed to connect to remote MCP server: url={} id={} error={}",
                        url, server_id, e
                    );
                    e
                })?;
            }
            super::MCPServerType::Container => {
                error!("Container MCP servers not supported: id={}", server_id);
                return Err(BitFunError::NotImplemented(
                    "Container MCP servers not yet supported".to_string(),
                ));
            }
        }

        if let Some(connection) = proc.connection() {
            self.connection_pool
                .add_connection(server_id.to_string(), connection.clone())
                .await;

            match Self::register_mcp_tools(server_id, &config.name, connection).await {
                Ok(count) => {
                    info!(
                        "Registered {} MCP tools: server_name={} server_id={}",
                        count, config.name, server_id
                    );
                }
                Err(e) => {
                    warn!(
                        "Failed to register MCP tools: server_name={} server_id={} error={}",
                        config.name, server_id, e
                    );
                }
            }
        } else {
            warn!(
                "Connection not available, server may not have started correctly: id={}",
                server_id
            );
        }

        info!("MCP server started successfully: id={}", server_id);
        Ok(())
    }

    /// Stops a server.
    pub async fn stop_server(&self, server_id: &str) -> BitFunResult<()> {
        info!("Stopping MCP server: id={}", server_id);

        let process =
            self.registry.get_process(server_id).await.ok_or_else(|| {
                BitFunError::NotFound(format!("MCP server not found: {}", server_id))
            })?;

        let mut proc = process.write().await;
        let stop_result = proc.stop().await;

        self.connection_pool.remove_connection(server_id).await;

        Self::unregister_mcp_tools(server_id).await;

        stop_result
    }

    /// Restarts a server.
    pub async fn restart_server(&self, server_id: &str) -> BitFunResult<()> {
        info!("Restarting MCP server: id={}", server_id);

        let config = self
            .config_service
            .get_server_config(server_id)
            .await?
            .ok_or_else(|| {
                BitFunError::NotFound(format!("MCP server config not found: {}", server_id))
            })?;

        match config.server_type {
            super::MCPServerType::Local => {
                self.ensure_registered(server_id).await?;

                let process = self.registry.get_process(server_id).await.ok_or_else(|| {
                    BitFunError::NotFound(format!("MCP server not found: {}", server_id))
                })?;
                let mut proc = process.write().await;

                let command = config
                    .command
                    .as_ref()
                    .ok_or_else(|| BitFunError::Configuration("Missing command".to_string()))?;
                proc.restart(command, &config.args, &config.env).await?;
            }
            super::MCPServerType::Remote => {
                // Treat restart as reconnect for remote servers.
                self.ensure_registered(server_id).await?;
                let _ = self.stop_server(server_id).await;
                self.start_server(server_id).await?;
            }
            _ => {
                return Err(BitFunError::NotImplemented(
                    "Restart not supported for this server type".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Returns server status.
    pub async fn get_server_status(&self, server_id: &str) -> BitFunResult<MCPServerStatus> {
        if !self.registry.contains(server_id).await {
            // If the server exists in config but isn't registered yet, register it so status
            // reflects reality (Uninitialized) instead of heuristics in the UI.
            let _ = self.ensure_registered(server_id).await;
        }

        let process =
            self.registry.get_process(server_id).await.ok_or_else(|| {
                BitFunError::NotFound(format!("MCP server not found: {}", server_id))
            })?;

        let proc = process.read().await;
        Ok(proc.status().await)
    }

    /// Returns statuses of all servers.
    pub async fn get_all_server_statuses(&self) -> Vec<(String, MCPServerStatus)> {
        let processes = self.registry.get_all_processes().await;
        let mut statuses = Vec::new();

        for process in processes {
            let proc = process.read().await;
            let id = proc.id().to_string();
            let status = proc.status().await;
            statuses.push((id, status));
        }

        statuses
    }

    /// Returns a connection.
    pub async fn get_connection(&self, server_id: &str) -> Option<Arc<MCPConnection>> {
        self.connection_pool.get_connection(server_id).await
    }

    /// Returns all server IDs.
    pub async fn get_all_server_ids(&self) -> Vec<String> {
        self.registry.get_all_server_ids().await
    }

    /// Adds a server.
    pub async fn add_server(&self, config: MCPServerConfig) -> BitFunResult<()> {
        config.validate()?;

        self.config_service.save_server_config(&config).await?;

        self.registry.register(&config).await?;

        if config.enabled && config.auto_start {
            self.start_server(&config.id).await?;
        }

        Ok(())
    }

    /// Removes a server.
    pub async fn remove_server(&self, server_id: &str) -> BitFunResult<()> {
        info!("Removing MCP server: id={}", server_id);

        match self.registry.unregister(server_id).await {
            Ok(_) => {
                info!("Unregistered MCP server: id={}", server_id);
            }
            Err(e) => {
                warn!(
                    "Server not running, skipping unregister: id={} error={}",
                    server_id, e
                );
            }
        }

        self.config_service.delete_server_config(server_id).await?;
        info!("Deleted MCP server config: id={}", server_id);

        Ok(())
    }

    /// Updates server configuration.
    pub async fn update_server_config(&self, config: MCPServerConfig) -> BitFunResult<()> {
        config.validate()?;

        self.config_service.save_server_config(&config).await?;

        let status = self.get_server_status(&config.id).await;
        if matches!(
            status,
            Ok(MCPServerStatus::Connected | MCPServerStatus::Healthy)
        ) {
            info!(
                "Restarting MCP server to apply new configuration: id={}",
                config.id
            );
            self.restart_server(&config.id).await?;
        }

        Ok(())
    }

    /// Shuts down all servers.
    pub async fn shutdown(&self) -> BitFunResult<()> {
        info!("Shutting down all MCP servers");

        let server_ids = self.registry.get_all_server_ids().await;
        for server_id in server_ids {
            if let Err(e) = self.stop_server(&server_id).await {
                error!("Failed to stop MCP server: id={} error={}", server_id, e);
            }
        }

        self.registry.clear().await?;

        info!("All MCP servers shut down");
        Ok(())
    }

    /// Registers MCP tools into the global tool registry.
    async fn register_mcp_tools(
        server_id: &str,
        server_name: &str,
        connection: Arc<MCPConnection>,
    ) -> BitFunResult<usize> {
        info!(
            "Registering MCP tools: server_name={} server_id={}",
            server_name, server_id
        );

        let mut adapter = MCPToolAdapter::new();

        adapter
            .load_tools_from_server(server_id, server_name, connection)
            .await
            .map_err(|e| {
                error!(
                    "Failed to load tools from MCP server: server_name={} server_id={} error={}",
                    server_name, server_id, e
                );
                e
            })?;

        let tools = adapter.get_tools();
        let tool_count = tools.len();

        for tool in tools {
            debug!(
                "Loaded MCP tool: name={} server={}",
                tool.name(),
                server_name
            );
        }

        let registry = crate::agentic::tools::registry::get_global_tool_registry();
        let mut registry_lock = registry.write().await;

        let tools_to_register = adapter.get_tools().to_vec();
        registry_lock.register_mcp_tools(tools_to_register);
        drop(registry_lock);

        info!(
            "Registered {} MCP tools: server_name={} server_id={}",
            tool_count, server_name, server_id
        );

        Ok(tool_count)
    }

    /// Unregisters MCP tools from the global tool registry.
    async fn unregister_mcp_tools(server_id: &str) {
        let registry = crate::agentic::tools::registry::get_global_tool_registry();
        let mut registry_lock = registry.write().await;
        registry_lock.unregister_mcp_server_tools(server_id);
        info!("Unregistered MCP tools: server_id={}", server_id);
    }
}
