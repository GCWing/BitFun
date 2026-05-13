//! MCP tool adapter
//!
//! Wraps MCP tools as implementations of BitFun's `Tool` trait.

use crate::agentic::tools::framework::{
    DynamicMcpToolInfo, DynamicToolInfo, Tool, ToolRenderOptions, ToolResult, ToolUseContext,
    ValidationResult,
};
use crate::service::mcp::protocol::{MCPTool, MCPToolResult};
use crate::service::mcp::server::MCPConnection;
use crate::util::errors::BitFunResult;
use async_trait::async_trait;
use bitfun_services_integrations::mcp::adapter::{
    build_mcp_tool_descriptor, render_mcp_tool_result_for_assistant, MCPDynamicToolProvider,
    McpDynamicToolDescriptor,
};
use log::{debug, error, info, warn};
use serde_json::Value;
use std::sync::Arc;

/// MCP tool wrapper that adapts an MCP tool to BitFun's `Tool`.
pub struct MCPToolWrapper {
    mcp_tool: MCPTool,
    connection: Arc<MCPConnection>,
    server_id: String,
    server_name: String,
    descriptor: McpDynamicToolDescriptor,
}

impl MCPToolWrapper {
    const MAX_RESULT_TEXT_CHARS: usize = 12_000;

    /// Creates a new MCP tool wrapper.
    pub fn new(
        mcp_tool: MCPTool,
        connection: Arc<MCPConnection>,
        server_id: String,
        server_name: String,
    ) -> Self {
        let descriptor = build_mcp_tool_descriptor(&server_id, &server_name, &mcp_tool);
        Self {
            mcp_tool,
            connection,
            server_id,
            server_name,
            descriptor,
        }
    }

    fn tool_title(&self) -> String {
        self.descriptor.title.clone()
    }

    fn is_blocked_in_context(&self, _context: Option<&ToolUseContext>) -> bool {
        false
    }
}

#[async_trait]
impl Tool for MCPToolWrapper {
    fn name(&self) -> &str {
        // Use server_id as a prefix to avoid naming conflicts.
        // Example: mcp__github__search_repos
        &self.descriptor.full_name
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(self.descriptor.description.clone())
    }

    fn short_description(&self) -> String {
        let summary = self
            .mcp_tool
            .description
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("MCP tool");
        format!("{} ({})", summary, self.server_name)
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

    fn dynamic_provider_id(&self) -> Option<&str> {
        Some(&self.server_id)
    }

    fn user_facing_name(&self) -> String {
        self.descriptor.user_facing_name.clone()
    }

    fn dynamic_tool_info(&self) -> Option<DynamicToolInfo> {
        Some(DynamicToolInfo {
            provider_id: self.descriptor.provider_id.clone(),
            provider_kind: Some(self.descriptor.provider_kind.clone()),
            mcp: Some(DynamicMcpToolInfo {
                server_id: self.descriptor.tool_info.server_id.clone(),
                server_name: self.descriptor.tool_info.server_name.clone(),
                tool_name: self.descriptor.tool_info.tool_name.clone(),
            }),
        })
    }

    async fn is_enabled(&self) -> bool {
        true
    }

    async fn is_available_in_context(&self, context: Option<&ToolUseContext>) -> bool {
        !self.is_blocked_in_context(context)
    }

    fn is_readonly(&self) -> bool {
        self.descriptor.read_only
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        self.is_readonly()
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        !self.is_readonly()
    }

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        if self.is_blocked_in_context(context) {
            return ValidationResult {
                result: false,
                message: Some(format!(
                    "MCP server '{}' runs locally and is unavailable in remote workspace sessions",
                    self.server_name
                )),
                error_code: Some(400),
                meta: None,
            };
        }

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
            return render_mcp_tool_result_for_assistant(
                &self.mcp_tool.name,
                &result,
                Self::MAX_RESULT_TEXT_CHARS,
            );
        }

        "MCP tool execution completed".to_string()
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        format!(
            "Using MCP tool '{}' from '{}' with input: {}",
            self.tool_title(),
            self.server_name,
            input
        )
    }

    fn render_tool_use_rejected_message(&self) -> String {
        format!(
            "MCP tool '{}' from '{}' was rejected by user",
            self.tool_title(),
            self.server_name
        )
    }

    fn render_tool_result_message(&self, output: &Value) -> String {
        format!(
            "MCP tool '{}' completed. Result: {}",
            self.tool_title(),
            self.render_result_for_assistant(output)
        )
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let _ = context;

        info!(
            "Calling MCP tool: {} from server: {}",
            self.tool_title(),
            self.server_name
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
            image_attachments: None,
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

        let provider = MCPDynamicToolProvider::new(server_id, server_name);
        let definitions = provider
            .load_tool_definitions(connection.as_ref())
            .await
            .map_err(|e| {
                error!("list_tools call failed: {}", e);
                crate::util::errors::BitFunError::from(e)
            })?;

        info!(
            "Found {} MCP tool(s) from server {}",
            definitions.len(),
            server_name
        );

        if definitions.is_empty() {
            warn!("Server {} provided no tools", server_name);
            return Ok(());
        }

        for definition in definitions.into_iter() {
            let wrapper = Arc::new(MCPToolWrapper::new(
                definition.mcp_tool,
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
