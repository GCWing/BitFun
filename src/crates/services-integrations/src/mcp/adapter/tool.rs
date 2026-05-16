//! MCP dynamic tool metadata and result rendering helpers.

use crate::mcp::protocol::{MCPTool, MCPToolResult, MCPToolResultContent};
use crate::mcp::{build_mcp_tool_name, MCPRuntimeResult, McpToolInfo};
use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpDynamicToolDescriptor {
    pub full_name: String,
    pub title: String,
    pub user_facing_name: String,
    pub description: String,
    pub provider_id: String,
    pub provider_kind: String,
    pub tool_info: McpToolInfo,
    pub read_only: bool,
}

#[derive(Debug, Clone)]
pub struct MCPDynamicToolDefinition {
    pub mcp_tool: MCPTool,
    pub descriptor: McpDynamicToolDescriptor,
}

#[async_trait]
pub trait MCPToolCatalogClient: Send + Sync {
    async fn list_mcp_tools(&self) -> MCPRuntimeResult<Vec<MCPTool>>;
}

#[derive(Debug, Clone)]
pub struct MCPDynamicToolProvider {
    server_id: String,
    server_name: String,
}

impl MCPDynamicToolProvider {
    pub fn new(server_id: impl Into<String>, server_name: impl Into<String>) -> Self {
        Self {
            server_id: server_id.into(),
            server_name: server_name.into(),
        }
    }

    pub async fn load_tool_definitions(
        &self,
        client: &dyn MCPToolCatalogClient,
    ) -> MCPRuntimeResult<Vec<MCPDynamicToolDefinition>> {
        Ok(client
            .list_mcp_tools()
            .await?
            .into_iter()
            .map(|mcp_tool| {
                let descriptor =
                    build_mcp_tool_descriptor(&self.server_id, &self.server_name, &mcp_tool);
                MCPDynamicToolDefinition {
                    mcp_tool,
                    descriptor,
                }
            })
            .collect())
    }
}

fn tool_title(tool: &MCPTool) -> String {
    tool.annotations
        .as_ref()
        .and_then(|annotations| annotations.title.clone())
        .or_else(|| tool.title.clone())
        .unwrap_or_else(|| tool.name.clone())
}

fn behavior_hints(tool: &MCPTool) -> Vec<&'static str> {
    let annotations = tool.annotations.clone().unwrap_or_default();
    let mut hints = Vec::new();
    if annotations.read_only_hint.unwrap_or(false) {
        hints.push("read-only");
    }
    if annotations.destructive_hint.unwrap_or(false) {
        hints.push("destructive");
    }
    if annotations.open_world_hint.unwrap_or(false) {
        hints.push("open-world");
    }
    hints
}

pub fn build_mcp_tool_descriptor(
    server_id: &str,
    server_name: &str,
    tool: &MCPTool,
) -> McpDynamicToolDescriptor {
    let title = tool_title(tool);
    let mut description = format!(
        "Tool '{}' from MCP server '{}': {}",
        title,
        server_name,
        tool.description.as_deref().unwrap_or("")
    );

    let hints = behavior_hints(tool);
    if !hints.is_empty() {
        description.push_str(&format!(" [Hints: {}]", hints.join(", ")));
    }

    McpDynamicToolDescriptor {
        full_name: build_mcp_tool_name(server_id, &tool.name),
        title: title.clone(),
        user_facing_name: format!("{} ({})", title, server_name),
        description,
        provider_id: server_id.to_string(),
        provider_kind: "mcp".to_string(),
        tool_info: McpToolInfo {
            server_id: server_id.to_string(),
            server_name: server_name.to_string(),
            tool_name: tool.name.clone(),
        },
        read_only: tool
            .annotations
            .as_ref()
            .and_then(|annotations| annotations.read_only_hint)
            .unwrap_or(false),
    }
}

fn truncate_for_assistant(text: String, max_result_text_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_result_text_chars {
        return text;
    }

    let truncated: String = text.chars().take(max_result_text_chars).collect();
    format!(
        "{}\n[Result truncated: {} of {} characters shown]",
        truncated, max_result_text_chars, char_count
    )
}

pub fn render_mcp_tool_result_for_assistant(
    tool_name: &str,
    result: &MCPToolResult,
    max_result_text_chars: usize,
) -> String {
    if result.is_error {
        return format!("Error executing MCP tool '{}'", tool_name);
    }

    if let Some(contents) = result.content.as_ref() {
        let rendered = contents
            .iter()
            .map(|c| match c {
                MCPToolResultContent::Text { text } => text.clone(),
                MCPToolResultContent::Image { mime_type, .. } => {
                    format!("[Image: {}]", mime_type)
                }
                MCPToolResultContent::Audio { mime_type, .. } => {
                    format!("[Audio: {}]", mime_type)
                }
                MCPToolResultContent::ResourceLink { uri, name, .. } => name
                    .as_ref()
                    .map_or_else(|| uri.clone(), |n| format!("[Resource: {} ({})]", n, uri)),
                MCPToolResultContent::Resource { resource } => {
                    format!("[Resource: {}]", resource.uri)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        return truncate_for_assistant(rendered, max_result_text_chars);
    }

    if let Some(structured_content) = result.structured_content.as_ref() {
        return truncate_for_assistant(structured_content.to_string(), max_result_text_chars);
    }

    "MCP tool execution completed".to_string()
}
