use crate::infrastructure::FileSearchOutcome;
use crate::service::search::flashgrep::daemon::protocol::{
    FileMatch as FlashgrepFileMatch, MatchLocation as FlashgrepMatchLocation,
    SearchHit as FlashgrepSearchHit, SearchLine as FlashgrepSearchLine,
};
use crate::service::search::flashgrep::sdk::{
    DirtyFileStats as FlashgrepDirtyFileStats, FileCount as FlashgrepFileCount,
    RepoPhase as FlashgrepRepoPhase, RepoStatus as FlashgrepRepoStatus,
    SearchBackend as FlashgrepSearchBackend, SearchModeConfig, TaskKind as FlashgrepTaskKind,
    TaskPhase as FlashgrepTaskPhase, TaskState as FlashgrepTaskState, TaskStatus as FlashgrepTaskStatus,
    WorkspaceOverlayStatus as FlashgrepWorkspaceOverlayStatus,
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
    pub(crate) fn search_mode(self) -> SearchModeConfig {
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
    IndexedWorkspace,
    TextFallback,
    ScanFallback,
}

impl From<FlashgrepSearchBackend> for WorkspaceSearchBackend {
    fn from(value: FlashgrepSearchBackend) -> Self {
        match value {
            FlashgrepSearchBackend::IndexedSnapshot | FlashgrepSearchBackend::IndexedClean => {
                Self::Indexed
            }
            FlashgrepSearchBackend::IndexedWorkspaceView => Self::IndexedWorkspace,
            FlashgrepSearchBackend::RgFallback => Self::TextFallback,
            FlashgrepSearchBackend::ScanFallback => Self::ScanFallback,
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

impl From<FlashgrepRepoPhase> for WorkspaceSearchRepoPhase {
    fn from(value: FlashgrepRepoPhase) -> Self {
        match value {
            FlashgrepRepoPhase::Opening => Self::Preparing,
            FlashgrepRepoPhase::MissingBaseSnapshot => Self::NeedsIndex,
            FlashgrepRepoPhase::BuildingBaseSnapshot => Self::Building,
            FlashgrepRepoPhase::ReadyClean => Self::Ready,
            FlashgrepRepoPhase::ReadyDirty => Self::Stale,
            FlashgrepRepoPhase::RebuildingBaseSnapshot => Self::Refreshing,
            FlashgrepRepoPhase::Degraded => Self::Limited,
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

impl From<FlashgrepTaskKind> for WorkspaceSearchTaskKind {
    fn from(value: FlashgrepTaskKind) -> Self {
        match value {
            FlashgrepTaskKind::BuildBaseSnapshot => Self::Build,
            FlashgrepTaskKind::RebuildBaseSnapshot => Self::Rebuild,
            FlashgrepTaskKind::RefreshWorkspace => Self::Refresh,
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

impl From<FlashgrepTaskState> for WorkspaceSearchTaskState {
    fn from(value: FlashgrepTaskState) -> Self {
        match value {
            FlashgrepTaskState::Queued => Self::Queued,
            FlashgrepTaskState::Running => Self::Running,
            FlashgrepTaskState::Completed => Self::Completed,
            FlashgrepTaskState::Failed => Self::Failed,
            FlashgrepTaskState::Cancelled => Self::Cancelled,
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

impl From<FlashgrepTaskPhase> for WorkspaceSearchTaskPhase {
    fn from(value: FlashgrepTaskPhase) -> Self {
        match value {
            FlashgrepTaskPhase::Scanning => Self::Discovering,
            FlashgrepTaskPhase::Tokenizing => Self::Processing,
            FlashgrepTaskPhase::Writing => Self::Persisting,
            FlashgrepTaskPhase::Finalizing => Self::Finalizing,
            FlashgrepTaskPhase::RefreshingOverlay => Self::Refreshing,
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

impl From<FlashgrepDirtyFileStats> for WorkspaceSearchDirtyFiles {
    fn from(value: FlashgrepDirtyFileStats) -> Self {
        Self {
            modified: value.modified,
            deleted: value.deleted,
            new: value.new,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchOverlayStatus {
    pub committed_seq_no: u64,
    pub last_seq_no: u64,
    pub uncommitted_ops: u64,
    pub pending_docs: usize,
    pub active_segments: usize,
    pub active_delete_segments: usize,
    pub merge_requested: bool,
    pub merge_running: bool,
    pub merge_attempts: u64,
    pub merge_completed: u64,
    pub merge_failed: u64,
    pub last_merge_error: Option<String>,
}

impl From<FlashgrepWorkspaceOverlayStatus> for WorkspaceSearchOverlayStatus {
    fn from(value: FlashgrepWorkspaceOverlayStatus) -> Self {
        Self {
            committed_seq_no: value.committed_seq_no,
            last_seq_no: value.last_seq_no,
            uncommitted_ops: value.uncommitted_ops,
            pending_docs: value.pending_docs,
            active_segments: value.active_segments,
            active_delete_segments: value.active_delete_segments,
            merge_requested: value.merge_requested,
            merge_running: value.merge_running,
            merge_attempts: value.merge_attempts,
            merge_completed: value.merge_completed,
            merge_failed: value.merge_failed,
            last_merge_error: value.last_merge_error,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchRepoStatus {
    pub repo_id: String,
    pub repo_path: String,
    pub storage_root: String,
    pub base_snapshot_root: String,
    pub workspace_overlay_root: String,
    pub phase: WorkspaceSearchRepoPhase,
    pub snapshot_key: Option<String>,
    pub last_probe_unix_secs: Option<u64>,
    pub last_rebuild_unix_secs: Option<u64>,
    pub dirty_files: WorkspaceSearchDirtyFiles,
    pub rebuild_recommended: bool,
    pub active_task_id: Option<String>,
    pub probe_healthy: bool,
    pub last_error: Option<String>,
    pub overlay: Option<WorkspaceSearchOverlayStatus>,
}

impl From<FlashgrepRepoStatus> for WorkspaceSearchRepoStatus {
    fn from(value: FlashgrepRepoStatus) -> Self {
        Self {
            repo_id: value.repo_id,
            repo_path: value.repo_path,
            storage_root: value.storage_root,
            base_snapshot_root: value.base_snapshot_root,
            workspace_overlay_root: value.workspace_overlay_root,
            phase: value.phase.into(),
            snapshot_key: value.snapshot_key,
            last_probe_unix_secs: value.last_probe_unix_secs,
            last_rebuild_unix_secs: value.last_rebuild_unix_secs,
            dirty_files: value.dirty_files.into(),
            rebuild_recommended: value.rebuild_recommended,
            active_task_id: value.active_task_id,
            probe_healthy: value.probe_healthy,
            last_error: value.last_error,
            overlay: value.overlay.map(Into::into),
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

impl From<FlashgrepTaskStatus> for WorkspaceSearchTaskStatus {
    fn from(value: FlashgrepTaskStatus) -> Self {
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

impl From<FlashgrepFileCount> for WorkspaceSearchFileCount {
    fn from(value: FlashgrepFileCount) -> Self {
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

impl From<FlashgrepMatchLocation> for WorkspaceSearchMatchLocation {
    fn from(value: FlashgrepMatchLocation) -> Self {
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

impl From<FlashgrepFileMatch> for WorkspaceSearchMatch {
    fn from(value: FlashgrepFileMatch) -> Self {
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

impl From<FlashgrepSearchLine> for WorkspaceSearchLine {
    fn from(value: FlashgrepSearchLine) -> Self {
        match value {
            FlashgrepSearchLine::Match { value } => Self::Match {
                value: value.into(),
            },
            FlashgrepSearchLine::Context {
                line_number,
                snippet,
            } => Self::Context {
                value: WorkspaceSearchContextLine {
                    line_number,
                    snippet,
                },
            },
            FlashgrepSearchLine::ContextBreak => Self::ContextBreak,
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

impl From<FlashgrepSearchHit> for WorkspaceSearchHit {
    fn from(value: FlashgrepSearchHit) -> Self {
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
