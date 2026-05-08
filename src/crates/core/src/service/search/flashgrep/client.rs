use std::{
    collections::HashMap,
    ffi::OsString,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use serde::Serialize;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command},
    sync::{oneshot, Mutex},
    time::{sleep, timeout},
};

use super::{
    error::{AppError, Result},
    protocol::{
        ClientCapabilities, ClientInfo, GlobParams, InitializeParams, RepoRef, Request,
        RequestEnvelope, Response, ResponseEnvelope, SearchParams, ServerMessage, TaskRef,
    },
    types::{
        GlobOutcome, GlobRequest, OpenRepoParams, RepoStatus, SearchOutcome, SearchRequest,
        TaskStatus,
    },
};

const JSONRPC_VERSION: &str = "2.0";
const CLIENT_NAME: &str = "bitfun-workspace-search";
const REPO_CLOSE_TIMEOUT: Duration = Duration::from_secs(2);
const SHUTDOWN_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

type PendingResponseSender = oneshot::Sender<Result<ResponseEnvelope>>;
type PendingResponses = HashMap<u64, PendingResponseSender>;

#[derive(Debug, Clone)]
pub(crate) struct ManagedClient {
    daemon_program: Option<OsString>,
    start_timeout: Duration,
    retry_interval: Duration,
    shutting_down: Arc<AtomicBool>,
    state: Arc<Mutex<ManagedClientState>>,
    start_guard: Arc<Mutex<()>>,
}

#[derive(Debug)]
pub(crate) struct RepoSession {
    repo_id: String,
    client: ManagedClient,
}

#[derive(Debug, Default)]
struct ManagedClientState {
    daemon: Option<Arc<AsyncDaemonClient>>,
}

#[derive(Debug)]
struct AsyncDaemonClient {
    child: Mutex<Option<Child>>,
    writer: Mutex<BufWriter<ChildStdin>>,
    shared: Arc<DaemonShared>,
    next_id: AtomicU64,
    reader_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    stderr_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

#[derive(Debug, Default)]
struct DaemonShared {
    pending: Mutex<PendingResponses>,
    closed: AtomicBool,
}

impl Default for ManagedClient {
    fn default() -> Self {
        Self {
            daemon_program: None,
            start_timeout: Duration::from_secs(10),
            retry_interval: Duration::from_millis(100),
            shutting_down: Arc::new(AtomicBool::new(false)),
            state: Arc::new(Mutex::new(ManagedClientState::default())),
            start_guard: Arc::new(Mutex::new(())),
        }
    }
}

impl ManagedClient {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_daemon_program(mut self, program: impl Into<OsString>) -> Self {
        self.daemon_program = Some(program.into());
        self
    }

    pub(crate) fn with_start_timeout(mut self, timeout: Duration) -> Self {
        self.start_timeout = timeout;
        self
    }

    pub(crate) fn with_retry_interval(mut self, interval: Duration) -> Self {
        self.retry_interval = interval;
        self
    }

    pub(crate) async fn open_repo(&self, params: OpenRepoParams) -> Result<RepoSession> {
        match self
            .send_request_with_restart(Request::OpenRepo { params })
            .await?
        {
            Response::RepoOpened { repo_id, .. } => Ok(RepoSession {
                repo_id,
                client: self.clone(),
            }),
            other => unexpected_response("open_repo", other),
        }
    }

    pub(crate) async fn shutdown_daemon(&self) -> Result<()> {
        self.shutting_down.store(true, Ordering::Relaxed);
        let daemon = self.state.lock().await.daemon.take();
        if let Some(daemon) = daemon {
            daemon.shutdown().await?;
        }
        Ok(())
    }

    pub(crate) async fn stop_daemon(&self) -> Result<()> {
        let daemon = self.state.lock().await.daemon.take();
        if let Some(daemon) = daemon {
            daemon.shutdown().await?;
        }
        Ok(())
    }

    async fn send_request_with_restart(&self, request: Request) -> Result<Response> {
        self.send_request_with_restart_timeout(request, None).await
    }

    async fn send_request_with_restart_timeout(
        &self,
        request: Request,
        timeout: Option<Duration>,
    ) -> Result<Response> {
        if self.is_shutting_down() {
            return Err(AppError::Protocol(
                "flashgrep stdio backend is shutting down".into(),
            ));
        }

        let daemon = self.get_or_start_daemon().await?;
        match daemon
            .send_request_with_timeout(request.clone(), timeout)
            .await
        {
            Ok(response) => Ok(response),
            Err(error)
                if !self.is_shutting_down() && should_restart_daemon(&error, daemon.as_ref()) =>
            {
                self.clear_daemon_if_current(&daemon).await;
                if let Err(shutdown_error) = daemon.shutdown().await {
                    log::debug!(
                        "Flashgrep stdio daemon shutdown after transport error failed: {}",
                        shutdown_error
                    );
                }
                let restarted = self.get_or_start_daemon().await?;
                restarted.send_request_with_timeout(request, timeout).await
            }
            Err(error) => Err(error),
        }
    }

    async fn get_or_start_daemon(&self) -> Result<Arc<AsyncDaemonClient>> {
        if self.is_shutting_down() {
            return Err(AppError::Protocol(
                "flashgrep stdio backend is shutting down".into(),
            ));
        }

        if let Some(daemon) = self.current_daemon().await {
            return Ok(daemon);
        }

        let _start_guard = self.start_guard.lock().await;
        if self.is_shutting_down() {
            return Err(AppError::Protocol(
                "flashgrep stdio backend is shutting down".into(),
            ));
        }
        if let Some(daemon) = self.current_daemon().await {
            return Ok(daemon);
        }

        let deadline = Instant::now() + self.start_timeout;
        loop {
            match AsyncDaemonClient::spawn(self.daemon_program.clone()).await {
                Ok(daemon) => {
                    let daemon = Arc::new(daemon);
                    self.state.lock().await.daemon = Some(daemon.clone());
                    return Ok(daemon);
                }
                Err(error) if Instant::now() < deadline => {
                    sleep(self.retry_interval).await;
                    let _ = error;
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn current_daemon(&self) -> Option<Arc<AsyncDaemonClient>> {
        let mut state = self.state.lock().await;
        match state.daemon.clone() {
            Some(daemon) if !daemon.is_closed() => Some(daemon),
            Some(_) => {
                state.daemon = None;
                None
            }
            None => None,
        }
    }

    async fn clear_daemon_if_current(&self, current: &Arc<AsyncDaemonClient>) {
        let mut state = self.state.lock().await;
        if state
            .daemon
            .as_ref()
            .is_some_and(|daemon| Arc::ptr_eq(daemon, current))
        {
            state.daemon = None;
        }
    }

    fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::Relaxed)
    }
}

impl RepoSession {
    pub(crate) async fn status(&self) -> Result<RepoStatus> {
        self.send_repo_request(
            "get_repo_status",
            Request::GetRepoStatus {
                params: self.repo_ref(),
            },
            |response| match response {
                Response::RepoStatus { status } => Ok(status),
                other => unexpected_response("get_repo_status", other),
            },
            None,
        )
        .await
    }

    pub(crate) async fn search(&self, request: SearchRequest) -> Result<SearchOutcome> {
        self.send_repo_request(
            "search",
            Request::Search {
                params: SearchParams {
                    repo_id: self.repo_id.clone(),
                    query: request.query,
                    scope: request.scope,
                    consistency: request.consistency,
                    allow_scan_fallback: request.allow_scan_fallback,
                },
            },
            |response| match response {
                Response::SearchCompleted {
                    backend,
                    status,
                    results,
                    ..
                } => Ok(SearchOutcome {
                    backend,
                    status,
                    results,
                }),
                other => unexpected_response("search", other),
            },
            None,
        )
        .await
    }

    pub(crate) async fn glob(&self, request: GlobRequest) -> Result<GlobOutcome> {
        self.send_repo_request(
            "glob",
            Request::Glob {
                params: GlobParams {
                    repo_id: self.repo_id.clone(),
                    scope: request.scope,
                },
            },
            |response| match response {
                Response::GlobCompleted { status, paths, .. } => Ok(GlobOutcome { status, paths }),
                other => unexpected_response("glob", other),
            },
            None,
        )
        .await
    }

    pub(crate) async fn index_build(&self) -> Result<TaskStatus> {
        self.send_repo_request(
            "base_snapshot/build",
            Request::BaseSnapshotBuild {
                params: self.repo_ref(),
            },
            |response| match response {
                Response::TaskStarted { task } => Ok(task),
                other => unexpected_response("base_snapshot/build", other),
            },
            None,
        )
        .await
    }

    pub(crate) async fn index_rebuild(&self) -> Result<TaskStatus> {
        self.send_repo_request(
            "base_snapshot/rebuild",
            Request::BaseSnapshotRebuild {
                params: self.repo_ref(),
            },
            |response| match response {
                Response::TaskStarted { task } => Ok(task),
                other => unexpected_response("base_snapshot/rebuild", other),
            },
            None,
        )
        .await
    }

    pub(crate) async fn task_status(&self, task_id: impl Into<String>) -> Result<TaskStatus> {
        self.send_repo_request(
            "task/status",
            Request::TaskStatus {
                params: TaskRef {
                    task_id: task_id.into(),
                },
            },
            |response| match response {
                Response::TaskStatus { task } => Ok(task),
                other => unexpected_response("task/status", other),
            },
            None,
        )
        .await
    }

    pub(crate) async fn close(&self) -> Result<()> {
        self.send_repo_request(
            "close_repo",
            Request::CloseRepo {
                params: self.repo_ref(),
            },
            |response| match response {
                Response::RepoClosed { .. } => Ok(()),
                other => unexpected_response("close_repo", other),
            },
            Some(REPO_CLOSE_TIMEOUT),
        )
        .await
    }

    fn repo_ref(&self) -> RepoRef {
        RepoRef {
            repo_id: self.repo_id.clone(),
        }
    }

    async fn send_repo_request<T>(
        &self,
        _method: &'static str,
        request: Request,
        decode: impl FnOnce(Response) -> Result<T>,
        timeout: Option<Duration>,
    ) -> Result<T> {
        let response = self
            .client
            .send_request_with_restart_timeout(request, timeout)
            .await?;
        decode(response)
    }
}

impl AsyncDaemonClient {
    async fn spawn(daemon_program: Option<OsString>) -> Result<Self> {
        let program = daemon_program
            .or_else(|| std::env::var_os("FLASHGREP_DAEMON_BIN"))
            .unwrap_or_else(|| OsString::from("flashgrep"));

        let mut command = Command::new(program);
        command
            .arg("serve")
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = command.spawn()?;
        let stdin = child.stdin.take().ok_or_else(|| {
            AppError::Protocol("flashgrep stdio backend did not provide stdin".into())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AppError::Protocol("flashgrep stdio backend did not provide stdout".into())
        })?;
        let stderr = child.stderr.take();

        let client = Self {
            child: Mutex::new(Some(child)),
            writer: Mutex::new(BufWriter::new(stdin)),
            shared: Arc::new(DaemonShared::default()),
            next_id: AtomicU64::new(1),
            reader_task: Mutex::new(None),
            stderr_task: Mutex::new(None),
        };

        client.spawn_reader_task(stdout).await;
        client.spawn_stderr_task(stderr).await;
        client.initialize().await?;
        Ok(client)
    }

    fn is_closed(&self) -> bool {
        self.shared.closed.load(Ordering::Relaxed)
    }

    async fn initialize(&self) -> Result<()> {
        match self
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
                None,
            )
            .await?
        {
            Response::InitializeResult { .. } => self.send_notification(Request::Initialized).await,
            other => unexpected_response("initialize", other),
        }
    }

    async fn send_request_with_timeout(
        &self,
        request: Request,
        request_timeout: Option<Duration>,
    ) -> Result<Response> {
        if self.is_closed() {
            return Err(AppError::Protocol(
                "flashgrep stdio backend is not running".into(),
            ));
        }

        let request_name = request_name(&request);
        let request_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let envelope = RequestEnvelope {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(request_id),
            request,
        };
        let (sender, receiver) = oneshot::channel();
        self.shared.pending.lock().await.insert(request_id, sender);

        if let Err(error) = self.write_envelope(&envelope).await {
            self.shared.pending.lock().await.remove(&request_id);
            return Err(error);
        }

        let response = match request_timeout {
            Some(duration) => match timeout(duration, receiver).await {
                Ok(result) => result.map_err(|_| {
                    AppError::Protocol(
                        "flashgrep stdio backend closed without sending a response".into(),
                    )
                })??,
                Err(_) => {
                    self.shared.pending.lock().await.remove(&request_id);
                    return Err(AppError::Protocol(format!(
                        "flashgrep stdio backend request timed out: {request_name}"
                    )));
                }
            },
            None => receiver.await.map_err(|_| {
                AppError::Protocol(
                    "flashgrep stdio backend closed without sending a response".into(),
                )
            })??,
        };
        decode_response(request_id, response)
    }

    async fn send_notification(&self, request: Request) -> Result<()> {
        let envelope = RequestEnvelope {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: None,
            request,
        };
        self.write_envelope(&envelope).await
    }

    async fn write_envelope(&self, envelope: &RequestEnvelope) -> Result<()> {
        let mut writer = self.writer.lock().await;
        write_content_length_message(&mut writer, envelope).await
    }

    async fn shutdown(&self) -> Result<()> {
        let shutdown_result = if self.is_closed() {
            Ok(())
        } else {
            self.send_request_with_timeout(Request::Shutdown, Some(SHUTDOWN_REQUEST_TIMEOUT))
                .await
                .map(|_| ())
        };

        self.mark_closed();
        self.reject_pending("flashgrep stdio backend is shutting down")
            .await;

        let wait_result = self.wait_for_child_exit().await;
        self.stop_background_tasks().await;

        shutdown_result?;
        wait_result
    }

    fn mark_closed(&self) {
        self.shared.closed.store(true, Ordering::Relaxed);
    }

    async fn wait_for_child_exit(&self) -> Result<()> {
        let mut child = self.child.lock().await.take();
        let Some(child) = child.as_mut() else {
            return Ok(());
        };

        match timeout(SHUTDOWN_TIMEOUT, child.wait()).await {
            Ok(wait_result) => {
                wait_result?;
                Ok(())
            }
            Err(_) => {
                child.kill().await?;
                child.wait().await?;
                Ok(())
            }
        }
    }

    async fn stop_background_tasks(&self) {
        if let Some(handle) = self.reader_task.lock().await.take() {
            handle.abort();
            let _ = handle.await;
        }
        if let Some(handle) = self.stderr_task.lock().await.take() {
            handle.abort();
            let _ = handle.await;
        }
    }

    async fn spawn_reader_task(&self, stdout: ChildStdout) {
        let shared = self.shared.clone();
        let handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let result = reader_loop(&mut reader, &shared).await;
            shared.closed.store(true, Ordering::Relaxed);
            match result {
                Ok(()) => {
                    reject_pending_requests(
                        &shared.pending,
                        "flashgrep stdio backend closed its stdout pipe",
                    )
                    .await;
                }
                Err(error) => {
                    reject_pending_requests(
                        &shared.pending,
                        format!("flashgrep stdio backend reader failed: {error}"),
                    )
                    .await;
                }
            }
        });

        *self.reader_task.lock().await = Some(handle);
    }

    async fn spawn_stderr_task(&self, stderr: Option<ChildStderr>) {
        let Some(stderr) = stderr else {
            return;
        };

        let handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => log::debug!("flashgrep stdio daemon stderr: {}", line.trim_end()),
                    Err(error) => {
                        log::debug!("flashgrep stdio daemon stderr read failed: {}", error);
                        break;
                    }
                }
            }
        });

        *self.stderr_task.lock().await = Some(handle);
    }

    async fn reject_pending(&self, message: impl Into<String>) {
        reject_pending_requests(&self.shared.pending, message.into()).await;
    }
}

async fn reader_loop(
    reader: &mut BufReader<ChildStdout>,
    shared: &Arc<DaemonShared>,
) -> Result<()> {
    while let Some(message) = read_content_length_message(reader).await? {
        match message {
            ServerMessage::Response(response) => {
                let Some(request_id) = response.id else {
                    continue;
                };
                if let Some(sender) = shared.pending.lock().await.remove(&request_id) {
                    let _ = sender.send(Ok(response));
                }
            }
            ServerMessage::Notification(_) => {}
        }
    }
    Ok(())
}

async fn reject_pending_requests(pending: &Mutex<PendingResponses>, message: impl Into<String>) {
    let message = message.into();
    let mut pending = pending.lock().await;
    if pending.is_empty() {
        return;
    }

    for (_, sender) in pending.drain() {
        let _ = sender.send(Err(AppError::Protocol(message.clone())));
    }
}

async fn read_content_length_message(
    reader: &mut BufReader<ChildStdout>,
) -> Result<Option<ServerMessage>> {
    let mut content_length = None;

    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line).await?;
        if read == 0 {
            return Ok(None);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);
        let Some((name, value)) = trimmed.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("Content-Length") {
            let length = value.trim().parse::<usize>().map_err(|error| {
                AppError::Protocol(format!("invalid Content-Length header: {error}"))
            })?;
            content_length = Some(length);
        }
    }

    let content_length =
        content_length.ok_or_else(|| AppError::Protocol("missing Content-Length header".into()))?;
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body).await?;
    serde_json::from_slice(&body)
        .map_err(|error| AppError::Protocol(format!("failed to decode daemon message: {error}")))
}

async fn write_content_length_message(
    writer: &mut BufWriter<ChildStdin>,
    message: &impl Serialize,
) -> Result<()> {
    let body = serde_json::to_vec(message)
        .map_err(|error| AppError::Protocol(format!("failed to encode request: {error}")))?;
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
        .await?;
    writer.write_all(&body).await?;
    writer.flush().await?;
    Ok(())
}

fn request_name(request: &Request) -> &'static str {
    match request {
        Request::Initialize { .. } => "initialize",
        Request::Initialized => "initialized",
        Request::Ping => "ping",
        Request::BaseSnapshotBuild { .. } => "base_snapshot/build",
        Request::BaseSnapshotRebuild { .. } => "base_snapshot/rebuild",
        Request::TaskStatus { .. } => "task/status",
        Request::OpenRepo { .. } => "open_repo",
        Request::GetRepoStatus { .. } => "get_repo_status",
        Request::Search { .. } => "search",
        Request::Glob { .. } => "glob",
        Request::CloseRepo { .. } => "close_repo",
        Request::Shutdown => "shutdown",
    }
}

fn decode_response(request_id: u64, response: ResponseEnvelope) -> Result<Response> {
    if response.id != Some(request_id) {
        return Err(AppError::Protocol(format!(
            "daemon response id mismatch: expected {request_id:?}, got {:?}",
            response.id
        )));
    }

    if response.jsonrpc != JSONRPC_VERSION {
        return Err(AppError::Protocol(format!(
            "unsupported daemon jsonrpc version: {}",
            response.jsonrpc
        )));
    }

    if let Some(error) = response.error {
        return Err(AppError::Protocol(error.message));
    }

    response
        .result
        .ok_or_else(|| AppError::Protocol("daemon response missing result".into()))
}

fn should_restart_daemon(error: &AppError, daemon: &AsyncDaemonClient) -> bool {
    daemon.is_closed() || matches!(error, AppError::Io(_))
}

fn unexpected_response<T>(method: &str, response: Response) -> Result<T> {
    Err(AppError::Protocol(format!(
        "unexpected {method} response: {response:?}"
    )))
}
