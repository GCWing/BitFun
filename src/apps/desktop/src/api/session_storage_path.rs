//! Shared desktop resolution of on-disk session roots for remote workspaces.

use crate::api::app_state::AppState;
use bitfun_core::service::remote_ssh::workspace_state::get_effective_session_path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStorageScopeDto {
    Workspace,
    AgenticOs,
}

pub async fn desktop_effective_session_storage_path(
    app_state: &AppState,
    workspace_path: Option<&str>,
    remote_connection_id: Option<&str>,
    remote_ssh_host: Option<&str>,
    storage_scope: Option<SessionStorageScopeDto>,
) -> std::path::PathBuf {
    if matches!(storage_scope, Some(SessionStorageScopeDto::AgenticOs)) {
        return app_state.workspace_service.path_manager().agentic_os_runtime_root();
    }
    let workspace_path = workspace_path.unwrap_or_default();
    let conn = remote_connection_id
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let host_from_request = remote_ssh_host
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let mut host_owned = host_from_request.clone();
    if host_owned.is_none() {
        if let Some(cid) = conn {
            host_owned = app_state
                .workspace_service
                .remote_ssh_host_for_remote_workspace(cid, workspace_path)
                .await;
        }
    }
    if host_owned.is_none() {
        if let Some(cid) = conn {
            if let Ok(mgr) = app_state.get_ssh_manager_async().await {
                host_owned = mgr.get_saved_host_for_connection_id(cid).await;
            }
        }
    }
    get_effective_session_path(workspace_path, conn, host_owned.as_deref()).await
}
