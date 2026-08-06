//! Interactive TUI session state projected over the App Server backend boundary.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::tui_backend::{TuiBackend, TuiBackendError, TuiBackendErrorKind};
use anyhow::Result;
use bitfun_app_server_client::AppServerEvent;
use bitfun_app_server_protocol::account::*;
use bitfun_app_server_protocol::agent::*;
use bitfun_app_server_protocol::event::EventStreamState;
use bitfun_app_server_protocol::external_source::*;
use bitfun_app_server_protocol::hook::*;
use bitfun_app_server_protocol::mcp::*;
use bitfun_app_server_protocol::model::*;
use bitfun_app_server_protocol::session::*;
use bitfun_app_server_protocol::skill::*;
use bitfun_app_server_protocol::subagent::*;
use bitfun_app_server_protocol::workspace::*;
use bitfun_core_types::SessionUsageReport;
use bitfun_events::{AgenticEvent, AgenticEventEnvelope, AgenticEventPriority};
use bitfun_product_domains::external_source_control::ExternalSourceControlRequestV1;
use bitfun_product_domains::external_sources::{
    ExternalSourceOperationError, ExternalSourceOperationErrorCode, ExternalSourcePublicSnapshot,
    NativePromptCommandDescriptor, PromptCommandShellReviewDecision,
};
use bitfun_product_domains::tool_permissions::{
    PermissionReply, PermissionRequest, PermissionRequestEvent,
};
use bitfun_runtime_ports::{
    put_agent_workspace_references, AgentContextReloadRequest, AgentDialogSteerRequest,
    AgentDialogTurnExecution, AgentDialogTurnRequest, AgentInputAttachment,
    AgentLocalCommandTurnRecordRequest, AgentMessageWorkspaceReferencesRequest,
    AgentSessionCompactionRequest, AgentSessionCreateRequest, AgentSessionDeleteRequest,
    AgentSessionLineageCancellationRequest, AgentSessionLineageInspection,
    AgentSessionLineageRequest, AgentSessionLineageSnapshot, AgentSessionLineageTranscriptRequest,
    AgentSessionListRequest, AgentSessionModeUpdateRequest, AgentSessionModelUpdateRequest,
    AgentSessionRenameRequest, AgentSessionRevertRequest, AgentSessionRevertResult,
    AgentSessionSummary, AgentSessionUsageRequest, AgentSessionWorkspaceBinding,
    AgentSubmissionSource, AgentTurnCancellationRequest, AgentTurnCancellationResult,
    AgentTurnSettlementRequest, AgentUserShellCommandRequest, AgentWorkspaceReference,
    AgentWorkspaceReferenceSearchRequest, AgentWorkspaceReferenceSearchResult,
    DialogSubmissionPolicy, SessionExecutionTarget, SessionTranscript, WorkspaceDiffSnapshot,
};
use tokio::sync::{broadcast, Mutex};

use crate::runtime::approval::{approval_metadata, CliApprovalPolicy};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TuiAgentMode {
    pub(crate) id: String,
    pub(crate) description: String,
    pub(crate) model_id: Option<String>,
    pub(crate) is_external: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SessionMigrationNotice {
    Mode {
        previous_id: String,
        restored_id: String,
    },
    Model {
        previous_id: String,
        restored_id: String,
    },
}

impl SessionMigrationNotice {
    pub(crate) fn user_message(&self) -> String {
        let (setting, previous_id, restored_id) = match self {
            Self::Mode {
                previous_id,
                restored_id,
            } => ("mode", previous_id, restored_id),
            Self::Model {
                previous_id,
                restored_id,
            } => ("model", previous_id, restored_id),
        };
        format!(
            "Session {setting} \"{previous_id}\" is unavailable. This session was restored with \"{restored_id}\". Review the {setting} before continuing."
        )
    }
}

#[derive(Debug)]
pub(crate) struct SessionOperationError {
    message: String,
    outcome_unknown: bool,
}

impl fmt::Display for SessionOperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SessionOperationError {}

impl SessionOperationError {
    fn backend(error: TuiBackendError) -> Self {
        Self {
            message: error.message,
            outcome_unknown: error.outcome_unknown,
        }
    }

    pub(crate) fn outcome_unknown(&self) -> bool {
        self.outcome_unknown
    }
}

#[derive(Clone, Debug)]
struct TuiWorkspacePaths {
    workspace_id: Option<String>,
    project: Option<PathBuf>,
    execution: Option<PathBuf>,
    execution_target: Option<SessionExecutionTarget>,
    remote_connection_id: Option<String>,
    remote_ssh_host: Option<String>,
}

impl TuiWorkspacePaths {
    fn new(workspace_path: Option<PathBuf>) -> Self {
        Self {
            workspace_id: None,
            project: workspace_path.clone(),
            execution: workspace_path,
            execution_target: None,
            remote_connection_id: None,
            remote_ssh_host: None,
        }
    }

    fn execution(&self) -> PathBuf {
        self.execution
            .clone()
            .or_else(|| self.project.clone())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn project(&self) -> PathBuf {
        self.project
            .clone()
            .or_else(|| self.execution.clone())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn apply_binding(&mut self, binding: &AgentSessionWorkspaceBinding) {
        self.workspace_id = binding.workspace_id.clone();
        let execution = PathBuf::from(&binding.workspace_path);
        let project = binding
            .project_workspace_path
            .as_deref()
            .filter(|path| !path.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| self.project());
        self.execution = Some(execution);
        self.project = Some(project);
        self.execution_target = binding.execution_target.clone();
        self.remote_connection_id = binding.remote_connection_id.clone();
        self.remote_ssh_host = binding.remote_ssh_host.clone();
    }

    fn reset_execution_to_project(&mut self) -> PathBuf {
        let project = self.project();
        self.execution = Some(project.clone());
        self.execution_target = Some(SessionExecutionTarget::local(
            project.to_string_lossy().to_string(),
        ));
        self.workspace_id = None;
        self.remote_connection_id = None;
        self.remote_ssh_host = None;
        project
    }

    fn workspace_diff_unavailable_reason(&self) -> Option<&'static str> {
        if self.remote_connection_id.is_some() || self.remote_ssh_host.is_some() {
            return Some("Workspace diff is unavailable for remote Sessions");
        }
        if !same_workspace_location(&self.execution(), &self.project()) {
            return Some(
                "Workspace diff is unavailable when the Session uses a different worktree",
            );
        }
        None
    }
}

pub(crate) struct TuiAgentClient {
    backend: Arc<dyn TuiBackend>,
    shared: bool,
    approval_policy: Arc<RwLock<CliApprovalPolicy>>,
    workspace_paths: Arc<RwLock<TuiWorkspacePaths>>,
    session_id: Arc<Mutex<Option<String>>>,
    current_turn_id: Arc<Mutex<Option<String>>>,
    agent_events: Arc<RwLock<Option<broadcast::Sender<AgenticEventEnvelope>>>>,
    permission_events: Arc<RwLock<Option<broadcast::Sender<PermissionRequestEvent>>>>,
    external_source_events:
        Arc<RwLock<Option<broadcast::Sender<(String, ExternalSourcePublicSnapshot)>>>>,
    pending_permissions: Arc<RwLock<HashMap<String, PermissionRequest>>>,
}

impl TuiAgentClient {
    pub(crate) fn new(
        backend: Arc<dyn TuiBackend>,
        workspace_path: Option<PathBuf>,
        shared: bool,
        approval_policy: CliApprovalPolicy,
    ) -> Self {
        let (agent_sender, _) = broadcast::channel(256);
        let (permission_sender, _) = broadcast::channel(64);
        let (external_source_sender, _) = broadcast::channel(64);
        let agent_events = Arc::new(RwLock::new(Some(agent_sender.clone())));
        let permission_events = Arc::new(RwLock::new(Some(permission_sender.clone())));
        let external_source_events = Arc::new(RwLock::new(Some(external_source_sender.clone())));
        let pending_permissions = Arc::new(RwLock::new(HashMap::new()));
        spawn_event_bridge(
            backend.subscribe_events(),
            agent_sender,
            permission_sender,
            external_source_sender,
            agent_events.clone(),
            permission_events.clone(),
            external_source_events.clone(),
            pending_permissions.clone(),
        );
        Self {
            backend,
            shared,
            approval_policy: Arc::new(RwLock::new(approval_policy)),
            workspace_paths: Arc::new(RwLock::new(TuiWorkspacePaths::new(workspace_path))),
            session_id: Arc::new(Mutex::new(None)),
            current_turn_id: Arc::new(Mutex::new(None)),
            agent_events,
            permission_events,
            external_source_events,
            pending_permissions,
        }
    }

    pub(crate) fn is_shared(&self) -> bool {
        self.shared
    }

    pub(crate) async fn model_catalog(&self) -> Result<TuiModelCatalogResponse> {
        Ok(self.backend.model_catalog().await?)
    }

    pub(crate) async fn available_agent_modes(&self) -> Result<Vec<TuiAgentMode>> {
        let response = self
            .backend
            .list_agent_modes(ListAgentModesRequest {
                workspace_path: Some(self.workspace_path_buf().to_string_lossy().to_string()),
                include_external: true,
            })
            .await
            .map_err(|error| anyhow::anyhow!(error))?;
        Ok(response
            .modes
            .into_iter()
            .map(|mode| TuiAgentMode {
                id: mode.id,
                description: mode.description,
                model_id: mode.model_id,
                is_external: mode.is_external,
            })
            .collect())
    }

    pub(crate) async fn list_models(&self) -> Result<ListModelsResponse> {
        self.backend
            .list_models()
            .await
            .map_err(|error| anyhow::anyhow!(error))
    }

    pub(crate) async fn get_model(&self, model_id: String) -> Result<GetModelResponse> {
        self.backend
            .get_model(GetModelRequest { model_id })
            .await
            .map_err(|error| anyhow::anyhow!(error))
    }

    pub(crate) async fn add_model(&self, request: AddModelRequest) -> Result<AddModelResponse> {
        self.backend
            .add_model(request)
            .await
            .map_err(|error| anyhow::anyhow!(error))
    }

    pub(crate) async fn update_model(
        &self,
        request: UpdateModelRequest,
    ) -> Result<UpdateModelResponse> {
        self.backend
            .update_model(request)
            .await
            .map_err(|error| anyhow::anyhow!(error))
    }

    pub(crate) async fn delete_model(&self, model_id: String) -> Result<DeleteModelResponse> {
        self.backend
            .delete_model(DeleteModelRequest { model_id })
            .await
            .map_err(|error| anyhow::anyhow!(error))
    }

    pub(crate) async fn set_model_default(
        &self,
        request: SetModelDefaultRequest,
    ) -> Result<SetModelDefaultResponse> {
        self.backend
            .set_model_default(request)
            .await
            .map_err(|error| anyhow::anyhow!(error))
    }

    pub(crate) async fn list_skills(
        &self,
        mode_id: String,
        manageable: bool,
    ) -> Result<ListSkillsResponse> {
        self.backend
            .list_skills(ListSkillsRequest {
                workspace_path: self.workspace_path_buf().to_string_lossy().to_string(),
                mode_id,
                manageable,
            })
            .await
            .map_err(|error| anyhow::anyhow!(error))
    }

    pub(crate) async fn set_skill_enabled(
        &self,
        mode_id: String,
        skill_key: String,
        enabled: bool,
        default_enabled: bool,
        level: String,
    ) -> Result<SetSkillEnabledResponse> {
        self.backend
            .set_skill_enabled(SetSkillEnabledRequest {
                workspace_path: self.workspace_path_buf().to_string_lossy().to_string(),
                mode_id,
                skill_key,
                enabled,
                default_enabled,
                level,
            })
            .await
            .map_err(|error| anyhow::anyhow!(error))
    }

    pub(crate) async fn list_subagents(
        &self,
        parent_mode_id: String,
        management: bool,
    ) -> Result<ListSubagentsResponse> {
        self.backend
            .list_subagents(ListSubagentsRequest {
                workspace_path: self.workspace_path_buf().to_string_lossy().to_string(),
                parent_mode_id,
                management,
            })
            .await
            .map_err(|error| anyhow::anyhow!(error))
    }

    pub(crate) async fn set_subagent_enabled(
        &self,
        parent_mode_id: String,
        subagent_id: String,
        enabled: bool,
    ) -> Result<SetSubagentEnabledResponse> {
        self.backend
            .set_subagent_enabled(SetSubagentEnabledRequest {
                workspace_path: self.workspace_path_buf().to_string_lossy().to_string(),
                parent_mode_id,
                subagent_id,
                enabled,
            })
            .await
            .map_err(|error| anyhow::anyhow!(error))
    }

    pub(crate) async fn list_mcp_servers(&self) -> Result<ListMcpServersResponse> {
        self.backend
            .list_mcp_servers(ListMcpServersRequest {
                workspace_path: self.workspace_path_buf().to_string_lossy().to_string(),
            })
            .await
            .map_err(|error| anyhow::anyhow!(error))
    }

    pub(crate) async fn toggle_mcp_server(
        &self,
        server_id: String,
    ) -> Result<ToggleMcpServerResponse> {
        self.backend
            .toggle_mcp_server(ToggleMcpServerRequest { server_id })
            .await
            .map_err(|error| anyhow::anyhow!(error))
    }

    pub(crate) async fn add_mcp_server(
        &self,
        name: String,
        config: McpServerMutation,
    ) -> Result<AddMcpServerResponse> {
        self.backend
            .add_mcp_server(AddMcpServerRequest { name, config })
            .await
            .map_err(|error| anyhow::anyhow!(error))
    }

    pub(crate) async fn delete_mcp_server(
        &self,
        server_id: String,
    ) -> Result<DeleteMcpServerResponse> {
        self.backend
            .delete_mcp_server(DeleteMcpServerRequest { server_id })
            .await
            .map_err(|error| anyhow::anyhow!(error))
    }

    pub(crate) async fn external_mcp_decision(
        &self,
        request: ExternalMcpDecisionRequest,
    ) -> Result<ExternalMcpDecisionResponse> {
        self.backend
            .external_mcp_decision(request)
            .await
            .map_err(|error| anyhow::anyhow!(error))
    }

    pub(crate) async fn mcp_conflict_choice(
        &self,
        request: McpConflictChoiceRequest,
    ) -> Result<McpConflictChoiceResponse> {
        self.backend
            .mcp_conflict_choice(request)
            .await
            .map_err(|error| anyhow::anyhow!(error))
    }

    pub(crate) async fn external_source_snapshot(
        &self,
        force_refresh: bool,
    ) -> std::result::Result<ExternalSourceSnapshotResponse, ExternalSourceOperationError> {
        self.backend
            .external_source_snapshot(ExternalSourceSnapshotRequest {
                workspace_path: self.workspace_path_string(),
                force_refresh,
            })
            .await
            .map_err(external_source_backend_error)
    }

    pub(crate) fn subscribe_external_source_updates(
        &self,
    ) -> Result<broadcast::Receiver<(String, ExternalSourcePublicSnapshot)>> {
        shared_receiver(
            &self.external_source_events,
            "App Server external source event stream is unavailable",
        )
    }

    pub(crate) async fn external_source_control(
        &self,
        request: ExternalSourceControlRequestV1,
    ) -> std::result::Result<ExternalSourceControlResponse, ExternalSourceOperationError> {
        self.backend
            .external_source_control(ExternalSourceControlRequest {
                workspace_path: self.workspace_path_string(),
                request,
            })
            .await
            .map_err(external_source_backend_error)
    }

    pub(crate) async fn external_source_review(
        &self,
        action: ExternalSourceReviewAction,
    ) -> std::result::Result<ExternalSourceSnapshotResponse, ExternalSourceOperationError> {
        self.backend
            .external_source_review(ExternalSourceReviewRequest {
                workspace_path: self.workspace_path_string(),
                operation_id: format!("tui-{}", uuid::Uuid::new_v4()),
                action,
            })
            .await
            .map(|response| response.0)
            .map_err(external_source_backend_error)
    }

    pub(crate) async fn set_native_command_choice(
        &self,
        native_commands: Vec<NativePromptCommandDescriptor>,
        selected_candidate_id: String,
        expected_preference_revision: u64,
    ) -> std::result::Result<SetNativeCommandChoiceResponse, ExternalSourceOperationError> {
        self.backend
            .set_native_command_choice(SetNativeCommandChoiceRequest {
                workspace_path: self.workspace_path_string(),
                operation_id: format!("tui-{}", uuid::Uuid::new_v4()),
                native_commands,
                selected_candidate_id,
                expected_preference_revision,
            })
            .await
            .map_err(external_source_backend_error)
    }

    pub(crate) async fn expand_external_command(
        &self,
        command_name: String,
        arguments: String,
        native_commands: Vec<NativePromptCommandDescriptor>,
        candidate_id: Option<String>,
        content_version: Option<String>,
        native_conflict_key: Option<String>,
        expected_preference_revision: Option<u64>,
        shell_review_decision: Option<PromptCommandShellReviewDecision>,
    ) -> std::result::Result<ExpandExternalCommandResponse, ExternalSourceOperationError> {
        self.backend
            .expand_external_command(ExpandExternalCommandRequest {
                workspace_path: self.workspace_path_string(),
                operation_id: format!("tui-{}", uuid::Uuid::new_v4()),
                command_name,
                arguments,
                native_commands,
                candidate_id,
                content_version,
                native_conflict_key,
                expected_preference_revision,
                shell_review_decision,
            })
            .await
            .map_err(external_source_backend_error)
    }

    pub(crate) async fn native_hook_overview(
        &self,
    ) -> std::result::Result<NativeHookOverview, ExternalSourceOperationError> {
        self.backend
            .native_hook_overview(NativeHookOverviewRequest {
                workspace_path: self.project_workspace_path_string(),
            })
            .await
            .map(|response| response.0)
            .map_err(external_source_backend_error)
    }

    pub(crate) async fn external_hook_snapshot(
        &self,
        refresh_updates: bool,
    ) -> std::result::Result<
        bitfun_product_domains::external_hook_import::ExternalHookImportSnapshotV1,
        ExternalSourceOperationError,
    > {
        self.backend
            .external_hook_snapshot(ExternalHookSnapshotRequest {
                workspace_path: self.project_workspace_path_string(),
                refresh_updates,
            })
            .await
            .map(|response| response.0)
            .map_err(external_source_backend_error)
    }

    pub(crate) async fn external_hook_plan(
        &self,
        source: bitfun_product_domains::external_sources::SourceKey,
    ) -> std::result::Result<
        bitfun_product_domains::external_hook_import::ExternalHookImportPlanV1,
        ExternalSourceOperationError,
    > {
        self.backend
            .external_hook_plan(ExternalHookPlanRequest {
                workspace_path: self.project_workspace_path_string(),
                source,
            })
            .await
            .map(|response| response.0)
            .map_err(external_source_backend_error)
    }

    pub(crate) async fn external_hook_apply(
        &self,
        import_request: bitfun_product_domains::external_hook_import::ExternalHookImportApplyRequestV1,
    ) -> std::result::Result<
        bitfun_product_domains::external_hook_import::ExternalHookImportApplyResultV1,
        ExternalSourceOperationError,
    > {
        self.backend
            .external_hook_apply(ExternalHookApplyRequest {
                workspace_path: self.project_workspace_path_string(),
                operation_id: format!("tui-hook-{}", uuid::Uuid::new_v4()),
                import_request,
            })
            .await
            .map(|response| response.0)
            .map_err(external_source_backend_error)
    }

    pub(crate) async fn external_hook_mutate(
        &self,
        mutation: bitfun_product_domains::external_hook_import::ExternalHookImportMutationRequestV1,
    ) -> std::result::Result<
        bitfun_product_domains::external_hook_import::ExternalHookImportSnapshotV1,
        ExternalSourceOperationError,
    > {
        self.backend
            .external_hook_mutate(ExternalHookMutationRequest {
                workspace_path: self.project_workspace_path_string(),
                operation_id: format!("tui-hook-{}", uuid::Uuid::new_v4()),
                mutation,
            })
            .await
            .map(|response| response.0)
            .map_err(external_source_backend_error)
    }

    pub(crate) async fn account_snapshot(&self) -> Result<AccountSnapshotResponse> {
        self.backend
            .account_snapshot(AccountSnapshotRequest {
                workspace_path: self.project_workspace_path_string(),
            })
            .await
            .map_err(Into::into)
    }

    pub(crate) async fn account_login(
        &self,
        relay_url: String,
        username: String,
        password: String,
    ) -> Result<AccountLoginResponse> {
        self.backend
            .account_login(AccountLoginRequest {
                operation_id: account_operation_id(),
                relay_url,
                username,
                password,
            })
            .await
            .map_err(Into::into)
    }

    pub(crate) async fn account_finalize_login(
        &self,
        choice: AccountSyncChoice,
    ) -> Result<AccountSnapshotResponse> {
        self.backend
            .account_finalize_login(AccountFinalizeLoginRequest {
                operation_id: account_operation_id(),
                choice,
                workspace_path: self.project_workspace_path_string(),
            })
            .await
            .map_err(Into::into)
    }

    pub(crate) async fn account_logout(&self) -> Result<AccountSnapshotResponse> {
        self.backend
            .account_logout(AccountLogoutRequest {
                operation_id: account_operation_id(),
                workspace_path: self.project_workspace_path_string(),
            })
            .await
            .map_err(Into::into)
    }

    pub(crate) async fn settings_sync_start(
        &self,
        is_first_login: bool,
    ) -> Result<SettingsSyncResponse> {
        self.backend
            .settings_sync_start(SettingsSyncStartRequest {
                operation_id: account_operation_id(),
                workspace_path: self.project_workspace_path_string(),
                is_first_login,
            })
            .await
            .map_err(Into::into)
    }

    pub(crate) async fn settings_sync_snapshot(&self) -> Result<SettingsSyncResponse> {
        self.backend
            .settings_sync_snapshot(SettingsSyncSnapshotRequest {
                workspace_path: self.project_workspace_path_string(),
            })
            .await
            .map_err(Into::into)
    }

    pub(crate) async fn settings_sync_cancel(&self) -> Result<SettingsSyncResponse> {
        self.backend
            .settings_sync_cancel(SettingsSyncCancelRequest {
                operation_id: account_operation_id(),
                workspace_path: self.project_workspace_path_string(),
            })
            .await
            .map_err(Into::into)
    }

    pub(crate) async fn settings_sync_local_changed(&self) -> Result<SettingsSyncResponse> {
        self.backend
            .settings_sync_local_changed(SettingsSyncLocalChangedRequest {
                operation_id: account_operation_id(),
                workspace_path: self.project_workspace_path_string(),
            })
            .await
            .map_err(Into::into)
    }

    pub(crate) fn subscribe_events(&self) -> Result<broadcast::Receiver<AgenticEventEnvelope>> {
        shared_receiver(
            &self.agent_events,
            "App Server agent event stream is unavailable",
        )
    }

    pub(crate) fn subscribe_permission_requests(
        &self,
    ) -> Result<broadcast::Receiver<PermissionRequestEvent>> {
        shared_receiver(
            &self.permission_events,
            "App Server permission event stream is unavailable",
        )
    }

    pub(crate) fn pending_permission_requests(&self) -> Result<Vec<PermissionRequest>> {
        Ok(self
            .pending_permissions
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .cloned()
            .collect())
    }

    pub(crate) async fn respond_permission(
        &self,
        request_id: &str,
        reply: PermissionReply,
    ) -> Result<()> {
        self.backend
            .respond_permission(RespondPermissionRequest {
                request_id: request_id.to_string(),
                reply,
            })
            .await?;
        self.pending_permissions
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(request_id);
        Ok(())
    }

    pub(crate) async fn record_completed_local_command_turn(
        &self,
        request: AgentLocalCommandTurnRecordRequest,
    ) -> Result<()> {
        self.backend
            .record_local_command_turn(RecordLocalCommandTurnRequest(request))
            .await?;
        Ok(())
    }

    pub(crate) fn set_approval_policy(&self, policy: CliApprovalPolicy) {
        *self
            .approval_policy
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = policy;
    }

    fn approval_policy(&self) -> CliApprovalPolicy {
        *self
            .approval_policy
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) fn workspace_path_buf(&self) -> PathBuf {
        self.workspace_paths
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .execution()
    }

    pub(crate) fn workspace_path_string(&self) -> String {
        self.workspace_path_buf().to_string_lossy().to_string()
    }

    pub(crate) fn project_workspace_path_string(&self) -> String {
        self.workspace_paths
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .project()
            .to_string_lossy()
            .to_string()
    }

    pub(crate) fn set_workspace_binding(&self, binding: &AgentSessionWorkspaceBinding) {
        self.workspace_paths
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .apply_binding(binding);
    }

    pub(crate) async fn list_sessions(&self) -> Result<Vec<AgentSessionSummary>> {
        Ok(self
            .backend
            .list_sessions(ListSessionsRequest(AgentSessionListRequest {
                workspace_path: self.project_workspace_path_string(),
                remote_connection_id: None,
                remote_ssh_host: None,
            }))
            .await?
            .sessions)
    }

    pub(crate) async fn session_lineage(
        &self,
        root_session_id: &str,
    ) -> Result<Option<AgentSessionLineageSnapshot>> {
        Ok(self
            .backend
            .session_lineage(SessionLineageRequest(AgentSessionLineageRequest {
                workspace_path: self.project_workspace_path_string(),
                anchor_session_id: root_session_id.to_string(),
                remote_connection_id: None,
                remote_ssh_host: None,
            }))
            .await?
            .0)
    }

    pub(crate) async fn inspect_lineage_session(
        &self,
        root_session_id: &str,
        session_id: &str,
        required_settled_turn_ids: &[String],
    ) -> std::result::Result<AgentSessionLineageInspection, SessionOperationError> {
        self.backend
            .inspect_lineage(InspectLineageRequest(
                AgentSessionLineageTranscriptRequest {
                    workspace_path: self.project_workspace_path_string(),
                    root_session_id: root_session_id.to_string(),
                    session_id: session_id.to_string(),
                    required_settled_turn_ids: required_settled_turn_ids.to_vec(),
                    remote_connection_id: None,
                    remote_ssh_host: None,
                },
            ))
            .await
            .map(|response| response.0)
            .map_err(SessionOperationError::backend)
    }

    pub(crate) async fn cancel_lineage_session(
        &self,
        root_session_id: &str,
        session_id: &str,
        expected_active_turn_id: &str,
    ) -> Result<AgentTurnCancellationResult> {
        Ok(self
            .backend
            .cancel_lineage(CancelLineageRequest(
                AgentSessionLineageCancellationRequest {
                    workspace_path: self.project_workspace_path_string(),
                    root_session_id: root_session_id.to_string(),
                    session_id: session_id.to_string(),
                    expected_active_turn_id: Some(expected_active_turn_id.to_string()),
                    source: Some(AgentSubmissionSource::Cli),
                    reason: Some("user_cancelled".to_string()),
                    wait_timeout_ms: Some(5_000),
                    remote_connection_id: None,
                    remote_ssh_host: None,
                },
            ))
            .await?
            .0)
    }

    pub(crate) async fn restore_session_in_current_workspace(
        &self,
        session_id: &str,
    ) -> Result<(
        AgentSessionSummary,
        AgentSessionWorkspaceBinding,
        Vec<SessionMigrationNotice>,
        SessionTranscript,
    )> {
        let previous = self
            .list_sessions()
            .await?
            .into_iter()
            .find(|summary| summary.session_id == session_id)
            .ok_or_else(|| anyhow::anyhow!("Session {session_id} was not found"))?;
        let response = self
            .backend
            .sync_session(SyncSessionRequest {
                workspace_path: self.project_workspace_path_string(),
                session_id: session_id.to_string(),
                include_internal: false,
                remote_connection_id: None,
                remote_ssh_host: None,
            })
            .await?;
        self.set_workspace_binding(&response.workspace_binding);
        *self.session_id.lock().await = Some(session_id.to_string());
        *self.current_turn_id.lock().await = match &response.state {
            SessionRuntimeState::Processing {
                current_turn_id, ..
            } => Some(current_turn_id.clone()),
            SessionRuntimeState::Idle | SessionRuntimeState::Error { .. } => None,
        };
        self.replace_pending_permissions(response.pending_permissions.clone());
        let notices = session_migration_notices(&previous, &response.session);
        Ok((
            response.session,
            response.workspace_binding,
            notices,
            response.transcript,
        ))
    }

    pub(crate) async fn session_workspace_binding(
        &self,
        session_id: &str,
    ) -> Result<AgentSessionWorkspaceBinding> {
        if self.session_id.lock().await.as_deref() != Some(session_id) {
            return Err(anyhow::anyhow!("Session {session_id} is not attached"));
        }
        let paths = self
            .workspace_paths
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let execution = paths.execution();
        Ok(AgentSessionWorkspaceBinding {
            workspace_id: paths.workspace_id.clone(),
            workspace_path: execution.to_string_lossy().to_string(),
            project_workspace_path: Some(paths.project().to_string_lossy().to_string()),
            execution_target: paths.execution_target.clone().or_else(|| {
                Some(SessionExecutionTarget::local(
                    execution.to_string_lossy().to_string(),
                ))
            }),
            remote_connection_id: paths.remote_connection_id.clone(),
            remote_ssh_host: paths.remote_ssh_host.clone(),
        })
    }

    pub(crate) async fn delete_session(
        &self,
        session_id: &str,
    ) -> std::result::Result<(), SessionOperationError> {
        self.backend
            .delete_session(DeleteSessionRequest(AgentSessionDeleteRequest {
                workspace_path: self.project_workspace_path_string(),
                session_id: session_id.to_string(),
                remote_connection_id: None,
                remote_ssh_host: None,
            }))
            .await
            .map(|_| ())
            .map_err(SessionOperationError::backend)
    }

    pub(crate) async fn update_session_model(
        &self,
        session_id: &str,
        model_id: &str,
    ) -> std::result::Result<(), SessionOperationError> {
        self.backend
            .update_session_model(UpdateSessionModelRequest(AgentSessionModelUpdateRequest {
                session_id: session_id.to_string(),
                model_id: model_id.to_string(),
            }))
            .await
            .map(|_| ())
            .map_err(SessionOperationError::backend)
    }

    pub(crate) async fn rename_session(
        &self,
        session_id: &str,
        session_name: &str,
    ) -> std::result::Result<(), SessionOperationError> {
        self.backend
            .rename_session(RenameSessionRequest(AgentSessionRenameRequest {
                workspace_path: self.project_workspace_path_string(),
                session_id: session_id.to_string(),
                session_name: session_name.to_string(),
                remote_connection_id: None,
                remote_ssh_host: None,
            }))
            .await
            .map(|_| ())
            .map_err(SessionOperationError::backend)
    }

    pub(crate) async fn update_session_mode(
        &self,
        session_id: &str,
        mode_id: &str,
    ) -> std::result::Result<(), SessionOperationError> {
        self.backend
            .update_session_mode(UpdateSessionModeRequest(AgentSessionModeUpdateRequest {
                session_id: session_id.to_string(),
                mode_id: mode_id.to_string(),
            }))
            .await
            .map(|_| ())
            .map_err(SessionOperationError::backend)
    }

    pub(crate) async fn fork_current_session(
        &self,
        before_turn_id: Option<&str>,
    ) -> Result<(
        AgentSessionSummary,
        AgentSessionWorkspaceBinding,
        SessionTranscript,
    )> {
        let source_session_id = self.require_session_id().await?;
        let forked = match before_turn_id {
            Some(source_turn_id) => {
                self.backend
                    .fork_session_before_turn(ForkSessionBeforeTurnRequest(
                        bitfun_runtime_ports::AgentSessionForkBeforeTurnRequest {
                            workspace_path: self.project_workspace_path_string(),
                            source_session_id,
                            source_turn_id: source_turn_id.to_string(),
                            remote_connection_id: None,
                            remote_ssh_host: None,
                        },
                    ))
                    .await?
            }
            None => {
                self.backend
                    .fork_session(ForkSessionRequest(
                        bitfun_runtime_ports::AgentSessionForkRequest {
                            workspace_path: self.project_workspace_path_string(),
                            source_session_id,
                            remote_connection_id: None,
                            remote_ssh_host: None,
                        },
                    ))
                    .await?
            }
        };
        let new_session_id = forked.0.session_id;
        let (summary, binding, _, transcript) = self
            .restore_session_in_current_workspace(&new_session_id)
            .await?;
        Ok((summary, binding, transcript))
    }

    pub(crate) async fn revert_current_session(
        &self,
        undo: bool,
    ) -> Result<AgentSessionRevertResult> {
        let session_id = self.require_session_id().await?;
        let request = AgentSessionRevertRequest {
            workspace_path: self.project_workspace_path_string(),
            session_id: session_id.clone(),
            remote_connection_id: None,
            remote_ssh_host: None,
        };
        let mut result = if undo {
            self.backend
                .undo_session(UndoSessionRequest(request))
                .await?
                .0
        } else {
            self.backend
                .redo_session(RedoSessionRequest(request))
                .await?
                .0
        };
        if let Some(turn_id) = self.current_turn_id.lock().await.take() {
            if !result.retired_turn_ids.contains(&turn_id) {
                result.retired_turn_ids.push(turn_id);
            }
        }
        Ok(result)
    }

    pub(crate) async fn workspace_diff(&self) -> Result<WorkspaceDiffSnapshot> {
        if let Some(reason) = self
            .workspace_paths
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .workspace_diff_unavailable_reason()
        {
            return Err(anyhow::anyhow!(reason));
        }
        Ok(self.backend.workspace_diff().await?.0)
    }

    pub(crate) async fn generate_session_usage_report(
        &self,
        request: AgentSessionUsageRequest,
    ) -> Result<SessionUsageReport> {
        Ok(self
            .backend
            .session_usage(SessionUsageRequest(request))
            .await?
            .0)
    }

    pub(crate) async fn reload_context(&self, request: AgentContextReloadRequest) -> Result<()> {
        self.backend
            .reload_context(ReloadContextRequest(request))
            .await?;
        Ok(())
    }

    pub(crate) async fn wait_for_turn_settlement(
        &self,
        session_id: &str,
        turn_id: &str,
        wait_timeout_ms: u64,
    ) -> Result<()> {
        self.backend
            .wait_for_settlement(WaitForSettlementRequest(AgentTurnSettlementRequest {
                session_id: session_id.to_string(),
                turn_id: turn_id.to_string(),
                wait_timeout_ms,
            }))
            .await?;
        Ok(())
    }

    pub(crate) async fn ensure_session(&self, agent_type: &str) -> Result<String> {
        self.ensure_session_with_model(agent_type, None).await
    }

    pub(crate) async fn ensure_session_with_model(
        &self,
        agent_type: &str,
        model_id: Option<String>,
    ) -> Result<String> {
        if let Some(id) = self.session_id.lock().await.clone() {
            return Ok(id);
        }
        self.create_session(agent_type, model_id, false).await
    }

    pub(crate) async fn create_new_session(&self, agent_type: &str) -> Result<String> {
        self.create_session(agent_type, None, true).await
    }

    async fn create_session(
        &self,
        agent_type: &str,
        model_id: Option<String>,
        reset_to_project: bool,
    ) -> Result<String> {
        let workspace = if reset_to_project {
            self.workspace_paths
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .reset_execution_to_project()
        } else {
            self.workspace_path_buf()
        };
        let project = self.project_workspace_path_string();
        let response = self
            .backend
            .create_session(CreateSessionRequest(AgentSessionCreateRequest {
                session_name: default_session_name(),
                agent_type: agent_type.to_string(),
                workspace_path: Some(workspace.to_string_lossy().to_string()),
                project_workspace_path: Some(project.clone()),
                execution_target: Some(SessionExecutionTarget::local(
                    workspace.to_string_lossy().to_string(),
                )),
                workspace_id: None,
                remote_connection_id: None,
                remote_ssh_host: None,
                model_id,
                metadata: serde_json::Map::new(),
            }))
            .await?;
        let session = response.0;
        let id = session.session_id.clone();
        let binding = AgentSessionWorkspaceBinding {
            workspace_id: session.workspace_id,
            workspace_path: session
                .workspace_path
                .unwrap_or_else(|| workspace.to_string_lossy().to_string()),
            project_workspace_path: session.project_workspace_path.or(Some(project)),
            execution_target: session.execution_target,
            remote_connection_id: None,
            remote_ssh_host: None,
        };
        self.set_workspace_binding(&binding);
        *self.session_id.lock().await = Some(id.clone());
        *self.current_turn_id.lock().await = None;
        self.replace_pending_permissions(Vec::new());
        Ok(id)
    }

    pub(crate) async fn start_session_compaction(&self, session_id: &str) -> Result<String> {
        let turn_id = uuid::Uuid::new_v4().to_string();
        *self.current_turn_id.lock().await = Some(turn_id.clone());
        let result = self
            .backend
            .compact_session(CompactSessionRequest(AgentSessionCompactionRequest {
                session_id: session_id.to_string(),
                turn_id: turn_id.clone(),
            }))
            .await;
        match result {
            Ok(response)
                if response.0.session_id == session_id && response.0.turn_id == turn_id =>
            {
                Ok(turn_id)
            }
            Ok(_) => {
                *self.current_turn_id.lock().await = None;
                Err(anyhow::anyhow!(
                    "App Server accepted compaction with an unexpected identity"
                ))
            }
            Err(error) => {
                *self.current_turn_id.lock().await = None;
                Err(error.into())
            }
        }
    }

    pub(crate) async fn send_message_with_context(
        &self,
        message: String,
        workspace_references: Vec<AgentWorkspaceReference>,
        attachments: Vec<AgentInputAttachment>,
        agent_type: &str,
    ) -> Result<String> {
        self.submit_dialog_turn(
            message,
            None,
            workspace_references,
            attachments,
            AgentDialogTurnExecution::Standard,
            agent_type,
        )
        .await
    }

    pub(crate) async fn send_external_subagent_command(
        &self,
        prompt: String,
        original_command: String,
        ecosystem_id: String,
        logical_id: String,
        agent_type: &str,
    ) -> Result<String> {
        self.submit_dialog_turn(
            prompt,
            Some(original_command),
            Vec::new(),
            Vec::new(),
            AgentDialogTurnExecution::FreshExternalSubagent {
                ecosystem_id,
                logical_id,
            },
            agent_type,
        )
        .await
    }

    async fn submit_dialog_turn(
        &self,
        message: String,
        original_message: Option<String>,
        workspace_references: Vec<AgentWorkspaceReference>,
        attachments: Vec<AgentInputAttachment>,
        execution: AgentDialogTurnExecution,
        agent_type: &str,
    ) -> Result<String> {
        let session_id = self.ensure_session(agent_type).await?;
        let turn_id = uuid::Uuid::new_v4().to_string();
        *self.current_turn_id.lock().await = Some(turn_id.clone());
        let mut metadata = approval_metadata(self.approval_policy());
        put_agent_workspace_references(&mut metadata, &workspace_references)
            .map_err(|error| anyhow::anyhow!(error.message))?;
        let result = self
            .backend
            .submit_dialog_turn(SubmitDialogTurnRequest(AgentDialogTurnRequest {
                session_id: session_id.clone(),
                message,
                original_message,
                turn_id: Some(turn_id.clone()),
                execution,
                agent_type: agent_type.to_string(),
                workspace_path: Some(self.project_workspace_path_string()),
                remote_connection_id: None,
                remote_ssh_host: None,
                policy: DialogSubmissionPolicy::for_source(AgentSubmissionSource::Cli),
                reply_route: None,
                prepended_reminders: Vec::new(),
                attachments,
                metadata,
            }))
            .await;
        match result {
            Ok(SubmitDialogTurnResponse::Started {
                session_id: accepted_session,
                turn_id: accepted_turn,
            })
            | Ok(SubmitDialogTurnResponse::Queued {
                session_id: accepted_session,
                turn_id: accepted_turn,
            }) if accepted_session == session_id => {
                *self.current_turn_id.lock().await = Some(accepted_turn.clone());
                Ok(accepted_turn)
            }
            Ok(_) => {
                *self.current_turn_id.lock().await = None;
                Err(anyhow::anyhow!(
                    "App Server accepted a turn with an unexpected identity"
                ))
            }
            Err(error) => {
                *self.current_turn_id.lock().await = None;
                Err(error.into())
            }
        }
    }

    pub(crate) async fn steer_current_turn(
        &self,
        content: String,
        display_content: Option<String>,
    ) -> Result<String> {
        let session_id = self.require_session_id().await?;
        let turn_id = self
            .current_turn_id
            .lock()
            .await
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active turn is available for steering"))?;
        Ok(self
            .backend
            .steer_turn(SteerTurnRequest(AgentDialogSteerRequest {
                session_id,
                turn_id,
                content,
                display_content,
            }))
            .await?
            .steering_id)
    }

    pub(crate) async fn run_user_shell_command(
        &self,
        command: String,
        agent_type: &str,
    ) -> Result<String> {
        let session_id = self.ensure_session(agent_type).await?;
        let turn_id = uuid::Uuid::new_v4().to_string();
        *self.current_turn_id.lock().await = Some(turn_id.clone());
        let response = self
            .backend
            .run_user_shell_command(RunUserShellCommandRequest(AgentUserShellCommandRequest {
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
                command,
            }))
            .await;
        match response {
            Ok(response)
                if response.0.session_id == session_id && response.0.turn_id == turn_id =>
            {
                Ok(turn_id)
            }
            Ok(_) => {
                *self.current_turn_id.lock().await = None;
                Err(anyhow::anyhow!(
                    "App Server accepted a Shell command with an unexpected identity"
                ))
            }
            Err(error) => {
                *self.current_turn_id.lock().await = None;
                Err(error.into())
            }
        }
    }

    pub(crate) async fn search_workspace_references(
        &self,
        query: String,
    ) -> Result<AgentWorkspaceReferenceSearchResult> {
        Ok(self
            .backend
            .search_workspace_references(SearchWorkspaceReferencesRequest(
                AgentWorkspaceReferenceSearchRequest {
                    session_id: self.require_session_id().await?,
                    query,
                    limit: 20,
                },
            ))
            .await?
            .0)
    }

    pub(crate) async fn workspace_references_for_message(
        &self,
        session_id: String,
        message_id: String,
    ) -> Result<Vec<AgentWorkspaceReference>> {
        Ok(self
            .backend
            .message_references(MessageReferencesRequest(
                AgentMessageWorkspaceReferencesRequest {
                    session_id,
                    message_id,
                },
            ))
            .await?
            .0)
    }

    pub(crate) async fn cancel_current_turn(&self) -> Result<()> {
        let session_id = self.session_id.lock().await.clone();
        let turn_id = self.current_turn_id.lock().await.clone();
        if let (Some(session_id), Some(turn_id)) = (session_id, turn_id) {
            self.backend
                .cancel_turn(CancelTurnRequest(AgentTurnCancellationRequest {
                    session_id,
                    turn_id: Some(turn_id.clone()),
                    source: Some(AgentSubmissionSource::Cli),
                    requester_session_id: None,
                    reason: Some("user_cancelled".to_string()),
                    wait_timeout_ms: None,
                    cancel_descendants: true,
                }))
                .await?;
            let mut current = self.current_turn_id.lock().await;
            if current.as_deref() == Some(turn_id.as_str()) {
                *current = None;
            }
        }
        Ok(())
    }

    pub(crate) async fn submit_user_answers(
        &self,
        tool_id: &str,
        answers: serde_json::Value,
    ) -> Result<()> {
        self.backend
            .submit_user_answers(SubmitUserAnswersRequest {
                tool_id: tool_id.to_string(),
                answers,
            })
            .await?;
        Ok(())
    }

    async fn require_session_id(&self) -> Result<String> {
        self.session_id
            .lock()
            .await
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active Session"))
    }

    fn replace_pending_permissions(&self, requests: Vec<PermissionRequest>) {
        let mut pending = self
            .pending_permissions
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        pending.clear();
        pending.extend(
            requests
                .into_iter()
                .map(|request| (request.request_id.clone(), request)),
        );
    }
}

fn external_source_backend_error(error: TuiBackendError) -> ExternalSourceOperationError {
    if let Some(decoded) = ExternalSourceOperationError::decode(&error.message) {
        return decoded;
    }
    let code = if error.outcome_unknown {
        ExternalSourceOperationErrorCode::Timeout
    } else if matches!(error.kind, TuiBackendErrorKind::Unsupported { .. }) {
        ExternalSourceOperationErrorCode::HostCapabilityUnavailable
    } else {
        ExternalSourceOperationErrorCode::Internal
    };
    ExternalSourceOperationError::new(code, error.message, error.outcome_unknown)
        .with_default_recovery_actions()
}

fn account_operation_id() -> String {
    format!("tui-account-{}", uuid::Uuid::new_v4())
}

fn shared_receiver<T: Clone>(
    source: &Arc<RwLock<Option<broadcast::Sender<T>>>>,
    message: &str,
) -> Result<broadcast::Receiver<T>> {
    source
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
        .map(broadcast::Sender::subscribe)
        .ok_or_else(|| anyhow::anyhow!(message.to_string()))
}

fn spawn_event_bridge(
    mut source: broadcast::Receiver<AppServerEvent>,
    agent_sender: broadcast::Sender<AgenticEventEnvelope>,
    permission_sender: broadcast::Sender<PermissionRequestEvent>,
    external_source_sender: broadcast::Sender<(String, ExternalSourcePublicSnapshot)>,
    agent_owner: Arc<RwLock<Option<broadcast::Sender<AgenticEventEnvelope>>>>,
    permission_owner: Arc<RwLock<Option<broadcast::Sender<PermissionRequestEvent>>>>,
    external_source_owner: Arc<
        RwLock<Option<broadcast::Sender<(String, ExternalSourcePublicSnapshot)>>>,
    >,
    pending: Arc<RwLock<HashMap<String, PermissionRequest>>>,
) {
    tokio::spawn(async move {
        loop {
            match source.recv().await {
                Ok(AppServerEvent::Agent(notification)) => {
                    let _ = agent_sender.send(notification.event);
                }
                Ok(AppServerEvent::Permission(notification)) => {
                    match &notification.event {
                        PermissionRequestEvent::Asked { request } => {
                            pending
                                .write()
                                .unwrap_or_else(|poisoned| poisoned.into_inner())
                                .insert(request.request_id.clone(), request.clone());
                        }
                        PermissionRequestEvent::Replied { request_id, .. }
                        | PermissionRequestEvent::Cancelled { request_id, .. } => {
                            pending
                                .write()
                                .unwrap_or_else(|poisoned| poisoned.into_inner())
                                .remove(request_id);
                        }
                    }
                    let _ = permission_sender.send(notification.event);
                }
                Ok(AppServerEvent::ExternalSource(notification)) => {
                    let _ = external_source_sender
                        .send((notification.workspace_path, notification.snapshot));
                }
                Ok(AppServerEvent::StreamState(notification))
                    if notification.stream
                        == bitfun_app_server_protocol::event::EventStream::ExternalSource =>
                {
                    // The next TUI snapshot request is the authoritative recovery path.
                }
                Ok(AppServerEvent::StreamState(notification))
                    if matches!(
                        notification.state,
                        EventStreamState::Closed | EventStreamState::Invalidated
                    ) =>
                {
                    send_stream_error(
                        &agent_sender,
                        notification.resync.reason.unwrap_or_else(|| {
                            "App Server event stream is unavailable".to_string()
                        }),
                    );
                    break;
                }
                Ok(AppServerEvent::ConnectionClosed)
                | Err(broadcast::error::RecvError::Closed)
                | Err(broadcast::error::RecvError::Lagged(_)) => {
                    send_stream_error(
                        &agent_sender,
                        "App Server connection was lost; this view is no longer authoritative",
                    );
                    break;
                }
                Ok(AppServerEvent::Config(_) | AppServerEvent::StreamState(_)) => {}
            }
        }
        *agent_owner
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        *permission_owner
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        *external_source_owner
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    });
}

fn send_stream_error(sender: &broadcast::Sender<AgenticEventEnvelope>, message: impl Into<String>) {
    let _ = sender.send(AgenticEventEnvelope::new(
        AgenticEvent::SystemError {
            session_id: None,
            error: message.into(),
            recoverable: false,
        },
        AgenticEventPriority::Critical,
    ));
}

fn session_migration_notices(
    previous: &AgentSessionSummary,
    restored: &AgentSessionSummary,
) -> Vec<SessionMigrationNotice> {
    let mut notices = Vec::new();
    if previous.agent_type != restored.agent_type {
        notices.push(SessionMigrationNotice::Mode {
            previous_id: previous.agent_type.clone(),
            restored_id: restored.agent_type.clone(),
        });
    }
    if let (Some(previous_id), Some(restored_id)) =
        (previous.model_id.as_ref(), restored.model_id.as_ref())
    {
        if previous_id != restored_id {
            notices.push(SessionMigrationNotice::Model {
                previous_id: previous_id.clone(),
                restored_id: restored_id.clone(),
            });
        }
    }
    notices
}

fn same_workspace_location(left: &Path, right: &Path) -> bool {
    left == right
        || dunce::canonicalize(left)
            .ok()
            .zip(dunce::canonicalize(right).ok())
            .is_some_and(|(left, right)| left == right)
}

fn default_session_name() -> String {
    format!(
        "CLI Session - {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    )
}
