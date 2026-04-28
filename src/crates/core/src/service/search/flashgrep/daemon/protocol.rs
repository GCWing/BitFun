use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

fn default_jsonrpc_version() -> String {
    "2.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RequestEnvelope {
    #[serde(default = "default_jsonrpc_version")]
    pub jsonrpc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(flatten)]
    pub request: Request,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub(crate) enum Request {
    #[serde(rename = "base_snapshot/build")]
    BaseSnapshotBuild {
        params: RepoRef,
    },
    #[serde(rename = "base_snapshot/rebuild")]
    BaseSnapshotRebuild {
        params: RepoRef,
    },
    #[serde(rename = "task/status")]
    TaskStatus {
        params: TaskRef,
    },
    OpenRepo {
        params: OpenRepoParams,
    },
    GetRepoStatus {
        params: RepoRef,
    },
    Search {
        params: SearchParams,
    },
    Glob {
        params: GlobParams,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RepoRef {
    pub repo_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TaskRef {
    pub task_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OpenRepoParams {
    pub repo_path: PathBuf,
    #[serde(default)]
    pub storage_root: Option<PathBuf>,
    #[serde(default)]
    pub config: RepoConfig,
    #[serde(default)]
    pub refresh: RefreshPolicyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SearchParams {
    pub repo_id: String,
    pub query: QuerySpec,
    #[serde(default)]
    pub scope: PathScope,
    #[serde(default)]
    pub consistency: ConsistencyMode,
    #[serde(default)]
    pub allow_scan_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GlobParams {
    pub repo_id: String,
    #[serde(default)]
    pub scope: PathScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct QuerySpec {
    pub pattern: String,
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub case_insensitive: bool,
    #[serde(default)]
    pub multiline: bool,
    #[serde(default)]
    pub dot_matches_new_line: bool,
    #[serde(default)]
    pub fixed_strings: bool,
    #[serde(default)]
    pub word_regexp: bool,
    #[serde(default)]
    pub line_regexp: bool,
    #[serde(default)]
    pub before_context: usize,
    #[serde(default)]
    pub after_context: usize,
    #[serde(default = "default_top_k_tokens")]
    pub top_k_tokens: usize,
    #[serde(default)]
    pub max_count: Option<usize>,
    #[serde(default)]
    pub global_max_results: Option<usize>,
    #[serde(default)]
    pub search_mode: SearchModeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct PathScope {
    #[serde(default)]
    pub roots: Vec<PathBuf>,
    #[serde(default)]
    pub globs: Vec<String>,
    #[serde(default)]
    pub iglobs: Vec<String>,
    #[serde(default)]
    pub type_add: Vec<String>,
    #[serde(default)]
    pub type_clear: Vec<String>,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default)]
    pub type_not: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RepoConfig {
    #[serde(default)]
    pub tokenizer: TokenizerModeConfig,
    #[serde(default)]
    pub corpus_mode: CorpusModeConfig,
    #[serde(default)]
    pub include_hidden: bool,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    #[serde(default = "default_min_sparse_len")]
    pub min_sparse_len: usize,
    #[serde(default = "default_max_sparse_len")]
    pub max_sparse_len: usize,
}

impl Default for RepoConfig {
    fn default() -> Self {
        Self {
            tokenizer: TokenizerModeConfig::default(),
            corpus_mode: CorpusModeConfig::default(),
            include_hidden: false,
            max_file_size: default_max_file_size(),
            min_sparse_len: default_min_sparse_len(),
            max_sparse_len: default_max_sparse_len(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RefreshPolicyConfig {
    #[serde(default = "default_rebuild_dirty_threshold")]
    pub rebuild_dirty_threshold: usize,
    #[serde(default = "default_overlay_auto_checkpoint_max_uncommitted_ops")]
    pub overlay_auto_checkpoint_max_uncommitted_ops: u64,
    #[serde(default = "default_overlay_merge_min_delay_ms")]
    pub overlay_merge_min_delay_ms: u64,
    #[serde(default = "default_overlay_merge_retry_delay_ms")]
    pub overlay_merge_retry_delay_ms: u64,
}

impl Default for RefreshPolicyConfig {
    fn default() -> Self {
        Self {
            rebuild_dirty_threshold: default_rebuild_dirty_threshold(),
            overlay_auto_checkpoint_max_uncommitted_ops:
                default_overlay_auto_checkpoint_max_uncommitted_ops(),
            overlay_merge_min_delay_ms: default_overlay_merge_min_delay_ms(),
            overlay_merge_retry_delay_ms: default_overlay_merge_retry_delay_ms(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TokenizerModeConfig {
    Trigram,
    #[default]
    SparseNgram,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CorpusModeConfig {
    #[default]
    RespectIgnore,
    NoIgnore,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SearchModeConfig {
    CountOnly,
    CountMatches,
    FirstHitOnly,
    #[default]
    MaterializeMatches,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConsistencyMode {
    SnapshotOnly,
    #[default]
    WorkspaceEventual,
    WorkspaceStrict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResponseEnvelope {
    #[serde(default = "default_jsonrpc_version")]
    pub jsonrpc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Response>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ErrorResponse {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Response {
    RepoOpened {
        repo_id: String,
        status: RepoStatus,
    },
    RepoStatus {
        status: RepoStatus,
    },
    TaskStarted {
        task: TaskStatus,
    },
    TaskStatus {
        task: TaskStatus,
    },
    SearchCompleted {
        repo_id: String,
        backend: SearchBackend,
        consistency_applied: ConsistencyMode,
        status: RepoStatus,
        results: SearchResults,
    },
    GlobCompleted {
        repo_id: String,
        status: RepoStatus,
        paths: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RepoStatus {
    pub repo_id: String,
    pub repo_path: String,
    pub storage_root: String,
    pub base_snapshot_root: String,
    pub workspace_overlay_root: String,
    pub phase: RepoPhase,
    pub snapshot_key: Option<String>,
    pub last_probe_unix_secs: Option<u64>,
    pub last_rebuild_unix_secs: Option<u64>,
    pub dirty_files: DirtyFileStats,
    pub rebuild_recommended: bool,
    pub active_task_id: Option<String>,
    pub probe_healthy: bool,
    pub last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay: Option<WorkspaceOverlayStatus>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RepoPhase {
    Opening,
    MissingBaseSnapshot,
    BuildingBaseSnapshot,
    ReadyClean,
    ReadyDirty,
    RebuildingBaseSnapshot,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DirtyFileStats {
    pub modified: usize,
    pub deleted: usize,
    pub new: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkspaceOverlayStatus {
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_merge_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TaskStatus {
    pub task_id: String,
    pub workspace_id: String,
    pub kind: TaskKind,
    pub state: TaskState,
    pub phase: Option<TaskPhase>,
    pub message: String,
    pub processed: usize,
    pub total: Option<usize>,
    pub started_unix_secs: u64,
    pub updated_unix_secs: u64,
    pub finished_unix_secs: Option<u64>,
    pub cancellable: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TaskKind {
    BuildBaseSnapshot,
    RebuildBaseSnapshot,
    RefreshWorkspace,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TaskState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TaskPhase {
    Scanning,
    Tokenizing,
    Writing,
    Finalizing,
    RefreshingOverlay,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SearchBackend {
    IndexedSnapshot,
    IndexedClean,
    IndexedWorkspaceView,
    RgFallback,
    ScanFallback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SearchResults {
    pub candidate_docs: usize,
    pub matched_lines: usize,
    pub matched_occurrences: usize,
    #[serde(default)]
    pub file_counts: Vec<FileCount>,
    #[serde(default)]
    pub hits: Vec<SearchHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FileCount {
    pub path: String,
    pub matched_lines: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SearchHit {
    pub path: String,
    pub matches: Vec<FileMatch>,
    pub lines: Vec<SearchLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FileMatch {
    pub location: MatchLocation,
    pub snippet: String,
    pub matched_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MatchLocation {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum SearchLine {
    Match { value: FileMatch },
    Context { line_number: usize, snippet: String },
    ContextBreak,
}

fn default_top_k_tokens() -> usize {
    6
}

fn default_max_file_size() -> u64 {
    2 * 1024 * 1024
}

fn default_min_sparse_len() -> usize {
    3
}

fn default_max_sparse_len() -> usize {
    8
}

fn default_rebuild_dirty_threshold() -> usize {
    256
}

fn default_overlay_auto_checkpoint_max_uncommitted_ops() -> u64 {
    1_024
}

fn default_overlay_merge_min_delay_ms() -> u64 {
    2_000
}

fn default_overlay_merge_retry_delay_ms() -> u64 {
    10_000
}
