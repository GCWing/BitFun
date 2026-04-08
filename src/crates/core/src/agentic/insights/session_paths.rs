//! Resolve on-disk session roots for insights (local + remote SSH mirror).

use crate::service::remote_ssh::workspace_state::get_effective_session_path;
use crate::service::workspace::{get_global_workspace_service, WorkspaceInfo};
use std::collections::HashSet;
use std::path::PathBuf;

/// Map a workspace record to the directory where `.bitfun/sessions` lives
/// (local project root or `~/.bitfun/remote_ssh/...` mirror).
pub async fn effective_session_storage_path_for_workspace(ws: &WorkspaceInfo) -> PathBuf {
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

/// Unique effective roots that have a `.bitfun/sessions` directory.
pub async fn collect_effective_session_storage_roots() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();

    let Some(ws_service) = get_global_workspace_service() else {
        return paths;
    };

    for ws in ws_service.list_workspace_infos().await {
        let root = effective_session_storage_path_for_workspace(&ws).await;
        let sessions_dir = root.join(".bitfun").join("sessions");
        if sessions_dir.exists() && seen.insert(root.clone()) {
            paths.push(root);
        }
    }

    paths
}
