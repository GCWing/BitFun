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
    Initialize {
        params: InitializeParams,
    },
    Initialized,
    Ping,
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
    RefreshRepo {
        params: RefreshRepoParams,
    },
    Search {
        params: SearchParams,
    },
    #[serde(rename = "search/grouped_line_matches")]
    SearchGroupedLineMatches {
        params: SearchParams,
    },
    Glob {
        params: GlobParams,
    },
    CloseRepo {
        params: RepoRef,
    },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct InitializeParams {
    #[serde(default)]
    pub client_info: Option<ClientInfo>,
    #[serde(default)]
    pub capabilities: ClientCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ClientInfo {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ClientCapabilities {
    #[serde(default)]
    pub progress: bool,
    #[serde(default)]
    pub status_notifications: bool,
    #[serde(default)]
    pub task_notifications: bool,
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
pub(crate) struct RefreshRepoParams {
    pub repo_id: String,
    /// `false` lets the daemon skip the walk when it already considers its view current; `true`
    /// forces a reconcile regardless.
    #[serde(default)]
    pub force: bool,
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
    #[serde(default = "default_base_delta_max_segments")]
    pub base_delta_max_segments: usize,
    #[serde(default = "default_base_delta_max_delete_segments")]
    pub base_delta_max_delete_segments: usize,
    #[serde(default = "default_base_delta_max_bytes_ratio")]
    pub base_delta_max_bytes_ratio: f64,
    #[serde(default = "default_base_head_cache_entries")]
    pub base_head_cache_entries: usize,
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
            base_delta_max_segments: default_base_delta_max_segments(),
            base_delta_max_delete_segments: default_base_delta_max_delete_segments(),
            base_delta_max_bytes_ratio: default_base_delta_max_bytes_ratio(),
            base_head_cache_entries: default_base_head_cache_entries(),
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SearchModeConfig {
    FilesWithMatches,
    #[default]
    LineMatches,
    CountOnly,
    CountMatches,
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
pub(crate) struct NotificationEnvelope {
    #[serde(default = "default_jsonrpc_version")]
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)] // response/notification payloads differ structurally
pub(crate) enum ServerMessage {
    Response(ResponseEnvelope),
    Notification(NotificationEnvelope),
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
    InitializeResult {
        protocol_version: u32,
        server_info: ServerInfo,
        capabilities: ServerCapabilities,
        search: SearchProtocolCapabilities,
    },
    InitializedAck,
    Pong {
        now_unix_secs: u64,
    },
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
        status: RepoStatus,
        results: SearchResults,
    },
    SearchGroupedLineMatchesCompleted {
        repo_id: String,
        backend: SearchBackend,
        status: RepoStatus,
        results: GroupedLineMatchResults,
    },
    GlobCompleted {
        repo_id: String,
        status: RepoStatus,
        paths: Vec<String>,
    },
    RepoClosed {
        repo_id: String,
    },
    ShutdownAck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ServerCapabilities {
    pub workspace_open: bool,
    pub workspace_ensure: bool,
    pub workspace_list: bool,
    pub workspace_refresh: bool,
    pub base_snapshot_build: bool,
    pub base_snapshot_rebuild: bool,
    pub task_status: bool,
    pub task_cancel: bool,
    pub search_query: bool,
    pub glob_query: bool,
    pub progress_notifications: bool,
    pub status_notifications: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SearchProtocolCapabilities {
    pub search_modes: Vec<SearchModeConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoStatus {
    pub repo_id: String,
    pub repo_path: String,
    pub storage_root: String,
    pub base_snapshot_root: String,
    pub workspace_overlay_root: String,
    pub phase: RepoPhase,
    pub snapshot_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_head_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_head_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay_head_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay_base_manifest_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_manifest_id: Option<String>,
    #[serde(default)]
    pub base_delta_depth: u32,
    #[serde(default)]
    pub base_delta_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_advance_target_head: Option<String>,
    #[serde(default)]
    pub cached_head_count: usize,
    #[serde(default)]
    pub base_compaction_recommended: bool,
    pub last_probe_unix_secs: Option<u64>,
    pub last_rebuild_unix_secs: Option<u64>,
    pub dirty_files: DirtyFileStats,
    pub active_task_id: Option<String>,
    pub probe_healthy: bool,
    /// `true` means the daemon still owes a worktree reconcile, so `dirty_files`, `phase` and the
    /// published overlay describe the *last observed* worktree instead of the current one. Callers
    /// that need authoritative state ask for it with `refresh_repo`.
    ///
    /// Defaulted so that any daemon older than v0.2.14 — which predates the field and never sends
    /// it — still decodes: those builds reconcile synchronously inside `open_repo`, so `false` is
    /// the correct reading for them.
    #[serde(default)]
    pub workspace_probe_pending: bool,
    pub last_error: Option<String>,
    /// Failure of the daemon's last background base-maintenance task (advance or compaction).
    ///
    /// Kept apart from `last_error` because that slot is shared with the worktree probe, and every
    /// successful probe clears it — measured at ~3.5 s on v0.2.14, well inside our own 5 s idle
    /// status poll, so a maintenance failure reported only there was gone before we could ever
    /// render it. This slot is cleared only when the same kind of maintenance work succeeds.
    ///
    /// Defaulted so daemons older than v0.2.15, which never send the field, still decode as
    /// "nothing failed" — for those builds the failure genuinely is unobservable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_maintenance_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay: Option<WorkspaceOverlayStatus>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoPhase {
    Opening,
    MissingBaseSnapshot,
    BuildingBaseSnapshot,
    ReadyClean,
    ReadyDirty,
    RebuildingBaseSnapshot,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirtyFileStats {
    pub modified: usize,
    pub deleted: usize,
    pub new: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceOverlayStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_manifest_id: Option<String>,
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
pub struct TaskStatus {
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
pub enum TaskKind {
    BuildBaseSnapshot,
    RebuildBaseSnapshot,
    AdvanceBaseSnapshot,
    CompactBaseDeltas,
    RefreshWorkspace,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPhase {
    Scanning,
    Tokenizing,
    Writing,
    Finalizing,
    RefreshingOverlay,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchBackend {
    IndexedClean,
    IndexedWorkspaceView,
    RgFallback,
    ScanFallback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SearchResults {
    pub candidate_docs: usize,
    #[serde(default)]
    pub searches_with_match: usize,
    #[serde(default)]
    pub bytes_searched: u64,
    pub matched_lines: usize,
    pub matched_occurrences: usize,
    #[serde(default)]
    pub matched_paths: Vec<String>,
    #[serde(default)]
    pub file_counts: Vec<FileCount>,
    #[serde(default)]
    pub file_match_counts: Vec<FileMatchCount>,
    #[serde(default)]
    pub line_matches: Vec<LineMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCount {
    pub path: String,
    pub matched_lines: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FileMatchCount {
    pub path: String,
    pub matched_occurrences: usize,
}

/// A single matched line.
///
/// The daemon reports positions only — there is no line text on the wire in any
/// search mode, so content previews have to be hydrated from disk by whoever can
/// read the files (see `workspace_search::line_hydration`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LineMatch {
    pub path: String,
    pub line_number: usize,
}

/// `search/grouped_line_matches` payload: one entry per file instead of one per
/// line, which is what makes single-open hydration possible.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GroupedLineMatchResults {
    pub candidate_docs: usize,
    #[serde(default)]
    pub searches_with_match: usize,
    #[serde(default)]
    pub bytes_searched: u64,
    pub matched_lines: usize,
    pub matched_occurrences: usize,
    #[serde(default)]
    pub limit_reached: bool,
    #[serde(default)]
    pub summary: LineMatchSummary,
    #[serde(default)]
    pub files: Vec<(String, Vec<usize>)>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct LineMatchSummary {
    #[serde(default)]
    pub files_with_matches: usize,
    #[serde(default)]
    pub top_files: Vec<LineMatchFileSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LineMatchFileSummary {
    pub path: String,
    pub matched_lines: usize,
}

fn default_top_k_tokens() -> usize {
    6
}

fn default_max_file_size() -> u64 {
    50 * 1024 * 1024
}

fn default_min_sparse_len() -> usize {
    3
}

fn default_max_sparse_len() -> usize {
    8
}

fn default_base_delta_max_segments() -> usize {
    8
}

fn default_base_delta_max_delete_segments() -> usize {
    8
}

fn default_base_delta_max_bytes_ratio() -> f64 {
    0.10
}

fn default_base_head_cache_entries() -> usize {
    4
}

fn default_overlay_auto_checkpoint_max_uncommitted_ops() -> u64 {
    256
}

fn default_overlay_merge_min_delay_ms() -> u64 {
    2_000
}

fn default_overlay_merge_retry_delay_ms() -> u64 {
    10_000
}
