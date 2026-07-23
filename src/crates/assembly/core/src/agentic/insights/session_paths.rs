//! Resolve on-disk session roots for insights (local + remote SSH mirror).

use crate::agentic::session::session_store_port::CoreSessionStorePort;
use crate::service::workspace::{get_global_workspace_service, WorkspaceInfo};
use bitfun_runtime_ports::{SessionStoragePathRequest, SessionStorePort};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct EffectiveSessionStorageTarget {
    pub workspace_path: PathBuf,
    pub session_storage_path: PathBuf,
}

/// Resolve the final sessions directory for a tracked workspace.
pub async fn effective_session_storage_dir_for_workspace(ws: &WorkspaceInfo) -> PathBuf {
    let path_str = ws.root_path.to_string_lossy().to_string();
    let conn = ws.remote_ssh_connection_id().map(|s| s.to_string());
    let mut host = ws
        .metadata
        .get("sshHost")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if host.is_none() {
        if let (Some(cid), Some(ws_service)) = (conn.as_ref(), get_global_workspace_service()) {
            host = ws_service
                .remote_ssh_host_for_remote_workspace(cid.as_str(), &path_str)
                .await;
        }
    }

    CoreSessionStorePort::default()
        .resolve_session_storage_path(SessionStoragePathRequest {
            workspace_path: ws.root_path.clone(),
            remote_connection_id: conn,
            remote_ssh_host: host,
        })
        .await
        .map(|resolution| resolution.effective_storage_path)
        .unwrap_or_else(|_| PathBuf::from(path_str))
}

/// Unique workspace paths whose persisted session directories exist on disk.
///
/// Each returned path is the value to pass to [`PersistenceManager::list_sessions`].
pub async fn collect_effective_session_storage_roots() -> Vec<PathBuf> {
    collect_effective_session_storage_targets()
        .await
        .into_iter()
        .map(|target| target.session_storage_path)
        .collect()
}

pub async fn collect_effective_session_storage_targets() -> Vec<EffectiveSessionStorageTarget> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();

    let Some(ws_service) = get_global_workspace_service() else {
        return paths;
    };

    for ws in ws_service.list_workspace_infos().await {
        let sessions_dir = effective_session_storage_dir_for_workspace(&ws).await;

        if sessions_dir.exists() && seen.insert(sessions_dir.clone()) {
            paths.push(EffectiveSessionStorageTarget {
                workspace_path: ws.root_path,
                session_storage_path: sessions_dir,
            });
        }
    }

    paths
}
