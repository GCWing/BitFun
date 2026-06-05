//! Resolve on-disk session roots for insights (local + remote SSH mirror).

use crate::infrastructure::get_path_manager_arc;
use crate::service::remote_ssh::workspace_state::get_effective_session_path;
use crate::service::workspace::{get_global_workspace_service, WorkspaceInfo};
use std::collections::HashSet;
use std::path::PathBuf;

/// Resolve the workspace path to pass to [`PersistenceManager`] for session lookups.
///
/// For local workspaces this is the workspace root path itself — the persistence layer
/// derives the actual sessions directory via [`PathManager::project_sessions_dir`].
/// For remote workspaces this is the local SSH mirror directory, which the persistence
/// layer treats as the storage root directly.
pub async fn effective_session_storage_path_for_workspace(ws: &WorkspaceInfo) -> PathBuf {
    if ws.remote_ssh_connection_id().is_none() {
        return ws.root_path.clone();
    }

    let path_str = ws.root_path.to_string_lossy().to_string();
    let conn = ws.remote_ssh_connection_id().map(|s| s.to_string());
    let mut host = ws
        .metadata
        .get("sshHost")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if host.is_none() {
        if let (Some(ref cid), Some(ws_service)) = (conn.as_ref(), get_global_workspace_service()) {
            host = ws_service
                .remote_ssh_host_for_remote_workspace(cid.as_str(), &path_str)
                .await;
        }
    }

    get_effective_session_path(&path_str, conn.as_deref(), host.as_deref()).await
}

/// Unique workspace paths whose persisted session directories exist on disk.
///
/// Each returned path is the value to pass to [`PersistenceManager::list_sessions`].
pub async fn collect_effective_session_storage_roots() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();

    let Some(ws_service) = get_global_workspace_service() else {
        return paths;
    };

    let path_manager = get_path_manager_arc();

    for ws in ws_service.list_workspace_infos().await {
        let workspace_path = effective_session_storage_path_for_workspace(&ws).await;

        // For local workspaces the actual sessions directory is derived from the
        // workspace root via the path manager. For remote workspaces the mirror
        // directory itself is the sessions root.
        let sessions_dir = if ws.remote_ssh_connection_id().is_none() {
            path_manager.project_sessions_dir(&workspace_path)
        } else {
            workspace_path.clone()
        };

        if sessions_dir.exists() && seen.insert(workspace_path.clone()) {
            paths.push(workspace_path);
        }
    }

    paths
}
