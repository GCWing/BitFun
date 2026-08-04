//! CLI-local App Server boundary for the interactive TUI.

use async_trait::async_trait;
use bitfun_app_server_client::{AppServerClient, AppServerEvent, ClientError};
use bitfun_app_server_protocol::app::{HealthResponse, InitializeRequest, InitializeResponse};
use bitfun_app_server_protocol::tui::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum TuiEffectRoute {
    Local,
    AppServer,
    HostCapability,
}

pub(crate) trait TuiEffect {
    fn route(&self) -> TuiEffectRoute;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TuiBackendError {
    pub message: String,
    pub outcome_unknown: bool,
}

impl std::fmt::Display for TuiBackendError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for TuiBackendError {}

#[async_trait]
#[allow(dead_code)]
pub(crate) trait TuiBackend: Send + Sync {
    async fn initialize(
        &self,
        request: InitializeRequest,
    ) -> Result<InitializeResponse, TuiBackendError>;

    async fn health(&self) -> Result<HealthResponse, TuiBackendError>;

    fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<AppServerEvent>;

    async fn list_sessions(
        &self,
        request: ListSessionsRequest,
    ) -> Result<ListSessionsResponse, TuiBackendError>;
    async fn sync_session(
        &self,
        request: SyncSessionRequest,
    ) -> Result<SyncSessionResponse, TuiBackendError>;
    async fn create_session(
        &self,
        request: CreateSessionRequest,
    ) -> Result<CreateSessionResponse, TuiBackendError>;
    async fn delete_session(
        &self,
        request: DeleteSessionRequest,
    ) -> Result<DeleteSessionResponse, TuiBackendError>;
    async fn rename_session(
        &self,
        request: RenameSessionRequest,
    ) -> Result<RenameSessionResponse, TuiBackendError>;
    async fn submit_dialog_turn(
        &self,
        request: SubmitDialogTurnRequest,
    ) -> Result<SubmitDialogTurnResponse, TuiBackendError>;
    async fn cancel_turn(
        &self,
        request: CancelTurnRequest,
    ) -> Result<CancelTurnResponse, TuiBackendError>;
    async fn steer_turn(
        &self,
        request: SteerTurnRequest,
    ) -> Result<SteerTurnResponse, TuiBackendError>;
    async fn run_user_shell_command(
        &self,
        request: RunUserShellCommandRequest,
    ) -> Result<RunUserShellCommandResponse, TuiBackendError>;
    async fn submit_user_answers(
        &self,
        request: SubmitUserAnswersRequest,
    ) -> Result<SubmitUserAnswersResponse, TuiBackendError>;
    async fn record_local_command_turn(
        &self,
        request: RecordLocalCommandTurnRequest,
    ) -> Result<RecordLocalCommandTurnResponse, TuiBackendError>;
    async fn respond_permission(
        &self,
        request: RespondPermissionRequest,
    ) -> Result<RespondPermissionResponse, TuiBackendError>;
    async fn pending_permissions(&self) -> Result<PendingPermissionsResponse, TuiBackendError>;
    async fn compact_session(
        &self,
        request: CompactSessionRequest,
    ) -> Result<CompactSessionResponse, TuiBackendError>;
    async fn undo_session(
        &self,
        request: UndoSessionRequest,
    ) -> Result<RevertSessionResponse, TuiBackendError>;
    async fn redo_session(
        &self,
        request: RedoSessionRequest,
    ) -> Result<RevertSessionResponse, TuiBackendError>;
    async fn reload_context(
        &self,
        request: ReloadContextRequest,
    ) -> Result<ReloadContextResponse, TuiBackendError>;
    async fn session_usage(
        &self,
        request: SessionUsageRequest,
    ) -> Result<SessionUsageResponse, TuiBackendError>;
    async fn wait_for_settlement(
        &self,
        request: WaitForSettlementRequest,
    ) -> Result<WaitForSettlementResponse, TuiBackendError>;
    async fn workspace_diff(&self) -> Result<WorkspaceDiffResponse, TuiBackendError>;
    async fn search_workspace_references(
        &self,
        request: SearchWorkspaceReferencesRequest,
    ) -> Result<SearchWorkspaceReferencesResponse, TuiBackendError>;
    async fn message_references(
        &self,
        request: MessageReferencesRequest,
    ) -> Result<MessageReferencesResponse, TuiBackendError>;
    async fn session_lineage(
        &self,
        request: SessionLineageRequest,
    ) -> Result<SessionLineageResponse, TuiBackendError>;
    async fn inspect_lineage(
        &self,
        request: InspectLineageRequest,
    ) -> Result<InspectLineageResponse, TuiBackendError>;
    async fn cancel_lineage(
        &self,
        request: CancelLineageRequest,
    ) -> Result<CancelLineageResponse, TuiBackendError>;
    async fn fork_session(
        &self,
        request: ForkSessionRequest,
    ) -> Result<ForkSessionResponse, TuiBackendError>;
    async fn fork_session_before_turn(
        &self,
        request: ForkSessionBeforeTurnRequest,
    ) -> Result<ForkSessionResponse, TuiBackendError>;
    async fn update_session_model(
        &self,
        request: UpdateSessionModelRequest,
    ) -> Result<UpdateSessionModelResponse, TuiBackendError>;
    async fn update_session_mode(
        &self,
        request: UpdateSessionModeRequest,
    ) -> Result<UpdateSessionModeResponse, TuiBackendError>;
}

pub(crate) struct AppServerTuiBackend {
    client: AppServerClient,
}

impl AppServerTuiBackend {
    pub(crate) fn new(client: AppServerClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl TuiBackend for AppServerTuiBackend {
    async fn initialize(
        &self,
        request: InitializeRequest,
    ) -> Result<InitializeResponse, TuiBackendError> {
        self.client
            .initialize(request)
            .await
            .map_err(|error| TuiBackendError {
                message: error.to_string(),
                outcome_unknown: false,
            })
    }

    async fn health(&self) -> Result<HealthResponse, TuiBackendError> {
        map(self.client.health().await)
    }

    fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<AppServerEvent> {
        self.client.subscribe_events()
    }

    async fn list_sessions(
        &self,
        request: ListSessionsRequest,
    ) -> Result<ListSessionsResponse, TuiBackendError> {
        map(self.client.list_sessions(request).await)
    }

    async fn sync_session(
        &self,
        request: SyncSessionRequest,
    ) -> Result<SyncSessionResponse, TuiBackendError> {
        map(self.client.sync_session(request).await)
    }

    async fn create_session(
        &self,
        request: CreateSessionRequest,
    ) -> Result<CreateSessionResponse, TuiBackendError> {
        map_client(self.client.create_session(request).await)
    }

    async fn delete_session(
        &self,
        request: DeleteSessionRequest,
    ) -> Result<DeleteSessionResponse, TuiBackendError> {
        map_client(self.client.delete_session(request).await)
    }

    async fn rename_session(
        &self,
        request: RenameSessionRequest,
    ) -> Result<RenameSessionResponse, TuiBackendError> {
        map_client(self.client.rename_session(request).await)
    }

    async fn submit_dialog_turn(
        &self,
        request: SubmitDialogTurnRequest,
    ) -> Result<SubmitDialogTurnResponse, TuiBackendError> {
        map_client(self.client.submit_dialog_turn(request).await)
    }

    async fn cancel_turn(
        &self,
        request: CancelTurnRequest,
    ) -> Result<CancelTurnResponse, TuiBackendError> {
        map_client(self.client.cancel_turn(request).await)
    }

    async fn steer_turn(
        &self,
        request: SteerTurnRequest,
    ) -> Result<SteerTurnResponse, TuiBackendError> {
        map_client(self.client.steer_turn(request).await)
    }

    async fn run_user_shell_command(
        &self,
        request: RunUserShellCommandRequest,
    ) -> Result<RunUserShellCommandResponse, TuiBackendError> {
        map_client(self.client.run_user_shell_command(request).await)
    }

    async fn submit_user_answers(
        &self,
        request: SubmitUserAnswersRequest,
    ) -> Result<SubmitUserAnswersResponse, TuiBackendError> {
        map_client(self.client.submit_user_answers(request).await)
    }

    async fn record_local_command_turn(
        &self,
        request: RecordLocalCommandTurnRequest,
    ) -> Result<RecordLocalCommandTurnResponse, TuiBackendError> {
        map_client(self.client.record_local_command_turn(request).await)
    }

    async fn respond_permission(
        &self,
        request: RespondPermissionRequest,
    ) -> Result<RespondPermissionResponse, TuiBackendError> {
        map_client(self.client.respond_permission(request).await)
    }

    async fn pending_permissions(&self) -> Result<PendingPermissionsResponse, TuiBackendError> {
        map(self.client.pending_permissions().await)
    }

    async fn compact_session(
        &self,
        request: CompactSessionRequest,
    ) -> Result<CompactSessionResponse, TuiBackendError> {
        map_client(self.client.compact_session(request).await)
    }

    async fn undo_session(
        &self,
        request: UndoSessionRequest,
    ) -> Result<RevertSessionResponse, TuiBackendError> {
        map_client(self.client.undo_session(request).await)
    }

    async fn redo_session(
        &self,
        request: RedoSessionRequest,
    ) -> Result<RevertSessionResponse, TuiBackendError> {
        map_client(self.client.redo_session(request).await)
    }

    async fn reload_context(
        &self,
        request: ReloadContextRequest,
    ) -> Result<ReloadContextResponse, TuiBackendError> {
        map_client(self.client.reload_context(request).await)
    }

    async fn session_usage(
        &self,
        request: SessionUsageRequest,
    ) -> Result<SessionUsageResponse, TuiBackendError> {
        map(self.client.session_usage(request).await)
    }

    async fn wait_for_settlement(
        &self,
        request: WaitForSettlementRequest,
    ) -> Result<WaitForSettlementResponse, TuiBackendError> {
        map(self.client.wait_for_settlement(request).await)
    }

    async fn workspace_diff(&self) -> Result<WorkspaceDiffResponse, TuiBackendError> {
        map(self.client.workspace_diff().await)
    }

    async fn search_workspace_references(
        &self,
        request: SearchWorkspaceReferencesRequest,
    ) -> Result<SearchWorkspaceReferencesResponse, TuiBackendError> {
        map(self.client.search_workspace_references(request).await)
    }

    async fn message_references(
        &self,
        request: MessageReferencesRequest,
    ) -> Result<MessageReferencesResponse, TuiBackendError> {
        map(self.client.message_references(request).await)
    }

    async fn session_lineage(
        &self,
        request: SessionLineageRequest,
    ) -> Result<SessionLineageResponse, TuiBackendError> {
        map(self.client.session_lineage(request).await)
    }

    async fn inspect_lineage(
        &self,
        request: InspectLineageRequest,
    ) -> Result<InspectLineageResponse, TuiBackendError> {
        map(self.client.inspect_lineage(request).await)
    }

    async fn cancel_lineage(
        &self,
        request: CancelLineageRequest,
    ) -> Result<CancelLineageResponse, TuiBackendError> {
        map_client(self.client.cancel_lineage(request).await)
    }

    async fn fork_session(
        &self,
        request: ForkSessionRequest,
    ) -> Result<ForkSessionResponse, TuiBackendError> {
        map_client(self.client.fork_session(request).await)
    }

    async fn fork_session_before_turn(
        &self,
        request: ForkSessionBeforeTurnRequest,
    ) -> Result<ForkSessionResponse, TuiBackendError> {
        map_client(self.client.fork_session_before_turn(request).await)
    }

    async fn update_session_model(
        &self,
        request: UpdateSessionModelRequest,
    ) -> Result<UpdateSessionModelResponse, TuiBackendError> {
        map_client(self.client.update_session_model(request).await)
    }

    async fn update_session_mode(
        &self,
        request: UpdateSessionModeRequest,
    ) -> Result<UpdateSessionModeResponse, TuiBackendError> {
        map_client(self.client.update_session_mode(request).await)
    }
}

fn map<T, E: std::fmt::Display>(result: Result<T, E>) -> Result<T, TuiBackendError> {
    result.map_err(|error| TuiBackendError {
        message: error.to_string(),
        outcome_unknown: false,
    })
}

fn map_client<T>(result: Result<T, ClientError>) -> Result<T, TuiBackendError> {
    result.map_err(|error| TuiBackendError {
        outcome_unknown: matches!(error, ClientError::Timeout(_)),
        message: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{TuiEffect, TuiEffectRoute};

    struct LocalEffect;

    impl TuiEffect for LocalEffect {
        fn route(&self) -> TuiEffectRoute {
            TuiEffectRoute::Local
        }
    }

    #[test]
    fn effect_routes_are_explicit() {
        assert_eq!(LocalEffect.route(), TuiEffectRoute::Local);
        assert_ne!(TuiEffectRoute::AppServer, TuiEffectRoute::HostCapability);
    }
}
