//! CLI-local App Server boundary for the interactive TUI.

use async_trait::async_trait;
use bitfun_app_server_client::{AppServerClient, AppServerEvent, ClientError, ProtocolError};
use bitfun_app_server_protocol::account::*;
use bitfun_app_server_protocol::agent::*;
use bitfun_app_server_protocol::app::{HealthResponse, InitializeRequest, InitializeResponse};
use bitfun_app_server_protocol::error::{AppServerErrorData, AppServerErrorKind};
use bitfun_app_server_protocol::external_source::*;
use bitfun_app_server_protocol::hook::*;
use bitfun_app_server_protocol::mcp::*;
use bitfun_app_server_protocol::model::*;
use bitfun_app_server_protocol::session::*;
use bitfun_app_server_protocol::skill::*;
use bitfun_app_server_protocol::subagent::*;
use bitfun_app_server_protocol::workspace::*;
use bitfun_app_server_protocol::worktree::*;

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
    pub kind: TuiBackendErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TuiBackendErrorKind {
    Backend,
    Unsupported { capability: String },
}

impl std::fmt::Display for TuiBackendError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for TuiBackendError {}

fn external_application_v2_unsupported() -> TuiBackendError {
    TuiBackendError {
        message: "External application V2 is unavailable on this TUI backend".to_string(),
        outcome_unknown: false,
        kind: TuiBackendErrorKind::Unsupported {
            capability: "tui.externalApplicationsV2".to_string(),
        },
    }
}

#[async_trait]
#[allow(dead_code)]
pub(crate) trait TuiBackend: Send + Sync {
    async fn initialize(
        &self,
        request: InitializeRequest,
    ) -> Result<InitializeResponse, TuiBackendError>;

    async fn health(&self) -> Result<HealthResponse, TuiBackendError>;

    fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<AppServerEvent>;

    async fn model_catalog(&self) -> Result<TuiModelCatalogResponse, TuiBackendError>;
    async fn account_snapshot(
        &self,
        request: AccountSnapshotRequest,
    ) -> Result<AccountSnapshotResponse, TuiBackendError>;
    async fn account_login(
        &self,
        request: AccountLoginRequest,
    ) -> Result<AccountLoginResponse, TuiBackendError>;
    async fn account_finalize_login(
        &self,
        request: AccountFinalizeLoginRequest,
    ) -> Result<AccountSnapshotResponse, TuiBackendError>;
    async fn account_logout(
        &self,
        request: AccountLogoutRequest,
    ) -> Result<AccountSnapshotResponse, TuiBackendError>;
    async fn settings_sync_start(
        &self,
        request: SettingsSyncStartRequest,
    ) -> Result<SettingsSyncResponse, TuiBackendError>;
    async fn settings_sync_snapshot(
        &self,
        request: SettingsSyncSnapshotRequest,
    ) -> Result<SettingsSyncResponse, TuiBackendError>;
    async fn settings_sync_cancel(
        &self,
        request: SettingsSyncCancelRequest,
    ) -> Result<SettingsSyncResponse, TuiBackendError>;
    async fn settings_sync_local_changed(
        &self,
        request: SettingsSyncLocalChangedRequest,
    ) -> Result<SettingsSyncResponse, TuiBackendError>;
    async fn worktree_repository_status(
        &self,
        request: WorktreeRepositoryStatusRequest,
    ) -> Result<WorktreeRepositoryStatusResponse, TuiBackendError>;
    async fn worktree_bind_session(
        &self,
        request: WorktreeBindSessionRequest,
    ) -> Result<WorktreeBindingResponse, TuiBackendError>;
    async fn worktree_release_session(
        &self,
        request: WorktreeReleaseSessionRequest,
    ) -> Result<WorktreeBindingResponse, TuiBackendError>;

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

    async fn list_agent_modes(
        &self,
        request: ListAgentModesRequest,
    ) -> Result<ListAgentModesResponse, TuiBackendError>;
    async fn list_models(&self) -> Result<ListModelsResponse, TuiBackendError>;
    async fn get_model(
        &self,
        request: GetModelRequest,
    ) -> Result<GetModelResponse, TuiBackendError>;
    async fn add_model(
        &self,
        request: AddModelRequest,
    ) -> Result<AddModelResponse, TuiBackendError>;
    async fn update_model(
        &self,
        request: UpdateModelRequest,
    ) -> Result<UpdateModelResponse, TuiBackendError>;
    async fn delete_model(
        &self,
        request: DeleteModelRequest,
    ) -> Result<DeleteModelResponse, TuiBackendError>;
    async fn set_model_default(
        &self,
        request: SetModelDefaultRequest,
    ) -> Result<SetModelDefaultResponse, TuiBackendError>;
    async fn list_skills(
        &self,
        request: ListSkillsRequest,
    ) -> Result<ListSkillsResponse, TuiBackendError>;
    async fn set_skill_enabled(
        &self,
        request: SetSkillEnabledRequest,
    ) -> Result<SetSkillEnabledResponse, TuiBackendError>;
    async fn list_subagents(
        &self,
        request: ListSubagentsRequest,
    ) -> Result<ListSubagentsResponse, TuiBackendError>;
    async fn set_subagent_enabled(
        &self,
        request: SetSubagentEnabledRequest,
    ) -> Result<SetSubagentEnabledResponse, TuiBackendError>;
    async fn list_mcp_servers(
        &self,
        request: ListMcpServersRequest,
    ) -> Result<ListMcpServersResponse, TuiBackendError>;
    async fn toggle_mcp_server(
        &self,
        request: ToggleMcpServerRequest,
    ) -> Result<ToggleMcpServerResponse, TuiBackendError>;
    async fn add_mcp_server(
        &self,
        request: AddMcpServerRequest,
    ) -> Result<AddMcpServerResponse, TuiBackendError>;
    async fn delete_mcp_server(
        &self,
        request: DeleteMcpServerRequest,
    ) -> Result<DeleteMcpServerResponse, TuiBackendError>;
    async fn external_mcp_decision(
        &self,
        request: ExternalMcpDecisionRequest,
    ) -> Result<ExternalMcpDecisionResponse, TuiBackendError>;
    async fn mcp_conflict_choice(
        &self,
        request: McpConflictChoiceRequest,
    ) -> Result<McpConflictChoiceResponse, TuiBackendError>;
    async fn external_source_snapshot(
        &self,
        request: ExternalSourceSnapshotRequest,
    ) -> Result<ExternalSourceSnapshotResponse, TuiBackendError>;
    async fn external_application_snapshot_v2(
        &self,
        _request: ExternalApplicationSnapshotRequestV2,
    ) -> Result<ExternalApplicationSnapshotResponseV2, TuiBackendError> {
        Err(external_application_v2_unsupported())
    }
    async fn external_application_review_page_v2(
        &self,
        _request: ExternalApplicationReviewPageRequest,
    ) -> Result<ExternalApplicationReviewPageResponseV2, TuiBackendError> {
        Err(external_application_v2_unsupported())
    }
    async fn apply_external_application_action_v2(
        &self,
        _request: ExternalApplicationActionRequest,
    ) -> Result<ExternalApplicationActionResponseV2, TuiBackendError> {
        Err(external_application_v2_unsupported())
    }
    async fn external_source_control(
        &self,
        request: ExternalSourceControlRequest,
    ) -> Result<ExternalSourceControlResponse, TuiBackendError>;
    async fn external_source_review(
        &self,
        request: ExternalSourceReviewRequest,
    ) -> Result<ExternalSourceReviewResponse, TuiBackendError>;
    async fn set_native_command_choice(
        &self,
        request: SetNativeCommandChoiceRequest,
    ) -> Result<SetNativeCommandChoiceResponse, TuiBackendError>;
    async fn expand_external_command(
        &self,
        request: ExpandExternalCommandRequest,
    ) -> Result<ExpandExternalCommandResponse, TuiBackendError>;
    async fn native_hook_overview(
        &self,
        request: NativeHookOverviewRequest,
    ) -> Result<NativeHookOverviewResponse, TuiBackendError>;
    async fn external_hook_snapshot(
        &self,
        request: ExternalHookSnapshotRequest,
    ) -> Result<ExternalHookSnapshotResponse, TuiBackendError>;
    async fn external_hook_plan(
        &self,
        request: ExternalHookPlanRequest,
    ) -> Result<ExternalHookPlanResponse, TuiBackendError>;
    async fn external_hook_apply(
        &self,
        request: ExternalHookApplyRequest,
    ) -> Result<ExternalHookApplyResponse, TuiBackendError>;
    async fn external_hook_mutate(
        &self,
        request: ExternalHookMutationRequest,
    ) -> Result<ExternalHookMutationResponse, TuiBackendError>;
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
        map(self.client.initialize(request).await)
    }

    async fn health(&self) -> Result<HealthResponse, TuiBackendError> {
        map(self.client.health().await)
    }

    async fn model_catalog(&self) -> Result<TuiModelCatalogResponse, TuiBackendError> {
        map(self.client.tui_model_catalog().await)
    }

    fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<AppServerEvent> {
        self.client.subscribe_events()
    }

    async fn account_snapshot(
        &self,
        request: AccountSnapshotRequest,
    ) -> Result<AccountSnapshotResponse, TuiBackendError> {
        map(self.client.account_snapshot(request).await)
    }

    async fn account_login(
        &self,
        request: AccountLoginRequest,
    ) -> Result<AccountLoginResponse, TuiBackendError> {
        map_client(self.client.account_login(request).await)
    }

    async fn account_finalize_login(
        &self,
        request: AccountFinalizeLoginRequest,
    ) -> Result<AccountSnapshotResponse, TuiBackendError> {
        map_client(self.client.account_finalize_login(request).await)
    }

    async fn account_logout(
        &self,
        request: AccountLogoutRequest,
    ) -> Result<AccountSnapshotResponse, TuiBackendError> {
        map_client(self.client.account_logout(request).await)
    }

    async fn settings_sync_start(
        &self,
        request: SettingsSyncStartRequest,
    ) -> Result<SettingsSyncResponse, TuiBackendError> {
        map_client(self.client.settings_sync_start(request).await)
    }

    async fn settings_sync_snapshot(
        &self,
        request: SettingsSyncSnapshotRequest,
    ) -> Result<SettingsSyncResponse, TuiBackendError> {
        map(self.client.settings_sync_snapshot(request).await)
    }

    async fn settings_sync_cancel(
        &self,
        request: SettingsSyncCancelRequest,
    ) -> Result<SettingsSyncResponse, TuiBackendError> {
        map_client(self.client.settings_sync_cancel(request).await)
    }

    async fn settings_sync_local_changed(
        &self,
        request: SettingsSyncLocalChangedRequest,
    ) -> Result<SettingsSyncResponse, TuiBackendError> {
        map_client(self.client.settings_sync_local_changed(request).await)
    }

    async fn worktree_repository_status(
        &self,
        request: WorktreeRepositoryStatusRequest,
    ) -> Result<WorktreeRepositoryStatusResponse, TuiBackendError> {
        map(self.client.worktree_repository_status(request).await)
    }

    async fn worktree_bind_session(
        &self,
        request: WorktreeBindSessionRequest,
    ) -> Result<WorktreeBindingResponse, TuiBackendError> {
        map_client(self.client.worktree_bind_session(request).await)
    }

    async fn worktree_release_session(
        &self,
        request: WorktreeReleaseSessionRequest,
    ) -> Result<WorktreeBindingResponse, TuiBackendError> {
        map_client(self.client.worktree_release_session(request).await)
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

    async fn list_agent_modes(
        &self,
        request: ListAgentModesRequest,
    ) -> Result<ListAgentModesResponse, TuiBackendError> {
        map(self.client.list_agent_modes(request).await)
    }

    async fn list_models(&self) -> Result<ListModelsResponse, TuiBackendError> {
        map(self.client.list_models().await)
    }

    async fn get_model(
        &self,
        request: GetModelRequest,
    ) -> Result<GetModelResponse, TuiBackendError> {
        map(self.client.get_model(request).await)
    }

    async fn add_model(
        &self,
        request: AddModelRequest,
    ) -> Result<AddModelResponse, TuiBackendError> {
        map_client(self.client.add_model(request).await)
    }

    async fn update_model(
        &self,
        request: UpdateModelRequest,
    ) -> Result<UpdateModelResponse, TuiBackendError> {
        map_client(self.client.update_model(request).await)
    }

    async fn delete_model(
        &self,
        request: DeleteModelRequest,
    ) -> Result<DeleteModelResponse, TuiBackendError> {
        map_client(self.client.delete_model(request).await)
    }

    async fn set_model_default(
        &self,
        request: SetModelDefaultRequest,
    ) -> Result<SetModelDefaultResponse, TuiBackendError> {
        map_client(self.client.set_model_default(request).await)
    }

    async fn list_skills(
        &self,
        request: ListSkillsRequest,
    ) -> Result<ListSkillsResponse, TuiBackendError> {
        map(self.client.list_skills(request).await)
    }

    async fn set_skill_enabled(
        &self,
        request: SetSkillEnabledRequest,
    ) -> Result<SetSkillEnabledResponse, TuiBackendError> {
        map_client(self.client.set_skill_enabled(request).await)
    }

    async fn list_subagents(
        &self,
        request: ListSubagentsRequest,
    ) -> Result<ListSubagentsResponse, TuiBackendError> {
        map(self.client.list_subagents(request).await)
    }

    async fn set_subagent_enabled(
        &self,
        request: SetSubagentEnabledRequest,
    ) -> Result<SetSubagentEnabledResponse, TuiBackendError> {
        map_client(self.client.set_subagent_enabled(request).await)
    }

    async fn list_mcp_servers(
        &self,
        request: ListMcpServersRequest,
    ) -> Result<ListMcpServersResponse, TuiBackendError> {
        map(self.client.list_mcp_servers(request).await)
    }

    async fn toggle_mcp_server(
        &self,
        request: ToggleMcpServerRequest,
    ) -> Result<ToggleMcpServerResponse, TuiBackendError> {
        map_client(self.client.toggle_mcp_server(request).await)
    }

    async fn add_mcp_server(
        &self,
        request: AddMcpServerRequest,
    ) -> Result<AddMcpServerResponse, TuiBackendError> {
        map_client(self.client.add_mcp_server(request).await)
    }

    async fn delete_mcp_server(
        &self,
        request: DeleteMcpServerRequest,
    ) -> Result<DeleteMcpServerResponse, TuiBackendError> {
        map_client(self.client.delete_mcp_server(request).await)
    }

    async fn external_mcp_decision(
        &self,
        request: ExternalMcpDecisionRequest,
    ) -> Result<ExternalMcpDecisionResponse, TuiBackendError> {
        map_client(self.client.external_mcp_decision(request).await)
    }

    async fn mcp_conflict_choice(
        &self,
        request: McpConflictChoiceRequest,
    ) -> Result<McpConflictChoiceResponse, TuiBackendError> {
        map_client(self.client.mcp_conflict_choice(request).await)
    }

    async fn external_source_snapshot(
        &self,
        request: ExternalSourceSnapshotRequest,
    ) -> Result<ExternalSourceSnapshotResponse, TuiBackendError> {
        map(self.client.external_source_snapshot(request).await)
    }

    async fn external_application_snapshot_v2(
        &self,
        request: ExternalApplicationSnapshotRequestV2,
    ) -> Result<ExternalApplicationSnapshotResponseV2, TuiBackendError> {
        map(self.client.external_application_snapshot_v2(request).await)
    }

    async fn external_application_review_page_v2(
        &self,
        request: ExternalApplicationReviewPageRequest,
    ) -> Result<ExternalApplicationReviewPageResponseV2, TuiBackendError> {
        map(self
            .client
            .external_application_review_page_v2(request)
            .await)
    }

    async fn apply_external_application_action_v2(
        &self,
        request: ExternalApplicationActionRequest,
    ) -> Result<ExternalApplicationActionResponseV2, TuiBackendError> {
        map_client(
            self.client
                .apply_external_application_action_v2(request)
                .await,
        )
    }

    async fn external_source_control(
        &self,
        request: ExternalSourceControlRequest,
    ) -> Result<ExternalSourceControlResponse, TuiBackendError> {
        map_client(self.client.external_source_control(request).await)
    }

    async fn external_source_review(
        &self,
        request: ExternalSourceReviewRequest,
    ) -> Result<ExternalSourceReviewResponse, TuiBackendError> {
        map_client(self.client.external_source_review(request).await)
    }

    async fn set_native_command_choice(
        &self,
        request: SetNativeCommandChoiceRequest,
    ) -> Result<SetNativeCommandChoiceResponse, TuiBackendError> {
        map_client(self.client.set_native_command_choice(request).await)
    }

    async fn expand_external_command(
        &self,
        request: ExpandExternalCommandRequest,
    ) -> Result<ExpandExternalCommandResponse, TuiBackendError> {
        map_client(self.client.expand_external_command(request).await)
    }

    async fn native_hook_overview(
        &self,
        request: NativeHookOverviewRequest,
    ) -> Result<NativeHookOverviewResponse, TuiBackendError> {
        map(self.client.native_hook_overview(request).await)
    }

    async fn external_hook_snapshot(
        &self,
        request: ExternalHookSnapshotRequest,
    ) -> Result<ExternalHookSnapshotResponse, TuiBackendError> {
        map(self.client.external_hook_snapshot(request).await)
    }

    async fn external_hook_plan(
        &self,
        request: ExternalHookPlanRequest,
    ) -> Result<ExternalHookPlanResponse, TuiBackendError> {
        map(self.client.external_hook_plan(request).await)
    }

    async fn external_hook_apply(
        &self,
        request: ExternalHookApplyRequest,
    ) -> Result<ExternalHookApplyResponse, TuiBackendError> {
        map_client(self.client.external_hook_apply(request).await)
    }

    async fn external_hook_mutate(
        &self,
        request: ExternalHookMutationRequest,
    ) -> Result<ExternalHookMutationResponse, TuiBackendError> {
        map_client(self.client.external_hook_mutate(request).await)
    }
}

fn map<T>(result: Result<T, ProtocolError>) -> Result<T, TuiBackendError> {
    result.map_err(map_protocol_error)
}

fn map_client<T>(result: Result<T, ClientError>) -> Result<T, TuiBackendError> {
    result.map_err(|error| match error {
        ClientError::Protocol(error) => map_protocol_error(error),
        ClientError::Timeout(data) => backend_error_from_data(
            "App Server request timed out with unknown outcome".to_string(),
            data,
        ),
    })
}

fn map_protocol_error(error: ProtocolError) -> TuiBackendError {
    let message = error.to_string();
    if i32::from(error.code) == -32601 {
        return TuiBackendError {
            message,
            outcome_unknown: false,
            kind: TuiBackendErrorKind::Unsupported {
                capability: "appServer.method".to_string(),
            },
        };
    }
    if let Some(value) = error.data {
        if let Ok(external) = serde_json::from_value::<ExternalSourceErrorData>(value.clone()) {
            let kind = match external.app.capability {
                Some(capability)
                    if matches!(external.app.kind, AppServerErrorKind::Unsupported) =>
                {
                    TuiBackendErrorKind::Unsupported { capability }
                }
                _ => TuiBackendErrorKind::Backend,
            };
            return TuiBackendError {
                message: external.error.encode(),
                outcome_unknown: external.app.outcome_unknown,
                kind,
            };
        }
        if let Ok(worktree) = serde_json::from_value::<WorktreeErrorData>(value.clone()) {
            let kind = if matches!(worktree.app.kind, AppServerErrorKind::Unsupported) {
                worktree
                    .app
                    .capability
                    .clone()
                    .map(|capability| TuiBackendErrorKind::Unsupported { capability })
                    .unwrap_or(TuiBackendErrorKind::Backend)
            } else {
                TuiBackendErrorKind::Backend
            };
            return TuiBackendError {
                message: worktree.error.encode(),
                outcome_unknown: worktree.app.outcome_unknown,
                kind,
            };
        }
        if let Ok(data) = serde_json::from_value::<AppServerErrorData>(value) {
            return backend_error_from_data(message, data);
        }
    }
    TuiBackendError {
        message,
        outcome_unknown: false,
        kind: TuiBackendErrorKind::Backend,
    }
}

fn backend_error_from_data(message: String, data: AppServerErrorData) -> TuiBackendError {
    let kind = match (data.kind, data.capability) {
        (AppServerErrorKind::Unsupported, Some(capability)) => {
            TuiBackendErrorKind::Unsupported { capability }
        }
        _ => TuiBackendErrorKind::Backend,
    };
    TuiBackendError {
        message,
        outcome_unknown: data.outcome_unknown,
        kind,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        external_application_v2_unsupported, map_protocol_error, TuiBackendErrorKind, TuiEffect,
        TuiEffectRoute,
    };
    use bitfun_app_server_protocol::error::{AppServerErrorData, AppServerErrorKind};
    use bitfun_app_server_protocol::external_source::ExternalSourceErrorData;
    use bitfun_product_domains::external_sources::{
        ExternalSourceOperationError, ExternalSourceOperationErrorCode,
    };

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

    #[test]
    fn unavailable_v2_backend_is_explicitly_read_only() {
        let error = external_application_v2_unsupported();
        assert_eq!(
            error.kind,
            TuiBackendErrorKind::Unsupported {
                capability: "tui.externalApplicationsV2".to_string()
            }
        );
        assert!(!error.outcome_unknown);
    }

    #[test]
    fn method_not_found_is_treated_as_an_unsupported_host_method() {
        let mapped =
            map_protocol_error(bitfun_app_server_client::ProtocolError::method_not_found());
        assert!(matches!(
            mapped.kind,
            TuiBackendErrorKind::Unsupported { .. }
        ));
        assert!(!mapped.outcome_unknown);
    }

    #[test]
    fn protocol_unsupported_preserves_the_capability_id() {
        let error = bitfun_app_server_client::ProtocolError::new(
            AppServerErrorKind::Unsupported.json_rpc_code() as i32,
            "not supported",
        )
        .data(
            serde_json::to_value(AppServerErrorData {
                kind: AppServerErrorKind::Unsupported,
                retryable: false,
                outcome_unknown: false,
                capability: Some("tui.models".to_string()),
                request_id: None,
            })
            .expect("serialize error data"),
        );

        let mapped = map_protocol_error(error);
        assert_eq!(
            mapped.kind,
            TuiBackendErrorKind::Unsupported {
                capability: "tui.models".to_string()
            }
        );
        assert!(!mapped.outcome_unknown);
    }

    #[test]
    fn external_source_protocol_error_preserves_domain_contract() {
        let domain = ExternalSourceOperationError::new(
            ExternalSourceOperationErrorCode::StaleRevision,
            "The external source catalog changed",
            false,
        )
        .with_correlation_id("external-source-ref-5")
        .with_default_recovery_actions();
        let error = bitfun_app_server_client::ProtocolError::new(
            AppServerErrorKind::StaleRevision.json_rpc_code() as i32,
            domain.detail.clone(),
        )
        .data(
            serde_json::to_value(ExternalSourceErrorData {
                app: AppServerErrorData {
                    kind: AppServerErrorKind::StaleRevision,
                    retryable: domain.retryable,
                    outcome_unknown: false,
                    capability: Some("tui.externalSources".to_string()),
                    request_id: domain.correlation_id.clone(),
                },
                error: domain.clone(),
            })
            .expect("serialize external source error data"),
        );

        let mapped = map_protocol_error(error);
        assert_eq!(mapped.kind, TuiBackendErrorKind::Backend);
        assert!(!mapped.outcome_unknown);
        let decoded = ExternalSourceOperationError::decode(&mapped.message)
            .expect("decode mapped external source error");
        assert_eq!(
            decoded.code,
            ExternalSourceOperationErrorCode::StaleRevision
        );
        assert_eq!(decoded.correlation_id, domain.correlation_id);
        assert_eq!(decoded.recovery_actions, domain.recovery_actions);
    }
}
