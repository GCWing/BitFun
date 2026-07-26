//! Core product-full runtime service adapters.
//!
//! This file registers existing core concrete adapters into typed runtime
//! service builders. It does not create new runtime behavior.

use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(feature = "ssh-remote")]
use bitfun_runtime_ports::{PortError, PortErrorKind, RemoteExecPort};
use bitfun_runtime_ports::{
    PortResult, RemoteProjectionPort, RemoteWorkspacePort, SessionStorePort, TerminalPort,
};
use bitfun_runtime_services::{
    RuntimeServiceMarkerPort, RuntimeServices, RuntimeServicesBuilder, RuntimeServicesProvider,
    RuntimeServicesRegistry,
};
use bitfun_services_core::local_runtime_ports::LocalRuntimePorts;
use terminal_core::TerminalRuntimePort;

use crate::agentic::session::CoreSessionStorePort;

#[cfg(feature = "service-integrations")]
use crate::service_agent_runtime::{
    CoreRemoteWorkspaceFileRuntimeHost, CoreRemoteWorkspaceRuntimeHost,
};

#[cfg(feature = "ssh-remote")]
#[derive(Debug, Clone, Copy, Default)]
struct CoreRemoteExecSshManagerProvider;

#[cfg(feature = "ssh-remote")]
#[async_trait::async_trait]
impl bitfun_services_integrations::remote_ssh::RemoteExecSshManagerProvider
    for CoreRemoteExecSshManagerProvider
{
    async fn ssh_manager(
        &self,
    ) -> PortResult<bitfun_services_integrations::remote_ssh::SSHConnectionManager> {
        let manager =
            crate::service::remote_ssh::get_remote_workspace_manager().ok_or_else(|| {
                PortError::new(
                    PortErrorKind::NotAvailable,
                    "remote workspace manager is not initialized",
                )
            })?;

        manager.get_ssh_manager().await.ok_or_else(|| {
            PortError::new(
                PortErrorKind::NotAvailable,
                "remote SSH manager is not initialized",
            )
        })
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CoreRuntimeServicesProvider;

impl CoreRuntimeServicesProvider {
    pub const fn new() -> Self {
        Self
    }

    pub fn terminal_port() -> Arc<dyn TerminalPort> {
        Arc::new(TerminalRuntimePort::default())
    }

    #[cfg(feature = "ssh-remote")]
    pub fn remote_exec_port() -> Arc<dyn RemoteExecPort> {
        Arc::new(
            bitfun_services_integrations::remote_ssh::RemoteExecRuntimePort::new(Arc::new(
                CoreRemoteExecSshManagerProvider,
            )),
        )
    }
}

impl RuntimeServicesProvider for CoreRuntimeServicesProvider {
    fn register(&self, builder: RuntimeServicesBuilder) -> RuntimeServicesBuilder {
        let session_store: Arc<dyn SessionStorePort> = Arc::new(CoreSessionStorePort::default());
        let terminal = Self::terminal_port();
        let builder = builder
            .with_session_store(session_store)
            .with_optional_terminal(Some(terminal))
            .with_optional_network(Some(RuntimeServiceMarkerPort::network_port()))
            .with_optional_git(Some(RuntimeServiceMarkerPort::git_port()))
            .with_optional_mcp_catalog(Some(RuntimeServiceMarkerPort::mcp_catalog_port()));

        #[cfg(feature = "ssh-remote")]
        let builder = builder.with_optional_remote_exec(Some(Self::remote_exec_port()));

        #[cfg(feature = "service-integrations")]
        {
            let remote_workspace: Arc<dyn RemoteWorkspacePort> =
                Arc::new(CoreRemoteWorkspaceRuntimeHost::new());
            let remote_projection: Arc<dyn RemoteProjectionPort> =
                Arc::new(CoreRemoteWorkspaceFileRuntimeHost::new());

            builder
                .with_optional_remote_workspace(Some(remote_workspace))
                .with_optional_remote_projection(Some(remote_projection))
        }

        #[cfg(not(feature = "service-integrations"))]
        {
            builder
        }
    }
}

#[derive(Clone)]
struct CoreLocalRuntimeServicesProvider {
    ports: LocalRuntimePorts,
}

impl CoreLocalRuntimeServicesProvider {
    fn new(workspace_root: impl AsRef<Path>, event_capacity: usize) -> anyhow::Result<Self> {
        Ok(Self {
            ports: LocalRuntimePorts::new(workspace_root, event_capacity)?,
        })
    }
}

impl RuntimeServicesProvider for CoreLocalRuntimeServicesProvider {
    fn register(&self, builder: RuntimeServicesBuilder) -> RuntimeServicesBuilder {
        builder
            .with_filesystem(self.ports.filesystem())
            .with_workspace(self.ports.workspace())
            .with_events(self.ports.events())
            .with_clock(self.ports.clock())
    }
}

/// Builds the shared local process service set used by sibling product hosts.
///
/// The caller remains the composition root and selects its delivery profile.
/// This function only binds the existing Core services and the required local
/// workspace, filesystem, event, and clock ports.
pub fn build_local_runtime_services(
    workspace_root: impl AsRef<Path>,
    event_capacity: usize,
) -> anyhow::Result<(PathBuf, RuntimeServices)> {
    let local = CoreLocalRuntimeServicesProvider::new(workspace_root, event_capacity)?;
    let canonical_root = local.ports.workspace_root().to_path_buf();
    let services = RuntimeServicesRegistry::new()
        .with_provider(CoreRuntimeServicesProvider::new())
        .with_provider(local)
        .build(RuntimeServicesBuilder::new())?;
    Ok((canonical_root, services))
}

#[cfg(test)]
mod local_runtime_tests {
    use super::build_local_runtime_services;
    use bitfun_runtime_ports::RuntimeServiceCapability;

    #[test]
    fn local_runtime_services_bind_required_core_and_workspace_ports() {
        let workspace = tempfile::tempdir().expect("workspace");
        let (canonical_root, services) =
            build_local_runtime_services(workspace.path(), 8).expect("local runtime services");

        assert_eq!(
            canonical_root,
            dunce::canonicalize(workspace.path()).unwrap()
        );
        for capability in [
            RuntimeServiceCapability::FileSystem,
            RuntimeServiceCapability::Workspace,
            RuntimeServiceCapability::SessionStore,
            RuntimeServiceCapability::Events,
            RuntimeServiceCapability::Clock,
            RuntimeServiceCapability::Terminal,
            RuntimeServiceCapability::Network,
            RuntimeServiceCapability::Git,
        ] {
            assert!(services.has_capability(capability), "missing {capability}");
        }
        assert!(services.clock.now_unix_millis() > 0);
    }

    #[test]
    fn local_runtime_services_reject_a_missing_workspace() {
        let temp = tempfile::tempdir().expect("tempdir");
        let error = build_local_runtime_services(temp.path().join("missing"), 8)
            .expect_err("missing workspace must fail");
        assert!(error.to_string().contains("workspace"), "{error}");
    }
}
