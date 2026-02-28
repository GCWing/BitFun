//! Workspace-level LSP API

use log::error;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use bitfun_core::infrastructure::events::TransportEmitter;
use bitfun_core::service::lsp::types::CompletionItem;
use bitfun_core::service::lsp::{
    close_workspace, get_workspace_manager, open_workspace_with_emitter, ServerState,
};
use bitfun_transport::TauriTransportAdapter;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenWorkspaceRequest {
    pub workspace_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenDocumentRequest {
    pub workspace_path: String,
    pub uri: String,
    pub language: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeDocumentRequest {
    pub workspace_path: String,
    pub uri: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDocumentRequest {
    pub workspace_path: String,
    pub uri: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseDocumentRequest {
    pub workspace_path: String,
    pub uri: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCompletionsRequest {
    pub workspace_path: String,
    pub language: String,
    pub uri: String,
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetHoverRequest {
    pub workspace_path: String,
    pub language: String,
    pub uri: String,
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GotoDefinitionRequest {
    pub workspace_path: String,
    pub language: String,
    pub uri: String,
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindReferencesRequest {
    pub workspace_path: String,
    pub language: String,
    pub uri: String,
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCodeActionsRequest {
    pub workspace_path: String,
    pub language: String,
    pub uri: String,
    pub range: serde_json::Value,
    pub context: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatDocumentRequest {
    pub workspace_path: String,
    pub language: String,
    pub uri: String,
    pub tab_size: Option<u32>,
    pub insert_spaces: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetInlayHintsRequest {
    pub workspace_path: String,
    pub language: String,
    pub uri: String,
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameRequest {
    pub workspace_path: String,
    pub language: String,
    pub uri: String,
    pub line: u32,
    pub character: u32,
    pub new_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetDocumentHighlightRequest {
    pub workspace_path: String,
    pub language: String,
    pub uri: String,
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetServerStateRequest {
    pub workspace_path: String,
    pub language: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetDocumentSymbolsRequest {
    pub workspace_path: String,
    pub language: String,
    pub uri: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSemanticTokensRequest {
    pub workspace_path: String,
    pub language: String,
    pub uri: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSemanticTokensRangeRequest {
    pub workspace_path: String,
    pub language: String,
    pub uri: String,
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

#[tauri::command]
pub async fn lsp_open_workspace(
    request: OpenWorkspaceRequest,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let workspace_path = PathBuf::from(&request.workspace_path);

    let transport = Arc::new(TauriTransportAdapter::new(app_handle));
    let emitter: Arc<dyn bitfun_core::infrastructure::events::EventEmitter> =
        Arc::new(TransportEmitter::new(transport));

    open_workspace_with_emitter(workspace_path, Some(emitter))
        .await
        .map_err(|e| format!("Failed to open workspace: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn lsp_close_workspace(request: OpenWorkspaceRequest) -> Result<(), String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    close_workspace(workspace_path)
        .await
        .map_err(|e| format!("Failed to close workspace: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn lsp_open_document(request: OpenDocumentRequest) -> Result<(), String> {
    let workspace_path = PathBuf::from(&request.workspace_path);

    let manager = get_workspace_manager(workspace_path.clone())
        .await
        .map_err(|e| {
            let error_msg = format!("Workspace not found: {}", e);
            error!(
                "Workspace not found: workspace_path={:?}, error={}",
                workspace_path, e
            );
            error_msg
        })?;

    manager
        .open_document(
            request.uri.clone(),
            request.language.clone(),
            request.content,
        )
        .await
        .map_err(|e| {
            let error_msg = format!("Failed to open document: {}", e);
            error!(
                "Failed to open document: uri={}, language={}, error={}",
                request.uri, request.language, e
            );
            error_msg
        })?;

    Ok(())
}

#[tauri::command]
pub async fn lsp_change_document(request: ChangeDocumentRequest) -> Result<(), String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    manager
        .change_document(request.uri, request.content)
        .await
        .map_err(|e| format!("Failed to change document: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn lsp_save_document(request: SaveDocumentRequest) -> Result<(), String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    manager
        .save_document(request.uri)
        .await
        .map_err(|e| format!("Failed to save document: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn lsp_close_document(request: CloseDocumentRequest) -> Result<(), String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    manager
        .close_document(request.uri)
        .await
        .map_err(|e| format!("Failed to close document: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn lsp_get_completions_workspace(
    request: GetCompletionsRequest,
) -> Result<Vec<CompletionItem>, String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    manager
        .get_completions(
            &request.language,
            &request.uri,
            request.line,
            request.character,
        )
        .await
        .map_err(|e| format!("Failed to get completions: {}", e))
}

#[tauri::command]
pub async fn lsp_get_hover_workspace(
    request: GetHoverRequest,
) -> Result<serde_json::Value, String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    manager
        .get_hover(
            &request.language,
            &request.uri,
            request.line,
            request.character,
        )
        .await
        .map_err(|e| format!("Failed to get hover: {}", e))
}

#[tauri::command]
pub async fn lsp_goto_definition_workspace(
    request: GotoDefinitionRequest,
) -> Result<serde_json::Value, String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    manager
        .goto_definition(
            &request.language,
            &request.uri,
            request.line,
            request.character,
        )
        .await
        .map_err(|e| format!("Failed to goto definition: {}", e))
}

#[tauri::command]
pub async fn lsp_find_references_workspace(
    request: FindReferencesRequest,
) -> Result<serde_json::Value, String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    manager
        .find_references(
            &request.language,
            &request.uri,
            request.line,
            request.character,
        )
        .await
        .map_err(|e| format!("Failed to find references: {}", e))
}

#[tauri::command]
pub async fn lsp_get_code_actions_workspace(
    request: GetCodeActionsRequest,
) -> Result<serde_json::Value, String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    manager
        .get_code_actions(
            &request.language,
            &request.uri,
            request.range,
            request.context,
        )
        .await
        .map_err(|e| format!("Failed to get code actions: {}", e))
}

#[tauri::command]
pub async fn lsp_format_document_workspace(
    request: FormatDocumentRequest,
) -> Result<serde_json::Value, String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    manager
        .format_document(
            &request.language,
            &request.uri,
            request.tab_size.unwrap_or(4),
            request.insert_spaces.unwrap_or(true),
        )
        .await
        .map_err(|e| format!("Failed to format document: {}", e))
}

#[tauri::command]
pub async fn lsp_get_inlay_hints_workspace(
    request: GetInlayHintsRequest,
) -> Result<Vec<bitfun_core::service::lsp::types::InlayHint>, String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    manager
        .get_inlay_hints(
            &request.language,
            &request.uri,
            request.start_line,
            request.start_character,
            request.end_line,
            request.end_character,
        )
        .await
        .map_err(|e| format!("Failed to get inlay hints: {}", e))
}

#[tauri::command]
pub async fn lsp_rename_workspace(request: RenameRequest) -> Result<serde_json::Value, String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    manager
        .rename(
            &request.language,
            &request.uri,
            request.line,
            request.character,
            &request.new_name,
        )
        .await
        .map_err(|e| format!("Failed to rename: {}", e))
}

#[tauri::command]
pub async fn lsp_get_document_highlight_workspace(
    request: GetDocumentHighlightRequest,
) -> Result<serde_json::Value, String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    manager
        .get_document_highlight(
            &request.language,
            &request.uri,
            request.line,
            request.character,
        )
        .await
        .map_err(|e| format!("Failed to get document highlight: {}", e))
}

#[tauri::command]
pub async fn lsp_get_server_state(request: GetServerStateRequest) -> Result<ServerState, String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    let state = manager.get_server_state(&request.language).await;
    Ok(state)
}

#[tauri::command]
pub async fn lsp_get_all_server_states(
    request: OpenWorkspaceRequest,
) -> Result<HashMap<String, ServerState>, String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    let states = manager.get_all_server_states().await;
    Ok(states)
}

#[tauri::command]
pub async fn lsp_stop_server_workspace(request: GetServerStateRequest) -> Result<(), String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    manager
        .stop_server(&request.language)
        .await
        .map_err(|e| format!("Failed to stop server: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn lsp_list_workspaces() -> Result<Vec<String>, String> {
    use bitfun_core::service::lsp::get_all_workspace_paths;

    let workspaces = get_all_workspace_paths()
        .await
        .map_err(|e| format!("Failed to get workspaces: {}", e))?;

    Ok(workspaces)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectProjectRequest {
    pub workspace_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrestartServerRequest {
    pub workspace_path: String,
    pub language: String,
}

#[tauri::command]
pub async fn lsp_detect_project(
    request: DetectProjectRequest,
) -> Result<serde_json::Value, String> {
    use bitfun_core::service::lsp::project_detector::ProjectDetector;

    let workspace_path = PathBuf::from(&request.workspace_path);
    let project_info = ProjectDetector::detect(&workspace_path)
        .await
        .map_err(|e| format!("Failed to detect project: {}", e))?;

    serde_json::to_value(&project_info)
        .map_err(|e| format!("Failed to serialize project info: {}", e))
}

#[tauri::command]
pub async fn lsp_prestart_server(request: PrestartServerRequest) -> Result<(), String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    manager
        .prestart_server(&request.language)
        .await
        .map_err(|e| format!("Failed to pre-start server: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn lsp_get_document_symbols_workspace(
    request: GetDocumentSymbolsRequest,
) -> Result<serde_json::Value, String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    let symbols = manager
        .get_document_symbols(&request.language, &request.uri)
        .await
        .map_err(|e| format!("Failed to get document symbols: {}", e))?;

    Ok(symbols)
}

#[tauri::command]
pub async fn lsp_get_semantic_tokens_workspace(
    request: GetSemanticTokensRequest,
) -> Result<serde_json::Value, String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    let tokens = manager
        .get_semantic_tokens(&request.language, &request.uri)
        .await
        .map_err(|e| format!("Failed to get semantic tokens: {}", e))?;

    Ok(tokens)
}

#[tauri::command]
pub async fn lsp_get_semantic_tokens_range_workspace(
    request: GetSemanticTokensRangeRequest,
) -> Result<serde_json::Value, String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let manager = get_workspace_manager(workspace_path)
        .await
        .map_err(|e| format!("Workspace not found: {}", e))?;

    let range = serde_json::json!({
        "start": {
            "line": request.start_line,
            "character": request.start_character
        },
        "end": {
            "line": request.end_line,
            "character": request.end_character
        }
    });

    let tokens = manager
        .get_semantic_tokens_range(&request.language, &request.uri, range)
        .await
        .map_err(|e| format!("Failed to get semantic tokens range: {}", e))?;

    Ok(tokens)
}
