//! Remote Workspace Global State
//!
//! Provides a **registry** of remote SSH workspaces so that multiple remote
//! workspaces can coexist. Each registration is uniquely identified by
//! **`(connection_id, remote_root_path)`** — *not* by remote path alone, so two
//! different servers opened at the same path (e.g. `/`) do not overwrite each other.

use crate::infrastructure::PathManager;
use crate::service::remote_ssh::{RemoteFileService, RemoteTerminalManager, SSHConnectionManager};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Normalize a remote (POSIX) workspace path for registry lookup on any client OS.
/// Converts backslashes to slashes, collapses duplicate slashes, and trims trailing slashes
/// except for the filesystem root `/`.
pub fn normalize_remote_workspace_path(path: &str) -> String {
    let mut s = path.replace('\\', "/");
    while s.contains("//") {
        s = s.replace("//", "/");
    }
    if s == "/" {
        return s;
    }
    s.trim_end_matches('/').to_string()
}

/// Characters invalid in a single Windows path component (e.g. `user@host:port` breaks on `:`).
/// On Unix, `:` is allowed in file names; we only rewrite on Windows.
pub fn sanitize_ssh_connection_id_for_local_dir(connection_id: &str) -> String {
    #[cfg(windows)]
    {
        connection_id
            .chars()
            .map(|c| match c {
                '<' | '>' | '"' | ':' | '/' | '\\' | '|' | '?' | '*' => '-',
                c if c.is_control() => '-',
                _ => c,
            })
            .collect()
    }
    #[cfg(not(windows))]
    {
        connection_id.to_string()
    }
}

fn remote_path_is_under_root(path: &str, root: &str) -> bool {
    if path == root {
        return true;
    }
    if root == "/" {
        return path.starts_with('/') && path != "/";
    }
    path.starts_with(&format!("{}/", root))
}

fn registration_matches_path(reg: &RegisteredRemoteWorkspace, path_norm: &str) -> bool {
    path_norm == reg.remote_root || remote_path_is_under_root(path_norm, &reg.remote_root)
}

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

#[derive(Debug, Clone)]
struct RegisteredRemoteWorkspace {
    connection_id: String,
    remote_root: String,
    connection_name: String,
}

/// Global remote workspace state manager.
///
/// Registrations are keyed logically by **`(connection_id, remote_root)`** so the same
/// POSIX path on different SSH hosts never collides.
pub struct RemoteWorkspaceStateManager {
    registrations: Arc<RwLock<Vec<RegisteredRemoteWorkspace>>>,
    /// Disambiguates file APIs when multiple registrations share the same remote root
    /// (e.g. two servers at `/`). Updated when the user focuses a remote workspace tab.
    active_connection_hint: Arc<RwLock<Option<String>>>,
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
        let local_session_base = PathManager::remote_ssh_sessions_root();

        Self {
            registrations: Arc::new(RwLock::new(Vec::new())),
            active_connection_hint: Arc::new(RwLock::new(None)),
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

    /// Prefer this SSH `connection_id` when resolving an ambiguous remote path.
    pub async fn set_active_connection_hint(&self, connection_id: Option<String>) {
        *self.active_connection_hint.write().await = connection_id;
    }

    // ── Registry API ───────────────────────────────────────────────

    /// Register (or replace) a remote workspace for **`(connection_id, remote_path)`**.
    pub async fn register_remote_workspace(
        &self,
        remote_path: String,
        connection_id: String,
        connection_name: String,
    ) {
        let remote_root = normalize_remote_workspace_path(&remote_path);
        let mut guard = self.registrations.write().await;
        guard.retain(|r| {
            !(r.connection_id == connection_id && r.remote_root == remote_root)
        });
        guard.push(RegisteredRemoteWorkspace {
            connection_id,
            remote_root,
            connection_name,
        });
    }

    /// Remove the registration for this **exact** SSH connection + remote root.
    pub async fn unregister_remote_workspace(&self, connection_id: &str, remote_path: &str) {
        let remote_root = normalize_remote_workspace_path(remote_path);
        let mut guard = self.registrations.write().await;
        guard.retain(|r| !(r.connection_id == connection_id && r.remote_root == remote_root));
    }

    /// Look up the connection info for a given remote path.
    ///
    /// `preferred_connection_id` should be supplied when known (e.g. from session metadata).
    /// If omitted and multiple registrations share the same longest matching root,
    /// [`Self::active_connection_hint`] is used when it matches one of them.
    pub async fn lookup_connection(
        &self,
        path: &str,
        preferred_connection_id: Option<&str>,
    ) -> Option<RemoteWorkspaceEntry> {
        let path_norm = normalize_remote_workspace_path(path);
        let hint = self.active_connection_hint.read().await.clone();
        let guard = self.registrations.read().await;

        let mut candidates: Vec<&RegisteredRemoteWorkspace> = guard
            .iter()
            .filter(|r| registration_matches_path(r, &path_norm))
            .collect();

        if let Some(pref) = preferred_connection_id {
            candidates.retain(|r| r.connection_id == pref);
        }

        let best_len = candidates.iter().map(|r| r.remote_root.len()).max()?;
        candidates.retain(|r| r.remote_root.len() == best_len);

        if candidates.is_empty() {
            return None;
        }
        if candidates.len() == 1 {
            let r = candidates[0];
            return Some(RemoteWorkspaceEntry {
                connection_id: r.connection_id.clone(),
                connection_name: r.connection_name.clone(),
            });
        }

        if let Some(ref h) = hint {
            if let Some(r) = candidates.iter().find(|r| r.connection_id == *h) {
                return Some(RemoteWorkspaceEntry {
                    connection_id: r.connection_id.clone(),
                    connection_name: r.connection_name.clone(),
                });
            }
        }

        None
    }

    /// True if `path` could belong to **any** registered remote root (before disambiguation).
    pub async fn is_remote_path(&self, path: &str) -> bool {
        let path_norm = normalize_remote_workspace_path(path);
        let guard = self.registrations.read().await;
        guard
            .iter()
            .any(|r| registration_matches_path(r, &path_norm))
    }

    /// Returns `true` if at least one remote workspace is registered.
    pub async fn has_any(&self) -> bool {
        !self.registrations.read().await.is_empty()
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
    /// Clears all registrations and the active hint (use sparingly).
    pub async fn deactivate_remote_workspace(&self) {
        self.registrations.write().await.clear();
        *self.active_connection_hint.write().await = None;
    }

    /// **Compat** — returns a snapshot shaped like the old single-workspace
    /// state.  Picks the *first* registered workspace.
    pub async fn get_state(&self) -> RemoteWorkspaceState {
        let guard = self.registrations.read().await;
        if let Some(r) = guard.first() {
            RemoteWorkspaceState {
                is_active: true,
                connection_id: Some(r.connection_id.clone()),
                remote_path: Some(r.remote_root.clone()),
                connection_name: Some(r.connection_name.clone()),
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
        let dir_name = sanitize_ssh_connection_id_for_local_dir(connection_id);
        self.local_session_base.join(dir_name).join("sessions")
    }

    /// Map a workspace path to the effective session storage path.
    /// Remote paths → local session dir.  Local paths → returned as-is.
    pub async fn get_effective_session_path(
        &self,
        workspace_path: &str,
        remote_connection_id: Option<&str>,
    ) -> PathBuf {
        if let Some(entry) = self
            .lookup_connection(workspace_path, remote_connection_id)
            .await
        {
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

/// Resolve persisted session directory for a workspace path.
pub async fn get_effective_session_path(
    workspace_path: &str,
    remote_connection_id: Option<&str>,
) -> std::path::PathBuf {
    if let Some(manager) = get_remote_workspace_manager() {
        manager
            .get_effective_session_path(workspace_path, remote_connection_id)
            .await
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

/// Look up the connection entry for a given path (optional explicit `connection_id`).
pub async fn lookup_remote_connection_with_hint(
    path: &str,
    preferred_connection_id: Option<&str>,
) -> Option<RemoteWorkspaceEntry> {
    let manager = get_remote_workspace_manager()?;
    manager
        .lookup_connection(path, preferred_connection_id)
        .await
}

/// Look up using path only (uses active hint when ambiguous).
pub async fn lookup_remote_connection(path: &str) -> Option<RemoteWorkspaceEntry> {
    lookup_remote_connection_with_hint(path, None).await
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

#[cfg(test)]
mod tests {
    use super::{normalize_remote_workspace_path, sanitize_ssh_connection_id_for_local_dir};

    #[tokio::test]
    async fn two_servers_same_root_both_registered() {
        let m = super::RemoteWorkspaceStateManager::new();
        m.register_remote_workspace(
            "/".to_string(),
            "conn-a".to_string(),
            "Server A".to_string(),
        )
        .await;
        m.register_remote_workspace(
            "/".to_string(),
            "conn-b".to_string(),
            "Server B".to_string(),
        )
        .await;
        m.set_active_connection_hint(Some("conn-a".to_string())).await;
        let a = m.lookup_connection("/tmp", None).await.unwrap();
        assert_eq!(a.connection_id, "conn-a");
        m.set_active_connection_hint(Some("conn-b".to_string())).await;
        let b = m.lookup_connection("/tmp", None).await.unwrap();
        assert_eq!(b.connection_id, "conn-b");
    }

    #[tokio::test]
    async fn preferred_connection_wins_over_hint() {
        let m = super::RemoteWorkspaceStateManager::new();
        m.register_remote_workspace("/".to_string(), "c1".to_string(), "A".to_string())
            .await;
        m.register_remote_workspace("/".to_string(), "c2".to_string(), "B".to_string())
            .await;
        m.set_active_connection_hint(Some("c1".to_string())).await;
        let x = m.lookup_connection("/x", Some("c2")).await.unwrap();
        assert_eq!(x.connection_id, "c2");
    }

    #[test]
    fn sanitize_connection_id_port_colon_on_windows_only() {
        #[cfg(windows)]
        assert_eq!(
            sanitize_ssh_connection_id_for_local_dir("ssh-root@1.95.50.146:22"),
            "ssh-root@1.95.50.146-22"
        );
        #[cfg(not(windows))]
        assert_eq!(
            sanitize_ssh_connection_id_for_local_dir("ssh-root@1.95.50.146:22"),
            "ssh-root@1.95.50.146:22"
        );
    }

    #[test]
    fn normalize_remote_collapses_slashes_and_backslashes() {
        assert_eq!(
            normalize_remote_workspace_path(r"\\home\\user\\repo//src"),
            "/home/user/repo/src"
        );
    }

    #[test]
    fn normalize_remote_root_unchanged() {
        assert_eq!(normalize_remote_workspace_path("/"), "/");
        assert_eq!(normalize_remote_workspace_path("///"), "/");
    }

    #[test]
    fn normalize_remote_trims_trailing_slash() {
        assert_eq!(
            normalize_remote_workspace_path("/home/user/repo/"),
            "/home/user/repo"
        );
    }
}
