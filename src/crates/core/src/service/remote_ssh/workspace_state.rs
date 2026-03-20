//! Remote Workspace Global State
//!
//! Provides a **registry** of remote SSH workspaces so that multiple remote
//! workspaces can be open simultaneously.  Each workspace is keyed by its
//! remote path and maps to the SSH connection that serves it.

use crate::service::remote_ssh::{RemoteFileService, RemoteTerminalManager, SSHConnectionManager};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A single registered remote workspace entry.
#[derive(Debug, Clone)]
pub struct RemoteWorkspaceEntry {
    pub connection_id: String,
    pub connection_name: String,
}

// ── Legacy compat alias (used by a handful of call-sites that still read
//    the old struct shape).  Will be removed once every consumer is migrated.
/// Legacy alias – prefer `RemoteWorkspaceEntry` + `lookup_connection`.
#[derive(Clone)]
pub struct RemoteWorkspaceState {
    pub is_active: bool,
    pub connection_id: Option<String>,
    pub remote_path: Option<String>,
    pub connection_name: Option<String>,
}

/// Global remote workspace state manager.
///
/// Instead of storing a **single** active workspace it now maintains a
/// `HashMap<remote_path, RemoteWorkspaceEntry>` so that several remote
/// workspaces can coexist.
pub struct RemoteWorkspaceStateManager {
    /// Key = remote_path (e.g. "/root/project"), Value = connection info.
    workspaces: Arc<RwLock<HashMap<String, RemoteWorkspaceEntry>>>,
    /// SSH connection manager (shared across all workspaces).
    ssh_manager: Arc<RwLock<Option<SSHConnectionManager>>>,
    /// Remote file service (shared).
    file_service: Arc<RwLock<Option<RemoteFileService>>>,
    /// Remote terminal manager (shared).
    terminal_manager: Arc<RwLock<Option<RemoteTerminalManager>>>,
    /// Local base path for session persistence.
    local_session_base: PathBuf,
}

impl RemoteWorkspaceStateManager {
    pub fn new() -> Self {
        let local_session_base = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("BitFun")
            .join("remote-workspaces");

        Self {
            workspaces: Arc::new(RwLock::new(HashMap::new())),
            ssh_manager: Arc::new(RwLock::new(None)),
            file_service: Arc::new(RwLock::new(None)),
            terminal_manager: Arc::new(RwLock::new(None)),
            local_session_base,
        }
    }

    // ── Service setters (shared across all workspaces) ─────────────

    pub async fn set_ssh_manager(&self, manager: SSHConnectionManager) {
        *self.ssh_manager.write().await = Some(manager);
    }

    pub async fn set_file_service(&self, service: RemoteFileService) {
        *self.file_service.write().await = Some(service);
    }

    pub async fn set_terminal_manager(&self, manager: RemoteTerminalManager) {
        *self.terminal_manager.write().await = Some(manager);
    }

    // ── Registry API ───────────────────────────────────────────────

    /// Register (or update) a remote workspace.
    pub async fn register_remote_workspace(
        &self,
        remote_path: String,
        connection_id: String,
        connection_name: String,
    ) {
        let mut guard = self.workspaces.write().await;
        guard.insert(
            remote_path,
            RemoteWorkspaceEntry {
                connection_id,
                connection_name,
            },
        );
    }

    /// Unregister a remote workspace by its path.
    pub async fn unregister_remote_workspace(&self, remote_path: &str) {
        let mut guard = self.workspaces.write().await;
        guard.remove(remote_path);
    }

    /// Look up the connection info for a given path.
    ///
    /// Returns `Some(entry)` if `path` equals a registered remote root **or**
    /// is a sub-path of one (e.g. `/root/project/src/main.rs` matches
    /// `/root/project`).
    pub async fn lookup_connection(&self, path: &str) -> Option<RemoteWorkspaceEntry> {
        let guard = self.workspaces.read().await;
        // Exact match first (most common).
        if let Some(entry) = guard.get(path) {
            return Some(entry.clone());
        }
        // Sub-path match.
        for (root, entry) in guard.iter() {
            if path.starts_with(&format!("{}/", root)) {
                return Some(entry.clone());
            }
        }
        None
    }

    /// Quick boolean check: is `path` inside any registered remote workspace?
    pub async fn is_remote_path(&self, path: &str) -> bool {
        self.lookup_connection(path).await.is_some()
    }

    /// Returns `true` if at least one remote workspace is registered.
    pub async fn has_any(&self) -> bool {
        !self.workspaces.read().await.is_empty()
    }

    // ── Legacy compat ──────────────────────────────────────────────

    /// **Compat** — old code calls `activate_remote_workspace`.  Now just
    /// delegates to `register_remote_workspace`.
    pub async fn activate_remote_workspace(
        &self,
        connection_id: String,
        remote_path: String,
        connection_name: String,
    ) {
        self.register_remote_workspace(remote_path, connection_id, connection_name)
            .await;
    }

    /// **Compat** — old code calls `deactivate_remote_workspace`.
    /// Now unregisters ALL workspaces.  Callers that need to remove a
    /// specific workspace should use `unregister_remote_workspace`.
    pub async fn deactivate_remote_workspace(&self) {
        self.workspaces.write().await.clear();
    }

    /// **Compat** — returns a snapshot shaped like the old single-workspace
    /// state.  Picks the *first* registered workspace.
    pub async fn get_state(&self) -> RemoteWorkspaceState {
        let guard = self.workspaces.read().await;
        if let Some((path, entry)) = guard.iter().next() {
            RemoteWorkspaceState {
                is_active: true,
                connection_id: Some(entry.connection_id.clone()),
                remote_path: Some(path.clone()),
                connection_name: Some(entry.connection_name.clone()),
            }
        } else {
            RemoteWorkspaceState {
                is_active: false,
                connection_id: None,
                remote_path: None,
                connection_name: None,
            }
        }
    }

    /// **Compat** — returns true if any workspace is registered.
    pub async fn is_active(&self) -> bool {
        self.has_any().await
    }

    // ── Service getters ────────────────────────────────────────────

    pub async fn get_ssh_manager(&self) -> Option<SSHConnectionManager> {
        self.ssh_manager.read().await.clone()
    }

    pub async fn get_file_service(&self) -> Option<RemoteFileService> {
        self.file_service.read().await.clone()
    }

    pub async fn get_terminal_manager(&self) -> Option<RemoteTerminalManager> {
        self.terminal_manager.read().await.clone()
    }

    // ── Session storage ────────────────────────────────────────────

    pub fn get_local_session_path(&self, connection_id: &str) -> PathBuf {
        self.local_session_base.join(connection_id).join("sessions")
    }

    /// Map a workspace path to the effective session storage path.
    /// Remote paths → local session dir.  Local paths → returned as-is.
    pub async fn get_effective_session_path(&self, workspace_path: &str) -> PathBuf {
        if let Some(entry) = self.lookup_connection(workspace_path).await {
            return self.get_local_session_path(&entry.connection_id);
        }
        PathBuf::from(workspace_path)
    }
}

// ── Global singleton ────────────────────────────────────────────────

static REMOTE_WORKSPACE_MANAGER: std::sync::OnceLock<Arc<RemoteWorkspaceStateManager>> =
    std::sync::OnceLock::new();

pub fn init_remote_workspace_manager() -> Arc<RemoteWorkspaceStateManager> {
    if let Some(existing) = REMOTE_WORKSPACE_MANAGER.get() {
        return existing.clone();
    }
    let manager = Arc::new(RemoteWorkspaceStateManager::new());
    match REMOTE_WORKSPACE_MANAGER.set(manager.clone()) {
        Ok(()) => manager,
        Err(_) => REMOTE_WORKSPACE_MANAGER.get().cloned().unwrap_or(manager),
    }
}

pub fn get_remote_workspace_manager() -> Option<Arc<RemoteWorkspaceStateManager>> {
    REMOTE_WORKSPACE_MANAGER.get().cloned()
}

// ── Free-standing helpers (convenience) ─────────────────────────────

/// Get the effective session path for a workspace.
pub async fn get_effective_session_path(workspace_path: &str) -> std::path::PathBuf {
    if let Some(manager) = get_remote_workspace_manager() {
        manager.get_effective_session_path(workspace_path).await
    } else {
        std::path::PathBuf::from(workspace_path)
    }
}

/// Check if a specific path belongs to any registered remote workspace.
pub async fn is_remote_path(path: &str) -> bool {
    if let Some(manager) = get_remote_workspace_manager() {
        manager.is_remote_path(path).await
    } else {
        false
    }
}

/// Look up the connection entry for a given path.
pub async fn lookup_remote_connection(path: &str) -> Option<RemoteWorkspaceEntry> {
    let manager = get_remote_workspace_manager()?;
    manager.lookup_connection(path).await
}

/// **Compat** — old boolean check.  Now returns true if ANY remote workspace
/// is registered.  Prefer `is_remote_path(path)` for path-specific checks.
pub async fn is_remote_workspace_active() -> bool {
    if let Some(manager) = get_remote_workspace_manager() {
        manager.has_any().await
    } else {
        false
    }
}
