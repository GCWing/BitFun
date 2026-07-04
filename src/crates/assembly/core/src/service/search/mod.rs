#[cfg(feature = "ssh-remote")]
mod remote;
pub mod service;

#[cfg(not(feature = "ssh-remote"))]
pub use bitfun_services_integrations::remote_ssh::workspace_search::disabled::{
    remote_workspace_search_service_for_path, RemoteWorkspaceSearchService,
};
pub use bitfun_services_integrations::workspace_search::{
    ContentSearchOutputMode, ContentSearchRequest, ContentSearchResult, GlobSearchRequest,
    GlobSearchResult, IndexTaskHandle, WorkspaceIndexStatus, WorkspaceSearchBackend,
    WorkspaceSearchContextLine, WorkspaceSearchDirtyFiles, WorkspaceSearchFileCount,
    WorkspaceSearchHit, WorkspaceSearchLine, WorkspaceSearchMatch, WorkspaceSearchMatchLocation,
    WorkspaceSearchOverlayStatus, WorkspaceSearchRepoPhase, WorkspaceSearchRepoStatus,
    WorkspaceSearchTaskKind, WorkspaceSearchTaskPhase, WorkspaceSearchTaskState,
    WorkspaceSearchTaskStatus,
};
#[cfg(feature = "ssh-remote")]
pub use remote::{remote_workspace_search_service_for_path, RemoteWorkspaceSearchService};
pub use service::{
    get_global_workspace_search_service, resolve_workspace_search_daemon_program_path,
    set_global_workspace_search_service, workspace_search_daemon_available,
    workspace_search_daemon_binary_name, workspace_search_daemon_binary_names,
    workspace_search_daemon_missing_hint, workspace_search_feature_enabled,
    workspace_search_runtime_available, WorkspaceSearchService,
};
