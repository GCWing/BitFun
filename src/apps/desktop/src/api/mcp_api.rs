//! MCP API

use tauri::State;
use serde::{Deserialize, Serialize};
use crate::api::app_state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPServerInfo {
    pub id: String,
    pub name: String,
    pub status: String,
    pub server_type: String,
    pub enabled: bool,
    pub auto_start: bool,
}

#[tauri::command]
pub async fn initialize_mcp_servers(state: State<'_, AppState>) -> Result<(), String> {
    let mcp_service = state.mcp_service.as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;
    
    mcp_service.server_manager()
        .initialize_all()
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
pub async fn get_mcp_servers(state: State<'_, AppState>) -> Result<Vec<MCPServerInfo>, String> {
    let mcp_service = state.mcp_service.as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;
    
    let configs = mcp_service.config_service()
        .load_all_configs()
        .await
        .map_err(|e| e.to_string())?;
    
    let mut infos = Vec::new();
    
    for config in configs {
        let status = match mcp_service.server_manager().get_server_status(&config.id).await {
            Ok(s) => format!("{:?}", s),
            Err(_) => {
                if !config.enabled {
                    "Stopped".to_string()
                } else if config.auto_start {
                    "Starting".to_string()
                } else {
                    "Uninitialized".to_string()
                }
            }
        };
        
        infos.push(MCPServerInfo {
            id: config.id.clone(),
            name: config.name.clone(),
            status,
            server_type: format!("{:?}", config.server_type),
            enabled: config.enabled,
            auto_start: config.auto_start,
        });
    }
    
    Ok(infos)
}

#[tauri::command]
pub async fn start_mcp_server(
    state: State<'_, AppState>,
    server_id: String,
) -> Result<(), String> {
    let mcp_service = state.mcp_service.as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;
    
    mcp_service.server_manager()
        .start_server(&server_id)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
pub async fn stop_mcp_server(
    state: State<'_, AppState>,
    server_id: String,
) -> Result<(), String> {
    let mcp_service = state.mcp_service.as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;
    
    mcp_service.server_manager()
        .stop_server(&server_id)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
pub async fn restart_mcp_server(
    state: State<'_, AppState>,
    server_id: String,
) -> Result<(), String> {
    let mcp_service = state.mcp_service.as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;
    
    mcp_service.server_manager()
        .restart_server(&server_id)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
pub async fn get_mcp_server_status(
    state: State<'_, AppState>,
    server_id: String,
) -> Result<String, String> {
    let mcp_service = state.mcp_service.as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;
    
    let status = mcp_service.server_manager()
        .get_server_status(&server_id)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(format!("{:?}", status))
}

#[tauri::command]
pub async fn load_mcp_json_config(state: State<'_, AppState>) -> Result<String, String> {
    let mcp_service = state.mcp_service.as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;
    
    mcp_service.config_service()
        .load_mcp_json_config()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_mcp_json_config(
    state: State<'_, AppState>,
    json_config: String,
) -> Result<(), String> {
    let mcp_service = state.mcp_service.as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;
    
    mcp_service.config_service()
        .save_mcp_json_config(&json_config)
        .await
        .map_err(|e| e.to_string())
}

/// Content Security Policy configuration for MCP App UI (aligned with VSCode/MCP Apps spec).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct McpUiResourceCsp {
    /// Origins for network requests (fetch/XHR/WebSocket).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connect_domains: Option<Vec<String>>,
    /// Origins for static resources (scripts, images, styles, fonts).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_domains: Option<Vec<String>>,
    /// Origins for nested iframes (frame-src directive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_domains: Option<Vec<String>>,
    /// Allowed base URIs for the document (base-uri directive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_uri_domains: Option<Vec<String>>,
}

/// Sandbox permissions requested by the UI resource (aligned with VSCode/MCP Apps spec).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct McpUiResourcePermissions {
    /// Request camera access.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera: Option<serde_json::Value>,
    /// Request microphone access.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub microphone: Option<serde_json::Value>,
    /// Request geolocation access.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geolocation: Option<serde_json::Value>,
    /// Request clipboard write access.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clipboard_write: Option<serde_json::Value>,
}

/// Request to fetch MCP App UI resource (ui:// scheme).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchMCPAppResourceRequest {
    /// MCP server ID (e.g. from tool name mcp_{server_id}_{tool_name})
    pub server_id: String,
    /// Full resource URI, e.g. "ui://my-server/widget"
    pub resource_uri: String,
}

/// Response containing MCP App UI resource content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchMCPAppResourceResponse {
    pub contents: Vec<MCPAppResourceContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPAppResourceContent {
    pub uri: String,
    /// Text content (for HTML, etc.). Omitted when resource has blob only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Base64-encoded binary content (MCP spec). Used for video, images, etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Content Security Policy configuration for MCP App UI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub csp: Option<McpUiResourceCsp>,
    /// Sandbox permissions requested by the UI resource.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<McpUiResourcePermissions>,
}

#[tauri::command]
pub async fn get_mcp_tool_ui_uri(
    _state: State<'_, AppState>,
    tool_name: String,
) -> Result<Option<String>, String> {
    if !tool_name.starts_with("mcp_") {
        return Ok(None);
    }
    let registry = bitfun_core::agentic::tools::registry::get_global_tool_registry();
    let guard = registry.read().await;
    let tool = guard.get_tool(&tool_name);
    drop(guard);
    Ok(tool.and_then(|t| t.ui_resource_uri()))
}

#[tauri::command]
pub async fn fetch_mcp_app_resource(
    state: State<'_, AppState>,
    request: FetchMCPAppResourceRequest,
) -> Result<FetchMCPAppResourceResponse, String> {
    let mcp_service = state.mcp_service.as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;

    if !request.resource_uri.starts_with("ui://") {
        return Err("Resource URI must use ui:// scheme".to_string());
    }

    let connection = mcp_service.server_manager()
        .get_connection(&request.server_id)
        .await
        .ok_or_else(|| format!("MCP server not connected: {}", request.server_id))?;

    let result = connection
        .read_resource(&request.resource_uri)
        .await
        .map_err(|e| e.to_string())?;

    let contents = result
        .contents
        .into_iter()
        .map(|c| {
            // Extract CSP and permissions from _meta.ui (MCP Apps spec path)
            let (csp, permissions) = c.meta
                .as_ref()
                .and_then(|meta| meta.ui.as_ref())
                .map(|ui| {
                    let csp = ui.csp.as_ref().map(|core_csp| McpUiResourceCsp {
                        connect_domains: core_csp.connect_domains.clone(),
                        resource_domains: core_csp.resource_domains.clone(),
                        frame_domains: core_csp.frame_domains.clone(),
                        base_uri_domains: core_csp.base_uri_domains.clone(),
                    });
                    let permissions = ui.permissions.as_ref().map(|core_perm| McpUiResourcePermissions {
                        camera: core_perm.camera.clone(),
                        microphone: core_perm.microphone.clone(),
                        geolocation: core_perm.geolocation.clone(),
                        clipboard_write: core_perm.clipboard_write.clone(),
                    });
                    (csp, permissions)
                })
                .unwrap_or((None, None));
            MCPAppResourceContent {
                uri: c.uri,
                content: c.content,
                blob: c.blob,
                mime_type: c.mime_type,
                csp,
                permissions,
            }
        })
        .collect();

    Ok(FetchMCPAppResourceResponse { contents })
}

/// JSON-RPC message from MCP App iframe (guest) to be forwarded to MCP server or handled by host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMCPAppMessageRequest {
    pub server_id: String,
    /// JSON-RPC 2.0 request: { "jsonrpc": "2.0", "method": "...", "params": {...}, "id": ... }
    #[serde(flatten)]
    pub message: serde_json::Value,
}

/// Response is the JSON-RPC response to send back to the iframe (result or error).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMCPAppMessageResponse {
    /// Full JSON-RPC 2.0 response object to postMessage back to iframe.
    #[serde(flatten)]
    pub response: serde_json::Value,
}

#[tauri::command]
pub async fn send_mcp_app_message(
    state: State<'_, AppState>,
    request: SendMCPAppMessageRequest,
) -> Result<SendMCPAppMessageResponse, String> {
    let mcp_service = state.mcp_service.as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;

    let connection = mcp_service.server_manager()
        .get_connection(&request.server_id)
        .await
        .ok_or_else(|| format!("MCP server not connected: {}", request.server_id))?;

    let msg = &request.message;
    let method = msg.get("method").and_then(|m| m.as_str()).ok_or_else(|| "Missing method".to_string())?;
    let id = msg.get("id").cloned();
    let params = msg.get("params").cloned().unwrap_or(serde_json::Value::Null);

    let result_value: serde_json::Value = match method {
        "tools/call" => {
            let name = params.get("name").and_then(|n| n.as_str()).ok_or_else(|| "tools/call: missing name".to_string())?;
            let arguments = params.get("arguments").cloned();
            let result = connection.call_tool(name, arguments).await.map_err(|e| e.to_string())?;
            serde_json::to_value(result).map_err(|e| e.to_string())?
        }
        "resources/read" => {
            let uri = params.get("uri").and_then(|u| u.as_str()).ok_or_else(|| "resources/read: missing uri".to_string())?;
            let result = connection.read_resource(uri).await.map_err(|e| e.to_string())?;
            serde_json::to_value(result).map_err(|e| e.to_string())?
        }
        "ping" => {
            connection.ping().await.map_err(|e| e.to_string())?;
            serde_json::json!({})
        }
        _ => {
            let code = -32601;
            let error_msg = format!("Method not found: {}", method);
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": code, "message": error_msg }
            });
            return Ok(SendMCPAppMessageResponse { response });
        }
    };

    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result_value
    });
    Ok(SendMCPAppMessageResponse { response })
}
