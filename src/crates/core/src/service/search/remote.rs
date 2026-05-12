use crate::infrastructure::{FileSearchOutcome, FileSearchResult, SearchMatchType};
use crate::service::config::{get_global_config_service, types::WorkspaceConfig, ConfigService};
use crate::service::remote_ssh::workspace_state::{
    get_remote_workspace_manager, lookup_remote_connection, lookup_remote_connection_with_hint,
    RemoteWorkspaceEntry,
};
use crate::service::remote_ssh::{
    normalize_remote_workspace_path, RemoteFileService, SSHConnectionManager,
};
use crate::service::search::flashgrep::{
    drain_content_length_messages, ClientCapabilities, ClientInfo, ConsistencyMode, GlobOutcome,
    GlobParams, GlobRequest, InitializeParams, OpenRepoParams, PathScope, ProtocolClient,
    QuerySpec, RefreshPolicyConfig, RepoConfig, RepoRef, RepoStatus, Request, Response,
    SearchBackend, SearchModeConfig, SearchOutcome, SearchParams, SearchRequest, SearchResults,
    TaskRef, TaskStatus,
};
use crate::service::search::flashgrep::{error::AppError, FlashgrepRepoSession};
use crate::service::search::{
    ContentSearchOutputMode, ContentSearchRequest, ContentSearchResult, GlobSearchRequest,
    GlobSearchResult, IndexTaskHandle, WorkspaceIndexStatus, WorkspaceSearchFileCount,
    WorkspaceSearchHit, WorkspaceSearchRepoStatus,
};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, LazyLock,
};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{sleep, timeout};

const REMOTE_FLASHGREP_INSTALL_DIR: &str = ".bitfun/bin";
const REMOTE_STDIO_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const REMOTE_STDIO_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const REMOTE_STDIO_SESSION_IDLE_GRACE: Duration = Duration::from_secs(45);
const CLIENT_NAME: &str = "bitfun-remote-workspace-search";
const REMOTE_OS_PROBES: &[&str] = &["uname -s", "sh -c 'uname -s 2>/dev/null'"];
const REMOTE_ARCHITECTURE_PROBES: &[&str] = &[
    "uname -m",
    "arch",
    "sh -c 'uname -m 2>/dev/null || arch 2>/dev/null'",
];
const LINUX_X86_64_FLASHGREP_BUNDLES: &[&str] = &[
    "flashgrep-x86_64-unknown-linux-musl",
    "flashgrep-x86_64-unknown-linux-gnu",
];
const LINUX_AARCH64_FLASHGREP_BUNDLES: &[&str] = &[
    "flashgrep-aarch64-unknown-linux-musl",
    "flashgrep-aarch64-unknown-linux-gnu",
];

static REMOTE_STDIO_SESSIONS: LazyLock<RwLock<HashMap<String, RemoteStdioSessionEntry>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static REMOTE_STDIO_OPEN_GUARDS: LazyLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static REMOTE_SEARCH_CONTEXTS: LazyLock<RwLock<HashMap<String, RemoteSearchContext>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

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
        ssh_manager: SSHConnectionManager,
        connection_id: String,
        binary_path: String,
    ) -> Result<Arc<Self>, String> {
        let command = format!("{} serve --stdio", shell_escape(&binary_path));
        let channel = ssh_manager
            .open_exec_channel(&connection_id, &command)
            .await
            .map_err(|error| format!("Failed to start remote flashgrep stdio daemon: {error}"))?;

        let (protocol, write_rx) = ProtocolClient::channel("remote flashgrep stdio daemon");
        spawn_remote_stdio_owner(connection_id, channel, write_rx, protocol.clone());

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
        scope: PathScope,
    ) -> Result<
        (
            crate::service::search::flashgrep::SearchBackend,
            RepoStatus,
            SearchResults,
        ),
        String,
    > {
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

    async fn glob(&self, scope: PathScope) -> Result<(RepoStatus, Vec<String>), String> {
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
    async fn status(&self) -> crate::service::search::flashgrep::error::Result<RepoStatus> {
        RemoteStdioRepoSession::status(self)
            .await
            .map_err(AppError::Protocol)
    }

    async fn task_status(
        &self,
        task_id: String,
    ) -> crate::service::search::flashgrep::error::Result<TaskStatus> {
        RemoteStdioRepoSession::task_status(self, task_id)
            .await
            .map_err(AppError::Protocol)
    }

    async fn build_index(&self) -> crate::service::search::flashgrep::error::Result<TaskStatus> {
        RemoteStdioRepoSession::build_index(self)
            .await
            .map_err(AppError::Protocol)
    }

    async fn rebuild_index(&self) -> crate::service::search::flashgrep::error::Result<TaskStatus> {
        RemoteStdioRepoSession::rebuild_index(self)
            .await
            .map_err(AppError::Protocol)
    }

    async fn search(
        &self,
        request: SearchRequest,
    ) -> crate::service::search::flashgrep::error::Result<SearchOutcome> {
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
    ) -> crate::service::search::flashgrep::error::Result<GlobOutcome> {
        let (status, paths) = RemoteStdioRepoSession::glob(self, request.scope)
            .await
            .map_err(AppError::Protocol)?;
        Ok(GlobOutcome { status, paths })
    }

    async fn close(&self) -> crate::service::search::flashgrep::error::Result<()> {
        RemoteStdioRepoSession::close(self).await;
        Ok(())
    }
}

fn spawn_remote_stdio_owner(
    connection_id: String,
    mut channel: russh::Channel<russh::client::Msg>,
    mut write_rx: mpsc::Receiver<Vec<u8>>,
    protocol: ProtocolClient,
) {
    tokio::spawn(async move {
        let mut writer = channel.make_writer();
        let mut read_buffer = Vec::<u8>::new();

        loop {
            tokio::select! {
                outbound = write_rx.recv() => {
                    let Some(outbound) = outbound else {
                        let _ = channel.eof().await;
                        let _ = channel.close().await;
                        break;
                    };
                    if let Err(error) = writer.write_all(&outbound).await {
                        log::warn!(
                            "Failed to write remote flashgrep stdio request: connection_id={}, error={}",
                            connection_id,
                            error
                        );
                        protocol
                            .close_with_message("remote flashgrep stdio daemon write failed")
                            .await;
                        break;
                    }
                    if let Err(error) = writer.flush().await {
                        log::warn!(
                            "Failed to flush remote flashgrep stdio request: connection_id={}, error={}",
                            connection_id,
                            error
                        );
                        protocol
                            .close_with_message("remote flashgrep stdio daemon flush failed")
                            .await;
                        break;
                    }
                }

                message = channel.wait() => {
                    match message {
                        Some(russh::ChannelMsg::Data { data }) => {
                            read_buffer.extend_from_slice(&data);
                            match drain_content_length_messages(&mut read_buffer) {
                                Ok(messages) => {
                                    for message in messages {
                                        protocol.handle_server_message(message).await;
                                    }
                                }
                                Err(error) => {
                                    log::warn!(
                                        "Failed to decode remote flashgrep stdio message: connection_id={}, error={}",
                                        connection_id,
                                        error
                                    );
                                    protocol
                                        .close_with_message(format!(
                                            "remote flashgrep stdio daemon decode failed: {error}"
                                        ))
                                        .await;
                                    break;
                                }
                            }
                        }
                        Some(russh::ChannelMsg::ExtendedData { data, .. }) => {
                            let text = String::from_utf8_lossy(&data);
                            for line in text.lines() {
                                log::debug!(
                                    "remote flashgrep stdio daemon stderr: connection_id={}, line={}",
                                    connection_id,
                                    line
                                );
                            }
                        }
                        Some(russh::ChannelMsg::ExitStatus { exit_status }) => {
                            log::debug!(
                                "Remote flashgrep stdio daemon exited: connection_id={}, exit_status={}",
                                connection_id,
                                exit_status
                            );
                            break;
                        }
                        Some(russh::ChannelMsg::Eof) | Some(russh::ChannelMsg::Close) | None => {
                            break;
                        }
                        Some(_) => {}
                    }
                }
            }
        }

        protocol
            .close_with_message("remote flashgrep stdio daemon closed before sending a response")
            .await;
    });
}

#[derive(Clone)]
pub struct RemoteWorkspaceSearchService {
    ssh_manager: SSHConnectionManager,
    remote_file_service: RemoteFileService,
    config_service: Arc<ConfigService>,
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

struct LocalFlashgrepBundle {
    binary_name: String,
    path: PathBuf,
    bytes: Vec<u8>,
    sha256: String,
}

impl RemoteWorkspaceSearchService {
    pub fn new(
        ssh_manager: SSHConnectionManager,
        remote_file_service: RemoteFileService,
        config_service: Arc<ConfigService>,
    ) -> Self {
        Self {
            ssh_manager,
            remote_file_service,
            config_service,
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
        // The bundled flashgrep daemon (v0.2.6) only emits summary statistics
        // (`matched_lines`/`matched_occurrences`) when it falls back to the
        // file-system scanner because the workspace has not been indexed yet.
        // In that mode `LineMatches` returns no `hits`, no `matched_paths`,
        // and no `file_counts`, leaving the UI showing "no results" even
        // though the daemon reports thousands of matches. Re-issue the same
        // query as `FilesWithMatches`, which the daemon does populate with
        // the matching file paths, so the user at least sees the hit list
        // while the index is being built.
        let primary_has_details = !raw_results.hits.is_empty()
            || !raw_results.file_counts.is_empty()
            || !raw_results.file_match_counts.is_empty()
            || !raw_results.matched_paths.is_empty();
        if matches!(backend, SearchBackend::ScanFallback)
            && !primary_has_details
            && raw_results.matched_lines > 0
            && !matches!(primary_search_mode, SearchModeConfig::FilesWithMatches)
        {
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
                    // Surface the failure instead of silently keeping the
                    // summary-only `raw_results` from the primary LineMatches
                    // call. Otherwise the converter produces an empty result
                    // list while `matched_lines > 0`, recreating the original
                    // "found N lines but no results" UI inconsistency.
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

        let mut results = convert_stdio_search_results(&raw_results, output_mode);
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
        let _guard = guard.lock().await;

        if let Some(entry) = REMOTE_STDIO_SESSIONS.read().await.get(&key).cloned() {
            entry.activity_epoch.fetch_add(1, Ordering::Relaxed);
            return Ok(RemoteStdioSessionLease::new(entry.session));
        }

        let client = RemoteStdioDaemonClient::spawn(
            self.ssh_manager.clone(),
            context.connection.connection_id.clone(),
            context.binary_path.clone(),
        )
        .await?;
        let mut repo_config = RepoConfig::default();
        repo_config.max_file_size = self.max_file_size().await;
        let session = client
            .open_repo(OpenRepoParams {
                repo_path: PathBuf::from(&context.repo_root),
                storage_root: Some(PathBuf::from(&context.storage_root)),
                config: repo_config,
                refresh: RefreshPolicyConfig::default(),
            })
            .await?;
        let activity_epoch = session.activity_epoch.clone();
        let session = Arc::new(session);
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

    pub async fn resolve_remote_workspace_entry(
        &self,
        root_path: &str,
    ) -> Result<RemoteWorkspaceEntry, String> {
        if let Some(entry) =
            lookup_remote_connection_with_hint(root_path, self.preferred_connection_id.as_deref())
                .await
        {
            return Ok(entry);
        }
        lookup_remote_connection(root_path)
            .await
            .ok_or_else(|| format!("Remote workspace is not registered for path: {root_path}"))
    }

    async fn ensure_remote_search_context(
        &self,
        root_path: &str,
    ) -> Result<RemoteSearchContext, String> {
        let repo_root = normalize_remote_workspace_path(root_path);
        let cache_key =
            remote_search_context_key(self.preferred_connection_id.as_deref(), &repo_root);
        if let Some(context) = REMOTE_SEARCH_CONTEXTS.read().await.get(&cache_key).cloned() {
            let local_bundle = local_flashgrep_bundle_for_arch(&context.remote_arch).await?;
            if local_bundle.sha256 == context.local_binary_sha256 {
                return Ok(context);
            }

            log::info!(
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

        let connection = self.resolve_remote_workspace_entry(&repo_root).await?;
        let cached_server_info = self
            .ssh_manager
            .get_server_info(&connection.connection_id)
            .await;
        let remote_os = if let Some(server_info) = cached_server_info {
            if server_info.os_type.eq_ignore_ascii_case("unknown") {
                self.detect_remote_os_type(&connection.connection_id)
                    .await
                    .unwrap_or_else(|| server_info.os_type.clone())
            } else {
                server_info.os_type
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
        let storage_root = join_remote_path(&repo_root, ".bitfun/search/flashgrep-index");

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
            match self.ssh_manager.execute_command(connection_id, probe).await {
                Ok((stdout, stderr, exit_code)) => {
                    if let Some(arch) = parse_remote_architecture_output(&stdout, &stderr) {
                        return Ok(arch);
                    }
                    attempts.push(format!(
                        "probe=`{probe}` exit_code={exit_code} stdout={:?} stderr={:?}",
                        stdout.trim(),
                        stderr.trim()
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
            let Ok((stdout, stderr, _exit_code)) =
                self.ssh_manager.execute_command(connection_id, probe).await
            else {
                continue;
            };
            if let Some(os_type) = parse_remote_os_output(&stdout, &stderr) {
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

        self.remote_file_service
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
                "Uploading bundled remote flashgrep binary: connection_id={}, path={}, bundle={}, local_path={}, local_sha256={}, remote_sha256={}",
                connection_id,
                remote_binary_path,
                local_bundle.binary_name,
                local_bundle.path.display(),
                local_bundle.sha256,
                remote_sha256.as_deref().unwrap_or("missing")
            );
            self.remote_file_service
                .write_file(connection_id, &remote_binary_path, &local_bundle.bytes)
                .await
                .map_err(|error| format!("Failed to upload flashgrep to remote host: {error}"))?;
        }
        self.ssh_manager
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
        let (stdout, _stderr, exit_code) = self
            .ssh_manager
            .execute_command(connection_id, &command)
            .await
            .map_err(|error| format!("Failed to hash remote flashgrep binary: {error}"))?;
        if exit_code != 0 {
            return Ok(None);
        }
        let hash = stdout.trim();
        if hash.len() == 64 && hash.chars().all(|character| character.is_ascii_hexdigit()) {
            Ok(Some(hash.to_ascii_lowercase()))
        } else {
            Ok(None)
        }
    }

    async fn max_file_size(&self) -> u64 {
        match self
            .config_service
            .get_config::<WorkspaceConfig>(Some("workspace"))
            .await
        {
            Ok(workspace_config) => workspace_config.max_file_size,
            Err(error) => {
                log::warn!(
                    "Failed to read workspace config for remote flashgrep repo open, using default max_file_size: {}",
                    error
                );
                WorkspaceConfig::default().max_file_size
            }
        }
    }
}

pub async fn remote_workspace_search_service_for_path(
    root_path: &str,
    preferred_connection_id: Option<String>,
) -> Result<RemoteWorkspaceSearchService, String> {
    let manager = get_remote_workspace_manager()
        .ok_or_else(|| "Remote workspace manager is unavailable".to_string())?;
    let preferred_connection_id = match preferred_connection_id {
        Some(connection_id) => Some(connection_id),
        None => lookup_remote_connection(root_path)
            .await
            .map(|entry| entry.connection_id),
    };

    Ok(RemoteWorkspaceSearchService::new(
        manager
            .get_ssh_manager()
            .await
            .ok_or_else(|| "SSH manager unavailable".to_string())?,
        manager
            .get_file_service()
            .await
            .ok_or_else(|| "Remote file service unavailable".to_string())?,
        get_global_config_service()
            .await
            .map_err(|error| format!("Config service unavailable: {error}"))?,
    )
    .with_preferred_connection_id(preferred_connection_id))
}

fn remote_stdio_session_key(connection_id: &str, repo_root: &str) -> String {
    format!(
        "{connection_id}\0{}",
        normalize_remote_workspace_path(repo_root)
    )
}

fn remote_search_context_key(preferred_connection_id: Option<&str>, repo_root: &str) -> String {
    format!(
        "{}\0{}",
        preferred_connection_id.unwrap_or(""),
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
            log::info!(
                "Releasing idle remote workspace search stdio session: key={}",
                key.replace('\0', ":")
            );
            entry.session.close().await;
            entry.session.client.shutdown().await;
            REMOTE_STDIO_OPEN_GUARDS.lock().await.remove(&key);
        }
    });
}

fn build_remote_scope(
    repo_root: &str,
    search_path: Option<&Path>,
    globs: Vec<String>,
    file_types: Vec<String>,
    exclude_file_types: Vec<String>,
) -> Result<PathScope, String> {
    let repo_root = normalize_remote_workspace_path(repo_root);
    let roots = match search_path {
        Some(path) => {
            let normalized = normalize_remote_scope_path(&repo_root, path)?;
            if normalized == repo_root {
                Vec::new()
            } else {
                vec![PathBuf::from(normalized)]
            }
        }
        None => Vec::new(),
    };

    Ok(PathScope {
        roots,
        globs,
        iglobs: Vec::new(),
        type_add: Vec::new(),
        type_clear: Vec::new(),
        types: file_types,
        type_not: exclude_file_types,
    })
}

fn normalize_remote_scope_path(repo_root: &str, search_path: &Path) -> Result<String, String> {
    let raw_path = search_path.to_string_lossy();
    let normalized = if raw_path.starts_with('/') {
        normalize_remote_workspace_path(&raw_path)
    } else {
        join_remote_path(repo_root, &raw_path)
    };
    let repo_root_with_slash = format!("{}/", repo_root.trim_end_matches('/'));
    if normalized != repo_root && !normalized.starts_with(&repo_root_with_slash) {
        return Err(format!(
            "Remote search path is outside workspace root: {normalized}"
        ));
    }
    Ok(normalized)
}

fn remote_flashgrep_install_dir(repo_root: &str) -> String {
    join_remote_path(
        &normalize_remote_workspace_path(repo_root),
        REMOTE_FLASHGREP_INSTALL_DIR,
    )
}

fn looks_like_linux_workspace_root(path: &str) -> bool {
    path.starts_with('/') && !path.contains(':')
}

fn parse_remote_architecture_output(stdout: &str, stderr: &str) -> Option<String> {
    for stream in [stdout, stderr] {
        for line in stream.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let normalized = trimmed.to_ascii_lowercase();
            if normalized.contains("x86_64") || normalized.contains("amd64") {
                return Some("x86_64".to_string());
            }
            if normalized.contains("aarch64")
                || normalized.contains("arm64")
                || normalized.contains("armv8")
            {
                return Some("aarch64".to_string());
            }
        }
    }

    None
}

fn parse_remote_os_output(stdout: &str, stderr: &str) -> Option<String> {
    for stream in [stdout, stderr] {
        for line in stream.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let normalized = trimmed.to_ascii_lowercase();
            if normalized.contains("linux") {
                return Some("Linux".to_string());
            }
            if normalized.contains("darwin") || normalized.contains("macos") {
                return Some("Darwin".to_string());
            }
            if normalized.contains("windows")
                || normalized.contains("mingw")
                || normalized.contains("msys")
                || normalized.contains("cygwin")
            {
                return Some("Windows".to_string());
            }
        }
    }

    None
}

fn resolve_local_flashgrep_bundle(binary_name: &str) -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.join("../../..");
    let mut candidates = vec![workspace_root.join("resources/flashgrep").join(binary_name)];

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            candidates.push(parent.join("resources/flashgrep").join(binary_name));
            candidates.push(parent.join("flashgrep").join(binary_name));
            candidates.push(parent.join("../Resources/flashgrep").join(binary_name));
            candidates.push(parent.join("../share/bitfun/flashgrep").join(binary_name));
            candidates.push(
                parent
                    .join("../share/com.bitfun.desktop/flashgrep")
                    .join(binary_name),
            );
        }
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.exists())
        .map(|candidate| candidate.canonicalize().unwrap_or(candidate))
}

async fn local_flashgrep_bundle_for_arch(
    remote_arch: &str,
) -> Result<LocalFlashgrepBundle, String> {
    let bundled_binary_names = match remote_arch {
        "x86_64" | "amd64" => LINUX_X86_64_FLASHGREP_BUNDLES,
        "aarch64" | "arm64" => LINUX_AARCH64_FLASHGREP_BUNDLES,
        arch => {
            return Err(format!(
                "Remote workspace search does not support Linux architecture: {arch}"
            ));
        }
    };

    let (binary_name, path) = bundled_binary_names
        .iter()
        .find_map(|binary_name| {
            resolve_local_flashgrep_bundle(binary_name)
                .map(|path| ((*binary_name).to_string(), path))
        })
        .ok_or_else(|| {
            format!(
                "Bundled Linux flashgrep binary is missing. Expected one of: {}",
                bundled_binary_names
                    .iter()
                    .map(|name| format!("resources/flashgrep/{name}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
    let bytes = tokio::fs::read(&path).await.map_err(|error| {
        format!(
            "Failed to read bundled flashgrep binary {}: {error}",
            path.display()
        )
    })?;
    let sha256 = hex::encode(Sha256::digest(&bytes));

    Ok(LocalFlashgrepBundle {
        binary_name,
        path,
        bytes,
        sha256,
    })
}

fn convert_stdio_search_results(
    search_results: &SearchResults,
    output_mode: ContentSearchOutputMode,
) -> Vec<FileSearchResult> {
    match output_mode {
        ContentSearchOutputMode::Content => {
            let hit_results = convert_stdio_hits_to_file_search_results(search_results);
            if !hit_results.is_empty() {
                return hit_results;
            }

            let count_results = convert_stdio_file_counts_to_search_results(search_results);
            if !count_results.is_empty() {
                return count_results;
            }

            let match_count_results =
                convert_stdio_file_match_counts_to_search_results(search_results);
            if !match_count_results.is_empty() {
                return match_count_results;
            }

            convert_stdio_matched_paths_to_file_only_results(search_results)
        }
        ContentSearchOutputMode::Count => {
            convert_stdio_file_counts_to_search_results(search_results)
        }
        ContentSearchOutputMode::FilesWithMatches => {
            convert_stdio_matched_paths_to_file_only_results(search_results)
        }
    }
}

fn remote_stdio_search_mode(output_mode: ContentSearchOutputMode) -> SearchModeConfig {
    match output_mode {
        ContentSearchOutputMode::Content => SearchModeConfig::LineMatches,
        ContentSearchOutputMode::Count => SearchModeConfig::CountOnly,
        ContentSearchOutputMode::FilesWithMatches => SearchModeConfig::FilesWithMatches,
    }
}

fn convert_stdio_file_counts_to_search_results(
    search_results: &SearchResults,
) -> Vec<FileSearchResult> {
    search_results
        .file_counts
        .iter()
        .map(|count| FileSearchResult {
            path: count.path.clone(),
            name: Path::new(&count.path)
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .unwrap_or(&count.path)
                .to_string(),
            is_directory: false,
            match_type: SearchMatchType::Content,
            line_number: None,
            matched_content: Some(count.matched_lines.to_string()),
            preview_before: None,
            preview_inside: None,
            preview_after: None,
        })
        .collect()
}

fn convert_stdio_file_match_counts_to_search_results(
    search_results: &SearchResults,
) -> Vec<FileSearchResult> {
    search_results
        .file_match_counts
        .iter()
        .map(|count| FileSearchResult {
            path: count.path.clone(),
            name: Path::new(&count.path)
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .unwrap_or(&count.path)
                .to_string(),
            is_directory: false,
            match_type: SearchMatchType::Content,
            line_number: None,
            matched_content: Some(count.matched_occurrences.to_string()),
            preview_before: None,
            preview_inside: None,
            preview_after: None,
        })
        .collect()
}

fn convert_stdio_hits_to_file_search_results(
    search_results: &SearchResults,
) -> Vec<FileSearchResult> {
    let mut file_results = Vec::new();
    for hit in &search_results.hits {
        let name = Path::new(&hit.path)
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .unwrap_or(&hit.path)
            .to_string();

        let mut lines = BTreeMap::new();
        for file_match in &hit.matches {
            lines
                .entry(file_match.location.line)
                .or_insert_with(|| file_match.clone());
        }

        for (_, file_match) in lines {
            let (preview_before, preview_inside, preview_after) =
                split_preview(&file_match.snippet, &file_match.matched_text);
            file_results.push(FileSearchResult {
                path: hit.path.clone(),
                name: name.clone(),
                is_directory: false,
                match_type: SearchMatchType::Content,
                line_number: Some(file_match.location.line),
                matched_content: Some(file_match.snippet),
                preview_before,
                preview_inside,
                preview_after,
            });
        }
    }
    file_results
}

fn convert_stdio_matched_paths_to_file_only_results(
    search_results: &SearchResults,
) -> Vec<FileSearchResult> {
    search_results
        .matched_paths
        .iter()
        .map(|path| FileSearchResult {
            path: path.clone(),
            name: Path::new(path)
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .unwrap_or(path)
                .to_string(),
            is_directory: false,
            match_type: SearchMatchType::Content,
            line_number: None,
            matched_content: None,
            preview_before: None,
            preview_inside: None,
            preview_after: None,
        })
        .collect()
}

fn split_preview(
    snippet: &str,
    matched_text: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    if matched_text.is_empty() {
        return (None, Some(snippet.to_string()), None);
    }

    if let Some(offset) = snippet.find(matched_text) {
        let before = snippet[..offset].to_string();
        let inside = matched_text.to_string();
        let after = snippet[offset + matched_text.len()..].to_string();
        return (
            (!before.is_empty()).then_some(before),
            Some(inside),
            (!after.is_empty()).then_some(after),
        );
    }

    (None, Some(snippet.to_string()), None)
}

fn join_remote_path(base: &str, child: &str) -> String {
    let base = normalize_remote_workspace_path(base);
    let child = child.trim_start_matches('/');
    if base == "/" {
        format!("/{child}")
    } else {
        format!("{base}/{child}")
    }
}

fn shell_escape(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '-' | '_' | ':' | '='))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        looks_like_linux_workspace_root, parse_remote_architecture_output, parse_remote_os_output,
        remote_flashgrep_install_dir,
    };
    use crate::service::search::flashgrep::drain_content_length_messages;

    #[test]
    fn parses_plain_uname_architecture_output() {
        assert_eq!(
            parse_remote_architecture_output("x86_64\n", ""),
            Some("x86_64".to_string())
        );
        assert_eq!(
            parse_remote_architecture_output("aarch64\n", ""),
            Some("aarch64".to_string())
        );
    }

    #[test]
    fn parses_architecture_from_banner_prefixed_output() {
        let stdout = "Welcome to Ubuntu 24.04 LTS\nLast login: today\nArchitecture: amd64\n";
        assert_eq!(
            parse_remote_architecture_output(stdout, ""),
            Some("x86_64".to_string())
        );
    }

    #[test]
    fn parses_architecture_from_stderr_when_needed() {
        assert_eq!(
            parse_remote_architecture_output("", "machine: arm64\n"),
            Some("aarch64".to_string())
        );
    }

    #[test]
    fn installs_remote_flashgrep_under_workspace_root() {
        assert_eq!(
            remote_flashgrep_install_dir("/home/wgq/workspace/bot_detection"),
            "/home/wgq/workspace/bot_detection/.bitfun/bin"
        );
    }

    #[test]
    fn parses_remote_os_from_uname_output() {
        assert_eq!(
            parse_remote_os_output("Linux\n", ""),
            Some("Linux".to_string())
        );
        assert_eq!(
            parse_remote_os_output("Darwin Kernel Version\n", ""),
            Some("Darwin".to_string())
        );
    }

    #[test]
    fn parses_remote_os_from_banner_prefixed_output() {
        assert_eq!(
            parse_remote_os_output("Welcome\nOperating system: linux\n", ""),
            Some("Linux".to_string())
        );
    }

    #[test]
    fn infers_linux_from_posix_workspace_root() {
        assert!(looks_like_linux_workspace_root(
            "/home/wgq/workspace/bot_detection"
        ));
        assert!(!looks_like_linux_workspace_root(
            "C:/Users/wgq/workspace/bot_detection"
        ));
    }

    #[test]
    fn drains_remote_stdio_content_length_messages() {
        let body = r#"{"jsonrpc":"2.0","id":7,"result":{"kind":"pong","now_unix_secs":1}}"#;
        let mut buffer = format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes();
        let messages = drain_content_length_messages(&mut buffer)
            .expect("expected content-length message to decode");

        assert_eq!(messages.len(), 1);
        assert!(buffer.is_empty());
    }

    #[test]
    fn drains_remote_stdio_initialize_response_with_legacy_search_modes() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":{"kind":"initialize_result","protocol_version":1,"server_info":{"name":"flashgrep","version":"0.1.0"},"capabilities":{"workspace_open":true,"workspace_ensure":true,"workspace_list":false,"workspace_refresh":true,"base_snapshot_build":true,"base_snapshot_rebuild":true,"task_status":true,"task_cancel":true,"search_query":true,"glob_query":true,"progress_notifications":true,"status_notifications":true},"search":{"search_modes":["files_with_matches","line_matches","count_only","count_matches"]}}}"#;
        let mut buffer = format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes();
        let messages = drain_content_length_messages(&mut buffer)
            .expect("expected initialize response to decode");

        assert_eq!(messages.len(), 1);
        let debug = format!("{:?}", messages[0]);
        assert!(debug.contains("InitializeResult"));
    }
}
