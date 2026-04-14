//! Rust SDK for the codgrep daemon process API.
//!
//! This module is the intended Rust-facing facade when integrating with
//! `codgrep` as an external daemon-backed service.

#[cfg(feature = "tokio-sdk")]
#[path = "sdk/tokio.rs"]
pub mod tokio;

use std::{
    ffi::OsString,
    io::{BufRead, BufReader, BufWriter, Write},
    net::TcpStream,
    time::{Duration, Instant},
};

use crate::{
    daemon::{
        protocol::{
            ClientCapabilities, ClientInfo, GlobParams, InitializeParams, Notification,
            ProgressNotificationParams, RefreshRepoParams, RepoRef, Request, RequestEnvelope,
            Response, ResponseEnvelope, SearchParams, ServerMessage, TaskFinishedParams,
            WorkspaceStatusChangedParams,
        },
        DaemonClient, ManagedDaemonClient,
    },
    error::{AppError, Result},
};

pub use crate::daemon::protocol::{
    ConsistencyMode, DirtyFileStats, EnsureRepoParams, FileCount, FileMatchCount, OpenRepoParams,
    PathScope, QuerySpec, RefreshPolicyConfig, RepoConfig, RepoPhase, RepoStatus, SearchBackend,
    SearchModeConfig, SearchResults, TaskKind, TaskPhase, TaskState, TaskStatus,
};

pub type RepoProgress = ProgressNotificationParams;
pub type RepoWorkspaceStatusChanged = WorkspaceStatusChangedParams;
pub type RepoTaskFinished = TaskFinishedParams;

#[derive(Debug, Clone)]
pub struct ManagedClient {
    inner: ManagedDaemonClient,
}

#[derive(Debug, Clone)]
pub struct EnsuredRepo {
    pub addr: String,
    pub repo_id: String,
    pub status: RepoStatus,
    pub indexed_docs: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct OpenedRepo {
    pub addr: String,
    pub repo_id: String,
    pub status: RepoStatus,
}

#[derive(Debug)]
pub struct RepoSession {
    addr: String,
    repo_id: String,
    opened: Option<OpenedRepo>,
    ensured: Option<EnsuredRepo>,
    client: DaemonClient,
}

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
    pub repo_id: String,
    pub backend: SearchBackend,
    pub consistency_applied: ConsistencyMode,
    pub status: RepoStatus,
    pub results: SearchResults,
}

#[derive(Debug, Clone)]
pub struct GlobOutcome {
    pub repo_id: String,
    pub status: RepoStatus,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum RepoEvent {
    Progress(RepoProgress),
    WorkspaceStatusChanged(RepoWorkspaceStatusChanged),
    TaskFinished(RepoTaskFinished),
}

#[derive(Debug)]
pub struct RepoEventSubscription {
    repo_id: String,
    reader: BufReader<TcpStream>,
}

impl Default for ManagedClient {
    fn default() -> Self {
        Self {
            inner: ManagedDaemonClient::new(),
        }
    }
}

impl ManagedClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_daemon_program(mut self, program: impl Into<OsString>) -> Self {
        self.inner = self.inner.with_daemon_program(program);
        self
    }

    pub fn with_start_timeout(mut self, timeout: Duration) -> Self {
        self.inner = self.inner.with_start_timeout(timeout);
        self
    }

    pub fn with_retry_interval(mut self, interval: Duration) -> Self {
        self.inner = self.inner.with_retry_interval(interval);
        self
    }

    pub fn ensure_repo(&self, params: EnsureRepoParams) -> Result<RepoSession> {
        let ensured = self.inner.ensure_repo(params)?;
        let ensured = EnsuredRepo {
            addr: ensured.addr,
            repo_id: ensured.repo_id,
            status: ensured.status,
            indexed_docs: ensured.indexed_docs,
        };
        Ok(RepoSession::from_ensured(ensured))
    }

    pub fn open_repo(&self, params: OpenRepoParams) -> Result<RepoSession> {
        let opened = self.inner.open_repo(params)?;
        Ok(RepoSession::from_opened(OpenedRepo {
            addr: opened.addr,
            repo_id: opened.repo_id,
            status: opened.status,
        }))
    }
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

impl RepoSession {
    fn from_opened(opened: OpenedRepo) -> Self {
        Self {
            client: DaemonClient::new(opened.addr.clone()),
            addr: opened.addr.clone(),
            repo_id: opened.repo_id.clone(),
            opened: Some(opened),
            ensured: None,
        }
    }

    fn from_ensured(ensured: EnsuredRepo) -> Self {
        Self {
            client: DaemonClient::new(ensured.addr.clone()),
            addr: ensured.addr.clone(),
            repo_id: ensured.repo_id.clone(),
            opened: Some(OpenedRepo {
                addr: ensured.addr.clone(),
                repo_id: ensured.repo_id.clone(),
                status: ensured.status.clone(),
            }),
            ensured: Some(ensured),
        }
    }

    pub fn connect(addr: impl Into<String>, repo_id: impl Into<String>) -> Self {
        let addr = addr.into();
        let repo_id = repo_id.into();
        Self {
            client: DaemonClient::new(addr.clone()),
            addr,
            repo_id,
            opened: None,
            ensured: None,
        }
    }

    pub fn addr(&self) -> &str {
        &self.addr
    }

    pub fn repo_id(&self) -> &str {
        &self.repo_id
    }

    pub fn opened_repo(&self) -> Option<&OpenedRepo> {
        self.opened.as_ref()
    }

    pub fn ensured_repo(&self) -> Option<&EnsuredRepo> {
        self.ensured.as_ref()
    }

    pub fn subscribe_events(&self) -> Result<RepoEventSubscription> {
        RepoEventSubscription::connect(self.addr.clone(), self.repo_id.clone())
    }

    pub fn status(&self) -> Result<RepoStatus> {
        match self.client.get_repo_status(self.repo_id.clone())? {
            Response::RepoStatus { status } => Ok(status),
            other => unexpected_response("get_repo_status", other),
        }
    }

    pub fn refresh(&self, force: bool) -> Result<RepoStatus> {
        match self
            .client
            .send(crate::daemon::protocol::Request::RefreshRepo {
                params: RefreshRepoParams {
                    repo_id: self.repo_id.clone(),
                    force,
                },
            })? {
            Response::RepoStatus { status } => Ok(status),
            other => unexpected_response("refresh_repo", other),
        }
    }

    pub fn build_index(&self) -> Result<(usize, RepoStatus)> {
        match self
            .client
            .send(crate::daemon::protocol::Request::BuildIndex {
                params: RepoRef {
                    repo_id: self.repo_id.clone(),
                },
            })? {
            Response::RepoBuilt {
                indexed_docs,
                status,
            } => Ok((indexed_docs, status)),
            other => unexpected_response("build_index", other),
        }
    }

    pub fn rebuild_index(&self) -> Result<(usize, RepoStatus)> {
        match self
            .client
            .send(crate::daemon::protocol::Request::RebuildIndex {
                params: RepoRef {
                    repo_id: self.repo_id.clone(),
                },
            })? {
            Response::RepoRebuilt {
                indexed_docs,
                status,
            } => Ok((indexed_docs, status)),
            other => unexpected_response("rebuild_index", other),
        }
    }

    pub fn search(&self, request: SearchRequest) -> Result<SearchOutcome> {
        match self.client.search(SearchParams {
            repo_id: self.repo_id.clone(),
            query: request.query,
            scope: request.scope,
            consistency: request.consistency,
            allow_scan_fallback: request.allow_scan_fallback,
        })? {
            Response::SearchCompleted {
                repo_id,
                backend,
                consistency_applied,
                status,
                results,
            } => Ok(SearchOutcome {
                repo_id,
                backend,
                consistency_applied,
                status,
                results,
            }),
            other => unexpected_response("search", other),
        }
    }

    pub fn glob(&self, request: GlobRequest) -> Result<GlobOutcome> {
        match self.client.glob(GlobParams {
            repo_id: self.repo_id.clone(),
            scope: request.scope,
        })? {
            Response::GlobCompleted {
                repo_id,
                status,
                paths,
            } => Ok(GlobOutcome {
                repo_id,
                status,
                paths,
            }),
            other => unexpected_response("glob", other),
        }
    }

    pub fn index_build(&self) -> Result<TaskStatus> {
        match self.client.index_build(self.repo_id.clone())? {
            Response::TaskStarted { task } => Ok(task),
            other => unexpected_response("index/build", other),
        }
    }

    pub fn index_rebuild(&self) -> Result<TaskStatus> {
        match self.client.index_rebuild(self.repo_id.clone())? {
            Response::TaskStarted { task } => Ok(task),
            other => unexpected_response("index/rebuild", other),
        }
    }

    pub fn task_status(&self, task_id: impl Into<String>) -> Result<TaskStatus> {
        match self.client.task_status(task_id)? {
            Response::TaskStatus { task } => Ok(task),
            other => unexpected_response("task/status", other),
        }
    }

    pub fn task_cancel(&self, task_id: impl Into<String>) -> Result<bool> {
        match self.client.task_cancel(task_id)? {
            Response::TaskCancelled { accepted, .. } => Ok(accepted),
            other => unexpected_response("task/cancel", other),
        }
    }

    pub fn wait_task(&self, task_id: impl Into<String>, timeout: Duration) -> Result<TaskStatus> {
        const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(100);
        const EVENT_WAIT_SLICE: Duration = Duration::from_millis(500);

        let started = std::time::Instant::now();
        let task_id = task_id.into();
        let mut events = self.subscribe_events().ok();

        loop {
            let task = self.task_status(task_id.clone())?;
            if is_terminal_task_state(task.state) {
                return Ok(task);
            }

            let elapsed = started.elapsed();
            if elapsed >= timeout {
                return Err(AppError::Protocol(format!(
                    "timed out waiting for task {} to finish after {:?}",
                    task_id, timeout
                )));
            }

            let remaining = timeout.saturating_sub(elapsed);
            if let Some(subscription) = events.as_mut() {
                match subscription.recv_timeout(remaining.min(EVENT_WAIT_SLICE)) {
                    Ok(Some(RepoEvent::TaskFinished(params))) if params.task.task_id == task_id => {
                        return Ok(params.task);
                    }
                    Ok(Some(_)) | Ok(None) => {}
                    Err(_) => {
                        events = None;
                    }
                }
            } else {
                std::thread::sleep(remaining.min(DEFAULT_POLL_INTERVAL));
            }
        }
    }

    pub fn close(self) -> Result<()> {
        match self
            .client
            .send(crate::daemon::protocol::Request::CloseRepo {
                params: RepoRef {
                    repo_id: self.repo_id,
                },
            })? {
            Response::RepoClosed { .. } => Ok(()),
            other => unexpected_response("close_repo", other),
        }
    }

    pub fn shutdown_daemon(&self) -> Result<()> {
        match self
            .client
            .send(crate::daemon::protocol::Request::Shutdown)?
        {
            Response::ShutdownAck => Ok(()),
            other => unexpected_response("shutdown", other),
        }
    }
}

impl RepoEvent {
    pub fn workspace_id(&self) -> &str {
        match self {
            RepoEvent::Progress(params) => &params.workspace_id,
            RepoEvent::WorkspaceStatusChanged(params) => &params.workspace_id,
            RepoEvent::TaskFinished(params) => &params.task.workspace_id,
        }
    }

    pub fn task_id(&self) -> Option<&str> {
        match self {
            RepoEvent::Progress(params) => Some(&params.task_id),
            RepoEvent::WorkspaceStatusChanged(_) => None,
            RepoEvent::TaskFinished(params) => Some(&params.task.task_id),
        }
    }

    fn from_notification(notification: Notification) -> Self {
        match notification {
            Notification::Progress { params } => RepoEvent::Progress(params),
            Notification::WorkspaceStatusChanged { params } => {
                RepoEvent::WorkspaceStatusChanged(params)
            }
            Notification::TaskFinished { params } => RepoEvent::TaskFinished(params),
        }
    }
}

impl RepoEventSubscription {
    fn connect(addr: String, repo_id: String) -> Result<Self> {
        let stream = TcpStream::connect(addr)?;
        let reader_stream = stream.try_clone()?;
        let mut reader = BufReader::new(reader_stream);
        let mut writer = BufWriter::new(stream);

        initialize_event_connection(&mut reader, &mut writer)?;
        Ok(Self { repo_id, reader })
    }

    pub fn recv(&mut self) -> Result<RepoEvent> {
        self.recv_internal(None)?
            .ok_or_else(|| AppError::Protocol("event subscription closed".into()))
    }

    pub fn recv_timeout(&mut self, timeout: Duration) -> Result<Option<RepoEvent>> {
        self.recv_internal(Some(Instant::now() + timeout))
    }

    fn recv_internal(&mut self, deadline: Option<Instant>) -> Result<Option<RepoEvent>> {
        loop {
            let remaining =
                deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
            if matches!(remaining, Some(duration) if duration.is_zero()) {
                return Ok(None);
            }
            self.reader.get_mut().set_read_timeout(remaining)?;

            let message = match read_server_message(&mut self.reader) {
                Ok(message) => message,
                Err(AppError::Io(error)) if is_timeout(&error) => return Ok(None),
                Err(error) => return Err(error),
            };

            match message {
                ServerMessage::Notification(envelope) => {
                    let event = RepoEvent::from_notification(envelope.notification);
                    if event.workspace_id() == self.repo_id {
                        self.reader.get_mut().set_read_timeout(None)?;
                        return Ok(Some(event));
                    }
                }
                ServerMessage::Response(response) => {
                    return Err(AppError::Protocol(format!(
                        "unexpected daemon response on event subscription: {response:?}"
                    )));
                }
            }
        }
    }
}

fn unexpected_response<T>(method: &str, response: Response) -> Result<T> {
    Err(AppError::Protocol(format!(
        "unexpected {method} response: {response:?}"
    )))
}

fn is_terminal_task_state(state: TaskState) -> bool {
    matches!(
        state,
        TaskState::Completed | TaskState::Failed | TaskState::Cancelled
    )
}

fn initialize_event_connection(
    reader: &mut BufReader<TcpStream>,
    writer: &mut BufWriter<TcpStream>,
) -> Result<()> {
    write_request(
        writer,
        RequestEnvelope {
            jsonrpc: "2.0".into(),
            id: Some(1),
            request: Request::Initialize {
                params: InitializeParams {
                    client_info: Some(ClientInfo {
                        name: "codgrep-rust-sdk".into(),
                        version: Some(env!("CARGO_PKG_VERSION").into()),
                    }),
                    capabilities: event_capabilities(),
                },
            },
        },
    )?;

    let response = match read_server_message(reader)? {
        ServerMessage::Response(response) => response,
        ServerMessage::Notification(notification) => {
            return Err(AppError::Protocol(format!(
                "unexpected notification during event subscription handshake: {notification:?}"
            )))
        }
    };
    validate_initialize_response(response)?;

    write_request(
        writer,
        RequestEnvelope {
            jsonrpc: "2.0".into(),
            id: None,
            request: Request::Initialized,
        },
    )?;
    Ok(())
}

fn event_capabilities() -> ClientCapabilities {
    ClientCapabilities {
        progress: true,
        status_notifications: true,
        task_notifications: true,
    }
}

fn write_request(writer: &mut impl Write, envelope: RequestEnvelope) -> Result<()> {
    serde_json::to_writer(&mut *writer, &envelope)
        .map_err(|error| AppError::Protocol(format!("failed to encode request: {error}")))?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn read_server_message(reader: &mut impl BufRead) -> Result<ServerMessage> {
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return Err(AppError::Protocol(
                "daemon closed event subscription connection".into(),
            ));
        }
        if line.trim().is_empty() {
            continue;
        }
        return serde_json::from_str(&line).map_err(|error| {
            AppError::Protocol(format!("failed to decode daemon message: {error}"))
        });
    }
}

fn validate_initialize_response(response: ResponseEnvelope) -> Result<()> {
    if response.jsonrpc != "2.0" {
        return Err(AppError::Protocol(format!(
            "unsupported daemon jsonrpc version: {}",
            response.jsonrpc
        )));
    }
    if response.id != Some(1) {
        return Err(AppError::Protocol(format!(
            "daemon response id mismatch during event subscription handshake: {:?}",
            response.id
        )));
    }
    if let Some(error) = response.error {
        return Err(AppError::Protocol(error.message));
    }
    match response.result {
        Some(Response::InitializeResult { .. }) => Ok(()),
        Some(other) => Err(AppError::Protocol(format!(
            "unexpected initialize response during event subscription handshake: {other:?}"
        ))),
        None => Err(AppError::Protocol(
            "daemon initialize response missing result".into(),
        )),
    }
}

fn is_timeout(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    )
}

pub fn count_only_query(pattern: impl Into<String>) -> QuerySpec {
    QuerySpec {
        pattern: pattern.into(),
        patterns: Vec::new(),
        case_insensitive: false,
        multiline: false,
        dot_matches_new_line: false,
        fixed_strings: false,
        word_regexp: false,
        line_regexp: false,
        before_context: 0,
        after_context: 0,
        top_k_tokens: 6,
        max_count: None,
        global_max_results: None,
        search_mode: SearchModeConfig::CountOnly,
    }
}

pub fn count_matches_query(pattern: impl Into<String>) -> QuerySpec {
    QuerySpec {
        search_mode: SearchModeConfig::CountMatches,
        ..count_only_query(pattern)
    }
}

pub fn materialize_query(pattern: impl Into<String>) -> QuerySpec {
    QuerySpec {
        search_mode: SearchModeConfig::MaterializeMatches,
        ..count_only_query(pattern)
    }
}

pub fn fixed_string_count_query(pattern: impl Into<String>) -> QuerySpec {
    let pattern = pattern.into();
    QuerySpec {
        pattern: pattern.clone(),
        fixed_strings: true,
        patterns: vec![pattern],
        search_mode: SearchModeConfig::CountOnly,
        ..count_only_query("")
    }
}
