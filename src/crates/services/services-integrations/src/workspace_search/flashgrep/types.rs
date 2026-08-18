pub(crate) use super::protocol::{
    DirtyFileStats, FileCount, GroupedLineMatchResults, OpenRepoParams, PathScope, QuerySpec,
    RefreshPolicyConfig, RepoConfig, RepoPhase, RepoStatus, SearchBackend, SearchModeConfig,
    SearchResults, TaskKind, TaskPhase, TaskState, TaskStatus, WorkspaceOverlayStatus,
};

#[derive(Debug, Clone)]
pub(crate) struct SearchRequest {
    pub query: QuerySpec,
    pub scope: PathScope,
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
pub(crate) struct GroupedLineMatchOutcome {
    pub backend: SearchBackend,
    pub status: RepoStatus,
    pub results: GroupedLineMatchResults,
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
        }
    }

    pub(crate) fn with_scope(mut self, scope: PathScope) -> Self {
        self.scope = scope;
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
