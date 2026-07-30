//! Shared MCP helper for UI verification tools.
//!
//! All three tools (`verify_ui`, `get_ui_verification_log`, `save_ui_screenshot`)
//! proxy to the `ui-verification-mcp` MCP server. Configure it in app settings
//! (same as `deveco-mcp`) with the `ui-verification-mcp` server id.

use crate::service::mcp::get_global_mcp_service;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_services_integrations::mcp::protocol::{MCPToolResult, MCPToolResultContent};
use serde_json::Value;

pub(crate) const MCP_SERVER_ID: &str = "ui-verification-mcp";

/// Call a tool on the `ui-verification-mcp` MCP server and return the text output.
pub(crate) async fn call_ui_verification_mcp(
    tool_name: &str,
    args: Value,
) -> BitFunResult<String> {
    let mcp_service = get_global_mcp_service().ok_or_else(|| {
        BitFunError::tool("MCP service is not initialized".to_string())
    })?;

    let connection = mcp_service
        .server_manager()
        .get_connection(MCP_SERVER_ID)
        .await
        .ok_or_else(|| {
            BitFunError::tool(format!(
                "MCP server '{}' is not connected. Configure it in app settings.",
                MCP_SERVER_ID
            ))
        })?;

    let result = connection
        .call_tool(tool_name, Some(args))
        .await
        .map_err(|e| BitFunError::tool(format!("MCP {} call failed: {}", tool_name, e)))?;

    if result.is_error {
        return Err(BitFunError::tool(extract_text(&result)));
    }

    Ok(extract_text(&result))
}

/// Extract concatenated text content from an MCP tool result.
pub(crate) fn extract_text(result: &MCPToolResult) -> String {
    if let Some(content) = &result.content {
        let texts: Vec<String> = content
            .iter()
            .filter_map(|c| match c {
                MCPToolResultContent::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect();
        if !texts.is_empty() {
            return texts.join("\n");
        }
    }
    serde_json::to_string_pretty(result).unwrap_or_default()
}
