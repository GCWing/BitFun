//! Remote Workspace Global State
//!
//! Provides a **registry** of remote SSH workspaces so that multiple remote
//! workspaces can coexist. Each registration is uniquely identified by
//! **`(connection_id, remote_root_path)`** — *not* by remote path alone, so two
//! different servers opened at the same path (e.g. `/`) do not overwrite each other.

use crate::infrastructure::{get_path_manager_arc, PathManager};
use crate::service::remote_ssh::{RemoteFileService, RemoteTerminalManager, SSHConnectionManager};
use dunce::canonicalize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Unified workspace identity used to resolve session persistence for both
/// local and remote workspaces. The only semantic difference is `hostname`:
/// local workspaces use [`LOCAL_WORKSPACE_SSH_HOST`], while remote workspaces
/// use the SSH host from connection metadata.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkspaceSessionIdentity {
    pub hostname: String,
    /// Canonical local root or normalized remote root used to identify the
    /// logical workspace. This is not always the on-disk session storage path.
    pub logical_workspace_path: String,
    pub remote_connection_id: Option<String>,
}

impl WorkspaceSessionIdentity {
    pub fn is_remote(&self) -> bool {
        self.hostname != LOCAL_WORKSPACE_SSH_HOST
    }

    pub fn logical_workspace_path(&self) -> &str {
        &self.logical_workspace_path
    }

    pub fn session_storage_path(&self) -> PathBuf {
        if self.is_remote() {
            remote_workspace_session_mirror_dir(&self.hostname, &self.logical_workspace_path)
        } else {
            PathBuf::from(&self.logical_workspace_path)
        }
    }
}

/// Build a unified session identity for local or remote workspaces.
///
/// Local: `hostname=localhost`, `logical_workspace_path=canonical local root`
/// Remote: `hostname=ssh_host`, `logical_workspace_path=normalized remote root`
pub fn workspace_session_identity(
    workspace_path: &str,
    remote_connection_id: Option<&str>,
    remote_ssh_host: Option<&str>,
) -> Option<WorkspaceSessionIdentity> {
    let remote_connection_id = remote_connection_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    if let Some(connection_id) = remote_connection_id {
        let hostname = remote_ssh_host
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)?;
        return Some(WorkspaceSessionIdentity {
            hostname,
            logical_workspace_path: normalize_remote_workspace_path(workspace_path),
            remote_connection_id: Some(connection_id),
        });
    }

    let local_root =
        normalize_local_workspace_root_for_stable_id(Path::new(workspace_path)).ok()?;
    Some(WorkspaceSessionIdentity {
        hostname: LOCAL_WORKSPACE_SSH_HOST.to_string(),
        logical_workspace_path: local_root,
        remote_connection_id: None,
    })
}

/// Resolve a session identity while tolerating temporarily unresolved remote hosts.
/// If the remote host is unknown, fall back to the dedicated unresolved session tree.
pub async fn resolve_workspace_session_identity(
    workspace_path: &str,
    remote_connection_id: Option<&str>,
    remote_ssh_host: Option<&str>,
) -> Option<WorkspaceSessionIdentity> {
    let remote_connection_id = remote_connection_id
        .map(str::trim)
        .filter(|s| !s.is_empty());

    if let Some(connection_id) = remote_connection_id {
        if let Some(host) = remote_ssh_host.map(str::trim).filter(|s| !s.is_empty()) {
            return workspace_session_identity(workspace_path, Some(connection_id), Some(host));
        }

        if let Some(entry) =
            lookup_remote_connection_with_hint(workspace_path, Some(connection_id)).await
        {
            return Some(WorkspaceSessionIdentity {
                hostname: entry.ssh_host,
                logical_workspace_path: entry.remote_root,
                remote_connection_id: Some(entry.connection_id),
            });
        }

        return Some(WorkspaceSessionIdentity {
            hostname: "_unresolved".to_string(),
            logical_workspace_path: normalize_remote_workspace_path(workspace_path),
            remote_connection_id: Some(connection_id.to_string()),
        });
    }

    workspace_session_identity(workspace_path, None, None)
}
/// SSH host label for **local disk** workspaces (`Normal` / `Assistant`).
/// Remote workspaces use the SSH config host instead. Together with a normalized absolute
/// root path this forms a globally unique workspace scope: `{host}:{path}`.
pub const LOCAL_WORKSPACE_SSH_HOST: &str = "localhost";

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

/// Sanitize a single path component for the local mirror tree (host label or one path segment).
pub fn sanitize_remote_mirror_path_component(component: &str) -> String {
    let t = component.trim();
    if t.is_empty() {
        return "_".to_string();
    }
    #[cfg(windows)]
    {
        t.chars()
            .map(|c| match c {
                '<' | '>' | '"' | ':' | '/' | '\\' | '|' | '?' | '*' => '-',
                c if c.is_control() => '-',
                _ => c,
            })
            .collect()
    }
    #[cfg(not(windows))]
    {
        t.chars()
            .map(|c| if c == '/' || c == '\0' { '-' } else { c })
            .collect()
    }
}

/// SSH host / alias as a single directory name under `remote_ssh/`.
pub fn sanitize_ssh_hostname_for_mirror(host: &str) -> String {
    sanitize_remote_mirror_path_component(&host.trim().to_lowercase())
}

/// Map normalized remote workspace root to path segments under the host directory.
/// `/home/u/proj` → `home/u/proj`; `/` → `_root`.
pub fn remote_root_to_mirror_subpath(remote_root_norm: &str) -> PathBuf {
    let mut pb = PathBuf::new();
    if remote_root_norm == "/" {
        pb.push("_root");
        return pb;
    }
    for seg in remote_root_norm.trim_start_matches('/').split('/') {
        if seg.is_empty() {
            continue;
        }
        pb.push(sanitize_remote_mirror_path_component(seg));
    }
    if pb.as_os_str().is_empty() {
        pb.push("_root");
    }
    pb
}

/// Local directory where persisted sessions for this remote workspace root are stored.
pub fn remote_workspace_runtime_root(ssh_host: &str, remote_root_norm: &str) -> PathBuf {
    PathManager::remote_ssh_mirror_root()
        .join(sanitize_ssh_hostname_for_mirror(ssh_host))
        .join(remote_root_to_mirror_subpath(remote_root_norm))
}

/// Local directory where persisted sessions for this remote workspace root are stored.
pub fn remote_workspace_session_mirror_dir(ssh_host: &str, remote_root_norm: &str) -> PathBuf {
    remote_workspace_runtime_root(ssh_host, remote_root_norm).join("sessions")
}

/// Human-readable logical key: `{host}:{normalized_absolute_root}` (for logs / UI; not a directory name).
pub fn workspace_logical_key(ssh_host: &str, root_norm: &str) -> String {
    format!("{}:{}", ssh_host.trim(), root_norm)
}

fn hash_host_and_root(host: &str, root_norm: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(host.trim().to_lowercase().as_bytes());
    hasher.update(b"\n");
    hasher.update(root_norm.as_bytes());
    hex::encode(&hasher.finalize()[..16])
}

/// Stable storage id for a **local** workspace (`localhost` + canonical absolute root).
pub fn local_workspace_stable_storage_id(canonical_root_norm: &str) -> String {
    format!(
        "local_{}",
        hash_host_and_root(LOCAL_WORKSPACE_SSH_HOST, canonical_root_norm)
    )
}

/// Canonical local root [`PathBuf`] plus normalized string form (single `canonicalize` call).
pub fn canonicalize_local_workspace_root(path: &Path) -> Result<(PathBuf, String), String> {
    let pb = canonicalize(path).map_err(|e| {
        format!(
            "Failed to canonicalize local workspace path '{}': {}",
            path.display(),
            e
        )
    })?;
    let s = path_buf_to_stable_local_root_string(&pb);
    Ok((pb, s))
}

/// Canonical absolute local path as a stable UTF-8 string (forward slashes, dunce-simplified).
pub fn normalize_local_workspace_root_for_stable_id(path: &Path) -> Result<String, String> {
    Ok(canonicalize_local_workspace_root(path)?.1)
}

fn path_buf_to_stable_local_root_string(canonical: &Path) -> String {
    canonical.to_string_lossy().replace('\\', "/")
}

/// Whether two local paths refer to the same workspace root (canonical comparison when possible).
pub fn local_workspace_roots_equal(a: &Path, b: &Path) -> bool {
    match (
        normalize_local_workspace_root_for_stable_id(a),
        normalize_local_workspace_root_for_stable_id(b),
    ) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// Stable workspace id from SSH host + normalized remote root (for deduplication across reconnects).
pub fn remote_workspace_stable_id(ssh_host: &str, remote_root_norm: &str) -> String {
    format!("remote_{}", hash_host_and_root(ssh_host, remote_root_norm))
}

/// When a remote scope has `connection_id` but no resolvable SSH host, we must not read/write the
/// legacy per-connection tree (it is not the same layout as `remote_ssh/{host}/.../sessions`).
/// This returns a dedicated stub under `~/.bitfun/remote_ssh/_unresolved/.../sessions` that is
/// usually absent, so session listing is empty until host can be resolved.
pub fn unresolved_remote_session_storage_dir(
    connection_id: &str,
    workspace_path_norm: &str,
) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(b"unresolved_remote_session\x01");
    hasher.update(connection_id.trim().as_bytes());
    hasher.update(b"\0");
    hasher.update(workspace_path_norm.as_bytes());
    let key = hex::encode(&hasher.finalize()[..12]);
    PathManager::remote_ssh_mirror_root()
        .join("_unresolved")
        .join(key)
        .join("sessions")
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
    /// SSH `host` from connection config (or best-effort label for mirror paths).
    pub ssh_host: String,
    /// Normalized remote workspace root this registration applies to.
    pub remote_root: String,
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
    ssh_host: String,
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
}

impl Default for RemoteWorkspaceStateManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteWorkspaceStateManager {
    pub fn new() -> Self {
        Self {
            registrations: Arc::new(RwLock::new(Vec::new())),
            active_connection_hint: Arc::new(RwLock::new(None)),
            ssh_manager: Arc::new(RwLock::new(None)),
            file_service: Arc::new(RwLock::new(None)),
            terminal_manager: Arc::new(RwLock::new(None)),
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
        // Assistant sessions use client-local paths under ~/.bitfun/personal_assistant.
        // A registered remote root of `/` matches every absolute path; without an explicit
        // `remote_connection_id`, those paths must not be treated as SSH workspaces.
        let is_local_assistant_path =
            get_path_manager_arc().is_local_assistant_workspace_path(path);
        if is_local_assistant_path {
            let preferred_connection_id = preferred_connection_id?;
            let guard = self.registrations.read().await;
            let registration = guard
                .iter()
                .find(|r| r.connection_id == preferred_connection_id)?;
            return Some(RemoteWorkspaceEntry {
                connection_id: registration.connection_id.clone(),
                connection_name: registration.connection_name.clone(),
                ssh_host: registration.ssh_host.clone(),
                remote_root: registration.remote_root.clone(),
            });
        }

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
                ssh_host: r.ssh_host.clone(),
                remote_root: r.remote_root.clone(),
            });
        }

        if let Some(ref h) = hint {
            if let Some(r) = candidates.iter().find(|r| r.connection_id == *h) {
                return Some(RemoteWorkspaceEntry {
                    connection_id: r.connection_id.clone(),
                    connection_name: r.connection_name.clone(),
                    ssh_host: r.ssh_host.clone(),
                    remote_root: r.remote_root.clone(),
                });
            }
        }

        None
    }

    /// True if `path` could belong to **any** registered remote root (before disambiguation).
    pub async fn is_remote_path(&self, path: &str) -> bool {
        if get_path_manager_arc().is_local_assistant_workspace_path(path) {
            return false;
        }
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
        self.register_remote_workspace(remote_path, connection_id, connection_name, String::new())
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

    /// Local mirror directory for persisted sessions (`~/.bitfun/remote_ssh/.../sessions`).
    pub fn get_remote_session_mirror_path(
        &self,
        ssh_host: &str,
        remote_root_norm: &str,
    ) -> PathBuf {
        remote_workspace_session_mirror_dir(ssh_host, remote_root_norm)
    }

    /// Map a workspace path to the effective session storage path.
    /// When `remote_connection_id` is set, remote roots map to the local session mirror dir;
    /// otherwise the path is returned as-is (no path-only inference).
    pub async fn get_effective_session_path(
        &self,
        workspace_path: &str,
        remote_connection_id: Option<&str>,
        remote_ssh_host: Option<&str>,
    ) -> PathBuf {
        let remote_id = remote_connection_id
            .map(str::trim)
            .filter(|s| !s.is_empty());
        if remote_id.is_none() {
            return PathBuf::from(workspace_path);
        }
        let path_norm = normalize_remote_workspace_path(workspace_path);
        if let Some(host) = remote_ssh_host.map(str::trim).filter(|s| !s.is_empty()) {
            return remote_workspace_session_mirror_dir(host, &path_norm);
        }
        if let Some(entry) = self.lookup_connection(workspace_path, remote_id).await {
            if !entry.ssh_host.trim().is_empty() {
                return remote_workspace_session_mirror_dir(&entry.ssh_host, &entry.remote_root);
            }
            return unresolved_remote_session_storage_dir(remote_id.unwrap(), &path_norm);
        }
        unresolved_remote_session_storage_dir(remote_id.unwrap(), &path_norm)
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
    remote_ssh_host: Option<&str>,
) -> std::path::PathBuf {
    if let Some(identity) =
        resolve_workspace_session_identity(workspace_path, remote_connection_id, remote_ssh_host)
            .await
    {
        if identity.hostname == "_unresolved" {
            if let Some(connection_id) = identity.remote_connection_id.as_deref() {
                return unresolved_remote_session_storage_dir(
                    connection_id,
                    identity.logical_workspace_path(),
                );
            }
        }
        return identity.session_storage_path();
    }

    std::path::PathBuf::from(workspace_path)
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
    use super::{
        normalize_remote_workspace_path, remote_workspace_session_mirror_dir,
        sanitize_ssh_connection_id_for_local_dir, workspace_session_identity,
        LOCAL_WORKSPACE_SSH_HOST,
    };
    use crate::infrastructure::PathManager;
    use std::path::PathBuf;

    #[tokio::test]
    async fn local_assistant_path_not_remote_without_connection_id() {
        let pm = PathManager::default();
        let assistant_path = pm
            .assistant_workspace_dir("d3726520", None)
            .to_string_lossy()
            .to_string();
        let m = super::RemoteWorkspaceStateManager::new();
        m.register_remote_workspace(
            "/".to_string(),
            "conn".to_string(),
            "S".to_string(),
            "h1".to_string(),
        )
        .await;
        assert!(
            m.lookup_connection(&assistant_path, None).await.is_none(),
            "assistant workspace must not bind to SSH when remote_connection_id is omitted"
        );
        assert!(
            m.lookup_connection(&assistant_path, Some("conn"))
                .await
                .is_some(),
            "explicit remote_connection_id should still resolve for edge cases"
        );
    }

    #[tokio::test]
    async fn two_servers_same_root_both_registered() {
        let m = super::RemoteWorkspaceStateManager::new();
        m.register_remote_workspace(
            "/".to_string(),
            "conn-a".to_string(),
            "Server A".to_string(),
            "host-a".to_string(),
        )
        .await;
        m.register_remote_workspace(
            "/".to_string(),
            "conn-b".to_string(),
            "Server B".to_string(),
            "host-b".to_string(),
        )
        .await;
        m.set_active_connection_hint(Some("conn-a".to_string()))
            .await;
        let a = m.lookup_connection("/tmp", None).await.unwrap();
        assert_eq!(a.connection_id, "conn-a");
        m.set_active_connection_hint(Some("conn-b".to_string()))
            .await;
        let b = m.lookup_connection("/tmp", None).await.unwrap();
        assert_eq!(b.connection_id, "conn-b");
    }

    #[tokio::test]
    async fn preferred_connection_wins_over_hint() {
        let m = super::RemoteWorkspaceStateManager::new();
        m.register_remote_workspace(
            "/".to_string(),
            "c1".to_string(),
            "A".to_string(),
            "h1".to_string(),
        )
        .await;
        m.register_remote_workspace(
            "/".to_string(),
            "c2".to_string(),
            "B".to_string(),
            "h1".to_string(),
        )
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

    #[test]
    fn local_stable_id_is_deterministic_and_prefixed() {
        let id1 = super::local_workspace_stable_storage_id("/Users/foo/BitFun");
        let id2 = super::local_workspace_stable_storage_id("/Users/foo/BitFun");
        assert_eq!(id1, id2);
        assert!(id1.starts_with("local_"));
        assert_eq!(id1.len(), 6 + 32);
    }

    #[test]
    fn workspace_logical_key_joins_host_and_path() {
        assert_eq!(
            super::workspace_logical_key("localhost", "/Users/p/w"),
            "localhost:/Users/p/w"
        );
    }

    #[test]
    fn remote_stable_id_unchanged_shape() {
        let id = super::remote_workspace_stable_id("myhost", "/root/proj");
        assert!(id.starts_with("remote_"));
        assert_eq!(id.len(), 7 + 32);
    }

    #[test]
    fn unresolved_session_dir_is_stable_and_under_remote_ssh_mirror() {
        let a = super::unresolved_remote_session_storage_dir("conn-1", "/home/u/p");
        let b = super::unresolved_remote_session_storage_dir("conn-1", "/home/u/p");
        assert_eq!(a, b);
        let name = a.file_name().and_then(|n| n.to_str()).unwrap();
        assert_eq!(name, "sessions");
        assert!(a.to_string_lossy().contains("_unresolved"));
    }

    #[test]
    fn remote_workspace_session_identity_uses_mirror_dir_for_storage() {
        let identity = workspace_session_identity(
            "/home/wsp/projects/test",
            Some("conn-1"),
            Some("127.0.0.1"),
        )
        .expect("remote identity should resolve");

        assert_eq!(identity.hostname, "127.0.0.1");
        assert_eq!(identity.logical_workspace_path(), "/home/wsp/projects/test");
        assert_eq!(
            identity.session_storage_path(),
            remote_workspace_session_mirror_dir("127.0.0.1", "/home/wsp/projects/test")
        );
    }

    #[test]
    fn local_workspace_session_identity_uses_workspace_root_for_storage() {
        let workspace_root = std::env::temp_dir().join(format!(
            "bitfun-workspace-identity-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace_root).expect("workspace should exist");

        let identity = workspace_session_identity(&workspace_root.to_string_lossy(), None, None)
            .expect("local identity should resolve");

        assert_eq!(identity.hostname, LOCAL_WORKSPACE_SSH_HOST);
        assert_eq!(
            identity.session_storage_path(),
            PathBuf::from(identity.logical_workspace_path())
        );

        let _ = std::fs::remove_dir_all(workspace_root);
    }
}
