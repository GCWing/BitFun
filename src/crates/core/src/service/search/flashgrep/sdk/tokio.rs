use std::sync::atomic::{AtomicU64, Ordering};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::Mutex,
    task,
};

use crate::service::search::flashgrep::{
    daemon::{
        protocol::{
            GlobParams, RepoRef, Request, RequestEnvelope, Response, ResponseEnvelope,
            SearchParams, TaskRef,
        },
        ManagedDaemonClient, OpenedRepo,
    },
    error::{AppError, Result},
    sdk::{
        GlobOutcome, GlobRequest, OpenRepoParams, RepoStatus, SearchOutcome, SearchRequest,
        TaskStatus,
    },
};

#[derive(Debug, Clone)]
pub(crate) struct ManagedClient {
    inner: ManagedDaemonClient,
}

#[derive(Debug)]
pub(crate) struct RepoSession {
    repo_id: String,
    client: AsyncDaemonClient,
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
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_daemon_program(mut self, program: impl Into<std::ffi::OsString>) -> Self {
        self.inner = self.inner.with_daemon_program(program);
        self
    }

    pub(crate) fn with_start_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.inner = self.inner.with_start_timeout(timeout);
        self
    }

    pub(crate) fn with_retry_interval(mut self, interval: std::time::Duration) -> Self {
        self.inner = self.inner.with_retry_interval(interval);
        self
    }

    pub(crate) async fn open_repo(&self, params: OpenRepoParams) -> Result<RepoSession> {
        let inner = self.inner.clone();
        let opened = task::spawn_blocking(move || inner.open_repo(params))
            .await
            .map_err(|error| {
                AppError::Protocol(format!("async open_repo task failed: {error}"))
            })??;
        Ok(RepoSession::from_opened(opened))
    }
}

impl RepoSession {
    fn from_opened(opened: OpenedRepo) -> Self {
        Self {
            client: AsyncDaemonClient::new(opened.addr),
            repo_id: opened.repo_id,
        }
    }

    pub(crate) async fn status(&self) -> Result<RepoStatus> {
        match self
            .client
            .get_repo_status_isolated(self.repo_id.clone())
            .await?
        {
            Response::RepoStatus { status } => Ok(status),
            other => unexpected_response("get_repo_status", other),
        }
    }

    pub(crate) async fn search(&self, request: SearchRequest) -> Result<SearchOutcome> {
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
                repo_id: _,
                backend,
                consistency_applied: _,
                status,
                results,
            } => Ok(SearchOutcome {
                backend,
                status,
                results,
            }),
            other => unexpected_response("search", other),
        }
    }

    pub(crate) async fn glob(&self, request: GlobRequest) -> Result<GlobOutcome> {
        match self
            .client
            .glob(GlobParams {
                repo_id: self.repo_id.clone(),
                scope: request.scope,
            })
            .await?
        {
            Response::GlobCompleted {
                repo_id: _,
                status,
                paths,
            } => Ok(GlobOutcome { status, paths }),
            other => unexpected_response("glob", other),
        }
    }

    pub(crate) async fn index_build(&self) -> Result<TaskStatus> {
        match self
            .client
            .base_snapshot_build(self.repo_id.clone())
            .await?
        {
            Response::TaskStarted { task } => Ok(task),
            other => unexpected_response("base_snapshot/build", other),
        }
    }

    pub(crate) async fn index_rebuild(&self) -> Result<TaskStatus> {
        match self
            .client
            .base_snapshot_rebuild(self.repo_id.clone())
            .await?
        {
            Response::TaskStarted { task } => Ok(task),
            other => unexpected_response("base_snapshot/rebuild", other),
        }
    }

    pub(crate) async fn task_status(&self, task_id: impl Into<String>) -> Result<TaskStatus> {
        match self.client.task_status(task_id).await? {
            Response::TaskStatus { task } => Ok(task),
            other => unexpected_response("task/status", other),
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

    async fn search(&self, params: SearchParams) -> Result<Response> {
        self.send_isolated(Request::Search { params }).await
    }

    async fn glob(&self, params: GlobParams) -> Result<Response> {
        self.send(Request::Glob { params }).await
    }

    async fn get_repo_status_isolated(&self, repo_id: impl Into<String>) -> Result<Response> {
        self.send_isolated(Request::GetRepoStatus {
            params: RepoRef {
                repo_id: repo_id.into(),
            },
        })
        .await
    }

    async fn base_snapshot_build(&self, repo_id: impl Into<String>) -> Result<Response> {
        self.send(Request::BaseSnapshotBuild {
            params: RepoRef {
                repo_id: repo_id.into(),
            },
        })
        .await
    }

    async fn base_snapshot_rebuild(&self, repo_id: impl Into<String>) -> Result<Response> {
        self.send(Request::BaseSnapshotRebuild {
            params: RepoRef {
                repo_id: repo_id.into(),
            },
        })
        .await
    }

    async fn task_status(&self, task_id: impl Into<String>) -> Result<Response> {
        self.send(Request::TaskStatus {
            params: TaskRef {
                task_id: task_id.into(),
            },
        })
        .await
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

        decode_response(request_id, response)
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
        decode_response(request_id, response)
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

    async fn connect(&self) -> Result<AsyncDaemonConnection> {
        let stream = TcpStream::connect(&self.addr).await?;
        let (reader, writer) = stream.into_split();
        Ok(AsyncDaemonConnection {
            reader: BufReader::new(reader),
            writer: BufWriter::new(writer),
        })
    }
}

fn decode_response(request_id: u64, response: ResponseEnvelope) -> Result<Response> {
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

fn unexpected_response<T>(method: &str, response: Response) -> Result<T> {
    Err(AppError::Protocol(format!(
        "unexpected {method} response: {response:?}"
    )))
}
