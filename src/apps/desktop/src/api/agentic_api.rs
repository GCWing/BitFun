//! Agentic API

use log::warn;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, State};

use crate::api::app_state::AppState;
use bitfun_core::agentic::tools::image_context::get_image_context;
use bitfun_core::agentic::coordination::{ConversationCoordinator, DialogScheduler, DialogTriggerSource};
use bitfun_core::agentic::core::*;
use bitfun_core::agentic::image_analysis::ImageContextData;
use bitfun_core::infrastructure::get_workspace_path;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    pub session_id: Option<String>,
    pub session_name: String,
    pub agent_type: String,
    pub config: Option<SessionConfigDTO>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionConfigDTO {
    pub max_context_tokens: Option<usize>,
    pub auto_compact: Option<bool>,
    pub enable_tools: Option<bool>,
    pub safe_mode: Option<bool>,
    pub max_turns: Option<usize>,
    pub enable_context_compression: Option<bool>,
    pub compression_threshold: Option<f32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionResponse {
    pub session_id: String,
    pub session_name: String,
    pub agent_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDialogTurnRequest {
    pub session_id: String,
    pub user_input: String,
    pub agent_type: String,
    pub turn_id: Option<String>,
    #[serde(default)]
    pub image_contexts: Option<Vec<ImageContextData>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDialogTurnResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSessionRequest {
    pub session_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResponse {
    pub session_id: String,
    pub session_name: String,
    pub agent_type: String,
    pub state: String,
    pub turn_count: usize,
    pub created_at: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMessagesRequest {
    pub session_id: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageDTO {
    pub id: String,
    pub role: String,
    pub content: serde_json::Value,
    pub timestamp: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelDialogTurnRequest {
    pub session_id: String,
    pub dialog_turn_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelToolRequest {
    pub tool_use_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSessionRequest {
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreSessionRequest {
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmToolRequest {
    pub session_id: String,
    pub tool_id: String,
    pub updated_input: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectToolRequest {
    pub session_id: String,
    pub tool_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateSessionTitleRequest {
    pub session_id: String,
    pub user_message: String,
    pub max_length: Option<usize>,
}

#[tauri::command]
pub async fn create_session(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: CreateSessionRequest,
) -> Result<CreateSessionResponse, String> {
    let workspace_path = get_workspace_path()
        .map(|p| p.to_string_lossy().to_string());

    let config = request
        .config
        .map(|c| SessionConfig {
            max_context_tokens: c.max_context_tokens.unwrap_or(128128),
            auto_compact: c.auto_compact.unwrap_or(true),
            enable_tools: c.enable_tools.unwrap_or(true),
            safe_mode: c.safe_mode.unwrap_or(true),
            max_turns: c.max_turns.unwrap_or(200),
            enable_context_compression: c.enable_context_compression.unwrap_or(true),
            compression_threshold: c.compression_threshold.unwrap_or(0.8),
            workspace_path: None,
        })
        .unwrap_or_default();

    let session = coordinator
        .create_session_with_workspace(
            request.session_id,
            request.session_name.clone(),
            request.agent_type.clone(),
            config,
            workspace_path,
        )
        .await
        .map_err(|e| format!("Failed to create session: {}", e))?;

    Ok(CreateSessionResponse {
        session_id: session.session_id,
        session_name: session.session_name,
        agent_type: session.agent_type,
    })
}

#[tauri::command]
pub async fn start_dialog_turn(
    _app: AppHandle,
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    scheduler: State<'_, Arc<DialogScheduler>>,
    request: StartDialogTurnRequest,
) -> Result<StartDialogTurnResponse, String> {
    let StartDialogTurnRequest {
        session_id,
        user_input,
        agent_type,
        turn_id,
        image_contexts,
    } = request;

    if let Some(image_contexts) = image_contexts
        .as_ref()
        .filter(|images| !images.is_empty())
        .cloned()
    {
        let resolved_image_contexts = resolve_missing_image_payloads(image_contexts)?;
        coordinator
            .start_dialog_turn_with_image_contexts(
                session_id,
                user_input,
                resolved_image_contexts,
                turn_id,
                agent_type,
                DialogTriggerSource::DesktopUi,
            )
            .await
            .map_err(|e| format!("Failed to start dialog turn: {}", e))?;
    } else {
        scheduler
            .submit(
                session_id,
                user_input,
                turn_id,
                agent_type,
                DialogTriggerSource::DesktopUi,
            )
            .await
            .map_err(|e| format!("Failed to start dialog turn: {}", e))?;
    }

    Ok(StartDialogTurnResponse {
        success: true,
        message: "Dialog turn started".to_string(),
    })
}

fn is_blank_text(value: Option<&String>) -> bool {
    value.map(|s| s.trim().is_empty()).unwrap_or(true)
}

fn resolve_missing_image_payloads(
    image_contexts: Vec<ImageContextData>,
) -> Result<Vec<ImageContextData>, String> {
    let mut resolved = Vec::with_capacity(image_contexts.len());

    for mut image in image_contexts {
        let missing_payload =
            is_blank_text(image.image_path.as_ref()) && is_blank_text(image.data_url.as_ref());
        if !missing_payload {
            resolved.push(image);
            continue;
        }

        let stored = get_image_context(&image.id).ok_or_else(|| {
            format!(
                "Image context not found for image_id={}. It may have expired. Please re-attach the image and retry.",
                image.id
            )
        })?;

        if is_blank_text(image.image_path.as_ref()) {
            image.image_path = stored
                .image_path
                .clone()
                .filter(|s: &String| !s.trim().is_empty());
        }
        if is_blank_text(image.data_url.as_ref()) {
            image.data_url = stored
                .data_url
                .clone()
                .filter(|s: &String| !s.trim().is_empty());
        }
        if image.mime_type.trim().is_empty() {
            image.mime_type = stored.mime_type.clone();
        }

        let mut metadata = image.metadata.take().unwrap_or_else(|| serde_json::json!({}));
        if !metadata.is_object() {
            metadata = serde_json::json!({ "raw_metadata": metadata });
        }
        if let Some(obj) = metadata.as_object_mut() {
            if !obj.contains_key("name") {
                obj.insert("name".to_string(), serde_json::json!(stored.image_name));
            }
            if !obj.contains_key("width") {
                obj.insert("width".to_string(), serde_json::json!(stored.width));
            }
            if !obj.contains_key("height") {
                obj.insert("height".to_string(), serde_json::json!(stored.height));
            }
            if !obj.contains_key("file_size") {
                obj.insert("file_size".to_string(), serde_json::json!(stored.file_size));
            }
            if !obj.contains_key("source") {
                obj.insert("source".to_string(), serde_json::json!(stored.source));
            }
            obj.insert(
                "resolved_from_upload_cache".to_string(),
                serde_json::json!(true),
            );
        }
        image.metadata = Some(metadata);

        let still_missing =
            is_blank_text(image.image_path.as_ref()) && is_blank_text(image.data_url.as_ref());
        if still_missing {
            return Err(format!(
                "Image context {} is missing image_path/data_url after cache resolution",
                image.id
            ));
        }

        resolved.push(image);
    }

    Ok(resolved)
}

#[tauri::command]
pub async fn cancel_dialog_turn(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: CancelDialogTurnRequest,
) -> Result<(), String> {
    coordinator
        .cancel_dialog_turn(&request.session_id, &request.dialog_turn_id)
        .await
        .map_err(|e| {
            log::error!(
                "Failed to cancel dialog turn: session_id={}, dialog_turn_id={}, error={}",
                request.session_id,
                request.dialog_turn_id,
                e
            );
            format!("Failed to cancel dialog turn: {}", e)
        })
}

#[tauri::command]
pub async fn cancel_tool(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: CancelToolRequest,
) -> Result<(), String> {
    let reason = request
        .reason
        .unwrap_or_else(|| "User cancelled".to_string());

    coordinator
        .cancel_tool(&request.tool_use_id, reason)
        .await
        .map_err(|e| {
            log::error!(
                "Failed to cancel tool execution: tool_use_id={}, error={}",
                request.tool_use_id,
                e
            );
            format!("Failed to cancel tool execution: {}", e)
        })
}

#[tauri::command]
pub async fn delete_session(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: DeleteSessionRequest,
) -> Result<(), String> {
    coordinator
        .delete_session(&request.session_id)
        .await
        .map_err(|e| format!("Failed to delete session: {}", e))
}

#[tauri::command]
pub async fn restore_session(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: RestoreSessionRequest,
) -> Result<SessionResponse, String> {
    let session = coordinator
        .restore_session(&request.session_id)
        .await
        .map_err(|e| format!("Failed to restore session: {}", e))?;

    Ok(session_to_response(session))
}

#[tauri::command]
pub async fn list_sessions(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
) -> Result<Vec<SessionResponse>, String> {
    let summaries = coordinator
        .list_sessions()
        .await
        .map_err(|e| format!("Failed to list sessions: {}", e))?;

    let responses = summaries
        .into_iter()
        .map(|summary| SessionResponse {
            session_id: summary.session_id,
            session_name: summary.session_name,
            agent_type: summary.agent_type,
            state: format!("{:?}", summary.state),
            turn_count: summary.turn_count,
            created_at: system_time_to_unix_secs(summary.created_at),
        })
        .collect();

    Ok(responses)
}

#[tauri::command]
pub async fn get_session_messages(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: GetMessagesRequest,
) -> Result<Vec<MessageDTO>, String> {
    let messages = coordinator
        .get_messages(&request.session_id)
        .await
        .map_err(|e| format!("Failed to get messages: {}", e))?;

    let message_dtos = messages.into_iter().map(message_to_dto).collect();

    Ok(message_dtos)
}

#[tauri::command]
pub async fn confirm_tool_execution(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: ConfirmToolRequest,
) -> Result<(), String> {
    coordinator
        .confirm_tool(&request.tool_id, request.updated_input)
        .await
        .map_err(|e| format!("Confirm tool failed: {}", e))
}

#[tauri::command]
pub async fn reject_tool_execution(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: RejectToolRequest,
) -> Result<(), String> {
    let reason = request
        .reason
        .unwrap_or_else(|| "User rejected".to_string());

    coordinator
        .reject_tool(&request.tool_id, reason)
        .await
        .map_err(|e| format!("Reject tool failed: {}", e))
}

#[tauri::command]
pub async fn generate_session_title(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: GenerateSessionTitleRequest,
) -> Result<String, String> {
    coordinator
        .generate_session_title(
            &request.session_id,
            &request.user_message,
            request.max_length,
        )
        .await
        .map_err(|e| format!("Failed to generate session title: {}", e))
}

#[tauri::command]
pub async fn get_available_modes(state: State<'_, AppState>) -> Result<Vec<ModeInfoDTO>, String> {
    let mode_infos = state.agent_registry.get_modes_info().await;

    let dtos: Vec<ModeInfoDTO> = mode_infos
        .into_iter()
        .map(|info| ModeInfoDTO {
            id: info.id,
            name: info.name,
            description: info.description,
            is_readonly: info.is_readonly,
            tool_count: info.tool_count,
            default_tools: info.default_tools,
            enabled: info.enabled,
        })
        .collect();

    Ok(dtos)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeInfoDTO {
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_readonly: bool,
    pub tool_count: usize,
    pub default_tools: Vec<String>,
    pub enabled: bool,
}

fn session_to_response(session: Session) -> SessionResponse {
    SessionResponse {
        session_id: session.session_id,
        session_name: session.session_name,
        agent_type: session.agent_type,
        state: format!("{:?}", session.state),
        turn_count: session.dialog_turn_ids.len(),
        created_at: system_time_to_unix_secs(session.created_at),
    }
}

fn message_to_dto(message: Message) -> MessageDTO {
    let role = match message.role {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
        MessageRole::System => "system",
    };

    let content = match message.content {
        MessageContent::Text(text) => serde_json::json!({ "type": "text", "text": text }),
        MessageContent::Multimodal { text, images } => {
            let images: Vec<serde_json::Value> = images
                .into_iter()
                .map(|img| {
                    serde_json::json!({
                        "id": img.id,
                        "image_path": img.image_path,
                        "mime_type": img.mime_type,
                        "metadata": img.metadata,
                        "has_data_url": img.data_url.as_ref().is_some_and(|s| !s.is_empty()),
                    })
                })
                .collect();

            serde_json::json!({
                "type": "multimodal",
                "text": text,
                "images": images,
            })
        }
        MessageContent::ToolResult {
            tool_id,
            tool_name,
            result,
            result_for_assistant,
            is_error: _,
        } => {
            serde_json::json!({
                "type": "tool_result",
                "tool_id": tool_id,
                "tool_name": tool_name,
                "result": result,
                "result_for_assistant": result_for_assistant,
            })
        }
        MessageContent::Mixed {
            reasoning_content,
            text,
            tool_calls,
        } => {
            serde_json::json!({
                "type": "mixed",
                "reasoning_content": reasoning_content,
                "text": text,
                "tool_calls": tool_calls,
            })
        }
    };

    MessageDTO {
        id: message.id,
        role: role.to_string(),
        content,
        timestamp: system_time_to_unix_secs(message.timestamp),
    }
}

fn system_time_to_unix_secs(time: std::time::SystemTime) -> u64 {
    match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(err) => {
            warn!("Failed to convert SystemTime to unix timestamp: {}", err);
            0
        }
    }
}
