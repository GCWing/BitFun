//! Tool API

use log::error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

use bitfun_agent_runtime::sdk::AgentUserAnswersRequest;
use bitfun_core::agentic::{
    tools::framework::ToolUseContext,
    tools::{get_all_tools, get_readonly_tools},
    workspace::{local_workspace_services, remote_workspace_services},
    WorkspaceBinding,
};
use bitfun_core::product_runtime::CoreRuntimeServicesProvider;
use bitfun_core::service::remote_ssh::workspace_state::{
    get_remote_workspace_manager, lookup_remote_connection, workspace_session_identity,
};
use bitfun_core::util::elapsed_ms_u64;

use crate::runtime::DesktopRuntimeContext;

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
pub struct SetGlobalToolDisabledRequest {
    pub tool_name: String,
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalToolSettingsResponse {
    pub globally_disabled_user_tool_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceModeToolSelectionRequest {
    pub mode_id: String,
    pub enabled_tool_names: Vec<String>,
    pub workspace_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetModeToolSelectionRequest {
    pub mode_id: String,
    pub workspace_path: Option<String>,
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

    let remote_exec_port = workspace
        .as_ref()
        .is_some_and(WorkspaceBinding::is_remote)
        .then(CoreRuntimeServicesProvider::remote_exec_port);

    ToolUseContext::for_tool_listing_with_remote_exec_port(
        workspace,
        workspace_services,
        remote_exec_port,
    )
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

fn write_file_path(input: &serde_json::Value) -> Option<&str> {
    let value = input.get("payload")?.as_str()?;
    let first_line = value
        .split_once('\n')
        .map_or(value, |(file_path, _)| file_path);
    let first_line = first_line.strip_suffix('\r').unwrap_or(first_line);
    let file_path = first_line.strip_prefix("+++ ")?;
    (!file_path.trim().is_empty()).then_some(file_path)
}

fn tool_requires_workspace_path(tool_name: &str, input: &serde_json::Value) -> bool {
    match tool_name {
        "Bash" => true,
        "Glob" | "Grep" => input.get("path").is_none() || is_relative_path(input.get("path")),
        "Write" => write_file_path(input).map_or_else(
            || input.get("payload").is_some(),
            |path| !PathBuf::from(path).is_absolute(),
        ),
        "Read" | "Edit" | "GetFileDiff" => is_relative_path(input.get("file_path")),
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
    runtime: State<'_, DesktopRuntimeContext>,
    tool_id: String,
    answers: serde_json::Value,
) -> Result<(), String> {
    runtime
        .agent_runtime()
        .submit_user_answers(AgentUserAnswersRequest {
            tool_id: tool_id.clone(),
            answers,
        })
        .await
        .map_err(|error| {
            let error = desktop_user_answers_error_message(error.into_message());
            error!(
                "Failed to send user answer: tool_id={}, error={}",
                tool_id, error
            );
            error
        })
}

fn desktop_user_answers_error_message(message: String) -> String {
    message
        .strip_prefix("Tool error: ")
        .unwrap_or(&message)
        .to_string()
}

/// Tool-side global availability toggle (mirrors `set_global_skill_disabled`).
#[tauri::command]
pub async fn set_global_tool_disabled(
    request: SetGlobalToolDisabledRequest,
) -> Result<GlobalToolSettingsResponse, String> {
    let tool_name = request.tool_name.trim();
    if tool_name.is_empty() {
        return Err("Tool name must not be empty".to_string());
    }

    let known_tool = bitfun_core::agentic::tools::get_all_tools()
        .await
        .into_iter()
        .any(|tool| tool.name() == tool_name);
    if !known_tool {
        return Err(format!("Tool '{}' was not found", tool_name));
    }

    let globally_disabled_user_tool_names =
        bitfun_core::agentic::tools::implementations::tools::mode_overrides::set_global_user_tool_disabled(
            tool_name,
            request.disabled,
        )
        .await
        .map_err(|error| format!("Failed to update global Tool settings: {}", error))?;
    if let Err(error) = bitfun_core::service::config::reload_global_config().await {
        log::warn!(
            "Failed to reload global configuration after Tool availability update: tool_name={}, error={}",
            tool_name,
            error
        );
    }

    Ok(GlobalToolSettingsResponse {
        globally_disabled_user_tool_names,
    })
}

/// Replace the enabled-tool selection for a mode profile
/// (mirrors `replace_mode_skill_selection`, persisted via `enabled_tools`).
#[tauri::command]
pub async fn replace_mode_tool_selection(
    request: ReplaceModeToolSelectionRequest,
) -> Result<String, String> {
    let enabled_tool_names = normalize_tool_name_list(request.enabled_tool_names);

    // Validate against the live registry (same strictness as skill keys).
    let known_tools = bitfun_core::agentic::tools::get_all_tools().await;
    let known_names: std::collections::HashSet<String> = known_tools
        .iter()
        .map(|tool| tool.name().to_string())
        .collect();
    let unknown_tools: Vec<String> = enabled_tool_names
        .iter()
        .filter(|name| !known_names.contains(*name))
        .cloned()
        .collect();
    if !unknown_tools.is_empty() {
        return Err(format!(
            "Unknown tool names for mode '{}': {}",
            request.mode_id,
            unknown_tools.join(", ")
        ));
    }

    bitfun_core::service::config::mode_config_canonicalizer::persist_agent_profile_from_value(
        &request.mode_id,
        serde_json::json!({ "enabled_tools": enabled_tool_names }),
    )
    .await
    .map_err(|error| format!("Failed to update user tool overrides: {}", error))?;

    if let Err(error) = bitfun_core::service::config::reload_global_config().await {
        log::warn!(
            "Failed to reload global config after tool selection update: mode_id={}, error={}",
            request.mode_id,
            error
        );
    }

    Ok(format!(
        "Mode '{}' tool selection updated successfully",
        request.mode_id
    ))
}

/// Reset the enabled-tool selection for a mode profile back to defaults
/// (mirrors `reset_mode_skill_selection`).
#[tauri::command]
pub async fn reset_mode_tool_selection(
    request: ResetModeToolSelectionRequest,
) -> Result<String, String> {
    bitfun_core::agentic::tools::implementations::tools::mode_overrides::clear_user_mode_tool_overrides(
        &request.mode_id,
    )
    .await
    .map_err(|error| format!("Failed to reset user tool overrides: {}", error))?;

    if let Err(error) = bitfun_core::service::config::reload_global_config().await {
        log::warn!(
            "Failed to reload global config after resetting tool selection: mode_id={}, error={}",
            request.mode_id,
            error
        );
    }

    Ok(format!(
        "Mode '{}' tool selection reset successfully",
        request.mode_id
    ))
}

fn normalize_tool_name_list(tool_names: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut normalized = Vec::new();
    for name in tool_names {
        let trimmed = name.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            continue;
        }
        normalized.push(trimmed.to_string());
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::desktop_user_answers_error_message;

    #[test]
    fn user_answers_errors_keep_the_existing_desktop_text() {
        assert_eq!(
            desktop_user_answers_error_message(
                "Tool error: Waiting channel not found: tool-1".to_string(),
            ),
            "Waiting channel not found: tool-1"
        );
        assert_eq!(
            desktop_user_answers_error_message("Runtime unavailable".to_string()),
            "Runtime unavailable"
        );
    }
}
