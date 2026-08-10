//! Shared `deveco-mcp` check helper for the ArkTS / C++ static-check tools.
//!
//! Calls the `check` MCP tool on the `deveco-mcp` server. When the server is
//! not connected (devecocli's MCP server was never configured, was stopped, or
//! the connection dropped), it is provisioned and started on demand from the
//! session harmony project path — mirroring deveco-code `harmony-mcp.ts`
//! `callDevecoCheck` / `restartHarmonyMcp` — so the check flow stays smooth
//! without a manual "configure the MCP server" step.
//!
//! The server command is wrapped in the user's configured terminal shell (same
//! mechanism as `devecocli_run`) so npm-installed `devecocli` `.cmd` shims on
//! Windows and SDK paths only present in shell profiles are resolved correctly.

use crate::agentic::tools::framework::ToolUseContext;
use crate::service::mcp::get_global_mcp_service;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_services_integrations::mcp::config::ConfigLocation;
use bitfun_services_integrations::mcp::protocol::{MCPToolResult, MCPToolResultContent};
use bitfun_services_integrations::mcp::{MCPConnection, MCPServerConfig, MCPServerType};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) const MCP_SERVER_ID: &str = "deveco-mcp";

/// Call the `check` tool on the `deveco-mcp` MCP server. If the server is not
/// connected, provision/start it on demand, then retry.
pub(crate) async fn call_deveco_mcp_check(
    files: &[String],
    context: &ToolUseContext,
) -> BitFunResult<String> {
    let mcp_service = get_global_mcp_service()
        .ok_or_else(|| BitFunError::tool("MCP service is not initialized".to_string()))?;
    let server_manager = mcp_service.server_manager();

    // Fast path: already connected.
    if let Some(connection) = server_manager.get_connection(MCP_SERVER_ID).await {
        return call_check_tool(&connection, files).await;
    }

    // Auto-provision / start the server, then retry.
    if let Err(e) = ensure_deveco_mcp_connected(context).await {
        return Err(BitFunError::tool(format!(
            "MCP server '{}' is not connected and could not be started automatically: {}. \
             Ensure devecocli is installed (`npm install -g devecocli`) and `devecocli serve mcp` can start.",
            MCP_SERVER_ID, e
        )));
    }

    let connection = server_manager
        .get_connection(MCP_SERVER_ID)
        .await
        .ok_or_else(|| {
            BitFunError::tool(format!(
                "MCP server '{}' was started but no connection is available.",
                MCP_SERVER_ID
            ))
        })?;

    call_check_tool(&connection, files).await
}

async fn call_check_tool(
    connection: &Arc<MCPConnection>,
    files: &[String],
) -> BitFunResult<String> {
    let result = connection
        .call_tool("check", Some(json!({ "files": files })))
        .await
        .map_err(|e| BitFunError::tool(format!("MCP check call failed: {}", e)))?;
    if result.is_error {
        return Err(BitFunError::tool(extract_text(&result)));
    }
    Ok(extract_text(&result))
}

/// Ensure the `deveco-mcp` server is connected. If a persisted/builtin config
/// exists, (re)start it; otherwise provision a runtime-only (ephemeral) server
/// from `devecocli serve mcp` with `PROJECT_PATH` set to the harmony cwd.
async fn ensure_deveco_mcp_connected(context: &ToolUseContext) -> BitFunResult<()> {
    let mcp_service = get_global_mcp_service()
        .ok_or_else(|| BitFunError::tool("MCP service is not initialized".to_string()))?;
    let sm = mcp_service.server_manager();

    if sm.get_connection(MCP_SERVER_ID).await.is_some() {
        return Ok(());
    }

    // 1. If a persisted/builtin config exists, (re)starting it is the cheapest
    //    path and respects the user's own config. No-op error when there is
    //    none (we then provision an ephemeral server below).
    let _ = sm.start_server(MCP_SERVER_ID).await;
    if sm.get_connection(MCP_SERVER_ID).await.is_some() {
        return Ok(());
    }

    // 2. Provision a runtime-only server. If a stale runtime registration from a
    //    failed prior start blocks `add_ephemeral_server`, clear it and retry.
    let config = build_deveco_mcp_config(context).await?;
    if sm.add_ephemeral_server(config.clone()).await.is_err() {
        let _ = sm.remove_ephemeral_server(MCP_SERVER_ID).await;
        sm.add_ephemeral_server(config).await?;
    }

    if sm.get_connection(MCP_SERVER_ID).await.is_some() {
        return Ok(());
    }
    Err(BitFunError::tool(format!(
        "'{}' started but no MCP connection was established",
        MCP_SERVER_ID
    )))
}

/// Build a runtime-only `deveco-mcp` config, resolving the user's shell so the
/// `devecocli` command is spawnable on every platform.
async fn build_deveco_mcp_config(context: &ToolUseContext) -> BitFunResult<MCPServerConfig> {
    let command_str = "devecocli serve mcp";
    let shell_argv = super::exec_command::resolve_shell_argv_for_command(command_str).await;
    if shell_argv.is_empty() {
        return Err(BitFunError::tool(
            super::devecocli_run::DEVECOCLI_MISSING.to_string(),
        ));
    }
    let project_path = super::devecocli_run::resolve_harmony_cwd(context);
    Ok(build_config_from_shell(shell_argv, project_path))
}

/// Pure config builder (separated for unit testing).
fn build_config_from_shell(shell_argv: Vec<String>, project_path: String) -> MCPServerConfig {
    let command = shell_argv.first().cloned().unwrap_or_default();
    let args: Vec<String> = if shell_argv.len() > 1 {
        shell_argv[1..].to_vec()
    } else {
        Vec::new()
    };
    let mut env = HashMap::new();
    if !project_path.is_empty() {
        env.insert("PROJECT_PATH".to_string(), project_path);
    }
    MCPServerConfig {
        id: MCP_SERVER_ID.to_string(),
        name: "DevEco CLI MCP".to_string(),
        server_type: MCPServerType::Local,
        transport: None,
        command: Some(command),
        args,
        env,
        working_directory: None,
        inherit_parent_environment: None,
        headers: HashMap::new(),
        url: None,
        auto_start: true,
        enabled: true,
        location: ConfigLocation::BuiltIn,
        capabilities: Vec::new(),
        settings: HashMap::new(),
        oauth: None,
        oauth_enabled: None,
        xaa: None,
        timeouts: Default::default(),
    }
}

/// Extract concatenated text content from an MCP tool result.
fn extract_text(result: &MCPToolResult) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_services_integrations::mcp::MCPServerType;

    #[test]
    fn config_uses_shell_argv_and_project_path() {
        let cfg = build_config_from_shell(
            vec![
                "/bin/zsh".to_string(),
                "-c".to_string(),
                "devecocli serve mcp".to_string(),
            ],
            "/home/user/project".to_string(),
        );
        assert_eq!(cfg.id, MCP_SERVER_ID);
        assert_eq!(cfg.server_type, MCPServerType::Local);
        assert_eq!(cfg.command.as_deref(), Some("/bin/zsh"));
        assert_eq!(
            cfg.args,
            vec!["-c".to_string(), "devecocli serve mcp".to_string()]
        );
        assert_eq!(
            cfg.env.get("PROJECT_PATH").map(|s| s.as_str()),
            Some("/home/user/project")
        );
        assert!(cfg.enabled);
        assert!(cfg.auto_start);
    }

    #[test]
    fn config_handles_empty_shell_argv() {
        let cfg = build_config_from_shell(Vec::new(), String::new());
        assert!(cfg.command.as_deref().unwrap_or("").is_empty());
        assert!(!cfg.env.contains_key("PROJECT_PATH"));
    }

    #[test]
    fn extract_text_joins_text_content() {
        let result = MCPToolResult {
            content: Some(vec![
                MCPToolResultContent::Text {
                    text: "line1".to_string(),
                },
                MCPToolResultContent::Text {
                    text: "line2".to_string(),
                },
            ]),
            is_error: false,
            structured_content: None,
            meta: None,
        };
        assert_eq!(extract_text(&result), "line1\nline2");
    }
}
