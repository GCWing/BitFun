//! Commands API - Core Application Commands

use crate::api::app_state::AppState;
use crate::api::dto::WorkspaceInfoDto;
use bitfun_core::infrastructure::{file_watcher, FileOperationOptions, SearchMatchType};
use bitfun_core::service::remote_ssh::workspace_state::is_remote_path;
use bitfun_core::service::remote_ssh::{get_remote_workspace_manager, RemoteWorkspaceEntry};
use bitfun_core::service::workspace::{
    ScanOptions, WorkspaceInfo, WorkspaceKind, WorkspaceOpenOptions,
};
use log::{debug, error, info, warn};
use serde::Deserialize;
use std::path::Path;
use tauri::{AppHandle, State};

fn remote_workspace_from_info(info: &WorkspaceInfo) -> Option<crate::api::RemoteWorkspace> {
    if info.workspace_kind != WorkspaceKind::Remote {
        return None;
    }
    let cid = info.metadata.get("connectionId")?.as_str()?.to_string();
    let name = info
        .metadata
        .get("connectionName")
        .and_then(|v| v.as_str())
        .unwrap_or(&cid)
        .to_string();
    let rp = bitfun_core::service::remote_ssh::normalize_remote_workspace_path(
        &info.root_path.to_string_lossy(),
    );
    Some(crate::api::RemoteWorkspace {
        connection_id: cid,
        remote_path: rp,
        connection_name: name,
    })
}

async fn lookup_remote_entry_for_path(
    state: &State<'_, AppState>,
    path: &str,
) -> Option<RemoteWorkspaceEntry> {
    let hint = state
        .get_remote_workspace_async()
        .await
        .map(|w| w.connection_id);
    let manager = get_remote_workspace_manager()?;
    manager.lookup_connection(path, hint.as_deref()).await
}

#[derive(Debug, Deserialize)]
pub struct OpenWorkspaceRequest {
    pub path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenRemoteWorkspaceRequest {
    pub remote_path: String,
    pub connection_id: String,
    pub connection_name: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct CreateAssistantWorkspaceRequest {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanWorkspaceInfoRequest {
    pub workspace_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseWorkspaceRequest {
    pub workspace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetActiveWorkspaceRequest {
    pub workspace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAssistantWorkspaceRequest {
    pub workspace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetAssistantWorkspaceRequest {
    pub workspace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderOpenedWorkspacesRequest {
    pub workspace_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct TestAIConfigConnectionRequest {
    pub config: bitfun_core::service::config::types::AIModelConfig,
}

#[derive(Debug, Deserialize)]
pub struct ListAIModelsByConfigRequest {
    pub config: bitfun_core::service::config::types::AIModelConfig,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixMermaidCodeRequest {
    pub source_code: String,
    pub error_message: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAppStatusRequest {
    pub status: String,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReadFileContentRequest {
    #[serde(rename = "filePath")]
    pub file_path: String,
    pub encoding: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WriteFileContentRequest {
    #[serde(rename = "workspacePath")]
    pub workspace_path: String,
    #[serde(rename = "filePath")]
    pub file_path: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetWorkspacePersonaFilesRequest {
    pub workspace_path: String,
}

#[derive(Debug, Deserialize)]
pub struct CheckPathExistsRequest {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct GetFileMetadataRequest {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct GetFileTreeRequest {
    pub path: String,
    pub max_depth: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct GetDirectoryChildrenRequest {
    pub path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetDirectoryChildrenPaginatedRequest {
    pub path: String,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFilesRequest {
    pub root_path: String,
    pub pattern: String,
    pub search_content: bool,
    #[serde(default)]
    pub case_sensitive: bool,
    #[serde(default)]
    pub use_regex: bool,
    #[serde(default)]
    pub whole_word: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameFileRequest {
    pub old_path: String,
    pub new_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportLocalFileRequest {
    pub source_path: String,
    pub destination_path: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteFileRequest {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteDirectoryRequest {
    pub path: String,
    pub recursive: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFileRequest {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateDirectoryRequest {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct RevealInExplorerRequest {
    pub path: String,
}

async fn clear_active_workspace_context(state: &State<'_, AppState>, app: &AppHandle) {
    #[cfg(not(target_os = "macos"))]
    let _ = app;

    *state.workspace_path.write().await = None;

    if let Some(ref pool) = state.js_worker_pool {
        pool.stop_all().await;
    }

    state.ai_rules_service.clear_workspace().await;
    state.agent_registry.clear_custom_subagents();

    #[cfg(target_os = "macos")]
    {
        let language = state
            .config_service
            .get_config::<String>(Some("app.language"))
            .await
            .unwrap_or_else(|_| "zh-CN".to_string());
        let edit_mode = *state.macos_edit_menu_mode.read().await;
        let _ = crate::macos_menubar::set_macos_menubar_with_mode(
            app,
            &language,
            crate::macos_menubar::MenubarMode::Startup,
            edit_mode,
        );
    }
}

async fn apply_active_workspace_context(
    state: &State<'_, AppState>,
    app: &AppHandle,
    workspace_info: &bitfun_core::service::workspace::manager::WorkspaceInfo,
) {
    #[cfg(not(target_os = "macos"))]
    let _ = app;

    clear_active_workspace_context(state, app).await;

    *state.workspace_path.write().await = Some(workspace_info.root_path.clone());

    // Remote workspace roots are POSIX paths on the SSH host — not writable local directories on
    // Windows. Snapshot hooks already skip file tracking for registered remote paths; avoid
    // creating `/.bitfun` (or drive root) here which fails with access denied.
    let root_str = workspace_info.root_path.to_string_lossy().to_string();
    let skip_local_snapshot = workspace_info.workspace_kind == WorkspaceKind::Remote
        || is_remote_path(root_str.trim()).await;
    if !skip_local_snapshot {
        if let Err(e) = bitfun_core::service::snapshot::initialize_snapshot_manager_for_workspace(
            workspace_info.root_path.clone(),
            None,
        )
        .await
        {
            warn!(
                "Failed to initialize snapshot system: path={}, error={}",
                workspace_info.root_path.display(),
                e
            );
        }
    } else {
        debug!(
            "Skipping local snapshot manager init for remote/non-local workspace root_path={}",
            workspace_info.root_path.display()
        );
    }

    state
        .agent_registry
        .load_custom_subagents(&workspace_info.root_path)
        .await;

    if let Err(e) = state
        .ai_rules_service
        .set_workspace(workspace_info.root_path.clone())
        .await
    {
        warn!(
            "Failed to set AI rules workspace: path={}, error={}",
            workspace_info.root_path.display(),
            e
        );
    }

    #[cfg(target_os = "macos")]
    {
        let language = state
            .config_service
            .get_config::<String>(Some("app.language"))
            .await
            .unwrap_or_else(|_| "zh-CN".to_string());
        let edit_mode = *state.macos_edit_menu_mode.read().await;
        let _ = crate::macos_menubar::set_macos_menubar_with_mode(
            app,
            &language,
            crate::macos_menubar::MenubarMode::Workspace,
            edit_mode,
        );
    }

    // Keep global SSH registry + active connection hint aligned with the **foreground** workspace
    // so two servers opened at the same remote path (e.g. `/`) stay distinct.
    if workspace_info.workspace_kind == WorkspaceKind::Remote {
        if let Some(rw) = remote_workspace_from_info(workspace_info) {
            if let Err(e) = state.set_remote_workspace(rw).await {
                warn!("Failed to sync remote workspace registry for active workspace: {}", e);
            }
        }
    } else {
        *state.remote_workspace.write().await = None;
        if let Some(m) = get_remote_workspace_manager() {
            m.set_active_connection_hint(None).await;
        }
    }
}

#[tauri::command]
pub async fn initialize_global_state(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    if let Some(workspace_info) = state.workspace_service.get_current_workspace().await {
        apply_active_workspace_context(&state, &app, &workspace_info).await;

        info!(
            "Global state initialized with active workspace: workspace_id={}, path={}",
            workspace_info.id,
            workspace_info.root_path.display()
        );
    } else {
        clear_active_workspace_context(&state, &app).await;
        info!("Global state initialized without active workspace");
    }

    Ok("Global state initialized successfully".to_string())
}

#[tauri::command]
pub async fn get_available_tools(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(state.get_tool_names())
}

#[tauri::command]
pub async fn get_health_status(
    state: State<'_, AppState>,
) -> Result<crate::api::HealthStatus, String> {
    Ok(state.get_health_status().await)
}

#[tauri::command]
pub async fn get_statistics(
    state: State<'_, AppState>,
) -> Result<crate::api::AppStatistics, String> {
    Ok(state.get_statistics().await)
}

#[tauri::command]
pub async fn test_ai_connection(state: State<'_, AppState>) -> Result<bool, String> {
    let ai_client = state.ai_client.read().await;
    Ok(ai_client.is_some())
}

#[tauri::command]
pub async fn initialize_ai(state: State<'_, AppState>) -> Result<String, String> {
    let config_service = &state.config_service;
    let global_config: bitfun_core::service::config::GlobalConfig = config_service
        .get_config(None)
        .await
        .map_err(|e| format!("Failed to get configuration: {}", e))?;

    let primary_model_id = global_config.ai.default_models.primary.ok_or_else(|| {
        "Primary model not configured, please configure it in settings".to_string()
    })?;
    let model_config = global_config
        .ai
        .models
        .iter()
        .find(|m| m.id == primary_model_id)
        .ok_or_else(|| format!("Primary model '{}' does not exist", primary_model_id))?;

    let ai_config = bitfun_core::util::types::AIConfig::try_from(model_config.clone())
        .map_err(|e| format!("Failed to convert AI configuration: {}", e))?;
    let ai_client = bitfun_core::infrastructure::ai::AIClient::new(ai_config);

    {
        let mut ai_client_guard = state.ai_client.write().await;
        *ai_client_guard = Some(ai_client);
    }

    info!("AI client initialized: model={}", model_config.name);
    Ok(format!(
        "AI client initialized successfully: {}",
        model_config.name
    ))
}

#[tauri::command]
pub async fn test_ai_config_connection(
    request: TestAIConfigConnectionRequest,
) -> Result<bitfun_core::util::types::ConnectionTestResult, String> {
    let model_name = request.config.name.clone();
    let supports_image_input = request.config.capabilities.iter().any(|cap| {
        matches!(
            cap,
            bitfun_core::service::config::types::ModelCapability::ImageUnderstanding
        )
    }) || matches!(
        request.config.category,
        bitfun_core::service::config::types::ModelCategory::Multimodal
    );

    let ai_config = match request.config.try_into() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to convert AI config: {}", e);
            return Err(format!("Failed to convert configuration: {}", e));
        }
    };

    let ai_client = bitfun_core::infrastructure::ai::client::AIClient::new(ai_config);

    match ai_client.test_connection().await {
        Ok(result) => {
            if !result.success {
                info!(
                    "AI config connection test completed: model={}, success={}, response_time={}ms",
                    model_name, result.success, result.response_time_ms
                );
                return Ok(result);
            }

            if supports_image_input {
                match ai_client.test_image_input_connection().await {
                    Ok(image_result) => {
                        let response_time_ms =
                            result.response_time_ms + image_result.response_time_ms;

                        if !image_result.success {
                            let merged = bitfun_core::util::types::ConnectionTestResult {
                                success: false,
                                response_time_ms,
                                model_response: image_result.model_response.or(result.model_response),
                                message_code: image_result.message_code,
                                error_details: image_result.error_details,
                            };
                            info!(
                                "AI config connection test completed: model={}, success={}, response_time={}ms",
                                model_name, merged.success, merged.response_time_ms
                            );
                            return Ok(merged);
                        }

                        let merged = bitfun_core::util::types::ConnectionTestResult {
                            success: true,
                            response_time_ms,
                            model_response: image_result.model_response.or(result.model_response),
                            message_code: result.message_code,
                            error_details: result.error_details,
                        };
                        info!(
                            "AI config connection test completed: model={}, success={}, response_time={}ms",
                            model_name, merged.success, merged.response_time_ms
                        );
                        return Ok(merged);
                    }
                    Err(e) => {
                        error!(
                            "AI config multimodal image input test failed unexpectedly: model={}, error={}",
                            model_name, e
                        );
                        return Err(format!("Connection test failed: {}", e));
                    }
                }
            }

            info!(
                "AI config connection test completed: model={}, success={}, response_time={}ms",
                model_name, result.success, result.response_time_ms
            );
            Ok(result)
        }
        Err(e) => {
            error!(
                "AI config connection test failed: model={}, error={}",
                model_name, e
            );
            Err(format!("Connection test failed: {}", e))
        }
    }
}

#[tauri::command]
pub async fn list_ai_models_by_config(
    request: ListAIModelsByConfigRequest,
) -> Result<Vec<bitfun_core::util::types::RemoteModelInfo>, String> {
    let config_name = request.config.name.clone();
    let ai_config = request
        .config
        .try_into()
        .map_err(|e| format!("Failed to convert configuration: {}", e))?;
    let ai_client = bitfun_core::infrastructure::ai::client::AIClient::new(ai_config);

    ai_client.list_models().await.map_err(|e| {
        error!(
            "Failed to list models for config: name={}, error={}",
            config_name, e
        );
        format!("Failed to list models: {}", e)
    })
}

#[tauri::command]
pub async fn fix_mermaid_code(
    state: State<'_, AppState>,
    request: FixMermaidCodeRequest,
) -> Result<String, String> {
    use bitfun_core::util::types::message::Message;

    let ai_client_guard = state.ai_client.read().await;
    let ai_client = ai_client_guard.as_ref().ok_or_else(|| {
        "AI client not initialized, please configure AI model in settings first".to_string()
    })?;

    const MERMAID_FIX_PROMPT: &str = r#"role:

You are a Mermaid diagram syntax expert specialized in fixing erroneous Mermaid code.

mission:

Fix syntax errors in the provided Mermaid diagram code to ensure it renders correctly.

workflow:

1. Analyze the provided Mermaid code and error message
2. Identify and fix the syntax errors
3. Preserve the original diagram structure and content
4. Return ONLY the fixed Mermaid code without any wrapper or explanation

context:

**Original Mermaid Code:**
```
{source_code}
```

**Error Message:**
```
{error_message}
```

**Output Requirements:**
- Return ONLY the fixed Mermaid code as plain text
- Do NOT wrap the code in markdown code blocks (no ```)
- Do NOT add any explanations or comments
- Preserve the original diagram type, direction, and node content
- Only fix syntax errors
"#;
    let prompt = MERMAID_FIX_PROMPT
        .replace("{source_code}", &request.source_code)
        .replace("{error_message}", &request.error_message);

    let messages = vec![Message::user(prompt)];

    let response = ai_client.send_message(messages, None).await.map_err(|e| {
        error!("Failed to call AI for Mermaid code fix: {}", e);
        format!("AI call failed: {}", e)
    })?;

    let fixed_code = response.text.trim().to_string();

    if fixed_code.is_empty() {
        error!("AI returned empty fix code for Mermaid diagram");
        return Err("AI returned empty fix code, please try again".to_string());
    }

    info!(
        "Mermaid code fixed successfully: original_length={}, fixed_length={}",
        request.source_code.len(),
        fixed_code.len()
    );
    Ok(fixed_code)
}

#[tauri::command]
pub async fn set_agent_model(
    state: State<'_, AppState>,
    agent_name: String,
    model_id: String,
) -> Result<String, String> {
    let config_service = &state.config_service;
    let global_config: bitfun_core::service::config::GlobalConfig = config_service
        .get_config(None)
        .await
        .map_err(|e| e.to_string())?;

    if !global_config.ai.models.iter().any(|m| m.id == model_id) {
        return Err(format!("Model does not exist: {}", model_id));
    }

    let path = format!("ai.agent_models.{}", agent_name);
    config_service
        .set_config(&path, model_id.clone())
        .await
        .map_err(|e| e.to_string())?;

    state.ai_client_factory.invalidate_cache();

    info!("Agent model set: agent={}, model={}", agent_name, model_id);
    Ok(format!(
        "Agent '{}' model has been set to: {}",
        agent_name, model_id
    ))
}

#[tauri::command]
pub async fn get_agent_models(
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let config_service = &state.config_service;
    let global_config: bitfun_core::service::config::GlobalConfig = config_service
        .get_config(None)
        .await
        .map_err(|e| e.to_string())?;

    Ok(global_config.ai.agent_models)
}

#[tauri::command]
pub async fn refresh_model_client(
    state: State<'_, AppState>,
    model_id: String,
) -> Result<String, String> {
    state.ai_client_factory.invalidate_model(&model_id);

    Ok(format!("Model '{}' has been refreshed", model_id))
}

#[tauri::command]
pub async fn get_app_state(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let health = state.get_health_status().await;
    let stats = state.get_statistics().await;

    let app_state = serde_json::json!({
        "status": if health.status == "healthy" { "Running" } else { "Error" },
        "message": health.message,
        "uptime_seconds": health.uptime_seconds,
        "sessions_created": stats.sessions_created,
        "messages_processed": stats.messages_processed,
        "tools_executed": stats.tools_executed,
        "services": health.services,
        "tool_count": state.get_tool_names().len(),
    });

    Ok(app_state)
}

#[tauri::command]
pub async fn update_app_status(
    _state: State<'_, AppState>,
    _request: UpdateAppStatusRequest,
) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn open_workspace(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    request: OpenWorkspaceRequest,
) -> Result<WorkspaceInfoDto, String> {
    match state
        .workspace_service
        .open_workspace(request.path.clone().into())
        .await
    {
        Ok(workspace_info) => {
            apply_active_workspace_context(&state, &app, &workspace_info).await;

            if let Err(e) = state
                .workspace_identity_watch_service
                .sync_watched_workspaces()
                .await
            {
                warn!(
                    "Failed to sync workspace identity watchers after open: {}",
                    e
                );
            }

            info!(
                "Workspace opened: name={}, path={}",
                workspace_info.name,
                workspace_info.root_path.display()
            );
            Ok(WorkspaceInfoDto::from_workspace_info(&workspace_info))
        }
        Err(e) => {
            error!("Failed to open workspace: {}", e);
            Err(format!("Failed to open workspace: {}", e))
        }
    }
}

#[tauri::command]
pub async fn open_remote_workspace(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    request: OpenRemoteWorkspaceRequest,
) -> Result<WorkspaceInfoDto, String> {
    use bitfun_core::service::remote_ssh::normalize_remote_workspace_path;
    use bitfun_core::service::workspace::WorkspaceCreateOptions;

    let remote_path = normalize_remote_workspace_path(&request.remote_path);

    let display_name = remote_path
        .split('/')
        .filter(|s| !s.is_empty())
        .last()
        .unwrap_or(remote_path.as_str())
        .to_string();

    let options = WorkspaceCreateOptions {
        scan_options: ScanOptions {
            calculate_statistics: false,
            ..ScanOptions::default()
        },
        auto_set_current: true,
        add_to_recent: true,
        workspace_kind: WorkspaceKind::Remote,
        assistant_id: None,
        display_name: Some(display_name),
        description: None,
        tags: Vec::new(),
        remote_connection_id: Some(request.connection_id.clone()),
    };

    match state
        .workspace_service
        .open_workspace_with_options(remote_path.clone().into(), options)
        .await
    {
        Ok(mut workspace_info) => {
            workspace_info.metadata.insert(
                "connectionId".to_string(),
                serde_json::Value::String(request.connection_id.clone()),
            );
            workspace_info.metadata.insert(
                "connectionName".to_string(),
                serde_json::Value::String(request.connection_name.clone()),
            );

            {
                let manager = state.workspace_service.get_manager();
                let mut manager = manager.write().await;
                if let Some(ws) = manager.get_workspaces_mut().get_mut(&workspace_info.id) {
                    ws.metadata = workspace_info.metadata.clone();
                }
            }
            if let Err(e) = state.workspace_service.manual_save().await {
                warn!("Failed to save workspace data after opening remote workspace: {}", e);
            }

            // Register the remote mapping before applying workspace context so session storage path
            // resolution (`get_effective_session_path`) and related setup see this connection.
            let remote_workspace = crate::api::RemoteWorkspace {
                connection_id: request.connection_id.clone(),
                connection_name: request.connection_name.clone(),
                remote_path: remote_path.clone(),
            };
            if let Err(e) = state.set_remote_workspace(remote_workspace).await {
                warn!("Failed to set remote workspace state: {}", e);
            }

            apply_active_workspace_context(&state, &app, &workspace_info).await;

            info!(
                "Remote workspace opened: name={}, remote_path={}, connection_id={}",
                workspace_info.name,
                workspace_info.root_path.display(),
                request.connection_id
            );
            Ok(WorkspaceInfoDto::from_workspace_info(&workspace_info))
        }
        Err(e) => {
            error!("Failed to open remote workspace: {}", e);
            Err(format!("Failed to open remote workspace: {}", e))
        }
    }
}

#[tauri::command]
pub async fn create_assistant_workspace(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    _request: CreateAssistantWorkspaceRequest,
) -> Result<WorkspaceInfoDto, String> {
    match state
        .workspace_service
        .create_assistant_workspace(None)
        .await
    {
        Ok(workspace_info) => {
            apply_active_workspace_context(&state, &app, &workspace_info).await;

            if let Err(e) = state
                .workspace_identity_watch_service
                .sync_watched_workspaces()
                .await
            {
                warn!(
                    "Failed to sync workspace identity watchers after assistant workspace creation: {}",
                    e
                );
            }

            info!(
                "Assistant workspace created: workspace_id={}, path={}",
                workspace_info.id,
                workspace_info.root_path.display()
            );
            Ok(WorkspaceInfoDto::from_workspace_info(&workspace_info))
        }
        Err(e) => {
            error!("Failed to create assistant workspace: {}", e);
            Err(format!("Failed to create assistant workspace: {}", e))
        }
    }
}

#[tauri::command]
pub async fn delete_assistant_workspace(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    request: DeleteAssistantWorkspaceRequest,
) -> Result<(), String> {
    let workspace_info = state
        .workspace_service
        .get_workspace(&request.workspace_id)
        .await
        .ok_or_else(|| format!("Assistant workspace not found: {}", request.workspace_id))?;

    if workspace_info.workspace_kind != WorkspaceKind::Assistant {
        return Err(format!(
            "Workspace is not an assistant workspace: {}",
            request.workspace_id
        ));
    }

    let assistant_id = workspace_info
        .assistant_id
        .clone()
        .ok_or_else(|| "Default assistant workspace cannot be deleted".to_string())?;

    if !state
        .workspace_service
        .is_assistant_workspace_path(&workspace_info.root_path)
    {
        return Err(format!(
            "Workspace path is not a managed assistant workspace: {}",
            workspace_info.root_path.display()
        ));
    }

    let is_active_workspace = state
        .workspace_service
        .get_current_workspace()
        .await
        .map(|workspace| workspace.id == request.workspace_id)
        .unwrap_or(false);

    if is_active_workspace {
        state
            .workspace_service
            .close_workspace(&request.workspace_id)
            .await
            .map_err(|e| format!("Failed to close assistant workspace before deletion: {}", e))?;
    }

    let workspace_path = workspace_info.root_path.to_string_lossy().to_string();

    state
        .filesystem_service
        .delete_directory(&workspace_path, true)
        .await
        .map_err(|e| format!("Failed to delete assistant workspace files: {}", e))?;

    state
        .workspace_service
        .remove_workspace(&request.workspace_id)
        .await
        .map_err(|e| format!("Failed to remove assistant workspace state: {}", e))?;

    if let Some(current_workspace) = state.workspace_service.get_current_workspace().await {
        apply_active_workspace_context(&state, &app, &current_workspace).await;
    } else {
        clear_active_workspace_context(&state, &app).await;
    }

    if let Err(e) = state
        .workspace_identity_watch_service
        .sync_watched_workspaces()
        .await
    {
        warn!(
            "Failed to sync workspace identity watchers after assistant workspace deletion: {}",
            e
        );
    }

    info!(
        "Assistant workspace deleted: workspace_id={}, assistant_id={}, path={}",
        request.workspace_id,
        assistant_id,
        workspace_info.root_path.display()
    );

    Ok(())
}

async fn clear_directory_contents(directory: &Path) -> Result<(), String> {
    tokio::fs::create_dir_all(directory).await.map_err(|e| {
        format!(
            "Failed to create workspace directory '{}': {}",
            directory.display(),
            e
        )
    })?;

    let mut entries = tokio::fs::read_dir(directory).await.map_err(|e| {
        format!(
            "Failed to read workspace directory '{}': {}",
            directory.display(),
            e
        )
    })?;

    while let Some(entry) = entries.next_entry().await.map_err(|e| {
        format!(
            "Failed to iterate workspace directory '{}': {}",
            directory.display(),
            e
        )
    })? {
        let entry_path = entry.path();
        let file_type = entry.file_type().await.map_err(|e| {
            format!(
                "Failed to inspect workspace entry '{}': {}",
                entry_path.display(),
                e
            )
        })?;

        if file_type.is_dir() {
            tokio::fs::remove_dir_all(&entry_path).await.map_err(|e| {
                format!(
                    "Failed to remove workspace directory '{}': {}",
                    entry_path.display(),
                    e
                )
            })?;
        } else {
            tokio::fs::remove_file(&entry_path).await.map_err(|e| {
                format!(
                    "Failed to remove workspace file '{}': {}",
                    entry_path.display(),
                    e
                )
            })?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn reset_assistant_workspace(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    request: ResetAssistantWorkspaceRequest,
) -> Result<WorkspaceInfoDto, String> {
    let workspace_info = state
        .workspace_service
        .get_workspace(&request.workspace_id)
        .await
        .ok_or_else(|| format!("Assistant workspace not found: {}", request.workspace_id))?;

    if workspace_info.workspace_kind != WorkspaceKind::Assistant {
        return Err(format!(
            "Workspace is not an assistant workspace: {}",
            request.workspace_id
        ));
    }

    if !state
        .workspace_service
        .is_assistant_workspace_path(&workspace_info.root_path)
    {
        return Err(format!(
            "Workspace path is not a managed assistant workspace: {}",
            workspace_info.root_path.display()
        ));
    }

    clear_directory_contents(&workspace_info.root_path).await?;

    bitfun_core::service::reset_workspace_persona_files_to_default(&workspace_info.root_path)
        .await
        .map_err(|e| format!("Failed to restore assistant workspace persona files: {}", e))?;

    let updated_workspace = state
        .workspace_service
        .rescan_workspace(&request.workspace_id)
        .await
        .map_err(|e| format!("Failed to rescan assistant workspace after reset: {}", e))?;

    if state
        .workspace_service
        .get_current_workspace()
        .await
        .map(|workspace| workspace.id == request.workspace_id)
        .unwrap_or(false)
    {
        apply_active_workspace_context(&state, &app, &updated_workspace).await;
    }

    info!(
        "Assistant workspace reset: workspace_id={}, assistant_id={:?}, path={}",
        request.workspace_id,
        workspace_info.assistant_id,
        workspace_info.root_path.display()
    );

    Ok(WorkspaceInfoDto::from_workspace_info(&updated_workspace))
}

#[tauri::command]
pub async fn close_workspace(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    request: CloseWorkspaceRequest,
) -> Result<(), String> {
    let closing = state
        .workspace_service
        .get_workspace(&request.workspace_id)
        .await;

    match state
        .workspace_service
        .close_workspace(&request.workspace_id)
        .await
    {
        Ok(_) => {
            if let Some(ref ws) = closing {
                if ws.workspace_kind == WorkspaceKind::Remote {
                    if let Some(rw) = remote_workspace_from_info(ws) {
                        state
                            .unregister_remote_workspace_entry(&rw.connection_id, &rw.remote_path)
                            .await;
                    }
                }
            }

            if let Some(workspace_info) = state.workspace_service.get_current_workspace().await {
                apply_active_workspace_context(&state, &app, &workspace_info).await;
            } else {
                clear_active_workspace_context(&state, &app).await;
            }

            info!("Workspace closed: workspace_id={}", request.workspace_id);
            Ok(())
        }
        Err(e) => {
            error!("Failed to close workspace: {}", e);
            Err(format!("Failed to close workspace: {}", e))
        }
    }
}

#[tauri::command]
pub async fn set_active_workspace(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    request: SetActiveWorkspaceRequest,
) -> Result<WorkspaceInfoDto, String> {
    match state
        .workspace_service
        .set_active_workspace(&request.workspace_id)
        .await
    {
        Ok(_) => {
            let workspace_info = state
                .workspace_service
                .get_current_workspace()
                .await
                .ok_or_else(|| "Active workspace not found after switching".to_string())?;

            apply_active_workspace_context(&state, &app, &workspace_info).await;

            info!(
                "Active workspace changed: workspace_id={}, path={}",
                workspace_info.id,
                workspace_info.root_path.display()
            );

            Ok(WorkspaceInfoDto::from_workspace_info(&workspace_info))
        }
        Err(e) => {
            error!("Failed to set active workspace: {}", e);
            Err(format!("Failed to set active workspace: {}", e))
        }
    }
}

#[tauri::command]
pub async fn reorder_opened_workspaces(
    state: State<'_, AppState>,
    request: ReorderOpenedWorkspacesRequest,
) -> Result<(), String> {
    match state
        .workspace_service
        .reorder_opened_workspaces(request.workspace_ids.clone())
        .await
    {
        Ok(_) => {
            info!(
                "Opened workspaces reordered: count={}",
                request.workspace_ids.len()
            );
            Ok(())
        }
        Err(e) => {
            error!("Failed to reorder opened workspaces: {}", e);
            Err(format!("Failed to reorder opened workspaces: {}", e))
        }
    }
}

#[tauri::command]
pub async fn get_current_workspace(
    state: State<'_, AppState>,
) -> Result<Option<WorkspaceInfoDto>, String> {
    let workspace_service = &state.workspace_service;
    Ok(workspace_service
        .get_current_workspace()
        .await
        .map(|info| WorkspaceInfoDto::from_workspace_info(&info)))
}

#[tauri::command]
pub async fn get_recent_workspaces(
    state: State<'_, AppState>,
) -> Result<Vec<WorkspaceInfoDto>, String> {
    let workspace_service = &state.workspace_service;
    Ok(workspace_service
        .get_recent_workspaces()
        .await
        .into_iter()
        .map(|info| WorkspaceInfoDto::from_workspace_info(&info))
        .collect())
}

#[tauri::command]
pub async fn cleanup_invalid_workspaces(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<usize, String> {
    match state.workspace_service.cleanup_invalid_workspaces().await {
        Ok(removed_count) => {
            if let Some(workspace_info) = state.workspace_service.get_current_workspace().await {
                apply_active_workspace_context(&state, &app, &workspace_info).await;
            } else {
                clear_active_workspace_context(&state, &app).await;
            }

            if let Err(e) = state
                .workspace_identity_watch_service
                .sync_watched_workspaces()
                .await
            {
                warn!(
                    "Failed to sync workspace identity watchers after workspace cleanup: {}",
                    e
                );
            }

            info!(
                "Invalid workspaces cleaned up: removed_count={}",
                removed_count
            );
            Ok(removed_count)
        }
        Err(e) => {
            error!("Failed to cleanup invalid workspaces: {}", e);
            Err(format!("Failed to cleanup invalid workspaces: {}", e))
        }
    }
}

#[tauri::command]
pub async fn get_opened_workspaces(
    state: State<'_, AppState>,
) -> Result<Vec<WorkspaceInfoDto>, String> {
    let workspace_service = &state.workspace_service;
    Ok(workspace_service
        .get_opened_workspaces()
        .await
        .into_iter()
        .map(|info| WorkspaceInfoDto::from_workspace_info(&info))
        .collect())
}

#[tauri::command]
pub async fn scan_workspace_info(
    state: State<'_, AppState>,
    request: ScanWorkspaceInfoRequest,
) -> Result<Option<WorkspaceInfoDto>, String> {
    let workspace_path = std::path::PathBuf::from(&request.workspace_path);

    if let Some(existing_workspace) = state
        .workspace_service
        .get_workspace_by_path(&workspace_path)
        .await
    {
        return state
            .workspace_service
            .rescan_workspace(&existing_workspace.id)
            .await
            .map(|workspace| Some(WorkspaceInfoDto::from_workspace_info(&workspace)))
            .map_err(|e| format!("Failed to rescan workspace: {}", e));
    }

    WorkspaceInfo::new(
        workspace_path,
        WorkspaceOpenOptions {
            scan_options: ScanOptions::default(),
            auto_set_current: false,
            add_to_recent: false,
            workspace_kind: WorkspaceKind::Normal,
            assistant_id: None,
            display_name: None,
            remote_connection_id: None,
        },
    )
    .await
    .map(|workspace| Some(WorkspaceInfoDto::from_workspace_info(&workspace)))
    .map_err(|e| format!("Failed to scan workspace info: {}", e))
}

#[tauri::command]
pub async fn get_file_tree(
    state: State<'_, AppState>,
    request: GetFileTreeRequest,
) -> Result<serde_json::Value, String> {
    use std::path::Path;
    use bitfun_core::service::remote_ssh::workspace_state::is_remote_path;

    let is_remote = is_remote_path(&request.path).await;
    if !is_remote {
        let path_buf = Path::new(&request.path);
        if !path_buf.exists() {
            return Err("Directory does not exist".to_string());
        }
        if !path_buf.is_dir() {
            return Err("Path is not a directory".to_string());
        }
    }

    let filesystem_service = &state.filesystem_service;
    match filesystem_service.build_file_tree(&request.path).await {
        Ok(nodes) => {
            fn convert_node_to_json(
                node: bitfun_core::infrastructure::FileTreeNode,
            ) -> serde_json::Value {
                let mut json = serde_json::json!({
                    "path": node.path,
                    "name": node.name,
                    "isDirectory": node.is_directory,
                    "size": node.size,
                    "extension": node.extension,
                    "lastModified": node.last_modified
                });

                if let Some(children) = node.children {
                    json["children"] = serde_json::Value::Array(
                        children.into_iter().map(convert_node_to_json).collect(),
                    );
                }

                json
            }

            let root_name = Path::new(&request.path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&request.path);

            let root_node = serde_json::json!({
                "path": request.path,
                "name": root_name,
                "isDirectory": true,
                "size": null,
                "extension": null,
                "lastModified": null,
                "children": nodes.into_iter().map(convert_node_to_json).collect::<Vec<_>>()
            });

            Ok(serde_json::json!([root_node]))
        }
        Err(e) => {
            error!("Failed to build file tree: {}", e);
            Err(format!("Failed to build file tree: {}", e))
        }
    }
}

#[tauri::command]
pub async fn get_directory_children(
    state: State<'_, AppState>,
    request: GetDirectoryChildrenRequest,
) -> Result<serde_json::Value, String> {
    use std::path::Path;
    use bitfun_core::service::remote_ssh::workspace_state::is_remote_path;

    let is_remote = is_remote_path(&request.path).await;
    if !is_remote {
        let path_buf = Path::new(&request.path);
        if !path_buf.exists() {
            return Err("Directory does not exist".to_string());
        }
        if !path_buf.is_dir() {
            return Err("Path is not a directory".to_string());
        }
    }

    let filesystem_service = &state.filesystem_service;
    match filesystem_service
        .get_directory_contents(&request.path)
        .await
    {
        Ok(nodes) => {
            let json_nodes: Vec<serde_json::Value> = nodes
                .into_iter()
                .map(|node| {
                    serde_json::json!({
                        "path": node.path,
                        "name": node.name,
                        "isDirectory": node.is_directory,
                        "size": node.size,
                        "extension": node.extension,
                        "lastModified": node.last_modified
                    })
                })
                .collect();

            Ok(serde_json::json!(json_nodes))
        }
        Err(e) => {
            error!("Failed to get directory children: {}", e);
            Err(format!("Failed to get directory children: {}", e))
        }
    }
}

#[tauri::command]
pub async fn get_directory_children_paginated(
    state: State<'_, AppState>,
    request: GetDirectoryChildrenPaginatedRequest,
) -> Result<serde_json::Value, String> {
    use std::path::Path;
    use bitfun_core::service::remote_ssh::workspace_state::is_remote_path;

    let offset = request.offset.unwrap_or(0);
    let limit = request.limit.unwrap_or(100);

    let is_remote = is_remote_path(&request.path).await;
    if !is_remote {
        let path_buf = Path::new(&request.path);
        if !path_buf.exists() {
            return Err("Directory does not exist".to_string());
        }
        if !path_buf.is_dir() {
            return Err("Path is not a directory".to_string());
        }
    }

    let filesystem_service = &state.filesystem_service;
    match filesystem_service
        .get_directory_contents(&request.path)
        .await
    {
        Ok(nodes) => {
            let total = nodes.len();
            let has_more = total > offset + limit;
            let page_nodes: Vec<_> = nodes.into_iter().skip(offset).take(limit).collect();
            let json_nodes: Vec<serde_json::Value> = page_nodes
                .into_iter()
                .map(|node| {
                    serde_json::json!({
                        "path": node.path,
                        "name": node.name,
                        "isDirectory": node.is_directory,
                        "size": node.size,
                        "extension": node.extension,
                        "lastModified": node.last_modified
                    })
                })
                .collect();

            Ok(serde_json::json!({
                "children": json_nodes,
                "total": total,
                "hasMore": has_more,
                "offset": offset,
                "limit": limit
            }))
        }
        Err(e) => {
            error!("Failed to get directory children: {}", e);
            Err(format!("Failed to get directory children: {}", e))
        }
    }
}

#[tauri::command]
pub async fn read_file_content(
    state: State<'_, AppState>,
    request: ReadFileContentRequest,
) -> Result<String, String> {
    if let Some(entry) = lookup_remote_entry_for_path(&state, &request.file_path).await {
        let remote_fs = state.get_remote_file_service_async().await
            .map_err(|e| format!("Remote file service not available: {}", e))?;
        let bytes = remote_fs.read_file(&entry.connection_id, &request.file_path).await
            .map_err(|e| format!("Failed to read remote file: {}", e))?;
        return String::from_utf8(bytes)
            .map_err(|e| format!("File is not valid UTF-8: {}", e));
    }

    match state.filesystem_service.read_file(&request.file_path).await {
        Ok(result) => Ok(result.content),
        Err(e) => {
            error!(
                "Failed to read file content: path={}, error={}",
                request.file_path, e
            );
            Err(format!("Failed to read file content: {}", e))
        }
    }
}

#[tauri::command]
pub async fn write_file_content(
    state: State<'_, AppState>,
    request: WriteFileContentRequest,
) -> Result<(), String> {
    if let Some(entry) = lookup_remote_entry_for_path(&state, &request.file_path).await {
        let remote_fs = state.get_remote_file_service_async().await
            .map_err(|e| format!("Remote file service not available: {}", e))?;
        remote_fs.write_file(&entry.connection_id, &request.file_path, request.content.as_bytes()).await
            .map_err(|e| format!("Failed to write remote file: {}", e))?;
        return Ok(());
    }

    let full_path = request.file_path;
    let mut options = FileOperationOptions::default();
    options.backup_on_overwrite = false;

    match state
        .filesystem_service
        .write_file_with_options(&full_path, &request.content, options)
        .await
    {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("Failed to write file: path={}, error={}", full_path, e);
            Err(format!("Failed to write file {}, error: {}", full_path, e))
        }
    }
}

#[tauri::command]
pub async fn reset_workspace_persona_files(
    state: State<'_, AppState>,
    request: ResetWorkspacePersonaFilesRequest,
) -> Result<(), String> {
    let workspace_path = std::path::PathBuf::from(&request.workspace_path);

    if !state
        .workspace_service
        .is_assistant_workspace_path(&workspace_path)
    {
        return Err(format!(
            "Workspace is not a managed assistant workspace: {}",
            request.workspace_path
        ));
    }

    bitfun_core::service::reset_workspace_persona_files_to_default(&workspace_path)
        .await
        .map_err(|e| {
            error!(
                "Failed to reset workspace persona files: path={} error={}",
                request.workspace_path, e
            );
            format!("Failed to reset workspace persona files: {}", e)
        })?;

    info!(
        "Workspace persona files reset to defaults: path={}",
        request.workspace_path
    );

    Ok(())
}

#[tauri::command]
pub async fn check_path_exists(
    state: State<'_, AppState>,
    request: CheckPathExistsRequest,
) -> Result<bool, String> {
    if let Some(entry) = lookup_remote_entry_for_path(&state, &request.path).await {
        let remote_fs = state.get_remote_file_service_async().await
            .map_err(|e| format!("Remote file service not available: {}", e))?;
        return remote_fs.exists(&entry.connection_id, &request.path).await
            .map_err(|e| format!("Failed to check remote path: {}", e));
    }

    let path = std::path::Path::new(&request.path);
    Ok(path.exists())
}

#[tauri::command]
pub async fn get_file_metadata(
    state: State<'_, AppState>,
    request: GetFileMetadataRequest,
) -> Result<serde_json::Value, String> {
    use std::time::SystemTime;

    if let Some(entry) = lookup_remote_entry_for_path(&state, &request.path).await {
        let remote_fs = state.get_remote_file_service_async().await
            .map_err(|e| format!("Remote file service not available: {}", e))?;

        // Use SFTP stat for stable mtime/size. Returning `SystemTime::now()` as `modified` caused
        // the editor's 5s poll to always see a "newer" file and spam the external-change dialog.
        let stat_entry = remote_fs
            .stat(&entry.connection_id, &request.path)
            .await
            .map_err(|e| format!("Failed to stat remote file: {}", e))?;

        let (is_file, is_dir, size, modified) = match stat_entry {
            Some(e) => (
                e.is_file,
                e.is_dir,
                e.size.unwrap_or(0),
                e.modified.unwrap_or(0),
            ),
            None => (false, false, 0, 0),
        };

        return Ok(serde_json::json!({
            "path": request.path,
            "modified": modified,
            "size": size,
            "is_file": is_file,
            "is_dir": is_dir,
            "is_remote": true
        }));
    }

    let path = std::path::Path::new(&request.path);
    match std::fs::metadata(path) {
        Ok(metadata) => {
            let modified = metadata
                .modified()
                .unwrap_or(SystemTime::UNIX_EPOCH)
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            let size = metadata.len();
            let is_file = metadata.is_file();
            let is_dir = metadata.is_dir();

            Ok(serde_json::json!({
                "path": request.path,
                "modified": modified,
                "size": size,
                "is_file": is_file,
                "is_dir": is_dir
            }))
        }
        Err(e) => {
            error!(
                "Failed to get file metadata: path={}, error={}",
                request.path, e
            );
            Err(format!("Failed to get file metadata: {}", e))
        }
    }
}

/// Returns SHA-256 hex (lowercase) of file bytes after the same normalization as the web editor
/// external-sync check, so the UI can compare with a local hash without transferring file contents.
#[tauri::command]
pub async fn get_file_editor_sync_hash(
    state: State<'_, AppState>,
    request: GetFileMetadataRequest,
) -> Result<serde_json::Value, String> {
    if let Some(entry) = lookup_remote_entry_for_path(&state, &request.path).await {
        let remote_fs = state
            .get_remote_file_service_async()
            .await
            .map_err(|e| format!("Remote file service not available: {}", e))?;
        let bytes = remote_fs
            .read_file(&entry.connection_id, &request.path)
            .await
            .map_err(|e| format!("Failed to read remote file: {}", e))?;
        let hash = state
            .filesystem_service
            .editor_sync_sha256_hex_from_raw_bytes(&bytes);
        return Ok(serde_json::json!({
            "path": request.path,
            "hash": hash,
            "is_remote": true
        }));
    }

    let hash = state
        .filesystem_service
        .editor_sync_content_sha256_hex(&request.path)
        .await
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "path": request.path,
        "hash": hash
    }))
}

#[tauri::command]
pub async fn rename_file(
    state: State<'_, AppState>,
    request: RenameFileRequest,
) -> Result<(), String> {
    if let Some(entry) = lookup_remote_entry_for_path(&state, &request.old_path).await {
        let remote_fs = state.get_remote_file_service_async().await
            .map_err(|e| format!("Remote file service not available: {}", e))?;
        remote_fs.rename(&entry.connection_id, &request.old_path, &request.new_path).await
            .map_err(|e| format!("Failed to rename remote file: {}", e))?;
        return Ok(());
    }

    state
        .filesystem_service
        .move_file(&request.old_path, &request.new_path)
        .await
        .map_err(|e| format!("Failed to rename file: {}", e))?;

    Ok(())
}

/// Copy a local file to another local path (binary-safe). Used for export and drag-upload into local workspaces.
#[tauri::command]
pub async fn export_local_file_to_path(request: ExportLocalFileRequest) -> Result<(), String> {
    let src = request.source_path;
    let dst = request.destination_path;
    tokio::task::spawn_blocking(move || {
        let dst_path = Path::new(&dst);
        if let Some(parent) = dst_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        std::fs::copy(&src, &dst).map_err(|e| e.to_string())?;
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn delete_file(
    state: State<'_, AppState>,
    request: DeleteFileRequest,
) -> Result<(), String> {
    if let Some(entry) = lookup_remote_entry_for_path(&state, &request.path).await {
        let remote_fs = state.get_remote_file_service_async().await
            .map_err(|e| format!("Remote file service not available: {}", e))?;
        remote_fs.remove_file(&entry.connection_id, &request.path).await
            .map_err(|e| format!("Failed to delete remote file: {}", e))?;
        return Ok(());
    }

    state
        .filesystem_service
        .delete_file(&request.path)
        .await
        .map_err(|e| format!("Failed to delete file: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn delete_directory(
    state: State<'_, AppState>,
    request: DeleteDirectoryRequest,
) -> Result<(), String> {
    let recursive = request.recursive.unwrap_or(false);

    if let Some(entry) = lookup_remote_entry_for_path(&state, &request.path).await {
        let remote_fs = state.get_remote_file_service_async().await
            .map_err(|e| format!("Remote file service not available: {}", e))?;
        if recursive {
            remote_fs.remove_dir_all(&entry.connection_id, &request.path).await
                .map_err(|e| format!("Failed to delete remote directory: {}", e))?;
        } else {
            remote_fs.remove_dir_all(&entry.connection_id, &request.path).await
                .map_err(|e| format!("Failed to delete remote directory: {}", e))?;
        }
        return Ok(());
    }

    state
        .filesystem_service
        .delete_directory(&request.path, recursive)
        .await
        .map_err(|e| format!("Failed to delete directory: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn create_file(
    state: State<'_, AppState>,
    request: CreateFileRequest,
) -> Result<(), String> {
    if let Some(entry) = lookup_remote_entry_for_path(&state, &request.path).await {
        let remote_fs = state.get_remote_file_service_async().await
            .map_err(|e| format!("Remote file service not available: {}", e))?;
        remote_fs.write_file(&entry.connection_id, &request.path, b"").await
            .map_err(|e| format!("Failed to create remote file: {}", e))?;
        return Ok(());
    }

    let options = FileOperationOptions::default();
    state
        .filesystem_service
        .write_file_with_options(&request.path, "", options)
        .await
        .map_err(|e| format!("Failed to create file: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn create_directory(
    state: State<'_, AppState>,
    request: CreateDirectoryRequest,
) -> Result<(), String> {
    if let Some(entry) = lookup_remote_entry_for_path(&state, &request.path).await {
        let remote_fs = state.get_remote_file_service_async().await
            .map_err(|e| format!("Remote file service not available: {}", e))?;
        remote_fs.create_dir_all(&entry.connection_id, &request.path).await
            .map_err(|e| format!("Failed to create remote directory: {}", e))?;
        return Ok(());
    }

    state
        .filesystem_service
        .create_directory(&request.path)
        .await
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ListDirectoryFilesRequest {
    pub path: String,
    pub extensions: Option<Vec<String>>,
}

#[tauri::command]
pub async fn list_directory_files(
    state: State<'_, AppState>,
    request: ListDirectoryFilesRequest,
) -> Result<Vec<String>, String> {
    use std::path::Path;

    if let Some(entry) = lookup_remote_entry_for_path(&state, &request.path).await {
        let remote_fs = state.get_remote_file_service_async().await
            .map_err(|e| format!("Remote file service not available: {}", e))?;
        let entries = remote_fs.read_dir(&entry.connection_id, &request.path).await
            .map_err(|e| format!("Failed to read remote directory: {}", e))?;
        let mut files: Vec<String> = entries.into_iter()
            .filter(|e| !e.is_dir)
            .filter(|e| {
                if let Some(ref extensions) = request.extensions {
                    if let Some(ext) = Path::new(&e.name).extension().and_then(|x| x.to_str()) {
                        extensions.iter().any(|x| x.eq_ignore_ascii_case(ext))
                    } else {
                        false
                    }
                } else {
                    true
                }
            })
            .map(|e| e.name)
            .collect();
        files.sort();
        return Ok(files);
    }

    let dir_path = Path::new(&request.path);
    if !dir_path.exists() {
        return Ok(Vec::new());
    }

    if !dir_path.is_dir() {
        return Err("Path is not a directory".to_string());
    }

    let mut files = Vec::new();
    let entries =
        std::fs::read_dir(dir_path).map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_file() {
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(ref extensions) = request.extensions {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
                            files.push(file_name.to_string());
                        }
                    }
                } else {
                    files.push(file_name.to_string());
                }
            }
        }
    }

    files.sort();
    Ok(files)
}

#[tauri::command]
pub async fn reveal_in_explorer(request: RevealInExplorerRequest) -> Result<(), String> {
    let path = std::path::Path::new(&request.path);
    if !path.exists() {
        return Err(format!("Path does not exist: {}", request.path));
    }
    let is_directory = path.is_dir();

    #[cfg(target_os = "windows")]
    {
        if is_directory {
            let normalized_path = request.path.replace("/", "\\");
            bitfun_core::util::process_manager::create_command("explorer")
                .arg(&normalized_path)
                .spawn()
                .map_err(|e| format!("Failed to open explorer: {}", e))?;
        } else {
            let normalized_path = request.path.replace("/", "\\");
            bitfun_core::util::process_manager::create_command("explorer")
                .args(&["/select,", &normalized_path])
                .spawn()
                .map_err(|e| format!("Failed to open explorer: {}", e))?;
        }
    }

    #[cfg(target_os = "macos")]
    {
        if is_directory {
            bitfun_core::util::process_manager::create_command("open")
                .arg(&request.path)
                .spawn()
                .map_err(|e| format!("Failed to open finder: {}", e))?;
        } else {
            bitfun_core::util::process_manager::create_command("open")
                .args(&["-R", &request.path])
                .spawn()
                .map_err(|e| format!("Failed to open finder: {}", e))?;
        }
    }

    #[cfg(target_os = "linux")]
    {
        let target = if is_directory {
            path.to_path_buf()
        } else {
            path.parent()
                .ok_or_else(|| "Failed to get parent directory".to_string())?
                .to_path_buf()
        };
        bitfun_core::util::process_manager::create_command("xdg-open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Failed to open file manager: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn search_files(
    state: State<'_, AppState>,
    request: SearchFilesRequest,
) -> Result<serde_json::Value, String> {
    use bitfun_core::service::filesystem::FileSearchOptions;

    let options = FileSearchOptions {
        include_content: request.search_content,
        case_sensitive: request.case_sensitive,
        use_regex: request.use_regex,
        whole_word: request.whole_word,
        max_results: None,
        file_extensions: None,
        include_directories: true,
    };

    match state
        .filesystem_service
        .search_files(&request.root_path, &request.pattern, options)
        .await
    {
        Ok(results) => {
            let json_results: Vec<serde_json::Value> = results
                .into_iter()
                .map(|result| {
                    serde_json::json!({
                        "path": result.path,
                        "name": result.name,
                        "isDirectory": result.is_directory,
                        "matchType": match result.match_type {
                            SearchMatchType::FileName => "fileName",
                            SearchMatchType::Content => "content",
                        },
                        "lineNumber": result.line_number,
                        "matchedContent": result.matched_content,
                    })
                })
                .collect();

            info!(
                "File search completed: root_path={}, pattern={}, results_count={}",
                request.root_path,
                request.pattern,
                json_results.len()
            );
            Ok(serde_json::json!(json_results))
        }
        Err(e) => {
            error!(
                "Failed to search files: root_path={}, pattern={}, error={}",
                request.root_path, request.pattern, e
            );
            Err(format!("Failed to search files: {}", e))
        }
    }
}

#[tauri::command]
pub async fn reload_global_config() -> Result<String, String> {
    match bitfun_core::service::config::reload_global_config().await {
        Ok(_) => {
            info!("Global config reloaded");
            Ok("Configuration reloaded successfully".to_string())
        }
        Err(e) => {
            error!("Failed to reload global config: {}", e);
            Err(format!("Failed to reload configuration: {}", e))
        }
    }
}

#[tauri::command]
pub async fn get_global_config_status() -> Result<bool, String> {
    Ok(bitfun_core::service::config::GlobalConfigManager::is_initialized())
}

#[tauri::command]
pub async fn subscribe_config_updates() -> Result<(), String> {
    if let Some(mut receiver) = bitfun_core::service::config::subscribe_config_updates() {
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                debug!("Config update event: {:?}", event);
            }
        });
        Ok(())
    } else {
        Err("Config update subscription not available".to_string())
    }
}

#[tauri::command]
pub async fn get_model_configs(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let config_service = &state.config_service;

    match config_service.get_ai_models().await {
        Ok(models) => {
            let model_configs: Vec<serde_json::Value> = models
                .into_iter()
                .map(|model| serde_json::to_value(model).unwrap_or_default())
                .collect();

            Ok(model_configs)
        }
        Err(e) => {
            error!("Failed to get AI model configs: {}", e);
            Err(format!("Failed to get model configurations: {}", e))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct IdeControlResultRequest {
    pub request_id: String,
    pub success: bool,
    pub message: Option<String>,
    pub error: Option<String>,
    pub timestamp: i64,
}

#[tauri::command]
pub async fn report_ide_control_result(request: IdeControlResultRequest) -> Result<(), String> {
    if !request.success {
        if let Some(error) = &request.error {
            error!(
                "IDE Control operation failed: request_id={}, error={}",
                request.request_id, error
            );
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn start_file_watch(path: String, recursive: Option<bool>) -> Result<(), String> {
    file_watcher::start_file_watch(path, recursive).await
}

#[tauri::command]
pub async fn stop_file_watch(path: String) -> Result<(), String> {
    file_watcher::stop_file_watch(path).await
}

#[tauri::command]
pub async fn get_watched_paths() -> Result<Vec<String>, String> {
    file_watcher::get_watched_paths().await
}
