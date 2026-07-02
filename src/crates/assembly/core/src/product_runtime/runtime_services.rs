//! Core product-full runtime service adapters.
//!
//! This file registers existing core concrete adapters into typed runtime
//! service builders. It does not create new runtime behavior.

use std::sync::Arc;

use bitfun_runtime_ports::{
    RemoteProjectionPort, RemoteWorkspacePort, SessionStorePort, TerminalPort,
};
use bitfun_runtime_services::{
    RuntimeServiceMarkerPort, RuntimeServicesBuilder, RuntimeServicesProvider,
};
use terminal_core::TerminalRuntimePort;

use crate::agentic::session::CoreSessionStorePort;

#[cfg(feature = "service-integrations")]
use crate::service_agent_runtime::{
    CoreRemoteWorkspaceFileRuntimeHost, CoreRemoteWorkspaceRuntimeHost,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct CoreRuntimeServicesProvider;

impl CoreRuntimeServicesProvider {
    pub const fn new() -> Self {
        Self
    }

    pub fn terminal_port() -> Arc<dyn TerminalPort> {
        Arc::new(TerminalRuntimePort::default())
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
