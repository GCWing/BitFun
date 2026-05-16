use std::{
    ffi::OsString,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex as StdMutex, MutexGuard as StdMutexGuard,
    },
    time::{Duration, Instant},
};

use crate::util::process_manager;
use async_trait::async_trait;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    process::{Child, ChildStderr, ChildStdin, ChildStdout},
    sync::{mpsc, Mutex},
    time::{sleep, timeout},
};

use super::{
    error::{AppError, Result},
    log_flashgrep_stderr_line, FLASHGREP_LOG_TARGET,
    protocol::{
        ClientCapabilities, ClientInfo, GlobParams, InitializeParams, RepoRef, Request, Response,
        SearchParams, TaskRef,
    },
    repo_session::FlashgrepRepoSession,
    rpc_client::{read_content_length_message, ProtocolClient},
    types::{
        GlobOutcome, GlobRequest, OpenRepoParams, RepoStatus, SearchOutcome, SearchRequest,
        TaskStatus,
    },
};

const CLIENT_NAME: &str = "bitfun-workspace-search";
const REPO_CLOSE_TIMEOUT: Duration = Duration::from_secs(2);
const SHUTDOWN_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const DROP_CLEANUP_TIMEOUT: Duration = Duration::from_millis(150);

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
    child: StdMutex<Option<Child>>,
    protocol: ProtocolClient,
    writer_task: StdMutex<Option<tokio::task::JoinHandle<()>>>,
    reader_task: StdMutex<Option<tokio::task::JoinHandle<()>>>,
    stderr_task: StdMutex<Option<tokio::task::JoinHandle<()>>>,
}

fn lock_std_mutex<T>(mutex: &StdMutex<T>) -> StdMutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn take_std_option<T>(mutex: &StdMutex<Option<T>>) -> Option<T> {
    let mut guard = lock_std_mutex(mutex);
    guard.take()
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
                        target: FLASHGREP_LOG_TARGET,
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

#[async_trait]
impl FlashgrepRepoSession for RepoSession {
    async fn status(&self) -> Result<RepoStatus> {
        RepoSession::status(self).await
    }

    async fn task_status(&self, task_id: String) -> Result<TaskStatus> {
        RepoSession::task_status(self, task_id).await
    }

    async fn build_index(&self) -> Result<TaskStatus> {
        RepoSession::index_build(self).await
    }

    async fn rebuild_index(&self) -> Result<TaskStatus> {
        RepoSession::index_rebuild(self).await
    }

    async fn search(&self, request: SearchRequest) -> Result<SearchOutcome> {
        RepoSession::search(self, request).await
    }

    async fn glob(&self, request: GlobRequest) -> Result<GlobOutcome> {
        RepoSession::glob(self, request).await
    }

    async fn close(&self) -> Result<()> {
        RepoSession::close(self).await
    }
}

impl AsyncDaemonClient {
    async fn spawn(daemon_program: Option<OsString>) -> Result<Self> {
        let program = daemon_program
            .or_else(|| std::env::var_os("FLASHGREP_DAEMON_BIN"))
            .unwrap_or_else(|| OsString::from("flashgrep"));

        let mut command = process_manager::create_tokio_command(program);
        command
            .arg("serve")
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        process_manager::configure_process_group(&mut command);

        let mut child = command.spawn()?;
        let stdin = child.stdin.take().ok_or_else(|| {
            AppError::Protocol("flashgrep stdio backend did not provide stdin".into())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AppError::Protocol("flashgrep stdio backend did not provide stdout".into())
        })?;
        let stderr = child.stderr.take();

        let (protocol, write_rx) = ProtocolClient::channel("flashgrep stdio backend");

        let client = Self {
            child: StdMutex::new(Some(child)),
            protocol,
            writer_task: StdMutex::new(None),
            reader_task: StdMutex::new(None),
            stderr_task: StdMutex::new(None),
        };

        client.spawn_writer_task(stdin, write_rx).await;
        client.spawn_reader_task(stdout).await;
        client.spawn_stderr_task(stderr).await;
        if let Err(error) = client.initialize().await {
            client.mark_closed();
            client
                .reject_pending("flashgrep stdio backend failed during startup")
                .await;
            if let Err(terminate_error) = client.wait_for_child_exit().await {
                log::debug!(
                    target: FLASHGREP_LOG_TARGET,
                    "Flashgrep stdio daemon cleanup after failed startup errored: {}",
                    terminate_error
                );
            }
            client.stop_background_tasks().await;
            return Err(error);
        }
        Ok(client)
    }

    fn is_closed(&self) -> bool {
        self.protocol.is_closed()
    }

    async fn initialize(&self) -> Result<()> {
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
                None,
            )
            .await?
        {
            Response::InitializeResult { .. } => {
                self.protocol.send_notification(Request::Initialized).await
            }
            other => unexpected_response("initialize", other),
        }
    }

    async fn send_request_with_timeout(
        &self,
        request: Request,
        request_timeout: Option<Duration>,
    ) -> Result<Response> {
        self.protocol
            .send_request_with_timeout(request, request_timeout)
            .await
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
        self.protocol.mark_closed();
    }

    async fn wait_for_child_exit(&self) -> Result<()> {
        let mut child = take_std_option(&self.child);
        let Some(child) = child.as_mut() else {
            return Ok(());
        };

        match timeout(SHUTDOWN_TIMEOUT, child.wait()).await {
            Ok(wait_result) => {
                wait_result?;
                Ok(())
            }
            Err(_) => {
                process_manager::terminate_child_process_tree(child, Duration::from_millis(750))
                    .await
                    .map_err(AppError::Io)
            }
        }
    }

    async fn stop_background_tasks(&self) {
        let writer_handle = take_std_option(&self.writer_task);
        if let Some(handle) = writer_handle {
            handle.abort();
            let _ = handle.await;
        }
        let reader_handle = take_std_option(&self.reader_task);
        if let Some(handle) = reader_handle {
            handle.abort();
            let _ = handle.await;
        }
        let stderr_handle = take_std_option(&self.stderr_task);
        if let Some(handle) = stderr_handle {
            handle.abort();
            let _ = handle.await;
        }
    }

    async fn spawn_writer_task(&self, stdin: ChildStdin, mut write_rx: mpsc::Receiver<Vec<u8>>) {
        let protocol = self.protocol.clone();
        let handle = tokio::spawn(async move {
            let mut writer = BufWriter::new(stdin);
            while let Some(outbound) = write_rx.recv().await {
                if let Err(error) = writer.write_all(&outbound).await {
                    log::debug!(
                        target: FLASHGREP_LOG_TARGET,
                        "flashgrep stdio daemon stdin write failed: {}",
                        error
                    );
                    protocol
                        .close_with_message("flashgrep stdio backend stdin write failed")
                        .await;
                    return;
                }
                if let Err(error) = writer.flush().await {
                    log::debug!(
                        target: FLASHGREP_LOG_TARGET,
                        "flashgrep stdio daemon stdin flush failed: {}",
                        error
                    );
                    protocol
                        .close_with_message("flashgrep stdio backend stdin flush failed")
                        .await;
                    return;
                }
            }
        });

        *lock_std_mutex(&self.writer_task) = Some(handle);
    }

    async fn spawn_reader_task(&self, stdout: ChildStdout) {
        let protocol = self.protocol.clone();
        let handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let result = reader_loop(&mut reader, &protocol).await;
            match result {
                Ok(()) => {
                    protocol
                        .close_with_message("flashgrep stdio backend closed its stdout pipe")
                        .await;
                }
                Err(error) => {
                    protocol
                        .close_with_message(format!(
                            "flashgrep stdio backend reader failed: {error}"
                        ))
                        .await;
                }
            }
        });

        *lock_std_mutex(&self.reader_task) = Some(handle);
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
                    Ok(_) => log_flashgrep_stderr_line(&line),
                    Err(error) => {
                        log::debug!(
                            target: FLASHGREP_LOG_TARGET,
                            "flashgrep stdio daemon stderr read failed: {}",
                            error
                        );
                        break;
                    }
                }
            }
        });

        *lock_std_mutex(&self.stderr_task) = Some(handle);
    }

    async fn reject_pending(&self, message: impl Into<String>) {
        self.protocol.reject_pending(message.into()).await;
    }

    fn take_child_for_drop(&self) -> Option<Child> {
        take_std_option(&self.child)
    }

    fn abort_background_tasks_for_drop(&self) {
        if let Some(handle) = take_std_option(&self.writer_task) {
            handle.abort();
        }
        if let Some(handle) = take_std_option(&self.reader_task) {
            handle.abort();
        }
        if let Some(handle) = take_std_option(&self.stderr_task) {
            handle.abort();
        }
    }
}

impl Drop for AsyncDaemonClient {
    fn drop(&mut self) {
        self.mark_closed();
        self.abort_background_tasks_for_drop();
        if let Some(child) = self.take_child_for_drop() {
            process_manager::spawn_child_process_tree_cleanup(child, DROP_CLEANUP_TIMEOUT);
        }
    }
}

async fn reader_loop(reader: &mut BufReader<ChildStdout>, protocol: &ProtocolClient) -> Result<()> {
    while let Some(message) = read_content_length_message(reader).await? {
        protocol.handle_server_message(message).await;
    }
    Ok(())
}

fn should_restart_daemon(error: &AppError, daemon: &AsyncDaemonClient) -> bool {
    daemon.is_closed() || matches!(error, AppError::Io(_))
}

fn unexpected_response<T>(method: &str, response: Response) -> Result<T> {
    Err(AppError::Protocol(format!(
        "unexpected {method} response: {response:?}"
    )))
}
