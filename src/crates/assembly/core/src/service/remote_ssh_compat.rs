//! Dependency-light compatibility surface for local workspace identity.
//!
//! The concrete SSH facade is compiled only by `remote-workspace`. Local Agent
//! Runtime code still shares the stable workspace/session identity helpers
//! owned by `bitfun-services-core`.

pub mod workspace_state {
    use std::path::PathBuf;

    pub use bitfun_services_core::workspace_identity::{
        canonicalize_local_workspace_root, local_workspace_roots_equal,
        local_workspace_stable_storage_id, normalize_local_workspace_root_for_stable_id,
        normalize_remote_workspace_path, remote_root_to_mirror_subpath,
        remote_workspace_session_mirror_dir as remote_workspace_session_mirror_dir_at,
        remote_workspace_stable_id, sanitize_remote_mirror_path_component,
        sanitize_ssh_connection_id_for_local_dir, sanitize_ssh_hostname_for_mirror,
        unresolved_remote_session_storage_key, workspace_logical_key, workspace_session_identity,
        WorkspaceSessionIdentity, LOCAL_WORKSPACE_SSH_HOST,
    };

    pub async fn resolve_workspace_session_identity(
        workspace_path: &str,
        remote_connection_id: Option<&str>,
        remote_ssh_host: Option<&str>,
    ) -> Option<WorkspaceSessionIdentity> {
        workspace_session_identity(workspace_path, remote_connection_id, remote_ssh_host)
    }

    pub fn remote_workspace_runtime_root(ssh_host: &str, remote_root_norm: &str) -> PathBuf {
        bitfun_services_core::workspace_identity::remote_workspace_runtime_root(
            crate::infrastructure::get_path_manager_arc().remote_ssh_mirror_root_dir(),
            ssh_host,
            remote_root_norm,
        )
    }

    pub fn remote_workspace_session_mirror_dir(ssh_host: &str, remote_root_norm: &str) -> PathBuf {
        bitfun_services_core::workspace_identity::remote_workspace_session_mirror_dir(
            crate::infrastructure::get_path_manager_arc().remote_ssh_mirror_root_dir(),
            ssh_host,
            remote_root_norm,
        )
    }

    pub fn unresolved_remote_session_storage_dir(
        connection_id: &str,
        workspace_path_norm: &str,
    ) -> PathBuf {
        bitfun_services_core::workspace_identity::unresolved_remote_session_storage_dir(
            crate::infrastructure::get_path_manager_arc().remote_ssh_mirror_root_dir(),
            connection_id,
            workspace_path_norm,
        )
    }

    /// Resolve the on-disk persisted sessions directory for a workspace path.
    /// In the dependency-light compat surface there is no SSH registry, so this
    /// falls back to the local workspace runtime layout. Kept in sync with the
    /// full `remote-workspace` implementation in `workspace_state.rs`.
    pub async fn get_effective_session_path(
        workspace_path: &str,
        remote_connection_id: Option<&str>,
        remote_ssh_host: Option<&str>,
    ) -> PathBuf {
        let runtime_service = crate::service::workspace_runtime::WorkspaceRuntimeService::new(
            crate::infrastructure::get_path_manager_arc(),
        );
        let identity = resolve_workspace_session_identity(
            workspace_path,
            remote_connection_id,
            remote_ssh_host,
        )
        .await;
        let Some(identity) = identity else {
            return runtime_service
                .context_for_local_workspace(std::path::Path::new(workspace_path))
                .sessions_dir;
        };
        if identity.hostname == "_unresolved" {
            if let Some(connection_id) = identity.remote_connection_id.as_deref() {
                return unresolved_remote_session_storage_dir(
                    connection_id,
                    identity.logical_workspace_path(),
                );
            }
        }
        if identity.hostname == LOCAL_WORKSPACE_SSH_HOST {
            return runtime_service
                .context_for_local_workspace(std::path::Path::new(
                    identity.logical_workspace_path(),
                ))
                .sessions_dir;
        }
        runtime_service
            .context_for_local_workspace(std::path::Path::new(workspace_path))
            .sessions_dir
    }

    pub async fn is_remote_path(_path: &str) -> bool {
        false
    }
}

pub use workspace_state::normalize_remote_workspace_path;
