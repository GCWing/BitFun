//! Plugin Management API
//!
//! Tauri IPC commands for plugin discovery, status query, and trust control.

use crate::api::app_state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::State;

/// Status view of a single discovered plugin, returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginStatusView {
    pub plugin_id: String,
    pub name: String,
    pub version: Option<String>,
    pub source: String,
    pub trust_level: String,
    pub enabled: bool,
    pub skill_count: usize,
    pub diagnostics: Vec<String>,
}

/// Request body for set_plugin_trust.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPluginTrustRequest {
    pub plugin_id: String,
    pub trusted: bool,
}

/// Response for get_plugin_status / refresh_plugins.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginStatusResponse {
    pub plugins_enabled: bool,
    pub plugins: Vec<PluginStatusView>,
}

const PLUGINS_ENABLED_KEY: &str = "plugins.enabled";

/// Returns the current plugin discovery status.
#[tauri::command]
pub async fn get_plugin_status(
    state: State<'_, AppState>,
    workspace_path: Option<String>,
) -> Result<PluginStatusResponse, String> {
    let plugins_enabled = get_plugins_enabled(&state).await;
    if !plugins_enabled {
        return Ok(PluginStatusResponse {
            plugins_enabled: false,
            plugins: Vec::new(),
        });
    }

    let plugins = discover_plugins(workspace_path.as_deref().map(std::path::Path::new));
    Ok(PluginStatusResponse {
        plugins_enabled: true,
        plugins,
    })
}

/// Enables or disables the plugin system globally.
#[tauri::command]
pub async fn set_plugins_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<PluginStatusResponse, String> {
    state
        .config_service
        .set_config(PLUGINS_ENABLED_KEY, serde_json::Value::Bool(enabled))
        .await
        .map_err(|e| format!("Failed to save plugin enabled state: {e}"))?;

    Ok(PluginStatusResponse {
        plugins_enabled: enabled,
        plugins: if enabled {
            discover_plugins(None)
        } else {
            Vec::new()
        },
    })
}

/// Sets trust for a specific plugin.
#[tauri::command]
pub async fn set_plugin_trust(
    state: State<'_, AppState>,
    request: SetPluginTrustRequest,
) -> Result<PluginStatusView, String> {
    let denied_key = plugin_denied_config_key(&request.plugin_id);
    if request.trusted {
        // Clear the denied flag (restore trust).
        state
            .config_service
            .reset_config(Some(&denied_key))
            .await
            .map_err(|e| format!("Failed to update plugin trust: {e}"))?;
    } else {
        state
            .config_service
            .set_config(&denied_key, serde_json::Value::Bool(true))
            .await
            .map_err(|e| format!("Failed to save plugin trust: {e}"))?;
    }

    // Return updated view for this plugin
    let plugins = discover_plugins(None);
    plugins
        .into_iter()
        .find(|p| p.plugin_id == request.plugin_id)
        .ok_or_else(|| format!("Plugin '{}' not found", request.plugin_id))
}

/// Refreshes plugin discovery and returns updated status.
#[tauri::command]
pub async fn refresh_plugins(
    state: State<'_, AppState>,
    workspace_path: Option<String>,
) -> Result<PluginStatusResponse, String> {
    let plugins_enabled = get_plugins_enabled(&state).await;
    if !plugins_enabled {
        return Ok(PluginStatusResponse {
            plugins_enabled: false,
            plugins: Vec::new(),
        });
    }

    // Re-run discovery (sync I/O, acceptable for a user-initiated refresh).
    let ws_root: Option<PathBuf> = workspace_path.as_deref().map(PathBuf::from);
    let ws_root_ref: Option<&std::path::Path> = ws_root.as_deref();
    let plugins = discover_plugins(ws_root_ref);
    Ok(PluginStatusResponse {
        plugins_enabled: true,
        plugins,
    })
}

// ── helpers ──────────────────────────────────────────────────────────────

async fn get_plugins_enabled(state: &AppState) -> bool {
    match state.config_service.get_config::<serde_json::Value>(Some(PLUGINS_ENABLED_KEY)).await {
        Ok(value) => value.as_bool().unwrap_or(true),
        Err(_) => true, // Default to enabled when not configured
    }
}

fn plugin_denied_config_key(plugin_id: &str) -> String {
    format!("plugins.denied.{plugin_id}")
}

fn discover_plugins(workspace_root: Option<&std::path::Path>) -> Vec<PluginStatusView> {
    let discoveries = bitfun_codex_adapter::discovery::discover_all(workspace_root);
    let mut views = Vec::new();

    for d in &discoveries {
        match bitfun_codex_adapter::discovery::load_plugin_manifest(d) {
            Ok(plugin) => {
                let skill_count = plugin.skill_roots.len();
                views.push(PluginStatusView {
                    plugin_id: plugin.plugin_id.clone(),
                    name: plugin.name.clone(),
                    version: plugin.version.clone(),
                    source: plugin.root.to_string_lossy().to_string(),
                    trust_level: "Trusted".to_string(),
                    enabled: true,
                    skill_count,
                    diagnostics: Vec::new(),
                });
            }
            Err(e) => {
                views.push(PluginStatusView {
                    plugin_id: d.dir_name.clone(),
                    name: d.dir_name.clone(),
                    version: None,
                    source: d.plugin_root.to_string_lossy().to_string(),
                    trust_level: "Unknown".to_string(),
                    enabled: false,
                    skill_count: 0,
                    diagnostics: vec![format!("Manifest error: {e}")],
                });
            }
        }
    }

    views
}
