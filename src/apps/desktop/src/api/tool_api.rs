//! Tool API

use log::error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use bitfun_core::agentic::{
    tools::framework::ToolUseContext,
    tools::{get_all_tools, get_readonly_tools},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionRequest {
    pub tool_name: String,
    pub input: serde_json::Value,
    pub context: Option<HashMap<String, String>>,
    pub safe_mode: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub is_readonly: bool,
    pub is_concurrency_safe: bool,
    pub needs_permissions: bool,
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
pub struct ToolValidationRequest {
    pub tool_name: String,
    pub input: serde_json::Value,
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

#[tauri::command]
pub async fn get_all_tools_info() -> Result<Vec<ToolInfo>, String> {
    let tools = get_all_tools().await;

    let mut tool_infos = Vec::new();

    for tool in tools {
        let description = tool
            .description()
            .await
            .unwrap_or_else(|_| "No description available".to_string());

        tool_infos.push(ToolInfo {
            name: tool.name().to_string(),
            description,
            input_schema: tool.input_schema(),
            is_readonly: tool.is_readonly(),
            is_concurrency_safe: tool.is_concurrency_safe(None),
            needs_permissions: tool.needs_permissions(None),
        });
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
        let description = tool
            .description()
            .await
            .unwrap_or_else(|_| "No description available".to_string());

        tool_infos.push(ToolInfo {
            name: tool.name().to_string(),
            description,
            input_schema: tool.input_schema(),
            is_readonly: tool.is_readonly(),
            is_concurrency_safe: tool.is_concurrency_safe(None),
            needs_permissions: tool.needs_permissions(None),
        });
    }

    Ok(tool_infos)
}

#[tauri::command]
pub async fn get_tool_info(tool_name: String) -> Result<Option<ToolInfo>, String> {
    let tools = get_all_tools().await;

    for tool in tools {
        if tool.name() == tool_name {
            let description = tool
                .description()
                .await
                .unwrap_or_else(|_| "No description available".to_string());

            return Ok(Some(ToolInfo {
                name: tool.name().to_string(),
                description,
                input_schema: tool.input_schema(),
                is_readonly: tool.is_readonly(),
                is_concurrency_safe: tool.is_concurrency_safe(None),
                needs_permissions: tool.needs_permissions(None),
            }));
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
            let context = ToolUseContext {
                tool_call_id: None,
                message_id: None,
                agent_type: None,
                session_id: None,
                dialog_turn_id: None,
                safe_mode: Some(false),
                abort_controller: None,
                read_file_timestamps: HashMap::new(),
                options: None,
                response_state: None,
                image_context_provider: None,
                subagent_parent_info: None,
                cancellation_token: None,
            };

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
            let context = ToolUseContext {
                tool_call_id: None,
                message_id: None,
                agent_type: None,
                session_id: None,
                dialog_turn_id: None,
                safe_mode: Some(false),
                abort_controller: None,
                read_file_timestamps: HashMap::new(),
                options: None,
                response_state: None,
                image_context_provider: None,
                subagent_parent_info: None,
                cancellation_token: None,
            };

            let validation_result = tool.validate_input(&request.input, Some(&context)).await;
            if !validation_result.result {
                return Ok(ToolExecutionResponse {
                    tool_name: request.tool_name,
                    success: false,
                    result: None,
                    error: None,
                    validation_error: validation_result.message,
                    duration_ms: start_time.elapsed().as_millis() as u64,
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
                        bitfun_core::agentic::tools::framework::ToolResult::Result { data, .. } => data.clone(),
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
                        duration_ms: start_time.elapsed().as_millis() as u64,
                    });
                }
                Err(e) => {
                    return Ok(ToolExecutionResponse {
                        tool_name: request.tool_name,
                        success: false,
                        result: None,
                        error: Some(format!("Tool execution failed: {}", e)),
                        validation_error: None,
                        duration_ms: start_time.elapsed().as_millis() as u64,
                    });
                }
            }
        }
    }

    Err(format!("Tool '{}' not found", request.tool_name))
}

#[tauri::command]
pub async fn is_tool_enabled(tool_name: String) -> Result<Option<bool>, String> {
    let tools = get_all_tools().await;

    for tool in tools {
        if tool.name() == tool_name {
            return Ok(Some(tool.is_enabled().await));
        }
    }

    Ok(None)
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
