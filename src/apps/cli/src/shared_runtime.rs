use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use bitfun_agent_runtime::sdk::{
    AgentRuntime, AgentSessionRestoreRequest, AgentUserAnswersRequest, DialogSubmitOutcome,
    PermissionRequest, PermissionRequestEvent, PortErrorKind, RuntimeError,
    SessionTranscriptRequest,
};
use bitfun_agent_runtime_ipc::{
    DiscoveryStore, RuntimeInstanceIdentity, RuntimeIpcClient, RuntimeIpcError,
    RuntimeIpcErrorCode, RuntimeIpcEvent, RuntimeIpcOperation, RuntimeIpcOperationResult,
    RuntimeIpcRequestHandler, RuntimeIpcServer, RuntimeIpcServerConfig,
    RuntimeIpcStreamInvalidationReason, PROTOCOL_VERSION,
};
use bitfun_core::runtime_ownership::CoreRuntimeOwnership;
use bitfun_events::{AgenticEvent, ToolEventData};
use bitfun_services_core::runtime_ownership::RuntimeDeployment;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, watch, Notify};

const RELEASE_CHANNEL: &str = "stable";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(45);
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const SERVER_OPERATION_TIMEOUT: Duration = Duration::from_secs(120);
const CLIENT_REQUEST_TIMEOUT: Duration = Duration::from_secs(125);
const EVENT_BUFFER: usize = 256;
const SUBAGENT_ROUTE_TIMEOUT: Duration = Duration::from_secs(2);
type SessionEventSenders = Mutex<HashMap<String, broadcast::Sender<RuntimeIpcEvent>>>;

pub(crate) struct SharedRuntimeHandler {
    runtime: AgentRuntime,
    workspace: PathBuf,
    events: Arc<SessionEventSenders>,
    question_sessions: Arc<Mutex<HashMap<String, String>>>,
    subagent_routes: Arc<Mutex<HashMap<String, (String, String, String)>>>,
    event_stream_available: watch::Sender<bool>,
}

impl SharedRuntimeHandler {
    pub(crate) fn build(runtime: AgentRuntime, workspace: &Path) -> Result<Self> {
        let mut agent_events = runtime
            .subscribe_events()
            .map_err(runtime_error_message)
            .context("subscribe Shared Runtime agent events")?;
        let mut permission_events = runtime
            .subscribe_permission_requests()
            .map_err(runtime_error_message)
            .context("subscribe Shared Runtime permission events")?;
        let events = Arc::new(Mutex::new(HashMap::new()));
        let permission_sessions = Arc::new(Mutex::new(HashMap::new()));
        let question_sessions = Arc::new(Mutex::new(HashMap::new()));
        let subagent_routes = Arc::new(Mutex::new(
            HashMap::<String, (String, String, String)>::new(),
        ));
        let route_updates = Arc::new(Notify::new());
        let (event_stream_available, _) = watch::channel(true);

        let agent_output = events.clone();
        let agent_questions = question_sessions.clone();
        let agent_routes = subagent_routes.clone();
        let agent_route_updates = route_updates.clone();
        let agent_stream_available = event_stream_available.clone();
        tokio::spawn(async move {
            loop {
                match agent_events.recv().await {
                    Ok(mut envelope) => {
                        let Some(source_session_id) =
                            envelope.event.session_id().map(ToOwned::to_owned)
                        else {
                            continue;
                        };
                        let (session_id, routed_turn_id, routed_tool_call_id) =
                            route_agent_event(&envelope.event, &source_session_id, &agent_routes);
                        project_subagent_link_route(
                            &mut envelope.event,
                            &session_id,
                            routed_turn_id.as_deref(),
                            routed_tool_call_id.as_deref(),
                        );
                        project_user_question_route(
                            &mut envelope.event,
                            &session_id,
                            routed_turn_id.as_deref(),
                        );
                        if matches!(envelope.event, AgenticEvent::SubagentSessionLinked { .. }) {
                            agent_route_updates.notify_waiters();
                        }
                        index_user_question(&envelope.event, &session_id, &agent_questions);
                        publish_event(
                            &agent_output,
                            &session_id,
                            RuntimeIpcEvent::Agent {
                                session_id: session_id.clone(),
                                envelope,
                            },
                        );
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        invalidate_event_stream(
                            &agent_stream_available,
                            &agent_output,
                            RuntimeIpcStreamInvalidationReason::Lagged,
                        );
                        break;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        invalidate_event_stream(
                            &agent_stream_available,
                            &agent_output,
                            RuntimeIpcStreamInvalidationReason::Closed,
                        );
                        break;
                    }
                }
            }
        });

        let permission_output = events.clone();
        let permission_index = permission_sessions.clone();
        let permission_routes = subagent_routes.clone();
        let permission_route_updates = route_updates.clone();
        let permission_stream_available = event_stream_available.clone();
        tokio::spawn(async move {
            loop {
                let event = match permission_events.recv().await {
                    Ok(event) => event,
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        invalidate_event_stream(
                            &permission_stream_available,
                            &permission_output,
                            RuntimeIpcStreamInvalidationReason::Lagged,
                        );
                        break;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        invalidate_event_stream(
                            &permission_stream_available,
                            &permission_output,
                            RuntimeIpcStreamInvalidationReason::Closed,
                        );
                        break;
                    }
                };
                if let PermissionRequestEvent::Asked { request } = &event {
                    if !await_permission_route(
                        request,
                        &permission_routes,
                        &permission_route_updates,
                    )
                    .await
                    {
                        invalidate_event_stream(
                            &permission_stream_available,
                            &permission_output,
                            RuntimeIpcStreamInvalidationReason::Closed,
                        );
                        break;
                    }
                }
                let session_id =
                    permission_event_session(&event, &permission_index, &permission_routes);
                if let Some(session_id) = session_id {
                    publish_event(
                        &permission_output,
                        &session_id,
                        RuntimeIpcEvent::Permission {
                            session_id: session_id.clone(),
                            event,
                        },
                    );
                }
            }
        });

        Ok(Self {
            runtime,
            workspace: dunce::canonicalize(workspace)
                .context("canonicalize Shared Runtime workspace")?,
            events,
            question_sessions,
            subagent_routes,
            event_stream_available,
        })
    }
}

#[async_trait]
impl RuntimeIpcRequestHandler for SharedRuntimeHandler {
    fn ensure_available(&self) -> std::result::Result<(), RuntimeIpcError> {
        (*self.event_stream_available.borrow())
            .then_some(())
            .ok_or_else(event_stream_unavailable_error)
    }

    fn subscribe_availability(&self) -> Option<watch::Receiver<bool>> {
        Some(self.event_stream_available.subscribe())
    }

    async fn execute(
        &self,
        operation: RuntimeIpcOperation,
    ) -> std::result::Result<RuntimeIpcOperationResult, RuntimeIpcError> {
        self.validate_workspace(&operation)?;
        match operation {
            RuntimeIpcOperation::Health => unreachable!("Health is owned by the IPC server"),
            RuntimeIpcOperation::ListSessions { request } => self
                .runtime
                .list_sessions(request)
                .await
                .map(|sessions| RuntimeIpcOperationResult::Sessions { sessions })
                .map_err(runtime_ipc_error),
            RuntimeIpcOperation::CreateSession { request } => self
                .runtime
                .create_session(request)
                .await
                .map(|session| RuntimeIpcOperationResult::SessionCreated { session })
                .map_err(runtime_ipc_error),
            RuntimeIpcOperation::RestoreSession { request } => {
                let restored = self
                    .runtime
                    .restore_session(AgentSessionRestoreRequest {
                        workspace_path: request.workspace_path,
                        session_id: request.session_id,
                        include_internal: false,
                        remote_connection_id: None,
                        remote_ssh_host: None,
                    })
                    .await
                    .map_err(runtime_ipc_error)?;
                let transcript = self
                    .runtime
                    .read_session_transcript(SessionTranscriptRequest {
                        session_id: restored.session.session_id.clone(),
                        turn_id: None,
                    })
                    .await
                    .map_err(runtime_ipc_error)?;
                let pending_permissions = self
                    .runtime
                    .pending_permission_requests()
                    .map_err(runtime_ipc_error)?
                    .into_iter()
                    .filter(|request| {
                        permission_targets_session(
                            request,
                            &restored.session.session_id,
                            &self.subagent_routes,
                        )
                    })
                    .collect();
                Ok(RuntimeIpcOperationResult::SessionRestored {
                    session: restored.session,
                    transcript,
                    pending_permissions,
                })
            }
            RuntimeIpcOperation::UpdateSessionMode { request } => {
                self.runtime
                    .update_session_mode(request)
                    .await
                    .map_err(runtime_ipc_error)?;
                Ok(RuntimeIpcOperationResult::Unit)
            }
            RuntimeIpcOperation::UpdateSessionModel { request } => {
                self.runtime
                    .update_session_model(request)
                    .await
                    .map_err(runtime_ipc_error)?;
                Ok(RuntimeIpcOperationResult::Unit)
            }
            RuntimeIpcOperation::SubmitTurn { request } => {
                let outcome = self
                    .runtime
                    .submit_dialog_turn(request)
                    .await
                    .map_err(runtime_ipc_error)?;
                let (session_id, turn_id) = match outcome {
                    DialogSubmitOutcome::Started {
                        session_id,
                        turn_id,
                    }
                    | DialogSubmitOutcome::Queued {
                        session_id,
                        turn_id,
                    } => (session_id, turn_id),
                };
                Ok(RuntimeIpcOperationResult::TurnAccepted {
                    session_id,
                    turn_id,
                })
            }
            RuntimeIpcOperation::CancelTurn { request } => self
                .runtime
                .cancel_turn(request)
                .await
                .map(|cancellation| RuntimeIpcOperationResult::TurnCancelled { cancellation })
                .map_err(runtime_ipc_error),
            RuntimeIpcOperation::PendingPermissions { session_id } => {
                let requests = self
                    .runtime
                    .pending_permission_requests()
                    .map_err(runtime_ipc_error)?
                    .into_iter()
                    .filter(|request| {
                        permission_targets_session(request, &session_id, &self.subagent_routes)
                    })
                    .collect();
                Ok(RuntimeIpcOperationResult::PendingPermissions { requests })
            }
            RuntimeIpcOperation::RespondPermission {
                session_id,
                request_id,
                reply,
            } => {
                let permitted = self
                    .runtime
                    .pending_permission_requests()
                    .map_err(runtime_ipc_error)?
                    .iter()
                    .any(|request| {
                        request.request_id == request_id
                            && permission_targets_session(
                                request,
                                &session_id,
                                &self.subagent_routes,
                            )
                    });
                if !permitted {
                    return Err(RuntimeIpcError {
                        code: RuntimeIpcErrorCode::SessionMismatch,
                        message: "permission request does not belong to the controlled session"
                            .to_string(),
                    });
                }
                self.runtime
                    .respond_permission(&request_id, reply)
                    .await
                    .map_err(runtime_ipc_error)?;
                Ok(RuntimeIpcOperationResult::Unit)
            }
            RuntimeIpcOperation::SubmitUserAnswers { request } => {
                let permitted = self
                    .question_sessions
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .get(&request.tool_id)
                    .is_some_and(|session_id| session_id == &request.session_id);
                if !permitted {
                    return Err(RuntimeIpcError {
                        code: RuntimeIpcErrorCode::SessionMismatch,
                        message: "user-input request does not belong to the controlled session"
                            .to_string(),
                    });
                }
                self.runtime
                    .submit_user_answers(AgentUserAnswersRequest {
                        tool_id: request.tool_id.clone(),
                        answers: request.answers,
                    })
                    .await
                    .map_err(runtime_ipc_error)?;
                self.question_sessions
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .remove(&request.tool_id);
                Ok(RuntimeIpcOperationResult::Unit)
            }
        }
    }

    fn subscribe_events(
        &self,
        session_id: &str,
    ) -> std::result::Result<broadcast::Receiver<RuntimeIpcEvent>, RuntimeIpcError> {
        subscribe_session_events(&self.events, &self.event_stream_available, session_id)
    }
}

fn subscribe_session_events(
    events: &SessionEventSenders,
    available: &watch::Sender<bool>,
    session_id: &str,
) -> std::result::Result<broadcast::Receiver<RuntimeIpcEvent>, RuntimeIpcError> {
    let mut events = events
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !*available.borrow() {
        return Err(event_stream_unavailable_error());
    }
    events.retain(|_, sender| sender.receiver_count() > 0);
    Ok(events
        .entry(session_id.to_string())
        .or_insert_with(|| broadcast::channel(EVENT_BUFFER).0)
        .subscribe())
}

fn event_stream_unavailable_error() -> RuntimeIpcError {
    RuntimeIpcError {
        code: RuntimeIpcErrorCode::Unavailable,
        message: "Shared Runtime event stream is unavailable; restart Shared TUI".to_string(),
    }
}

fn publish_event(events: &SessionEventSenders, session_id: &str, event: RuntimeIpcEvent) {
    if let Some(sender) = events
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(session_id)
    {
        let _ = sender.send(event);
    }
}

fn invalidate_event_stream(
    available: &watch::Sender<bool>,
    events: &SessionEventSenders,
    reason: RuntimeIpcStreamInvalidationReason,
) {
    if available.send_replace(false) {
        for sender in events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
        {
            let _ = sender.send(RuntimeIpcEvent::StreamInvalidated { reason });
        }
    }
}

async fn await_permission_route(
    request: &PermissionRequest,
    routes: &Mutex<HashMap<String, (String, String, String)>>,
    updates: &Notify,
) -> bool {
    if request.delegation.is_none() {
        return true;
    }
    let deadline = tokio::time::Instant::now() + SUBAGENT_ROUTE_TIMEOUT;
    loop {
        let updated = updates.notified();
        if routes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains_key(&request.session_id)
        {
            return true;
        }
        if tokio::time::timeout_at(deadline, updated).await.is_err() {
            return false;
        }
    }
}

impl SharedRuntimeHandler {
    fn validate_workspace(
        &self,
        operation: &RuntimeIpcOperation,
    ) -> std::result::Result<(), RuntimeIpcError> {
        let requested = match operation {
            RuntimeIpcOperation::ListSessions { request } => Some(request.workspace_path.as_str()),
            RuntimeIpcOperation::CreateSession { request } => Some(
                request
                    .workspace_path
                    .as_deref()
                    .ok_or_else(workspace_mismatch_error)?,
            ),
            RuntimeIpcOperation::RestoreSession { request } => {
                Some(request.workspace_path.as_str())
            }
            RuntimeIpcOperation::SubmitTurn { request } => Some(
                request
                    .workspace_path
                    .as_deref()
                    .ok_or_else(workspace_mismatch_error)?,
            ),
            _ => None,
        };
        let Some(requested) = requested else {
            return Ok(());
        };
        let matches = dunce::canonicalize(Path::new(requested))
            .is_ok_and(|requested| requested == self.workspace);
        if matches {
            Ok(())
        } else {
            Err(workspace_mismatch_error())
        }
    }
}

fn workspace_mismatch_error() -> RuntimeIpcError {
    RuntimeIpcError {
        code: RuntimeIpcErrorCode::SessionMismatch,
        message: "Shared TUI operation targets a different workspace".to_string(),
    }
}

fn route_agent_event(
    event: &AgenticEvent,
    source_session_id: &str,
    routes: &Mutex<HashMap<String, (String, String, String)>>,
) -> (String, Option<String>, Option<String>) {
    let mut routes = routes
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let AgenticEvent::SubagentSessionLinked {
        session_id,
        parent_session_id,
        parent_dialog_turn_id,
        parent_tool_call_id,
        ..
    } = event
    {
        let root_route = routes.get(parent_session_id).cloned().unwrap_or_else(|| {
            (
                parent_session_id.clone(),
                parent_dialog_turn_id.clone(),
                parent_tool_call_id.clone(),
            )
        });
        routes.insert(session_id.clone(), root_route);
    }
    let routed = routes
        .get(source_session_id)
        .cloned()
        .map(|(session_id, turn_id, tool_call_id)| (session_id, Some(turn_id), Some(tool_call_id)))
        .unwrap_or_else(|| (source_session_id.to_string(), None, None));
    if let AgenticEvent::DialogTurnCompleted {
        session_id,
        turn_id,
        ..
    }
    | AgenticEvent::DialogTurnCancelled {
        session_id,
        turn_id,
    }
    | AgenticEvent::DialogTurnFailed {
        session_id,
        turn_id,
        ..
    } = event
    {
        routes.retain(|_, (parent_session_id, parent_turn_id, _)| {
            parent_session_id != session_id || parent_turn_id != turn_id
        });
    }
    routed
}

fn project_subagent_link_route(
    event: &mut AgenticEvent,
    routed_session_id: &str,
    routed_turn_id: Option<&str>,
    routed_tool_call_id: Option<&str>,
) {
    let AgenticEvent::SubagentSessionLinked {
        parent_session_id,
        parent_dialog_turn_id,
        parent_tool_call_id,
        ..
    } = event
    else {
        return;
    };
    *parent_session_id = routed_session_id.to_string();
    if let Some(routed_turn_id) = routed_turn_id {
        *parent_dialog_turn_id = routed_turn_id.to_string();
    }
    if let Some(routed_tool_call_id) = routed_tool_call_id {
        *parent_tool_call_id = routed_tool_call_id.to_string();
    }
}

fn project_user_question_route(
    event: &mut AgenticEvent,
    routed_session_id: &str,
    routed_turn_id: Option<&str>,
) {
    let AgenticEvent::ToolEvent {
        session_id,
        turn_id,
        tool_event,
        ..
    } = event
    else {
        return;
    };
    if tool_event.effective_tool_name() == "AskUserQuestion" {
        *session_id = routed_session_id.to_string();
        if let Some(routed_turn_id) = routed_turn_id {
            *turn_id = routed_turn_id.to_string();
        }
    }
}

fn index_user_question(
    event: &AgenticEvent,
    routed_session_id: &str,
    questions: &Mutex<HashMap<String, String>>,
) {
    let AgenticEvent::ToolEvent { tool_event, .. } = event else {
        return;
    };
    let mut questions = questions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match tool_event {
        ToolEventData::Started { .. } if tool_event.effective_tool_name() == "AskUserQuestion" => {
            questions.insert(
                tool_event.tool_id().to_string(),
                routed_session_id.to_string(),
            );
        }
        ToolEventData::Completed { .. }
        | ToolEventData::Failed { .. }
        | ToolEventData::Cancelled { .. } => {
            questions.remove(tool_event.tool_id());
        }
        _ => {}
    }
}

pub(crate) async fn run_service(workspace: PathBuf, expected_identity: String) -> Result<()> {
    bitfun_services_core::process_manager::contain_current_process_tree()
        .context("contain Shared Runtime process tree")?;
    prepare_client_environment().await?;
    let identity = instance_identity(&workspace)?;
    if identity.as_str() != expected_identity {
        return Err(anyhow!(
            "Shared Runtime identity does not match its workspace"
        ));
    }
    let runtime = crate::initialize_core_services_for_deployment(
        &workspace,
        crate::runtime::approval::CliApprovalPolicy::Ask,
        crate::BootstrapProfile::Interactive,
        RuntimeDeployment::Shared,
    )
    .await?;
    let handler = Arc::new(SharedRuntimeHandler::build(
        runtime.agent_runtime().clone(),
        &workspace,
    )?);
    let server = RuntimeIpcServer::bind_with_handler(
        &ipc_root()?,
        identity,
        RuntimeIpcServerConfig {
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            idle_timeout: IDLE_TIMEOUT,
            handshake_timeout: CONNECT_TIMEOUT,
            request_timeout: SERVER_OPERATION_TIMEOUT,
            max_connections: 64,
        },
        handler,
    )
    .await
    .context("bind Shared Runtime IPC")?;
    let result = server.serve().await.context("serve Shared Runtime IPC");
    crate::shutdown_mcp_servers().await;
    result
}

pub(crate) async fn connect_or_start(workspace: &Path) -> Result<RuntimeIpcClient> {
    prepare_client_environment().await?;
    let identity = instance_identity(workspace)?;
    let runtime_root = ipc_root()?;
    let store = DiscoveryStore::new(&runtime_root, identity.clone());
    let client_id = uuid::Uuid::new_v4().to_string();
    let mut last_connect_error = None;
    match connect_existing(&store, &runtime_root, &client_id).await {
        Ok(Some(client)) => return require_interactive_tui(client),
        Ok(None) => {}
        Err(error) => last_connect_error = Some(error),
    }

    let mut child = StartupChild::spawn(workspace, identity.as_str())?;
    let mut started = Instant::now();
    let mut respawned = false;
    loop {
        match connect_existing(&store, &runtime_root, &client_id).await {
            Ok(Some(client)) => {
                let client = require_interactive_tui(client)?;
                child.disarm();
                return Ok(client);
            }
            Ok(None) => {}
            Err(error) => last_connect_error = Some(error),
        }
        if let Some(status) = child.try_wait().context("poll Shared Runtime startup")? {
            if embedded_runtime_owner_present(workspace)? {
                return Err(anyhow!(
                    "Agent Runtime ownership failed (runtime_ownership_unavailable): an Embedded Runtime owns this workspace; close it before starting Shared TUI ({status})"
                ));
            }
            if runtime_owner_present(workspace)? {
                // Another Shared child may still be initializing and has not
                // published discovery yet. Keep connecting until the normal
                // bounded startup timeout instead of mislabeling it Embedded.
            } else {
                if respawned {
                    return Err(anyhow!(
                        "Shared Runtime exited before becoming ready ({status})"
                    ));
                }
                child = StartupChild::spawn(workspace, identity.as_str())?;
                respawned = true;
                started = Instant::now();
            }
        }
        if started.elapsed() >= STARTUP_TIMEOUT {
            let owner_guidance = if runtime_owner_present(workspace)? {
                "; Agent Runtime ownership failed (runtime_ownership_unavailable): another local Runtime still owns this workspace, so close its clients and wait up to 30 seconds"
            } else {
                ""
            };
            let connection_detail = last_connect_error
                .as_ref()
                .map(|error| format!("; last connection error: {error}"))
                .unwrap_or_default();
            return Err(anyhow!(
                "Shared Runtime did not become ready within {} seconds{owner_guidance}{connection_detail}",
                STARTUP_TIMEOUT.as_secs()
            ));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn runtime_owner_present(workspace: &Path) -> Result<bool> {
    CoreRuntimeOwnership::runtime_owner_present(path_manager()?.as_ref(), workspace)
        .map_err(anyhow::Error::from)
}

fn embedded_runtime_owner_present(workspace: &Path) -> Result<bool> {
    CoreRuntimeOwnership::embedded_runtime_owner_present(path_manager()?.as_ref(), workspace)
        .map_err(anyhow::Error::from)
}

fn require_interactive_tui(client: RuntimeIpcClient) -> Result<RuntimeIpcClient> {
    if client.capabilities().interactive_tui {
        Ok(client)
    } else {
        Err(anyhow!(
            "local Runtime does not support Shared TUI operations"
        ))
    }
}

async fn prepare_client_environment() -> Result<()> {
    crate::agent::agentic_system::select_agentic_system_profile(
        bitfun_core::product_assembly::DeliveryProfile::Cli,
    )?;
    bitfun_core::service::config::initialize_global_config()
        .await
        .map_err(|error| anyhow!("Failed to initialize Shared TUI configuration: {error}"))
}

async fn connect_existing(
    store: &DiscoveryStore,
    runtime_root: &Path,
    client_id: &str,
) -> Result<Option<RuntimeIpcClient>> {
    let Some(discovery) = store.read().context("read Shared Runtime discovery")? else {
        return Ok(None);
    };
    RuntimeIpcClient::connect(
        runtime_root,
        &discovery,
        client_id,
        env!("CARGO_PKG_VERSION"),
        CONNECT_TIMEOUT,
        CLIENT_REQUEST_TIMEOUT,
    )
    .await
    .context("connect existing Shared Runtime")
    .map(Some)
}

struct StartupChild {
    child: Option<Child>,
}

impl StartupChild {
    fn spawn(workspace: &Path, identity: &str) -> Result<Self> {
        let executable = std::env::current_exe().context("resolve BitFun executable")?;
        let mut command = bitfun_services_core::process_manager::create_command(executable);
        command
            .arg("__shared-runtime")
            .arg("--workspace")
            .arg(workspace)
            .arg("--instance-identity")
            .arg(identity)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_detached_process(&mut command);
        let child = command.spawn().context("start Shared Runtime process")?;
        Ok(Self { child: Some(child) })
    }

    fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.child
            .as_mut()
            .expect("startup child is armed")
            .try_wait()
    }

    fn disarm(mut self) {
        self.child.take();
    }
}

impl Drop for StartupChild {
    fn drop(&mut self) {
        let Some(child) = self.child.as_mut() else {
            return;
        };
        #[cfg(unix)]
        if let Ok(process_id) = i32::try_from(child.id()) {
            // SAFETY: the child calls setsid before exec, so its PID is the
            // process-group ID owned by this startup attempt.
            let _ = unsafe { libc::kill(-process_id, libc::SIGKILL) };
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn configure_detached_process(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    }
    #[cfg(windows)]
    let _ = command;
}

fn instance_identity(workspace: &Path) -> Result<RuntimeInstanceIdentity> {
    let user_root = path_manager()?.user_data_dir();
    RuntimeInstanceIdentity::for_workspace(
        workspace,
        CoreRuntimeOwnership::distribution_identity(),
        RELEASE_CHANNEL,
        &user_root.to_string_lossy(),
        PROTOCOL_VERSION,
    )
    .context("resolve Shared Runtime identity")
}

fn ipc_root() -> Result<PathBuf> {
    Ok(path_manager()?
        .user_data_dir()
        .join("agent-runtime")
        .join(format!("ipc-v{PROTOCOL_VERSION}")))
}

fn path_manager() -> Result<Arc<bitfun_core::infrastructure::PathManager>> {
    bitfun_core::infrastructure::try_get_path_manager_arc()
        .map_err(|error| anyhow!(error.to_string()))
}

fn permission_targets_session(
    request: &PermissionRequest,
    session_id: &str,
    routes: &Mutex<HashMap<String, (String, String, String)>>,
) -> bool {
    permission_request_session(request, routes) == session_id
}

fn permission_event_session(
    event: &PermissionRequestEvent,
    index: &Mutex<HashMap<String, String>>,
    routes: &Mutex<HashMap<String, (String, String, String)>>,
) -> Option<String> {
    let mut index = index
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match event {
        PermissionRequestEvent::Asked { request } => {
            let session_id = permission_request_session(request, routes);
            index.insert(request.request_id.clone(), session_id.clone());
            Some(session_id)
        }
        PermissionRequestEvent::Replied { request_id, .. }
        | PermissionRequestEvent::Cancelled { request_id, .. } => index.remove(request_id),
    }
}

fn permission_request_session(
    request: &PermissionRequest,
    routes: &Mutex<HashMap<String, (String, String, String)>>,
) -> String {
    let routes = routes
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    routes
        .get(&request.session_id)
        .or_else(|| {
            request
                .delegation
                .as_ref()
                .and_then(|delegation| routes.get(&delegation.parent_session_id))
        })
        .map(|(root_session_id, _, _)| root_session_id.clone())
        .or_else(|| {
            request
                .delegation
                .as_ref()
                .map(|delegation| delegation.parent_session_id.clone())
        })
        .unwrap_or_else(|| request.session_id.clone())
}

fn runtime_error_message(error: RuntimeError) -> anyhow::Error {
    anyhow!(error.into_message())
}

fn runtime_ipc_error(error: RuntimeError) -> RuntimeIpcError {
    let code = match &error {
        RuntimeError::Port(port_error) if port_error.kind == PortErrorKind::SessionInUse => {
            RuntimeIpcErrorCode::SessionInUse
        }
        RuntimeError::Port(port_error) if port_error.kind == PortErrorKind::InvalidRequest => {
            RuntimeIpcErrorCode::InvalidRequest
        }
        _ => RuntimeIpcErrorCode::Unavailable,
    };
    RuntimeIpcError {
        code,
        message: error.into_message(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        await_permission_route, connect_existing, index_user_question, invalidate_event_stream,
        permission_event_session, permission_targets_session, project_subagent_link_route,
        project_user_question_route, publish_event, route_agent_event, runtime_ipc_error,
        subscribe_session_events, SessionEventSenders, EVENT_BUFFER,
    };
    use bitfun_agent_runtime::sdk::{
        PermissionDelegationContext, PermissionReplySource, PermissionRequest,
        PermissionRequestEvent, PermissionRequestSource, PermissionRequestSourceKind, PortError,
        PortErrorKind, RuntimeError,
    };
    use bitfun_events::{AgenticEvent, ToolEventData, ToolEventIdentity};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::sync::{watch, Notify};

    #[test]
    fn session_writer_conflict_reuses_the_existing_ipc_error() {
        let error = runtime_ipc_error(RuntimeError::Port(PortError::new(
            PortErrorKind::SessionInUse,
            "Session is already open for writing: session-1",
        )));

        assert_eq!(
            error.code,
            bitfun_agent_runtime_ipc::RuntimeIpcErrorCode::SessionInUse
        );
    }

    #[test]
    fn invalid_runtime_requests_keep_their_ipc_error_category() {
        let error = runtime_ipc_error(RuntimeError::Port(PortError::new(
            PortErrorKind::InvalidRequest,
            "Unknown agent mode: missing",
        )));

        assert_eq!(
            error.code,
            bitfun_agent_runtime_ipc::RuntimeIpcErrorCode::InvalidRequest
        );
    }

    #[tokio::test]
    async fn existing_runtime_connection_errors_are_not_hidden_as_absence() {
        let root = tempfile::tempdir().unwrap();
        let identity = bitfun_agent_runtime_ipc::RuntimeInstanceIdentity::for_workspace(
            root.path(),
            "bitfun",
            "stable",
            "fixture-user",
            bitfun_agent_runtime_ipc::PROTOCOL_VERSION,
        )
        .unwrap();
        let store = bitfun_agent_runtime_ipc::DiscoveryStore::new(root.path(), identity.clone());
        store
            .write(&bitfun_agent_runtime_ipc::DiscoveryRecord::new(
                identity,
                "invalid-endpoint".to_string(),
                1,
                "token".to_string(),
                "owner".to_string(),
            ))
            .unwrap();
        assert!(connect_existing(&store, root.path(), "client")
            .await
            .is_err());
    }

    #[test]
    fn exited_shared_child_reports_embedded_owner_without_waiting_for_timeout() {
        let source = include_str!("shared_runtime.rs");
        let exited_child = source
            .split_once("if let Some(status) = child.try_wait()")
            .expect("Shared Runtime child exit branch")
            .1
            .split_once("if started.elapsed() >= STARTUP_TIMEOUT")
            .expect("startup timeout boundary")
            .0;

        assert!(exited_child.contains("embedded_runtime_owner_present"));
        assert!(exited_child.contains("runtime_ownership_unavailable"));
        assert!(exited_child.contains("Embedded Runtime owns this workspace"));
        assert!(exited_child.contains("return Err"));
    }

    fn delegated_permission(session_id: &str, parent_session_id: &str) -> PermissionRequest {
        PermissionRequest {
            request_id: "permission-1".to_string(),
            round_id: "round-1".to_string(),
            order: 0,
            tool_call_id: Some("tool-1".to_string()),
            project_path: None,
            project_id: "project-1".to_string(),
            session_id: session_id.to_string(),
            agent_id: "agentic".to_string(),
            action: "run command".to_string(),
            resources: Vec::new(),
            save_resources: Vec::new(),
            source: PermissionRequestSource {
                kind: PermissionRequestSourceKind::ToolCall,
                identity: "shell".to_string(),
            },
            delegation: Some(PermissionDelegationContext {
                parent_session_id: parent_session_id.to_string(),
                parent_dialog_turn_id: Some("parent-turn".to_string()),
                parent_tool_call_id: "task-1".to_string(),
                subagent_type: "general".to_string(),
            }),
            display_metadata: serde_json::Map::new(),
        }
    }

    #[test]
    fn unrelated_session_events_do_not_consume_a_clients_lag_budget() {
        let events = SessionEventSenders::new(HashMap::new());
        let (available, _) = watch::channel(true);
        let _noisy = subscribe_session_events(&events, &available, "noisy").unwrap();
        let mut quiet = subscribe_session_events(&events, &available, "quiet").unwrap();
        for _ in 0..=EVENT_BUFFER {
            publish_event(
                &events,
                "noisy",
                bitfun_agent_runtime_ipc::RuntimeIpcEvent::StreamInvalidated {
                    reason: bitfun_agent_runtime_ipc::RuntimeIpcStreamInvalidationReason::Lagged,
                },
            );
        }
        assert!(matches!(
            quiet.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
        invalidate_event_stream(
            &available,
            &events,
            bitfun_agent_runtime_ipc::RuntimeIpcStreamInvalidationReason::Lagged,
        );
        assert!(quiet.try_recv().is_ok());
        assert!(subscribe_session_events(&events, &available, "late").is_err());
    }

    #[test]
    fn subagent_events_route_to_the_parent_until_its_turn_finishes() {
        let routes = Mutex::new(HashMap::new());
        let root_route = (
            "parent-session".to_string(),
            Some("parent-turn".to_string()),
            Some("delegate-tool".to_string()),
        );
        let linked = AgenticEvent::SubagentSessionLinked {
            session_id: "child-session".to_string(),
            subagent_dialog_turn_id: "child-turn".to_string(),
            parent_session_id: "parent-session".to_string(),
            parent_dialog_turn_id: "parent-turn".to_string(),
            parent_tool_call_id: "delegate-tool".to_string(),
            agent_type: None,
            model_id: None,
            focused_review_display_label: None,
        };
        assert_eq!(
            route_agent_event(&linked, "child-session", &routes),
            root_route
        );

        let mut nested = AgenticEvent::SubagentSessionLinked {
            session_id: "grandchild-session".to_string(),
            subagent_dialog_turn_id: "grandchild-turn".to_string(),
            parent_session_id: "child-session".to_string(),
            parent_dialog_turn_id: "child-turn".to_string(),
            parent_tool_call_id: "nested-tool".to_string(),
            agent_type: None,
            model_id: None,
            focused_review_display_label: None,
        };
        let nested_route = route_agent_event(&nested, "grandchild-session", &routes);
        assert_eq!(nested_route, root_route);
        project_subagent_link_route(
            &mut nested,
            &nested_route.0,
            nested_route.1.as_deref(),
            nested_route.2.as_deref(),
        );
        assert!(matches!(
            nested,
            AgenticEvent::SubagentSessionLinked {
                parent_session_id,
                parent_dialog_turn_id,
                parent_tool_call_id,
                ..
            } if parent_session_id == "parent-session"
                && parent_dialog_turn_id == "parent-turn"
                && parent_tool_call_id == "delegate-tool"
        ));
        let grandchild_output = AgenticEvent::TextChunk {
            session_id: "grandchild-session".to_string(),
            turn_id: "grandchild-turn".to_string(),
            round_id: "round-2".to_string(),
            attempt_id: None,
            attempt_index: None,
            text: "nested output".to_string(),
        };
        assert_eq!(
            route_agent_event(&grandchild_output, "grandchild-session", &routes),
            root_route
        );

        let completed = AgenticEvent::DialogTurnCompleted {
            session_id: "parent-session".to_string(),
            turn_id: "parent-turn".to_string(),
            total_rounds: 1,
            total_tools: 1,
            duration_ms: 1,
            partial_recovery_reason: None,
            success: Some(true),
            finish_reason: None,
            has_final_response: Some(true),
        };
        route_agent_event(&completed, "parent-session", &routes);
        assert!(routes.lock().expect("routes").is_empty());
    }

    #[test]
    fn nested_subagent_permissions_route_to_the_root_controller() {
        let root_route = (
            "root-session".to_string(),
            "root-turn".to_string(),
            "root-tool".to_string(),
        );
        let routes = Mutex::new(HashMap::from([
            ("child-session".to_string(), root_route.clone()),
            ("nested-session".to_string(), root_route),
        ]));
        let index = Mutex::new(HashMap::new());
        let request = delegated_permission("nested-session", "child-session");

        assert!(permission_targets_session(
            &request,
            "root-session",
            &routes
        ));
        let events = [
            PermissionRequestEvent::Asked {
                request: request.clone(),
            },
            PermissionRequestEvent::Replied {
                request_id: request.request_id,
                reply: bitfun_agent_runtime::sdk::PermissionReply::Once,
                source: PermissionReplySource::User,
            },
        ];
        for event in events {
            assert_eq!(
                permission_event_session(&event, &index, &routes).as_deref(),
                Some("root-session")
            );
        }
    }

    #[tokio::test]
    async fn delegated_permission_waits_for_its_authoritative_subagent_route() {
        let routes = Arc::new(Mutex::new(HashMap::new()));
        let updates = Arc::new(Notify::new());
        let request = delegated_permission("child-session", "root-session");
        let waiting_routes = routes.clone();
        let waiting_updates = updates.clone();
        let waiting_request = request.clone();
        let waiting = tokio::spawn(async move {
            await_permission_route(&waiting_request, &waiting_routes, &waiting_updates).await
        });

        tokio::time::sleep(Duration::from_millis(10)).await;
        routes.lock().expect("routes").insert(
            "child-session".to_string(),
            (
                "root-session".to_string(),
                "root-turn".to_string(),
                "root-tool".to_string(),
            ),
        );
        updates.notify_waiters();

        assert!(waiting.await.expect("route waiter"));
        assert!(permission_targets_session(
            &request,
            "root-session",
            &routes
        ));
    }

    #[test]
    fn user_question_answers_remain_scoped_to_the_routed_parent_session() {
        let questions = Mutex::new(HashMap::new());
        let mut started = AgenticEvent::ToolEvent {
            session_id: "child-session".to_string(),
            turn_id: "child-turn".to_string(),
            round_id: "round-1".to_string(),
            attempt_id: None,
            attempt_index: None,
            tool_event: ToolEventData::Started {
                identity: ToolEventIdentity::direct("question-1", "AskUserQuestion"),
                params: serde_json::json!({}),
                timeout_seconds: None,
            },
        };
        project_user_question_route(&mut started, "parent-session", Some("parent-turn"));
        assert!(matches!(
            &started,
            AgenticEvent::ToolEvent { session_id, turn_id, .. }
                if session_id == "parent-session" && turn_id == "parent-turn"
        ));
        index_user_question(&started, "parent-session", &questions);
        assert_eq!(
            questions
                .lock()
                .expect("question index")
                .get("question-1")
                .map(String::as_str),
            Some("parent-session")
        );
    }
}
