impl ChatMode {
    /// Show MCP server selector popup
    fn show_mcp_selector(
        &self,
        chat_view: &mut ChatView,
        _chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let items = self.get_mcp_items(rt_handle);
        // Show even if empty — user can press 'a' to add
        chat_view.show_mcp_selector(items);
    }

    /// Get MCP server items for display
    fn get_mcp_items(&self, rt_handle: &tokio::runtime::Handle) -> Vec<McpItem> {
        let mcp_service = match crate::get_mcp_service() {
            Some(svc) => svc,
            None => return Vec::new(),
        };

        let server_manager = mcp_service.server_manager();
        let config_service = mcp_service.config_service();

        tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let configs = match config_service.load_all_configs().await {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("Failed to load MCP configs: {}", e);
                        return Vec::new();
                    }
                };

                let tool_registry =
                    bitfun_core::agentic::tools::registry::get_global_tool_registry();
                let registry_lock = tool_registry.read().await;
                let all_tools = registry_lock.get_all_tools();

                let mut items = Vec::new();
                for config in configs {
                    let status = if !config.enabled {
                        "Stopped".to_string()
                    } else {
                        // Avoid blocking UI while a slow auto-start server holds internal write lock.
                        match tokio::time::timeout(
                            Duration::from_millis(30),
                            server_manager.get_server_status(&config.id),
                        )
                        .await
                        {
                            Ok(Ok(s)) => format!("{:?}", s),
                            Ok(Err(_)) => "Unknown".to_string(),
                            Err(_) => "Starting".to_string(),
                        }
                    };

                    // Count tools from this server
                    let prefix = format!("mcp_{}_", config.id);
                    let tool_count = all_tools
                        .iter()
                        .filter(|t| t.name().starts_with(&prefix))
                        .count();

                    let server_type = format!("{:?}", config.server_type).to_lowercase();

                    items.push(McpItem {
                        id: config.id.clone(),
                        name: config.name.clone(),
                        server_type,
                        status,
                        tool_count,
                    });
                }
                items
            })
        })
    }

    /// Schedule an MCP server toggle (deferred to allow loading state to render)
    fn toggle_mcp_server(&mut self, server_id: &str, chat_view: &mut ChatView) {
        if self.pending_mcp_op.is_some() || self.is_mcp_server_task_running(server_id) {
            return;
        }

        // Set loading indicator immediately — will be rendered before execution
        chat_view.mcp_selector_set_loading(Some(server_id.to_string()));
        self.pending_mcp_op = Some(PendingMcpOp::Toggle(server_id.to_string()));
    }

    /// Execute MCP server toggle (called from main loop after render)
    fn execute_mcp_toggle(
        &mut self,
        server_id: &str,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let mcp_service = match crate::get_mcp_service() {
            Some(svc) => svc.clone(),
            None => {
                chat_state.add_system_message("MCP service not initialized".to_string());
                chat_view.mcp_selector_set_loading(None);
                return;
            }
        };

        let server_manager = mcp_service.server_manager();
        let task_server_id = server_id.to_string();
        let tracked_server_id = task_server_id.clone();

        let handle = rt_handle.spawn(async move {
            let status = server_manager.get_server_status(&task_server_id).await;
            match status {
                Ok(bitfun_core::service::mcp::MCPServerStatus::Connected)
                | Ok(bitfun_core::service::mcp::MCPServerStatus::Healthy) => {
                    server_manager.stop_server(&task_server_id).await
                }
                _ => server_manager.start_server(&task_server_id).await,
            }
        });

        self.pending_mcp_tasks.push(PendingMcpTask::Toggle {
            server_id: tracked_server_id,
            handle,
        });
    }

    fn is_mcp_server_task_running(&self, server_id: &str) -> bool {
        self.pending_mcp_tasks.iter().any(|task| match task {
            PendingMcpTask::Toggle { server_id: id, .. }
            | PendingMcpTask::Delete { server_id: id, .. } => id == server_id,
            PendingMcpTask::Add { .. } => false,
        })
    }

    fn has_pending_mcp_add_task(&self) -> bool {
        self.pending_mcp_tasks
            .iter()
            .any(|task| matches!(task, PendingMcpTask::Add { .. }))
    }

    fn poll_mcp_task_completion(
        &mut self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> bool {
        let mut changed = false;
        let mut i = 0;
        while i < self.pending_mcp_tasks.len() {
            let finished = match &self.pending_mcp_tasks[i] {
                PendingMcpTask::Toggle { handle, .. }
                | PendingMcpTask::Add { handle, .. }
                | PendingMcpTask::Delete { handle, .. } => handle.is_finished(),
            };
            if !finished {
                i += 1;
                continue;
            }

            let task = self.pending_mcp_tasks.swap_remove(i);
            changed = true;
            match task {
                PendingMcpTask::Toggle { server_id, handle } => {
                    let join_result = tokio::task::block_in_place(|| rt_handle.block_on(handle));

                    match join_result {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => {
                            tracing::error!("Failed to toggle MCP server {}: {}", server_id, e);
                            chat_state.add_system_message(format!(
                                "Failed to toggle MCP server '{}': {}",
                                server_id, e
                            ));
                        }
                        Err(e) => {
                            tracing::error!("MCP toggle task join error for {}: {}", server_id, e);
                            chat_state.add_system_message(format!(
                                "MCP server '{}' task failed: {}",
                                server_id, e
                            ));
                        }
                    }

                    chat_view.mcp_selector_set_loading(None);
                    let updated_items = self.get_mcp_items(rt_handle);
                    chat_view.mcp_selector_update_items(updated_items);
                }
                PendingMcpTask::Add { name, handle } => {
                    let join_result = tokio::task::block_in_place(|| rt_handle.block_on(handle));

                    match join_result {
                        Ok(Ok(())) => {
                            chat_state.add_system_message(format!(
                                "MCP server '{}' added and started",
                                name
                            ));
                            self.show_mcp_selector(chat_view, chat_state, rt_handle);
                        }
                        Ok(Err(e)) => {
                            chat_state
                                .add_system_message(format!("Failed to add MCP server: {}", e));
                        }
                        Err(e) => {
                            chat_state.add_system_message(format!(
                                "MCP add task failed for '{}': {}",
                                name, e
                            ));
                        }
                    }
                    chat_view.set_status(None);
                }
                PendingMcpTask::Delete { server_id, handle } => {
                    let join_result = tokio::task::block_in_place(|| rt_handle.block_on(handle));

                    match join_result {
                        Ok(Ok(())) => {
                            chat_state
                                .add_system_message(format!("MCP server '{}' deleted", server_id));
                        }
                        Ok(Err(e)) => {
                            chat_state
                                .add_system_message(format!("Failed to delete MCP server: {}", e));
                        }
                        Err(e) => {
                            chat_state.add_system_message(format!(
                                "MCP delete task failed for '{}': {}",
                                server_id, e
                            ));
                        }
                    }

                    chat_view.mcp_selector_set_loading(None);
                    let updated_items = self.get_mcp_items(rt_handle);
                    if updated_items.is_empty() {
                        chat_view.hide_mcp_selector();
                    } else {
                        chat_view.mcp_selector_update_items(updated_items);
                    }
                }
            }
        }
        changed
    }

    /// Schedule adding a new MCP server (deferred to allow loading state to render)
    fn add_mcp_server(&mut self, name: &str, config_json_str: &str, chat_view: &mut ChatView) {
        if self.pending_mcp_op.is_some() || self.has_pending_mcp_add_task() {
            return;
        }

        chat_view.set_status(Some(format!("Adding MCP server '{}'...", name)));
        self.pending_mcp_op = Some(PendingMcpOp::Add {
            name: name.to_string(),
            config_json: config_json_str.to_string(),
        });
    }

    /// Execute MCP server add (called from main loop after render)
    fn execute_mcp_add(
        &mut self,
        name: &str,
        config_json_str: &str,
        _chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let mcp_service = match crate::get_mcp_service() {
            Some(svc) => svc.clone(),
            None => {
                chat_state.add_system_message("MCP service not initialized".to_string());
                return;
            }
        };

        let config_value: serde_json::Value = match serde_json::from_str(config_json_str) {
            Ok(v) => v,
            Err(e) => {
                chat_state.add_system_message(format!("Invalid JSON: {}", e));
                _chat_view.set_status(None);
                return;
            }
        };

        let name_owned = name.to_string();
        let task_name = name_owned.clone();
        let handle = rt_handle.spawn(async move {
            let config_obj = config_value.as_object().ok_or_else(|| {
                bitfun_core::util::errors::BitFunError::Validation(
                    "MCP server config must be a JSON object".to_string(),
                )
            })?;

            let server_type = match config_obj.get("type").and_then(|v| v.as_str()) {
                Some("sse") => bitfun_core::service::mcp::MCPServerType::Remote,
                Some("streamable-http") | Some("streamable_http") | Some("http") => {
                    bitfun_core::service::mcp::MCPServerType::Remote
                }
                _ => bitfun_core::service::mcp::MCPServerType::Local,
            };

            let transport = match config_obj.get("type").and_then(|v| v.as_str()) {
                Some("sse") => bitfun_core::service::mcp::MCPServerTransport::Sse,
                Some("streamable-http") | Some("streamable_http") | Some("http") => {
                    bitfun_core::service::mcp::MCPServerTransport::StreamableHttp
                }
                _ => bitfun_core::service::mcp::MCPServerTransport::Stdio,
            };

            let command = config_obj
                .get("command")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let args = config_obj
                .get("args")
                .and_then(|v| v.as_array())
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let env = config_obj
                .get("env")
                .and_then(|v| v.as_object())
                .map(|map| {
                    map.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect::<std::collections::HashMap<_, _>>()
                })
                .unwrap_or_default();
            let headers = config_obj
                .get("headers")
                .and_then(|v| v.as_object())
                .map(|map| {
                    map.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect::<std::collections::HashMap<_, _>>()
                })
                .unwrap_or_default();
            let url = config_obj
                .get("url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let auto_start = config_obj
                .get("autoStart")
                .or_else(|| config_obj.get("auto_start"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let enabled = config_obj
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            let config = bitfun_core::service::mcp::MCPServerConfig {
                id: name_owned.clone(),
                name: name_owned.clone(),
                server_type,
                transport: Some(transport),
                command,
                args,
                env,
                headers,
                url,
                auto_start,
                enabled,
                location: bitfun_core::service::mcp::ConfigLocation::User,
                capabilities: Vec::new(),
                settings: Default::default(),
                oauth: config_obj
                    .get("oauth")
                    .cloned()
                    .and_then(|value| serde_json::from_value(value).ok()),
                xaa: config_obj
                    .get("xaa")
                    .cloned()
                    .and_then(|value| serde_json::from_value(value).ok()),
            };

            mcp_service.server_manager().add_server(config).await?;

            Ok::<(), bitfun_core::util::errors::BitFunError>(())
        });
        self.pending_mcp_tasks.push(PendingMcpTask::Add {
            name: task_name,
            handle,
        });
    }

    /// Schedule deleting an MCP server (deferred to allow loading state to render)
    fn delete_mcp_server(&mut self, server_id: &str, chat_view: &mut ChatView) {
        if self.pending_mcp_op.is_some() || self.is_mcp_server_task_running(server_id) {
            return;
        }

        chat_view.mcp_selector_set_loading(Some(server_id.to_string()));
        chat_view.mcp_selector_cancel_confirm_delete();
        self.pending_mcp_op = Some(PendingMcpOp::Delete(server_id.to_string()));
    }

    /// Execute MCP server delete (called from main loop after render)
    fn execute_mcp_delete(
        &mut self,
        server_id: &str,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let mcp_service = match crate::get_mcp_service() {
            Some(svc) => svc.clone(),
            None => {
                chat_state.add_system_message("MCP service not initialized".to_string());
                chat_view.mcp_selector_set_loading(None);
                return;
            }
        };

        let server_id_owned = server_id.to_string();
        let task_server_id = server_id_owned.clone();
        let handle = rt_handle.spawn(async move {
            // Delete config first so UI can reflect removal immediately even if stop is blocked.
            mcp_service
                .config_service()
                .delete_server_config(&server_id_owned)
                .await?;

            // Best-effort async cleanup: slow startups may hold process write lock for a long time.
            // Retry stop with short timeout, without blocking the delete operation completion.
            let cleanup_service = mcp_service.clone();
            let cleanup_server_id = server_id_owned.clone();
            tokio::spawn(async move {
                for attempt in 1..=20 {
                    let stop_result = tokio::time::timeout(
                        Duration::from_millis(250),
                        cleanup_service
                            .server_manager()
                            .stop_server(&cleanup_server_id),
                    )
                    .await;

                    match stop_result {
                        Ok(Ok(())) => return,
                        Ok(Err(bitfun_core::util::errors::BitFunError::NotFound(_))) => return,
                        Ok(Err(e)) => {
                            tracing::debug!(
                                "Best-effort MCP stop failed: id={} attempt={} error={}",
                                cleanup_server_id,
                                attempt,
                                e
                            );
                        }
                        Err(_) => {
                            tracing::debug!(
                                "Best-effort MCP stop timed out: id={} attempt={}",
                                cleanup_server_id,
                                attempt
                            );
                        }
                    }

                    tokio::time::sleep(Duration::from_millis(250)).await;
                }

                tracing::warn!(
                    "Best-effort MCP stop exhausted retries: id={}",
                    cleanup_server_id
                );
            });

            Ok::<(), bitfun_core::util::errors::BitFunError>(())
        });

        self.pending_mcp_tasks.push(PendingMcpTask::Delete {
            server_id: task_server_id,
            handle,
        });
    }

    /// Open MCP config file in system editor or show its path
    fn open_mcp_config(&self, chat_state: &mut ChatState) {
        match bitfun_core::infrastructure::try_get_path_manager_arc() {
            Ok(path_manager) => {
                let config_file = path_manager.app_config_file();
                chat_state.add_system_message(format!(
                    "MCP servers are configured in:\n  {}\n\n\
                     Edit the \"mcp_servers\" section. Example (Cursor format):\n\
                     {{\n  \"mcp_servers\": {{\n    \"mcpServers\": {{\n      \
                     \"my-server\": {{\n        \"type\": \"stdio\",\n        \
                     \"command\": \"npx\",\n        \"args\": [\"-y\", \"@modelcontextprotocol/server-xxx\"]\n      \
                     }}\n    }}\n  }}\n}}",
                    config_file.display()
                ));
            }
            Err(_) => {
                chat_state.add_system_message(
                    "Could not determine config file path. Check ~/.config/bitfun/config/app.json"
                        .to_string(),
                );
            }
        }
    }

}
