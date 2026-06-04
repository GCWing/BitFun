use std::sync::Arc;

use bitfun_runtime_ports::{
    ClockPort, FileSystemPort, GitPort, McpCatalogPort, NetworkPort, PermissionDecision,
    PermissionPort, PermissionRequest, PortResult, RemoteAssistantWorkspaceFacts,
    RemoteCapabilityPort, RemoteConnectionPort, RemoteProjectionPort, RemoteRecentWorkspaceFacts,
    RemoteWorkspaceFacts, RemoteWorkspaceFileRuntimeHost, RemoteWorkspaceKind, RemoteWorkspacePort,
    RemoteWorkspaceRuntimeHost, RemoteWorkspaceUpdate, RuntimeEventEnvelope, RuntimeEventSink,
    RuntimeServiceCapability, RuntimeServicePort, SessionStorageKind, SessionStoragePathRequest,
    SessionStoragePathResolution, SessionStorePort, TerminalPort, WorkspacePort,
};

use crate::{
    RuntimeServices, RuntimeServicesBuilder, RuntimeServicesError, RuntimeServicesProvider,
};

#[derive(Debug)]
pub struct FakeRuntimePort {
    capability: RuntimeServiceCapability,
}

impl FakeRuntimePort {
    pub fn new(capability: RuntimeServiceCapability) -> Self {
        Self { capability }
    }
}

impl RuntimeServicePort for FakeRuntimePort {
    fn capability(&self) -> RuntimeServiceCapability {
        self.capability
    }
}

impl FileSystemPort for FakeRuntimePort {}
impl WorkspacePort for FakeRuntimePort {}
#[async_trait::async_trait]
impl SessionStorePort for FakeRuntimePort {
    async fn resolve_session_storage_path(
        &self,
        request: SessionStoragePathRequest,
    ) -> PortResult<SessionStoragePathResolution> {
        Ok(SessionStoragePathResolution::new(
            request.workspace_path.clone(),
            request.workspace_path,
            SessionStorageKind::Local,
            request.remote_connection_id,
            request.remote_ssh_host,
        ))
    }
}
impl TerminalPort for FakeRuntimePort {}
impl NetworkPort for FakeRuntimePort {}
impl GitPort for FakeRuntimePort {}
impl McpCatalogPort for FakeRuntimePort {}
impl RemoteConnectionPort for FakeRuntimePort {}
impl RemoteCapabilityPort for FakeRuntimePort {}

#[async_trait::async_trait]
impl RemoteWorkspaceRuntimeHost for FakeRuntimePort {
    async fn current_workspace(&self) -> Option<RemoteWorkspaceFacts> {
        Some(RemoteWorkspaceFacts {
            path: "/remote/project".to_string(),
            name: "project".to_string(),
            git_branch: Some("main".to_string()),
            kind: RemoteWorkspaceKind::Remote,
            assistant_id: None,
        })
    }

    async fn recent_workspaces(&self) -> Vec<RemoteRecentWorkspaceFacts> {
        Vec::new()
    }

    async fn open_workspace(&self, path: &str) -> Result<RemoteWorkspaceUpdate, String> {
        Ok(RemoteWorkspaceUpdate {
            path: path.to_string(),
            name: "project".to_string(),
        })
    }

    async fn assistant_workspaces(&self) -> Vec<RemoteAssistantWorkspaceFacts> {
        Vec::new()
    }

    async fn open_assistant_workspace(&self, path: &str) -> Result<RemoteWorkspaceUpdate, String> {
        Ok(RemoteWorkspaceUpdate {
            path: path.to_string(),
            name: "assistant".to_string(),
        })
    }
}

#[async_trait::async_trait]
impl RemoteWorkspaceFileRuntimeHost for FakeRuntimePort {
    async fn resolve_remote_file_workspace_root(
        &self,
        _session_id: Option<&str>,
    ) -> Option<std::path::PathBuf> {
        Some(std::path::PathBuf::from("/remote/project"))
    }
}

#[async_trait::async_trait]
impl PermissionPort for FakeRuntimePort {
    async fn request_permission(
        &self,
        _request: PermissionRequest,
    ) -> PortResult<PermissionDecision> {
        Ok(PermissionDecision::Allow)
    }
}

impl ClockPort for FakeRuntimePort {
    fn now_unix_millis(&self) -> i64 {
        0
    }
}

#[derive(Debug, Default)]
pub struct FakeRuntimeEventSink;

#[async_trait::async_trait]
impl RuntimeEventSink for FakeRuntimeEventSink {
    async fn publish_runtime_event(&self, _event: RuntimeEventEnvelope) -> PortResult<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct FakeRuntimeServicesProvider {
    include_remote: bool,
}

impl FakeRuntimeServicesProvider {
    pub fn with_all_required() -> Self {
        Self {
            include_remote: false,
        }
    }

    pub fn with_all_remote(mut self) -> Self {
        self.include_remote = true;
        self
    }

    pub fn build_services(self) -> Result<RuntimeServices, RuntimeServicesError> {
        self.register(RuntimeServicesBuilder::new()).build()
    }
}

impl RuntimeServicesProvider for FakeRuntimeServicesProvider {
    fn register(&self, builder: RuntimeServicesBuilder) -> RuntimeServicesBuilder {
        let filesystem: Arc<dyn FileSystemPort> =
            Arc::new(FakeRuntimePort::new(RuntimeServiceCapability::FileSystem));
        let workspace: Arc<dyn WorkspacePort> =
            Arc::new(FakeRuntimePort::new(RuntimeServiceCapability::Workspace));
        let session_store: Arc<dyn SessionStorePort> =
            Arc::new(FakeRuntimePort::new(RuntimeServiceCapability::SessionStore));
        let permission: Arc<dyn PermissionPort> =
            Arc::new(FakeRuntimePort::new(RuntimeServiceCapability::Permission));
        let events: Arc<dyn RuntimeEventSink> = Arc::new(FakeRuntimeEventSink);
        let clock: Arc<dyn ClockPort> =
            Arc::new(FakeRuntimePort::new(RuntimeServiceCapability::Clock));

        let builder = builder
            .with_filesystem(filesystem)
            .with_workspace(workspace)
            .with_session_store(session_store)
            .with_permission(permission)
            .with_events(events)
            .with_clock(clock);

        if !self.include_remote {
            return builder;
        }

        let remote_connection: Arc<dyn RemoteConnectionPort> = Arc::new(FakeRuntimePort::new(
            RuntimeServiceCapability::RemoteConnection,
        ));
        let remote_workspace: Arc<dyn RemoteWorkspacePort> = Arc::new(FakeRuntimePort::new(
            RuntimeServiceCapability::RemoteWorkspace,
        ));
        let remote_projection: Arc<dyn RemoteProjectionPort> = Arc::new(FakeRuntimePort::new(
            RuntimeServiceCapability::RemoteProjection,
        ));
        let remote_capabilities: Arc<dyn RemoteCapabilityPort> = Arc::new(FakeRuntimePort::new(
            RuntimeServiceCapability::RemoteCapabilities,
        ));

        builder
            .with_optional_remote_connection(Some(remote_connection))
            .with_optional_remote_workspace(Some(remote_workspace))
            .with_optional_remote_projection(Some(remote_projection))
            .with_optional_remote_capabilities(Some(remote_capabilities))
    }
}
