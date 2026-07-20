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
        let _lifecycle_guard = self.ephemeral_lifecycle.lock().await;

        let existing_server_ids = self.runtime.get_all_server_ids().await;
        if !existing_server_ids.is_empty() {
            let external_ids = self.ephemeral_workspace_scopes.read().await;
            let refresh_ids = existing_server_ids
                .iter()
                .filter(|server_id| !external_ids.contains_key(*server_id))
                .cloned()
                .collect::<Vec<_>>();
            drop(external_ids);
            info!(
                "Refreshing persisted MCP servers while preserving external workspace runtimes: count={}",
                refresh_ids.len()
            );
            for server_id in refresh_ids {
                let _ = self.stop_server(&server_id).await;
                let _ = self.runtime.unregister(&server_id).await;
                self.runtime.remove_catalog(&server_id).await;
                self.clear_reconnect_state(&server_id).await;
            }
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

        self.runtime.ensure_registered(&config).await?;
        Ok(())
    }

    /// Starts a server.
    pub async fn start_server(&self, server_id: &str) -> BitFunResult<()> {
        self.start_server_with_external_token(server_id, None).await
    }

    async fn start_server_with_external_token(
        &self,
        server_id: &str,
        expected_external_start_token: Option<Arc<()>>,
    ) -> BitFunResult<()> {
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
            return Err(BitFunError::Configuration(format!(
                "MCP server is disabled: {}",
                server_id
            )));
        }

        self.runtime.ensure_registered(&config).await?;

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

                proc.start_with_environment_policy(
                    &resolved_command,
                    &config.args,
                    &config.env,
                    config.working_directory.as_deref().map(Path::new),
                    config.inherits_parent_environment(),
                )
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

                config.url.as_ref().ok_or_else(|| {
                    error!("Missing URL for remote MCP server: id={}", server_id);
                    BitFunError::Configuration("Missing URL for remote MCP server".to_string())
                })?;

                info!(
                    "Connecting to remote MCP server: transport={} id={}",
                    transport.as_str(),
                    server_id
                );

                let data_dir = crate::infrastructure::try_get_path_manager_arc()?.user_data_dir();
                proc.start_remote(data_dir, &config).await.map_err(|e| {
                    error!(
                        "Failed to connect to remote MCP server: id={} error={}",
                        server_id, e
                    );
                    e
                })?;
            }
        }

        let connection = proc.connection();
        drop(proc);
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
            return Err(BitFunError::Configuration(format!(
                "External MCP server was retired during startup: {}",
                server_id
            )));
        }
        if let Some(expected_token) = expected_external_start_token.as_ref() {
            let start_tokens = self.ephemeral_start_tokens.read().await;
            if !external_start_token_is_current(start_tokens.get(server_id), expected_token) {
                return Err(BitFunError::Configuration(format!(
                    "External MCP server startup was superseded: {}",
                    server_id
                )));
            }
        }

        if let Some(connection) = connection {
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
            self.warm_catalog_caches(server_id, connection).await;
            if external_workspace_scope.is_some() {
                self.ephemeral_ready_servers
                    .write()
                    .await
                    .insert(server_id.to_string());
            }
        } else {
            warn!(
                "Connection not available, server may not have started correctly: id={}",
                server_id
            );
            if external_workspace_scope.is_some() {
                return Err(BitFunError::MCPError(
                    "External MCP server did not establish a connection".to_string(),
                ));
            }
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
                proc.restart_with_environment_policy(
                    command,
                    &config.args,
                    &config.env,
                    config.working_directory.as_deref().map(Path::new),
                    config.inherits_parent_environment(),
                )
                .await?;
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

        if self
            .config_service
            .get_server_config(&config.id)
            .await?
            .is_some()
        {
            return Err(BitFunError::Configuration(format!(
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

    /// Adds a runtime-only MCP server without saving it to user or project config.
    pub async fn add_ephemeral_server(&self, config: MCPServerConfig) -> BitFunResult<()> {
        config.validate()?;

        let server_id = config.id.clone();
        if self.runtime.contains(&server_id).await {
            return Err(BitFunError::Configuration(format!(
                "MCP server already exists: {}",
                server_id
            )));
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

    async fn external_start_token_matches(&self, server_id: &str, expected: &Arc<()>) -> bool {
        let start_tokens = self.ephemeral_start_tokens.read().await;
        external_start_token_is_current(start_tokens.get(server_id), expected)
    }

    async fn remove_ephemeral_server_for_start(&self, server_id: &str, expected: &Arc<()>) -> bool {
        let _lifecycle_guard = self.ephemeral_lifecycle.lock().await;
        if !self.external_start_token_matches(server_id, expected).await {
            return false;
        }
        if let Err(error) = self.remove_ephemeral_server(server_id).await {
            warn!(
                "Could not clean up failed external MCP startup: id={} error={}",
                server_id, error
            );
        }
        true
    }

    /// Installs a product-approved runtime-only server. A matching retirement
    /// can be cancelled without restarting the process, which keeps rapid
    /// disable/enable actions from interrupting unrelated session work.
    pub async fn install_external_ephemeral_server(
        &self,
        config: MCPServerConfig,
        workspace_key: String,
    ) -> BitFunResult<()> {
        config.validate()?;
        let _lifecycle_guard = self.ephemeral_lifecycle.lock().await;
        let server_id = config.id.clone();
        let start_token = Arc::new(());
        self.ephemeral_start_tokens
            .write()
            .await
            .insert(server_id.clone(), Arc::clone(&start_token));
        self.ephemeral_workspace_scopes
            .write()
            .await
            .insert(server_id.clone(), workspace_key);
        self.ephemeral_ready_servers
            .write()
            .await
            .remove(&server_id);
        let cancelled_retirement = self
            .ephemeral_retirements
            .write()
            .await
            .remove(&server_id)
            .map(|cancelled| {
                cancelled.store(true, Ordering::Release);
                true
            })
            .unwrap_or(false);

        if cancelled_retirement && self.runtime.contains(&server_id).await {
            if let Err(error) = self.runtime.insert_runtime_config(config.clone()).await {
                let _ = self.remove_ephemeral_server(&server_id).await;
                return Err(error.into());
            }
            let connection = if let Some(process) = self.runtime.get_process(&server_id).await {
                process.read().await.connection()
            } else {
                None
            };
            if let Some(connection) = connection {
                self.runtime
                    .add_connection(server_id.clone(), connection.clone())
                    .await;
                if let Err(error) = self
                    .refresh_mcp_tools(&server_id, &config.name, connection.clone())
                    .await
                {
                    let _ = self.remove_ephemeral_server(&server_id).await;
                    return Err(error);
                }
                self.start_connection_event_listener(&server_id, &config.name, connection.clone())
                    .await;
                self.warm_catalog_caches(&server_id, connection).await;
                self.ephemeral_ready_servers
                    .write()
                    .await
                    .insert(server_id.clone());
            } else {
                let _ = self.remove_ephemeral_server(&server_id).await;
                return Err(BitFunError::MCPError(
                    "External MCP server did not retain its connection".to_string(),
                ));
            }
            return Ok(());
        }
        if self.runtime.contains(&server_id).await {
            self.ephemeral_workspace_scopes
                .write()
                .await
                .remove(&server_id);
            self.ephemeral_start_tokens.write().await.remove(&server_id);
            return Err(BitFunError::Configuration(format!(
                "MCP server already exists: {}",
                server_id
            )));
        }

        if let Err(error) = self.runtime.insert_runtime_config(config.clone()).await {
            self.ephemeral_workspace_scopes
                .write()
                .await
                .remove(&server_id);
            self.ephemeral_start_tokens.write().await.remove(&server_id);
            return Err(error.into());
        }
        if let Err(error) = self.runtime.register(&config).await {
            self.runtime.remove_runtime_config(&server_id).await;
            self.ephemeral_workspace_scopes
                .write()
                .await
                .remove(&server_id);
            self.ephemeral_start_tokens.write().await.remove(&server_id);
            return Err(error.into());
        }
        if config.enabled && config.auto_start {
            // External source refresh and product-surface reads must not wait
            // for a third-party process or network handshake. Registration is
            // synchronous so status reads immediately see Loading; startup is
            // bounded in the background and cleans up only this runtime item.
            const EXTERNAL_START_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
            let manager = self.clone();
            tokio::spawn(async move {
                let startup = tokio::time::timeout(
                    EXTERNAL_START_TIMEOUT,
                    manager.start_server_with_external_token(
                        &server_id,
                        Some(Arc::clone(&start_token)),
                    ),
                )
                .await;
                match startup {
                    Ok(Ok(())) => {
                        if manager
                            .external_start_token_matches(&server_id, &start_token)
                            .await
                        {
                            crate::external_sources::notify_external_tool_registry_changed();
                        }
                    }
                    Ok(Err(error)) => {
                        warn!(
                            "External ephemeral MCP server failed to start: id={} error={}",
                            server_id, error
                        );
                        if manager
                            .remove_ephemeral_server_for_start(&server_id, &start_token)
                            .await
                        {
                            crate::external_sources::notify_external_tool_registry_changed();
                        }
                    }
                    Err(_) => {
                        warn!(
                            "External ephemeral MCP server startup timed out: id={}",
                            server_id
                        );
                        if manager
                            .remove_ephemeral_server_for_start(&server_id, &start_token)
                            .await
                        {
                            crate::external_sources::notify_external_tool_registry_changed();
                        }
                    }
                }
            });
        }
        Ok(())
    }

    /// Withdraws new tool/resource access immediately, then lets already-held
    /// connection users finish before the process is reclaimed. The grace is
    /// bounded so a deleted or malicious server cannot remain indefinitely.
    pub async fn retire_external_ephemeral_server(&self, server_id: &str) -> BitFunResult<()> {
        const RETIREMENT_GRACE: std::time::Duration = std::time::Duration::from_secs(30);
        const RETIREMENT_RECLAIM_ATTEMPTS: usize = 3;
        const RETIREMENT_RETRY_DELAY: std::time::Duration =
            std::time::Duration::from_millis(250);
        let _lifecycle_guard = self.ephemeral_lifecycle.lock().await;
        self.ephemeral_start_tokens.write().await.remove(server_id);
        if !self.runtime.contains(server_id).await {
            self.runtime.remove_runtime_config(server_id).await;
            self.ephemeral_ready_servers.write().await.remove(server_id);
            self.ephemeral_workspace_scopes
                .write()
                .await
                .remove(server_id);
            return Ok(());
        }

        if let Some(previous) = self
            .ephemeral_retirements
            .write()
            .await
            .insert(server_id.to_string(), Arc::new(AtomicBool::new(false)))
        {
            previous.store(true, Ordering::Release);
        }
        let cancelled = self
            .ephemeral_retirements
            .read()
            .await
            .get(server_id)
            .cloned()
            .expect("retirement marker was just inserted");
        let connection = self.runtime.get_connection(server_id).await;

        self.ephemeral_ready_servers.write().await.remove(server_id);
        Self::unregister_mcp_tools(server_id).await;
        self.stop_connection_event_listener(server_id).await;
        self.runtime.remove_connection(server_id).await;
        self.runtime.remove_catalog(server_id).await;
        self.runtime.remove_runtime_config(server_id).await;
        self.clear_reconnect_state(server_id).await;

        let manager = self.clone();
        let server_id = server_id.to_string();
        tokio::spawn(async move {
            let started = std::time::Instant::now();
            loop {
                if cancelled.load(Ordering::Acquire) {
                    return;
                }
                let references = connection.as_ref().map_or(0, Arc::strong_count);
                if should_finish_ephemeral_retirement(
                    references,
                    started.elapsed(),
                    RETIREMENT_GRACE,
                ) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }

            for attempt in 1..=RETIREMENT_RECLAIM_ATTEMPTS {
                let lifecycle_guard = manager.ephemeral_lifecycle.lock().await;
                if cancelled.load(Ordering::Acquire) {
                    return;
                }
                let should_remove = manager
                    .ephemeral_retirements
                    .read()
                    .await
                    .get(&server_id)
                    .is_some_and(|current| Arc::ptr_eq(current, &cancelled));
                if !should_remove {
                    return;
                }
                match manager.runtime.unregister(&server_id).await {
                    Ok(()) => {
                        manager
                            .ephemeral_retirements
                            .write()
                            .await
                            .remove(&server_id);
                        Self::unregister_mcp_tools(&server_id).await;
                        manager.stop_connection_event_listener(&server_id).await;
                        manager.runtime.remove_connection(&server_id).await;
                        manager.runtime.remove_catalog(&server_id).await;
                        manager
                            .ephemeral_ready_servers
                            .write()
                            .await
                            .remove(&server_id);
                        manager
                            .ephemeral_workspace_scopes
                            .write()
                            .await
                            .remove(&server_id);
                        return;
                    }
                    Err(error) if attempt < RETIREMENT_RECLAIM_ATTEMPTS => {
                        warn!(
                            "Could not reclaim retired ephemeral MCP server; retrying: id={} attempt={} error={}",
                            server_id, attempt, error
                        );
                    }
                    Err(error) => {
                        warn!(
                            "Could not reclaim retired ephemeral MCP server; retaining ownership for a later retry: id={} attempts={} error={}",
                            server_id, RETIREMENT_RECLAIM_ATTEMPTS, error
                        );
                        return;
                    }
                }
                drop(lifecycle_guard);
                tokio::time::sleep(RETIREMENT_RETRY_DELAY).await;
            }
        });
        Ok(())
    }

    /// Removes a runtime-only MCP server and its registered tools without touching persisted config.
    pub async fn remove_ephemeral_server(&self, server_id: &str) -> BitFunResult<()> {
        info!("Removing ephemeral MCP server: id={}", server_id);

        if !self.runtime.contains(server_id).await {
            self.runtime.remove_runtime_config(server_id).await;
            self.clear_reconnect_state(server_id).await;
            self.runtime.remove_catalog(server_id).await;
            Self::unregister_mcp_tools(server_id).await;
            return Ok(());
        }

        let stop_result = self.stop_server(server_id).await;
        self.stop_connection_event_listener(server_id).await;
        self.clear_reconnect_state(server_id).await;
        self.runtime.remove_catalog(server_id).await;
        self.ephemeral_ready_servers.write().await.remove(server_id);
        self.ephemeral_start_tokens.write().await.remove(server_id);
        self.ephemeral_workspace_scopes
            .write()
            .await
            .remove(server_id);

        if let Err(error) = stop_result {
            warn!(
                "Failed to stop ephemeral MCP server; retaining runtime ownership for retry: id={} error={}",
                server_id, error
            );
            return Err(error);
        }

        self.runtime.unregister(server_id).await?;
        self.runtime.remove_runtime_config(server_id).await;
        info!("Unregistered ephemeral MCP server: id={}", server_id);
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
