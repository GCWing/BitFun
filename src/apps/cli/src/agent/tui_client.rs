//! Thin interactive TUI facade over the shared CLI Runtime client and domain owners.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use bitfun_core_types::SessionUsageReport;
use bitfun_events::AgenticEventEnvelope;
use bitfun_product_domains::external_hook_import::{
    ExternalHookImportApplyRequestV1, ExternalHookImportApplyResultV1,
    ExternalHookImportMutationRequestV1, ExternalHookImportPlanV1, ExternalHookImportSnapshotV1,
};
use bitfun_product_domains::external_source_control::ExternalSourceControlRequestV1;
use bitfun_product_domains::external_sources::{
    ExternalSourceOperationError, ExternalSourceOperationErrorCode, ExternalSourcePublicSnapshot,
    NativePromptCommandDescriptor, PromptCommandInvocationOutcome,
    PromptCommandShellReviewDecision, SourceKey,
};
use bitfun_product_domains::tool_permissions::{
    PermissionReply, PermissionRequest, PermissionRequestEvent,
};
use bitfun_runtime_ports::{
    AgentContextReloadRequest, AgentInputAttachment, AgentSessionLineageInspection,
    AgentSessionLineageSnapshot, AgentSessionRevertResult, AgentSessionSummary,
    AgentSessionUsageRequest, AgentSessionWorkspaceBinding, AgentTurnCancellationResult,
    AgentWorkspaceReference, AgentWorkspaceReferenceSearchResult, SessionTranscript,
    WorkspaceDiffSnapshot,
};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use super::runtime_client::{CliAgentMode, CliAgentRuntimeClient};
pub(crate) use super::runtime_client::{
    CliSessionRestoreSnapshot, SessionMigrationNotice, SessionOperationError,
};
use crate::runtime::approval::CliApprovalPolicy;
use crate::tui_management::*;

pub(crate) type TuiAgentMode = CliAgentMode;

/// Presentation facade only. Runtime state lives in `CliAgentRuntimeClient`;
/// management behavior remains in the concrete owners composed below.
pub(crate) struct TuiAgentClient {
    runtime: Arc<CliAgentRuntimeClient>,
    management: Arc<TuiManagementOwners>,
    external_source_events: broadcast::Sender<(String, ExternalSourcePublicSnapshot)>,
    external_source_subscriptions: Arc<Mutex<HashSet<String>>>,
    external_source_tasks: Mutex<Vec<JoinHandle<()>>>,
}

impl TuiAgentClient {
    pub(crate) fn new(
        runtime: Arc<CliAgentRuntimeClient>,
        management: Arc<TuiManagementOwners>,
    ) -> Self {
        let (external_source_events, _) = broadcast::channel(64);
        Self {
            runtime,
            management,
            external_source_events,
            external_source_subscriptions: Arc::new(Mutex::new(HashSet::new())),
            external_source_tasks: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn is_shared(&self) -> bool {
        self.runtime.is_shared()
    }

    fn management_scope(&self) -> ManagementScope {
        management_scope(
            self.project_workspace_path_buf(),
            self.runtime.is_remote_workspace(),
        )
    }

    fn local_management_scope(&self, capability: &str) -> ManagementResult<ManagementScope> {
        let scope = self.management_scope();
        scope.local_workspace(capability)?;
        Ok(scope)
    }

    pub(crate) async fn model_catalog(&self) -> Result<TuiModelCatalog> {
        let scope = self.management_scope();
        Ok(self.management.model.catalog(&scope).await?)
    }

    pub(crate) async fn available_agent_modes(&self) -> Result<Vec<TuiAgentMode>> {
        self.runtime.available_agent_modes().await
    }

    pub(crate) async fn list_models(&self) -> Result<ModelCatalog> {
        let scope = self.management_scope();
        Ok(self.management.model.list(&scope).await?)
    }

    pub(crate) async fn get_model(&self, model_id: String) -> Result<ModelEditProjection> {
        let scope = self.management_scope();
        Ok(self.management.model.get(&scope, &model_id).await?)
    }

    pub(crate) async fn add_model(&self, request: AddModelRequest) -> Result<()> {
        let scope = self.management_scope();
        Ok(self
            .management
            .model
            .add(&scope, request.model, request.make_primary_if_empty)
            .await?)
    }

    pub(crate) async fn update_model(&self, request: UpdateModelRequest) -> Result<()> {
        let scope = self.management_scope();
        Ok(self
            .management
            .model
            .update(&scope, &request.model_id, request.model)
            .await?)
    }

    pub(crate) async fn set_model_default(&self, request: SetModelDefaultRequest) -> Result<()> {
        let scope = self.management_scope();
        Ok(self
            .management
            .model
            .set_default(&scope, request.slot, request.model_id)
            .await?)
    }

    pub(crate) async fn list_skills(
        &self,
        mode_id: String,
        manageable: bool,
    ) -> Result<ListSkillsResponse> {
        let scope = self.management_scope();
        Ok(ListSkillsResponse {
            skills: self
                .management
                .registry
                .list_skills(&scope, &mode_id, manageable)
                .await?,
        })
    }

    pub(crate) async fn set_skill_enabled(
        &self,
        mode_id: String,
        skill_key: String,
        enabled: bool,
        default_enabled: bool,
        level: String,
    ) -> Result<()> {
        let scope = self.management_scope();
        Ok(self
            .management
            .registry
            .set_skill_enabled(
                &scope,
                &mode_id,
                &skill_key,
                enabled,
                default_enabled,
                &level,
            )
            .await?)
    }

    pub(crate) async fn list_subagents(
        &self,
        parent_mode_id: String,
        management: bool,
    ) -> Result<ListSubagentsResponse> {
        let scope = self.management_scope();
        let (subagents, has_external) = self
            .management
            .registry
            .list_subagents(&scope, &parent_mode_id, management, true)
            .await?;
        Ok(ListSubagentsResponse {
            subagents,
            has_external,
        })
    }

    pub(crate) async fn set_subagent_enabled(
        &self,
        parent_mode_id: String,
        subagent_id: String,
        enabled: bool,
    ) -> Result<()> {
        let scope = self.management_scope();
        Ok(self
            .management
            .registry
            .set_subagent_enabled(&scope, &parent_mode_id, &subagent_id, enabled)
            .await?)
    }

    pub(crate) async fn list_mcp_servers(&self) -> Result<McpServerList> {
        let scope = self.management_scope();
        Ok(self.management.mcp.list(&scope).await?)
    }

    pub(crate) async fn toggle_mcp_server(&self, server_id: String) -> Result<()> {
        let scope = self.management_scope();
        Ok(self.management.mcp.toggle(&scope, &server_id).await?)
    }

    pub(crate) async fn add_mcp_server(
        &self,
        name: String,
        config: McpServerMutation,
    ) -> Result<()> {
        let scope = self.management_scope();
        Ok(self.management.mcp.add(&scope, &name, config).await?)
    }

    pub(crate) async fn delete_mcp_server(&self, server_id: String) -> Result<()> {
        let scope = self.management_scope();
        Ok(self.management.mcp.delete(&scope, &server_id).await?)
    }

    pub(crate) async fn external_mcp_decision(
        &self,
        request: ExternalMcpDecisionRequest,
    ) -> Result<()> {
        let scope = self.management_scope();
        Ok(self.management.mcp.decide_external(&scope, request).await?)
    }

    pub(crate) async fn mcp_conflict_choice(
        &self,
        request: McpConflictChoiceRequest,
    ) -> Result<()> {
        let scope = self.management_scope();
        Ok(self.management.mcp.choose_conflict(&scope, request).await?)
    }

    pub(crate) async fn external_source_snapshot(
        &self,
        force_refresh: bool,
    ) -> std::result::Result<ExternalSourceSnapshotView, ExternalSourceOperationError> {
        let scope = self
            .local_management_scope("External source management")
            .map_err(external_source_management_error)?;
        self.ensure_external_source_subscription().await?;
        self.management
            .external_source
            .snapshot(&scope, force_refresh)
            .await
            .map_err(external_source_management_error)
    }

    pub(crate) fn subscribe_external_source_updates(
        &self,
    ) -> Result<broadcast::Receiver<(String, ExternalSourcePublicSnapshot)>> {
        Ok(self.external_source_events.subscribe())
    }

    pub(crate) async fn external_source_control(
        &self,
        request: ExternalSourceControlRequestV1,
    ) -> std::result::Result<ExternalSourceSnapshotView, ExternalSourceOperationError> {
        let scope = self
            .local_management_scope("External source management")
            .map_err(external_source_management_error)?;
        self.management
            .external_source
            .control(&scope, request)
            .await
            .map_err(external_source_management_error)
    }

    pub(crate) async fn external_source_review(
        &self,
        action: ExternalSourceReviewAction,
    ) -> std::result::Result<ExternalSourceSnapshotView, ExternalSourceOperationError> {
        let scope = self
            .local_management_scope("External source management")
            .map_err(external_source_management_error)?;
        let operation_id = format!("tui-{}", uuid::Uuid::new_v4());
        self.management
            .external_source
            .review(&scope, &operation_id, action)
            .await
            .map_err(external_source_management_error)
    }

    pub(crate) async fn set_native_command_choice(
        &self,
        native_commands: Vec<NativePromptCommandDescriptor>,
        selected_candidate_id: String,
        expected_preference_revision: u64,
    ) -> std::result::Result<NativeCommandChoiceView, ExternalSourceOperationError> {
        let scope = self
            .local_management_scope("External command management")
            .map_err(external_source_management_error)?;
        let operation_id = format!("tui-{}", uuid::Uuid::new_v4());
        self.management
            .external_command
            .set_native_choice(
                &scope,
                &operation_id,
                native_commands,
                &selected_candidate_id,
                expected_preference_revision,
            )
            .await
            .map(|(conflicts, preferences)| NativeCommandChoiceView {
                conflicts,
                preferences,
            })
            .map_err(external_source_management_error)
    }

    #[allow(clippy::too_many_arguments)]
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
    ) -> std::result::Result<PromptCommandInvocationOutcome, ExternalSourceOperationError> {
        let scope = self
            .local_management_scope("External command management")
            .map_err(external_source_management_error)?;
        self.management
            .external_command
            .expand(
                &scope,
                ExternalCommandExpansionRequest {
                    operation_id: format!("tui-{}", uuid::Uuid::new_v4()),
                    command_name,
                    arguments,
                    native_commands,
                    candidate_id,
                    content_version,
                    native_conflict_key,
                    expected_preference_revision,
                    shell_review_decision,
                },
            )
            .await
            .map_err(external_source_management_error)
    }

    pub(crate) async fn native_hook_overview(
        &self,
    ) -> std::result::Result<NativeHookOverviewView, ExternalSourceOperationError> {
        let scope = self
            .local_management_scope("Native hook management")
            .map_err(external_source_management_error)?;
        self.management
            .native_hook
            .overview(&scope)
            .await
            .map_err(external_source_management_error)
    }

    pub(crate) async fn external_hook_snapshot(
        &self,
        refresh_updates: bool,
    ) -> std::result::Result<ExternalHookImportSnapshotV1, ExternalSourceOperationError> {
        let scope = self
            .local_management_scope("External hook management")
            .map_err(external_source_management_error)?;
        self.management
            .external_hook
            .snapshot(&scope, refresh_updates)
            .await
            .map_err(external_source_management_error)
    }

    pub(crate) async fn external_hook_plan(
        &self,
        source: SourceKey,
    ) -> std::result::Result<ExternalHookImportPlanV1, ExternalSourceOperationError> {
        let scope = self
            .local_management_scope("External hook management")
            .map_err(external_source_management_error)?;
        self.management
            .external_hook
            .plan(&scope, source)
            .await
            .map_err(external_source_management_error)
    }

    pub(crate) async fn external_hook_apply(
        &self,
        request: ExternalHookImportApplyRequestV1,
    ) -> std::result::Result<ExternalHookImportApplyResultV1, ExternalSourceOperationError> {
        let scope = self
            .local_management_scope("External hook management")
            .map_err(external_source_management_error)?;
        self.management
            .external_hook
            .apply(
                &scope,
                &format!("tui-hook-{}", uuid::Uuid::new_v4()),
                request,
            )
            .await
            .map_err(external_source_management_error)
    }

    pub(crate) async fn external_hook_mutate(
        &self,
        request: ExternalHookImportMutationRequestV1,
    ) -> std::result::Result<ExternalHookImportSnapshotV1, ExternalSourceOperationError> {
        let scope = self
            .local_management_scope("External hook management")
            .map_err(external_source_management_error)?;
        self.management
            .external_hook
            .mutate(
                &scope,
                &format!("tui-hook-{}", uuid::Uuid::new_v4()),
                request,
            )
            .await
            .map_err(external_source_management_error)
    }

    pub(crate) async fn account_snapshot(&self) -> Result<AccountSnapshotResponse> {
        let scope = self.management_scope();
        Ok(self.management.account.snapshot(&scope).await?)
    }

    pub(crate) async fn account_login(
        &self,
        relay_url: String,
        username: String,
        password: String,
    ) -> Result<AccountLoginView> {
        let scope = self.management_scope();
        Ok(self
            .management
            .account
            .login(
                &scope,
                &account_operation_id(),
                &relay_url,
                &username,
                &password,
            )
            .await?)
    }

    pub(crate) async fn account_finalize_login(
        &self,
        choice: AccountSyncChoice,
    ) -> Result<AccountSnapshotResponse> {
        let scope = self.management_scope();
        Ok(self
            .management
            .account
            .finalize_login(
                &scope,
                account_operation_id(),
                choice == AccountSyncChoice::Local,
                self.project_workspace_path_buf(),
            )
            .await?)
    }

    pub(crate) async fn account_logout(&self) -> Result<AccountSnapshotResponse> {
        let scope = self.management_scope();
        Ok(self
            .management
            .account
            .logout(&scope, &account_operation_id())
            .await?)
    }

    pub(crate) async fn settings_sync_start(
        &self,
        is_first_login: bool,
    ) -> Result<SettingsSyncResponse> {
        let scope = self.management_scope();
        Ok(SettingsSyncResponse {
            progress: self
                .management
                .settings_sync
                .start(
                    &scope,
                    account_operation_id(),
                    is_first_login,
                    self.project_workspace_path_buf(),
                )
                .await?,
        })
    }

    pub(crate) async fn settings_sync_snapshot(&self) -> Result<SettingsSyncResponse> {
        let scope = self.management_scope();
        Ok(SettingsSyncResponse {
            progress: self.management.settings_sync.snapshot(&scope).await?,
        })
    }

    pub(crate) async fn settings_sync_cancel(&self) -> Result<SettingsSyncResponse> {
        let scope = self.management_scope();
        Ok(SettingsSyncResponse {
            progress: self
                .management
                .settings_sync
                .cancel(&scope, &account_operation_id())
                .await?,
        })
    }

    pub(crate) async fn settings_sync_local_changed(&self) -> Result<SettingsSyncResponse> {
        let scope = self.management_scope();
        Ok(SettingsSyncResponse {
            progress: self
                .management
                .settings_sync
                .local_changed(&scope, &account_operation_id())
                .await?,
        })
    }

    pub(crate) async fn worktree_repository_status(
        &self,
        workspace_path: String,
    ) -> Result<WorktreeRepositoryStatus> {
        let scope = management_scope(
            PathBuf::from(workspace_path),
            self.runtime.is_remote_workspace(),
        );
        Ok(self.management.worktree.repository_status(&scope).await?)
    }

    pub(crate) async fn worktree_bind_session(
        &self,
        session_id: String,
        project_workspace_path: Option<String>,
    ) -> Result<WorktreeBindingView> {
        self.worktree_transition(session_id, project_workspace_path, true)
            .await
    }

    pub(crate) async fn worktree_release_session(
        &self,
        session_id: String,
        project_workspace_path: Option<String>,
    ) -> Result<WorktreeBindingView> {
        self.worktree_transition(session_id, project_workspace_path, false)
            .await
    }

    async fn worktree_transition(
        &self,
        session_id: String,
        project_workspace_path: Option<String>,
        enabled: bool,
    ) -> Result<WorktreeBindingView> {
        let workspace = project_workspace_path
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.project_workspace_path_buf());
        let request = WorktreeBindingRequest {
            scope: management_scope(workspace, self.runtime.is_remote_workspace()),
            operation_id: worktree_operation_id(),
            session_id,
            project_workspace_path,
        };
        Ok(if enabled {
            self.management.worktree.bind(request).await?
        } else {
            self.management.worktree.release(request).await?
        })
    }

    pub(crate) fn subscribe_events(&self) -> Result<broadcast::Receiver<AgenticEventEnvelope>> {
        self.runtime
            .subscribe_events()
            .map_err(|error| anyhow::anyhow!(error.into_message()))
    }

    pub(crate) fn subscribe_permission_requests(
        &self,
    ) -> Result<broadcast::Receiver<PermissionRequestEvent>> {
        self.runtime
            .subscribe_permission_requests()
            .map_err(|error| anyhow::anyhow!(error.into_message()))
    }

    pub(crate) fn pending_permission_requests(&self) -> Result<Vec<PermissionRequest>> {
        self.runtime
            .pending_permission_requests()
            .map_err(|error| anyhow::anyhow!(error.into_message()))
    }

    pub(crate) async fn respond_permission(
        &self,
        request_id: &str,
        reply: PermissionReply,
    ) -> Result<()> {
        self.runtime.respond_permission(request_id, reply).await
    }

    pub(crate) fn set_approval_policy(&self, policy: CliApprovalPolicy) {
        self.runtime.set_approval_policy(policy);
    }

    pub(crate) fn workspace_path_string(&self) -> String {
        self.runtime.workspace_path_string()
    }

    pub(crate) fn project_workspace_path_string(&self) -> String {
        self.runtime.project_workspace_path_string()
    }

    fn project_workspace_path_buf(&self) -> PathBuf {
        self.runtime.project_workspace_path_buf()
    }

    pub(crate) fn set_workspace_binding(&self, binding: &AgentSessionWorkspaceBinding) {
        self.runtime.set_workspace_binding(binding);
    }

    pub(crate) async fn list_sessions(&self) -> Result<Vec<AgentSessionSummary>> {
        self.runtime.list_sessions().await
    }

    pub(crate) async fn session_lineage(
        &self,
        root_session_id: &str,
    ) -> Result<Option<AgentSessionLineageSnapshot>> {
        self.runtime.session_lineage(root_session_id).await
    }

    pub(crate) async fn inspect_lineage_session(
        &self,
        root_session_id: &str,
        target_session_id: &str,
        required_settled_turn_ids: &[String],
    ) -> std::result::Result<AgentSessionLineageInspection, SessionOperationError> {
        self.runtime
            .inspect_lineage_session(
                root_session_id,
                target_session_id,
                required_settled_turn_ids,
            )
            .await
    }

    pub(crate) async fn cancel_lineage_session(
        &self,
        root_session_id: &str,
        session_id: &str,
        expected_active_turn_id: &str,
    ) -> Result<AgentTurnCancellationResult> {
        self.runtime
            .cancel_lineage_session(root_session_id, session_id, expected_active_turn_id)
            .await
    }

    pub(crate) async fn restore_session_in_current_workspace(
        &self,
        session_id: &str,
    ) -> Result<CliSessionRestoreSnapshot> {
        self.runtime
            .restore_session_in_current_workspace(session_id)
            .await
    }

    pub(crate) async fn delete_session(
        &self,
        session_id: &str,
    ) -> std::result::Result<(), SessionOperationError> {
        self.runtime.delete_session(session_id).await
    }

    pub(crate) async fn update_session_model(
        &self,
        session_id: &str,
        model_id: &str,
    ) -> std::result::Result<(), SessionOperationError> {
        self.runtime
            .update_session_model(session_id, model_id)
            .await
    }

    pub(crate) async fn rename_session(
        &self,
        session_id: &str,
        session_name: &str,
    ) -> std::result::Result<(), SessionOperationError> {
        self.runtime.rename_session(session_id, session_name).await
    }

    pub(crate) async fn update_session_mode(
        &self,
        session_id: &str,
        mode_id: &str,
    ) -> std::result::Result<(), SessionOperationError> {
        self.runtime.update_session_mode(session_id, mode_id).await
    }

    pub(crate) async fn fork_current_session(
        &self,
        before_turn_id: Option<&str>,
    ) -> Result<(
        AgentSessionSummary,
        AgentSessionWorkspaceBinding,
        SessionTranscript,
    )> {
        self.runtime.fork_current_session(before_turn_id).await
    }

    pub(crate) async fn revert_current_session(
        &self,
        undo: bool,
    ) -> Result<AgentSessionRevertResult> {
        self.runtime.revert_current_session(undo).await
    }

    pub(crate) async fn workspace_diff(&self) -> Result<WorkspaceDiffSnapshot> {
        self.runtime.workspace_diff().await
    }

    pub(crate) async fn generate_session_usage_report(
        &self,
        request: AgentSessionUsageRequest,
    ) -> Result<SessionUsageReport> {
        self.runtime.generate_session_usage_report(request).await
    }

    pub(crate) async fn reload_context(&self, request: AgentContextReloadRequest) -> Result<()> {
        self.runtime.reload_context(request).await
    }

    pub(crate) async fn ensure_session_with_workspace_binding(
        &self,
        agent_type: &str,
    ) -> Result<(String, AgentSessionWorkspaceBinding)> {
        self.runtime
            .ensure_session_with_workspace_binding(agent_type)
            .await
    }

    pub(crate) async fn ensure_session_with_model(
        &self,
        agent_type: &str,
        model_id: Option<String>,
    ) -> Result<String> {
        self.runtime
            .ensure_session_with_model(agent_type, model_id)
            .await
    }

    pub(crate) async fn create_new_session_with_workspace_binding(
        &self,
        agent_type: &str,
    ) -> Result<(String, AgentSessionWorkspaceBinding)> {
        self.runtime
            .create_new_session_with_workspace_binding(agent_type)
            .await
    }

    pub(crate) async fn start_session_compaction(&self, session_id: &str) -> Result<String> {
        self.runtime.start_session_compaction(session_id).await
    }

    pub(crate) async fn send_message_with_context(
        &self,
        message: String,
        workspace_references: Vec<AgentWorkspaceReference>,
        attachments: Vec<AgentInputAttachment>,
        agent_type: &str,
    ) -> Result<String> {
        self.runtime
            .send_message_with_context(message, workspace_references, attachments, agent_type)
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
        self.runtime
            .send_external_subagent_command(
                prompt,
                original_command,
                ecosystem_id,
                logical_id,
                agent_type,
            )
            .await
    }

    pub(crate) async fn steer_current_turn(
        &self,
        content: String,
        display_content: Option<String>,
    ) -> Result<String> {
        self.runtime
            .steer_current_turn(content, display_content)
            .await
    }

    pub(crate) async fn run_user_shell_command(
        &self,
        command: String,
        agent_type: &str,
    ) -> Result<String> {
        self.runtime
            .run_user_shell_command(command, agent_type)
            .await
    }

    pub(crate) async fn search_workspace_references(
        &self,
        query: String,
    ) -> Result<AgentWorkspaceReferenceSearchResult> {
        self.runtime.search_workspace_references(query).await
    }

    pub(crate) async fn workspace_references_for_message(
        &self,
        session_id: String,
        message_id: String,
    ) -> Result<Vec<AgentWorkspaceReference>> {
        self.runtime
            .workspace_references_for_message(session_id, message_id)
            .await
    }

    pub(crate) async fn cancel_current_turn(&self) -> Result<()> {
        self.runtime.cancel_current_turn().await
    }

    pub(crate) async fn submit_user_answers(
        &self,
        session_id: String,
        turn_id: String,
        tool_id: String,
        registration_sequence: u64,
        answers: serde_json::Value,
    ) -> Result<()> {
        self.runtime
            .submit_user_answers(session_id, turn_id, tool_id, registration_sequence, answers)
            .await
    }

    async fn ensure_external_source_subscription(
        &self,
    ) -> std::result::Result<(), ExternalSourceOperationError> {
        let scope = self
            .local_management_scope("External source management")
            .map_err(external_source_management_error)?;
        let workspace = scope.workspace_path;
        let workspace_key = workspace.to_string_lossy().to_string();
        {
            let mut subscriptions = self
                .external_source_subscriptions
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !subscriptions.insert(workspace_key.clone()) {
                return Ok(());
            }
        }
        let mut subscription =
            match bitfun_core::external_sources::subscribe_external_source_updates(Some(&workspace))
                .await
            {
                Ok(subscription) => subscription,
                Err(error) => {
                    self.external_source_subscriptions
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .remove(&workspace_key);
                    return Err(
                        bitfun_core::external_sources::sanitize_external_source_operation_error(
                            error,
                        ),
                    );
                }
            };
        let sender = self.external_source_events.clone();
        let subscriptions = self.external_source_subscriptions.clone();
        let task = tokio::spawn(async move {
            loop {
                match subscription.recv().await {
                    Ok(snapshot) => {
                        let _ = sender.send((
                            workspace_key.clone(),
                            ExternalSourcePublicSnapshot::from(snapshot),
                        ));
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            subscriptions
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(&workspace_key);
        });
        self.external_source_tasks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(task);
        Ok(())
    }
}

impl Drop for TuiAgentClient {
    fn drop(&mut self) {
        for task in self
            .external_source_tasks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .drain(..)
        {
            task.abort();
        }
    }
}

fn management_scope(workspace_path: PathBuf, remote: bool) -> ManagementScope {
    if remote {
        ManagementScope::remote(workspace_path)
    } else {
        ManagementScope::local(workspace_path)
    }
}

fn account_operation_id() -> String {
    format!("tui-account-{}", uuid::Uuid::new_v4())
}

fn worktree_operation_id() -> String {
    format!("tui-worktree-{}", uuid::Uuid::new_v4())
}

fn external_source_management_error(error: ManagementError) -> ExternalSourceOperationError {
    if let Some(decoded) = ExternalSourceOperationError::decode(&error.message) {
        return decoded;
    }
    let code = match error.kind {
        ManagementErrorKind::InvalidRequest => ExternalSourceOperationErrorCode::InvalidRequest,
        ManagementErrorKind::NotFound => ExternalSourceOperationErrorCode::NotFound,
        ManagementErrorKind::Unsupported => ExternalSourceOperationErrorCode::Unsupported,
        ManagementErrorKind::Internal => ExternalSourceOperationErrorCode::Internal,
    };
    ExternalSourceOperationError::new(code, error.message, false).with_default_recovery_actions()
}
