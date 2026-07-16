//! Plugin Management API
//!
//! Tauri IPC commands for plugin discovery, status query, and trust control.

use crate::api::app_state::AppState;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::State;

/// Status view of a single discovered plugin, returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginStatusView {
    pub plugin_id: String,
    pub name: String,
    pub version: Option<String>,
    pub source: String,
    /// "user" for personal plugins (~/.agents/plugins/), "workspace" for project plugins.
    pub scope: String,
    pub trust_level: String,
    pub enabled: bool,
    pub skill_count: usize,
    pub diagnostics: Vec<String>,
}

// request structs (Tauri convention: { request: { ... } })

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPluginStatusRequest {
    pub workspace_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPluginsEnabledRequest {
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPluginTrustRequest {
    pub plugin_id: String,
    pub trusted: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshPluginsRequest {
    pub workspace_path: Option<String>,
}

// response

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginStatusResponse {
    pub plugins_enabled: bool,
    pub plugins: Vec<PluginStatusView>,
    pub workspace_path: Option<String>,
}

const PLUGINS_ENABLED_KEY: &str = "plugins.enabled";

// commands

#[tauri::command]
pub async fn get_plugin_status(
    state: State<'_, AppState>,
    request: GetPluginStatusRequest,
) -> Result<PluginStatusResponse, String> {
    let plugins_enabled = get_plugins_enabled(&state).await;
    let ws_path = request.workspace_path.clone();
    if !plugins_enabled {
        return Ok(PluginStatusResponse {
            plugins_enabled: false,
            plugins: Vec::new(),
            workspace_path: ws_path,
        });
    }

    let ws_root: Option<PathBuf> = ws_path.as_deref().map(PathBuf::from);
    let plugins = tokio::task::spawn_blocking(move || discover_plugins(ws_root.as_deref()))
        .await
        .map_err(|e| format!("Plugin discovery panicked: {e}"))?;

    Ok(PluginStatusResponse {
        plugins_enabled: true,
        plugins,
        workspace_path: request.workspace_path,
    })
}

#[tauri::command]
pub async fn set_plugins_enabled(
    state: State<'_, AppState>,
    request: SetPluginsEnabledRequest,
) -> Result<PluginStatusResponse, String> {
    state
        .config_service
        .set_config(PLUGINS_ENABLED_KEY, serde_json::Value::Bool(request.enabled))
        .await
        .map_err(|e| format!("Failed to save plugin enabled state: {e}"))?;

    let plugins = if request.enabled {
        tokio::task::spawn_blocking(|| discover_plugins(None))
            .await
            .map_err(|e| format!("Plugin discovery panicked: {e}"))?
    } else {
        Vec::new()
    };

    Ok(PluginStatusResponse {
        plugins_enabled: request.enabled,
        plugins,
        workspace_path: None,
    })
}

#[tauri::command]
pub async fn set_plugin_trust(
    state: State<'_, AppState>,
    request: SetPluginTrustRequest,
) -> Result<PluginStatusView, String> {
    let denied_key = plugin_denied_config_key(&request.plugin_id);
    if request.trusted {
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

    let pid = request.plugin_id.clone();
    let plugins = tokio::task::spawn_blocking(|| discover_plugins(None))
        .await
        .map_err(|e| format!("Plugin discovery panicked: {e}"))?;
    plugins
        .into_iter()
        .find(|p| p.plugin_id == pid)
        .ok_or_else(|| format!("Plugin '{}' not found", pid))
}

#[tauri::command]
pub async fn refresh_plugins(
    state: State<'_, AppState>,
    request: RefreshPluginsRequest,
) -> Result<PluginStatusResponse, String> {
    let plugins_enabled = get_plugins_enabled(&state).await;
    let ws_path = request.workspace_path.clone();
    if !plugins_enabled {
        return Ok(PluginStatusResponse {
            plugins_enabled: false,
            plugins: Vec::new(),
            workspace_path: ws_path,
        });
    }

    let ws_root: Option<PathBuf> = ws_path.as_deref().map(PathBuf::from);
    let plugins = tokio::task::spawn_blocking(move || discover_plugins(ws_root.as_deref()))
        .await
        .map_err(|e| format!("Plugin discovery panicked: {e}"))?;

    Ok(PluginStatusResponse {
        plugins_enabled: true,
        plugins,
        workspace_path: request.workspace_path,
    })
}

// helpers

async fn get_plugins_enabled(state: &AppState) -> bool {
    match state
        .config_service
        .get_config::<serde_json::Value>(Some(PLUGINS_ENABLED_KEY))
        .await
    {
        Ok(value) => value.as_bool().unwrap_or(true),
        Err(_) => true,
    }
}

fn plugin_denied_config_key(plugin_id: &str) -> String {
    format!("plugins.denied.{plugin_id}")
}

fn discover_plugins(workspace_root: Option<&Path>) -> Vec<PluginStatusView> {
    let mut views = Vec::new();

    // user-level plugins (~/.agents/plugins/)
    let user_discoveries = bitfun_codex_adapter::discovery::discover_all(None);
    for d in &user_discoveries {
        views.push(build_view(d, "user"));
    }

    // workspace-level plugins (<ws>/.agents/plugins/)
    if let Some(ws_root) = workspace_root {
        let ws_discoveries = bitfun_codex_adapter::discovery::discover_all(Some(ws_root));
        for d in &ws_discoveries {
            if !user_discoveries.iter().any(|u| u.plugin_root == d.plugin_root) {
                views.push(build_view(d, "workspace"));
            }
        }
    }

    views
}

fn build_view(
    d: &bitfun_codex_adapter::discovery::PluginDiscovery,
    scope: &str,
) -> PluginStatusView {
    match bitfun_codex_adapter::discovery::load_plugin_manifest(d) {
        Ok(plugin) => PluginStatusView {
            plugin_id: plugin.plugin_id.clone(),
            name: plugin.name.clone(),
            version: plugin.version.clone(),
            source: plugin.root.to_string_lossy().to_string(),
            scope: scope.to_string(),
            trust_level: "Trusted".to_string(),
            enabled: true,
            skill_count: plugin.skill_roots.len(),
            diagnostics: Vec::new(),
        },
        Err(e) => PluginStatusView {
            plugin_id: d.dir_name.clone(),
            name: d.dir_name.clone(),
            version: None,
            source: d.plugin_root.to_string_lossy().to_string(),
            scope: scope.to_string(),
            trust_level: "Unknown".to_string(),
            enabled: false,
            skill_count: 0,
            diagnostics: vec![format!("Manifest error: {e}")],
        },
    }
}
