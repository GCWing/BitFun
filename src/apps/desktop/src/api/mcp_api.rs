//! MCP API

use crate::api::app_state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

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
    let mcp_service = state
        .mcp_service
        .as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;

    mcp_service
        .server_manager()
        .initialize_all()
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn initialize_mcp_servers_non_destructive(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mcp_service = state
        .mcp_service
        .as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;

    mcp_service
        .server_manager()
        .initialize_non_destructive()
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn get_mcp_servers(state: State<'_, AppState>) -> Result<Vec<MCPServerInfo>, String> {
    let mcp_service = state
        .mcp_service
        .as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;

    let configs = mcp_service
        .config_service()
        .load_all_configs()
        .await
        .map_err(|e| e.to_string())?;

    let mut infos = Vec::new();

    for config in configs {
        let status = match mcp_service
            .server_manager()
            .get_server_status(&config.id)
            .await
        {
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
pub async fn start_mcp_server(state: State<'_, AppState>, server_id: String) -> Result<(), String> {
    let mcp_service = state
        .mcp_service
        .as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;

    mcp_service
        .server_manager()
        .start_server(&server_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn stop_mcp_server(state: State<'_, AppState>, server_id: String) -> Result<(), String> {
    let mcp_service = state
        .mcp_service
        .as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;

    mcp_service
        .server_manager()
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
    let mcp_service = state
        .mcp_service
        .as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;

    mcp_service
        .server_manager()
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
    let mcp_service = state
        .mcp_service
        .as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;

    let status = mcp_service
        .server_manager()
        .get_server_status(&server_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(format!("{:?}", status))
}

#[tauri::command]
pub async fn load_mcp_json_config(state: State<'_, AppState>) -> Result<String, String> {
    let mcp_service = state
        .mcp_service
        .as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;

    mcp_service
        .config_service()
        .load_mcp_json_config()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_mcp_json_config(
    state: State<'_, AppState>,
    json_config: String,
) -> Result<(), String> {
    let mcp_service = state
        .mcp_service
        .as_ref()
        .ok_or_else(|| "MCP service not initialized".to_string())?;

    mcp_service
        .config_service()
        .save_mcp_json_config(&json_config)
        .await
        .map_err(|e| e.to_string())
}
