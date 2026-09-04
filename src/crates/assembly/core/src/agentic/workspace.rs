use crate::agentic::core::SessionConfig;
use crate::service::workspace_runtime::WorkspaceRuntimeService;
use openbitfun_core_types::SessionExecutionTarget;
pub use openbitfun_runtime_ports::{
    WorkspaceCommandOptions, WorkspaceCommandResult, WorkspaceDirEntry, WorkspaceFileSystem,
    WorkspaceServices, WorkspaceShell,
};
pub use openbitfun_services_core::workspace::{
    local_workspace_services, LocalWorkspaceFs, LocalWorkspaceShell,
};
use openbitfun_services_core::workspace_identity::{
    workspace_session_identity, WorkspaceSessionIdentity, LOCAL_WORKSPACE_SSH_HOST,
};
#[cfg(feature = "remote-workspace")]
pub use openbitfun_services_integrations::remote_ssh::{
    remote_workspace_services, RemoteWorkspaceFs, RemoteWorkspaceShell,
};
use std::path::{Path, PathBuf};

/// Return the stable identity path used by local workspace-scoped registries.
///
/// Callers may arrive through lexical aliases (`.` / `..`) or filesystem
/// aliases. Keep normalization at the shared workspace boundary so producers
/// and consumers cannot publish and query the same workspace under different
/// keys. A missing path is retained for creation-safe discovery.
pub(crate) fn canonical_local_workspace_path(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Stable local workspace identity shared by external-source and MCP routers.
#[cfg(any(feature = "external-sources", feature = "mcp-runtime"))]
pub(crate) fn workspace_route_key(workspace_root: Option<&Path>) -> String {
    workspace_root
        .map(|path| {
            canonical_local_workspace_path(path)
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_else(|| "<global>".to_string())
}

/// Return the Session root that owns execution-scoped discovery and routing.
///
/// A managed worktree executes from `workspace_path` while Session persistence
/// remains anchored at `project_workspace_path`. Extension sources and agent
/// routes must follow the former so UI discovery and turn execution share one
/// workspace identity.
pub(crate) fn session_execution_workspace_root(config: &SessionConfig) -> Option<&Path> {
    config
        .workspace_path
        .as_deref()
        .or(config.project_workspace_path.as_deref())
        .map(Path::new)
}

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
    /// Main project root used for persistence and product-level orchestration.
    /// It equals `root_path` for legacy, local, and remote sessions.
    pub project_root_path: PathBuf,
    /// Resolved execution target persisted with the session.
    pub execution_target: Option<SessionExecutionTarget>,
    pub backend: WorkspaceBackend,
    /// Unified identity for session persistence. Local and remote workspaces
    /// share the same model; the only semantic difference is hostname.
    pub session_identity: WorkspaceSessionIdentity,
}

impl WorkspaceBinding {
    pub fn new(workspace_id: Option<String>, root_path: PathBuf) -> Self {
        let logical_workspace_path = root_path.to_string_lossy().to_string();
        let session_identity = workspace_session_identity(&logical_workspace_path, None, None)
            .unwrap_or(WorkspaceSessionIdentity {
                hostname: LOCAL_WORKSPACE_SSH_HOST.to_string(),
                logical_workspace_path,
                remote_connection_id: None,
            });
        Self {
            workspace_id,
            project_root_path: root_path.clone(),
            execution_target: None,
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
            project_root_path: root_path.clone(),
            execution_target: None,
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

    pub fn project_root_path(&self) -> &Path {
        &self.project_root_path
    }

    pub fn project_root_path_string(&self) -> String {
        self.project_root_path.to_string_lossy().to_string()
    }

    /// Binds a local execution root to the main project that owns its session
    /// data. Remote workspaces intentionally keep a single root.
    pub fn with_project_root_path(mut self, project_root_path: PathBuf) -> Self {
        if !self.is_remote() {
            self.project_root_path = project_root_path;
        }
        self
    }

    pub fn with_execution_target(
        mut self,
        execution_target: Option<SessionExecutionTarget>,
    ) -> Self {
        self.execution_target = execution_target;
        self
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
                    return openbitfun_services_core::workspace_identity::unresolved_remote_session_storage_dir(
                        crate::infrastructure::get_path_manager_arc().remote_ssh_mirror_root_dir(),
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
            .context_for_local_workspace(self.project_root_path())
            .sessions_dir
    }
}

#[cfg(test)]
mod tests {
    use super::{session_execution_workspace_root, WorkspaceBackend, WorkspaceBinding};
    use crate::agentic::core::SessionConfig;
    use crate::service::workspace_runtime::WorkspaceRuntimeService;
    use openbitfun_core_types::{
        SessionExecutionTarget, SessionExecutionTargetKind, WorktreeLifecycle,
    };
    use openbitfun_services_core::workspace_identity::{
        remote_workspace_session_mirror_dir, workspace_session_identity,
    };
    use std::path::PathBuf;

    #[test]
    fn session_execution_root_prefers_worktree_over_persistence_root() {
        let config = SessionConfig {
            workspace_path: Some("D:/worktrees/feature".to_string()),
            project_workspace_path: Some("D:/projects/main".to_string()),
            ..SessionConfig::default()
        };

        assert_eq!(
            session_execution_workspace_root(&config),
            Some(std::path::Path::new("D:/worktrees/feature"))
        );

        let legacy = SessionConfig {
            project_workspace_path: Some("D:/projects/main".to_string()),
            ..SessionConfig::default()
        };
        assert_eq!(
            session_execution_workspace_root(&legacy),
            Some(std::path::Path::new("D:/projects/main"))
        );
    }

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
            remote_workspace_session_mirror_dir(
                crate::infrastructure::get_path_manager_arc().remote_ssh_mirror_root_dir(),
                "127.0.0.1",
                "/home/wsp/projects/test"
            )
        );
    }

    #[test]
    fn worktree_binding_executes_in_worktree_but_persists_in_project() {
        let project_root = PathBuf::from("/tmp/openbitfun-project");
        let worktree_root = PathBuf::from("/tmp/openbitfun-worktrees/wt-1");
        let execution_target = SessionExecutionTarget {
            kind: SessionExecutionTargetKind::ManagedWorktree,
            worktree_id: Some("wt-1".to_string()),
            root_path: worktree_root.to_string_lossy().to_string(),
            base_ref: Some("HEAD".to_string()),
            base_commit: Some("0123456789abcdef".to_string()),
            branch: None,
            lifecycle: Some(WorktreeLifecycle::Managed),
        };
        let binding = WorkspaceBinding::new(None, worktree_root.clone())
            .with_project_root_path(project_root.clone())
            .with_execution_target(Some(execution_target.clone()));
        let runtime = WorkspaceRuntimeService::new(crate::infrastructure::get_path_manager_arc());

        assert_eq!(binding.root_path(), worktree_root);
        assert_eq!(binding.project_root_path(), project_root);
        assert_eq!(binding.execution_target, Some(execution_target));
        assert_eq!(
            binding.session_storage_dir(),
            runtime
                .context_for_local_workspace(&project_root)
                .sessions_dir
        );
    }
}

// Workspace-level I/O contracts are owned by openbitfun-runtime-ports and the
// concrete providers are re-exported from their service owner crates above.
