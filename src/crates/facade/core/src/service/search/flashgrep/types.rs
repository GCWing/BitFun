pub(crate) use super::protocol::{
    ConsistencyMode, DirtyFileStats, FileCount, OpenRepoParams, PathScope, QuerySpec,
    RefreshPolicyConfig, RepoConfig, RepoPhase, RepoStatus, SearchBackend, SearchModeConfig,
    SearchResults, TaskKind, TaskPhase, TaskState, TaskStatus, WorkspaceOverlayStatus,
};

#[derive(Debug, Clone)]
pub(crate) struct SearchRequest {
    pub query: QuerySpec,
    pub scope: PathScope,
    pub consistency: ConsistencyMode,
    pub allow_scan_fallback: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct GlobRequest {
    pub scope: PathScope,
}

#[derive(Debug, Clone)]
pub(crate) struct SearchOutcome {
    pub backend: SearchBackend,
    pub status: RepoStatus,
    pub results: SearchResults,
}

#[derive(Debug, Clone)]
pub(crate) struct GlobOutcome {
    pub status: RepoStatus,
    pub paths: Vec<String>,
}

impl SearchRequest {
    pub(crate) fn new(query: QuerySpec) -> Self {
        Self {
            query,
            scope: PathScope::default(),
            consistency: ConsistencyMode::WorkspaceEventual,
            allow_scan_fallback: false,
        }
    }

    pub(crate) fn with_scope(mut self, scope: PathScope) -> Self {
        self.scope = scope;
        self
    }

    pub(crate) fn with_consistency(mut self, consistency: ConsistencyMode) -> Self {
        self.consistency = consistency;
        self
    }

    pub(crate) fn with_scan_fallback(mut self, allow_scan_fallback: bool) -> Self {
        self.allow_scan_fallback = allow_scan_fallback;
        self
    }
}

impl GlobRequest {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_scope(mut self, scope: PathScope) -> Self {
        self.scope = scope;
        self
    }
}
