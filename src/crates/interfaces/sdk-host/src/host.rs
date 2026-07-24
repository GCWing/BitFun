//! Connection-scoped SDK Host request and Query lifecycle.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bitfun_agent_runtime::sdk::{
    AgentDialogTurnRequest, AgentRuntime, AgentSessionCreateRequest, AgentSessionCreateResult,
    AgentSubmissionSource, AgentTransientSessionDiscardRequest, AgentTurnCancellationRequest,
    AgentTurnSettlementRequest, DialogSubmissionPolicy, DialogSubmitOutcome, PermissionReply,
    PermissionReplySource, PermissionRequest, PermissionRequestEvent, PortErrorKind, RuntimeError,
    AUTO_APPROVE_ASK_CONTEXT_KEY,
};
use bitfun_agent_runtime::user_questions::USER_INPUT_AVAILABLE_CONTEXT_KEY;
use bitfun_core_types::ErrorCategory;
use bitfun_events::AgenticEvent;
use futures_util::{stream::FuturesUnordered, FutureExt, StreamExt};
use tokio::sync::{mpsc, oneshot, Mutex, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::{timeout, Instant};
use tokio_util::sync::CancellationToken;

use crate::protocol::{
    ErrorCode, ErrorData, ErrorStage, InitializeParams, InitializeResult, JsonRpcErrorResponse,
    JsonRpcNotification, JsonRpcRequest, JsonRpcSuccessResponse, QueryCancelParams,
    QueryCancelResult, QueryEvent, QueryEventParams, QueryResultError, QueryResultParams,
    QueryStartParams, QueryStartResult, QueryTerminalStatus, RecoveryAction, RequestId,
    SessionCloseParams, SessionCloseResult, SessionCreateParams, SessionCreateResult,
    SessionLifetime, ShutdownParams, ShutdownResult, JSON_RPC_VERSION, METHOD_INITIALIZE,
    METHOD_QUERY_CANCEL, METHOD_QUERY_START, METHOD_SESSION_CLOSE, METHOD_SESSION_CREATE,
    METHOD_SHUTDOWN, NOTIFICATION_QUERY_EVENT, NOTIFICATION_QUERY_RESULT, PROTOCOL_VERSION,
};

const DEFAULT_SESSION_NAME: &str = "BitFun SDK query";
const DEFAULT_AGENT: &str = "agentic";
const DEFAULT_TURN_SETTLEMENT_TIMEOUT_MS: u64 = 5_000;
const PERMISSION_REJECTION_TIMEOUT_MS: u64 = 2_000;
const MAX_SESSION_CLOSE_TIMEOUT_MS: u64 = 30_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionControl {
    Continue,
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct SdkHostConfig {
    pub max_in_flight_requests: usize,
    pub max_in_flight_control_requests: usize,
    pub max_active_queries: usize,
    pub max_leased_sessions: usize,
}

impl Default for SdkHostConfig {
    fn default() -> Self {
        Self {
            max_in_flight_requests: 32,
            max_in_flight_control_requests: 4,
            max_active_queries: 16,
            max_leased_sessions: 64,
        }
    }
}

#[derive(Clone)]
pub struct SdkHostConnection {
    inner: Arc<ConnectionInner>,
}

#[async_trait::async_trait]
pub trait HostOutput: Send + Sync {
    async fn send(&self, value: serde_json::Value) -> Result<(), ()>;
}

struct ChannelHostOutput(mpsc::Sender<serde_json::Value>);

#[async_trait::async_trait]
impl HostOutput for ChannelHostOutput {
    async fn send(&self, value: serde_json::Value) -> Result<(), ()> {
        self.0.send(value).await.map_err(|_| ())
    }
}

struct ConnectionInner {
    runtime: AgentRuntime,
    runtime_version: &'static str,
    default_cwd: String,
    output: Arc<dyn HostOutput>,
    state: Arc<Mutex<ConnectionState>>,
    request_budget: Arc<Semaphore>,
    control_request_budget: Arc<Semaphore>,
    query_budget: Arc<Semaphore>,
    session_budget: Arc<Semaphore>,
    shutdown_started: CancellationToken,
}

#[derive(Default)]
struct ConnectionState {
    initialized: bool,
    shutting_down: bool,
    cleanup_failed: bool,
    sessions: HashMap<String, SessionLease>,
    queries: HashMap<String, Arc<QueryLease>>,
    starting_query_sessions: HashSet<String>,
    active_query_sessions: HashSet<String>,
    closing_sessions: HashSet<String>,
    poisoned_sessions: HashSet<String>,
    pending_session_tasks: Vec<PendingSessionTask>,
    untracked_transient_cleanups: HashMap<String, TransientSessionCleanup>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueryReservationError {
    Unavailable,
    Poisoned,
}

#[derive(Clone)]
struct SessionLease {
    workspace_path: String,
    remote_connection_id: Option<String>,
    remote_ssh_host: Option<String>,
    exposed: bool,
    _budget: Arc<OwnedSemaphorePermit>,
}

struct PendingSessionTask {
    transient_cleanup: Option<TransientSessionCleanup>,
    task: JoinHandle<()>,
}

#[derive(Clone)]
struct TransientSessionCleanup {
    session_id: String,
    workspace_path: String,
}

struct QueryLease {
    query_id: String,
    session_id: String,
    turn_id: String,
    terminal: AtomicBool,
    stop_forwarding: CancellationToken,
    emit_output: bool,
    _budget: OwnedSemaphorePermit,
}

impl QueryLease {
    fn finish_once(&self) -> bool {
        !self.terminal.swap(true, Ordering::AcqRel)
    }
}

impl SdkHostConnection {
    pub fn new(
        runtime: AgentRuntime,
        default_cwd: impl Into<String>,
        output: mpsc::Sender<serde_json::Value>,
        config: SdkHostConfig,
    ) -> Self {
        Self::with_output(
            runtime,
            default_cwd,
            Arc::new(ChannelHostOutput(output)),
            config,
        )
    }

    pub fn with_output(
        runtime: AgentRuntime,
        default_cwd: impl Into<String>,
        output: Arc<dyn HostOutput>,
        config: SdkHostConfig,
    ) -> Self {
        Self {
            inner: Arc::new(ConnectionInner {
                runtime,
                runtime_version: env!("CARGO_PKG_VERSION"),
                default_cwd: default_cwd.into(),
                output,
                state: Arc::new(Mutex::new(ConnectionState::default())),
                request_budget: Arc::new(Semaphore::new(config.max_in_flight_requests.max(1))),
                control_request_budget: Arc::new(Semaphore::new(
                    config.max_in_flight_control_requests.max(1),
                )),
                query_budget: Arc::new(Semaphore::new(config.max_active_queries.max(1))),
                session_budget: Arc::new(Semaphore::new(config.max_leased_sessions.max(1))),
                shutdown_started: CancellationToken::new(),
            }),
        }
    }

    pub async fn handle_request(&self, request: JsonRpcRequest) -> ConnectionControl {
        let request_id = request.id.clone();
        if request.jsonrpc != JSON_RPC_VERSION {
            self.send_rpc_error(
                request_id,
                -32600,
                ErrorCode::InvalidRequest,
                ErrorStage::Protocol,
                false,
                None,
                "jsonrpc must be 2.0",
            )
            .await;
            return ConnectionControl::Continue;
        }

        // Resource and request lifecycle methods require an addressable
        // response. Executing them as notifications would create Sessions or
        // Queries that the caller can neither identify nor close. Shutdown is
        // the only fire-and-forget client method in this protocol version.
        if request.id.is_none() && request.method != METHOD_SHUTDOWN {
            return ConnectionControl::Continue;
        }

        if request.method != METHOD_SHUTDOWN {
            self.reap_finished_pending_session_tasks().await;
        }

        let _request_permit = if request.method == METHOD_SHUTDOWN {
            None
        } else {
            let budget = if matches!(
                request.method.as_str(),
                METHOD_QUERY_CANCEL | METHOD_SESSION_CLOSE
            ) {
                self.inner.control_request_budget.clone()
            } else {
                self.inner.request_budget.clone()
            };
            let Ok(permit) = budget.try_acquire_owned() else {
                self.send_error(
                    request_id,
                    ErrorCode::Overloaded,
                    ErrorStage::Protocol,
                    true,
                    Some(RecoveryAction::Retry),
                    "SDK Host request capacity is exhausted",
                )
                .await;
                return ConnectionControl::Continue;
            };
            Some(permit)
        };

        if request.method == METHOD_INITIALIZE {
            self.handle_initialize(request).await;
            return ConnectionControl::Continue;
        }

        let (initialized, shutting_down, cleanup_failed) = {
            let state = self.inner.state.lock().await;
            (
                state.initialized,
                state.shutting_down,
                state.cleanup_failed || !state.untracked_transient_cleanups.is_empty(),
            )
        };
        if !initialized {
            self.send_error(
                request_id,
                ErrorCode::NotInitialized,
                ErrorStage::Protocol,
                true,
                Some(RecoveryAction::Initialize),
                "initialize must complete before this method",
            )
            .await;
            return ConnectionControl::Continue;
        }
        if cleanup_failed && request.method != METHOD_SHUTDOWN {
            self.send_error(
                request_id,
                ErrorCode::CleanupRequired,
                ErrorStage::Protocol,
                false,
                Some(RecoveryAction::RestartHost),
                "SDK Host cleanup is incomplete; shut down and restart the Host before retrying",
            )
            .await;
            return ConnectionControl::Continue;
        }
        if shutting_down && request.method != METHOD_SHUTDOWN {
            self.send_error(
                request_id,
                ErrorCode::Cancelled,
                ErrorStage::Protocol,
                false,
                None,
                "SDK Host connection is shutting down",
            )
            .await;
            return ConnectionControl::Continue;
        }

        match request.method.as_str() {
            METHOD_SESSION_CREATE => self.handle_session_create(request).await,
            METHOD_QUERY_START => self.handle_query_start(request).await,
            METHOD_QUERY_CANCEL => self.handle_query_cancel(request).await,
            METHOD_SESSION_CLOSE => self.handle_session_close(request).await,
            METHOD_SHUTDOWN => {
                if self
                    .parse_params::<ShutdownParams>(&request, ErrorStage::Shutdown)
                    .await
                    .is_none()
                {
                    return ConnectionControl::Continue;
                }
                {
                    let mut state = self.inner.state.lock().await;
                    state.shutting_down = true;
                }
                self.send_success(request.id.clone(), ShutdownResult { accepted: true })
                    .await;
                return ConnectionControl::Shutdown;
            }
            _ => {
                self.send_rpc_error(
                    request.id.clone(),
                    -32601,
                    ErrorCode::CapabilityUnavailable,
                    ErrorStage::Protocol,
                    false,
                    None,
                    "method is not supported by this SDK Host",
                )
                .await;
            }
        }
        ConnectionControl::Continue
    }

    /// Emits the protocol-owned overload response when a transport reaches its
    /// bounded in-flight request capacity.
    pub async fn reject_overloaded(&self, request_id: Option<RequestId>) {
        self.send_error(
            request_id,
            ErrorCode::Overloaded,
            ErrorStage::Protocol,
            true,
            Some(RecoveryAction::Retry),
            "SDK Host request capacity is exhausted",
        )
        .await;
    }

    /// Reports whether this connection has completed the required initialize
    /// handshake so a transport can serialize re-initialization safely.
    pub async fn is_initialized(&self) -> bool {
        self.inner.state.lock().await.initialized
    }

    async fn reap_finished_pending_session_tasks(&self) {
        let mut state = self.inner.state.lock().await;
        let mut active = Vec::with_capacity(state.pending_session_tasks.len());
        for mut pending in std::mem::take(&mut state.pending_session_tasks) {
            if !pending.task.is_finished() {
                active.push(pending);
                continue;
            }
            match (&mut pending.task).now_or_never() {
                Some(Ok(())) => {}
                Some(Err(error)) => {
                    tracing::warn!(
                        error = %error,
                        "SDK Host Session ownership task failed"
                    );
                    if let Some(cleanup) = pending.transient_cleanup {
                        state
                            .untracked_transient_cleanups
                            .insert(cleanup.session_id.clone(), cleanup);
                    }
                }
                None => active.push(pending),
            }
        }
        state.pending_session_tasks = active;
    }

    pub async fn shutdown_connection(&self) {
        self.shutdown_connection_inner(None).await;
    }

    /// Shuts down one connection without allowing a transient Session create
    /// task to keep the Host process alive indefinitely.
    pub async fn shutdown_connection_bounded(&self, total_timeout: Duration) -> bool {
        self.shutdown_connection_inner(Some(total_timeout)).await
    }

    async fn shutdown_connection_inner(&self, total_timeout: Option<Duration>) -> bool {
        self.inner.shutdown_started.cancel();
        let (pending_session_tasks, prior_cleanup_failed) = {
            let mut state = self.inner.state.lock().await;
            state.shutting_down = true;
            (
                std::mem::take(&mut state.pending_session_tasks),
                state.cleanup_failed,
            )
        };
        let started_at = Instant::now();
        let deadline = total_timeout.map(|timeout| started_at + timeout);
        let graceful_deadline = total_timeout.map(|timeout| started_at + timeout / 2);
        let mut cleanup_complete = !prior_cleanup_failed;
        for pending in pending_session_tasks {
            self.settle_pending_session_task(pending, graceful_deadline)
                .await;
        }
        if !self
            .compensate_registered_transient_sessions(deadline)
            .await
        {
            cleanup_complete = false;
        }
        let (queries, sessions) = {
            let mut state = self.inner.state.lock().await;
            for query in state.queries.values() {
                query.stop_forwarding.cancel();
            }
            let queries = std::mem::take(&mut state.queries)
                .into_values()
                .collect::<Vec<_>>();
            state.active_query_sessions.clear();
            state.starting_query_sessions.clear();
            (queries, std::mem::take(&mut state.sessions))
        };

        let mut cancellations = queries
            .into_iter()
            .map(|query| {
                let runtime = self.inner.runtime.clone();
                let cancellation_timeout = deadline
                    .map(|deadline| {
                        deadline
                            .saturating_duration_since(Instant::now())
                            .min(Duration::from_millis(2_500))
                    })
                    .unwrap_or(Duration::from_millis(2_500));
                async move {
                    timeout(
                        cancellation_timeout,
                        runtime.cancel_turn(AgentTurnCancellationRequest {
                            session_id: query.session_id.clone(),
                            turn_id: Some(query.turn_id.clone()),
                            source: Some(AgentSubmissionSource::SdkHost),
                            requester_session_id: None,
                            reason: Some("sdk_host_connection_shutdown".to_string()),
                            wait_timeout_ms: Some(2_000),
                        }),
                    )
                    .await
                }
            })
            .collect::<FuturesUnordered<_>>();
        while let Some(result) = cancellations.next().await {
            match result {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => {
                    cleanup_complete = false;
                    tracing::warn!(
                        error_kind = runtime_error_kind(&error),
                        "Failed to cancel SDK Host Query during connection shutdown"
                    );
                }
                Err(_) => {
                    cleanup_complete = false;
                    tracing::warn!(
                        "SDK Host Query cancellation timed out during connection shutdown"
                    );
                }
            }
        }

        let mut cleanup = sessions
            .into_iter()
            .map(|(session_id, session)| {
                let runtime = self.inner.runtime.clone();
                let session_cleanup_timeout = deadline
                    .map(|deadline| {
                        deadline
                            .saturating_duration_since(Instant::now())
                            .min(Duration::from_millis(5_500))
                    })
                    .unwrap_or(Duration::from_millis(5_500));
                async move {
                    let reported_session_id = session_id.clone();
                    let cleanup = async move {
                        runtime
                            .discard_transient_session(AgentTransientSessionDiscardRequest {
                                workspace_path: session.workspace_path,
                                session_id: session_id.clone(),
                                remote_connection_id: session.remote_connection_id,
                                remote_ssh_host: session.remote_ssh_host,
                                wait_timeout_ms: duration_ms(
                                    session_cleanup_timeout
                                        .saturating_sub(Duration::from_millis(500)),
                                ),
                            })
                            .await?;
                        Ok(())
                    };
                    (
                        reported_session_id,
                        timeout(session_cleanup_timeout, cleanup).await,
                    )
                }
            })
            .collect::<FuturesUnordered<_>>();
        while let Some((session_id, result)) = cleanup.next().await {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    cleanup_complete = false;
                    tracing::warn!(
                        session_id = %session_id,
                        error_kind = runtime_error_kind(&error),
                        "Failed to clean up SDK Host Session during connection shutdown"
                    );
                }
                Err(_) => {
                    cleanup_complete = false;
                    tracing::warn!(
                        session_id = %session_id,
                        "SDK Host Session cleanup timed out during connection shutdown"
                    );
                }
            }
        }
        cleanup_complete
    }

    async fn settle_pending_session_task(
        &self,
        pending: PendingSessionTask,
        wait_deadline: Option<Instant>,
    ) {
        let PendingSessionTask {
            transient_cleanup,
            mut task,
        } = pending;
        let completed = if task.is_finished() {
            Some((&mut task).await)
        } else {
            match wait_deadline {
                Some(deadline) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    timeout(remaining, &mut task).await.ok()
                }
                None => Some((&mut task).await),
            }
        };

        match completed {
            Some(Ok(())) => {}
            Some(Err(error)) => {
                tracing::warn!(
                    error = %error,
                    "SDK Host Session ownership task failed"
                );
                if let Some(cleanup) = transient_cleanup {
                    self.register_untracked_transient_cleanup(cleanup).await;
                }
            }
            None => {
                task.abort();
                let _ = task.await;
                if let Some(cleanup) = transient_cleanup {
                    self.register_untracked_transient_cleanup(cleanup).await;
                }
            }
        }
    }

    async fn register_untracked_transient_cleanup(&self, cleanup: TransientSessionCleanup) {
        self.inner
            .state
            .lock()
            .await
            .untracked_transient_cleanups
            .insert(cleanup.session_id.clone(), cleanup);
    }

    async fn compensate_registered_transient_sessions(
        &self,
        cleanup_deadline: Option<Instant>,
    ) -> bool {
        let cleanups = self
            .inner
            .state
            .lock()
            .await
            .untracked_transient_cleanups
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let mut compensations = cleanups
            .into_iter()
            .map(|cleanup| {
                let connection = self.clone();
                async move {
                    let session_id = cleanup.session_id.clone();
                    let completed = connection
                        .compensate_untracked_transient_session(cleanup, cleanup_deadline)
                        .await;
                    (session_id, completed)
                }
            })
            .collect::<FuturesUnordered<_>>();
        let mut cleanup_complete = true;
        while let Some((session_id, completed)) = compensations.next().await {
            if completed {
                self.inner
                    .state
                    .lock()
                    .await
                    .untracked_transient_cleanups
                    .remove(&session_id);
            } else {
                cleanup_complete = false;
            }
        }
        cleanup_complete
    }

    async fn compensate_untracked_transient_session(
        &self,
        cleanup: TransientSessionCleanup,
        cleanup_deadline: Option<Instant>,
    ) -> bool {
        let cleanup_timeout = cleanup_deadline
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
            .unwrap_or(Duration::from_secs(5));
        let result = timeout(
            cleanup_timeout,
            self.inner
                .runtime
                .discard_transient_session(AgentTransientSessionDiscardRequest {
                    workspace_path: cleanup.workspace_path,
                    session_id: cleanup.session_id.clone(),
                    remote_connection_id: None,
                    remote_ssh_host: None,
                    wait_timeout_ms: duration_ms(
                        cleanup_timeout.saturating_sub(Duration::from_millis(500)),
                    ),
                }),
        )
        .await;
        match result {
            Ok(Ok(_))
            | Ok(Err(RuntimeError::Port(bitfun_runtime_ports::PortError {
                kind: PortErrorKind::NotFound,
                ..
            }))) => true,
            Ok(Err(error)) => {
                tracing::warn!(
                    session_id = %cleanup.session_id,
                    error_kind = runtime_error_kind(&error),
                    "Failed to compensate an untracked transient SDK Host Session"
                );
                false
            }
            Err(_) => {
                tracing::warn!(
                    session_id = %cleanup.session_id,
                    "Transient SDK Host Session compensation timed out"
                );
                false
            }
        }
    }

    async fn handle_initialize(&self, request: JsonRpcRequest) {
        let Some(params) = self
            .parse_params::<InitializeParams>(&request, ErrorStage::Initialize)
            .await
        else {
            return;
        };
        if params.protocol_version != PROTOCOL_VERSION {
            self.send_error(
                request.id.clone(),
                ErrorCode::VersionMismatch,
                ErrorStage::Initialize,
                false,
                Some(RecoveryAction::UpdateSdk),
                "SDK Host protocol version is incompatible",
            )
            .await;
            return;
        }
        if !params.capabilities.server_notifications {
            self.send_error(
                request.id.clone(),
                ErrorCode::CapabilityUnavailable,
                ErrorStage::Initialize,
                false,
                None,
                "query event notifications are required",
            )
            .await;
            return;
        }
        {
            let mut state = self.inner.state.lock().await;
            if state.initialized {
                drop(state);
                self.send_error(
                    request.id.clone(),
                    ErrorCode::AlreadyInitialized,
                    ErrorStage::Initialize,
                    false,
                    None,
                    "SDK Host connection is already initialized",
                )
                .await;
                return;
            }
            state.initialized = true;
        }
        self.send_success(
            request.id.clone(),
            InitializeResult::current(self.inner.runtime_version),
        )
        .await;
    }

    async fn handle_session_create(&self, request: JsonRpcRequest) {
        let Some(params) = self
            .parse_params::<SessionCreateParams>(&request, ErrorStage::Session)
            .await
        else {
            return;
        };
        let Ok(session_budget) = self.inner.session_budget.clone().try_acquire_owned() else {
            self.send_error(
                request.id.clone(),
                ErrorCode::Overloaded,
                ErrorStage::Session,
                true,
                Some(RecoveryAction::Retry),
                "SDK Host Session capacity is exhausted",
            )
            .await;
            return;
        };
        let workspace_path = params.cwd.unwrap_or_else(|| self.inner.default_cwd.clone());
        let result = self
            .create_leased_session(
                AgentSessionCreateRequest {
                    session_name: params
                        .session_name
                        .unwrap_or_else(|| DEFAULT_SESSION_NAME.to_string()),
                    agent_type: params.agent.unwrap_or_else(|| DEFAULT_AGENT.to_string()),
                    workspace_path: Some(workspace_path.clone()),
                    workspace_id: None,
                    remote_connection_id: None,
                    remote_ssh_host: None,
                    model_id: params.model,
                    metadata: serde_json::Map::new(),
                },
                workspace_path,
                session_budget,
            )
            .await;
        match result {
            Ok(created) => {
                let session_id = created.session_id.clone();
                self.deliver_session_create_response(request.id.clone(), created, session_id)
                    .await;
            }
            Err(error) => {
                self.send_runtime_error(request.id.clone(), ErrorStage::Session, error)
                    .await
            }
        }
    }

    async fn handle_query_start(&self, request: JsonRpcRequest) {
        let emit_output = request.id.is_some();
        let Some(params) = self
            .parse_params::<QueryStartParams>(&request, ErrorStage::Query)
            .await
        else {
            return;
        };
        if params.prompt.trim().is_empty() {
            self.send_invalid_params(
                request.id.clone(),
                ErrorStage::Query,
                "prompt must not be empty",
            )
            .await;
            return;
        }
        if params.session_id.is_some()
            && (params.session_name.is_some()
                || params.agent.is_some()
                || params.cwd.is_some()
                || params.model.is_some())
        {
            self.send_invalid_params(
                request.id.clone(),
                ErrorStage::Query,
                "sessionName, agent, cwd, and model are only valid when creating a Session",
            )
            .await;
            return;
        }
        let Ok(query_budget) = self.inner.query_budget.clone().try_acquire_owned() else {
            self.send_error(
                request.id.clone(),
                ErrorCode::Overloaded,
                ErrorStage::Query,
                true,
                Some(RecoveryAction::Retry),
                "SDK Host active Query capacity is exhausted",
            )
            .await;
            return;
        };

        let (session_id, agent_type, created_session) = match params.session_id.clone() {
            Some(session_id) => {
                let lease = match self.ensure_session_lease(&session_id).await {
                    Ok(lease) => lease,
                    Err(error) => {
                        self.send_runtime_error(request.id.clone(), ErrorStage::Session, error)
                            .await;
                        return;
                    }
                };
                let agent_type = match self
                    .inner
                    .runtime
                    .resolve_session_agent_type(&session_id)
                    .await
                {
                    Ok(Some(agent_type)) => agent_type,
                    Ok(None) => DEFAULT_AGENT.to_string(),
                    Err(error) => {
                        drop(lease);
                        self.send_runtime_error(request.id.clone(), ErrorStage::Session, error)
                            .await;
                        return;
                    }
                };
                (session_id, agent_type, false)
            }
            None => {
                let Ok(session_budget) = self.inner.session_budget.clone().try_acquire_owned()
                else {
                    self.send_error(
                        request.id.clone(),
                        ErrorCode::Overloaded,
                        ErrorStage::Session,
                        true,
                        Some(RecoveryAction::Retry),
                        "SDK Host Session capacity is exhausted",
                    )
                    .await;
                    return;
                };
                let workspace_path = params
                    .cwd
                    .clone()
                    .unwrap_or_else(|| self.inner.default_cwd.clone());
                match self
                    .create_leased_session(
                        AgentSessionCreateRequest {
                            session_name: params
                                .session_name
                                .clone()
                                .unwrap_or_else(|| DEFAULT_SESSION_NAME.to_string()),
                            agent_type: params
                                .agent
                                .clone()
                                .unwrap_or_else(|| DEFAULT_AGENT.to_string()),
                            workspace_path: Some(workspace_path.clone()),
                            workspace_id: None,
                            remote_connection_id: None,
                            remote_ssh_host: None,
                            model_id: params.model.clone(),
                            metadata: serde_json::Map::new(),
                        },
                        workspace_path,
                        session_budget,
                    )
                    .await
                {
                    Ok(created) => {
                        let agent_type = created.agent_type.clone();
                        (created.session_id, agent_type, true)
                    }
                    Err(error) => {
                        self.send_runtime_error(request.id.clone(), ErrorStage::Session, error)
                            .await;
                        return;
                    }
                }
            }
        };

        let session = match self.reserve_query_session(&session_id).await {
            Ok(session) => session,
            Err(reservation_error) => {
                if created_session && self.delete_unexposed_session(&session_id).await.is_err() {
                    self.send_cleanup_required(request.id.clone(), ErrorStage::Query, &session_id)
                        .await;
                    return;
                }
                match reservation_error {
                    QueryReservationError::Unavailable => {
                        self.send_error(
                            request.id.clone(),
                            ErrorCode::Overloaded,
                            ErrorStage::Query,
                            true,
                            Some(RecoveryAction::Retry),
                            "Session cannot accept a new Query while another Query, close, or shutdown is active",
                        )
                        .await;
                    }
                    QueryReservationError::Poisoned => {
                        self.send_cleanup_required(
                            request.id.clone(),
                            ErrorStage::Query,
                            &session_id,
                        )
                        .await;
                    }
                }
                return;
            }
        };
        let session_lifetime = SessionLifetime::Connection;

        let mut events = match self.inner.runtime.subscribe_session_events(&session_id) {
            Ok(events) => events,
            Err(error) => {
                self.release_query_session(&session_id).await;
                if created_session && self.delete_unexposed_session(&session_id).await.is_err() {
                    self.send_cleanup_required(request.id.clone(), ErrorStage::Query, &session_id)
                        .await;
                    return;
                }
                self.send_runtime_error(request.id.clone(), ErrorStage::Query, error)
                    .await;
                return;
            }
        };
        let mut permission_events = match self.inner.runtime.subscribe_permission_requests() {
            Ok(events) => events,
            Err(error) => {
                self.release_query_session(&session_id).await;
                if created_session && self.delete_unexposed_session(&session_id).await.is_err() {
                    self.send_cleanup_required(request.id.clone(), ErrorStage::Query, &session_id)
                        .await;
                    return;
                }
                self.send_runtime_error(request.id.clone(), ErrorStage::Query, error)
                    .await;
                return;
            }
        };
        let mut submission_metadata = serde_json::Map::new();
        submission_metadata.insert(
            USER_INPUT_AVAILABLE_CONTEXT_KEY.to_string(),
            serde_json::Value::Bool(false),
        );
        submission_metadata.insert(
            AUTO_APPROVE_ASK_CONTEXT_KEY.to_string(),
            serde_json::Value::Bool(false),
        );
        let submitted = match self
            .inner
            .runtime
            .submit_dialog_turn(AgentDialogTurnRequest {
                session_id: session_id.clone(),
                message: params.prompt,
                original_message: None,
                turn_id: None,
                agent_type,
                workspace_path: Some(session.workspace_path.clone()),
                remote_connection_id: session.remote_connection_id.clone(),
                remote_ssh_host: session.remote_ssh_host.clone(),
                policy: DialogSubmissionPolicy::for_source(AgentSubmissionSource::SdkHost),
                reply_route: None,
                prepended_reminders: Vec::new(),
                attachments: Vec::new(),
                metadata: submission_metadata,
            })
            .await
        {
            Ok(outcome) => outcome,
            Err(error) => {
                self.release_query_session(&session_id).await;
                if created_session && self.delete_unexposed_session(&session_id).await.is_err() {
                    self.send_cleanup_required(request.id.clone(), ErrorStage::Query, &session_id)
                        .await;
                    return;
                }
                self.send_runtime_error(request.id.clone(), ErrorStage::Query, error)
                    .await;
                return;
            }
        };
        let (submitted_session_id, turn_id) = match submitted {
            DialogSubmitOutcome::Started {
                session_id,
                turn_id,
            } => (session_id, turn_id),
            DialogSubmitOutcome::Queued {
                session_id,
                turn_id,
            } => (session_id, turn_id),
        };
        let query_id = format!("query_{}", uuid::Uuid::new_v4());
        let lease = Arc::new(QueryLease {
            query_id: query_id.clone(),
            session_id: submitted_session_id.clone(),
            turn_id: turn_id.clone(),
            terminal: AtomicBool::new(false),
            stop_forwarding: CancellationToken::new(),
            emit_output,
            _budget: query_budget,
        });
        {
            let mut state = self.inner.state.lock().await;
            state.queries.insert(query_id.clone(), lease.clone());
            state.starting_query_sessions.remove(&session_id);
            state.active_query_sessions.insert(session_id.clone());
        }

        let start_delivered = self
            .deliver_query_start_response(
                request.id.clone(),
                QueryStartResult {
                    query_id: query_id.clone(),
                    session_id: submitted_session_id.clone(),
                    turn_id: turn_id.clone(),
                    accepted: true,
                    created_session,
                    session_lifetime,
                },
                lease.clone(),
                created_session,
            )
            .await;
        if !start_delivered {
            return;
        }

        let connection = self.clone();
        tokio::spawn(async move {
            let mut sequence = 0u64;
            loop {
                let envelope = tokio::select! {
                    _ = lease.stop_forwarding.cancelled() => return,
                    permission = permission_events.recv() => {
                        match permission {
                            Ok(PermissionRequestEvent::Asked { request })
                                if permission_request_targets_query(
                                    &request,
                                    connection
                                        .inner
                                        .runtime
                                        .permission_request_dialog_turn_id(&request.request_id)
                                        .ok()
                                        .flatten()
                                        .as_deref(),
                                    &lease,
                                ) =>
                            {
                                connection.reject_permission_and_finish(&lease, &request).await;
                                return;
                            }
                            Ok(_) => continue,
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                match connection.inner.runtime.pending_permission_requests() {
                                    Ok(pending) => {
                                        if let Some(request) = pending
                                            .into_iter()
                                            .find(|request| {
                                                permission_request_targets_query(
                                                    request,
                                                    connection
                                                        .inner
                                                        .runtime
                                                        .permission_request_dialog_turn_id(
                                                            &request.request_id,
                                                        )
                                                        .ok()
                                                        .flatten()
                                                        .as_deref(),
                                                    &lease,
                                                )
                                            })
                                        {
                                            connection.reject_permission_and_finish(&lease, &request).await;
                                            return;
                                        }
                                        continue;
                                    }
                                    Err(error) => {
                                        connection.cancel_and_finish(
                                            &lease,
                                            query_error_from_runtime(
                                                &lease.query_id,
                                                error,
                                                "SDK Host could not recover permission requests after event lag",
                                            ),
                                            true,
                                        ).await;
                                        return;
                                    }
                                }
                            }
                            Err(_) => {
                                connection.cancel_and_finish(
                                    &lease,
                                    QueryResultError::new(
                                        ErrorCode::Internal,
                                        true,
                                        Some(RecoveryAction::RestartHost),
                                        &lease.query_id,
                                        "SDK Host permission event stream is unavailable",
                                    ),
                                    true,
                                ).await;
                                return;
                            }
                        }
                    }
                    event = events.recv() => match event {
                        Ok(event) => event,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            connection.cancel_and_finish(
                                &lease,
                                QueryResultError::new(
                                    ErrorCode::Internal,
                                    true,
                                    Some(RecoveryAction::RestartHost),
                                    &lease.query_id,
                                    "SDK Host event stream lagged",
                                ),
                                true,
                            ).await;
                            return;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            connection.cancel_and_finish(
                                &lease,
                                QueryResultError::new(
                                    ErrorCode::Internal,
                                    true,
                                    Some(RecoveryAction::RestartHost),
                                    &lease.query_id,
                                    "SDK Host event stream closed",
                                ),
                                true,
                            ).await;
                            return;
                        }
                    }
                };
                if event_turn_id(&envelope.event) != Some(lease.turn_id.as_str()) {
                    continue;
                }
                let terminal = terminal_fact(&envelope.event, &lease.turn_id, &lease.query_id);
                if lease.emit_output {
                    if let Some(projected) = project_query_event(&envelope.event) {
                        sequence += 1;
                        if !connection
                            .send_notification(
                                NOTIFICATION_QUERY_EVENT,
                                QueryEventParams {
                                    query_id: lease.query_id.clone(),
                                    session_id: lease.session_id.clone(),
                                    turn_id: lease.turn_id.clone(),
                                    sequence,
                                    event: projected,
                                },
                            )
                            .await
                        {
                            connection
                                .cancel_and_finish(
                                    &lease,
                                    QueryResultError::new(
                                        ErrorCode::ProcessLost,
                                        false,
                                        None,
                                        &lease.query_id,
                                        "SDK Host output is unavailable",
                                    ),
                                    false,
                                )
                                .await;
                            return;
                        }
                    }
                }
                if let Some((status, error)) = terminal {
                    connection.finish_query(&lease, status, error, true).await;
                    return;
                }
            }
        });
    }

    async fn handle_query_cancel(&self, request: JsonRpcRequest) {
        let Some(params) = self
            .parse_params::<QueryCancelParams>(&request, ErrorStage::Query)
            .await
        else {
            return;
        };
        let lease = self
            .inner
            .state
            .lock()
            .await
            .queries
            .get(&params.query_id)
            .cloned();
        let Some(lease) = lease else {
            self.send_error(
                request.id.clone(),
                ErrorCode::NotFound,
                ErrorStage::Query,
                false,
                None,
                "Query was not found",
            )
            .await;
            return;
        };
        match timeout(
            Duration::from_millis(2_500),
            self.inner
                .runtime
                .cancel_turn(AgentTurnCancellationRequest {
                    session_id: lease.session_id.clone(),
                    turn_id: Some(lease.turn_id.clone()),
                    source: Some(AgentSubmissionSource::SdkHost),
                    requester_session_id: None,
                    reason: Some("sdk_query_cancel".to_string()),
                    wait_timeout_ms: Some(2_000),
                }),
        )
        .await
        {
            Ok(Ok(result)) => {
                self.send_success(
                    request.id.clone(),
                    QueryCancelResult {
                        query_id: lease.query_id.clone(),
                        session_id: lease.session_id.clone(),
                        turn_id: lease.turn_id.clone(),
                        requested: result.requested,
                    },
                )
                .await;
            }
            Ok(Err(error)) => {
                self.send_runtime_error(request.id.clone(), ErrorStage::Query, error)
                    .await
            }
            Err(_) => {
                self.send_error(
                    request.id.clone(),
                    ErrorCode::Timeout,
                    ErrorStage::Query,
                    true,
                    Some(RecoveryAction::Retry),
                    "SDK Host Query cancellation timed out",
                )
                .await;
            }
        }
    }

    async fn handle_session_close(&self, request: JsonRpcRequest) {
        let Some(params) = self
            .parse_params::<SessionCloseParams>(&request, ErrorStage::Session)
            .await
        else {
            return;
        };
        if params
            .wait_timeout_ms
            .is_some_and(|timeout| timeout == 0 || timeout > MAX_SESSION_CLOSE_TIMEOUT_MS)
        {
            self.send_invalid_params(
                request.id.clone(),
                ErrorStage::Session,
                "waitTimeoutMs must be between 1 and 30000",
            )
            .await;
            return;
        }
        let session = {
            let mut state = self.inner.state.lock().await;
            if state.closing_sessions.contains(&params.session_id) {
                drop(state);
                self.send_error(
                    request.id.clone(),
                    ErrorCode::Overloaded,
                    ErrorStage::Session,
                    true,
                    Some(RecoveryAction::Retry),
                    "Session close is already in progress",
                )
                .await;
                return;
            }
            if state.starting_query_sessions.contains(&params.session_id) {
                drop(state);
                self.send_error(
                    request.id.clone(),
                    ErrorCode::Overloaded,
                    ErrorStage::Session,
                    true,
                    Some(RecoveryAction::Retry),
                    "Session has a Query start in progress",
                )
                .await;
                return;
            }
            let session = state.sessions.get(&params.session_id).cloned();
            if session.is_some() {
                state.closing_sessions.insert(params.session_id.clone());
            }
            session
        };
        let Some(session) = session else {
            self.send_error(
                request.id.clone(),
                ErrorCode::NotFound,
                ErrorStage::Session,
                false,
                None,
                "Session is not owned by this SDK Host connection",
            )
            .await;
            return;
        };
        let close_timeout_ms = params.wait_timeout_ms.unwrap_or(5_000);
        let session_id = params.session_id.clone();
        let operation = async {
            self.inner
                .runtime
                .discard_transient_session(AgentTransientSessionDiscardRequest {
                    workspace_path: session.workspace_path,
                    session_id: session_id.clone(),
                    remote_connection_id: session.remote_connection_id,
                    remote_ssh_host: session.remote_ssh_host,
                    wait_timeout_ms: close_timeout_ms,
                })
                .await
        };
        match timeout(Duration::from_millis(close_timeout_ms + 500), operation).await {
            Ok(Ok(unloaded)) => {
                let queries = {
                    let mut state = self.inner.state.lock().await;
                    state.sessions.remove(&params.session_id);
                    state.closing_sessions.remove(&params.session_id);
                    state.poisoned_sessions.remove(&params.session_id);
                    state.starting_query_sessions.remove(&params.session_id);
                    state.active_query_sessions.remove(&params.session_id);
                    let query_ids = state
                        .queries
                        .iter()
                        .filter(|(_, lease)| lease.session_id == params.session_id)
                        .map(|(query_id, _)| query_id.clone())
                        .collect::<Vec<_>>();
                    query_ids
                        .into_iter()
                        .filter_map(|query_id| state.queries.remove(&query_id))
                        .collect::<Vec<_>>()
                };
                for query in queries {
                    query.stop_forwarding.cancel();
                    self.finish_query(&query, QueryTerminalStatus::Cancelled, None, true)
                        .await;
                }
                self.send_success(
                    request.id.clone(),
                    SessionCloseResult {
                        session_id: params.session_id,
                        unloaded,
                    },
                )
                .await;
            }
            Ok(Err(error)) => {
                tracing::warn!(
                    session_id = %params.session_id,
                    error_kind = runtime_error_kind(&error),
                    "SDK Host Session close ended with uncertain cleanup"
                );
                self.mark_session_cleanup_failed(&params.session_id).await;
                self.send_error(
                    request.id.clone(),
                    ErrorCode::CleanupRequired,
                    ErrorStage::Session,
                    false,
                    Some(RecoveryAction::RestartHost),
                    "SDK Host Session cleanup is incomplete; restart the Host before retrying",
                )
                .await;
            }
            Err(_) => {
                tracing::warn!(
                    session_id = %params.session_id,
                    "SDK Host Session close timed out with uncertain cleanup"
                );
                self.mark_session_cleanup_failed(&params.session_id).await;
                self.send_error(
                    request.id.clone(),
                    ErrorCode::CleanupRequired,
                    ErrorStage::Session,
                    false,
                    Some(RecoveryAction::RestartHost),
                    "SDK Host Session cleanup timed out; restart the Host before retrying",
                )
                .await;
            }
        }
    }

    async fn mark_session_cleanup_failed(&self, session_id: &str) {
        let mut state = self.inner.state.lock().await;
        state.closing_sessions.remove(session_id);
        state.poisoned_sessions.insert(session_id.to_string());
        state.cleanup_failed = true;
    }

    async fn ensure_session_lease(&self, session_id: &str) -> Result<SessionLease, RuntimeError> {
        self.inner
            .state
            .lock()
            .await
            .sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| {
                bitfun_runtime_ports::PortError::new(
                    PortErrorKind::NotAvailable,
                    "sessionId must belong to the same SDK Host connection; durable Session resume is not available in this protocol version",
                )
                .into()
            })
    }

    async fn create_leased_session(
        &self,
        request: AgentSessionCreateRequest,
        workspace_path: String,
        session_budget: OwnedSemaphorePermit,
    ) -> Result<bitfun_agent_runtime::sdk::AgentSessionCreateResult, RuntimeError> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let runtime = self.inner.runtime.clone();
        let state = self.inner.state.clone();
        let task_state = state.clone();
        let (result_tx, result_rx) = oneshot::channel();
        let mut connection_state = state.lock().await;
        if connection_state.shutting_down {
            return Err(bitfun_runtime_ports::PortError::new(
                PortErrorKind::Cancelled,
                "SDK Host connection is shutting down",
            )
            .into());
        }
        let task_session_id = session_id.clone();
        let cleanup_workspace_path = workspace_path.clone();
        let creation = tokio::spawn(async move {
            let result = runtime
                .create_transient_session_with_id(task_session_id, request)
                .await;
            if let Ok(created) = &result {
                task_state.lock().await.sessions.insert(
                    created.session_id.clone(),
                    SessionLease {
                        workspace_path,
                        remote_connection_id: None,
                        remote_ssh_host: None,
                        exposed: false,
                        _budget: Arc::new(session_budget),
                    },
                );
            }
            let _ = result_tx.send(result);
        });
        connection_state
            .pending_session_tasks
            .push(PendingSessionTask {
                transient_cleanup: Some(TransientSessionCleanup {
                    session_id,
                    workspace_path: cleanup_workspace_path,
                }),
                task: creation,
            });
        drop(connection_state);
        result_rx.await.map_err(|_| {
            RuntimeError::from(bitfun_runtime_ports::PortError::new(
                PortErrorKind::Backend,
                "SDK Host Session creation task ended without a result",
            ))
        })?
    }

    async fn deliver_session_create_response(
        &self,
        request_id: Option<RequestId>,
        created: AgentSessionCreateResult,
        session_id: String,
    ) -> bool {
        let connection = self.clone();
        let (delivered_tx, delivered_rx) = oneshot::channel();
        let mut state = self.inner.state.lock().await;
        if state.shutting_down {
            return false;
        }
        let delivery = tokio::spawn(async move {
            let delivered = connection
                .send_success(
                    request_id,
                    SessionCreateResult {
                        session_id: created.session_id,
                        session_name: created.session_name,
                        agent: created.agent_type,
                        lifetime: SessionLifetime::Connection,
                    },
                )
                .await;
            if delivered {
                connection.mark_session_exposed(&session_id).await;
            } else {
                tokio::select! {
                    _ = connection.inner.shutdown_started.cancelled() => {}
                    _ = connection.delete_unexposed_session(&session_id) => {}
                }
            }
            let _ = delivered_tx.send(delivered);
        });
        state.pending_session_tasks.push(PendingSessionTask {
            transient_cleanup: None,
            task: delivery,
        });
        drop(state);
        delivered_rx.await.unwrap_or(false)
    }

    async fn deliver_query_start_response(
        &self,
        request_id: Option<RequestId>,
        result: QueryStartResult,
        lease: Arc<QueryLease>,
        created_session: bool,
    ) -> bool {
        let connection = self.clone();
        let (delivered_tx, delivered_rx) = oneshot::channel();
        let mut state = self.inner.state.lock().await;
        if state.shutting_down {
            return false;
        }
        let delivery = tokio::spawn(async move {
            let delivered = connection.send_success(request_id, result).await;
            if delivered {
                if created_session {
                    connection.mark_session_exposed(&lease.session_id).await;
                }
            } else {
                let cleanup = async {
                    connection
                        .cancel_and_finish(
                            &lease,
                            QueryResultError::new(
                                ErrorCode::ProcessLost,
                                false,
                                None,
                                &lease.query_id,
                                "SDK Host output is unavailable",
                            ),
                            false,
                        )
                        .await;
                    if created_session {
                        let _ = connection.delete_unexposed_session(&lease.session_id).await;
                    }
                };
                tokio::select! {
                    _ = connection.inner.shutdown_started.cancelled() => {}
                    _ = cleanup => {}
                }
            }
            let _ = delivered_tx.send(delivered);
        });
        state.pending_session_tasks.push(PendingSessionTask {
            transient_cleanup: None,
            task: delivery,
        });
        drop(state);
        delivered_rx.await.unwrap_or(false)
    }

    async fn reserve_query_session(
        &self,
        session_id: &str,
    ) -> Result<SessionLease, QueryReservationError> {
        let mut state = self.inner.state.lock().await;
        if state.poisoned_sessions.contains(session_id) {
            return Err(QueryReservationError::Poisoned);
        }
        if state.shutting_down
            || state.closing_sessions.contains(session_id)
            || state.starting_query_sessions.contains(session_id)
            || state.active_query_sessions.contains(session_id)
        {
            return Err(QueryReservationError::Unavailable);
        }
        let session = state
            .sessions
            .get(session_id)
            .cloned()
            .ok_or(QueryReservationError::Unavailable)?;
        state.starting_query_sessions.insert(session_id.to_string());
        Ok(session)
    }

    async fn release_query_session(&self, session_id: &str) {
        let mut state = self.inner.state.lock().await;
        state.starting_query_sessions.remove(session_id);
        state.active_query_sessions.remove(session_id);
    }

    async fn mark_session_exposed(&self, session_id: &str) {
        if let Some(session) = self.inner.state.lock().await.sessions.get_mut(session_id) {
            session.exposed = true;
        }
    }

    async fn delete_unexposed_session(&self, session_id: &str) -> Result<(), ()> {
        let lease = self
            .inner
            .state
            .lock()
            .await
            .sessions
            .get(session_id)
            .cloned();
        let Some(lease) = lease else {
            return Ok(());
        };
        match timeout(
            Duration::from_millis(5_000),
            self.inner
                .runtime
                .discard_transient_session(AgentTransientSessionDiscardRequest {
                    workspace_path: lease.workspace_path,
                    session_id: session_id.to_string(),
                    remote_connection_id: lease.remote_connection_id,
                    remote_ssh_host: lease.remote_ssh_host,
                    wait_timeout_ms: 4_500,
                }),
        )
        .await
        {
            Ok(Ok(_)) => {
                self.inner.state.lock().await.sessions.remove(session_id);
                Ok(())
            }
            Ok(Err(error)) => {
                tracing::warn!(
                    session_id = %session_id,
                    error_kind = runtime_error_kind(&error),
                    "Failed to delete an unexposed SDK Host Session"
                );
                self.inner.state.lock().await.cleanup_failed = true;
                Err(())
            }
            Err(_) => {
                tracing::warn!(
                    session_id = %session_id,
                    "Timed out while deleting an unexposed SDK Host Session"
                );
                self.inner.state.lock().await.cleanup_failed = true;
                Err(())
            }
        }
    }

    async fn send_cleanup_required(
        &self,
        request_id: Option<RequestId>,
        stage: ErrorStage,
        session_id: &str,
    ) {
        let message = format!(
            "SDK Host could not remove unexposed Session {session_id}; restart the Host before retrying"
        );
        self.send_error(
            request_id,
            ErrorCode::CleanupRequired,
            stage,
            false,
            Some(RecoveryAction::RestartHost),
            &message,
        )
        .await;
    }

    async fn finish_query(
        &self,
        lease: &Arc<QueryLease>,
        mut status: QueryTerminalStatus,
        mut error: Option<QueryResultError>,
        emit_result: bool,
    ) {
        if !lease.finish_once() {
            return;
        }
        let settlement = timeout(
            Duration::from_millis(DEFAULT_TURN_SETTLEMENT_TIMEOUT_MS + 500),
            self.inner
                .runtime
                .wait_for_turn_settlement(AgentTurnSettlementRequest {
                    session_id: lease.session_id.clone(),
                    turn_id: lease.turn_id.clone(),
                    wait_timeout_ms: DEFAULT_TURN_SETTLEMENT_TIMEOUT_MS,
                }),
        )
        .await;
        let mut poison_session = false;
        match settlement {
            Ok(Ok(())) => {}
            Ok(Err(_settlement_error)) => {
                poison_session = true;
                status = QueryTerminalStatus::Failed;
                error = Some(QueryResultError::new(
                    ErrorCode::CleanupRequired,
                    false,
                    Some(RecoveryAction::RestartHost),
                    &lease.query_id,
                    "SDK Host could not confirm Turn settlement; restart the Host before retrying",
                ));
            }
            Err(_) => {
                poison_session = true;
                status = QueryTerminalStatus::Failed;
                error = Some(QueryResultError::new(
                    ErrorCode::CleanupRequired,
                    false,
                    Some(RecoveryAction::RestartHost),
                    &lease.query_id,
                    "SDK Host Turn settlement timed out; restart the Host before retrying",
                ));
            }
        }
        {
            let mut state = self.inner.state.lock().await;
            state.queries.remove(&lease.query_id);
            state.starting_query_sessions.remove(&lease.session_id);
            state.active_query_sessions.remove(&lease.session_id);
            if poison_session {
                state.poisoned_sessions.insert(lease.session_id.clone());
            }
        }
        if emit_result {
            self.send_query_result(lease, status, error).await;
        }
    }

    async fn send_query_result(
        &self,
        lease: &QueryLease,
        status: QueryTerminalStatus,
        error: Option<QueryResultError>,
    ) -> bool {
        if !lease.emit_output {
            return true;
        }
        self.send_notification(
            NOTIFICATION_QUERY_RESULT,
            QueryResultParams {
                query_id: lease.query_id.clone(),
                session_id: lease.session_id.clone(),
                turn_id: lease.turn_id.clone(),
                status,
                error,
            },
        )
        .await
    }

    async fn cancel_and_finish(
        &self,
        lease: &Arc<QueryLease>,
        mut error: QueryResultError,
        emit_result: bool,
    ) {
        let cancellation = timeout(
            Duration::from_millis(2_500),
            self.inner
                .runtime
                .cancel_turn(AgentTurnCancellationRequest {
                    session_id: lease.session_id.clone(),
                    turn_id: Some(lease.turn_id.clone()),
                    source: Some(AgentSubmissionSource::SdkHost),
                    requester_session_id: None,
                    reason: Some("sdk_host_fail_closed".to_string()),
                    wait_timeout_ms: Some(2_000),
                }),
        )
        .await;
        match cancellation {
            Ok(Ok(_)) => {}
            Ok(Err(cancel_error)) => {
                error = query_error_from_runtime(
                    &lease.query_id,
                    cancel_error,
                    "SDK Host could not cancel the Turn after a Host failure",
                );
            }
            Err(_) => {
                error = QueryResultError::new(
                    ErrorCode::Timeout,
                    true,
                    Some(RecoveryAction::RestartHost),
                    &lease.query_id,
                    "SDK Host cancellation timed out after a Host failure",
                );
            }
        }
        self.finish_query(lease, QueryTerminalStatus::Failed, Some(error), emit_result)
            .await;
    }

    async fn reject_permission_and_finish(
        &self,
        lease: &Arc<QueryLease>,
        request: &PermissionRequest,
    ) {
        let reply = PermissionReply::Reject {
            feedback: Some(
                "Non-interactive SDK execution requires an explicit permission callback"
                    .to_string(),
            ),
        };
        match timeout(
            Duration::from_millis(PERMISSION_REJECTION_TIMEOUT_MS),
            self.inner.runtime.respond_permission_with_source(
                &request.request_id,
                reply,
                PermissionReplySource::System,
            ),
        )
        .await
        {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => {
                self.cancel_and_finish(
                    lease,
                    query_error_from_runtime(
                        &lease.query_id,
                        error,
                        "SDK Host could not reject a pending permission request",
                    ),
                    true,
                )
                .await;
                return;
            }
            Err(_) => {
                self.cancel_and_finish(
                    lease,
                    QueryResultError::new(
                        ErrorCode::Timeout,
                        true,
                        Some(RecoveryAction::RestartHost),
                        &lease.query_id,
                        "SDK Host permission rejection timed out",
                    ),
                    true,
                )
                .await;
                return;
            }
        }
        self.cancel_and_finish(
            lease,
            QueryResultError::new(
                ErrorCode::ActionRequired,
                false,
                None,
                &lease.query_id,
                "Permission approval is required but permission callbacks are unavailable",
            ),
            true,
        )
        .await;
    }

    async fn parse_params<T>(&self, request: &JsonRpcRequest, stage: ErrorStage) -> Option<T>
    where
        T: serde::de::DeserializeOwned,
    {
        match request.params_as() {
            Ok(params) => Some(params),
            Err(_) => {
                self.send_invalid_params(request.id.clone(), stage, "invalid method parameters")
                    .await;
                None
            }
        }
    }

    async fn send_invalid_params(
        &self,
        id: Option<RequestId>,
        stage: ErrorStage,
        message: &'static str,
    ) {
        self.send_rpc_error(
            id,
            -32602,
            ErrorCode::InvalidRequest,
            stage,
            false,
            None,
            message,
        )
        .await;
    }

    async fn send_runtime_error(
        &self,
        id: Option<RequestId>,
        stage: ErrorStage,
        error: RuntimeError,
    ) {
        let (code, retryable, recovery) = runtime_error_facts(&error);
        self.send_error(id, code, stage, retryable, recovery, &error.into_message())
            .await;
    }

    async fn send_error(
        &self,
        id: Option<RequestId>,
        code: ErrorCode,
        stage: ErrorStage,
        retryable: bool,
        recovery: Option<RecoveryAction>,
        message: &str,
    ) {
        self.send_rpc_error(id, -32000, code, stage, retryable, recovery, message)
            .await;
    }

    #[allow(clippy::too_many_arguments)]
    async fn send_rpc_error(
        &self,
        id: Option<RequestId>,
        rpc_code: i32,
        code: ErrorCode,
        stage: ErrorStage,
        retryable: bool,
        recovery: Option<RecoveryAction>,
        message: &str,
    ) {
        let Some(id) = id else {
            return;
        };
        let correlation_id = id.correlation_id();
        self.send_value(JsonRpcErrorResponse::new(
            id,
            rpc_code,
            message,
            ErrorData {
                code,
                stage,
                retryable,
                correlation_id,
                recovery,
            },
        ))
        .await;
    }

    async fn send_success<T: serde::Serialize>(&self, id: Option<RequestId>, result: T) -> bool {
        match id {
            Some(id) => {
                self.send_value(JsonRpcSuccessResponse::new(id, result))
                    .await
            }
            None => true,
        }
    }

    async fn send_notification<T: serde::Serialize>(
        &self,
        method: &'static str,
        params: T,
    ) -> bool {
        self.send_value(JsonRpcNotification::new(method, params))
            .await
    }

    async fn send_value<T: serde::Serialize>(&self, value: T) -> bool {
        let Ok(value) = serde_json::to_value(value) else {
            return false;
        };
        self.inner.output.send(value).await.is_ok()
    }
}

fn event_turn_id(event: &AgenticEvent) -> Option<&str> {
    match event {
        AgenticEvent::DialogTurnCompleted { turn_id, .. }
        | AgenticEvent::DialogTurnCancelled { turn_id, .. }
        | AgenticEvent::DialogTurnFailed { turn_id, .. }
        | AgenticEvent::TextChunk { turn_id, .. } => Some(turn_id),
        _ => None,
    }
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}

fn project_query_event(event: &AgenticEvent) -> Option<QueryEvent> {
    match event {
        AgenticEvent::TextChunk { text, .. } => {
            Some(QueryEvent::AssistantTextDelta { text: text.clone() })
        }
        _ => None,
    }
}

fn terminal_fact(
    event: &AgenticEvent,
    expected_turn_id: &str,
    query_id: &str,
) -> Option<(QueryTerminalStatus, Option<QueryResultError>)> {
    match event {
        AgenticEvent::DialogTurnCompleted {
            turn_id,
            success,
            finish_reason,
            has_final_response,
            ..
        } if turn_id == expected_turn_id => {
            if success == &Some(false) || has_final_response == &Some(false) {
                Some((
                    QueryTerminalStatus::Failed,
                    Some(QueryResultError::new(
                        ErrorCode::Internal,
                        false,
                        None,
                        query_id,
                        format!(
                            "Query completed unsuccessfully: {}",
                            finish_reason
                                .as_deref()
                                .unwrap_or("unsuccessful_completion")
                        ),
                    )),
                ))
            } else {
                Some((QueryTerminalStatus::Completed, None))
            }
        }
        AgenticEvent::DialogTurnCancelled { turn_id, .. } if turn_id == expected_turn_id => {
            Some((QueryTerminalStatus::Cancelled, None))
        }
        AgenticEvent::DialogTurnFailed {
            turn_id,
            error,
            error_category,
            error_detail,
            ..
        } if turn_id == expected_turn_id => Some((
            QueryTerminalStatus::Failed,
            Some(query_error_from_failure(
                query_id,
                error,
                error_category.as_ref(),
                error_detail.as_ref().and_then(|detail| detail.retryable),
            )),
        )),
        _ => None,
    }
}

fn query_error_from_failure(
    correlation_id: &str,
    message: &str,
    category: Option<&ErrorCategory>,
    explicit_retryable: Option<bool>,
) -> QueryResultError {
    let (code, default_retryable, recovery) = match category {
        Some(ErrorCategory::Network | ErrorCategory::ProviderUnavailable) => (
            ErrorCode::ProviderUnavailable,
            true,
            Some(RecoveryAction::Retry),
        ),
        Some(ErrorCategory::Auth) => (ErrorCode::Authentication, false, None),
        Some(ErrorCategory::RateLimit) => {
            (ErrorCode::RateLimited, true, Some(RecoveryAction::Retry))
        }
        Some(ErrorCategory::ContextOverflow) => (ErrorCode::ContextOverflow, false, None),
        Some(ErrorCategory::Timeout) => (ErrorCode::Timeout, true, Some(RecoveryAction::Retry)),
        Some(ErrorCategory::ProviderQuota) => (ErrorCode::ProviderQuota, false, None),
        Some(ErrorCategory::ProviderBilling) => (ErrorCode::ProviderBilling, false, None),
        Some(ErrorCategory::Permission) => (ErrorCode::PermissionDenied, false, None),
        Some(ErrorCategory::InvalidRequest) => (ErrorCode::InvalidRequest, false, None),
        Some(ErrorCategory::ContentPolicy) => (ErrorCode::ContentPolicy, false, None),
        Some(ErrorCategory::ModelError | ErrorCategory::Unknown) | None => {
            (ErrorCode::Internal, false, None)
        }
    };
    QueryResultError::new(
        code,
        explicit_retryable.unwrap_or(default_retryable),
        recovery,
        correlation_id,
        message,
    )
}

fn query_error_from_runtime(
    query_id: &str,
    error: RuntimeError,
    context: &str,
) -> QueryResultError {
    let (code, retryable, recovery) = runtime_error_facts(&error);
    QueryResultError::new(
        code,
        retryable,
        recovery,
        query_id,
        format!("{context}: {}", error.into_message()),
    )
}

fn runtime_error_facts(error: &RuntimeError) -> (ErrorCode, bool, Option<RecoveryAction>) {
    match error {
        RuntimeError::Port(port) => match port.kind {
            PortErrorKind::NotAvailable => (ErrorCode::CapabilityUnavailable, false, None),
            PortErrorKind::NotFound => (ErrorCode::NotFound, false, None),
            PortErrorKind::InvalidRequest => (ErrorCode::InvalidRequest, false, None),
            PortErrorKind::PermissionDenied => (ErrorCode::PermissionDenied, false, None),
            PortErrorKind::Cancelled => (ErrorCode::Cancelled, false, None),
            PortErrorKind::Timeout => (ErrorCode::Timeout, true, Some(RecoveryAction::Retry)),
            PortErrorKind::CleanupRequired => (
                ErrorCode::CleanupRequired,
                false,
                Some(RecoveryAction::RestartHost),
            ),
            PortErrorKind::Backend => {
                (ErrorCode::Internal, true, Some(RecoveryAction::RestartHost))
            }
        },
        RuntimeError::PermissionRequest(_) => (ErrorCode::PermissionDenied, false, None),
        _ => (ErrorCode::CapabilityUnavailable, false, None),
    }
}

fn permission_request_targets_query(
    request: &PermissionRequest,
    dialog_turn_id: Option<&str>,
    lease: &QueryLease,
) -> bool {
    (request.session_id == lease.session_id && dialog_turn_id == Some(lease.turn_id.as_str()))
        || request.delegation.as_ref().is_some_and(|delegation| {
            delegation.parent_session_id == lease.session_id
                && delegation.parent_dialog_turn_id.as_deref() == Some(lease.turn_id.as_str())
        })
}

fn runtime_error_kind(error: &RuntimeError) -> &'static str {
    match error {
        RuntimeError::Port(port) => match port.kind {
            PortErrorKind::NotAvailable => "not_available",
            PortErrorKind::NotFound => "not_found",
            PortErrorKind::InvalidRequest => "invalid_request",
            PortErrorKind::PermissionDenied => "permission_denied",
            PortErrorKind::Cancelled => "cancelled",
            PortErrorKind::Timeout => "timeout",
            PortErrorKind::CleanupRequired => "cleanup_required",
            PortErrorKind::Backend => "backend",
        },
        RuntimeError::MissingDialogTurnPort
        | RuntimeError::MissingLifecycleDeliveryPort
        | RuntimeError::MissingCancellationPort
        | RuntimeError::MissingSessionManagementPort
        | RuntimeError::MissingSessionRestorePort
        | RuntimeError::MissingLocalCommandTurnPort
        | RuntimeError::MissingSessionTranscriptReader
        | RuntimeError::MissingThreadGoalManagementPort
        | RuntimeError::MissingInteractionResponsePort
        | RuntimeError::MissingEventSink
        | RuntimeError::MissingEventSource
        | RuntimeError::MissingPermissionRequestManager => "capability_unavailable",
        RuntimeError::PermissionRequest(_) => "permission_request",
    }
}
