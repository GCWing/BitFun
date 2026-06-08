//! Workspace search integration owner.
//!
//! This module owns the local flashgrep daemon/session lifecycle and shared
//! indexed-search DTOs. Product/runtime crates may wrap it to provide product
//! config, bootstrap hooks, and legacy error mapping.

pub(crate) mod flashgrep;
pub(crate) mod result_mapping;
mod service;
mod types;

pub use service::{
    resolve_workspace_search_daemon_program_path, workspace_search_daemon_available,
    workspace_search_daemon_binary_name, workspace_search_daemon_binary_names,
    workspace_search_daemon_missing_hint, WorkspaceSearchRepoConfig, WorkspaceSearchResult,
    WorkspaceSearchRuntimeHooks, WorkspaceSearchService,
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
