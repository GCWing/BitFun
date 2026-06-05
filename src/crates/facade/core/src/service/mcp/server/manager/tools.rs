use super::*;

impl MCPServerManager {
    pub(super) async fn refresh_mcp_tools(
        &self,
        server_id: &str,
        server_name: &str,
        connection: Arc<MCPConnection>,
    ) -> BitFunResult<usize> {
        Self::unregister_mcp_tools(server_id).await;
        Self::register_mcp_tools(server_id, server_name, connection).await
    }

    /// Registers MCP tools into the global tool registry.
    pub(super) async fn register_mcp_tools(
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
    pub(super) async fn unregister_mcp_tools(server_id: &str) {
        let registry = crate::agentic::tools::registry::get_global_tool_registry();
        let mut registry_lock = registry.write().await;
        registry_lock.unregister_mcp_server_tools(server_id);
        info!("Unregistered MCP tools: server_id={}", server_id);
    }
}
