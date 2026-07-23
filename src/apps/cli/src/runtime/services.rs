use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use bitfun_core::product_runtime::CoreRuntimeServicesProvider;
use bitfun_runtime_ports::{
    ClockPort, FileSystemPort, PortResult, RuntimeEventEnvelope, RuntimeEventSink,
    RuntimeServiceCapability, RuntimeServicePort, WorkspacePort,
};
use bitfun_runtime_services::{
    RuntimeServices, RuntimeServicesBuilder, RuntimeServicesError, RuntimeServicesProvider,
    RuntimeServicesRegistry,
};
use tokio::sync::broadcast;

#[derive(Debug)]
pub(crate) struct CliFileSystemService {
    workspace_root: PathBuf,
}

impl CliFileSystemService {
    fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }
}

impl RuntimeServicePort for CliFileSystemService {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::FileSystem
    }
}

impl FileSystemPort for CliFileSystemService {}

#[derive(Debug)]
pub(crate) struct CliWorkspaceService {
    workspace_root: PathBuf,
}

impl CliWorkspaceService {
    fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }
}

impl RuntimeServicePort for CliWorkspaceService {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::Workspace
    }
}

impl WorkspacePort for CliWorkspaceService {}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CliClock;

impl RuntimeServicePort for CliClock {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::Clock
    }
}

impl ClockPort for CliClock {
    fn now_unix_millis(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CliRuntimeEventSink {
    tx: broadcast::Sender<RuntimeEventEnvelope>,
}

impl CliRuntimeEventSink {
    pub(crate) fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity.max(1));
        Self { tx }
    }

    #[cfg(test)]
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<RuntimeEventEnvelope> {
        self.tx.subscribe()
    }
}

#[async_trait::async_trait]
impl RuntimeEventSink for CliRuntimeEventSink {
    async fn publish_runtime_event(&self, event: RuntimeEventEnvelope) -> PortResult<()> {
        let _ = self.tx.send(event);
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct CliRuntimeServicesProvider {
    workspace_root: PathBuf,
    filesystem: Arc<CliFileSystemService>,
    workspace: Arc<CliWorkspaceService>,
    events: Arc<dyn RuntimeEventSink>,
    clock: Arc<dyn ClockPort>,
}

impl fmt::Debug for CliRuntimeServicesProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CliRuntimeServicesProvider")
            .field("workspace_root", &self.workspace_root)
            .finish_non_exhaustive()
    }
}

impl CliRuntimeServicesProvider {
    pub(crate) fn new(
        workspace_root: impl AsRef<Path>,
        events: Arc<dyn RuntimeEventSink>,
        clock: Arc<dyn ClockPort>,
    ) -> anyhow::Result<Self> {
        let requested_root = workspace_root.as_ref();
        let canonical_root = dunce::canonicalize(requested_root).map_err(|error| {
            anyhow::anyhow!(
                "workspace root is not available ({}): {error}",
                requested_root.display()
            )
        })?;
        if !canonical_root.is_dir() {
            anyhow::bail!(
                "workspace root is not a directory: {}",
                canonical_root.display()
            );
        }

        Ok(Self {
            workspace_root: canonical_root.clone(),
            filesystem: Arc::new(CliFileSystemService {
                workspace_root: canonical_root.clone(),
            }),
            workspace: Arc::new(CliWorkspaceService {
                workspace_root: canonical_root,
            }),
            events,
            clock,
        })
    }

    pub(crate) fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub(crate) fn build(&self) -> Result<RuntimeServices, RuntimeServicesError> {
        RuntimeServicesRegistry::new()
            .with_provider(CoreRuntimeServicesProvider::new())
            .with_provider(self.clone())
            .build(RuntimeServicesBuilder::new())
    }
}

impl RuntimeServicesProvider for CliRuntimeServicesProvider {
    fn register(&self, builder: RuntimeServicesBuilder) -> RuntimeServicesBuilder {
        debug_assert_eq!(self.filesystem.workspace_root(), self.workspace_root);
        debug_assert_eq!(self.workspace.workspace_root(), self.workspace_root);
        let filesystem: Arc<dyn FileSystemPort> = self.filesystem.clone();
        let workspace: Arc<dyn WorkspacePort> = self.workspace.clone();
        builder
            .with_filesystem(filesystem)
            .with_workspace(workspace)
            .with_events(self.events.clone())
            .with_clock(self.clock.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bitfun_runtime_ports::{
        AgentSubmissionSource, RuntimeEventEnvelope, RuntimeEventType, RuntimeServiceCapability,
    };

    use super::{CliClock, CliRuntimeEventSink, CliRuntimeServicesProvider};

    #[tokio::test]
    async fn provider_registers_required_capability_contracts() {
        let workspace = tempfile::tempdir().expect("workspace");
        let events = Arc::new(CliRuntimeEventSink::new(8));
        let provider =
            CliRuntimeServicesProvider::new(workspace.path(), events.clone(), Arc::new(CliClock))
                .expect("provider");

        let services = provider.build().expect("runtime services");

        assert_eq!(
            provider.workspace_root(),
            dunce::canonicalize(workspace.path()).expect("canonical workspace")
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
            assert!(
                services.has_capability(capability),
                "missing runtime capability registration {capability}"
            );
        }
        assert!(services.clock.now_unix_millis() > 0);

        let mut receiver = events.subscribe();
        let envelope = RuntimeEventEnvelope {
            session_id: "session-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            source: Some(AgentSubmissionSource::Cli),
            event_type: RuntimeEventType::TurnStarted,
            payload: serde_json::json!({ "ready": true }),
        };
        services
            .events
            .publish_runtime_event(envelope.clone())
            .await
            .expect("publish runtime event");
        assert_eq!(receiver.recv().await.expect("runtime event"), envelope);
    }

    #[test]
    fn provider_rejects_a_missing_workspace_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let missing = temp.path().join("missing");

        let error = CliRuntimeServicesProvider::new(
            &missing,
            Arc::new(CliRuntimeEventSink::new(8)),
            Arc::new(CliClock),
        )
        .expect_err("missing workspace must fail");

        assert!(error.to_string().contains("workspace"), "{error}");
    }
}
