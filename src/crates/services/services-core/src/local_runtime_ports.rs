//! Reusable local implementations of the required runtime service ports.
//!
//! Product composition roots select these ports. This module only owns local
//! workspace identity, the system clock, and the in-process event sink.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use bitfun_runtime_ports::{
    ClockPort, FileSystemPort, PortResult, RuntimeEventEnvelope, RuntimeEventSink,
    RuntimeServiceCapability, RuntimeServicePort, WorkspacePort,
};
use tokio::sync::broadcast;

#[derive(Debug)]
struct LocalFileSystemPort {
    _workspace_root: PathBuf,
}

impl RuntimeServicePort for LocalFileSystemPort {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::FileSystem
    }
}

impl FileSystemPort for LocalFileSystemPort {}

#[derive(Debug)]
struct LocalWorkspacePort {
    _workspace_root: PathBuf,
}

impl RuntimeServicePort for LocalWorkspacePort {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::Workspace
    }
}

impl WorkspacePort for LocalWorkspacePort {}

#[derive(Debug, Clone, Copy, Default)]
struct SystemClock;

impl RuntimeServicePort for SystemClock {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::Clock
    }
}

impl ClockPort for SystemClock {
    fn now_unix_millis(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
struct LocalRuntimeEventSink {
    tx: broadcast::Sender<RuntimeEventEnvelope>,
}

impl LocalRuntimeEventSink {
    fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity.max(1));
        Self { tx }
    }
}

#[async_trait::async_trait]
impl RuntimeEventSink for LocalRuntimeEventSink {
    async fn publish_runtime_event(&self, event: RuntimeEventEnvelope) -> PortResult<()> {
        let _ = self.tx.send(event);
        Ok(())
    }
}

/// Local runtime ports bound to one canonical workspace.
#[derive(Clone)]
pub struct LocalRuntimePorts {
    workspace_root: PathBuf,
    filesystem: Arc<dyn FileSystemPort>,
    workspace: Arc<dyn WorkspacePort>,
    events: Arc<dyn RuntimeEventSink>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for LocalRuntimePorts {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LocalRuntimePorts")
            .field("workspace_root", &self.workspace_root)
            .finish_non_exhaustive()
    }
}

impl LocalRuntimePorts {
    pub fn new(workspace_root: impl AsRef<Path>, event_capacity: usize) -> anyhow::Result<Self> {
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
            filesystem: Arc::new(LocalFileSystemPort {
                _workspace_root: canonical_root.clone(),
            }),
            workspace: Arc::new(LocalWorkspacePort {
                _workspace_root: canonical_root,
            }),
            events: Arc::new(LocalRuntimeEventSink::new(event_capacity)),
            clock: Arc::new(SystemClock),
        })
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn filesystem(&self) -> Arc<dyn FileSystemPort> {
        self.filesystem.clone()
    }

    pub fn workspace(&self) -> Arc<dyn WorkspacePort> {
        self.workspace.clone()
    }

    pub fn events(&self) -> Arc<dyn RuntimeEventSink> {
        self.events.clone()
    }

    pub fn clock(&self) -> Arc<dyn ClockPort> {
        self.clock.clone()
    }
}
