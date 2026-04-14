use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

fn default_jsonrpc_version() -> String {
    "2.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestEnvelope {
    #[serde(default = "default_jsonrpc_version")]
    pub jsonrpc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(flatten)]
    pub request: Request,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum Request {
    Initialize {
        params: InitializeParams,
    },
    Initialized,
    Ping,
    #[serde(rename = "index/build")]
    IndexBuild {
        params: RepoRef,
    },
    #[serde(rename = "index/rebuild")]
    IndexRebuild {
        params: RepoRef,
    },
    #[serde(rename = "task/status")]
    TaskStatus {
        params: TaskRef,
    },
    #[serde(rename = "task/cancel")]
    TaskCancel {
        params: TaskRef,
    },
    OpenRepo {
        params: OpenRepoParams,
    },
    EnsureRepo {
        params: EnsureRepoParams,
    },
    GetRepoStatus {
        params: RepoRef,
    },
    RefreshRepo {
        params: RefreshRepoParams,
    },
    BuildIndex {
        params: RepoRef,
    },
    RebuildIndex {
        params: RepoRef,
    },
    Search {
        params: SearchParams,
    },
    Glob {
        params: GlobParams,
    },
    CloseRepo {
        params: RepoRef,
    },
    Shutdown,
    Exit,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InitializeParams {
    #[serde(default)]
    pub client_info: Option<ClientInfo>,
    #[serde(default)]
    pub capabilities: ClientCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientCapabilities {
    #[serde(default)]
    pub progress: bool,
    #[serde(default)]
    pub status_notifications: bool,
    #[serde(default)]
    pub task_notifications: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoRef {
    pub repo_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRef {
    pub task_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshRepoParams {
    pub repo_id: String,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRepoParams {
    pub repo_path: PathBuf,
    pub index_path: Option<PathBuf>,
    #[serde(default)]
    pub config: RepoConfig,
    #[serde(default)]
    pub refresh: RefreshPolicyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsureRepoParams {
    pub repo_path: PathBuf,
    pub index_path: Option<PathBuf>,
    #[serde(default)]
    pub config: RepoConfig,
    #[serde(default)]
    pub refresh: RefreshPolicyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchParams {
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
pub struct GlobParams {
    pub repo_id: String,
    #[serde(default)]
    pub scope: PathScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuerySpec {
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
pub struct PathScope {
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
pub struct RepoConfig {
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
pub struct RefreshPolicyConfig {
    #[serde(default = "default_rebuild_dirty_threshold")]
    pub rebuild_dirty_threshold: usize,
}

impl Default for RefreshPolicyConfig {
    fn default() -> Self {
        Self {
            rebuild_dirty_threshold: default_rebuild_dirty_threshold(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TokenizerModeConfig {
    Trigram,
    #[default]
    SparseNgram,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CorpusModeConfig {
    #[default]
    RespectIgnore,
    NoIgnore,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SearchModeConfig {
    CountOnly,
    CountMatches,
    FirstHitOnly,
    #[default]
    MaterializeMatches,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConsistencyMode {
    SnapshotOnly,
    #[default]
    WorkspaceEventual,
    WorkspaceStrict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseEnvelope {
    #[serde(default = "default_jsonrpc_version")]
    pub jsonrpc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Response>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorResponse>,
}

impl ResponseEnvelope {
    pub fn success(id: Option<u64>, result: Response) -> Self {
        Self {
            jsonrpc: default_jsonrpc_version(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<u64>, error: ErrorResponse) -> Self {
        Self {
            jsonrpc: default_jsonrpc_version(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationEnvelope {
    #[serde(default = "default_jsonrpc_version")]
    pub jsonrpc: String,
    #[serde(flatten)]
    pub notification: Notification,
}

impl NotificationEnvelope {
    pub fn new(notification: Notification) -> Self {
        Self {
            jsonrpc: default_jsonrpc_version(),
            notification,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ServerMessage {
    Response(ResponseEnvelope),
    Notification(NotificationEnvelope),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Response {
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
    RepoEnsured {
        repo_id: String,
        status: RepoStatus,
        indexed_docs: Option<usize>,
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
    TaskCancelled {
        task_id: String,
        accepted: bool,
    },
    RepoBuilt {
        indexed_docs: usize,
        status: RepoStatus,
    },
    RepoRebuilt {
        indexed_docs: usize,
        status: RepoStatus,
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
    RepoClosed {
        repo_id: String,
    },
    ShutdownAck,
    ExitAck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub workspace_open: bool,
    pub workspace_ensure: bool,
    pub workspace_list: bool,
    pub workspace_refresh: bool,
    pub index_build: bool,
    pub index_rebuild: bool,
    pub task_status: bool,
    pub task_cancel: bool,
    pub search_query: bool,
    pub glob_query: bool,
    pub progress_notifications: bool,
    pub status_notifications: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchProtocolCapabilities {
    pub consistency_modes: Vec<ConsistencyMode>,
    pub search_modes: Vec<SearchModeConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method")]
pub enum Notification {
    #[serde(rename = "$/progress")]
    Progress { params: ProgressNotificationParams },
    #[serde(rename = "workspace/statusChanged")]
    WorkspaceStatusChanged {
        params: WorkspaceStatusChangedParams,
    },
    #[serde(rename = "task/finished")]
    TaskFinished { params: TaskFinishedParams },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressNotificationParams {
    pub task_id: String,
    pub workspace_id: String,
    pub kind: TaskKind,
    pub phase: Option<TaskPhase>,
    pub message: String,
    pub processed: usize,
    pub total: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStatusChangedParams {
    pub workspace_id: String,
    pub status: RepoStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskFinishedParams {
    pub task: TaskStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoStatus {
    pub repo_id: String,
    pub repo_path: String,
    pub index_path: String,
    pub phase: RepoPhase,
    pub snapshot_key: Option<String>,
    pub last_probe_unix_secs: Option<u64>,
    pub last_rebuild_unix_secs: Option<u64>,
    pub dirty_files: DirtyFileStats,
    pub rebuild_recommended: bool,
    pub active_task_id: Option<String>,
    pub watcher_healthy: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoPhase {
    Opening,
    MissingIndex,
    Indexing,
    ReadyClean,
    ReadyDirty,
    Rebuilding,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirtyFileStats {
    pub modified: usize,
    pub deleted: usize,
    pub new: usize,
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
    BuildIndex,
    RebuildIndex,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
    IndexedSnapshot,
    IndexedClean,
    IndexedWorkspaceRepair,
    RgFallback,
    ScanFallback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResults {
    pub candidate_docs: usize,
    pub searches_with_match: usize,
    pub bytes_searched: u64,
    pub matched_lines: usize,
    pub matched_occurrences: usize,
    #[serde(default)]
    pub file_counts: Vec<FileCount>,
    #[serde(default)]
    pub file_match_counts: Vec<FileMatchCount>,
    pub hits: Vec<SearchHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCount {
    pub path: String,
    pub matched_lines: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMatchCount {
    pub path: String,
    pub matched_occurrences: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub path: String,
    pub matches: Vec<FileMatch>,
    pub lines: Vec<SearchLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMatch {
    pub location: MatchLocation,
    pub snippet: String,
    pub matched_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchLocation {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SearchLine {
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
