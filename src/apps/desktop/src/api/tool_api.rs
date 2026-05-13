//! Tool API

use log::error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use bitfun_core::agentic::{
    tools::framework::ToolUseContext,
    tools::{get_all_tools, get_readonly_tools},
    workspace::{local_workspace_services, remote_workspace_services},
    WorkspaceBinding,
};
use bitfun_core::service::remote_ssh::workspace_state::{
    get_remote_workspace_manager, lookup_remote_connection, workspace_session_identity,
};
use bitfun_core::util::elapsed_ms_u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionRequest {
    pub tool_name: String,
    pub input: serde_json::Value,
    pub workspace_path: Option<String>,
    pub context: Option<HashMap<String, String>>,
    pub safe_mode: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetToolInfoRequest {
    pub tool_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicMcpToolInfo {
    pub server_id: String,
    pub server_name: String,
    pub tool_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolInfo {
    pub provider_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp: Option<DynamicMcpToolInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub is_readonly: bool,
    pub is_concurrency_safe: bool,
    pub needs_permissions: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_info: Option<DynamicToolInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionResponse {
    pub tool_name: String,
    pub success: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub validation_error: Option<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolValidationRequest {
    pub tool_name: String,
    pub input: serde_json::Value,
    pub workspace_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolValidationResponse {
    pub tool_name: String,
    pub valid: bool,
    pub message: Option<String>,
    pub error_code: Option<i32>,
    pub meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolConfirmationRequest {
    #[serde(alias = "tool_use_id")]
    pub tool_use_id: String,
    #[serde(alias = "tool_name")]
    pub tool_name: String,
    pub action: String,
    #[serde(alias = "task_id")]
    pub task_id: Option<String>,
    #[serde(alias = "updated_input")]
    pub updated_input: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolConfirmationResponse {
    #[serde(alias = "tool_use_id")]
    pub tool_use_id: String,
    pub success: bool,
    pub message: String,
}

async fn build_tool_context(workspace_path: Option<&str>) -> ToolUseContext {
    let normalized_workspace_path = workspace_path
        .map(str::trim)
        .filter(|path| !path.is_empty());

    let workspace = match normalized_workspace_path {
        Some(path) => {
            if let Some(entry) = lookup_remote_connection(path).await {
                let identity = workspace_session_identity(
                    path,
                    Some(&entry.connection_id),
                    Some(&entry.ssh_host),
                )
                .unwrap_or_else(|| {
                    bitfun_core::service::remote_ssh::workspace_state::WorkspaceSessionIdentity {
                        hostname: entry.ssh_host.clone(),
                        logical_workspace_path: entry.remote_root.clone(),
                        remote_connection_id: Some(entry.connection_id.clone()),
                    }
                });
                Some(WorkspaceBinding::new_remote(
                    None,
                    PathBuf::from(path),
                    entry.connection_id,
                    entry.connection_name,
                    identity,
                ))
            } else {
                Some(WorkspaceBinding::new(None, PathBuf::from(path)))
            }
        }
        None => None,
    };

    let workspace_services = match workspace.as_ref() {
        Some(binding) if binding.is_remote() => {
            let connection_id = binding.connection_id().map(str::to_string);
            match (connection_id, get_remote_workspace_manager()) {
                (Some(connection_id), Some(manager)) => {
                    match (
                        manager.get_file_service().await,
                        manager.get_ssh_manager().await,
                    ) {
                        (Some(file_service), Some(ssh_manager)) => Some(remote_workspace_services(
                            connection_id,
                            file_service,
                            ssh_manager,
                            binding.root_path_string(),
                        )),
                        _ => None,
                    }
                }
                _ => None,
            }
        }
        Some(binding) => Some(local_workspace_services(binding.root_path_string())),
        None => None,
    };

    ToolUseContext {
        tool_call_id: None,
        agent_type: None,
        session_id: None,
        dialog_turn_id: None,
        workspace,
        unlocked_collapsed_tools: Vec::new(),
        custom_data: HashMap::new(),
        computer_use_host: None,
        cancellation_token: None,
        runtime_tool_restrictions: Default::default(),
        workspace_services,
    }
}

fn to_dynamic_mcp_tool_info(
    info: bitfun_core::agentic::tools::framework::DynamicMcpToolInfo,
) -> DynamicMcpToolInfo {
    DynamicMcpToolInfo {
        server_id: info.server_id,
        server_name: info.server_name,
        tool_name: info.tool_name,
    }
}

fn to_dynamic_tool_info(
    info: bitfun_core::agentic::tools::framework::DynamicToolInfo,
) -> DynamicToolInfo {
    DynamicToolInfo {
        provider_id: info.provider_id,
        provider_kind: info.provider_kind,
        mcp: info.mcp.map(to_dynamic_mcp_tool_info),
    }
}

async fn build_tool_info(tool: &Arc<dyn bitfun_core::agentic::tools::framework::Tool>) -> ToolInfo {
    let description = tool
        .description()
        .await
        .unwrap_or_else(|_| "No description available".to_string());

    ToolInfo {
        name: tool.name().to_string(),
        description,
        input_schema: tool.input_schema_for_model().await,
        is_readonly: tool.is_readonly(),
        is_concurrency_safe: tool.is_concurrency_safe(None),
        needs_permissions: tool.needs_permissions(None),
        dynamic_info: tool.dynamic_tool_info().map(to_dynamic_tool_info),
    }
}

fn has_explicit_workspace_path(workspace_path: Option<&str>) -> bool {
    workspace_path.is_some_and(|path| !path.trim().is_empty())
}

fn is_relative_path(value: Option<&serde_json::Value>) -> bool {
    value
        .and_then(|v| v.as_str())
        .is_some_and(|path| !path.is_empty() && !PathBuf::from(path).is_absolute())
}

fn tool_requires_workspace_path(tool_name: &str, input: &serde_json::Value) -> bool {
    match tool_name {
        "Bash" => true,
        "Glob" | "Grep" => input.get("path").is_none() || is_relative_path(input.get("path")),
        "Read" | "Write" | "Edit" | "GetFileDiff" => is_relative_path(input.get("file_path")),
        _ => false,
    }
}

fn ensure_workspace_requirement(
    tool_name: &str,
    input: &serde_json::Value,
    workspace_path: Option<&str>,
) -> Result<(), String> {
    if tool_requires_workspace_path(tool_name, input)
        && !has_explicit_workspace_path(workspace_path)
    {
        return Err(format!(
            "workspacePath is required to execute tool '{}' with workspace-relative input",
            tool_name
        ));
    }

    Ok(())
}

#[tauri::command]
pub async fn get_all_tools_info() -> Result<Vec<ToolInfo>, String> {
    let tools = get_all_tools().await;

    let mut tool_infos = Vec::new();

    for tool in tools {
        tool_infos.push(build_tool_info(&tool).await);
    }

    Ok(tool_infos)
}

#[tauri::command]
pub async fn get_readonly_tools_info() -> Result<Vec<ToolInfo>, String> {
    let tools = get_readonly_tools()
        .await
        .map_err(|e| format!("Failed to get readonly tools: {}", e))?;

    let mut tool_infos = Vec::new();

    for tool in tools {
        tool_infos.push(build_tool_info(&tool).await);
    }

    Ok(tool_infos)
}

#[tauri::command]
pub async fn get_tool_info(request: GetToolInfoRequest) -> Result<Option<ToolInfo>, String> {
    let tools = get_all_tools().await;

    for tool in tools {
        if tool.name() == request.tool_name {
            return Ok(Some(build_tool_info(&tool).await));
        }
    }

    Ok(None)
}

#[tauri::command]
pub async fn validate_tool_input(
    request: ToolValidationRequest,
) -> Result<ToolValidationResponse, String> {
    let tools = get_all_tools().await;

    for tool in tools {
        if tool.name() == request.tool_name {
            ensure_workspace_requirement(
                &request.tool_name,
                &request.input,
                request.workspace_path.as_deref(),
            )?;

            let context = build_tool_context(request.workspace_path.as_deref()).await;

            let validation_result = tool.validate_input(&request.input, Some(&context)).await;

            return Ok(ToolValidationResponse {
                tool_name: request.tool_name,
                valid: validation_result.result,
                message: validation_result.message,
                error_code: validation_result.error_code,
                meta: validation_result.meta,
            });
        }
    }

    Err(format!("Tool '{}' not found", request.tool_name))
}

#[tauri::command]
pub async fn execute_tool(request: ToolExecutionRequest) -> Result<ToolExecutionResponse, String> {
    let start_time = std::time::Instant::now();

    let tools = get_all_tools().await;

    for tool in tools {
        if tool.name() == request.tool_name {
            ensure_workspace_requirement(
                &request.tool_name,
                &request.input,
                request.workspace_path.as_deref(),
            )?;

            let context = build_tool_context(request.workspace_path.as_deref()).await;

            let validation_result = tool.validate_input(&request.input, Some(&context)).await;
            if !validation_result.result {
                return Ok(ToolExecutionResponse {
                    tool_name: request.tool_name,
                    success: false,
                    result: None,
                    error: None,
                    validation_error: validation_result.message,
                    duration_ms: elapsed_ms_u64(start_time),
                });
            }

            match tool.call(&request.input, &context).await {
                Ok(results) => {
                    let combined_result = if results.len() == 1 {
                        match &results[0] {
                            bitfun_core::agentic::tools::framework::ToolResult::Result {
                                data,
                                ..
                            } => Some(data.clone()),
                            bitfun_core::agentic::tools::framework::ToolResult::Progress {
                                content,
                                ..
                            } => Some(content.clone()),
                            bitfun_core::agentic::tools::framework::ToolResult::StreamChunk {
                                data,
                                ..
                            } => Some(data.clone()),
                        }
                    } else {
                        Some(serde_json::json!({
                                        "results": results.iter().map(|r| match r {
                        bitfun_core::agentic::tools::framework::ToolResult::Result { data, .. } => {
                            data.clone()
                        }
                        bitfun_core::agentic::tools::framework::ToolResult::Progress { content, .. } => content.clone(),
                        bitfun_core::agentic::tools::framework::ToolResult::StreamChunk { data, .. } => data.clone(),
                                        }).collect::<Vec<_>>()
                                    }))
                    };

                    return Ok(ToolExecutionResponse {
                        tool_name: request.tool_name,
                        success: true,
                        result: combined_result,
                        error: None,
                        validation_error: None,
                        duration_ms: elapsed_ms_u64(start_time),
                    });
                }
                Err(e) => {
                    return Ok(ToolExecutionResponse {
                        tool_name: request.tool_name,
                        success: false,
                        result: None,
                        error: Some(format!("Tool execution failed: {}", e)),
                        validation_error: None,
                        duration_ms: elapsed_ms_u64(start_time),
                    });
                }
            }
        }
    }

    Err(format!("Tool '{}' not found", request.tool_name))
}

#[tauri::command]
pub async fn submit_user_answers(
    tool_id: String,
    answers: serde_json::Value,
) -> Result<(), String> {
    use bitfun_core::agentic::tools::user_input_manager::get_user_input_manager;
    let manager = get_user_input_manager();

    manager.send_answer(&tool_id, answers).map_err(|e| {
        error!(
            "Failed to send user answer: tool_id={}, error={}",
            tool_id, e
        );
        e
    })?;

    Ok(())
}
