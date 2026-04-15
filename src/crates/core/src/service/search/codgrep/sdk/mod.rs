pub mod tokio;

pub use crate::service::search::codgrep::daemon::protocol::{
    ConsistencyMode, DirtyFileStats, FileCount, OpenRepoParams, PathScope, QuerySpec,
    RefreshPolicyConfig, RepoConfig, RepoPhase, RepoStatus, SearchBackend, SearchModeConfig,
    SearchResults, TaskKind, TaskPhase, TaskState, TaskStatus,
};

#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub query: QuerySpec,
    pub scope: PathScope,
    pub consistency: ConsistencyMode,
    pub allow_scan_fallback: bool,
}

#[derive(Debug, Clone, Default)]
pub struct GlobRequest {
    pub scope: PathScope,
}

#[derive(Debug, Clone)]
pub struct SearchOutcome {
    pub backend: SearchBackend,
    pub status: RepoStatus,
    pub results: SearchResults,
}

#[derive(Debug, Clone)]
pub struct GlobOutcome {
    pub status: RepoStatus,
    pub paths: Vec<String>,
}

impl SearchRequest {
    pub fn new(query: QuerySpec) -> Self {
        Self {
            query,
            scope: PathScope::default(),
            consistency: ConsistencyMode::WorkspaceEventual,
            allow_scan_fallback: false,
        }
    }

    pub fn with_scope(mut self, scope: PathScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn with_consistency(mut self, consistency: ConsistencyMode) -> Self {
        self.consistency = consistency;
        self
    }

    pub fn with_scan_fallback(mut self, allow_scan_fallback: bool) -> Self {
        self.allow_scan_fallback = allow_scan_fallback;
        self
    }
}

impl GlobRequest {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_scope(mut self, scope: PathScope) -> Self {
        self.scope = scope;
        self
    }
}
