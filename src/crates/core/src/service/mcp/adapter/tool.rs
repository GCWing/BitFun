//! MCP tool adapter
//!
//! Wraps MCP tools as implementations of BitFun's `Tool` trait.

use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::service::mcp::protocol::{MCPTool, MCPToolResult};
use crate::service::mcp::server::connection::MCPConnection;
use crate::util::errors::BitFunResult;
use async_trait::async_trait;
use log::{debug, error, info, warn};
use serde_json::Value;
use std::sync::Arc;

/// MCP tool wrapper that adapts an MCP tool to BitFun's `Tool`.
pub struct MCPToolWrapper {
    mcp_tool: MCPTool,
    connection: Arc<MCPConnection>,
    server_name: String,
    full_name: String,
}

impl MCPToolWrapper {
    /// Creates a new MCP tool wrapper.
    pub fn new(
        mcp_tool: MCPTool,
        connection: Arc<MCPConnection>,
        server_id: String,
        server_name: String,
    ) -> Self {
        let full_name = format!("mcp_{}_{}", server_id, mcp_tool.name);
        Self {
            mcp_tool,
            connection,
            server_name,
            full_name,
        }
    }
}

#[async_trait]
impl Tool for MCPToolWrapper {
    fn name(&self) -> &str {
        // Use server_id as a prefix to avoid naming conflicts.
        // Example: mcp_github_search_repos
        &self.full_name
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(format!(
            "Tool '{}' from MCP server '{}': {}",
            self.mcp_tool.name,
            self.server_name,
            self.mcp_tool.description.as_deref().unwrap_or("")
        ))
    }

    fn input_schema(&self) -> Value {
        self.mcp_tool.input_schema.clone()
    }

    fn ui_resource_uri(&self) -> Option<String> {
        self.mcp_tool
            .meta
            .as_ref()
            .and_then(|m| m.ui.as_ref())
            .and_then(|u| u.resource_uri.clone())
    }

    fn user_facing_name(&self) -> String {
        format!("{} ({})", self.mcp_tool.name, self.server_name)
    }

    async fn is_enabled(&self) -> bool {
        true
    }

    fn is_readonly(&self) -> bool {
        // MCP tools are non-readonly by default (requires permission confirmation).
        false
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        // MCP tools require permissions by default.
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        if !input.is_object() {
            return ValidationResult {
                result: false,
                message: Some("Input must be an object".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    fn render_result_for_assistant(&self, output: &Value) -> String {
        if let Ok(result) = serde_json::from_value::<MCPToolResult>(output.clone()) {
            if result.is_error {
                return format!("Error executing MCP tool '{}'", self.mcp_tool.name);
            }

            if let Some(contents) = result.content {
                return contents
                    .iter()
                    .map(|c| match c {
                        crate::service::mcp::protocol::MCPToolResultContent::Text { text } => {
                            text.clone()
                        }
                        crate::service::mcp::protocol::MCPToolResultContent::Image {
                            mime_type,
                            ..
                        } => format!("[Image: {}]", mime_type),
                        crate::service::mcp::protocol::MCPToolResultContent::Audio {
                            mime_type,
                            ..
                        } => format!("[Audio: {}]", mime_type),
                        crate::service::mcp::protocol::MCPToolResultContent::ResourceLink {
                            uri, name, ..
                        } => name.as_ref().map_or_else(|| uri.clone(), |n| format!("[Resource: {} ({})]", n, uri)),
                        crate::service::mcp::protocol::MCPToolResultContent::Resource { resource } => {
                            format!("[Resource: {}]", resource.uri)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
            }
        }

        "MCP tool execution completed".to_string()
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        format!(
            "Using MCP tool '{}' from '{}' with input: {}",
            self.mcp_tool.name, self.server_name, input
        )
    }

    fn render_tool_use_rejected_message(&self) -> String {
        format!(
            "MCP tool '{}' from '{}' was rejected by user",
            self.mcp_tool.name, self.server_name
        )
    }

    fn render_tool_result_message(&self, output: &Value) -> String {
        format!(
            "MCP tool '{}' completed. Result: {}",
            self.mcp_tool.name,
            self.render_result_for_assistant(output)
        )
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        info!(
            "Calling MCP tool: {} from server: {}",
            self.mcp_tool.name, self.server_name
        );
        debug!(
            "Input: {}",
            serde_json::to_string_pretty(input).unwrap_or_else(|_| "invalid json".to_string())
        );

        let start = std::time::Instant::now();

        let result = self
            .connection
            .call_tool(&self.mcp_tool.name, Some(input.clone()))
            .await?;

        let elapsed = start.elapsed();
        debug!("MCP tool returned after {:?}", elapsed);

        let result_value = serde_json::to_value(&result)?;

        let result_for_assistant = self.render_result_for_assistant(&result_value);
        Ok(vec![ToolResult::Result {
            data: result_value,
            result_for_assistant: Some(result_for_assistant),
        }])
    }
}

/// MCP tool adapter that manages multiple MCP tool wrappers.
pub struct MCPToolAdapter {
    tools: Vec<Arc<dyn Tool>>,
}

impl MCPToolAdapter {
    /// Creates a new tool adapter.
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// Loads tools from an MCP server.
    pub async fn load_tools_from_server(
        &mut self,
        server_id: &str,
        server_name: &str,
        connection: Arc<MCPConnection>,
    ) -> BitFunResult<()> {
        info!(
            "Loading tools from MCP server: {} (id={})",
            server_name, server_id
        );

        let result = connection.list_tools(None).await.map_err(|e| {
            error!("list_tools call failed: {}", e);
            e
        })?;

        info!(
            "Found {} MCP tool(s) from server {}",
            result.tools.len(),
            server_name
        );

        if result.tools.is_empty() {
            warn!("Server {} provided no tools", server_name);
            return Ok(());
        }

        for mcp_tool in result.tools.into_iter() {
            let wrapper = Arc::new(MCPToolWrapper::new(
                mcp_tool,
                connection.clone(),
                server_id.to_string(),
                server_name.to_string(),
            ));
            self.tools.push(wrapper);
        }

        info!(
            "Tool loading complete, adapter now has {} tool(s)",
            self.tools.len()
        );
        Ok(())
    }

    /// Returns all tools.
    pub fn get_tools(&self) -> &[Arc<dyn Tool>] {
        &self.tools
    }

    /// Clears all tools.
    pub fn clear(&mut self) {
        self.tools.clear();
    }
}

impl Default for MCPToolAdapter {
    fn default() -> Self {
        Self::new()
    }
}
