use std::path::PathBuf;

use bitfun_runtime_ports::{
    PortError, PortErrorKind, PortResult, RuntimeServiceCapability, RuntimeServicePort,
    SessionStorageKind, SessionStoragePathRequest, SessionStoragePathResolution, SessionStorePort,
};

use crate::agentic::core::SessionConfig;
use crate::service::remote_ssh::workspace_state::{
    resolve_workspace_session_identity, unresolved_remote_session_storage_dir,
    LOCAL_WORKSPACE_SSH_HOST,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct CoreSessionStorePort;

impl CoreSessionStorePort {
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
        let (effective_storage_path, storage_kind, remote_ssh_host) =
            if identity.hostname == LOCAL_WORKSPACE_SSH_HOST {
                (
                    PathBuf::from(identity.logical_workspace_path()),
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
                    identity.session_storage_path(),
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
