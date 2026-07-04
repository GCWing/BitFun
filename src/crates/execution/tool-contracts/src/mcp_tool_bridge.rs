use crate::{DynamicMcpToolInfo, DynamicToolInfo, ToolResult, ValidationResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MCP_TOOL_PREFIX: &str = "mcp__";
pub const MCP_TOOL_DELIMITER: &str = "__";

/// Normalize MCP server/tool names to the prompt-visible dynamic-tool format.
pub fn normalize_name_for_mcp(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub fn build_mcp_tool_bridge_name(server_id: &str, tool_name: &str) -> String {
    format!(
        "{}{}{}{}",
        MCP_TOOL_PREFIX,
        normalize_name_for_mcp(server_id),
        MCP_TOOL_DELIMITER,
        normalize_name_for_mcp(tool_name)
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpToolBridgeToolInfo {
    pub server_id: String,
    pub server_name: String,
    pub tool_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpToolBridgeDefinition {
    pub full_name: String,
    pub title: String,
    pub user_facing_name: String,
    pub description: String,
    pub provider_id: String,
    pub provider_kind: String,
    pub tool_info: McpToolBridgeToolInfo,
    pub read_only: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct McpToolBridgeBehaviorHints {
    pub read_only: bool,
    pub destructive: bool,
    pub open_world: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct McpToolBridgeDefinitionInput<'a> {
    pub server_id: &'a str,
    pub server_name: &'a str,
    pub tool_name: &'a str,
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub behavior_hints: McpToolBridgeBehaviorHints,
}

pub fn build_mcp_tool_bridge_definition(
    input: McpToolBridgeDefinitionInput<'_>,
) -> McpToolBridgeDefinition {
    let mut description = format!(
        "Tool '{}' from MCP server '{}': {}",
        input.title,
        input.server_name,
        input.description.unwrap_or("")
    );

    let hints = mcp_tool_bridge_behavior_hint_labels(input.behavior_hints);
    if !hints.is_empty() {
        description.push_str(&format!(" [Hints: {}]", hints.join(", ")));
    }

    McpToolBridgeDefinition {
        full_name: build_mcp_tool_bridge_name(input.server_id, input.tool_name),
        title: input.title.to_string(),
        user_facing_name: format!("{} ({})", input.title, input.server_name),
        description,
        provider_id: input.server_id.to_string(),
        provider_kind: "mcp".to_string(),
        tool_info: McpToolBridgeToolInfo {
            server_id: input.server_id.to_string(),
            server_name: input.server_name.to_string(),
            tool_name: input.tool_name.to_string(),
        },
        read_only: input.behavior_hints.read_only,
    }
}

pub fn mcp_tool_bridge_short_description(
    tool_description: Option<&str>,
    server_name: &str,
) -> String {
    let summary = tool_description
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("MCP tool");
    format!("{} ({})", summary, server_name)
}

pub fn mcp_tool_bridge_dynamic_tool_info(definition: &McpToolBridgeDefinition) -> DynamicToolInfo {
    DynamicToolInfo {
        provider_id: definition.provider_id.clone(),
        provider_kind: Some(definition.provider_kind.clone()),
        mcp: Some(DynamicMcpToolInfo {
            server_id: definition.tool_info.server_id.clone(),
            server_name: definition.tool_info.server_name.clone(),
            tool_name: definition.tool_info.tool_name.clone(),
        }),
    }
}

pub fn validate_mcp_tool_bridge_input(
    input: &Value,
    server_name: &str,
    blocked_in_context: bool,
) -> ValidationResult {
    if blocked_in_context {
        return ValidationResult {
            result: false,
            message: Some(format!(
                "MCP server '{}' runs locally and is unavailable in remote workspace sessions",
                server_name
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

    ValidationResult::default()
}

pub fn render_mcp_tool_bridge_use_message(title: &str, server_name: &str, input: &Value) -> String {
    format!(
        "Using MCP tool '{}' from '{}' with input: {}",
        title, server_name, input
    )
}

pub fn render_mcp_tool_bridge_rejected_message(title: &str, server_name: &str) -> String {
    format!(
        "MCP tool '{}' from '{}' was rejected by user",
        title, server_name
    )
}

pub fn render_mcp_tool_bridge_result_message(title: &str, rendered_result: &str) -> String {
    format!(
        "MCP tool '{}' completed. Result: {}",
        title, rendered_result
    )
}

pub fn build_mcp_tool_bridge_result(data: Value, result_for_assistant: String) -> ToolResult {
    ToolResult::Result {
        data,
        result_for_assistant: Some(result_for_assistant),
        image_attachments: None,
    }
}

fn mcp_tool_bridge_behavior_hint_labels(hints: McpToolBridgeBehaviorHints) -> Vec<&'static str> {
    let mut labels = Vec::new();
    if hints.read_only {
        labels.push("read-only");
    }
    if hints.destructive {
        labels.push("destructive");
    }
    if hints.open_world {
        labels.push("open-world");
    }
    labels
}
