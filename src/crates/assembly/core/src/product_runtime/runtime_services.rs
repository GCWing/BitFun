//! Core product-full runtime service adapters.
//!
//! This file registers existing core concrete adapters into typed runtime
//! service builders. It does not create new runtime behavior.

use std::sync::Arc;

use bitfun_runtime_ports::{
    GitPort, McpCatalogPort, NetworkPort, RemoteProjectionPort, RemoteWorkspacePort,
    RuntimeServiceCapability, RuntimeServicePort, SessionStorePort, TerminalPort,
};
use bitfun_runtime_services::{RuntimeServicesBuilder, RuntimeServicesProvider};

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
}

impl RuntimeServicesProvider for CoreRuntimeServicesProvider {
    fn register(&self, builder: RuntimeServicesBuilder) -> RuntimeServicesBuilder {
        let session_store: Arc<dyn SessionStorePort> = Arc::new(CoreSessionStorePort);
        let terminal: Arc<dyn TerminalPort> = Arc::new(CoreRuntimeServiceMarkerPort::new(
            RuntimeServiceCapability::Terminal,
        ));
        let network: Arc<dyn NetworkPort> = Arc::new(CoreRuntimeServiceMarkerPort::new(
            RuntimeServiceCapability::Network,
        ));
        let git: Arc<dyn GitPort> = Arc::new(CoreRuntimeServiceMarkerPort::new(
            RuntimeServiceCapability::Git,
        ));
        let mcp_catalog: Arc<dyn McpCatalogPort> = Arc::new(CoreRuntimeServiceMarkerPort::new(
            RuntimeServiceCapability::McpCatalog,
        ));
        let builder = builder
            .with_session_store(session_store)
            .with_optional_terminal(Some(terminal))
            .with_optional_network(Some(network))
            .with_optional_git(Some(git))
            .with_optional_mcp_catalog(Some(mcp_catalog));

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

#[derive(Debug)]
struct CoreRuntimeServiceMarkerPort {
    capability: RuntimeServiceCapability,
}

impl CoreRuntimeServiceMarkerPort {
    const fn new(capability: RuntimeServiceCapability) -> Self {
        Self { capability }
    }
}

impl RuntimeServicePort for CoreRuntimeServiceMarkerPort {
    fn capability(&self) -> RuntimeServiceCapability {
        self.capability
    }
}

impl TerminalPort for CoreRuntimeServiceMarkerPort {}
impl NetworkPort for CoreRuntimeServiceMarkerPort {}
impl GitPort for CoreRuntimeServiceMarkerPort {}
impl McpCatalogPort for CoreRuntimeServiceMarkerPort {}
