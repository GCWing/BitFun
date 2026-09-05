use super::*;

const EXTERNAL_START_GUARD_ALLOWANCE: Duration = Duration::from_secs(30);

fn notify_external_tool_registry_changed() {
    #[cfg(feature = "external-sources")]
    crate::external_sources::notify_external_tool_registry_changed();
}

fn external_start_timeout(timeouts: super::super::MCPServerTimeouts) -> Duration {
    // The outer guard also covers bounded orchestration that sits outside the
    // per-request initialize and Tool catalog deadlines.
    let explicit_budget_ms = timeouts
        .startup_ms
        .unwrap_or_default()
        .saturating_add(timeouts.catalog_ms.unwrap_or_default());
    EXTERNAL_START_GUARD_ALLOWANCE.saturating_add(Duration::from_millis(explicit_budget_ms))
}

impl MCPServerManager {
    /// Adds a runtime-only MCP server without saving it to user or project config.
    pub async fn add_ephemeral_server(&self, config: MCPServerConfig) -> OpenBitFunResult<()> {
        config.validate()?;

        let server_id = config.id.clone();
        if self.runtime.contains(&server_id).await {
            return Err(OpenBitFunError::Configuration(format!(
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

    async fn remove_ephemeral_server_for_start(
        &self,
        server_id: &str,
        expected: &Arc<()>,
        failure: MCPServerStartFailure,
    ) -> bool {
        let _lifecycle_guard = self.ephemeral_lifecycle.lock().await;
        if !self.external_start_token_matches(server_id, expected).await {
            return false;
        }
        // Publish the safe failure fact before cleanup. Cleanup may itself
        // fail, but the product surface must still be able to explain why the
        // server left Starting.
        self.runtime
            .set_start_failure(server_id.to_string(), failure)
            .await;
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
    ) -> OpenBitFunResult<()> {
        config.validate()?;
        let _lifecycle_guard = self.ephemeral_lifecycle.lock().await;
        let server_id = config.id.clone();
        self.runtime.clear_start_failure(&server_id).await;
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
            let connection = self.runtime.process_connection(&server_id).await;
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
                self.start_connection_event_listener(&server_id, &config.name, connection)
                    .await;
                self.ephemeral_ready_servers
                    .write()
                    .await
                    .insert(server_id.clone());
            } else {
                let _ = self.remove_ephemeral_server(&server_id).await;
                return Err(OpenBitFunError::MCPError(
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
            return Err(OpenBitFunError::Configuration(format!(
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
            let start_timeout = external_start_timeout(config.timeouts);
            let manager = self.clone();
            tokio::spawn(async move {
                let startup = tokio::time::timeout(
                    start_timeout,
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
                            notify_external_tool_registry_changed();
                        }
                    }
                    Ok(Err(error)) => {
                        let failure = MCPServerStartFailure::classify(&error.to_string());
                        warn!(
                            "External ephemeral MCP server failed to start: id={} error={}",
                            server_id, error
                        );
                        if manager
                            .remove_ephemeral_server_for_start(&server_id, &start_token, failure)
                            .await
                        {
                            notify_external_tool_registry_changed();
                        }
                    }
                    Err(_) => {
                        warn!(
                            "External ephemeral MCP server startup timed out: id={}",
                            server_id
                        );
                        if manager
                            .remove_ephemeral_server_for_start(
                                &server_id,
                                &start_token,
                                MCPServerStartFailure::Timeout,
                            )
                            .await
                        {
                            notify_external_tool_registry_changed();
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
    pub async fn retire_external_ephemeral_server(&self, server_id: &str) -> OpenBitFunResult<()> {
        const RETIREMENT_GRACE: std::time::Duration = std::time::Duration::from_secs(30);
        const RETIREMENT_RECLAIM_ATTEMPTS: usize = 3;
        const RETIREMENT_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(250);
        let _lifecycle_guard = self.ephemeral_lifecycle.lock().await;
        self.runtime.clear_start_failure(server_id).await;
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
    pub async fn remove_ephemeral_server(&self, server_id: &str) -> OpenBitFunResult<()> {
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
}

#[cfg(test)]
mod timeout_tests {
    use super::external_start_timeout;
    use crate::service::mcp::MCPServerTimeouts;
    use std::time::Duration;

    #[test]
    fn external_start_guard_preserves_explicit_startup_and_catalog_budgets() {
        assert_eq!(
            external_start_timeout(MCPServerTimeouts::default()),
            Duration::from_secs(30)
        );
        assert_eq!(
            external_start_timeout(MCPServerTimeouts {
                startup_ms: Some(45_000),
                catalog_ms: Some(20_000),
                execution_ms: Some(90_000),
            }),
            Duration::from_secs(95)
        );
        assert_eq!(
            external_start_timeout(MCPServerTimeouts {
                execution_ms: Some(90_000),
                ..Default::default()
            }),
            Duration::from_secs(30)
        );
    }
}
