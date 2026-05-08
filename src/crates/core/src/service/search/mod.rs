pub(crate) mod flashgrep;
pub mod service;
pub mod types;

pub use service::{
    get_global_workspace_search_service, resolve_workspace_search_daemon_program_path,
    set_global_workspace_search_service, workspace_search_daemon_available,
    workspace_search_daemon_binary_name, workspace_search_daemon_binary_names,
    workspace_search_daemon_missing_hint, workspace_search_feature_enabled,
    workspace_search_runtime_available, WorkspaceSearchService,
};
pub use types::{
    ContentSearchOutputMode, ContentSearchRequest, ContentSearchResult, GlobSearchRequest,
    GlobSearchResult, IndexTaskHandle, WorkspaceIndexStatus, WorkspaceSearchBackend,
    WorkspaceSearchContextLine, WorkspaceSearchDirtyFiles, WorkspaceSearchFileCount,
    WorkspaceSearchHit, WorkspaceSearchLine, WorkspaceSearchMatch, WorkspaceSearchMatchLocation,
    WorkspaceSearchOverlayStatus, WorkspaceSearchRepoPhase, WorkspaceSearchRepoStatus,
    WorkspaceSearchTaskKind, WorkspaceSearchTaskPhase, WorkspaceSearchTaskState,
    WorkspaceSearchTaskStatus,
};
