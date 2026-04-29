//! ACP client API

use crate::api::app_state::AppState;
use crate::api::session_storage_path::desktop_effective_session_storage_path;
use bitfun_acp::client::{
    AcpClientInfo, AcpClientPermissionResponse, AcpClientRequirementProbe, AcpClientStreamEvent,
    AcpSessionOptions, CreateAcpFlowSessionRecordResponse, SetAcpSessionModelRequest,
    SubmitAcpPermissionResponseRequest,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientIdRequest {
    pub client_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAcpFlowSessionRequest {
    pub client_id: String,
    #[serde(default)]
    pub session_name: Option<String>,
    pub workspace_path: String,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
}

pub type CreateAcpFlowSessionResponse = CreateAcpFlowSessionRecordResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartAcpDialogTurnRequest {
    pub session_id: String,
    pub client_id: String,
    pub user_input: String,
    #[serde(default)]
    pub original_user_input: Option<String>,
    pub turn_id: String,
    #[serde(default)]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelAcpDialogTurnRequest {
    pub session_id: String,
    pub client_id: String,
    #[serde(default)]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAcpSessionOptionsRequest {
    pub session_id: String,
    pub client_id: String,
    #[serde(default)]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
}

#[tauri::command]
pub async fn initialize_acp_clients(state: State<'_, AppState>) -> Result<(), String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    service.initialize_all().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_acp_clients(state: State<'_, AppState>) -> Result<Vec<AcpClientInfo>, String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    service.list_clients().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn probe_acp_client_requirements(
    state: State<'_, AppState>,
) -> Result<Vec<AcpClientRequirementProbe>, String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    service
        .probe_client_requirements()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn predownload_acp_client_adapter(
    state: State<'_, AppState>,
    request: AcpClientIdRequest,
) -> Result<(), String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    service
        .predownload_client_adapter(&request.client_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_acp_client_cli(
    state: State<'_, AppState>,
    request: AcpClientIdRequest,
) -> Result<(), String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    service
        .install_client_cli(&request.client_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_acp_flow_session(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    request: CreateAcpFlowSessionRequest,
) -> Result<CreateAcpFlowSessionResponse, String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;

    let session_storage_path = desktop_effective_session_storage_path(
        &state,
        &request.workspace_path,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    let response = service
        .create_flow_session_record(
            &session_storage_path,
            &request.workspace_path,
            &request.client_id,
            request.session_name,
        )
        .await
        .map_err(|e| e.to_string())?;
    if let Err(error) = service
        .start_client_for_session(&request.client_id, &response.session_id)
        .await
    {
        if let Err(cleanup_error) = service
            .delete_flow_session_record(&session_storage_path, &response.session_id)
            .await
        {
            log::warn!(
                "Failed to delete ACP session record after client start failure: session_id={}, error={}",
                response.session_id,
                cleanup_error
            );
        }
        return Err(error.to_string());
    }

    let _ = app_handle.emit(
        "agentic://session-created",
        serde_json::json!({
            "sessionId": response.session_id.clone(),
            "sessionName": response.session_name.clone(),
            "agentType": response.agent_type.clone(),
            "workspacePath": request.workspace_path,
            "remoteConnectionId": request.remote_connection_id,
            "remoteSshHost": request.remote_ssh_host,
        }),
    );

    Ok(response)
}

#[tauri::command]
pub async fn start_acp_dialog_turn(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    request: StartAcpDialogTurnRequest,
) -> Result<(), String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?
        .clone();

    let session_id = request.session_id.clone();
    let turn_id = request.turn_id.clone();
    let user_input = request.user_input.clone();
    let original_user_input = request
        .original_user_input
        .clone()
        .unwrap_or_else(|| request.user_input.clone());
    let session_storage_path = match request.workspace_path.as_deref() {
        Some(workspace_path) => Some(
            desktop_effective_session_storage_path(
                &state,
                workspace_path,
                request.remote_connection_id.as_deref(),
                request.remote_ssh_host.as_deref(),
            )
            .await,
        ),
        None => None,
    };

    app_handle
        .emit(
            "agentic://dialog-turn-started",
            serde_json::json!({
                "sessionId": session_id,
                "turnId": turn_id,
                "turnIndex": null,
                "userInput": user_input,
                "originalUserInput": original_user_input,
                "userMessageMetadata": null,
                "subagentParentInfo": null,
            }),
        )
        .map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        let mut current_round_id: Option<String> = None;
        let result = service
            .prompt_agent_stream(
                &request.client_id,
                request.user_input,
                request.workspace_path,
                Some(request.session_id.clone()),
                session_storage_path,
                request.timeout_seconds,
                |event| {
                    match event {
                        AcpClientStreamEvent::ModelRoundStarted {
                            round_id,
                            round_index,
                            disable_explore_grouping,
                        } => {
                            current_round_id = Some(round_id.clone());
                            app_handle
                                .emit(
                                    "agentic://model-round-started",
                                    serde_json::json!({
                                        "sessionId": request.session_id,
                                        "turnId": request.turn_id,
                                        "roundId": round_id,
                                        "roundIndex": round_index,
                                        "renderHints": {
                                            "disableExploreGrouping": disable_explore_grouping,
                                        },
                                        "subagentParentInfo": null,
                                    }),
                                )
                                .map_err(|e| {
                                    bitfun_core::util::errors::BitFunError::service(e.to_string())
                                })?;
                        }
                        AcpClientStreamEvent::AgentText(text) => {
                            let round_id = current_round_id.clone().ok_or_else(|| {
                                bitfun_core::util::errors::BitFunError::service(
                                    "ACP text arrived before model round start".to_string(),
                                )
                            })?;
                            app_handle
                                .emit(
                                    "agentic://text-chunk",
                                    serde_json::json!({
                                        "sessionId": request.session_id,
                                        "turnId": request.turn_id,
                                        "roundId": round_id,
                                        "text": text,
                                        "subagentParentInfo": null,
                                    }),
                                )
                                .map_err(|e| {
                                    bitfun_core::util::errors::BitFunError::service(e.to_string())
                                })?;
                        }
                        AcpClientStreamEvent::AgentThought(text) => {
                            let round_id = current_round_id.clone().ok_or_else(|| {
                                bitfun_core::util::errors::BitFunError::service(
                                    "ACP thought arrived before model round start".to_string(),
                                )
                            })?;
                            app_handle
                                .emit(
                                    "agentic://text-chunk",
                                    serde_json::json!({
                                        "sessionId": request.session_id,
                                        "turnId": request.turn_id,
                                        "roundId": round_id,
                                        "text": text,
                                        "contentType": "thinking",
                                        "isThinkingEnd": false,
                                        "subagentParentInfo": null,
                                    }),
                                )
                                .map_err(|e| {
                                    bitfun_core::util::errors::BitFunError::service(e.to_string())
                                })?;
                        }
                        AcpClientStreamEvent::ToolEvent(tool_event) => {
                            app_handle
                                .emit(
                                    "agentic://tool-event",
                                    serde_json::json!({
                                        "sessionId": request.session_id,
                                        "turnId": request.turn_id,
                                        "toolEvent": tool_event,
                                        "subagentParentInfo": null,
                                    }),
                                )
                                .map_err(|e| {
                                    bitfun_core::util::errors::BitFunError::service(e.to_string())
                                })?;
                        }
                        AcpClientStreamEvent::Completed => {
                            app_handle
                                .emit(
                                    "agentic://dialog-turn-completed",
                                    serde_json::json!({
                                        "sessionId": request.session_id,
                                        "turnId": request.turn_id,
                                        "subagentParentInfo": null,
                                        "partialRecoveryReason": null,
                                    }),
                                )
                                .map_err(|e| {
                                    bitfun_core::util::errors::BitFunError::service(e.to_string())
                                })?;
                        }
                        AcpClientStreamEvent::Cancelled => {
                            app_handle
                                .emit(
                                    "agentic://dialog-turn-cancelled",
                                    serde_json::json!({
                                        "sessionId": request.session_id,
                                        "turnId": request.turn_id,
                                        "subagentParentInfo": null,
                                    }),
                                )
                                .map_err(|e| {
                                    bitfun_core::util::errors::BitFunError::service(e.to_string())
                                })?;
                        }
                    }
                    Ok(())
                },
            )
            .await;

        if let Err(error) = result {
            let _ = app_handle.emit(
                "agentic://dialog-turn-failed",
                serde_json::json!({
                    "sessionId": request.session_id,
                    "turnId": request.turn_id,
                    "error": error.to_string(),
                    "errorCategory": null,
                    "errorDetail": null,
                    "subagentParentInfo": null,
                }),
            );
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn cancel_acp_dialog_turn(
    state: State<'_, AppState>,
    request: CancelAcpDialogTurnRequest,
) -> Result<(), String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    service
        .cancel_agent_session(
            &request.client_id,
            request.workspace_path,
            Some(request.session_id),
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_acp_session_options(
    state: State<'_, AppState>,
    request: GetAcpSessionOptionsRequest,
) -> Result<AcpSessionOptions, String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    let session_storage_path = match request.workspace_path.as_deref() {
        Some(workspace_path) => Some(
            desktop_effective_session_storage_path(
                &state,
                workspace_path,
                request.remote_connection_id.as_deref(),
                request.remote_ssh_host.as_deref(),
            )
            .await,
        ),
        None => None,
    };
    service
        .get_session_options(
            &request.client_id,
            request.workspace_path,
            session_storage_path,
            Some(request.session_id),
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_acp_session_model(
    state: State<'_, AppState>,
    request: SetAcpSessionModelRequest,
) -> Result<AcpSessionOptions, String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    let session_storage_path = match request.workspace_path.as_deref() {
        Some(workspace_path) => Some(
            desktop_effective_session_storage_path(
                &state,
                workspace_path,
                request.remote_connection_id.as_deref(),
                request.remote_ssh_host.as_deref(),
            )
            .await,
        ),
        None => None,
    };
    service
        .set_session_model(request, session_storage_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_acp_client(
    state: State<'_, AppState>,
    request: AcpClientIdRequest,
) -> Result<(), String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    service
        .stop_client(&request.client_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_acp_json_config(state: State<'_, AppState>) -> Result<String, String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    service.load_json_config().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_acp_json_config(
    state: State<'_, AppState>,
    json_config: String,
) -> Result<(), String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    service
        .save_json_config(&json_config)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn submit_acp_permission_response(
    state: State<'_, AppState>,
    request: SubmitAcpPermissionResponseRequest,
) -> Result<AcpClientPermissionResponse, String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    service
        .submit_permission_response(request)
        .await
        .map_err(|e| e.to_string())
}
