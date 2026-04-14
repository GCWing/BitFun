use crate::infrastructure::FileSearchOutcome;
use codgrep::daemon::protocol::{
    FileMatch as CodgrepFileMatch, MatchLocation as CodgrepMatchLocation,
    SearchHit as CodgrepSearchHit, SearchLine as CodgrepSearchLine,
};
use codgrep::sdk::{
    DirtyFileStats as CodgrepDirtyFileStats, FileCount as CodgrepFileCount,
    RepoPhase as CodgrepRepoPhase, RepoStatus as CodgrepRepoStatus,
    SearchBackend as CodgrepSearchBackend, SearchModeConfig, TaskKind as CodgrepTaskKind,
    TaskPhase as CodgrepTaskPhase, TaskState as CodgrepTaskState, TaskStatus as CodgrepTaskStatus,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentSearchOutputMode {
    Content,
    FilesWithMatches,
    Count,
}

impl ContentSearchOutputMode {
    pub fn search_mode(self) -> SearchModeConfig {
        match self {
            Self::Content => SearchModeConfig::MaterializeMatches,
            Self::Count => SearchModeConfig::CountOnly,
            Self::FilesWithMatches => SearchModeConfig::FirstHitOnly,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContentSearchRequest {
    pub repo_root: PathBuf,
    pub search_path: Option<PathBuf>,
    pub pattern: String,
    pub output_mode: ContentSearchOutputMode,
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub whole_word: bool,
    pub multiline: bool,
    pub before_context: usize,
    pub after_context: usize,
    pub max_results: Option<usize>,
    pub globs: Vec<String>,
    pub file_types: Vec<String>,
    pub exclude_file_types: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GlobSearchRequest {
    pub repo_root: PathBuf,
    pub search_path: Option<PathBuf>,
    pub pattern: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceSearchBackend {
    Indexed,
    IndexedRepair,
    TextFallback,
    ScanFallback,
}

impl From<CodgrepSearchBackend> for WorkspaceSearchBackend {
    fn from(value: CodgrepSearchBackend) -> Self {
        match value {
            CodgrepSearchBackend::IndexedSnapshot | CodgrepSearchBackend::IndexedClean => {
                Self::Indexed
            }
            CodgrepSearchBackend::IndexedWorkspaceRepair => Self::IndexedRepair,
            CodgrepSearchBackend::RgFallback => Self::TextFallback,
            CodgrepSearchBackend::ScanFallback => Self::ScanFallback,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceSearchRepoPhase {
    Preparing,
    NeedsIndex,
    Building,
    Ready,
    Stale,
    Refreshing,
    Limited,
}

impl From<CodgrepRepoPhase> for WorkspaceSearchRepoPhase {
    fn from(value: CodgrepRepoPhase) -> Self {
        match value {
            CodgrepRepoPhase::Opening => Self::Preparing,
            CodgrepRepoPhase::MissingIndex => Self::NeedsIndex,
            CodgrepRepoPhase::Indexing => Self::Building,
            CodgrepRepoPhase::ReadyClean => Self::Ready,
            CodgrepRepoPhase::ReadyDirty => Self::Stale,
            CodgrepRepoPhase::Rebuilding => Self::Refreshing,
            CodgrepRepoPhase::Degraded => Self::Limited,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceSearchTaskKind {
    Build,
    Rebuild,
    Refresh,
}

impl From<CodgrepTaskKind> for WorkspaceSearchTaskKind {
    fn from(value: CodgrepTaskKind) -> Self {
        match value {
            CodgrepTaskKind::BuildIndex => Self::Build,
            CodgrepTaskKind::RebuildIndex => Self::Rebuild,
            CodgrepTaskKind::RefreshWorkspace => Self::Refresh,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceSearchTaskState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl From<CodgrepTaskState> for WorkspaceSearchTaskState {
    fn from(value: CodgrepTaskState) -> Self {
        match value {
            CodgrepTaskState::Queued => Self::Queued,
            CodgrepTaskState::Running => Self::Running,
            CodgrepTaskState::Completed => Self::Completed,
            CodgrepTaskState::Failed => Self::Failed,
            CodgrepTaskState::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceSearchTaskPhase {
    Discovering,
    Processing,
    Persisting,
    Finalizing,
    Refreshing,
}

impl From<CodgrepTaskPhase> for WorkspaceSearchTaskPhase {
    fn from(value: CodgrepTaskPhase) -> Self {
        match value {
            CodgrepTaskPhase::Scanning => Self::Discovering,
            CodgrepTaskPhase::Tokenizing => Self::Processing,
            CodgrepTaskPhase::Writing => Self::Persisting,
            CodgrepTaskPhase::Finalizing => Self::Finalizing,
            CodgrepTaskPhase::RefreshingOverlay => Self::Refreshing,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchDirtyFiles {
    pub modified: usize,
    pub deleted: usize,
    pub new: usize,
}

impl From<CodgrepDirtyFileStats> for WorkspaceSearchDirtyFiles {
    fn from(value: CodgrepDirtyFileStats) -> Self {
        Self {
            modified: value.modified,
            deleted: value.deleted,
            new: value.new,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchRepoStatus {
    pub repo_id: String,
    pub repo_path: String,
    pub index_path: String,
    pub phase: WorkspaceSearchRepoPhase,
    pub snapshot_key: Option<String>,
    pub last_probe_unix_secs: Option<u64>,
    pub last_rebuild_unix_secs: Option<u64>,
    pub dirty_files: WorkspaceSearchDirtyFiles,
    pub rebuild_recommended: bool,
    pub active_task_id: Option<String>,
    pub watcher_healthy: bool,
    pub last_error: Option<String>,
}

impl From<CodgrepRepoStatus> for WorkspaceSearchRepoStatus {
    fn from(value: CodgrepRepoStatus) -> Self {
        Self {
            repo_id: value.repo_id,
            repo_path: value.repo_path,
            index_path: value.index_path,
            phase: value.phase.into(),
            snapshot_key: value.snapshot_key,
            last_probe_unix_secs: value.last_probe_unix_secs,
            last_rebuild_unix_secs: value.last_rebuild_unix_secs,
            dirty_files: value.dirty_files.into(),
            rebuild_recommended: value.rebuild_recommended,
            active_task_id: value.active_task_id,
            watcher_healthy: value.watcher_healthy,
            last_error: value.last_error,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchTaskStatus {
    pub task_id: String,
    pub workspace_id: String,
    pub kind: WorkspaceSearchTaskKind,
    pub state: WorkspaceSearchTaskState,
    pub phase: Option<WorkspaceSearchTaskPhase>,
    pub message: String,
    pub processed: usize,
    pub total: Option<usize>,
    pub started_unix_secs: u64,
    pub updated_unix_secs: u64,
    pub finished_unix_secs: Option<u64>,
    pub cancellable: bool,
    pub error: Option<String>,
}

impl From<CodgrepTaskStatus> for WorkspaceSearchTaskStatus {
    fn from(value: CodgrepTaskStatus) -> Self {
        Self {
            task_id: value.task_id,
            workspace_id: value.workspace_id,
            kind: value.kind.into(),
            state: value.state.into(),
            phase: value.phase.map(Into::into),
            message: value.message,
            processed: value.processed,
            total: value.total,
            started_unix_secs: value.started_unix_secs,
            updated_unix_secs: value.updated_unix_secs,
            finished_unix_secs: value.finished_unix_secs,
            cancellable: value.cancellable,
            error: value.error,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchFileCount {
    pub path: String,
    pub matched_lines: usize,
}

impl From<CodgrepFileCount> for WorkspaceSearchFileCount {
    fn from(value: CodgrepFileCount) -> Self {
        Self {
            path: value.path,
            matched_lines: value.matched_lines,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchMatchLocation {
    pub line: usize,
    pub column: usize,
}

impl From<CodgrepMatchLocation> for WorkspaceSearchMatchLocation {
    fn from(value: CodgrepMatchLocation) -> Self {
        Self {
            line: value.line,
            column: value.column,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchMatch {
    pub location: WorkspaceSearchMatchLocation,
    pub snippet: String,
    pub matched_text: String,
}

impl From<CodgrepFileMatch> for WorkspaceSearchMatch {
    fn from(value: CodgrepFileMatch) -> Self {
        Self {
            location: value.location.into(),
            snippet: value.snippet,
            matched_text: value.matched_text,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchContextLine {
    pub line_number: usize,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkspaceSearchLine {
    Match { value: WorkspaceSearchMatch },
    Context { value: WorkspaceSearchContextLine },
    ContextBreak,
}

impl From<CodgrepSearchLine> for WorkspaceSearchLine {
    fn from(value: CodgrepSearchLine) -> Self {
        match value {
            CodgrepSearchLine::Match { value } => Self::Match {
                value: value.into(),
            },
            CodgrepSearchLine::Context {
                line_number,
                snippet,
            } => Self::Context {
                value: WorkspaceSearchContextLine {
                    line_number,
                    snippet,
                },
            },
            CodgrepSearchLine::ContextBreak => Self::ContextBreak,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchHit {
    pub path: String,
    pub matches: Vec<WorkspaceSearchMatch>,
    pub lines: Vec<WorkspaceSearchLine>,
}

impl From<CodgrepSearchHit> for WorkspaceSearchHit {
    fn from(value: CodgrepSearchHit) -> Self {
        Self {
            path: value.path,
            matches: value.matches.into_iter().map(Into::into).collect(),
            lines: value.lines.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceIndexStatus {
    pub repo_status: WorkspaceSearchRepoStatus,
    pub active_task: Option<WorkspaceSearchTaskStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentSearchResult {
    pub outcome: FileSearchOutcome,
    pub file_counts: Vec<WorkspaceSearchFileCount>,
    pub hits: Vec<WorkspaceSearchHit>,
    pub backend: WorkspaceSearchBackend,
    pub repo_status: WorkspaceSearchRepoStatus,
    pub candidate_docs: usize,
    pub matched_lines: usize,
    pub matched_occurrences: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobSearchResult {
    pub paths: Vec<String>,
    pub repo_status: WorkspaceSearchRepoStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexTaskHandle {
    pub task: WorkspaceSearchTaskStatus,
    pub repo_status: WorkspaceSearchRepoStatus,
}
