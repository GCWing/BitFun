pub(crate) mod flashgrep;
pub mod service;
pub mod types;

pub use service::{
    get_global_workspace_search_service, set_global_workspace_search_service,
    WorkspaceSearchService,
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
