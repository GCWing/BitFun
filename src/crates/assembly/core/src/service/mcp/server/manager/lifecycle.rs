use super::*;
use openbitfun_services_integrations::mcp::server::{
    mcp_server_is_running, mcp_should_start_after_config_update, MCPProcessStartContext,
    MCPProcessStartOutcome,
};
use std::collections::BTreeMap;

impl MCPServerManager {
    pub(super) fn try_begin_persisted_server_operation(
        &self,
        server_id: &str,
    ) -> OpenBitFunResult<PersistedServerOperationGuard> {
        let mut in_flight = self
            .persisted_server_operations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !in_flight.insert(server_id.to_string()) {
            return Err(OpenBitFunError::Configuration(format!(
                "MCP server lifecycle operation already in progress: {}",
                server_id
            )));
        }
        drop(in_flight);

        Ok(PersistedServerOperationGuard {
            server_id: server_id.to_string(),
            in_flight: Arc::clone(&self.persisted_server_operations),
        })
    }

    async fn runtime_server_config(&self, server_id: &str) -> OpenBitFunResult<MCPServerConfig> {
        if let Some(config) = self.config_service.get_server_config(server_id).await? {
            return Ok(config);
        }

        self.runtime
            .get_runtime_config(server_id)
            .await
            .ok_or_else(|| {
                OpenBitFunError::NotFound(format!("MCP server config not found: {}", server_id))
            })
    }

    /// Initializes all servers.
    pub async fn initialize_all(&self) -> OpenBitFunResult<()> {
        // Initialization can be requested by more than one product surface.
        // It must never tear down a healthy runtime merely because another
        // caller is ensuring that configured servers exist.
        self.initialize_non_destructive().await
    }

    /// Initializes servers without shutting down existing ones.
    ///
    /// This is safe to call multiple times (e.g., from multiple frontend windows).
    pub async fn initialize_non_destructive(&self) -> OpenBitFunResult<()> {
        info!("Initializing MCP servers (non-destructive)");
        let _lifecycle_guard = self.persisted_lifecycle.write().await;

        let configs = self.config_service.load_all_configs().await?;
        if configs.is_empty() {
            return Ok(());
        }

        self.start_reconnect_monitor_if_needed();

        for config in &configs {
            if !config.enabled {
                continue;
            }
            if let Err(e) = self.runtime.ensure_registered(config).await {
                warn!(
                    "Failed to register MCP server during non-destructive init: name={} id={} error={}",
                    config.name, config.id, e
                );
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

            let _ = self
                .start_server_with_external_token(&config.id, None)
                .await;
        }

        Ok(())
    }

    /// Ensures a server is registered in the registry if it exists in config.
    ///
    /// This is useful after config changes (e.g. importing MCP servers) where the registry
    /// hasn't been re-initialized yet.
    pub async fn ensure_registered(&self, server_id: &str) -> OpenBitFunResult<()> {
        if self.runtime.contains(server_id).await {
            return Ok(());
        }

        let config = self.runtime_server_config(server_id).await?;

        if !config.enabled {
            return Ok(());
        }

        self.runtime.ensure_registered(&config).await?;
        Ok(())
    }

    /// Starts a server.
    pub async fn start_server(&self, server_id: &str) -> OpenBitFunResult<()> {
        let _operation_guard = self.try_begin_persisted_server_operation(server_id)?;
        let _lifecycle_guard = self.persisted_lifecycle.read().await;
        self.start_server_with_external_token(server_id, None).await
    }

    pub(super) async fn start_server_with_external_token(
        &self,
        server_id: &str,
        expected_external_start_token: Option<Arc<()>>,
    ) -> OpenBitFunResult<()> {
        self.start_reconnect_monitor_if_needed();
        info!("Starting MCP server: id={}", server_id);

        let config = self
            .runtime_server_config(server_id)
            .await
            .inspect_err(|_| {
                error!("MCP server config not found: id={}", server_id);
            })?;

        if !config.enabled {
            warn!("MCP server is disabled: id={}", server_id);
            return Err(OpenBitFunError::Configuration(format!(
                "MCP server is disabled: {}",
                server_id
            )));
        }

        self.runtime.ensure_registered(&config).await?;
        if mcp_server_is_running(self.runtime.process_status(server_id).await?) {
            warn!("MCP server already running: id={}", server_id);
            return Ok(());
        }

        let start_context = match config.server_type {
            super::super::MCPServerType::Local => MCPProcessStartContext::Local {
                managed_runtimes_dir: crate::infrastructure::get_path_manager_arc()
                    .managed_runtimes_dir(),
            },
            super::super::MCPServerType::Remote => MCPProcessStartContext::Remote {
                data_dir: crate::infrastructure::try_get_path_manager_arc()?.user_data_dir(),
            },
        };
        let connection = match self
            .runtime
            .start_process(&config, start_context)
            .await
            .inspect_err(|error| {
                error!(
                    "Failed to start MCP server runtime: id={} error={}",
                    server_id, error
                );
            })? {
            MCPProcessStartOutcome::AlreadyRunning => {
                warn!("MCP server already running: id={}", server_id);
                return Ok(());
            }
            MCPProcessStartOutcome::Started { connection } => connection,
        };
        let external_workspace_scope = self
            .ephemeral_workspace_scopes
            .read()
            .await
            .get(server_id)
            .cloned();
        let _external_publication_guard = if external_workspace_scope.is_some() {
            Some(self.ephemeral_lifecycle.lock().await)
        } else {
            None
        };
        if !external_start_publication_allowed(
            external_workspace_scope.is_some(),
            self.ephemeral_retirements
                .read()
                .await
                .contains_key(server_id),
        ) {
            return Err(OpenBitFunError::Configuration(format!(
                "External MCP server was retired during startup: {}",
                server_id
            )));
        }
        if let Some(expected_token) = expected_external_start_token.as_ref() {
            let start_tokens = self.ephemeral_start_tokens.read().await;
            if !external_start_token_is_current(start_tokens.get(server_id), expected_token) {
                return Err(OpenBitFunError::Configuration(format!(
                    "External MCP server startup was superseded: {}",
                    server_id
                )));
            }
        }

        self.runtime
            .add_connection(server_id.to_string(), connection.clone())
            .await;

        match self
            .register_mcp_tools(server_id, &config.name, connection.clone())
            .await
        {
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
                if external_workspace_scope.is_some() {
                    self.runtime.remove_connection(server_id).await;
                    return Err(e);
                }
            }
        }

        self.start_connection_event_listener(server_id, &config.name, connection.clone())
            .await;
        // Runtime-only external MCP currently publishes workspace-routed Tools
        // only. Resources and Prompts have no external ownership/routing path,
        // so their best-effort warmup must not delay external tool readiness.
        if external_workspace_scope.is_none() {
            self.warm_catalog_caches(server_id, connection).await;
        }
        if external_workspace_scope.is_some() {
            self.ephemeral_ready_servers
                .write()
                .await
                .insert(server_id.to_string());
        }

        info!("MCP server started successfully: id={}", server_id);
        self.clear_reconnect_state(server_id).await;
        Ok(())
    }

    /// Stops a server.
    pub async fn stop_server(&self, server_id: &str) -> OpenBitFunResult<()> {
        let _operation_guard = self.try_begin_persisted_server_operation(server_id)?;
        let _lifecycle_guard = self.persisted_lifecycle.read().await;
        self.stop_server_unlocked(server_id).await
    }

    async fn stop_server_unlocked(&self, server_id: &str) -> OpenBitFunResult<()> {
        info!("Stopping MCP server: id={}", server_id);

        self.stop_connection_event_listener(server_id).await;

        let stop_result = self.runtime.stop_process(server_id).await;

        Self::unregister_mcp_tools(server_id).await;

        Ok(stop_result?)
    }

    /// Schedules a best-effort asynchronous stop with bounded retries.
    ///
    /// Entrypoints that delete an MCP server from the config use this owner
    /// method instead of duplicating the retry and timeout policy in their own
    /// process layer.
    pub fn schedule_stop_server(&self, server_id: impl Into<String>) {
        let manager = MCPServerManager::clone(self);
        let server_id = server_id.into();
        tokio::spawn(async move {
            for attempt in 1..=20 {
                let result = tokio::time::timeout(
                    Duration::from_millis(250),
                    manager.stop_server(&server_id),
                )
                .await;
                match result {
                    Ok(Ok(())) | Ok(Err(OpenBitFunError::NotFound(_))) => return,
                    Ok(Err(error)) => debug!(
                        "Best-effort MCP stop failed: id={} attempt={} error={}",
                        server_id, attempt, error
                    ),
                    Err(_) => debug!(
                        "Best-effort MCP stop timed out: id={} attempt={}",
                        server_id, attempt
                    ),
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            warn!("Best-effort MCP stop exhausted retries: id={}", server_id);
        });
    }

    /// Restarts a server.
    pub async fn restart_server(&self, server_id: &str) -> OpenBitFunResult<()> {
        let _operation_guard = self.try_begin_persisted_server_operation(server_id)?;
        let _lifecycle_guard = self.persisted_lifecycle.read().await;
        info!("Restarting MCP server: id={}", server_id);
        self.runtime_server_config(server_id).await?;
        self.ensure_registered(server_id).await?;
        self.stop_server_unlocked(server_id).await?;
        self.start_server_with_external_token(server_id, None).await
    }

    /// Reconciles a persisted configuration replacement without disturbing
    /// unchanged MCP runtimes. Existing manual-running state is preserved for
    /// changed servers, while newly added servers follow their auto-start policy.
    pub async fn reconcile_persisted_configs(
        &self,
        previous: Vec<MCPServerConfig>,
        current: Vec<MCPServerConfig>,
    ) -> OpenBitFunResult<()> {
        let _lifecycle_guard = self.persisted_lifecycle.write().await;
        let previous = previous
            .into_iter()
            .map(|config| (config.id.clone(), config))
            .collect::<BTreeMap<_, _>>();
        let current = current
            .into_iter()
            .map(|config| (config.id.clone(), config))
            .collect::<BTreeMap<_, _>>();

        for server_id in previous
            .keys()
            .filter(|server_id| !current.contains_key(*server_id))
        {
            self.remove_persisted_runtime_unlocked(server_id).await?;
        }

        for (server_id, config) in &current {
            let previous_config = previous.get(server_id);
            let unchanged = match previous_config {
                Some(previous) => serde_json::to_value(previous)? == serde_json::to_value(config)?,
                None => false,
            };

            if unchanged {
                if config.enabled {
                    self.runtime.ensure_registered(config).await?;
                } else {
                    self.remove_persisted_runtime_unlocked(server_id).await?;
                }
                continue;
            }

            let previous_status = if self.runtime.contains(server_id).await {
                self.runtime.process_status(server_id).await.ok()
            } else {
                None
            };
            let was_active = previous_status.is_some_and(|status| {
                matches!(
                    status,
                    MCPServerStatus::Starting
                        | MCPServerStatus::Connected
                        | MCPServerStatus::Healthy
                        | MCPServerStatus::NeedsAuth
                        | MCPServerStatus::Reconnecting
                )
            });

            self.remove_persisted_runtime_unlocked(server_id).await?;
            if !config.enabled {
                continue;
            }

            self.runtime.register(config).await?;
            if was_active || config.auto_start {
                self.start_server_with_external_token(server_id, None)
                    .await?;
            }
        }

        Ok(())
    }

    async fn remove_persisted_runtime_unlocked(&self, server_id: &str) -> OpenBitFunResult<()> {
        if !self.runtime.contains(server_id).await {
            return Ok(());
        }

        self.stop_server_unlocked(server_id).await?;
        self.runtime.unregister(server_id).await?;
        self.runtime.remove_catalog(server_id).await;
        self.clear_reconnect_state(server_id).await;
        Ok(())
    }

    /// Returns server status.
    pub async fn get_server_status(&self, server_id: &str) -> OpenBitFunResult<MCPServerStatus> {
        if !self.runtime.contains(server_id).await {
            let _ = self.ensure_registered(server_id).await;
        }

        self.runtime
            .process_status(server_id)
            .await
            .map_err(Into::into)
    }

    /// Returns the current status detail/message for one server.
    pub async fn get_server_status_message(
        &self,
        server_id: &str,
    ) -> OpenBitFunResult<Option<String>> {
        if !self.runtime.contains(server_id).await {
            let _ = self.ensure_registered(server_id).await;
        }

        self.runtime
            .process_status_message(server_id)
            .await
            .map_err(Into::into)
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
    pub async fn add_server(&self, config: MCPServerConfig) -> OpenBitFunResult<()> {
        config.validate()?;

        if self
            .config_service
            .get_server_config(&config.id)
            .await?
            .is_some()
        {
            return Err(OpenBitFunError::Configuration(format!(
                "MCP server already exists: {}",
                config.id
            )));
        }

        self.runtime.register(&config).await?;
        if let Err(error) = self.config_service.save_server_config(&config).await {
            let _ = self.runtime.unregister(&config.id).await;
            return Err(error);
        }

        if config.enabled && config.auto_start {
            self.start_server(&config.id).await?;
        }

        Ok(())
    }

    /// Removes a server.
    pub async fn remove_server(&self, server_id: &str) -> OpenBitFunResult<()> {
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
    pub async fn update_server_config(&self, config: MCPServerConfig) -> OpenBitFunResult<()> {
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
    ) -> OpenBitFunResult<()> {
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
    pub async fn clear_remote_server_auth(&self, server_id: &str) -> OpenBitFunResult<()> {
        self.clear_remote_oauth_credentials(server_id).await?;
        self.config_service
            .clear_remote_authorization(server_id)
            .await?;
        let _ = self.stop_server(server_id).await;
        self.clear_reconnect_state(server_id).await;
        Ok(())
    }

    /// Shuts down all servers.
    pub async fn shutdown(&self) -> OpenBitFunResult<()> {
        info!("Shutting down all MCP servers");

        for (_, cancelled) in self.ephemeral_retirements.write().await.drain() {
            cancelled.store(true, Ordering::Release);
        }
        self.ephemeral_ready_servers.write().await.clear();
        self.ephemeral_start_tokens.write().await.clear();

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
