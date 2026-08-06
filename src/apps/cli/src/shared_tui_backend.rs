//! CLI Host compatibility adapter from the private Shared Runtime IPC to TUI v2.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::tui_backend::{TuiBackend, TuiBackendError};
use async_trait::async_trait;
use bitfun_agent_runtime_ipc::{
    RuntimeIpcClient, RuntimeIpcClientError, RuntimeIpcClientEvent, RuntimeIpcErrorCode,
    RuntimeIpcEvent, RuntimeIpcOperation, RuntimeIpcOperationResult,
    RuntimeIpcStreamInvalidationReason, RuntimeSessionForkRequest, RuntimeSessionProcessingPhase,
    RuntimeSessionRenameRequest, RuntimeSessionRestoreRequest, RuntimeSessionState,
    RuntimeUserAnswersRequest,
};
use bitfun_app_server_client::AppServerEvent;
use bitfun_app_server_protocol::app::{
    CapabilityAvailability, CapabilityDescriptor, HealthResponse, HealthStatus, InitializeRequest,
    InitializeResponse, ServerInfo, TransportLimits,
};
use bitfun_app_server_protocol::event::{
    AgentEventNotification, EventCursor, EventStream, EventStreamState,
    EventStreamStateNotification, PermissionEventNotification, ResyncDirective,
};
use bitfun_app_server_protocol::tui::*;
use bitfun_app_server_protocol::{MIN_PROTOCOL_VERSION, PROTOCOL_VERSION};
use bitfun_runtime_ports::{
    AgentSessionCompactionResult, AgentSessionForkResult, AgentUserShellCommandResult,
};
use tokio::sync::broadcast;

use crate::agent::tui_client::{TuiAgentMode, TuiHostCapabilities};

const EVENT_BUFFER: usize = 256;

pub(crate) struct SharedTuiBackend {
    client: RuntimeIpcClient,
    current_session_id: Arc<Mutex<Option<String>>>,
    events: broadcast::Sender<AppServerEvent>,
}

pub(crate) struct SharedTuiHostCapabilities {
    client: RuntimeIpcClient,
}

impl SharedTuiHostCapabilities {
    pub(crate) fn new(client: RuntimeIpcClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl TuiHostCapabilities for SharedTuiHostCapabilities {
    async fn available_agent_modes(
        &self,
        session_id: Option<String>,
        _workspace: std::path::PathBuf,
    ) -> anyhow::Result<Vec<TuiAgentMode>> {
        match self
            .client
            .request(RuntimeIpcOperation::ListAgentModes { session_id })
            .await?
        {
            RuntimeIpcOperationResult::AgentModes { modes } => Ok(modes
                .into_iter()
                .map(|mode| TuiAgentMode {
                    id: mode.id,
                    description: mode.description,
                    model_id: mode.model_id,
                    is_external: mode.is_external,
                })
                .collect()),
            other => Err(anyhow::anyhow!(
                "Shared Runtime returned an unexpected mode catalog result: {other:?}"
            )),
        }
    }
}

impl SharedTuiBackend {
    pub(crate) fn new(client: RuntimeIpcClient) -> Self {
        let (events, _) = broadcast::channel(EVENT_BUFFER);
        let connection_id = format!("shared-runtime-{}", uuid::Uuid::new_v4());
        spawn_event_bridge(client.subscribe_events(), events.clone(), connection_id);
        Self {
            client,
            current_session_id: Arc::new(Mutex::new(None)),
            events,
        }
    }

    fn set_current_session(&self, session_id: impl Into<String>) {
        *self
            .current_session_id
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(session_id.into());
    }

    fn current_session(&self) -> Result<String, TuiBackendError> {
        self.current_session_id
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .ok_or_else(|| backend_error("Shared TUI has no attached Session", false))
    }

    async fn request(
        &self,
        operation: RuntimeIpcOperation,
    ) -> Result<RuntimeIpcOperationResult, TuiBackendError> {
        self.client
            .request(operation)
            .await
            .map_err(map_client_error)
    }
}

#[async_trait]
impl TuiBackend for SharedTuiBackend {
    async fn initialize(
        &self,
        request: InitializeRequest,
    ) -> Result<InitializeResponse, TuiBackendError> {
        if request.protocol_version < MIN_PROTOCOL_VERSION
            || request.protocol_version > PROTOCOL_VERSION
        {
            return Err(backend_error(
                format!(
                    "Unsupported TUI protocol {}, expected {}",
                    request.protocol_version, PROTOCOL_VERSION
                ),
                false,
            ));
        }
        if !self.client.capabilities().interactive_tui {
            return Err(backend_error(
                "Shared Runtime does not advertise interactive TUI support",
                false,
            ));
        }
        Ok(InitializeResponse::new(
            ServerInfo {
                name: "bitfun-shared-runtime-host-adapter".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            tui_capabilities(),
            TransportLimits {
                max_frame_bytes: 16 * 1024 * 1024,
                event_buffer_capacity: EVENT_BUFFER as u32,
            },
        ))
    }

    async fn model_catalog(&self) -> Result<TuiModelCatalogResponse, TuiBackendError> {
        load_model_catalog().await
    }

    async fn health(&self) -> Result<HealthResponse, TuiBackendError> {
        self.client.health().await.map_err(map_client_error)?;
        Ok(HealthResponse {
            status: HealthStatus::Ready,
            protocol_version: PROTOCOL_VERSION,
        })
    }

    fn subscribe_events(&self) -> broadcast::Receiver<AppServerEvent> {
        self.events.subscribe()
    }

    async fn list_sessions(
        &self,
        request: ListSessionsRequest,
    ) -> Result<ListSessionsResponse, TuiBackendError> {
        match self
            .request(RuntimeIpcOperation::ListSessions { request: request.0 })
            .await?
        {
            RuntimeIpcOperationResult::Sessions { sessions } => {
                Ok(ListSessionsResponse { sessions })
            }
            other => Err(unexpected("list_sessions", other)),
        }
    }

    async fn sync_session(
        &self,
        request: SyncSessionRequest,
    ) -> Result<SyncSessionResponse, TuiBackendError> {
        let requested_session_id = request.session_id.clone();
        match self
            .request(RuntimeIpcOperation::RestoreSession {
                request: RuntimeSessionRestoreRequest {
                    workspace_path: request.workspace_path,
                    session_id: requested_session_id.clone(),
                },
            })
            .await?
        {
            RuntimeIpcOperationResult::SessionRestored {
                session,
                state,
                workspace_binding,
                transcript,
                pending_permissions,
            } => {
                self.set_current_session(requested_session_id);
                Ok(SyncSessionResponse {
                    session,
                    state: map_session_state(state),
                    transcript,
                    workspace_binding,
                    pending_permissions,
                })
            }
            other => Err(unexpected("sync_session", other)),
        }
    }

    async fn create_session(
        &self,
        request: CreateSessionRequest,
    ) -> Result<CreateSessionResponse, TuiBackendError> {
        match self
            .request(RuntimeIpcOperation::CreateSession { request: request.0 })
            .await?
        {
            RuntimeIpcOperationResult::SessionCreated { session } => {
                self.set_current_session(session.session_id.clone());
                Ok(CreateSessionResponse(session))
            }
            other => Err(unexpected("create_session", other)),
        }
    }

    async fn delete_session(
        &self,
        request: DeleteSessionRequest,
    ) -> Result<DeleteSessionResponse, TuiBackendError> {
        expect_unit(
            self.request(RuntimeIpcOperation::DeleteSession {
                session_id: request.0.session_id,
            })
            .await?,
            "delete_session",
        )?;
        Ok(DeleteSessionResponse {})
    }

    async fn rename_session(
        &self,
        request: RenameSessionRequest,
    ) -> Result<RenameSessionResponse, TuiBackendError> {
        expect_unit(
            self.request(RuntimeIpcOperation::RenameSession {
                request: RuntimeSessionRenameRequest {
                    session_id: request.0.session_id,
                    session_name: request.0.session_name,
                },
            })
            .await?,
            "rename_session",
        )?;
        Ok(RenameSessionResponse {})
    }

    async fn submit_dialog_turn(
        &self,
        request: SubmitDialogTurnRequest,
    ) -> Result<SubmitDialogTurnResponse, TuiBackendError> {
        match self
            .request(RuntimeIpcOperation::SubmitTurn { request: request.0 })
            .await?
        {
            RuntimeIpcOperationResult::TurnAccepted {
                session_id,
                turn_id,
            } => Ok(SubmitDialogTurnResponse::Started {
                session_id,
                turn_id,
            }),
            other => Err(unexpected("submit_dialog_turn", other)),
        }
    }

    async fn cancel_turn(
        &self,
        request: CancelTurnRequest,
    ) -> Result<CancelTurnResponse, TuiBackendError> {
        match self
            .request(RuntimeIpcOperation::CancelTurn { request: request.0 })
            .await?
        {
            RuntimeIpcOperationResult::TurnCancelled { cancellation } => {
                Ok(CancelTurnResponse(cancellation))
            }
            other => Err(unexpected("cancel_turn", other)),
        }
    }

    async fn steer_turn(
        &self,
        request: SteerTurnRequest,
    ) -> Result<SteerTurnResponse, TuiBackendError> {
        match self
            .request(RuntimeIpcOperation::SteerTurn { request: request.0 })
            .await?
        {
            RuntimeIpcOperationResult::TurnSteered { steering_id, .. } => {
                Ok(SteerTurnResponse { steering_id })
            }
            other => Err(unexpected("steer_turn", other)),
        }
    }

    async fn run_user_shell_command(
        &self,
        request: RunUserShellCommandRequest,
    ) -> Result<RunUserShellCommandResponse, TuiBackendError> {
        match self
            .request(RuntimeIpcOperation::RunUserShellCommand { request: request.0 })
            .await?
        {
            RuntimeIpcOperationResult::TurnAccepted {
                session_id,
                turn_id,
            } => Ok(RunUserShellCommandResponse(AgentUserShellCommandResult {
                session_id,
                turn_id,
            })),
            other => Err(unexpected("run_user_shell_command", other)),
        }
    }

    async fn submit_user_answers(
        &self,
        request: SubmitUserAnswersRequest,
    ) -> Result<SubmitUserAnswersResponse, TuiBackendError> {
        expect_unit(
            self.request(RuntimeIpcOperation::SubmitUserAnswers {
                request: RuntimeUserAnswersRequest {
                    session_id: self.current_session()?,
                    tool_id: request.tool_id,
                    answers: request.answers,
                },
            })
            .await?,
            "submit_user_answers",
        )?;
        Ok(SubmitUserAnswersResponse {})
    }

    async fn record_local_command_turn(
        &self,
        request: RecordLocalCommandTurnRequest,
    ) -> Result<RecordLocalCommandTurnResponse, TuiBackendError> {
        match self
            .request(RuntimeIpcOperation::RecordLocalCommandTurn { request: request.0 })
            .await?
        {
            RuntimeIpcOperationResult::LocalCommandTurnRecorded { record } => {
                Ok(RecordLocalCommandTurnResponse(record))
            }
            other => Err(unexpected("record_local_command_turn", other)),
        }
    }

    async fn respond_permission(
        &self,
        request: RespondPermissionRequest,
    ) -> Result<RespondPermissionResponse, TuiBackendError> {
        expect_unit(
            self.request(RuntimeIpcOperation::RespondPermission {
                session_id: self.current_session()?,
                request_id: request.request_id,
                reply: request.reply,
            })
            .await?,
            "respond_permission",
        )?;
        Ok(RespondPermissionResponse {})
    }

    async fn pending_permissions(&self) -> Result<PendingPermissionsResponse, TuiBackendError> {
        let Some(session_id) = self
            .current_session_id
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
        else {
            return Ok(PendingPermissionsResponse {
                requests: Vec::new(),
            });
        };
        match self
            .request(RuntimeIpcOperation::PendingPermissions { session_id })
            .await?
        {
            RuntimeIpcOperationResult::PendingPermissions { requests } => {
                Ok(PendingPermissionsResponse { requests })
            }
            other => Err(unexpected("pending_permissions", other)),
        }
    }

    async fn compact_session(
        &self,
        request: CompactSessionRequest,
    ) -> Result<CompactSessionResponse, TuiBackendError> {
        match self
            .request(RuntimeIpcOperation::CompactSession { request: request.0 })
            .await?
        {
            RuntimeIpcOperationResult::TurnAccepted {
                session_id,
                turn_id,
            } => Ok(CompactSessionResponse(AgentSessionCompactionResult {
                session_id,
                turn_id,
            })),
            other => Err(unexpected("compact_session", other)),
        }
    }

    async fn undo_session(
        &self,
        request: UndoSessionRequest,
    ) -> Result<RevertSessionResponse, TuiBackendError> {
        map_revert(
            self.request(RuntimeIpcOperation::UndoSession { request: request.0 })
                .await?,
            "undo_session",
        )
    }

    async fn redo_session(
        &self,
        request: RedoSessionRequest,
    ) -> Result<RevertSessionResponse, TuiBackendError> {
        map_revert(
            self.request(RuntimeIpcOperation::RedoSession { request: request.0 })
                .await?,
            "redo_session",
        )
    }

    async fn reload_context(
        &self,
        request: ReloadContextRequest,
    ) -> Result<ReloadContextResponse, TuiBackendError> {
        expect_unit(
            self.request(RuntimeIpcOperation::ReloadSessionContext { request: request.0 })
                .await?,
            "reload_context",
        )?;
        Ok(ReloadContextResponse {})
    }

    async fn session_usage(
        &self,
        request: SessionUsageRequest,
    ) -> Result<SessionUsageResponse, TuiBackendError> {
        match self
            .request(RuntimeIpcOperation::SessionUsage { request: request.0 })
            .await?
        {
            RuntimeIpcOperationResult::SessionUsage { usage } => Ok(SessionUsageResponse(usage)),
            other => Err(unexpected("session_usage", other)),
        }
    }

    async fn wait_for_settlement(
        &self,
        request: WaitForSettlementRequest,
    ) -> Result<WaitForSettlementResponse, TuiBackendError> {
        expect_unit(
            self.request(RuntimeIpcOperation::WaitForSettlement { request: request.0 })
                .await?,
            "wait_for_settlement",
        )?;
        Ok(WaitForSettlementResponse {})
    }

    async fn workspace_diff(&self) -> Result<WorkspaceDiffResponse, TuiBackendError> {
        match self.request(RuntimeIpcOperation::WorkspaceDiff).await? {
            RuntimeIpcOperationResult::WorkspaceDiff { snapshot } => {
                Ok(WorkspaceDiffResponse(snapshot))
            }
            other => Err(unexpected("workspace_diff", other)),
        }
    }

    async fn search_workspace_references(
        &self,
        request: SearchWorkspaceReferencesRequest,
    ) -> Result<SearchWorkspaceReferencesResponse, TuiBackendError> {
        match self
            .request(RuntimeIpcOperation::SearchWorkspaceReferences { request: request.0 })
            .await?
        {
            RuntimeIpcOperationResult::WorkspaceReferenceSearch { search } => {
                Ok(SearchWorkspaceReferencesResponse(search))
            }
            other => Err(unexpected("search_workspace_references", other)),
        }
    }

    async fn message_references(
        &self,
        request: MessageReferencesRequest,
    ) -> Result<MessageReferencesResponse, TuiBackendError> {
        match self
            .request(RuntimeIpcOperation::WorkspaceReferencesForMessage { request: request.0 })
            .await?
        {
            RuntimeIpcOperationResult::WorkspaceReferences { references } => {
                Ok(MessageReferencesResponse(references))
            }
            other => Err(unexpected("message_references", other)),
        }
    }

    async fn session_lineage(
        &self,
        request: SessionLineageRequest,
    ) -> Result<SessionLineageResponse, TuiBackendError> {
        match self
            .request(RuntimeIpcOperation::GetSessionLineage { request: request.0 })
            .await?
        {
            RuntimeIpcOperationResult::SessionLineage { snapshot } => {
                Ok(SessionLineageResponse(snapshot))
            }
            other => Err(unexpected("session_lineage", other)),
        }
    }

    async fn inspect_lineage(
        &self,
        request: InspectLineageRequest,
    ) -> Result<InspectLineageResponse, TuiBackendError> {
        match self
            .request(RuntimeIpcOperation::InspectLineageSession { request: request.0 })
            .await?
        {
            RuntimeIpcOperationResult::LineageSessionInspection { inspection } => {
                Ok(InspectLineageResponse(inspection))
            }
            other => Err(unexpected("inspect_lineage", other)),
        }
    }

    async fn cancel_lineage(
        &self,
        request: CancelLineageRequest,
    ) -> Result<CancelLineageResponse, TuiBackendError> {
        match self
            .request(RuntimeIpcOperation::CancelLineageSession { request: request.0 })
            .await?
        {
            RuntimeIpcOperationResult::TurnCancelled { cancellation } => {
                Ok(CancelLineageResponse(cancellation))
            }
            other => Err(unexpected("cancel_lineage", other)),
        }
    }

    async fn fork_session(
        &self,
        request: ForkSessionRequest,
    ) -> Result<ForkSessionResponse, TuiBackendError> {
        self.fork(request.0.source_session_id, None).await
    }

    async fn fork_session_before_turn(
        &self,
        request: ForkSessionBeforeTurnRequest,
    ) -> Result<ForkSessionResponse, TuiBackendError> {
        self.fork(request.0.source_session_id, Some(request.0.source_turn_id))
            .await
    }

    async fn update_session_model(
        &self,
        request: UpdateSessionModelRequest,
    ) -> Result<UpdateSessionModelResponse, TuiBackendError> {
        expect_unit(
            self.request(RuntimeIpcOperation::UpdateSessionModel { request: request.0 })
                .await?,
            "update_session_model",
        )?;
        Ok(UpdateSessionModelResponse {})
    }

    async fn update_session_mode(
        &self,
        request: UpdateSessionModeRequest,
    ) -> Result<UpdateSessionModeResponse, TuiBackendError> {
        expect_unit(
            self.request(RuntimeIpcOperation::UpdateSessionMode { request: request.0 })
                .await?,
            "update_session_mode",
        )?;
        Ok(UpdateSessionModeResponse {})
    }
}

impl SharedTuiBackend {
    async fn fork(
        &self,
        source_session_id: String,
        before_turn_id: Option<String>,
    ) -> Result<ForkSessionResponse, TuiBackendError> {
        match self
            .request(RuntimeIpcOperation::ForkSession {
                request: RuntimeSessionForkRequest {
                    session_id: source_session_id,
                    before_turn_id,
                },
            })
            .await?
        {
            RuntimeIpcOperationResult::SessionForked { session, .. } => {
                self.set_current_session(session.session_id.clone());
                Ok(ForkSessionResponse(AgentSessionForkResult {
                    session_id: session.session_id,
                    session_name: session.session_name,
                    agent_type: session.agent_type,
                }))
            }
            other => Err(unexpected("fork_session", other)),
        }
    }
}

async fn load_model_catalog() -> Result<TuiModelCatalogResponse, TuiBackendError> {
    let catalog = bitfun_core::get_ai_model_catalog()
        .await
        .map_err(|message| backend_error(message, false))?;
    let reasoning_presets_by_model = catalog
        .models
        .into_iter()
        .filter_map(|model| {
            model.reasoning.map(|reasoning| {
                (
                    model.id,
                    reasoning
                        .presets
                        .into_iter()
                        .map(|preset| preset.id)
                        .collect(),
                )
            })
        })
        .collect();
    Ok(TuiModelCatalogResponse {
        provider_catalog: catalog.provider_catalog,
        reasoning_presets_by_model,
    })
}

fn map_revert(
    result: RuntimeIpcOperationResult,
    operation: &str,
) -> Result<RevertSessionResponse, TuiBackendError> {
    match result {
        RuntimeIpcOperationResult::SessionReverted { revert } => Ok(RevertSessionResponse(revert)),
        other => Err(unexpected(operation, other)),
    }
}

fn expect_unit(result: RuntimeIpcOperationResult, operation: &str) -> Result<(), TuiBackendError> {
    match result {
        RuntimeIpcOperationResult::Unit => Ok(()),
        other => Err(unexpected(operation, other)),
    }
}

fn unexpected(operation: &str, result: RuntimeIpcOperationResult) -> TuiBackendError {
    backend_error(
        format!("Shared Runtime returned an unexpected result for {operation}: {result:?}"),
        true,
    )
}

fn map_client_error(error: RuntimeIpcClientError) -> TuiBackendError {
    let outcome_unknown = matches!(
        &error,
        RuntimeIpcClientError::Remote(remote)
            if remote.code == RuntimeIpcErrorCode::OutcomeUnknown
    ) || matches!(
        error,
        RuntimeIpcClientError::Timeout
            | RuntimeIpcClientError::Disconnected
            | RuntimeIpcClientError::UnexpectedResponse
            | RuntimeIpcClientError::Io(_)
    );
    backend_error(error.to_string(), outcome_unknown)
}

fn backend_error(message: impl Into<String>, outcome_unknown: bool) -> TuiBackendError {
    TuiBackendError {
        message: message.into(),
        outcome_unknown,
    }
}

fn map_session_state(state: RuntimeSessionState) -> SessionRuntimeState {
    match state {
        RuntimeSessionState::Idle => SessionRuntimeState::Idle,
        RuntimeSessionState::Processing {
            current_turn_id,
            phase,
        } => SessionRuntimeState::Processing {
            current_turn_id,
            phase: match phase {
                RuntimeSessionProcessingPhase::Starting => SessionProcessingPhase::Starting,
                RuntimeSessionProcessingPhase::Compacting => SessionProcessingPhase::Compacting,
                RuntimeSessionProcessingPhase::Thinking => SessionProcessingPhase::Thinking,
                RuntimeSessionProcessingPhase::Streaming => SessionProcessingPhase::Streaming,
                RuntimeSessionProcessingPhase::ToolCalling => SessionProcessingPhase::ToolCalling,
                RuntimeSessionProcessingPhase::ToolConfirming => {
                    SessionProcessingPhase::ToolConfirming
                }
            },
        },
        RuntimeSessionState::Error { error, recoverable } => {
            SessionRuntimeState::Error { error, recoverable }
        }
    }
}

fn tui_capabilities() -> Vec<CapabilityDescriptor> {
    [
        (
            "agent",
            vec![
                "agent/createSession",
                "agent/listSessions",
                "agent/deleteSession",
                "agent/submitDialogTurn",
                "agent/steerTurn",
                "agent/runUserShellCommand",
                "agent/submitUserAnswers",
                "agent/cancelTurn",
                "agent/event",
            ],
        ),
        (
            "session",
            vec![
                "session/sync",
                "session/recordLocalCommandTurn",
                "session/rename",
                "session/updateModel",
                "session/updateMode",
                "session/fork",
                "session/forkBeforeTurn",
                "session/compact",
                "session/undo",
                "session/redo",
                "session/reloadContext",
                "session/usage",
                "session/waitForSettlement",
                "session/lineage",
                "session/inspectLineage",
                "session/cancelLineage",
            ],
        ),
        (
            "permission",
            vec![
                "agent/permissionEvent",
                "agent/respondPermission",
                "agent/listPendingPermissionRequests",
            ],
        ),
        (
            "workspace",
            vec![
                "workspace/diff",
                "workspace/searchReferences",
                "workspace/messageReferences",
            ],
        ),
    ]
    .into_iter()
    .map(|(id, methods)| CapabilityDescriptor {
        id: id.to_string(),
        availability: CapabilityAvailability::Available,
        methods: methods.into_iter().map(str::to_string).collect(),
    })
    .collect()
}

fn spawn_event_bridge(
    mut source: broadcast::Receiver<RuntimeIpcClientEvent>,
    output: broadcast::Sender<AppServerEvent>,
    connection_id: String,
) {
    tokio::spawn(async move {
        let agent_sequence = AtomicU64::new(0);
        let permission_sequence = AtomicU64::new(0);
        loop {
            match source.recv().await {
                Ok(RuntimeIpcClientEvent::Runtime(RuntimeIpcEvent::Agent { envelope, .. })) => {
                    let _ = output.send(AppServerEvent::Agent(AgentEventNotification {
                        cursor: next_cursor(&connection_id, EventStream::Agent, &agent_sequence),
                        event: envelope,
                    }));
                }
                Ok(RuntimeIpcClientEvent::Runtime(RuntimeIpcEvent::Permission {
                    event, ..
                })) => {
                    let _ = output.send(AppServerEvent::Permission(PermissionEventNotification {
                        cursor: next_cursor(
                            &connection_id,
                            EventStream::Permission,
                            &permission_sequence,
                        ),
                        event,
                    }));
                }
                Ok(RuntimeIpcClientEvent::Runtime(RuntimeIpcEvent::StreamInvalidated {
                    reason,
                })) => {
                    let _ = output.send(stream_state_event(
                        &connection_id,
                        &agent_sequence,
                        EventStreamState::Invalidated,
                        Some(invalidation_reason(reason)),
                    ));
                    break;
                }
                Ok(RuntimeIpcClientEvent::Disconnected)
                | Err(broadcast::error::RecvError::Closed) => {
                    let _ = output.send(AppServerEvent::ConnectionClosed);
                    break;
                }
                Err(broadcast::error::RecvError::Lagged(missed)) => {
                    let mut event = stream_state_event(
                        &connection_id,
                        &agent_sequence,
                        EventStreamState::Lagged,
                        Some("Shared Runtime client event receiver lagged".to_string()),
                    );
                    if let AppServerEvent::StreamState(notification) = &mut event {
                        notification.missed = Some(missed);
                    }
                    let _ = output.send(event);
                    break;
                }
            }
        }
    });
}

fn next_cursor(connection_id: &str, stream: EventStream, sequence: &AtomicU64) -> EventCursor {
    EventCursor {
        connection_id: connection_id.to_string(),
        stream,
        sequence: sequence.fetch_add(1, Ordering::Relaxed) + 1,
    }
}

fn stream_state_event(
    connection_id: &str,
    sequence: &AtomicU64,
    state: EventStreamState,
    reason: Option<String>,
) -> AppServerEvent {
    AppServerEvent::StreamState(EventStreamStateNotification {
        cursor: next_cursor(connection_id, EventStream::Agent, sequence),
        stream: EventStream::Agent,
        state,
        missed: None,
        resync: ResyncDirective {
            method: "session/sync".to_string(),
            snapshot_available: true,
            reason,
        },
    })
}

fn invalidation_reason(reason: RuntimeIpcStreamInvalidationReason) -> String {
    match reason {
        RuntimeIpcStreamInvalidationReason::Lagged => "Shared Runtime event stream lagged",
        RuntimeIpcStreamInvalidationReason::Closed => "Shared Runtime event stream closed",
        RuntimeIpcStreamInvalidationReason::FrameTooLarge => {
            "Shared Runtime event exceeded the transport frame limit"
        }
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_events::{AgenticEvent, AgenticEventEnvelope, AgenticEventPriority};

    fn agent_event(text: &str) -> RuntimeIpcClientEvent {
        RuntimeIpcClientEvent::Runtime(RuntimeIpcEvent::Agent {
            session_id: "session-1".to_string(),
            envelope: AgenticEventEnvelope::new(
                AgenticEvent::TextChunk {
                    session_id: "session-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    round_id: "round-1".to_string(),
                    attempt_id: None,
                    attempt_index: None,
                    text: text.to_string(),
                },
                AgenticEventPriority::Normal,
            ),
        })
    }

    #[tokio::test]
    async fn event_bridge_preserves_monotonic_cursors_and_projects_invalidation() {
        let (source, source_rx) = broadcast::channel(8);
        let (output, mut output_rx) = broadcast::channel(8);
        spawn_event_bridge(source_rx, output, "connection-1".to_string());

        source.send(agent_event("one")).expect("first event");
        source.send(agent_event("two")).expect("second event");
        source
            .send(RuntimeIpcClientEvent::Runtime(
                RuntimeIpcEvent::StreamInvalidated {
                    reason: RuntimeIpcStreamInvalidationReason::Lagged,
                },
            ))
            .expect("invalidation");

        for expected in [1, 2] {
            let AppServerEvent::Agent(notification) = output_rx.recv().await.expect("agent event")
            else {
                panic!("expected agent event");
            };
            assert_eq!(notification.cursor.connection_id, "connection-1");
            assert_eq!(notification.cursor.stream, EventStream::Agent);
            assert_eq!(notification.cursor.sequence, expected);
        }
        let AppServerEvent::StreamState(notification) =
            output_rx.recv().await.expect("stream invalidation")
        else {
            panic!("expected stream state");
        };
        assert_eq!(notification.cursor.sequence, 3);
        assert_eq!(notification.state, EventStreamState::Invalidated);
        assert_eq!(notification.resync.method, "session/sync");
        assert!(notification.resync.snapshot_available);
    }

    #[tokio::test]
    async fn event_bridge_projects_disconnect_as_connection_closed() {
        let (source, source_rx) = broadcast::channel(2);
        let (output, mut output_rx) = broadcast::channel(2);
        spawn_event_bridge(source_rx, output, "connection-2".to_string());

        source
            .send(RuntimeIpcClientEvent::Disconnected)
            .expect("disconnect event");

        assert!(matches!(
            output_rx.recv().await.expect("connection closed"),
            AppServerEvent::ConnectionClosed
        ));
    }
}
