//! Workspace service module
//!
//! Full workspace management system: open, manage, scan, statistics, etc.

pub mod factory;
#[cfg(feature = "workspace-watch")]
pub mod identity_watch;
pub mod manager;
pub mod provider;
pub mod service;
#[cfg(feature = "git")]
pub mod worktree_topology;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeTopologyFreshness {
    Cached,
    ForceRefresh,
}

// Re-export main components
pub use factory::WorkspaceFactory;
#[cfg(feature = "workspace-watch")]
pub use identity_watch::WorkspaceIdentityWatchService;
pub use manager::{
    GitInfo, PrimaryAssistantKey, RelatedPath, ScanOptions, WorkspaceIdentity, WorkspaceInfo,
    WorkspaceKind, WorkspaceManager, WorkspaceManagerConfig, WorkspaceManagerStatistics,
    WorkspaceOpenOptions, WorkspaceStatistics, WorkspaceStatus, WorkspaceSummary, WorkspaceType,
    WorkspaceWorktreeInfo,
};
pub use provider::{WorkspaceCleanupResult, WorkspaceProvider, WorkspaceSystemSummary};
pub use service::{
    get_global_workspace_service, set_global_workspace_service, BatchImportResult,
    BatchRemoveResult, WorkspaceActivityMode, WorkspaceCreateOptions, WorkspaceExport,
    WorkspaceHealthStatus, WorkspaceIdentityChangedEvent, WorkspaceImportResult,
    WorkspaceInfoUpdates, WorkspaceQuickSummary, WorkspaceService,
};
#[cfg(feature = "git")]
pub use worktree_topology::{global_worktree_topology_service, WorktreeTopologyService};
