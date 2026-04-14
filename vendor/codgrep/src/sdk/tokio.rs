use std::sync::atomic::{AtomicU64, Ordering};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::Mutex,
    task,
    time::{sleep, timeout, Instant},
};

use crate::{
    daemon::{
        protocol::{
            ClientCapabilities, ClientInfo, GlobParams, InitializeParams, OpenRepoParams,
            RefreshRepoParams, RepoRef, Request, RequestEnvelope, Response, ResponseEnvelope,
            SearchParams, ServerMessage,
        },
        ManagedDaemonClient,
    },
    error::{AppError, Result},
    sdk::{
        EnsureRepoParams, EnsuredRepo, GlobOutcome, GlobRequest, OpenedRepo, RepoEvent, RepoStatus,
        SearchOutcome, SearchRequest, TaskState, TaskStatus,
    },
};

#[derive(Debug, Clone)]
pub struct ManagedClient {
    inner: ManagedDaemonClient,
}

#[derive(Debug)]
pub struct RepoSession {
    addr: String,
    repo_id: String,
    opened: Option<OpenedRepo>,
    ensured: Option<EnsuredRepo>,
    client: AsyncDaemonClient,
}

#[derive(Debug)]
pub struct RepoEventSubscription {
    repo_id: String,
    reader: BufReader<OwnedReadHalf>,
    _writer: BufWriter<OwnedWriteHalf>,
}

#[derive(Debug)]
struct AsyncDaemonClient {
    addr: String,
    next_id: AtomicU64,
    connection: Mutex<Option<AsyncDaemonConnection>>,
}

#[derive(Debug)]
struct AsyncDaemonConnection {
    reader: BufReader<OwnedReadHalf>,
    writer: BufWriter<OwnedWriteHalf>,
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

    pub fn with_daemon_program(mut self, program: impl Into<std::ffi::OsString>) -> Self {
        self.inner = self.inner.with_daemon_program(program);
        self
    }

    pub fn with_start_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.inner = self.inner.with_start_timeout(timeout);
        self
    }

    pub fn with_retry_interval(mut self, interval: std::time::Duration) -> Self {
        self.inner = self.inner.with_retry_interval(interval);
        self
    }

    pub async fn ensure_repo(&self, params: EnsureRepoParams) -> Result<RepoSession> {
        let inner = self.inner.clone();
        let ensured = task::spawn_blocking(move || inner.ensure_repo(params))
            .await
            .map_err(|error| {
                AppError::Protocol(format!("async ensure_repo task failed: {error}"))
            })??;
        Ok(RepoSession::from_ensured(EnsuredRepo {
            addr: ensured.addr,
            repo_id: ensured.repo_id,
            status: ensured.status,
            indexed_docs: ensured.indexed_docs,
        }))
    }

    pub async fn open_repo(&self, params: OpenRepoParams) -> Result<RepoSession> {
        let inner = self.inner.clone();
        let opened = task::spawn_blocking(move || inner.open_repo(params))
            .await
            .map_err(|error| {
                AppError::Protocol(format!("async open_repo task failed: {error}"))
            })??;
        Ok(RepoSession::from_opened(OpenedRepo {
            addr: opened.addr,
            repo_id: opened.repo_id,
            status: opened.status,
        }))
    }
}

impl RepoSession {
    fn from_opened(opened: OpenedRepo) -> Self {
        Self {
            client: AsyncDaemonClient::new(opened.addr.clone()),
            addr: opened.addr.clone(),
            repo_id: opened.repo_id.clone(),
            opened: Some(opened),
            ensured: None,
        }
    }

    fn from_ensured(ensured: EnsuredRepo) -> Self {
        Self {
            client: AsyncDaemonClient::new(ensured.addr.clone()),
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
            client: AsyncDaemonClient::new(addr.clone()),
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

    pub async fn subscribe_events(&self) -> Result<RepoEventSubscription> {
        RepoEventSubscription::connect(self.addr.clone(), self.repo_id.clone()).await
    }

    pub async fn status(&self) -> Result<RepoStatus> {
        match self
            .client
            .get_repo_status_isolated(self.repo_id.clone())
            .await?
        {
            Response::RepoStatus { status } => Ok(status),
            other => unexpected_response("get_repo_status", other),
        }
    }

    pub async fn refresh(&self, force: bool) -> Result<RepoStatus> {
        match self
            .client
            .send(Request::RefreshRepo {
                params: RefreshRepoParams {
                    repo_id: self.repo_id.clone(),
                    force,
                },
            })
            .await?
        {
            Response::RepoStatus { status } => Ok(status),
            other => unexpected_response("refresh_repo", other),
        }
    }

    pub async fn build_index(&self) -> Result<(usize, RepoStatus)> {
        match self
            .client
            .send(Request::BuildIndex {
                params: RepoRef {
                    repo_id: self.repo_id.clone(),
                },
            })
            .await?
        {
            Response::RepoBuilt {
                indexed_docs,
                status,
            } => Ok((indexed_docs, status)),
            other => unexpected_response("build_index", other),
        }
    }

    pub async fn rebuild_index(&self) -> Result<(usize, RepoStatus)> {
        match self
            .client
            .send(Request::RebuildIndex {
                params: RepoRef {
                    repo_id: self.repo_id.clone(),
                },
            })
            .await?
        {
            Response::RepoRebuilt {
                indexed_docs,
                status,
            } => Ok((indexed_docs, status)),
            other => unexpected_response("rebuild_index", other),
        }
    }

    pub async fn search(&self, request: SearchRequest) -> Result<SearchOutcome> {
        match self
            .client
            .search(SearchParams {
                repo_id: self.repo_id.clone(),
                query: request.query,
                scope: request.scope,
                consistency: request.consistency,
                allow_scan_fallback: request.allow_scan_fallback,
            })
            .await?
        {
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

    pub async fn glob(&self, request: GlobRequest) -> Result<GlobOutcome> {
        match self
            .client
            .glob(GlobParams {
                repo_id: self.repo_id.clone(),
                scope: request.scope,
            })
            .await?
        {
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

    pub async fn index_build(&self) -> Result<TaskStatus> {
        match self.client.index_build(self.repo_id.clone()).await? {
            Response::TaskStarted { task } => Ok(task),
            other => unexpected_response("index/build", other),
        }
    }

    pub async fn index_rebuild(&self) -> Result<TaskStatus> {
        match self.client.index_rebuild(self.repo_id.clone()).await? {
            Response::TaskStarted { task } => Ok(task),
            other => unexpected_response("index/rebuild", other),
        }
    }

    pub async fn task_status(&self, task_id: impl Into<String>) -> Result<TaskStatus> {
        match self.client.task_status(task_id).await? {
            Response::TaskStatus { task } => Ok(task),
            other => unexpected_response("task/status", other),
        }
    }

    pub async fn task_cancel(&self, task_id: impl Into<String>) -> Result<bool> {
        match self.client.task_cancel(task_id).await? {
            Response::TaskCancelled { accepted, .. } => Ok(accepted),
            other => unexpected_response("task/cancel", other),
        }
    }

    pub async fn wait_task(
        &self,
        task_id: impl Into<String>,
        timeout: std::time::Duration,
    ) -> Result<TaskStatus> {
        const DEFAULT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);
        const EVENT_WAIT_SLICE: std::time::Duration = std::time::Duration::from_millis(500);

        let started = Instant::now();
        let task_id = task_id.into();
        let mut events = self.subscribe_events().await.ok();

        loop {
            let task = self.task_status(task_id.clone()).await?;
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
                match subscription
                    .recv_timeout(remaining.min(EVENT_WAIT_SLICE))
                    .await
                {
                    Ok(Some(RepoEvent::TaskFinished(params))) if params.task.task_id == task_id => {
                        return Ok(params.task);
                    }
                    Ok(Some(_)) | Ok(None) => {}
                    Err(_) => {
                        events = None;
                    }
                }
            } else {
                sleep(remaining.min(DEFAULT_POLL_INTERVAL)).await;
            }
        }
    }

    pub async fn close(self) -> Result<()> {
        match self
            .client
            .send(Request::CloseRepo {
                params: RepoRef {
                    repo_id: self.repo_id,
                },
            })
            .await?
        {
            Response::RepoClosed { .. } => Ok(()),
            other => unexpected_response("close_repo", other),
        }
    }

    pub async fn shutdown_daemon(&self) -> Result<()> {
        match self.client.send(Request::Shutdown).await? {
            Response::ShutdownAck => Ok(()),
            other => unexpected_response("shutdown", other),
        }
    }
}

impl RepoEventSubscription {
    async fn connect(addr: String, repo_id: String) -> Result<Self> {
        let stream = TcpStream::connect(&addr).await?;
        let (reader, writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut writer = BufWriter::new(writer);

        initialize_event_connection(&mut reader, &mut writer).await?;
        Ok(Self {
            repo_id,
            reader,
            _writer: writer,
        })
    }

    pub async fn recv(&mut self) -> Result<RepoEvent> {
        loop {
            match read_server_message(&mut self.reader).await? {
                ServerMessage::Notification(envelope) => {
                    let event = RepoEvent::from_notification(envelope.notification);
                    if event.workspace_id() == self.repo_id {
                        return Ok(event);
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

    pub async fn recv_timeout(
        &mut self,
        duration: std::time::Duration,
    ) -> Result<Option<RepoEvent>> {
        match timeout(duration, self.recv()).await {
            Ok(result) => result.map(Some),
            Err(_) => Ok(None),
        }
    }
}

impl AsyncDaemonClient {
    fn new(addr: impl Into<String>) -> Self {
        Self {
            addr: addr.into(),
            next_id: AtomicU64::new(1),
            connection: Mutex::new(None),
        }
    }

    async fn send(&self, request: Request) -> Result<Response> {
        let request_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let envelope = RequestEnvelope {
            jsonrpc: "2.0".into(),
            id: Some(request_id),
            request,
        };

        let mut connection = self.connection.lock().await;
        let response = match self.send_with_connection(&mut connection, &envelope).await {
            Ok(response) => response,
            Err(_) => {
                *connection = None;
                self.send_with_connection(&mut connection, &envelope)
                    .await?
            }
        };

        if response.id != Some(request_id) {
            return Err(AppError::Protocol(format!(
                "daemon response id mismatch: expected {request_id:?}, got {:?}",
                response.id
            )));
        }

        if response.jsonrpc != "2.0" {
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

    async fn search(&self, params: SearchParams) -> Result<Response> {
        // Keep search requests off the shared session connection to avoid
        // head-of-line blocking from unrelated status/task calls.
        self.send_isolated(Request::Search { params }).await
    }

    async fn glob(&self, params: GlobParams) -> Result<Response> {
        self.send(Request::Glob { params }).await
    }

    async fn get_repo_status(&self, repo_id: impl Into<String>) -> Result<Response> {
        self.send(Request::GetRepoStatus {
            params: RepoRef {
                repo_id: repo_id.into(),
            },
        })
        .await
    }

    async fn get_repo_status_isolated(&self, repo_id: impl Into<String>) -> Result<Response> {
        self.send_isolated(Request::GetRepoStatus {
            params: RepoRef {
                repo_id: repo_id.into(),
            },
        })
        .await
    }

    async fn index_build(&self, repo_id: impl Into<String>) -> Result<Response> {
        self.send(Request::IndexBuild {
            params: RepoRef {
                repo_id: repo_id.into(),
            },
        })
        .await
    }

    async fn index_rebuild(&self, repo_id: impl Into<String>) -> Result<Response> {
        self.send(Request::IndexRebuild {
            params: RepoRef {
                repo_id: repo_id.into(),
            },
        })
        .await
    }

    async fn task_status(&self, task_id: impl Into<String>) -> Result<Response> {
        self.send(Request::TaskStatus {
            params: crate::daemon::protocol::TaskRef {
                task_id: task_id.into(),
            },
        })
        .await
    }

    async fn task_cancel(&self, task_id: impl Into<String>) -> Result<Response> {
        self.send(Request::TaskCancel {
            params: crate::daemon::protocol::TaskRef {
                task_id: task_id.into(),
            },
        })
        .await
    }

    async fn send_with_connection(
        &self,
        connection: &mut Option<AsyncDaemonConnection>,
        envelope: &RequestEnvelope,
    ) -> Result<ResponseEnvelope> {
        let connection = match connection {
            Some(connection) => connection,
            None => {
                *connection = Some(self.connect().await?);
                connection
                    .as_mut()
                    .expect("connection must exist after successful connect")
            }
        };

        let payload = serde_json::to_vec(envelope)
            .map_err(|error| AppError::Protocol(format!("failed to encode request: {error}")))?;
        connection.writer.write_all(&payload).await?;
        connection.writer.write_all(b"\n").await?;
        connection.writer.flush().await?;

        let mut line = String::new();
        let read = connection.reader.read_line(&mut line).await?;
        if read == 0 {
            return Err(AppError::Protocol(
                "daemon closed connection without a response".into(),
            ));
        }
        serde_json::from_str(&line)
            .map_err(|error| AppError::Protocol(format!("failed to decode response: {error}")))
    }

    async fn send_isolated(&self, request: Request) -> Result<Response> {
        let request_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let envelope = RequestEnvelope {
            jsonrpc: "2.0".into(),
            id: Some(request_id),
            request,
        };

        let mut connection = Some(self.connect().await?);
        let response = self
            .send_with_connection(&mut connection, &envelope)
            .await?;

        if response.id != Some(request_id) {
            return Err(AppError::Protocol(format!(
                "daemon response id mismatch: expected {request_id:?}, got {:?}",
                response.id
            )));
        }

        if response.jsonrpc != "2.0" {
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

    async fn connect(&self) -> Result<AsyncDaemonConnection> {
        let stream = TcpStream::connect(&self.addr).await?;
        let (reader, writer) = stream.into_split();
        Ok(AsyncDaemonConnection {
            reader: BufReader::new(reader),
            writer: BufWriter::new(writer),
        })
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

async fn initialize_event_connection(
    reader: &mut BufReader<OwnedReadHalf>,
    writer: &mut BufWriter<OwnedWriteHalf>,
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
    )
    .await?;

    let response = match read_server_message(reader).await? {
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
    )
    .await?;
    Ok(())
}

fn event_capabilities() -> ClientCapabilities {
    ClientCapabilities {
        progress: true,
        status_notifications: true,
        task_notifications: true,
    }
}

async fn write_request(
    writer: &mut BufWriter<OwnedWriteHalf>,
    envelope: RequestEnvelope,
) -> Result<()> {
    let payload = serde_json::to_vec(&envelope)
        .map_err(|error| AppError::Protocol(format!("failed to encode request: {error}")))?;
    writer.write_all(&payload).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}

async fn read_server_message(reader: &mut BufReader<OwnedReadHalf>) -> Result<ServerMessage> {
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line).await?;
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
