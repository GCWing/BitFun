//! Commands API - Core Application Commands

use crate::api::app_state::AppState;
use crate::api::dto::WorkspaceInfoDto;
use bitfun_core::infrastructure::{file_watcher, FileOperationOptions, SearchMatchType};
use log::{debug, error, info, warn};
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
pub struct OpenWorkspaceRequest {
    pub path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanWorkspaceInfoRequest {
    pub workspace_path: String,
}

#[derive(Debug, Deserialize)]
pub struct TestAIConfigConnectionRequest {
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

#[tauri::command]
pub async fn initialize_global_state(_state: State<'_, AppState>) -> Result<String, String> {
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
    _app: tauri::AppHandle,
    request: OpenWorkspaceRequest,
) -> Result<WorkspaceInfoDto, String> {
    match state
        .workspace_service
        .open_workspace(request.path.clone().into())
        .await
    {
        Ok(workspace_info) => {
            *state.workspace_path.write().await = Some(workspace_info.root_path.clone());

            if let Err(e) = bitfun_core::service::snapshot::initialize_global_snapshot_manager(
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
                let _ = crate::macos_menubar::set_macos_menubar_with_mode(
                    &_app,
                    &language,
                    crate::macos_menubar::MenubarMode::Workspace,
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
pub async fn close_workspace(
    state: State<'_, AppState>,
    _app: tauri::AppHandle,
) -> Result<(), String> {
    match state.workspace_service.close_workspace("default").await {
        Ok(_) => {
            *state.workspace_path.write().await = None;

            state.ai_rules_service.clear_workspace().await;

            state.agent_registry.clear_custom_subagents();

            #[cfg(target_os = "macos")]
            {
                let language = state
                    .config_service
                    .get_config::<String>(Some("app.language"))
                    .await
                    .unwrap_or_else(|_| "zh-CN".to_string());
                let _ = crate::macos_menubar::set_macos_menubar_with_mode(
                    &_app,
                    &language,
                    crate::macos_menubar::MenubarMode::Startup,
                );
            }

            info!("Workspace closed");
            Ok(())
        }
        Err(e) => {
            error!("Failed to close workspace: {}", e);
            Err(format!("Failed to close workspace: {}", e))
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
pub async fn scan_workspace_info(
    state: State<'_, AppState>,
    request: ScanWorkspaceInfoRequest,
) -> Result<serde_json::Value, String> {
    let path = std::path::Path::new(&request.workspace_path);
    let name = path.file_name().unwrap_or_default().to_string_lossy();

    let files_count = match state
        .filesystem_service
        .build_file_tree(&request.workspace_path)
        .await
    {
        Ok(nodes) => nodes.len(),
        Err(_) => 0,
    };

    Ok(serde_json::json!({
        "path": request.workspace_path,
        "name": name,
        "type": "workspace",
        "files_count": files_count
    }))
}

#[tauri::command]
pub async fn get_file_tree(
    state: State<'_, AppState>,
    request: GetFileTreeRequest,
) -> Result<serde_json::Value, String> {
    use std::path::Path;

    let path_buf = Path::new(&request.path);
    if !path_buf.exists() {
        return Err("Directory does not exist".to_string());
    }

    if !path_buf.is_dir() {
        return Err("Path is not a directory".to_string());
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

            let root_name = path_buf
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

    let path_buf = Path::new(&request.path);
    if !path_buf.exists() {
        return Err("Directory does not exist".to_string());
    }

    if !path_buf.is_dir() {
        return Err("Path is not a directory".to_string());
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

    let offset = request.offset.unwrap_or(0);
    let limit = request.limit.unwrap_or(100);

    let path_buf = Path::new(&request.path);
    if !path_buf.exists() {
        return Err("Directory does not exist".to_string());
    }

    if !path_buf.is_dir() {
        return Err("Path is not a directory".to_string());
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
pub async fn check_path_exists(request: CheckPathExistsRequest) -> Result<bool, String> {
    let path = std::path::Path::new(&request.path);
    Ok(path.exists())
}

#[tauri::command]
pub async fn get_file_metadata(
    request: GetFileMetadataRequest,
) -> Result<serde_json::Value, String> {
    use std::fs;
    use std::time::SystemTime;

    let path = std::path::Path::new(&request.path);

    match fs::metadata(path) {
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

#[tauri::command]
pub async fn rename_file(
    state: State<'_, AppState>,
    request: RenameFileRequest,
) -> Result<(), String> {
    state
        .filesystem_service
        .move_file(&request.old_path, &request.new_path)
        .await
        .map_err(|e| format!("Failed to rename file: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn delete_file(
    state: State<'_, AppState>,
    request: DeleteFileRequest,
) -> Result<(), String> {
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
    request: ListDirectoryFilesRequest,
) -> Result<Vec<String>, String> {
    use std::path::Path;

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
