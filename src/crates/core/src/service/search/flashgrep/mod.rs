mod client;
pub mod error;
mod protocol;
mod types;

pub(crate) use client::{ManagedClient, RepoSession};
pub(crate) use protocol::{FileMatch, MatchLocation, SearchHit, SearchLine};
pub(crate) use types::{
    ConsistencyMode, DirtyFileStats, FileCount, GlobRequest, OpenRepoParams, PathScope, QuerySpec,
    RefreshPolicyConfig, RepoConfig, RepoPhase, RepoStatus, SearchBackend, SearchModeConfig,
    SearchRequest, SearchResults, TaskKind, TaskPhase, TaskState, TaskStatus,
    WorkspaceOverlayStatus,
};
