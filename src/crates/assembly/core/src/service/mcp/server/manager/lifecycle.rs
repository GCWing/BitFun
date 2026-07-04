use super::*;
use bitfun_services_integrations::mcp::server::{
    mcp_server_is_running, mcp_should_start_after_config_update, resolve_mcp_local_command,
};

impl MCPServerManager {
    async fn runtime_server_config(&self, server_id: &str) -> BitFunResult<MCPServerConfig> {
        if let Some(config) = self.config_service.get_server_config(server_id).await? {
            return Ok(config);
        }

        self.runtime
            .get_runtime_config(server_id)
            .await
            .ok_or_else(|| {
                BitFunError::NotFound(format!("MCP server config not found: {}", server_id))
            })
    }

    fn resolve_local_command(command: &str) -> BitFunResult<(String, &'static str)> {
        let runtime_root = crate::infrastructure::get_path_manager_arc().managed_runtimes_dir();
        let resolved = resolve_mcp_local_command(command, runtime_root)?;
        Ok((resolved.command, resolved.source_label))
    }

    /// Initializes all servers.
    pub async fn initialize_all(&self) -> BitFunResult<()> {
        info!("Initializing all MCP servers");

        let existing_server_ids = self.runtime.get_all_server_ids().await;
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
            debug!("No MCP server configurations found, skipping initialization");
            return Ok(());
        }

        self.start_reconnect_monitor_if_needed();

        let mut registered_count = 0;
        for config in &configs {
            if config.enabled {
                match self.runtime.register(config).await {
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
                        return Err(e.into());
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

        self.start_reconnect_monitor_if_needed();

        for config in &configs {
            if !config.enabled {
                continue;
            }
            if !self.runtime.contains(&config.id).await {
                if let Err(e) = self.runtime.register(config).await {
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

            if let Ok(status) = self.get_server_status(&config.id).await {
                if matches!(
                    status,
                    MCPServerStatus::Connected | MCPServerStatus::Healthy
                ) {
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
        if self.runtime.contains(server_id).await {
            return Ok(());
        }

        let config = self.runtime_server_config(server_id).await?;

        if !config.enabled {
            return Ok(());
        }

        self.runtime.register(&config).await?;
        Ok(())
    }

    /// Starts a server.
    pub async fn start_server(&self, server_id: &str) -> BitFunResult<()> {
        self.start_reconnect_monitor_if_needed();
        info!("Starting MCP server: id={}", server_id);

        let config = self
            .runtime_server_config(server_id)
            .await
            .map_err(|error| {
                error!("MCP server config not found: id={}", server_id);
                error
            })?;

        if !config.enabled {
            warn!("MCP server is disabled: id={}", server_id);
            return Err(BitFunError::Configuration(format!(
                "MCP server is disabled: {}",
                server_id
            )));
        }

        if !self.runtime.contains(server_id).await {
            self.runtime.register(&config).await?;
        }

        let process = self.runtime.get_process(server_id).await.ok_or_else(|| {
            error!("MCP server not registered: id={}", server_id);
            BitFunError::NotFound(format!("MCP server not registered: {}", server_id))
        })?;

        let mut proc = process.write().await;

        let status = proc.status().await;
        if mcp_server_is_running(status) {
            warn!("MCP server already running: id={}", server_id);
            return Ok(());
        }

        match config.server_type {
            super::super::MCPServerType::Local => {
                let command = config.command.as_ref().ok_or_else(|| {
                    error!("Missing command for local MCP server: id={}", server_id);
                    BitFunError::Configuration("Missing command for local MCP server".to_string())
                })?;

                let (resolved_command, source_label) = Self::resolve_local_command(command)?;

                info!(
                    "Starting local MCP server: command={} source={} id={}",
                    resolved_command, source_label, server_id
                );

                proc.start(&resolved_command, &config.args, &config.env)
                    .await
                    .map_err(|e| {
                        error!(
                            "Failed to start local MCP server process: id={} command={} source={} error={}",
                            server_id, resolved_command, source_label, e
                    );
                    e
                })?;
            }
            super::super::MCPServerType::Remote => {
                let transport = config.resolved_transport();
                if transport != crate::service::mcp::server::MCPServerTransport::StreamableHttp {
                    error!(
                        "Remote MCP transport not supported yet: id={} transport={}",
                        server_id,
                        transport.as_str()
                    );
                    return Err(BitFunError::NotImplemented(format!(
                        "Remote MCP transport '{}' is not yet supported",
                        transport.as_str()
                    )));
                }

                let url = config.url.as_ref().ok_or_else(|| {
                    error!("Missing URL for remote MCP server: id={}", server_id);
                    BitFunError::Configuration("Missing URL for remote MCP server".to_string())
                })?;

                info!(
                    "Connecting to remote MCP server: transport={} url={} id={}",
                    transport.as_str(),
                    url,
                    server_id
                );

                let data_dir = crate::infrastructure::try_get_path_manager_arc()?.user_data_dir();
                proc.start_remote(data_dir, &config).await.map_err(|e| {
                    error!(
                        "Failed to connect to remote MCP server: url={} id={} error={}",
                        url, server_id, e
                    );
                    e
                })?;
            }
        }

        if let Some(connection) = proc.connection() {
            self.runtime
                .add_connection(server_id.to_string(), connection.clone())
                .await;

            match Self::register_mcp_tools(server_id, &config.name, connection.clone()).await {
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

            self.start_connection_event_listener(server_id, &config.name, connection.clone())
                .await;
            self.warm_catalog_caches(server_id, connection).await;
        } else {
            warn!(
                "Connection not available, server may not have started correctly: id={}",
                server_id
            );
        }

        info!("MCP server started successfully: id={}", server_id);
        self.clear_reconnect_state(server_id).await;
        Ok(())
    }

    /// Stops a server.
    pub async fn stop_server(&self, server_id: &str) -> BitFunResult<()> {
        info!("Stopping MCP server: id={}", server_id);

        self.stop_connection_event_listener(server_id).await;

        let process =
            self.runtime.get_process(server_id).await.ok_or_else(|| {
                BitFunError::NotFound(format!("MCP server not found: {}", server_id))
            })?;

        let mut proc = process.write().await;
        let stop_result = proc.stop().await;

        self.runtime.remove_connection(server_id).await;
        self.runtime.remove_catalog(server_id).await;

        Self::unregister_mcp_tools(server_id).await;

        Ok(stop_result?)
    }

    /// Restarts a server.
    pub async fn restart_server(&self, server_id: &str) -> BitFunResult<()> {
        info!("Restarting MCP server: id={}", server_id);

        let config = self.runtime_server_config(server_id).await?;

        match config.server_type {
            super::super::MCPServerType::Local => {
                self.ensure_registered(server_id).await?;

                let process = self.runtime.get_process(server_id).await.ok_or_else(|| {
                    BitFunError::NotFound(format!("MCP server not found: {}", server_id))
                })?;
                let mut proc = process.write().await;

                let command = config
                    .command
                    .as_ref()
                    .ok_or_else(|| BitFunError::Configuration("Missing command".to_string()))?;
                proc.restart(command, &config.args, &config.env).await?;
            }
            super::super::MCPServerType::Remote => {
                self.ensure_registered(server_id).await?;
                let _ = self.stop_server(server_id).await;
                self.start_server(server_id).await?;
            }
        }

        Ok(())
    }

    /// Returns server status.
    pub async fn get_server_status(&self, server_id: &str) -> BitFunResult<MCPServerStatus> {
        if !self.runtime.contains(server_id).await {
            let _ = self.ensure_registered(server_id).await;
        }

        let process =
            self.runtime.get_process(server_id).await.ok_or_else(|| {
                BitFunError::NotFound(format!("MCP server not found: {}", server_id))
            })?;

        let proc = process.read().await;
        Ok(proc.status().await)
    }

    /// Returns the current status detail/message for one server.
    pub async fn get_server_status_message(&self, server_id: &str) -> BitFunResult<Option<String>> {
        if !self.runtime.contains(server_id).await {
            let _ = self.ensure_registered(server_id).await;
        }

        let process =
            self.runtime.get_process(server_id).await.ok_or_else(|| {
                BitFunError::NotFound(format!("MCP server not found: {}", server_id))
            })?;

        let proc = process.read().await;
        Ok(proc.status_message().await)
    }

    /// Returns statuses of all servers.
    pub async fn get_all_server_statuses(&self) -> Vec<(String, MCPServerStatus)> {
        self.runtime.get_all_statuses().await
    }

    /// Returns a connection.
    pub async fn get_connection(&self, server_id: &str) -> Option<Arc<MCPConnection>> {
        self.runtime.get_connection(server_id).await
    }

    /// Returns all server IDs.
    pub async fn get_all_server_ids(&self) -> Vec<String> {
        self.runtime.get_all_server_ids().await
    }

    /// Adds a server.
    pub async fn add_server(&self, config: MCPServerConfig) -> BitFunResult<()> {
        config.validate()?;

        self.config_service.save_server_config(&config).await?;
        self.runtime.register(&config).await?;

        if config.enabled && config.auto_start {
            self.start_server(&config.id).await?;
        }

        Ok(())
    }

    /// Adds a runtime-only MCP server without saving it to user or project config.
    pub async fn add_ephemeral_server(&self, config: MCPServerConfig) -> BitFunResult<()> {
        config.validate()?;

        let server_id = config.id.clone();
        if self.runtime.contains(&server_id).await {
            let _ = self.remove_ephemeral_server(&server_id).await;
        }

        self.runtime.insert_runtime_config(config.clone()).await?;
        self.runtime.register(&config).await?;

        if config.enabled && config.auto_start {
            if let Err(error) = self.start_server(&server_id).await {
                let _ = self.remove_ephemeral_server(&server_id).await;
                return Err(error);
            }
        }

        Ok(())
    }

    /// Removes a runtime-only MCP server and its registered tools without touching persisted config.
    pub async fn remove_ephemeral_server(&self, server_id: &str) -> BitFunResult<()> {
        info!("Removing ephemeral MCP server: id={}", server_id);

        let _ = self.stop_server(server_id).await;
        self.stop_connection_event_listener(server_id).await;

        match self.runtime.unregister(server_id).await {
            Ok(_) => {
                info!("Unregistered ephemeral MCP server: id={}", server_id);
            }
            Err(e) => {
                warn!(
                    "Ephemeral MCP server was not registered, skipping unregister: id={} error={}",
                    server_id, e
                );
            }
        }

        self.runtime.remove_runtime_config(server_id).await;
        self.clear_reconnect_state(server_id).await;
        self.runtime.remove_catalog(server_id).await;

        Ok(())
    }

    /// Removes a server.
    pub async fn remove_server(&self, server_id: &str) -> BitFunResult<()> {
        info!("Removing MCP server: id={}", server_id);

        let _ = self.clear_remote_oauth_credentials(server_id).await;
        self.stop_connection_event_listener(server_id).await;

        match self.runtime.unregister(server_id).await {
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
        self.clear_reconnect_state(server_id).await;
        self.runtime.remove_catalog(server_id).await;
        info!("Deleted MCP server config: id={}", server_id);

        Ok(())
    }

    /// Updates server configuration.
    pub async fn update_server_config(&self, config: MCPServerConfig) -> BitFunResult<()> {
        config.validate()?;

        self.config_service.save_server_config(&config).await?;

        let status = self.get_server_status(&config.id).await?;
        if mcp_server_is_running(status) {
            info!(
                "Restarting MCP server to apply new configuration: id={}",
                config.id
            );
            self.restart_server(&config.id).await?;
        } else if mcp_should_start_after_config_update(&config, status) {
            info!(
                "Starting MCP server after configuration update: id={} previous_status={:?}",
                config.id, status
            );
            let _ = self.start_server(&config.id).await;
        }

        Ok(())
    }

    /// Updates remote MCP authorization and immediately retries the connection.
    pub async fn reauthenticate_remote_server(
        &self,
        server_id: &str,
        authorization_value: &str,
    ) -> BitFunResult<()> {
        self.clear_remote_oauth_credentials(server_id).await?;
        let config = self
            .config_service
            .set_remote_authorization(server_id, authorization_value)
            .await?;

        let _ = self.stop_server(server_id).await;
        self.clear_reconnect_state(server_id).await;

        if config.enabled {
            self.start_server(server_id).await?;
        }

        Ok(())
    }

    /// Clears remote MCP authorization and stops the current connection so stale credentials are dropped.
    pub async fn clear_remote_server_auth(&self, server_id: &str) -> BitFunResult<()> {
        self.clear_remote_oauth_credentials(server_id).await?;
        self.config_service
            .clear_remote_authorization(server_id)
            .await?;
        let _ = self.stop_server(server_id).await;
        self.clear_reconnect_state(server_id).await;
        Ok(())
    }

    /// Shuts down all servers.
    pub async fn shutdown(&self) -> BitFunResult<()> {
        info!("Shutting down all MCP servers");

        let server_ids = self.runtime.get_all_server_ids().await;
        for server_id in server_ids {
            if let Err(e) = self.stop_server(&server_id).await {
                error!("Failed to stop MCP server: id={} error={}", server_id, e);
            }
        }

        self.runtime.clear_registry().await?;
        self.runtime.clear_all_reconnect_state().await;
        self.runtime.clear_catalog().await;
        self.pending_interactions.write().await.clear();
        let oauth_sessions: Vec<_> = self
            .oauth_sessions
            .write()
            .await
            .drain()
            .map(|(_, session)| session)
            .collect();
        for session in oauth_sessions {
            Self::shutdown_oauth_session(&session).await;
        }
        let mut event_tasks = self.connection_event_tasks.write().await;
        for (_, handle) in event_tasks.drain() {
            handle.abort();
        }

        info!("All MCP servers shut down");
        Ok(())
    }
}
