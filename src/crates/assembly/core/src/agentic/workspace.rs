use crate::service::remote_ssh::workspace_state::WorkspaceSessionIdentity;
use crate::service::workspace_runtime::WorkspaceRuntimeService;
pub use bitfun_runtime_ports::{
    WorkspaceCommandOptions, WorkspaceCommandResult, WorkspaceDirEntry, WorkspaceFileSystem,
    WorkspaceServices, WorkspaceShell,
};
pub use bitfun_services_core::workspace::{
    local_workspace_services, LocalWorkspaceFs, LocalWorkspaceShell,
};
pub use bitfun_services_integrations::remote_ssh::{
    remote_workspace_services, RemoteWorkspaceFs, RemoteWorkspaceShell,
};
use std::path::{Path, PathBuf};

/// Describes whether the workspace is local or remote via SSH.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WorkspaceBackend {
    Local,
    Remote {
        connection_id: String,
        connection_name: String,
    },
}

/// Session-bound workspace information used during agent execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkspaceBinding {
    pub workspace_id: Option<String>,
    /// For local workspaces this is a local path; for remote workspaces it is
    /// the path on the remote server (e.g. `/root/project`).
    pub root_path: PathBuf,
    pub backend: WorkspaceBackend,
    /// Unified identity for session persistence. Local and remote workspaces
    /// share the same model; the only semantic difference is hostname.
    pub session_identity: WorkspaceSessionIdentity,
}

impl WorkspaceBinding {
    pub fn new(workspace_id: Option<String>, root_path: PathBuf) -> Self {
        let logical_workspace_path = root_path.to_string_lossy().to_string();
        let session_identity =
            crate::service::remote_ssh::workspace_state::workspace_session_identity(
                &logical_workspace_path,
                None,
                None,
            )
            .unwrap_or(WorkspaceSessionIdentity {
                hostname: crate::service::remote_ssh::workspace_state::LOCAL_WORKSPACE_SSH_HOST
                    .to_string(),
                logical_workspace_path,
                remote_connection_id: None,
            });
        Self {
            workspace_id,
            root_path,
            backend: WorkspaceBackend::Local,
            session_identity,
        }
    }

    pub fn new_remote(
        workspace_id: Option<String>,
        root_path: PathBuf,
        connection_id: String,
        connection_name: String,
        session_identity: WorkspaceSessionIdentity,
    ) -> Self {
        Self {
            workspace_id,
            root_path,
            backend: WorkspaceBackend::Remote {
                connection_id,
                connection_name,
            },
            session_identity,
        }
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn root_path_string(&self) -> String {
        self.root_path.to_string_lossy().to_string()
    }

    /// Logical workspace root used by tools, display, and workspace-bound IO.
    ///
    /// For local workspaces this is the local project root. For remote SSH
    /// workspaces this is the root path on the remote host.
    pub fn logical_workspace_path(&self) -> &Path {
        &self.root_path
    }

    pub fn logical_workspace_path_string(&self) -> String {
        self.logical_workspace_path().to_string_lossy().to_string()
    }

    pub fn is_remote(&self) -> bool {
        matches!(self.backend, WorkspaceBackend::Remote { .. })
    }

    pub fn connection_id(&self) -> Option<&str> {
        match &self.backend {
            WorkspaceBackend::Remote { connection_id, .. } => Some(connection_id),
            WorkspaceBackend::Local => None,
        }
    }

    /// Final on-disk sessions directory for this workspace binding.
    pub fn session_storage_dir(&self) -> PathBuf {
        let runtime_service =
            WorkspaceRuntimeService::new(crate::infrastructure::get_path_manager_arc());
        if self.is_remote() {
            if self.session_identity.hostname == "_unresolved" {
                if let Some(connection_id) = self.session_identity.remote_connection_id.as_deref() {
                    return crate::service::remote_ssh::workspace_state::unresolved_remote_session_storage_dir(
                        connection_id,
                        self.session_identity.logical_workspace_path(),
                    );
                }
            }
            return runtime_service
                .context_for_remote_workspace(
                    &self.session_identity.hostname,
                    self.session_identity.logical_workspace_path(),
                )
                .sessions_dir;
        }

        runtime_service
            .context_for_local_workspace(self.logical_workspace_path())
            .sessions_dir
    }
}

#[cfg(test)]
mod tests {
    use super::{WorkspaceBackend, WorkspaceBinding};
    use crate::service::remote_ssh::workspace_state::{
        remote_workspace_session_mirror_dir, workspace_session_identity,
    };
    use std::path::PathBuf;

    #[test]
    fn remote_workspace_binding_uses_session_identity_storage_dir() {
        let session_identity = workspace_session_identity(
            "/home/wsp/projects/test",
            Some("conn-1"),
            Some("127.0.0.1"),
        )
        .expect("remote identity should resolve");
        let binding = WorkspaceBinding::new_remote(
            Some("workspace-1".to_string()),
            PathBuf::from("/home/wsp/projects/test"),
            "conn-1".to_string(),
            "Localhost".to_string(),
            session_identity,
        );

        assert!(matches!(binding.backend, WorkspaceBackend::Remote { .. }));
        assert_eq!(
            binding.session_storage_dir(),
            remote_workspace_session_mirror_dir("127.0.0.1", "/home/wsp/projects/test")
        );
    }
}

// Workspace-level I/O contracts are owned by bitfun-runtime-ports and the
// concrete providers are re-exported from their service owner crates above.
