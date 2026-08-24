//! ACP client API

use crate::api::app_state::AppState;
use crate::api::session_storage_path::desktop_effective_session_storage_path;
use crate::runtime::{acp_dialog_turn_started_event, acp_session_created_event, AcpTurnMapper};
use crate::startup_trace::DesktopStartupTrace;
use bitfun_acp::client::{
    AcpAvailableCommand, AcpClientInfo, AcpClientPermissionResponse, AcpClientRequirementProbe,
    AcpSessionOptions, CreateAcpFlowSessionRecordResponse, SetAcpSessionConfigOptionRequest,
    SetAcpSessionModelRequest, SubmitAcpPermissionResponseRequest,
};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientIdRequest {
    pub client_id: String,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProbeAcpClientRequirementsRequest {
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub force_refresh: bool,
}

#[tauri::command]
pub async fn initialize_acp_clients(
    state: State<'_, AppState>,
    startup_trace: State<'_, DesktopStartupTrace>,
) -> Result<(), String> {
    let trace_started = Instant::now();
    let result = async {
        let service = state
            .acp_client_service
            .as_ref()
            .ok_or_else(|| "ACP client service not initialized".to_string())?;
        service.initialize_all().await.map_err(|e| e.to_string())
    }
    .await;
    startup_trace.record_tauri_command_elapsed("initialize_acp_clients", None, trace_started);
    result
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
    startup_trace: State<'_, DesktopStartupTrace>,
    request: ProbeAcpClientRequirementsRequest,
) -> Result<Vec<AcpClientRequirementProbe>, String> {
    let trace_started = Instant::now();
    let result = async {
        let service = state
            .acp_client_service
            .as_ref()
            .ok_or_else(|| "ACP client service not initialized".to_string())?;
        service
            .probe_client_requirements(
                request.remote_connection_id.as_deref(),
                request.force_refresh,
            )
            .await
            .map_err(|e| e.to_string())
    }
    .await;
    startup_trace.record_tauri_command_elapsed(
        "probe_acp_client_requirements",
        None,
        trace_started,
    );
    result
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
        .install_client_cli(&request.client_id, request.remote_connection_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_acp_flow_session(
    state: State<'_, AppState>,
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
        .start_client_for_session(
            &request.client_id,
            &response.session_id,
            Some(&request.workspace_path),
            request.remote_connection_id.as_deref(),
        )
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

    state
        .acp_event_publisher
        .publish_session_created(acp_session_created_event(
            response.session_id.clone(),
            response.session_name.clone(),
            response.agent_type.clone(),
            request.workspace_path,
            request.remote_connection_id,
            request.remote_ssh_host,
        ))?;

    Ok(response)
}

#[tauri::command]
pub async fn start_acp_dialog_turn(
    state: State<'_, AppState>,
    request: StartAcpDialogTurnRequest,
) -> Result<(), String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?
        .clone();
    let publisher = state.acp_event_publisher.clone();

    let session_id = request.session_id.clone();
    let turn_id = request.turn_id.clone();
    let user_input = request.user_input.clone();
    let original_user_input = request
        .original_user_input
        .clone()
        .filter(|value| value != &request.user_input);
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

    publisher.publish_turn_started(acp_dialog_turn_started_event(
        session_id.clone(),
        turn_id.clone(),
        user_input,
        original_user_input,
    ))?;
    tokio::spawn(async move {
        let mut mapper = AcpTurnMapper::new(
            request.session_id.clone(),
            request.turn_id.clone(),
            request.client_id.clone(),
        );
        let result = service
            .prompt_agent_stream(
                &request.client_id,
                request.user_input,
                request.workspace_path,
                request.remote_connection_id,
                request.session_id.clone(),
                session_storage_path,
                request.timeout_seconds,
                |event| {
                    let jobs = mapper.map(event)?;
                    publisher
                        .publish_jobs(jobs)
                        .map_err(bitfun_core::util::errors::BitFunError::service)
                },
            )
            .await;

        if let Err(error) = result {
            let _ = publisher.publish_jobs(mapper.fail(error.to_string()));
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
            request.session_id,
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
            request.remote_connection_id,
            session_storage_path,
            request.session_id,
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_acp_session_commands(
    state: State<'_, AppState>,
    request: GetAcpSessionOptionsRequest,
) -> Result<Vec<AcpAvailableCommand>, String> {
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
        .get_session_commands(
            &request.client_id,
            request.workspace_path,
            request.remote_connection_id,
            session_storage_path,
            request.session_id,
        )
        .await
        .map(|(commands, _version)| commands)
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
pub async fn set_acp_session_config_option(
    state: State<'_, AppState>,
    request: SetAcpSessionConfigOptionRequest,
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
        .set_session_config_option(request, session_storage_path)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAcpPendingPermissionsRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpPendingPermissionEntry {
    pub permission_id: String,
    pub session_id: String,
    pub tool_call: serde_json::Value,
    pub options: serde_json::Value,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
}

/// Rehydrate local Web UI from the shared ACP permission mailbox after refresh.
#[tauri::command]
pub async fn list_acp_pending_permissions(
    request: ListAcpPendingPermissionsRequest,
) -> Result<Vec<AcpPendingPermissionEntry>, String> {
    let Some(mailbox) = bitfun_services_integrations::remote_connect::acp_permission_mailbox()
    else {
        return Ok(Vec::new());
    };
    Ok(mailbox
        .list_for_session(&request.session_id)
        .into_iter()
        .map(|entry| AcpPendingPermissionEntry {
            permission_id: entry.permission_id,
            session_id: entry.session_id,
            tool_call: entry.tool_call,
            options: entry.options,
            created_at_ms: entry.created_at_ms,
            expires_at_ms: entry.expires_at_ms,
        })
        .collect())
}
