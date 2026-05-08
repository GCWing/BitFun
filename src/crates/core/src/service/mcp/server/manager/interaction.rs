use super::*;

impl MCPServerManager {
    pub(super) fn detect_list_changed_kind(method: &str) -> Option<ListChangedKind> {
        match method {
            "notifications/tools/list_changed"
            | "notifications/tools/listChanged"
            | "tools/list_changed" => Some(ListChangedKind::Tools),
            "notifications/prompts/list_changed"
            | "notifications/prompts/listChanged"
            | "prompts/list_changed" => Some(ListChangedKind::Prompts),
            "notifications/resources/list_changed"
            | "notifications/resources/listChanged"
            | "resources/list_changed" => Some(ListChangedKind::Resources),
            _ => None,
        }
    }

    fn path_to_file_uri(path: &Path) -> Option<String> {
        reqwest::Url::from_directory_path(path)
            .ok()
            .map(|u| u.to_string())
    }

    fn build_roots_list_result() -> Value {
        let mut candidate_roots = Vec::new();

        if let Some(workspace_service) = get_global_workspace_service() {
            if let Some(workspace_root) = workspace_service.try_get_current_workspace_path() {
                candidate_roots.push(workspace_root);
            }
        }

        let mut seen_uris = HashSet::new();
        let mut roots = Vec::new();
        for root in candidate_roots {
            let Some(uri) = Self::path_to_file_uri(&root) else {
                continue;
            };
            if !seen_uris.insert(uri.clone()) {
                continue;
            }
            let name = root
                .file_name()
                .and_then(|v| v.to_str())
                .filter(|v| !v.is_empty())
                .unwrap_or("BitFun Workspace")
                .to_string();
            roots.push(json!({
                "uri": uri,
                "name": name,
            }));
        }

        json!({ "roots": roots })
    }

    async fn handle_server_request(
        &self,
        server_id: &str,
        server_name: &str,
        connection: Arc<MCPConnection>,
        request_id: Value,
        method: String,
        params: Option<Value>,
    ) {
        match method.as_str() {
            "ping" => {
                if let Err(e) = connection.send_response(request_id, json!({})).await {
                    warn!(
                        "Failed to respond to MCP ping request: server_name={} server_id={} error={}",
                        server_name, server_id, e
                    );
                }
            }
            "roots/list" => {
                let result = Self::build_roots_list_result();
                if let Err(e) = connection.send_response(request_id, result).await {
                    warn!(
                        "Failed to respond to MCP roots/list request: server_name={} server_id={} error={}",
                        server_name, server_id, e
                    );
                } else {
                    info!(
                        "Handled MCP roots/list request: server_name={} server_id={}",
                        server_name, server_id
                    );
                }
            }
            "elicitation/create" | "sampling/createMessage" => {
                self.handle_interactive_server_request(
                    server_id,
                    server_name,
                    connection,
                    request_id,
                    method,
                    params,
                )
                .await;
            }
            _ => {
                let error = MCPError::method_not_found(method.clone());
                if let Err(e) = connection.send_error(request_id, error).await {
                    warn!(
                        "Failed to respond with method_not_found for MCP request: server_name={} server_id={} method={} error={}",
                        server_name, server_id, method, e
                    );
                } else {
                    warn!(
                        "Rejected unsupported MCP server request: server_name={} server_id={} method={}",
                        server_name, server_id, method
                    );
                }
            }
        }
    }

    async fn handle_interactive_server_request(
        &self,
        server_id: &str,
        server_name: &str,
        connection: Arc<MCPConnection>,
        request_id: Value,
        method: String,
        params: Option<Value>,
    ) {
        let interaction_id = format!("mcp_interaction_{}", uuid::Uuid::new_v4());
        let (tx, rx) = oneshot::channel();

        {
            let mut pending = self.pending_interactions.write().await;
            pending.insert(interaction_id.clone(), PendingMCPInteraction { sender: tx });
        }

        let event_payload = json!({
            "interactionId": interaction_id,
            "serverId": server_id,
            "serverName": server_name,
            "method": method.clone(),
            "params": params,
        });

        let event_system = get_global_event_system();
        if let Err(e) = event_system
            .emit(BackendEvent::Custom {
                event_name: "backend-event-mcpinteractionrequest".to_string(),
                payload: event_payload,
            })
            .await
        {
            warn!(
                "Failed to emit MCP interaction request event: server_name={} server_id={} method={} error={}",
                server_name, server_id, method, e
            );
        }

        let decision = rx.await;
        {
            let mut pending = self.pending_interactions.write().await;
            pending.remove(&interaction_id);
        }

        match decision {
            Ok(MCPInteractionDecision::Accept { result }) => {
                if let Err(e) = connection.send_response(request_id, result).await {
                    warn!(
                        "Failed to send interactive MCP response: server_name={} server_id={} method={} error={}",
                        server_name, server_id, method, e
                    );
                } else {
                    info!(
                        "Handled interactive MCP request: server_name={} server_id={} method={}",
                        server_name, server_id, method
                    );
                }
            }
            Ok(MCPInteractionDecision::Reject { error }) => {
                if let Err(e) = connection.send_error(request_id, error).await {
                    warn!(
                        "Failed to send interactive MCP rejection: server_name={} server_id={} method={} error={}",
                        server_name, server_id, method, e
                    );
                } else {
                    info!(
                        "Rejected interactive MCP request: server_name={} server_id={} method={}",
                        server_name, server_id, method
                    );
                }
            }
            Err(_) => {
                let error = MCPError::internal_error(format!(
                    "MCP interaction channel closed before response: {}",
                    method
                ));
                if let Err(e) = connection.send_error(request_id, error).await {
                    warn!(
                        "Failed to send interaction channel-closed error: server_name={} server_id={} method={} error={}",
                        server_name, server_id, method, e
                    );
                }
            }
        }
    }

    pub async fn submit_interaction_response(
        &self,
        interaction_id: &str,
        approve: bool,
        result: Option<Value>,
        error_message: Option<String>,
        error_code: Option<i32>,
        error_data: Option<Value>,
    ) -> BitFunResult<()> {
        let pending = {
            let mut interactions = self.pending_interactions.write().await;
            interactions.remove(interaction_id)
        };

        let Some(pending) = pending else {
            return Err(BitFunError::NotFound(format!(
                "MCP interaction not found: {}",
                interaction_id
            )));
        };

        let decision = if approve {
            MCPInteractionDecision::Accept {
                result: result.unwrap_or_else(|| json!({})),
            }
        } else {
            MCPInteractionDecision::Reject {
                error: MCPError {
                    code: error_code.unwrap_or(MCPError::INVALID_REQUEST),
                    message: error_message
                        .unwrap_or_else(|| "User rejected MCP interaction request".to_string()),
                    data: error_data,
                },
            }
        };

        pending.sender.send(decision).map_err(|_| {
            BitFunError::MCPError(format!(
                "Failed to deliver MCP interaction response (receiver dropped): {}",
                interaction_id
            ))
        })?;

        Ok(())
    }

    pub(super) async fn start_connection_event_listener(
        &self,
        server_id: &str,
        server_name: &str,
        connection: Arc<MCPConnection>,
    ) {
        self.stop_connection_event_listener(server_id).await;

        let manager = self.clone();
        let server_id_owned = server_id.to_string();
        let server_name_owned = server_name.to_string();
        let mut rx = connection.subscribe_events();
        let connection_for_refresh = connection.clone();

        let handle = tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(MCPConnectionEvent::Notification { method, .. }) => {
                        match Self::detect_list_changed_kind(&method) {
                            Some(ListChangedKind::Tools) => {
                                info!(
                                    "Received MCP tools list-changed notification: server_name={} server_id={}",
                                    server_name_owned, server_id_owned
                                );
                                if let Err(e) = manager
                                    .refresh_mcp_tools(
                                        &server_id_owned,
                                        &server_name_owned,
                                        connection_for_refresh.clone(),
                                    )
                                    .await
                                {
                                    warn!(
                                        "Failed to refresh MCP tools after list-changed notification: server_name={} server_id={} error={}",
                                        server_name_owned, server_id_owned, e
                                    );
                                }
                            }
                            Some(ListChangedKind::Prompts) => {
                                info!(
                                    "Received MCP prompts list-changed notification: server_name={} server_id={}",
                                    server_name_owned, server_id_owned
                                );
                                if let Err(e) = manager
                                    .refresh_prompts_catalog(
                                        &server_id_owned,
                                        connection_for_refresh.clone(),
                                    )
                                    .await
                                {
                                    warn!(
                                        "Failed to refresh MCP prompts catalog after list-changed notification: server_name={} server_id={} error={}",
                                        server_name_owned, server_id_owned, e
                                    );
                                }
                            }
                            Some(ListChangedKind::Resources) => {
                                info!(
                                    "Received MCP resources list-changed notification: server_name={} server_id={}",
                                    server_name_owned, server_id_owned
                                );
                                if let Err(e) = manager
                                    .refresh_resources_catalog(
                                        &server_id_owned,
                                        connection_for_refresh.clone(),
                                    )
                                    .await
                                {
                                    warn!(
                                        "Failed to refresh MCP resources catalog after list-changed notification: server_name={} server_id={} error={}",
                                        server_name_owned, server_id_owned, e
                                    );
                                }
                            }
                            None => {
                                debug!(
                                    "Ignoring MCP notification from server: server_name={} server_id={} method={}",
                                    server_name_owned, server_id_owned, method
                                );
                            }
                        }
                    }
                    Ok(MCPConnectionEvent::Request {
                        request_id,
                        method,
                        params,
                    }) => {
                        manager
                            .handle_server_request(
                                &server_id_owned,
                                &server_name_owned,
                                connection_for_refresh.clone(),
                                request_id,
                                method,
                                params,
                            )
                            .await;
                    }
                    Ok(MCPConnectionEvent::Closed) => {
                        warn!(
                            "MCP connection event stream closed: server_name={} server_id={}",
                            server_name_owned, server_id_owned
                        );
                        break;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                        warn!(
                            "Dropped MCP connection events due to lag: server_name={} server_id={} dropped={}",
                            server_name_owned, server_id_owned, count
                        );
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        });

        let mut tasks = self.connection_event_tasks.write().await;
        tasks.insert(server_id.to_string(), handle);
    }

    pub(super) async fn stop_connection_event_listener(&self, server_id: &str) {
        let mut tasks = self.connection_event_tasks.write().await;
        if let Some(handle) = tasks.remove(server_id) {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MCPServerManager;

    #[test]
    fn roots_list_does_not_fallback_to_process_current_dir_without_workspace() {
        let result = MCPServerManager::build_roots_list_result();
        let roots = result
            .get("roots")
            .and_then(|value| value.as_array())
            .expect("roots should be an array");

        assert!(
            roots.is_empty(),
            "MCP roots/list must not expose the process current directory when no workspace is active"
        );
    }
}
