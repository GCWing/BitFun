use super::{
    build_remote_scope, join_remote_path, local_flashgrep_bundle_for_arch,
    looks_like_linux_workspace_root, parse_remote_architecture_output, parse_remote_os_output,
    remote_flashgrep_install_dir, remote_stdio_search_mode, remote_workspace_search_storage_root,
    shell_escape, should_retry_remote_scan_fallback_as_files_with_matches, LocalFlashgrepBundle,
    REMOTE_ARCHITECTURE_PROBES, REMOTE_OS_PROBES,
};
use crate::remote_ssh::{normalize_remote_workspace_path, RemoteWorkspaceEntry};
use crate::workspace_search::flashgrep::error::AppError;
use crate::workspace_search::flashgrep::{
    drain_content_length_messages, log_flashgrep_stderr_line_with_context, ClientCapabilities,
    ClientInfo, ConsistencyMode, FlashgrepRepoSession, GlobOutcome, GlobParams, GlobRequest,
    InitializeParams, OpenRepoParams, ProtocolClient, QuerySpec, RefreshPolicyConfig, RepoConfig,
    RepoRef, RepoStatus, Request, Response, SearchBackend, SearchModeConfig, SearchOutcome,
    SearchParams, SearchRequest, SearchResults, TaskRef, TaskStatus, FLASHGREP_LOG_TARGET,
};
use crate::workspace_search::result_mapping::convert_search_results;
use crate::workspace_search::{
    ContentSearchRequest, ContentSearchResult, GlobSearchRequest, GlobSearchResult,
    IndexTaskHandle, WorkspaceIndexStatus, WorkspaceSearchFileCount, WorkspaceSearchHit,
    WorkspaceSearchRepoStatus,
};
use async_trait::async_trait;
use bitfun_services_core::filesystem::FileSearchOutcome;
use std::collections::HashMap;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, LazyLock,
};
use std::time::Duration;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{sleep, timeout};

const REMOTE_STDIO_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const REMOTE_STDIO_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const REMOTE_STDIO_SESSION_IDLE_GRACE: Duration = Duration::from_secs(45);
const CLIENT_NAME: &str = "bitfun-remote-workspace-search";

static REMOTE_STDIO_SESSIONS: LazyLock<RwLock<HashMap<String, RemoteStdioSessionEntry>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static REMOTE_STDIO_OPEN_GUARDS: LazyLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static REMOTE_SEARCH_CONTEXTS: LazyLock<RwLock<HashMap<String, RemoteSearchContext>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

#[derive(Debug, Clone)]
pub struct RemoteCommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Clone)]
pub struct RemoteWorkspaceSearchStdioProtocol {
    protocol: ProtocolClient,
}

impl RemoteWorkspaceSearchStdioProtocol {
    fn new(protocol: ProtocolClient) -> Self {
        Self { protocol }
    }

    pub async fn handle_stdout_chunk(
        &self,
        read_buffer: &mut Vec<u8>,
        data: &[u8],
    ) -> Result<(), String> {
        read_buffer.extend_from_slice(data);
        let messages =
            drain_content_length_messages(read_buffer).map_err(|error| error.to_string())?;
        for message in messages {
            self.protocol.handle_server_message(message).await;
        }
        Ok(())
    }

    pub fn log_stderr_line_with_context(&self, context: Option<&str>, line: &str) {
        log_flashgrep_stderr_line_with_context(context, line);
    }

    pub async fn close_with_message(&self, message: impl Into<String>) {
        self.protocol.close_with_message(message).await;
    }
}

#[async_trait]
pub trait RemoteWorkspaceSearchProvider: Send + Sync {
    async fn resolve_workspace_entry(
        &self,
        root_path: &str,
        preferred_connection_id: Option<&str>,
    ) -> Result<RemoteWorkspaceEntry, String>;

    async fn cached_server_os_type(&self, connection_id: &str) -> Option<String>;

    async fn execute_command(
        &self,
        connection_id: &str,
        command: &str,
    ) -> Result<RemoteCommandOutput, String>;

    async fn create_dir_all(&self, connection_id: &str, path: &str) -> Result<(), String>;

    async fn write_file(
        &self,
        connection_id: &str,
        path: &str,
        contents: &[u8],
    ) -> Result<(), String>;

    async fn repo_max_file_size(&self) -> u64;

    async fn spawn_stdio_daemon(
        &self,
        connection_id: &str,
        command: &str,
        write_rx: mpsc::Receiver<Vec<u8>>,
        protocol: RemoteWorkspaceSearchStdioProtocol,
    ) -> Result<(), String>;
}

#[derive(Clone)]
struct RemoteStdioSessionEntry {
    session: Arc<RemoteStdioRepoSession>,
    activity_epoch: Arc<AtomicU64>,
}

struct RemoteStdioRepoSession {
    repo_id: String,
    client: Arc<RemoteStdioDaemonClient>,
    activity_epoch: Arc<AtomicU64>,
    active_operations: Arc<AtomicU64>,
}

struct RemoteStdioDaemonClient {
    protocol: ProtocolClient,
}

struct RemoteStdioOperationLease {
    activity_epoch: Arc<AtomicU64>,
    active_operations: Arc<AtomicU64>,
}

struct RemoteStdioSessionLease {
    session: Arc<RemoteStdioRepoSession>,
    _operation: RemoteStdioOperationLease,
}

impl Drop for RemoteStdioOperationLease {
    fn drop(&mut self) {
        self.active_operations.fetch_sub(1, Ordering::Relaxed);
        self.activity_epoch.fetch_add(1, Ordering::Relaxed);
    }
}

impl RemoteStdioSessionLease {
    fn new(session: Arc<RemoteStdioRepoSession>) -> Self {
        let operation = session.acquire_operation();
        Self {
            session,
            _operation: operation,
        }
    }
}

impl Deref for RemoteStdioSessionLease {
    type Target = RemoteStdioRepoSession;

    fn deref(&self) -> &Self::Target {
        &self.session
    }
}

impl RemoteStdioDaemonClient {
    async fn spawn(
        provider: Arc<dyn RemoteWorkspaceSearchProvider>,
        connection_id: String,
        binary_path: String,
    ) -> Result<Arc<Self>, String> {
        let command = format!("{} serve --stdio", shell_escape(&binary_path));
        let (protocol, write_rx) = ProtocolClient::channel("remote flashgrep stdio daemon");
        let stdio_protocol = RemoteWorkspaceSearchStdioProtocol::new(protocol.clone());
        provider
            .spawn_stdio_daemon(&connection_id, &command, write_rx, stdio_protocol)
            .await?;

        let client = Arc::new(Self { protocol });
        client.initialize().await?;
        Ok(client)
    }

    async fn initialize(&self) -> Result<(), String> {
        match self
            .protocol
            .send_request_with_timeout(
                Request::Initialize {
                    params: InitializeParams {
                        client_info: Some(ClientInfo {
                            name: CLIENT_NAME.to_string(),
                            version: Some(env!("CARGO_PKG_VERSION").to_string()),
                        }),
                        capabilities: ClientCapabilities::default(),
                    },
                },
                Some(REMOTE_STDIO_REQUEST_TIMEOUT),
            )
            .await
            .map_err(|error| error.to_string())?
        {
            Response::InitializeResult { .. } => {
                self.protocol
                    .send_notification(Request::Initialized)
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(())
            }
            other => Err(format!(
                "Unexpected remote flashgrep initialize response: {other:?}"
            )),
        }
    }

    async fn open_repo(
        self: &Arc<Self>,
        params: OpenRepoParams,
    ) -> Result<RemoteStdioRepoSession, String> {
        match self.send_request(Request::OpenRepo { params }).await? {
            Response::RepoOpened { repo_id, .. } => Ok(RemoteStdioRepoSession {
                repo_id,
                client: self.clone(),
                activity_epoch: Arc::new(AtomicU64::new(1)),
                active_operations: Arc::new(AtomicU64::new(0)),
            }),
            other => Err(format!(
                "Unexpected remote flashgrep open_repo response: {other:?}"
            )),
        }
    }

    async fn send_request(&self, request: Request) -> Result<Response, String> {
        self.protocol
            .send_request_with_timeout(request, Some(REMOTE_STDIO_REQUEST_TIMEOUT))
            .await
            .map_err(|error| error.to_string())
    }

    async fn shutdown(&self) {
        let _ = timeout(
            REMOTE_STDIO_SHUTDOWN_TIMEOUT,
            self.send_request(Request::Shutdown),
        )
        .await;
        self.protocol
            .close_with_message("remote flashgrep stdio daemon is shutting down")
            .await;
    }

    fn is_closed(&self) -> bool {
        self.protocol.is_closed()
    }
}

impl RemoteStdioRepoSession {
    fn acquire_operation(&self) -> RemoteStdioOperationLease {
        self.active_operations.fetch_add(1, Ordering::Relaxed);
        self.activity_epoch.fetch_add(1, Ordering::Relaxed);
        RemoteStdioOperationLease {
            activity_epoch: self.activity_epoch.clone(),
            active_operations: self.active_operations.clone(),
        }
    }

    async fn status(&self) -> Result<RepoStatus, String> {
        let _lease = self.acquire_operation();
        self.status_without_activity_lease().await
    }

    async fn status_without_activity_lease(&self) -> Result<RepoStatus, String> {
        match self
            .client
            .send_request(Request::GetRepoStatus {
                params: self.repo_ref(),
            })
            .await?
        {
            Response::RepoStatus { status } => Ok(status),
            other => Err(format!(
                "Unexpected remote flashgrep get_repo_status response: {other:?}"
            )),
        }
    }

    async fn task_status(&self, task_id: impl Into<String>) -> Result<TaskStatus, String> {
        let _lease = self.acquire_operation();
        match self
            .client
            .send_request(Request::TaskStatus {
                params: TaskRef {
                    task_id: task_id.into(),
                },
            })
            .await?
        {
            Response::TaskStatus { task } => Ok(task),
            other => Err(format!(
                "Unexpected remote flashgrep task/status response: {other:?}"
            )),
        }
    }

    async fn build_index(&self) -> Result<TaskStatus, String> {
        let _lease = self.acquire_operation();
        match self
            .client
            .send_request(Request::BaseSnapshotBuild {
                params: self.repo_ref(),
            })
            .await?
        {
            Response::TaskStarted { task } => Ok(task),
            other => Err(format!(
                "Unexpected remote flashgrep build response: {other:?}"
            )),
        }
    }

    async fn rebuild_index(&self) -> Result<TaskStatus, String> {
        let _lease = self.acquire_operation();
        match self
            .client
            .send_request(Request::BaseSnapshotRebuild {
                params: self.repo_ref(),
            })
            .await?
        {
            Response::TaskStarted { task } => Ok(task),
            other => Err(format!(
                "Unexpected remote flashgrep rebuild response: {other:?}"
            )),
        }
    }

    async fn search(
        &self,
        query: QuerySpec,
        scope: crate::workspace_search::flashgrep::PathScope,
    ) -> Result<(SearchBackend, RepoStatus, SearchResults), String> {
        let _lease = self.acquire_operation();
        match self
            .client
            .send_request(Request::Search {
                params: SearchParams {
                    repo_id: self.repo_id.clone(),
                    query,
                    scope,
                    consistency: ConsistencyMode::WorkspaceEventual,
                    allow_scan_fallback: true,
                },
            })
            .await?
        {
            Response::SearchCompleted {
                backend,
                status,
                results,
                ..
            } => Ok((backend, status, results)),
            other => Err(format!(
                "Unexpected remote flashgrep search response: {other:?}"
            )),
        }
    }

    async fn glob(
        &self,
        scope: crate::workspace_search::flashgrep::PathScope,
    ) -> Result<(RepoStatus, Vec<String>), String> {
        let _lease = self.acquire_operation();
        match self
            .client
            .send_request(Request::Glob {
                params: GlobParams {
                    repo_id: self.repo_id.clone(),
                    scope,
                },
            })
            .await?
        {
            Response::GlobCompleted { status, paths, .. } => Ok((status, paths)),
            other => Err(format!(
                "Unexpected remote flashgrep glob response: {other:?}"
            )),
        }
    }

    async fn close(&self) {
        let _ = self
            .client
            .send_request(Request::CloseRepo {
                params: self.repo_ref(),
            })
            .await;
    }

    fn repo_ref(&self) -> RepoRef {
        RepoRef {
            repo_id: self.repo_id.clone(),
        }
    }
}

#[async_trait]
impl FlashgrepRepoSession for RemoteStdioRepoSession {
    async fn status(&self) -> crate::workspace_search::flashgrep::error::Result<RepoStatus> {
        RemoteStdioRepoSession::status(self)
            .await
            .map_err(AppError::Protocol)
    }

    async fn task_status(
        &self,
        task_id: String,
    ) -> crate::workspace_search::flashgrep::error::Result<TaskStatus> {
        RemoteStdioRepoSession::task_status(self, task_id)
            .await
            .map_err(AppError::Protocol)
    }

    async fn build_index(&self) -> crate::workspace_search::flashgrep::error::Result<TaskStatus> {
        RemoteStdioRepoSession::build_index(self)
            .await
            .map_err(AppError::Protocol)
    }

    async fn rebuild_index(&self) -> crate::workspace_search::flashgrep::error::Result<TaskStatus> {
        RemoteStdioRepoSession::rebuild_index(self)
            .await
            .map_err(AppError::Protocol)
    }

    async fn search(
        &self,
        request: SearchRequest,
    ) -> crate::workspace_search::flashgrep::error::Result<SearchOutcome> {
        let (backend, status, results) =
            RemoteStdioRepoSession::search(self, request.query, request.scope)
                .await
                .map_err(AppError::Protocol)?;
        Ok(SearchOutcome {
            backend,
            status,
            results,
        })
    }

    async fn glob(
        &self,
        request: GlobRequest,
    ) -> crate::workspace_search::flashgrep::error::Result<GlobOutcome> {
        let (status, paths) = RemoteStdioRepoSession::glob(self, request.scope)
            .await
            .map_err(AppError::Protocol)?;
        Ok(GlobOutcome { status, paths })
    }

    async fn close(&self) -> crate::workspace_search::flashgrep::error::Result<()> {
        RemoteStdioRepoSession::close(self).await;
        Ok(())
    }
}

#[derive(Clone)]
pub struct RemoteWorkspaceSearchService {
    provider: Arc<dyn RemoteWorkspaceSearchProvider>,
    preferred_connection_id: Option<String>,
}

#[derive(Debug, Clone)]
struct RemoteSearchContext {
    connection: RemoteWorkspaceEntry,
    binary_path: String,
    repo_root: String,
    storage_root: String,
    remote_arch: String,
    local_binary_sha256: String,
}

impl RemoteWorkspaceSearchService {
    pub fn new(provider: Arc<dyn RemoteWorkspaceSearchProvider>) -> Self {
        Self {
            provider,
            preferred_connection_id: None,
        }
    }

    pub fn with_preferred_connection_id(mut self, preferred_connection_id: Option<String>) -> Self {
        self.preferred_connection_id = preferred_connection_id;
        self
    }

    pub async fn get_index_status(&self, root_path: &str) -> Result<WorkspaceIndexStatus, String> {
        let session = self.get_or_open_stdio_session(root_path).await?;
        let repo_status: WorkspaceSearchRepoStatus = session.status().await?.into();
        let active_task = match repo_status.active_task_id.clone() {
            Some(task_id) => match session.task_status(task_id).await {
                Ok(task) => Some(task.into()),
                Err(error) => {
                    log::warn!(
                        target: FLASHGREP_LOG_TARGET,
                        "Failed to fetch active remote flashgrep task status: {}",
                        error
                    );
                    None
                }
            },
            None => None,
        };
        Ok(WorkspaceIndexStatus {
            active_task,
            repo_status,
        })
    }

    pub async fn build_index(&self, root_path: &str) -> Result<IndexTaskHandle, String> {
        let session = self.get_or_open_stdio_session(root_path).await?;
        let task = session.build_index().await?;
        let repo_status = session.status().await?;
        Ok(IndexTaskHandle {
            task: task.into(),
            repo_status: repo_status.into(),
        })
    }

    pub async fn rebuild_index(&self, root_path: &str) -> Result<IndexTaskHandle, String> {
        let session = self.get_or_open_stdio_session(root_path).await?;
        let task = session.rebuild_index().await?;
        let repo_status = session.status().await?;
        Ok(IndexTaskHandle {
            task: task.into(),
            repo_status: repo_status.into(),
        })
    }

    pub async fn search_content(
        &self,
        request: ContentSearchRequest,
    ) -> Result<ContentSearchResult, String> {
        let repo_root = normalize_remote_workspace_path(&request.repo_root.to_string_lossy());
        let session = self.get_or_open_stdio_session(&repo_root).await?;
        let scope = build_remote_scope(
            &repo_root,
            request.search_path.as_deref(),
            request.globs,
            request.file_types,
            request.exclude_file_types,
        )?;
        let max_results = request.max_results.filter(|limit| *limit > 0);
        let primary_search_mode = remote_stdio_search_mode(request.output_mode);
        let query = QuerySpec {
            pattern: request.pattern.clone(),
            patterns: Vec::new(),
            case_insensitive: !request.case_sensitive,
            multiline: request.multiline,
            dot_matches_new_line: request.multiline,
            fixed_strings: !request.use_regex,
            word_regexp: request.whole_word,
            line_regexp: false,
            before_context: request.before_context,
            after_context: request.after_context,
            top_k_tokens: 6,
            max_count: None,
            global_max_results: max_results,
            search_mode: primary_search_mode,
        };

        let output_mode = request.output_mode;
        let (backend, repo_status, mut raw_results) = session.search(query, scope.clone()).await?;
        if should_retry_remote_scan_fallback_as_files_with_matches(
            backend,
            primary_search_mode,
            &raw_results,
        ) {
            log::info!(
                "Remote workspace content search re-issuing as FilesWithMatches because daemon ScanFallback returned only summary statistics: pattern_chars={}, primary_search_mode={:?}, primary_matched_lines={}, primary_matched_occurrences={}",
                request.pattern.chars().count(),
                primary_search_mode,
                raw_results.matched_lines,
                raw_results.matched_occurrences,
            );
            let fallback_query = QuerySpec {
                pattern: request.pattern.clone(),
                patterns: Vec::new(),
                case_insensitive: !request.case_sensitive,
                multiline: request.multiline,
                dot_matches_new_line: request.multiline,
                fixed_strings: !request.use_regex,
                word_regexp: request.whole_word,
                line_regexp: false,
                before_context: request.before_context,
                after_context: request.after_context,
                top_k_tokens: 6,
                max_count: None,
                global_max_results: max_results,
                search_mode: SearchModeConfig::FilesWithMatches,
            };
            match session.search(fallback_query, scope).await {
                Ok((_, _, fallback_results)) => {
                    log::info!(
                        "Remote workspace content search FilesWithMatches fallback succeeded: matched_paths={}, matched_lines={}, matched_occurrences={}",
                        fallback_results.matched_paths.len(),
                        fallback_results.matched_lines,
                        fallback_results.matched_occurrences,
                    );
                    raw_results = fallback_results;
                }
                Err(error) => {
                    log::warn!(
                        "Remote workspace content search FilesWithMatches fallback failed: pattern_chars={}, primary_matched_lines={}, primary_matched_occurrences={}, error={}",
                        request.pattern.chars().count(),
                        raw_results.matched_lines,
                        raw_results.matched_occurrences,
                        error,
                    );
                    return Err(format!(
                        "Remote workspace search returned only summary statistics for {primary_matched_lines} line(s) and the file-list fallback failed: {error}",
                        primary_matched_lines = raw_results.matched_lines,
                    ));
                }
            }
        }

        let mut results = convert_search_results(&raw_results, output_mode);
        log::debug!(
            "Remote workspace content search converted: backend={:?}, repo_phase={:?}, hits={}, file_counts={}, file_match_counts={}, matched_paths={}, converted_results={}, matched_lines={}, matched_occurrences={}",
            backend,
            repo_status.phase,
            raw_results.hits.len(),
            raw_results.file_counts.len(),
            raw_results.file_match_counts.len(),
            raw_results.matched_paths.len(),
            results.len(),
            raw_results.matched_lines,
            raw_results.matched_occurrences
        );
        let truncated = max_results
            .map(|limit| results.len() >= limit)
            .unwrap_or(false);
        if let Some(limit) = max_results {
            results.truncate(limit);
        }

        Ok(ContentSearchResult {
            outcome: FileSearchOutcome { results, truncated },
            file_counts: raw_results
                .file_counts
                .clone()
                .into_iter()
                .map(WorkspaceSearchFileCount::from)
                .collect(),
            hits: raw_results
                .hits
                .clone()
                .into_iter()
                .map(WorkspaceSearchHit::from)
                .collect(),
            backend: backend.into(),
            repo_status: repo_status.into(),
            candidate_docs: raw_results.candidate_docs,
            matched_lines: raw_results.matched_lines,
            matched_occurrences: raw_results.matched_occurrences,
        })
    }

    pub async fn glob(&self, request: GlobSearchRequest) -> Result<GlobSearchResult, String> {
        let repo_root = normalize_remote_workspace_path(&request.repo_root.to_string_lossy());
        let session = self.get_or_open_stdio_session(&repo_root).await?;
        let scope = build_remote_scope(
            &repo_root,
            request.search_path.as_deref(),
            vec![request.pattern],
            Vec::new(),
            Vec::new(),
        )?;
        let (repo_status, mut paths) = session.glob(scope).await?;

        paths.sort();
        if request.limit > 0 {
            paths.truncate(request.limit);
        } else {
            paths.clear();
        }

        Ok(GlobSearchResult {
            paths,
            repo_status: repo_status.into(),
        })
    }

    pub async fn resolve_remote_workspace_entry(
        &self,
        root_path: &str,
    ) -> Result<RemoteWorkspaceEntry, String> {
        self.provider
            .resolve_workspace_entry(root_path, self.preferred_connection_id.as_deref())
            .await
    }

    async fn get_or_open_stdio_session(
        &self,
        root_path: &str,
    ) -> Result<RemoteStdioSessionLease, String> {
        let context = self.ensure_remote_search_context(root_path).await?;
        let key = remote_stdio_session_key(&context.connection.connection_id, &context.repo_root);

        if let Some(entry) = REMOTE_STDIO_SESSIONS.read().await.get(&key).cloned() {
            entry.activity_epoch.fetch_add(1, Ordering::Relaxed);
            if !entry.session.client.is_closed() {
                return Ok(RemoteStdioSessionLease::new(entry.session.clone()));
            }
            log::warn!(
                target: FLASHGREP_LOG_TARGET,
                "Remote workspace search stdio session became unhealthy, reopening: connection_id={}, path={}",
                context.connection.connection_id,
                context.repo_root
            );
            REMOTE_STDIO_SESSIONS.write().await.remove(&key);
            entry.session.close().await;
            entry.session.client.shutdown().await;
        }

        let guard = {
            let mut guards = REMOTE_STDIO_OPEN_GUARDS.lock().await;
            guards
                .entry(key.clone())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _open_guard = guard.lock().await;

        if let Some(entry) = REMOTE_STDIO_SESSIONS.read().await.get(&key).cloned() {
            entry.activity_epoch.fetch_add(1, Ordering::Relaxed);
            return Ok(RemoteStdioSessionLease::new(entry.session));
        }

        let open_result = async {
            let client = RemoteStdioDaemonClient::spawn(
                self.provider.clone(),
                context.connection.connection_id.clone(),
                context.binary_path.clone(),
            )
            .await?;
            let mut repo_config = RepoConfig::default();
            repo_config.max_file_size = self.provider.repo_max_file_size().await;
            let session = match client
                .open_repo(OpenRepoParams {
                    repo_path: PathBuf::from(&context.repo_root),
                    storage_root: Some(PathBuf::from(&context.storage_root)),
                    config: repo_config,
                    refresh: RefreshPolicyConfig::default(),
                })
                .await
            {
                Ok(session) => session,
                Err(error) => {
                    client.shutdown().await;
                    return Err(error);
                }
            };
            let activity_epoch = session.activity_epoch.clone();
            Ok::<_, String>((Arc::new(session), activity_epoch))
        }
        .await;
        let (session, activity_epoch) = match open_result {
            Ok(opened) => opened,
            Err(error) => {
                if Arc::strong_count(&guard) <= 2 {
                    REMOTE_STDIO_OPEN_GUARDS.lock().await.remove(&key);
                }
                return Err(error);
            }
        };
        REMOTE_STDIO_SESSIONS.write().await.insert(
            key.clone(),
            RemoteStdioSessionEntry {
                session: session.clone(),
                activity_epoch: activity_epoch.clone(),
            },
        );
        schedule_remote_stdio_session_release(key, activity_epoch);
        Ok(RemoteStdioSessionLease::new(session))
    }

    async fn ensure_remote_search_context(
        &self,
        root_path: &str,
    ) -> Result<RemoteSearchContext, String> {
        let repo_root = normalize_remote_workspace_path(root_path);
        let connection = self.resolve_remote_workspace_entry(&repo_root).await?;
        let cache_key = remote_search_context_key(&connection.connection_id, &repo_root);
        if let Some(context) = REMOTE_SEARCH_CONTEXTS.read().await.get(&cache_key).cloned() {
            let local_bundle = local_flashgrep_bundle_for_arch(&context.remote_arch).await?;
            if local_bundle.sha256 == context.local_binary_sha256 {
                return Ok(context);
            }

            log::info!(
                target: FLASHGREP_LOG_TARGET,
                "Bundled remote flashgrep binary changed; reopening remote search session: connection_id={}, path={}, old_sha256={}, new_sha256={}",
                context.connection.connection_id,
                context.repo_root,
                context.local_binary_sha256,
                local_bundle.sha256
            );
            REMOTE_SEARCH_CONTEXTS.write().await.remove(&cache_key);
            let session_key =
                remote_stdio_session_key(&context.connection.connection_id, &context.repo_root);
            if let Some(entry) = REMOTE_STDIO_SESSIONS.write().await.remove(&session_key) {
                entry.session.close().await;
                entry.session.client.shutdown().await;
            }
        }

        let cached_server_os_type = self
            .provider
            .cached_server_os_type(&connection.connection_id)
            .await;
        let remote_os = if let Some(os_type) = cached_server_os_type {
            if os_type.eq_ignore_ascii_case("unknown") {
                self.detect_remote_os_type(&connection.connection_id)
                    .await
                    .unwrap_or(os_type)
            } else {
                os_type
            }
        } else {
            self.detect_remote_os_type(&connection.connection_id)
                .await
                .unwrap_or_else(|| "unknown".to_string())
        };
        let inferred_linux = remote_os.eq_ignore_ascii_case("unknown")
            && looks_like_linux_workspace_root(&repo_root);
        if !remote_os.eq_ignore_ascii_case("linux") && !inferred_linux {
            return Err(format!(
                "Remote workspace search currently supports Linux only, but server OS is {}",
                remote_os
            ));
        }

        let remote_arch = self
            .detect_remote_architecture(&connection.connection_id)
            .await?;
        let local_bundle = local_flashgrep_bundle_for_arch(&remote_arch).await?;
        let binary_path = self
            .ensure_remote_flashgrep_binary(&connection.connection_id, &repo_root, &local_bundle)
            .await?;
        let storage_root = remote_workspace_search_storage_root(&repo_root);

        let context = RemoteSearchContext {
            connection,
            binary_path,
            repo_root,
            storage_root,
            remote_arch,
            local_binary_sha256: local_bundle.sha256,
        };
        REMOTE_SEARCH_CONTEXTS
            .write()
            .await
            .insert(cache_key, context.clone());
        Ok(context)
    }

    async fn detect_remote_architecture(&self, connection_id: &str) -> Result<String, String> {
        let mut attempts = Vec::new();

        for probe in REMOTE_ARCHITECTURE_PROBES {
            match self.provider.execute_command(connection_id, probe).await {
                Ok(output) => {
                    if let Some(arch) =
                        parse_remote_architecture_output(&output.stdout, &output.stderr)
                    {
                        return Ok(arch);
                    }
                    attempts.push(format!(
                        "probe=`{probe}` exit_code={} stdout={:?} stderr={:?}",
                        output.exit_code,
                        output.stdout.trim(),
                        output.stderr.trim()
                    ));
                }
                Err(error) => {
                    attempts.push(format!("probe=`{probe}` error={error}"));
                }
            }
        }

        Err(format!(
            "Failed to detect remote architecture from SSH output. Attempts: {}",
            attempts.join("; ")
        ))
    }

    async fn detect_remote_os_type(&self, connection_id: &str) -> Option<String> {
        for probe in REMOTE_OS_PROBES {
            let Ok(output) = self.provider.execute_command(connection_id, probe).await else {
                continue;
            };
            if let Some(os_type) = parse_remote_os_output(&output.stdout, &output.stderr) {
                return Some(os_type);
            }
        }
        None
    }

    async fn ensure_remote_flashgrep_binary(
        &self,
        connection_id: &str,
        repo_root: &str,
        local_bundle: &LocalFlashgrepBundle,
    ) -> Result<String, String> {
        let install_dir = remote_flashgrep_install_dir(repo_root);
        let remote_binary_path = join_remote_path(&install_dir, &local_bundle.binary_name);

        self.provider
            .create_dir_all(connection_id, &install_dir)
            .await
            .map_err(|error| {
                format!("Failed to create remote flashgrep install directory: {error}")
            })?;
        let remote_sha256 = self
            .remote_flashgrep_sha256(connection_id, &remote_binary_path)
            .await?;
        if remote_sha256.as_deref() != Some(local_bundle.sha256.as_str()) {
            log::info!(
                target: FLASHGREP_LOG_TARGET,
                "Uploading bundled remote flashgrep binary: connection_id={}, path={}, bundle={}, local_path={}, local_sha256={}, remote_sha256={}",
                connection_id,
                remote_binary_path,
                local_bundle.binary_name,
                local_bundle.path.display(),
                local_bundle.sha256,
                remote_sha256.as_deref().unwrap_or("missing")
            );
            let temp_remote_binary_path =
                format!("{}.upload-{}.tmp", remote_binary_path, local_bundle.sha256);
            self.provider
                .write_file(connection_id, &temp_remote_binary_path, &local_bundle.bytes)
                .await
                .map_err(|error| format!("Failed to upload flashgrep to remote host: {error}"))?;
            self.provider
                .execute_command(
                    connection_id,
                    &format!(
                        "mv -f {} {}",
                        shell_escape(&temp_remote_binary_path),
                        shell_escape(&remote_binary_path)
                    ),
                )
                .await
                .map_err(|error| {
                    format!("Failed to install uploaded flashgrep on remote host: {error}")
                })?;
        }
        self.provider
            .execute_command(
                connection_id,
                &format!("chmod 755 {}", shell_escape(&remote_binary_path)),
            )
            .await
            .map_err(|error| format!("Failed to mark remote flashgrep as executable: {error}"))?;

        Ok(remote_binary_path)
    }

    async fn remote_flashgrep_sha256(
        &self,
        connection_id: &str,
        remote_binary_path: &str,
    ) -> Result<Option<String>, String> {
        let escaped_path = shell_escape(remote_binary_path);
        let command = format!(
            "if [ -f {path} ]; then if command -v sha256sum >/dev/null 2>&1; then sha256sum {path} | awk '{{print $1}}'; elif command -v shasum >/dev/null 2>&1; then shasum -a 256 {path} | awk '{{print $1}}'; fi; fi",
            path = escaped_path
        );
        let output = self
            .provider
            .execute_command(connection_id, &command)
            .await
            .map_err(|error| format!("Failed to hash remote flashgrep binary: {error}"))?;
        if output.exit_code != 0 {
            return Ok(None);
        }
        let hash = output.stdout.trim();
        if hash.len() == 64 && hash.chars().all(|character| character.is_ascii_hexdigit()) {
            Ok(Some(hash.to_ascii_lowercase()))
        } else {
            Ok(None)
        }
    }
}

fn remote_stdio_session_key(connection_id: &str, repo_root: &str) -> String {
    format!(
        "{connection_id}\0{}",
        normalize_remote_workspace_path(repo_root)
    )
}

fn remote_search_context_key(connection_id: &str, repo_root: &str) -> String {
    format!(
        "{connection_id}\0{}",
        normalize_remote_workspace_path(repo_root)
    )
}

fn schedule_remote_stdio_session_release(key: String, activity_epoch: Arc<AtomicU64>) {
    tokio::spawn(async move {
        let expected_epoch = activity_epoch.load(Ordering::Relaxed);
        sleep(REMOTE_STDIO_SESSION_IDLE_GRACE).await;
        let entry = {
            let sessions = REMOTE_STDIO_SESSIONS.read().await;
            let Some(entry) = sessions.get(&key) else {
                return;
            };
            if entry.session.active_operations.load(Ordering::Relaxed) > 0 {
                schedule_remote_stdio_session_release(key.clone(), entry.activity_epoch.clone());
                return;
            }
            if entry.activity_epoch.load(Ordering::Relaxed) != expected_epoch {
                schedule_remote_stdio_session_release(key.clone(), entry.activity_epoch.clone());
                return;
            }
            entry.clone()
        };

        match entry.session.status_without_activity_lease().await {
            Ok(status) if status.active_task_id.is_some() => {
                schedule_remote_stdio_session_release(key.clone(), entry.activity_epoch.clone());
                return;
            }
            Ok(_) => {}
            Err(error) => {
                log::warn!(
                    target: FLASHGREP_LOG_TARGET,
                    "Failed to check idle remote workspace search status before release: key={}, error={}",
                    key.replace('\0', ":"),
                    error
                );
            }
        }

        let entry = {
            let mut sessions = REMOTE_STDIO_SESSIONS.write().await;
            let Some(current_entry) = sessions.get(&key) else {
                return;
            };
            if !Arc::ptr_eq(&current_entry.session, &entry.session) {
                return;
            }
            if current_entry
                .session
                .active_operations
                .load(Ordering::Relaxed)
                > 0
            {
                schedule_remote_stdio_session_release(
                    key.clone(),
                    current_entry.activity_epoch.clone(),
                );
                return;
            }
            if current_entry.activity_epoch.load(Ordering::Relaxed) != expected_epoch {
                schedule_remote_stdio_session_release(
                    key.clone(),
                    current_entry.activity_epoch.clone(),
                );
                return;
            }
            sessions.remove(&key)
        };

        if let Some(entry) = entry {
            log::debug!(
                target: FLASHGREP_LOG_TARGET,
                "Releasing idle remote workspace search stdio session: key={}",
                key.replace('\0', ":")
            );
            entry.session.close().await;
            entry.session.client.shutdown().await;
            REMOTE_STDIO_OPEN_GUARDS.lock().await.remove(&key);
        }
    });
}

#[cfg(test)]
pub(crate) fn test_remote_stdio_session_key(connection_id: &str, repo_root: &str) -> String {
    remote_stdio_session_key(connection_id, repo_root)
}

#[cfg(test)]
pub(crate) fn test_remote_search_context_key(connection_id: &str, repo_root: &str) -> String {
    remote_search_context_key(connection_id, repo_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static REMOTE_SEARCH_TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    async fn clear_remote_search_test_state() {
        REMOTE_STDIO_SESSIONS.write().await.clear();
        REMOTE_STDIO_OPEN_GUARDS.lock().await.clear();
        REMOTE_SEARCH_CONTEXTS.write().await.clear();
    }

    #[test]
    fn remote_search_cache_keys_normalize_workspace_root() {
        assert_eq!(
            test_remote_stdio_session_key("conn-1", "/home/user/repo/"),
            "conn-1\0/home/user/repo"
        );
        assert_eq!(
            test_remote_search_context_key("conn-1", "/home/user/repo/"),
            "conn-1\0/home/user/repo"
        );
    }

    #[tokio::test]
    async fn remote_search_rejects_non_linux_before_stdio_open() {
        let _test_guard = REMOTE_SEARCH_TEST_LOCK.lock().await;
        clear_remote_search_test_state().await;
        let provider = Arc::new(FakeRemoteSearchProvider {
            cached_os_type: Some("Darwin".to_string()),
            connection_id: "conn-1".to_string(),
            remote_root: "/Users/example/project".to_string(),
            fail_stdio_spawn: false,
            resolve_count: AtomicU64::new(0),
            stdio_spawn_count: AtomicU64::new(0),
        });
        let service = RemoteWorkspaceSearchService::new(provider.clone());

        let error = service
            .get_index_status("/Users/example/project")
            .await
            .expect_err("non-linux remotes must fail before opening flashgrep");

        assert!(error.contains("supports Linux only"));
        assert!(error.contains("Darwin"));
        assert_eq!(provider.stdio_spawn_count.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn remote_search_context_ignores_stale_cache_before_resolving_connection() {
        let _test_guard = REMOTE_SEARCH_TEST_LOCK.lock().await;
        clear_remote_search_test_state().await;
        let repo_root = "/home/user/repo";
        let stale_empty_connection_key = format!("\0{repo_root}");
        REMOTE_SEARCH_CONTEXTS.write().await.insert(
            stale_empty_connection_key,
            RemoteSearchContext {
                connection: RemoteWorkspaceEntry {
                    connection_id: "conn-stale".to_string(),
                    connection_name: "stale".to_string(),
                    ssh_host: "stale.example.test".to_string(),
                    remote_root: repo_root.to_string(),
                },
                binary_path: "/stale/flashgrep".to_string(),
                repo_root: repo_root.to_string(),
                storage_root: "/stale/search".to_string(),
                remote_arch: "riscv64".to_string(),
                local_binary_sha256: "stale".to_string(),
            },
        );
        let provider = Arc::new(FakeRemoteSearchProvider {
            cached_os_type: Some("Darwin".to_string()),
            connection_id: "conn-new".to_string(),
            remote_root: repo_root.to_string(),
            fail_stdio_spawn: false,
            resolve_count: AtomicU64::new(0),
            stdio_spawn_count: AtomicU64::new(0),
        });
        let service = RemoteWorkspaceSearchService::new(provider.clone());

        let error = service
            .get_index_status(repo_root)
            .await
            .expect_err("resolved non-Linux connection should reject without using stale cache");

        assert_eq!(provider.resolve_count.load(Ordering::Relaxed), 1);
        assert!(error.contains("Darwin"));
        assert!(!error.contains("riscv64"));
    }

    #[tokio::test]
    async fn remote_search_open_guard_is_removed_when_stdio_spawn_fails() {
        let _test_guard = REMOTE_SEARCH_TEST_LOCK.lock().await;
        clear_remote_search_test_state().await;
        let repo_root = "/home/user/repo";
        let provider = Arc::new(FakeRemoteSearchProvider {
            cached_os_type: Some("Linux".to_string()),
            connection_id: "conn-guard".to_string(),
            remote_root: repo_root.to_string(),
            fail_stdio_spawn: true,
            resolve_count: AtomicU64::new(0),
            stdio_spawn_count: AtomicU64::new(0),
        });
        let service = RemoteWorkspaceSearchService::new(provider.clone());

        let error = service
            .get_index_status(repo_root)
            .await
            .expect_err("fake provider rejects stdio spawn");

        assert!(error.contains("spawn failed"));
        assert_eq!(provider.stdio_spawn_count.load(Ordering::Relaxed), 1);
        let key = remote_stdio_session_key("conn-guard", repo_root);
        assert!(
            !REMOTE_STDIO_OPEN_GUARDS.lock().await.contains_key(&key),
            "failed stdio opens must not leave a global guard entry behind"
        );
    }

    struct FakeRemoteSearchProvider {
        cached_os_type: Option<String>,
        connection_id: String,
        remote_root: String,
        fail_stdio_spawn: bool,
        resolve_count: AtomicU64,
        stdio_spawn_count: AtomicU64,
    }

    #[async_trait]
    impl RemoteWorkspaceSearchProvider for FakeRemoteSearchProvider {
        async fn resolve_workspace_entry(
            &self,
            _root_path: &str,
            _preferred_connection_id: Option<&str>,
        ) -> Result<RemoteWorkspaceEntry, String> {
            self.resolve_count.fetch_add(1, Ordering::Relaxed);
            Ok(RemoteWorkspaceEntry {
                connection_id: self.connection_id.clone(),
                connection_name: "test".to_string(),
                ssh_host: "example.test".to_string(),
                remote_root: self.remote_root.clone(),
            })
        }

        async fn cached_server_os_type(&self, _connection_id: &str) -> Option<String> {
            self.cached_os_type.clone()
        }

        async fn execute_command(
            &self,
            _connection_id: &str,
            command: &str,
        ) -> Result<RemoteCommandOutput, String> {
            if command == "uname -m" || command == "arch" || command.contains("uname -m") {
                return Ok(RemoteCommandOutput {
                    stdout: "x86_64\n".to_string(),
                    stderr: String::new(),
                    exit_code: 0,
                });
            }
            if command.contains("sha256sum")
                || command.starts_with("mv -f ")
                || command.starts_with("chmod 755 ")
            {
                return Ok(RemoteCommandOutput {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 0,
                });
            }
            Err(format!("unexpected remote command: {command}"))
        }

        async fn create_dir_all(&self, _connection_id: &str, _path: &str) -> Result<(), String> {
            Ok(())
        }

        async fn write_file(
            &self,
            _connection_id: &str,
            _path: &str,
            _contents: &[u8],
        ) -> Result<(), String> {
            Ok(())
        }

        async fn repo_max_file_size(&self) -> u64 {
            0
        }

        async fn spawn_stdio_daemon(
            &self,
            _connection_id: &str,
            _command: &str,
            _write_rx: mpsc::Receiver<Vec<u8>>,
            _protocol: RemoteWorkspaceSearchStdioProtocol,
        ) -> Result<(), String> {
            self.stdio_spawn_count.fetch_add(1, Ordering::Relaxed);
            if self.fail_stdio_spawn {
                Err("spawn failed".to_string())
            } else {
                Err("unexpected stdio spawn".to_string())
            }
        }
    }
}
