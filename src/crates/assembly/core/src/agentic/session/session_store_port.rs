use std::path::{Path, PathBuf};
use std::sync::Arc;

use bitfun_runtime_ports::{
    PortError, PortErrorKind, PortResult, RuntimeServiceCapability, RuntimeServicePort,
    SessionStorageKind, SessionStoragePathRequest, SessionStoragePathResolution, SessionStorePort,
};

use crate::agentic::core::SessionConfig;
use crate::infrastructure::{get_path_manager_arc, PathManager};
use crate::service::remote_ssh::workspace_state::{
    resolve_workspace_session_identity, unresolved_remote_session_storage_dir,
    LOCAL_WORKSPACE_SSH_HOST,
};
use crate::service::WorkspaceRuntimeService;

#[derive(Debug, Clone, Default)]
pub struct CoreSessionStorePort {
    path_manager: Option<Arc<PathManager>>,
}

impl CoreSessionStorePort {
    pub(crate) fn with_path_manager(path_manager: Arc<PathManager>) -> Self {
        Self {
            path_manager: Some(path_manager),
        }
    }

    #[cfg(test)]
    pub fn with_path_manager_for_tests(path_manager: Arc<PathManager>) -> Self {
        Self::with_path_manager(path_manager)
    }

    fn path_manager(&self) -> Arc<PathManager> {
        self.path_manager
            .clone()
            .unwrap_or_else(get_path_manager_arc)
    }

    pub async fn resolve_storage_path_for_config(
        config: &SessionConfig,
    ) -> Option<SessionStoragePathResolution> {
        let workspace_path = config.workspace_path.as_ref()?;
        let request = SessionStoragePathRequest {
            workspace_path: PathBuf::from(workspace_path),
            remote_connection_id: config.remote_connection_id.clone(),
            remote_ssh_host: config.remote_ssh_host.clone(),
        };
        Self::default()
            .resolve_session_storage_path(request)
            .await
            .ok()
    }

    fn resolved_sessions_dir_kind(
        path_manager: &PathManager,
        path: &std::path::Path,
    ) -> Option<SessionStorageKind> {
        if path.file_name().and_then(|value| value.to_str()) != Some("sessions") {
            return None;
        }

        let remote_mirror_root = path_manager.remote_ssh_mirror_root_dir();
        if path.starts_with(&remote_mirror_root) {
            return Some(
                if path
                    .components()
                    .any(|component| component.as_os_str() == std::ffi::OsStr::new("_unresolved"))
                {
                    SessionStorageKind::UnresolvedRemote
                } else {
                    SessionStorageKind::Remote
                },
            );
        }

        let projects_root = path_manager.projects_root();
        path.parent()
            .and_then(|runtime_root| runtime_root.parent())
            .is_some_and(|candidate| candidate == projects_root.as_path())
            .then_some(SessionStorageKind::Local)
    }
}

impl RuntimeServicePort for CoreSessionStorePort {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::SessionStore
    }
}

#[async_trait::async_trait]
impl SessionStorePort for CoreSessionStorePort {
    async fn resolve_session_storage_path(
        &self,
        request: SessionStoragePathRequest,
    ) -> PortResult<SessionStoragePathResolution> {
        let path_manager = self.path_manager();
        if let Some(storage_kind) =
            Self::resolved_sessions_dir_kind(&path_manager, &request.workspace_path)
        {
            return Ok(SessionStoragePathResolution::new(
                request.workspace_path.clone(),
                request.workspace_path,
                storage_kind,
                request.remote_connection_id,
                request.remote_ssh_host,
            ));
        }

        let workspace_path = request.workspace_path.to_string_lossy().to_string();
        let identity = resolve_workspace_session_identity(
            &workspace_path,
            request.remote_connection_id.as_deref(),
            request.remote_ssh_host.as_deref(),
        )
        .await
        .ok_or_else(|| {
            PortError::new(
                PortErrorKind::InvalidRequest,
                "Session workspace_path is required",
            )
        })?;

        let requested_workspace_path = request.workspace_path;
        let runtime_service = WorkspaceRuntimeService::new(path_manager);
        let (effective_storage_path, storage_kind, remote_ssh_host) =
            if identity.hostname == LOCAL_WORKSPACE_SSH_HOST {
                (
                    runtime_service
                        .context_for_local_workspace(Path::new(identity.logical_workspace_path()))
                        .sessions_dir,
                    SessionStorageKind::Local,
                    None,
                )
            } else if identity.hostname == "_unresolved" {
                (
                    unresolved_remote_session_storage_dir(
                        identity.remote_connection_id.as_deref().unwrap_or_default(),
                        identity.logical_workspace_path(),
                    ),
                    SessionStorageKind::UnresolvedRemote,
                    None,
                )
            } else {
                (
                    runtime_service
                        .context_for_remote_workspace(
                            &identity.hostname,
                            identity.logical_workspace_path(),
                        )
                        .sessions_dir,
                    SessionStorageKind::Remote,
                    Some(identity.hostname.clone()),
                )
            };

        Ok(SessionStoragePathResolution::new(
            requested_workspace_path,
            effective_storage_path,
            storage_kind,
            identity.remote_connection_id,
            remote_ssh_host,
        ))
    }
}
