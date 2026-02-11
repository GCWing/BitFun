//! LSP API

use log::{error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use bitfun_core::service::lsp::types::{CompletionItem, LspPlugin};
use bitfun_core::service::lsp::{get_global_lsp_manager, initialize_global_lsp_manager};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartServerForFileRequest {
    pub file_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopServerRequest {
    pub language: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidOpenRequest {
    pub language: String,
    pub uri: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidChangeRequest {
    pub language: String,
    pub uri: String,
    pub version: i32,
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidSaveRequest {
    pub language: String,
    pub uri: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidCloseRequest {
    pub language: String,
    pub uri: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCompletionsRequest {
    pub language: String,
    pub uri: String,
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetHoverRequest {
    pub language: String,
    pub uri: String,
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GotoDefinitionRequest {
    pub language: String,
    pub uri: String,
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindReferencesRequest {
    pub language: String,
    pub uri: String,
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatDocumentRequest {
    pub language: String,
    pub uri: String,
    pub tab_size: Option<u32>,
    pub insert_spaces: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPluginRequest {
    pub package_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallPluginRequest {
    pub plugin_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPluginRequest {
    pub plugin_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetServerCapabilitiesRequest {
    pub language: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartServerResponse {
    pub success: bool,
    pub message: String,
}

#[tauri::command]
pub async fn lsp_initialize() -> Result<(), String> {
    initialize_global_lsp_manager()
        .await
        .map_err(|e| format!("Failed to initialize LSP: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn lsp_start_server_for_file(
    request: StartServerForFileRequest,
) -> Result<StartServerResponse, String> {
    let manager = get_global_lsp_manager().map_err(|e| format!("LSP not initialized: {}", e))?;

    info!("Starting LSP server for file: {}", request.file_path);

    let guard = manager.read().await;
    if let Some(plugin) = guard.find_plugin_by_file(&request.file_path).await {
        let language = &plugin.languages[0];
        match guard
            .start_server(language, None, None, None, None, None)
            .await
        {
            Ok(_) => Ok(StartServerResponse {
                success: true,
                message: format!("LSP server started for {}", request.file_path),
            }),
            Err(e) => {
                error!("Failed to start LSP server: {}", e);
                Err(format!("Failed to start LSP server: {}", e))
            }
        }
    } else {
        Err(format!(
            "No LSP plugin found for file: {}",
            request.file_path
        ))
    }
}

#[tauri::command]
pub async fn lsp_stop_server(request: StopServerRequest) -> Result<(), String> {
    let manager = get_global_lsp_manager().map_err(|e| format!("LSP not initialized: {}", e))?;

    let guard = manager.read().await;
    guard
        .stop_server(&request.language)
        .await
        .map_err(|e| format!("Failed to stop LSP server: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn lsp_stop_all_servers() -> Result<(), String> {
    let manager = get_global_lsp_manager().map_err(|e| format!("LSP not initialized: {}", e))?;

    let guard = manager.read().await;
    guard
        .stop_all_servers()
        .await
        .map_err(|e| format!("Failed to stop all LSP servers: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn lsp_did_open(request: DidOpenRequest) -> Result<(), String> {
    let manager = get_global_lsp_manager().map_err(|e| format!("LSP not initialized: {}", e))?;

    let guard = manager.read().await;
    guard
        .did_open(&request.language, &request.uri, &request.text)
        .await
        .map_err(|e| format!("Failed to send didOpen: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn lsp_did_change(request: DidChangeRequest) -> Result<(), String> {
    let manager = get_global_lsp_manager().map_err(|e| format!("LSP not initialized: {}", e))?;

    let guard = manager.read().await;
    guard
        .did_change(
            &request.language,
            &request.uri,
            request.version,
            &request.text,
        )
        .await
        .map_err(|e| format!("Failed to send didChange: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn lsp_did_save(request: DidSaveRequest) -> Result<(), String> {
    let manager = get_global_lsp_manager().map_err(|e| format!("LSP not initialized: {}", e))?;

    let guard = manager.read().await;
    guard
        .did_save(&request.language, &request.uri)
        .await
        .map_err(|e| format!("Failed to send didSave: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn lsp_did_close(request: DidCloseRequest) -> Result<(), String> {
    let manager = get_global_lsp_manager().map_err(|e| format!("LSP not initialized: {}", e))?;

    let guard = manager.read().await;
    guard
        .did_close(&request.language, &request.uri)
        .await
        .map_err(|e| format!("Failed to send didClose: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn lsp_get_completions(
    request: GetCompletionsRequest,
) -> Result<Vec<CompletionItem>, String> {
    let manager = get_global_lsp_manager().map_err(|e| format!("LSP not initialized: {}", e))?;

    let guard = manager.read().await;
    let items = guard
        .get_completions(
            &request.language,
            &request.uri,
            request.line,
            request.character,
        )
        .await
        .map_err(|e| format!("Failed to get completions: {}", e))?;

    Ok(items)
}

#[tauri::command]
pub async fn lsp_get_hover(request: GetHoverRequest) -> Result<serde_json::Value, String> {
    let manager = get_global_lsp_manager().map_err(|e| format!("LSP not initialized: {}", e))?;

    let guard = manager.read().await;
    let hover = guard
        .get_hover(
            &request.language,
            &request.uri,
            request.line,
            request.character,
        )
        .await
        .map_err(|e| format!("Failed to get hover: {}", e))?;

    Ok(hover)
}

#[tauri::command]
pub async fn lsp_goto_definition(
    request: GotoDefinitionRequest,
) -> Result<serde_json::Value, String> {
    let manager = get_global_lsp_manager().map_err(|e| format!("LSP not initialized: {}", e))?;

    let guard = manager.read().await;
    let definition = guard
        .goto_definition(
            &request.language,
            &request.uri,
            request.line,
            request.character,
        )
        .await
        .map_err(|e| format!("Failed to goto definition: {}", e))?;

    Ok(definition)
}

#[tauri::command]
pub async fn lsp_find_references(
    request: FindReferencesRequest,
) -> Result<serde_json::Value, String> {
    let manager = get_global_lsp_manager().map_err(|e| format!("LSP not initialized: {}", e))?;

    let guard = manager.read().await;
    let references = guard
        .find_references(
            &request.language,
            &request.uri,
            request.line,
            request.character,
        )
        .await
        .map_err(|e| format!("Failed to find references: {}", e))?;

    Ok(references)
}

#[tauri::command]
pub async fn lsp_format_document(
    request: FormatDocumentRequest,
) -> Result<serde_json::Value, String> {
    let manager = get_global_lsp_manager().map_err(|e| format!("LSP not initialized: {}", e))?;

    let tab_size = request.tab_size.unwrap_or(4);
    let insert_spaces = request.insert_spaces.unwrap_or(true);

    let guard = manager.read().await;
    let edits = guard
        .format_document(&request.language, &request.uri, tab_size, insert_spaces)
        .await
        .map_err(|e| format!("Failed to format document: {}", e))?;

    Ok(edits)
}

#[tauri::command]
pub async fn lsp_install_plugin(request: InstallPluginRequest) -> Result<String, String> {
    let manager = get_global_lsp_manager().map_err(|e| format!("LSP not initialized: {}", e))?;

    let package_path = PathBuf::from(request.package_path);

    let guard = manager.read().await;
    let plugin_id = guard
        .install_plugin(package_path)
        .await
        .map_err(|e| format!("Failed to install plugin: {}", e))?;

    Ok(plugin_id)
}

#[tauri::command]
pub async fn lsp_uninstall_plugin(request: UninstallPluginRequest) -> Result<(), String> {
    let manager = get_global_lsp_manager().map_err(|e| format!("LSP not initialized: {}", e))?;

    let guard = manager.read().await;
    guard
        .uninstall_plugin(&request.plugin_id)
        .await
        .map_err(|e| format!("Failed to uninstall plugin: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn lsp_list_plugins() -> Result<Vec<LspPlugin>, String> {
    let manager = get_global_lsp_manager().map_err(|e| format!("LSP not initialized: {}", e))?;

    let guard = manager.read().await;
    let plugins = guard.list_plugins().await;
    Ok(plugins)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportedExtensionsResponse {
    pub extension_to_language: HashMap<String, String>,
    pub supported_languages: Vec<String>,
}

#[tauri::command]
pub async fn lsp_get_supported_extensions() -> Result<SupportedExtensionsResponse, String> {
    use std::collections::HashMap;

    let manager = get_global_lsp_manager().map_err(|e| format!("LSP not initialized: {}", e))?;

    let guard = manager.read().await;
    let plugins = guard.list_plugins().await;

    let mut extension_to_language: HashMap<String, String> = HashMap::new();
    let mut supported_languages: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    for plugin in plugins {
        for lang in &plugin.languages {
            supported_languages.insert(lang.clone());
        }

        for ext in &plugin.file_extensions {
            if !plugin.languages.is_empty() {
                extension_to_language.insert(ext.clone(), plugin.languages[0].clone());
            }
        }
    }

    Ok(SupportedExtensionsResponse {
        extension_to_language,
        supported_languages: supported_languages.into_iter().collect(),
    })
}

#[tauri::command]
pub async fn lsp_get_plugin(request: GetPluginRequest) -> Result<Option<LspPlugin>, String> {
    let manager = get_global_lsp_manager().map_err(|e| format!("LSP not initialized: {}", e))?;

    let guard = manager.read().await;
    let plugin = guard.get_plugin(&request.plugin_id).await;
    Ok(plugin)
}

#[tauri::command]
pub async fn lsp_get_server_capabilities(
    request: GetServerCapabilitiesRequest,
) -> Result<serde_json::Value, String> {
    let manager = get_global_lsp_manager().map_err(|e| format!("LSP not initialized: {}", e))?;

    let guard = manager.read().await;
    let capabilities = guard
        .get_server_capabilities(&request.language)
        .await
        .map_err(|e| format!("Failed to get server capabilities: {}", e))?;

    Ok(capabilities)
}
