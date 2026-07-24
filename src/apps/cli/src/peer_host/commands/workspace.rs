//! Workspace and config HostInvoke handlers.

use std::path::PathBuf;

use serde_json::{json, Value};

use crate::peer_host::args::{get_string, request_value};
use crate::peer_host::state::PeerHostState;
use crate::peer_host::workspace_dto::{workspace_info_to_json, workspace_list_to_json};

pub(crate) async fn initialize_workspace_startup_state(
    state: &PeerHostState,
) -> Result<Value, String> {
    let cleanup_removed_count = state
        .workspace_service
        .cleanup_invalid_workspaces()
        .await
        .map_err(|e| format!("Failed to cleanup invalid workspaces: {e}"))?;

    let current_workspace = state.workspace_service.get_current_workspace().await;
    let recent_workspaces = state.workspace_service.get_recent_workspaces().await;
    let opened_workspaces = state.workspace_service.get_opened_workspaces().await;

    Ok(json!({
        "cleanupRemovedCount": cleanup_removed_count,
        "currentWorkspace": current_workspace.as_ref().map(workspace_info_to_json),
        "recentWorkspaces": workspace_list_to_json(&recent_workspaces),
        "openedWorkspaces": workspace_list_to_json(&opened_workspaces),
        "legacyRemoteWorkspace": Value::Null,
    }))
}

pub(crate) async fn get_opened_workspaces(state: &PeerHostState) -> Result<Value, String> {
    let list = state.workspace_service.get_opened_workspaces().await;
    Ok(workspace_list_to_json(&list))
}

pub(crate) async fn get_recent_workspaces(state: &PeerHostState) -> Result<Value, String> {
    let list = state.workspace_service.get_recent_workspaces().await;
    Ok(workspace_list_to_json(&list))
}

pub(crate) async fn get_current_workspace(state: &PeerHostState) -> Result<Value, String> {
    let ws = state.workspace_service.get_current_workspace().await;
    Ok(ws
        .as_ref()
        .map(workspace_info_to_json)
        .unwrap_or(Value::Null))
}

pub(crate) async fn open_workspace(state: &PeerHostState, args: &Value) -> Result<Value, String> {
    let request = request_value(args);
    let path = get_string(request, "path")?;
    let info = state
        .workspace_service
        .open_workspace(PathBuf::from(path))
        .await
        .map_err(|e| format!("Failed to open workspace: {e}"))?;

    // Best-effort snapshot init for agent tools (mirrors server bootstrap).
    if let Err(error) = state
        .local_workspace_snapshot
        .prepare_local_workspace(info.root_path.clone())
        .await
    {
        tracing::warn!("Failed to initialize snapshot system: {}", error.message);
    }

    Ok(workspace_info_to_json(&info))
}

pub(crate) async fn reload_config() -> Result<Value, String> {
    bitfun_core::service::config::reload_global_config()
        .await
        .map_err(|e| format!("Failed to reload config: {e}"))?;
    Ok(json!("Configuration reloaded successfully"))
}

pub(crate) async fn cleanup_invalid_workspaces(state: &PeerHostState) -> Result<Value, String> {
    let removed = state
        .workspace_service
        .cleanup_invalid_workspaces()
        .await
        .map_err(|e| format!("Failed to cleanup invalid workspaces: {e}"))?;
    Ok(json!(removed))
}
