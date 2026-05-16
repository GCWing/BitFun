//! Remote SSH workspace registry and lookup policy.
//!
//! This owns the path-to-connection registry without depending on the concrete
//! SSH runtime managers. Product assembly can wrap it with platform-specific
//! path guards and service handles.

use std::sync::Arc;

use tokio::sync::RwLock;

use super::paths::normalize_remote_workspace_path;

/// A single registered remote workspace entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteWorkspaceEntry {
    pub connection_id: String,
    pub connection_name: String,
    /// SSH `host` from connection config (or best-effort label for mirror paths).
    pub ssh_host: String,
    /// Normalized remote workspace root this registration applies to.
    pub remote_root: String,
}

/// Legacy alias – prefer `RemoteWorkspaceEntry` + `lookup_connection`.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    ssh_host: String,
}

/// Registry keyed by `(connection_id, remote_root)`.
#[derive(Debug, Clone)]
pub struct RemoteWorkspaceRegistry {
    registrations: Arc<RwLock<Vec<RegisteredRemoteWorkspace>>>,
    active_connection_hint: Arc<RwLock<Option<String>>>,
}

impl Default for RemoteWorkspaceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteWorkspaceRegistry {
    pub fn new() -> Self {
        Self {
            registrations: Arc::new(RwLock::new(Vec::new())),
            active_connection_hint: Arc::new(RwLock::new(None)),
        }
    }

    /// Prefer this SSH `connection_id` when resolving an ambiguous remote path.
    pub async fn set_active_connection_hint(&self, connection_id: Option<String>) {
        *self.active_connection_hint.write().await = connection_id;
    }

    /// Register (or replace) a remote workspace for `(connection_id, remote_path)`.
    pub async fn register_remote_workspace(
        &self,
        remote_path: String,
        connection_id: String,
        connection_name: String,
        ssh_host: String,
    ) {
        let remote_root = normalize_remote_workspace_path(&remote_path);
        let ssh_host = ssh_host.trim().to_string();
        let mut guard = self.registrations.write().await;
        guard.retain(|r| !(r.connection_id == connection_id && r.remote_root == remote_root));
        guard.push(RegisteredRemoteWorkspace {
            connection_id,
            remote_root,
            connection_name,
            ssh_host,
        });
    }

    /// Remove the registration for this exact SSH connection + remote root.
    pub async fn unregister_remote_workspace(&self, connection_id: &str, remote_path: &str) {
        let remote_root = normalize_remote_workspace_path(remote_path);
        let mut guard = self.registrations.write().await;
        guard.retain(|r| !(r.connection_id == connection_id && r.remote_root == remote_root));
    }

    /// Clears all registrations and the active hint.
    pub async fn clear(&self) {
        self.registrations.write().await.clear();
        *self.active_connection_hint.write().await = None;
    }

    /// Look up a registered workspace by connection id only.
    pub async fn lookup_by_connection_id(
        &self,
        connection_id: &str,
    ) -> Option<RemoteWorkspaceEntry> {
        let guard = self.registrations.read().await;
        guard
            .iter()
            .find(|r| r.connection_id == connection_id)
            .map(entry_from_registration)
    }

    /// Look up the connection info for a given remote path.
    ///
    /// `preferred_connection_id` should be supplied when known. If omitted and
    /// multiple registrations share the same longest matching root, the active
    /// connection hint is used when it matches one of them.
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
            return Some(entry_from_registration(candidates[0]));
        }

        if let Some(ref h) = hint {
            if let Some(r) = candidates.iter().find(|r| r.connection_id == *h) {
                return Some(entry_from_registration(r));
            }
        }

        None
    }

    /// True if `path` could belong to any registered remote root before disambiguation.
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

    /// Compat snapshot shaped like the old single-workspace state.
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
}

fn entry_from_registration(reg: &RegisteredRemoteWorkspace) -> RemoteWorkspaceEntry {
    RemoteWorkspaceEntry {
        connection_id: reg.connection_id.clone(),
        connection_name: reg.connection_name.clone(),
        ssh_host: reg.ssh_host.clone(),
        remote_root: reg.remote_root.clone(),
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
