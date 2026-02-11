//! Plugin management API
//!
//! Supports installing/uninstalling plugins, toggling enabled state, and importing MCP servers
//! from plugin `.mcp.json` into the user's MCP config.

use crate::api::app_state::AppState;
use bitfun_core::infrastructure::get_path_manager_arc;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginState {
    pub enabled: bool,
}

impl Default for PluginState {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub path: String,
    pub enabled: bool,
    pub has_mcp_config: bool,
    pub mcp_server_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportMcpServersResult {
    pub added: usize,
    pub skipped: usize,
    pub overwritten: usize,
}

fn plugin_state_path(plugin_dir: &std::path::Path) -> std::path::PathBuf {
    plugin_dir.join(".bitfun-plugin").join("state.json")
}

fn plugin_manifest_path(plugin_dir: &std::path::Path) -> std::path::PathBuf {
    plugin_dir.join(".claude-plugin").join("plugin.json")
}

fn plugin_mcp_path(plugin_dir: &std::path::Path) -> std::path::PathBuf {
    plugin_dir.join(".mcp.json")
}

fn validate_plugin_id(id: &str) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err("Plugin id cannot be empty".to_string());
    }
    if id.contains('/') || id.contains('\\') {
        return Err("Plugin id must not contain path separators".to_string());
    }
    Ok(())
}

async fn read_plugin_state(plugin_dir: &std::path::Path) -> PluginState {
    let path = plugin_state_path(plugin_dir);
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => serde_json::from_str::<PluginState>(&content).unwrap_or_default(),
        Err(_) => PluginState::default(),
    }
}

async fn write_plugin_state(plugin_dir: &std::path::Path, state: &PluginState) -> Result<(), String> {
    let state_path = plugin_state_path(plugin_dir);
    if let Some(parent) = state_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create plugin state directory: {}", e))?;
    }
    let content = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Failed to serialize plugin state: {}", e))?;
    tokio::fs::write(&state_path, content)
        .await
        .map_err(|e| format!("Failed to write plugin state: {}", e))?;
    Ok(())
}

async fn read_plugin_manifest(plugin_dir: &std::path::Path) -> Result<PluginManifest, String> {
    let path = plugin_manifest_path(plugin_dir);
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Failed to read plugin manifest: {}", e))?;
    serde_json::from_str::<PluginManifest>(&content)
        .map_err(|e| format!("Failed to parse plugin manifest: {}", e))
}

async fn count_mcp_servers(plugin_dir: &std::path::Path) -> (bool, usize) {
    let path = plugin_mcp_path(plugin_dir);
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(_) => return (false, 0),
    };
    let parsed = serde_json::from_str::<Value>(&content).ok();
    let count = parsed
        .as_ref()
        .and_then(|v| v.get("mcpServers"))
        .and_then(|v| v.as_object())
        .map(|o| o.len())
        .unwrap_or(0);
    (true, count)
}

async fn build_plugin_info(plugin_dir: &std::path::Path) -> Result<PluginInfo, String> {
    let manifest = read_plugin_manifest(plugin_dir).await?;
    let state = read_plugin_state(plugin_dir).await;
    let (has_mcp_config, mcp_server_count) = count_mcp_servers(plugin_dir).await;

    let id = manifest.name.clone();
    validate_plugin_id(&id)?;

    Ok(PluginInfo {
        id: id.clone(),
        name: manifest.name,
        version: manifest.version,
        description: manifest.description,
        path: plugin_dir.to_string_lossy().to_string(),
        enabled: state.enabled,
        has_mcp_config,
        mcp_server_count,
    })
}

async fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    tokio::fs::create_dir_all(dst).await?;

    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let ty = entry.file_type().await?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            Box::pin(copy_dir_all(&src_path, &dst_path)).await?;
        } else {
            tokio::fs::copy(&src_path, &dst_path).await?;
        }
    }

    Ok(())
}

fn resolve_plugin_root(extracted_root: &std::path::Path) -> Option<std::path::PathBuf> {
    let direct = extracted_root.to_path_buf();
    if plugin_manifest_path(&direct).exists() {
        return Some(direct);
    }

    // If there is exactly one top-level directory, treat it as plugin root.
    let mut dirs = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(extracted_root) {
        for entry in read_dir.flatten() {
            if let Ok(ft) = entry.file_type() {
                if ft.is_dir() {
                    dirs.push(entry.path());
                }
            }
        }
    }
    if dirs.len() == 1 && plugin_manifest_path(&dirs[0]).exists() {
        return Some(dirs.remove(0));
    }

    None
}

fn safe_join(root: &std::path::Path, relative: &std::path::Path) -> Result<std::path::PathBuf, String> {
    use std::path::Component;
    if relative.is_absolute() {
        return Err(format!(
            "Unexpected absolute path in plugin archive: {}",
            relative.display()
        ));
    }
    for c in relative.components() {
        if matches!(c, Component::ParentDir) {
            return Err(format!(
                "Unexpected parent dir component in plugin archive path: {}",
                relative.display()
            ));
        }
        if matches!(c, Component::Prefix(_)) {
            return Err(format!(
                "Unexpected prefix component in plugin archive path: {}",
                relative.display()
            ));
        }
    }
    Ok(root.join(relative))
}

async fn extract_zip_to_dir(zip_path: &std::path::Path, dest_dir: &std::path::Path) -> Result<(), String> {
    let zip_path = zip_path.to_path_buf();
    let dest_dir = dest_dir.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let file = std::fs::File::open(&zip_path)
            .map_err(|e| format!("Failed to open plugin archive: {}", e))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("Failed to read plugin archive: {}", e))?;

        std::fs::create_dir_all(&dest_dir)
            .map_err(|e| format!("Failed to create extraction directory: {}", e))?;

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("Failed to read archive entry: {}", e))?;

            let Some(name) = entry.enclosed_name() else {
                return Err(format!("Unsafe path in plugin archive at entry {}", i));
            };

            let out_path = safe_join(&dest_dir, name)?;

            if entry.name().ends_with('/') {
                std::fs::create_dir_all(&out_path)
                    .map_err(|e| format!("Failed to create directory: {}", e))?;
                continue;
            }

            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directory: {}", e))?;
            }

            let mut out_file = std::fs::File::create(&out_path)
                .map_err(|e| format!("Failed to create file: {}", e))?;
            std::io::copy(&mut entry, &mut out_file)
                .map_err(|e| format!("Failed to extract file: {}", e))?;
        }

        Ok(())
    })
    .await
    .map_err(|e| format!("Plugin extraction task failed: {}", e))?
}

#[tauri::command]
pub async fn list_plugins(_state: State<'_, AppState>) -> Result<Vec<PluginInfo>, String> {
    let pm = get_path_manager_arc();
    let plugins_dir = pm.user_plugins_dir();

    if let Err(e) = tokio::fs::create_dir_all(&plugins_dir).await {
        return Err(format!("Failed to create plugins directory: {}", e));
    }

    let mut result = Vec::new();
    let mut entries = tokio::fs::read_dir(&plugins_dir)
        .await
        .map_err(|e| format!("Failed to read plugins directory: {}", e))?;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        if !plugin_manifest_path(&path).exists() {
            continue;
        }

        match build_plugin_info(&path).await {
            Ok(info) => result.push(info),
            Err(e) => {
                warn!("Skipping invalid plugin directory: path={}, error={}", path.display(), e);
            }
        }
    }

    result.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(result)
}

#[tauri::command]
pub async fn install_plugin(
    _state: State<'_, AppState>,
    source_path: String,
) -> Result<PluginInfo, String> {
    use std::path::Path;

    let pm = get_path_manager_arc();
    let plugins_dir = pm.user_plugins_dir();
    tokio::fs::create_dir_all(&plugins_dir)
        .await
        .map_err(|e| format!("Failed to create plugins directory: {}", e))?;

    let source = Path::new(&source_path);
    if !source.exists() {
        return Err("Source path does not exist".to_string());
    }

    let temp_root = pm.temp_dir().join(format!("plugin_install_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_root)
        .await
        .map_err(|e| format!("Failed to create temp directory: {}", e))?;

    let plugin_root: std::path::PathBuf;

    if source.is_file() {
        extract_zip_to_dir(source, &temp_root).await?;
        plugin_root = resolve_plugin_root(&temp_root)
            .ok_or_else(|| "Plugin archive does not contain a valid .claude-plugin/plugin.json".to_string())?;
    } else if source.is_dir() {
        if !plugin_manifest_path(source).exists() {
            return Err("Plugin folder is missing .claude-plugin/plugin.json".to_string());
        }
        plugin_root = source.to_path_buf();
    } else {
        return Err("Source path is neither file nor directory".to_string());
    }

    let manifest = read_plugin_manifest(&plugin_root).await?;
    validate_plugin_id(&manifest.name)?;

    let dest_dir = plugins_dir.join(&manifest.name);
    if dest_dir.exists() {
        return Err(format!("Plugin '{}' is already installed", manifest.name));
    }

    if source.is_dir() {
        copy_dir_all(&plugin_root, &dest_dir)
            .await
            .map_err(|e| format!("Failed to copy plugin folder: {}", e))?;
    } else {
        copy_dir_all(&plugin_root, &dest_dir)
            .await
            .map_err(|e| format!("Failed to install plugin from archive: {}", e))?;
    }

    // Ensure default state exists (enabled=true).
    let state = PluginState::default();
    if let Err(e) = write_plugin_state(&dest_dir, &state).await {
        warn!("Failed to write plugin state, continuing: {}", e);
    }

    // Cleanup temp extraction directory if used.
    if source.is_file() {
        if let Err(e) = tokio::fs::remove_dir_all(&temp_root).await {
            debug!("Failed to remove temp plugin dir: path={}, error={}", temp_root.display(), e);
        }
    }

    info!("Plugin installed: id={}, path={}", manifest.name, dest_dir.display());
    build_plugin_info(&dest_dir).await
}

#[tauri::command]
pub async fn uninstall_plugin(
    _state: State<'_, AppState>,
    plugin_id: String,
) -> Result<String, String> {
    validate_plugin_id(&plugin_id)?;

    let pm = get_path_manager_arc();
    let plugin_dir = pm.user_plugins_dir().join(&plugin_id);
    if !plugin_dir.exists() {
        return Err(format!("Plugin '{}' not found", plugin_id));
    }

    tokio::fs::remove_dir_all(&plugin_dir)
        .await
        .map_err(|e| format!("Failed to uninstall plugin: {}", e))?;

    info!("Plugin uninstalled: id={}", plugin_id);
    Ok(format!("Plugin '{}' uninstalled", plugin_id))
}

#[tauri::command]
pub async fn set_plugin_enabled(
    _state: State<'_, AppState>,
    plugin_id: String,
    enabled: bool,
) -> Result<String, String> {
    validate_plugin_id(&plugin_id)?;

    let pm = get_path_manager_arc();
    let plugin_dir = pm.user_plugins_dir().join(&plugin_id);
    if !plugin_dir.exists() {
        return Err(format!("Plugin '{}' not found", plugin_id));
    }
    if !plugin_manifest_path(&plugin_dir).exists() {
        return Err(format!("Plugin '{}' is missing manifest", plugin_id));
    }

    let state = PluginState { enabled };
    write_plugin_state(&plugin_dir, &state).await?;

    info!("Plugin state updated: id={}, enabled={}", plugin_id, enabled);
    Ok(format!(
        "Plugin '{}' {}",
        plugin_id,
        if enabled { "enabled" } else { "disabled" }
    ))
}

#[tauri::command]
pub async fn import_plugin_mcp_servers(
    state: State<'_, AppState>,
    plugin_id: String,
    overwrite_existing: bool,
) -> Result<ImportMcpServersResult, String> {
    validate_plugin_id(&plugin_id)?;

    let pm = get_path_manager_arc();
    let plugin_dir = pm.user_plugins_dir().join(&plugin_id);
    if !plugin_dir.exists() {
        return Err(format!("Plugin '{}' not found", plugin_id));
    }

    let mcp_path = plugin_mcp_path(&plugin_dir);
    if !mcp_path.exists() {
        return Err("Plugin does not provide .mcp.json".to_string());
    }

    let plugin_mcp_content = tokio::fs::read_to_string(&mcp_path)
        .await
        .map_err(|e| format!("Failed to read plugin .mcp.json: {}", e))?;
    let plugin_mcp_json: Value = serde_json::from_str(&plugin_mcp_content)
        .map_err(|e| format!("Invalid plugin .mcp.json: {}", e))?;

    let plugin_servers = plugin_mcp_json
        .get("mcpServers")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "Plugin .mcp.json missing 'mcpServers' object".to_string())?;

    // Load existing user MCP config (Cursor format).
    let current_value = state
        .config_service
        .get_config::<Value>(Some("mcp_servers"))
        .await
        .unwrap_or_else(|_| serde_json::json!({ "mcpServers": {} }));

    let mut merged_root = if current_value.is_null() {
        serde_json::json!({ "mcpServers": {} })
    } else {
        current_value
    };

    if merged_root.get("mcpServers").is_none() {
        // Support array format by converting to cursor format-ish.
        if let Some(arr) = merged_root.as_array() {
            let mut map = serde_json::Map::new();
            for item in arr {
                if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                    map.insert(id.to_string(), item.clone());
                }
            }
            merged_root = serde_json::json!({ "mcpServers": map });
        } else {
            merged_root = serde_json::json!({ "mcpServers": {} });
        }
    }

    let merged_servers = merged_root
        .get_mut("mcpServers")
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| "Internal error: mcpServers is not an object".to_string())?;

    let mut added = 0usize;
    let mut skipped = 0usize;
    let mut overwritten = 0usize;

    for (server_id, server_config) in plugin_servers {
        if merged_servers.contains_key(server_id) {
            if overwrite_existing {
                merged_servers.insert(server_id.clone(), server_config.clone());
                overwritten += 1;
            } else {
                skipped += 1;
            }
        } else {
            merged_servers.insert(server_id.clone(), server_config.clone());
            added += 1;
        }
    }

    state
        .config_service
        .set_config("mcp_servers", merged_root)
        .await
        .map_err(|e| format!("Failed to save MCP config: {}", e))?;

    // Best-effort: register imported servers into the running MCP registry so they can be
    // started/restarted immediately without requiring a full initialize.
    if let Some(mcp_service) = state.mcp_service.as_ref() {
        for server_id in plugin_servers.keys() {
            if let Err(e) = mcp_service.server_manager().ensure_registered(server_id).await {
                warn!(
                    "Failed to register imported MCP server (continuing): server_id={} error={}",
                    server_id, e
                );
            }
        }
    }

    info!(
        "Imported plugin MCP servers: plugin={}, added={}, overwritten={}, skipped={}",
        plugin_id, added, overwritten, skipped
    );

    Ok(ImportMcpServersResult {
        added,
        skipped,
        overwritten,
    })
}
