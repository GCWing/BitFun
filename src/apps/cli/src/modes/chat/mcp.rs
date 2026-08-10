use bitfun_app_server_protocol::mcp::{
    ExternalMcpDecisionRequest, McpConflictChoiceRequest, McpServerAction, McpServerMutation,
    McpServerSummary, McpTransport,
};

fn bounded_mcp_terminal_text(value: &str) -> String {
    let escaped = crate::plugin_diagnostics::escape_terminal_text(value);
    let mut chars = escaped.chars();
    let bounded = chars.by_ref().take(512).collect::<String>();
    if chars.next().is_some() {
        format!("{bounded}…")
    } else {
        bounded
    }
}

fn mcp_item_from_summary(server: McpServerSummary) -> McpItem {
    let action = match server.action {
        McpServerAction::NativeToggle => McpItemAction::NativeToggle,
        McpServerAction::ReadOnly { reason } => McpItemAction::ReadOnly {
            reason: bounded_mcp_terminal_text(&reason),
        },
        McpServerAction::ExternalDecision {
            candidate_id,
            decision_key,
            approved,
            expected_mcp_generation,
            expected_preference_revision,
        } => McpItemAction::ExternalDecision {
            candidate_id,
            decision_key,
            approved,
            expected_mcp_generation,
            expected_preference_revision,
        },
        McpServerAction::ConflictChoice {
            conflict_key,
            candidate_id,
            approve_external,
            expected_mcp_generation,
            expected_preference_revision,
        } => McpItemAction::ConflictChoice {
            conflict_key,
            candidate_id,
            approve_external,
            expected_mcp_generation,
            expected_preference_revision,
        },
    };
    McpItem {
        id: server.id,
        name: bounded_mcp_terminal_text(&server.name),
        server_type: bounded_mcp_terminal_text(&server.server_type),
        status: bounded_mcp_terminal_text(&server.status),
        tool_count: server.tool_count,
        source_label: bounded_mcp_terminal_text(&server.source_label),
        external: server.external,
        detail: bounded_mcp_terminal_text(&server.detail),
        action,
    }
}

/// Completion message for the `/mcp` add flow. In Shared TUI mode the add
/// mutates the local MCP compatibility owner of this CLI process, not the
/// already-running Shared Runtime Host, so it must not be reported as
/// "started" for that Runtime.
fn mcp_add_completion_message(name: &str, shared: bool) -> String {
    if shared {
        format!(
            "MCP server '{name}' added to the local compatibility owner; the Shared Runtime Host is not reconfigured"
        )
    } else {
        format!("MCP server '{name}' added and started")
    }
}

impl ChatMode {
    fn show_mcp_selector(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        match tokio::task::block_in_place(|| rt_handle.block_on(self.agent.list_mcp_servers())) {
            Ok(response) => chat_view.show_mcp_selector(
                response
                    .servers
                    .into_iter()
                    .map(mcp_item_from_summary)
                    .collect(),
            ),
            Err(error) => {
                chat_state.add_system_message(format!("Could not load MCP servers: {error}"));
                chat_view.show_mcp_selector(Vec::new());
            }
        }
    }

    pub(super) fn get_mcp_items(&self, rt_handle: &tokio::runtime::Handle) -> Vec<McpItem> {
        tokio::task::block_in_place(|| rt_handle.block_on(self.agent.list_mcp_servers()))
            .map(|response| {
                response
                    .servers
                    .into_iter()
                    .map(mcp_item_from_summary)
                    .collect()
            })
            .unwrap_or_else(|error| {
                tracing::warn!("Failed to load MCP server catalog: {error}");
                Vec::new()
            })
    }

    fn activate_mcp_item(
        &mut self,
        item: McpItem,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
    ) {
        match &item.action {
            McpItemAction::NativeToggle => self.toggle_mcp_server(&item.id, chat_view),
            McpItemAction::ReadOnly { reason } => {
                chat_state.add_system_message(format!("{}: {}", item.name, reason));
            }
            McpItemAction::ExternalDecision { .. } | McpItemAction::ConflictChoice { .. } => {
                if self.pending_mcp_op.is_some() || self.is_mcp_server_task_running(&item.id) {
                    return;
                }
                chat_view.mcp_selector_set_loading(Some(item.id.clone()));
                self.pending_mcp_op = Some(PendingMcpOp::External(item));
            }
        }
    }

    fn toggle_mcp_server(&mut self, server_id: &str, chat_view: &mut ChatView) {
        if self.pending_mcp_op.is_some() || self.is_mcp_server_task_running(server_id) {
            return;
        }
        chat_view.mcp_selector_set_loading(Some(server_id.to_string()));
        self.pending_mcp_op = Some(PendingMcpOp::Toggle(server_id.to_string()));
    }

    fn execute_mcp_toggle(
        &mut self,
        server_id: &str,
        _chat_view: &mut ChatView,
        _chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let agent = Arc::clone(&self.agent);
        let server_id = server_id.to_string();
        let tracked_server_id = server_id.clone();
        let handle = rt_handle.spawn(async move {
            agent
                .toggle_mcp_server(server_id)
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        });
        self.pending_mcp_tasks.push(PendingMcpTask::Toggle {
            server_id: tracked_server_id,
            handle,
        });
    }

    fn execute_external_mcp_action(
        &mut self,
        item: McpItem,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let agent = Arc::clone(&self.agent);
        let workspace_path = self.agent.workspace_path_string();
        let action = item.action.clone();
        let item_id = item.id.clone();
        let item_name = item.name.clone();
        let handle = rt_handle.spawn(async move {
            match action {
                McpItemAction::ExternalDecision {
                    candidate_id,
                    decision_key,
                    approved,
                    expected_mcp_generation,
                    expected_preference_revision,
                } => agent
                    .external_mcp_decision(ExternalMcpDecisionRequest {
                        workspace_path,
                        candidate_id,
                        decision_key,
                        approved,
                        expected_mcp_generation,
                        expected_preference_revision,
                    })
                    .await
                    .map(|_| ()),
                McpItemAction::ConflictChoice {
                    conflict_key,
                    candidate_id,
                    approve_external,
                    expected_mcp_generation,
                    expected_preference_revision,
                } => agent
                    .mcp_conflict_choice(McpConflictChoiceRequest {
                        workspace_path,
                        conflict_key,
                        candidate_id,
                        approve_external,
                        expected_mcp_generation,
                        expected_preference_revision,
                    })
                    .await
                    .map(|_| ()),
                McpItemAction::NativeToggle | McpItemAction::ReadOnly { .. } => Err(anyhow!(
                    "The MCP action is no longer available; reopen /mcp"
                )),
            }
            .map_err(|error| error.to_string())
        });
        self.pending_mcp_tasks.push(PendingMcpTask::External {
            item_id,
            item_name,
            handle,
        });
        chat_state.add_system_message(
            "Saving the MCP server choice. Existing sessions continue running while it is applied."
                .to_string(),
        );
        chat_view.mcp_selector_cancel_confirm_external();
    }

    fn is_mcp_server_task_running(&self, server_id: &str) -> bool {
        self.pending_mcp_tasks.iter().any(|task| match task {
            PendingMcpTask::Toggle { server_id: id, .. }
            | PendingMcpTask::Delete { server_id: id, .. } => id == server_id,
            PendingMcpTask::Add { .. } => false,
            PendingMcpTask::External { item_id, .. } => item_id == server_id,
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
        let mut index = 0;
        while index < self.pending_mcp_tasks.len() {
            let finished = match &self.pending_mcp_tasks[index] {
                PendingMcpTask::Toggle { handle, .. }
                | PendingMcpTask::Add { handle, .. }
                | PendingMcpTask::Delete { handle, .. }
                | PendingMcpTask::External { handle, .. } => handle.is_finished(),
            };
            if !finished {
                index += 1;
                continue;
            }

            changed = true;
            let task = self.pending_mcp_tasks.swap_remove(index);
            let (success_message, failure_context, result) = match task {
                PendingMcpTask::Toggle { server_id, handle } => (
                    None,
                    format!("toggle MCP server '{server_id}'"),
                    tokio::task::block_in_place(|| rt_handle.block_on(handle)),
                ),
                PendingMcpTask::Add { name, handle } => (
                    Some(mcp_add_completion_message(&name, self.agent.is_shared())),
                    format!("add MCP server '{name}'"),
                    tokio::task::block_in_place(|| rt_handle.block_on(handle)),
                ),
                PendingMcpTask::Delete { server_id, handle } => (
                    Some(format!("MCP server '{server_id}' deleted")),
                    format!("delete MCP server '{server_id}'"),
                    tokio::task::block_in_place(|| rt_handle.block_on(handle)),
                ),
                PendingMcpTask::External {
                    item_name, handle, ..
                } => (
                    Some(format!("MCP server choice saved for '{item_name}'")),
                    format!("save the MCP server choice for '{item_name}'"),
                    tokio::task::block_in_place(|| rt_handle.block_on(handle)),
                ),
            };
            match result {
                Ok(Ok(())) => {
                    if let Some(message) = success_message {
                        chat_state.add_system_message(message);
                    }
                }
                Ok(Err(error)) => {
                    chat_state.add_system_message(format!("Could not {failure_context}: {error}"))
                }
                Err(error) => chat_state.add_system_message(format!(
                    "MCP task failed while trying to {failure_context}: {error}"
                )),
            }
            chat_view.set_status(None);
            chat_view.mcp_selector_set_loading(None);
            chat_view.mcp_selector_update_items(self.get_mcp_items(rt_handle));
        }
        changed
    }

    fn add_mcp_server(&mut self, name: &str, config_json_str: &str, chat_view: &mut ChatView) {
        if self.pending_mcp_op.is_some() || self.has_pending_mcp_add_task() {
            return;
        }
        chat_view.set_status(Some(format!("Adding MCP server '{name}'...")));
        self.pending_mcp_op = Some(PendingMcpOp::Add {
            name: name.to_string(),
            config_json: config_json_str.to_string(),
        });
    }

    fn execute_mcp_add(
        &mut self,
        name: &str,
        config_json_str: &str,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let value: serde_json::Value = match serde_json::from_str(config_json_str) {
            Ok(config) => config,
            Err(error) => {
                chat_state.add_system_message(format!("Invalid JSON: {error}"));
                chat_view.set_status(None);
                return;
            }
        };
        let Some(config) = value.as_object() else {
            chat_state.add_system_message("MCP server config must be a JSON object".to_string());
            chat_view.set_status(None);
            return;
        };
        let string_map = |key: &str| {
            config
                .get(key)
                .and_then(serde_json::Value::as_object)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|(key, value)| {
                            value.as_str().map(|value| (key.clone(), value.to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        let transport = match config.get("type").and_then(serde_json::Value::as_str) {
            Some("sse") => McpTransport::Sse,
            Some("streamable-http" | "streamable_http" | "http") => McpTransport::StreamableHttp,
            _ => McpTransport::Stdio,
        };
        let mutation = McpServerMutation {
            transport,
            command: config
                .get("command")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            args: config
                .get("args")
                .and_then(serde_json::Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| value.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            env: string_map("env"),
            headers: string_map("headers"),
            url: config
                .get("url")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            auto_start: config
                .get("autoStart")
                .or_else(|| config.get("auto_start"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
            enabled: config
                .get("enabled")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
            oauth: config.get("oauth").cloned(),
            xaa: config.get("xaa").cloned(),
        };
        let agent = Arc::clone(&self.agent);
        let name = name.to_string();
        let task_name = name.clone();
        let handle = rt_handle.spawn(async move {
            agent
                .add_mcp_server(name, mutation)
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        });
        self.pending_mcp_tasks.push(PendingMcpTask::Add {
            name: task_name,
            handle,
        });
    }

    fn delete_mcp_server(&mut self, server_id: &str, chat_view: &mut ChatView) {
        if self.pending_mcp_op.is_some() || self.is_mcp_server_task_running(server_id) {
            return;
        }
        chat_view.mcp_selector_set_loading(Some(server_id.to_string()));
        chat_view.mcp_selector_cancel_confirm_delete();
        self.pending_mcp_op = Some(PendingMcpOp::Delete(server_id.to_string()));
    }

    fn execute_mcp_delete(
        &mut self,
        server_id: &str,
        _chat_view: &mut ChatView,
        _chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let agent = Arc::clone(&self.agent);
        let server_id = server_id.to_string();
        let task_server_id = server_id.clone();
        let handle = rt_handle.spawn(async move {
            agent
                .delete_mcp_server(server_id)
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        });
        self.pending_mcp_tasks.push(PendingMcpTask::Delete {
            server_id: task_server_id,
            handle,
        });
    }

    fn open_mcp_config(&self, chat_state: &mut ChatState, rt_handle: &tokio::runtime::Handle) {
        let config_path = tokio::task::block_in_place(|| {
            rt_handle
                .block_on(self.agent.list_mcp_servers())
                .ok()
                .and_then(|response| response.config_path)
        });
        match config_path {
            Some(config_path) => chat_state.add_system_message(format!(
                "MCP servers are configured in:\n  {config_path}\n\nEdit the \"mcp_servers\" section."
            )),
            None => chat_state.add_system_message(
                "The MCP configuration path is unavailable from this Host.".to_string(),
            ),
        }
    }
}

#[cfg(test)]
mod mcp_terminal_tests {
    use super::*;

    #[test]
    fn mcp_summary_text_is_terminal_safe_and_bounded() {
        let item = mcp_item_from_summary(McpServerSummary {
            id: "server-id".to_string(),
            name: "unsafe\nname".to_string(),
            server_type: "local".to_string(),
            status: "Running\u{202e}".to_string(),
            tool_count: 1,
            source_label: "source\rlabel".to_string(),
            external: false,
            detail: "x".repeat(600),
            action: McpServerAction::ReadOnly {
                reason: "reason\ttext".to_string(),
            },
        });

        assert_eq!(item.name, "unsafe\\nname");
        assert_eq!(item.status, "Running\\u{202e}");
        assert_eq!(item.source_label, "source\\rlabel");
        assert_eq!(item.detail.chars().count(), 513);
        assert!(item.detail.ends_with('…'));
        assert!(matches!(
            item.action,
            McpItemAction::ReadOnly { ref reason } if reason == "reason\\ttext"
        ));
    }

    #[test]
    fn shared_add_does_not_report_runtime_started() {
        // In Shared TUI mode the add mutates this CLI process's local MCP
        // compatibility owner, not the already-running Shared Runtime Host,
        // so the completion message must not claim the server "started" for
        // that Runtime and must state the local scope.
        let shared = mcp_add_completion_message("srv", true);
        assert!(shared.contains("local compatibility owner"));
        assert!(shared.contains("Shared Runtime Host is not reconfigured"));
        assert!(!shared.contains("added and started"));

        // Embedded mode owns the runtime, so "added and started" stays accurate.
        assert_eq!(
            mcp_add_completion_message("srv", false),
            "MCP server 'srv' added and started"
        );
    }
}
