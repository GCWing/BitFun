//! Desktop host API for ecosystem-neutral external AI application sources.

use openbitfun_core::external_sources::{
    acknowledge_external_ecosystems, apply_external_source_control_action,
    choose_external_mcp_conflict, choose_external_subagent_conflict,
    expand_external_prompt_command, external_source_location_for_host_action,
    external_source_snapshot,
    get_external_source_control_snapshot as core_get_external_source_control_snapshot,
    native_prompt_command_conflicts, set_external_mcp_server_decision,
    set_external_mcp_servers_enabled, set_external_prompt_command_conflict_choice,
    set_external_source_enabled, set_external_subagent_activation,
    set_external_subagent_model_binding, set_external_subagents_enabled,
    set_external_tool_conflict_choice, set_external_tool_target_decision,
    set_external_tool_targets_enabled, set_native_prompt_command_conflict_choice,
    unacknowledged_external_ecosystems, update_external_integration_policy,
    workspace_reference_snapshot, ExternalIntegrationPolicyMutation,
    ExternalSourceControlRequestV1, ExternalSourceHostCapabilities, ExternalSourceOperationError,
    ExternalSourceOperationErrorCode, ExternalSourceOperationResult, ExternalSourcePublicSnapshot,
    ExternalSourceSurfaceSnapshotV1, ExternalSubagentModelBindingTarget,
    NativePromptCommandConflictSnapshot, NativePromptCommandDescriptor,
    PromptCommandInvocationOutcome, PromptCommandShellReviewDecision,
};
use openbitfun_core::service::remote_ssh::workspace_state::is_remote_path;
use openbitfun_core::service::remote_ssh::workspace_state::{
    canonicalize_local_workspace_root, local_workspace_roots_equal,
};
use openbitfun_core::service::workspace::manager::WorkspaceKind;
use openbitfun_product_domains::external_sources::{
    ExternalMcpImportApplyRequestV1, ExternalMcpImportApplyResultV1, ExternalMcpImportPlanV1,
};
use openbitfun_product_domains::workspace_references::WorkspaceReferenceSnapshot;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::State;

use super::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceSnapshotRequest {
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub force_refresh: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceReferenceSnapshotRequest {
    pub workspace_path: String,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub force_refresh: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceControlCommandRequest {
    pub workspace_path: Option<String>,
    pub control: ExternalSourceControlRequestV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetExternalSourceEnabledRequest {
    pub workspace_path: Option<String>,
    pub source_key: String,
    pub enabled: bool,
    pub expected_preference_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevealExternalSourceLocationRequest {
    pub workspace_path: Option<String>,
    pub source_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalEcosystemAwarenessRequest {
    pub workspace_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalEcosystemAwarenessResponse {
    pub unacknowledged_ecosystem_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcknowledgeExternalEcosystemsRequest {
    pub workspace_path: Option<String>,
    pub ecosystem_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateExternalIntegrationPolicyRequest {
    pub workspace_path: Option<String>,
    pub mutation: ExternalIntegrationPolicyMutation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetExternalSourceConflictChoiceRequest {
    pub workspace_path: Option<String>,
    pub conflict_key: String,
    pub candidate_id: String,
    pub expected_preference_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativePromptCommandConflictsRequest {
    pub workspace_path: Option<String>,
    pub native_commands: Vec<NativePromptCommandDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetNativePromptCommandConflictChoiceRequest {
    pub workspace_path: Option<String>,
    pub native_commands: Vec<NativePromptCommandDescriptor>,
    pub selected_candidate_id: String,
    pub expected_preference_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpandExternalPromptCommandRequest {
    pub workspace_path: Option<String>,
    pub name: String,
    #[serde(default)]
    pub arguments: String,
    pub native_commands: Vec<NativePromptCommandDescriptor>,
    pub candidate_id: String,
    pub expected_content_version: String,
    #[serde(default)]
    pub expected_native_conflict_key: Option<String>,
    #[serde(default)]
    pub expected_preference_revision: Option<u64>,
    #[serde(default)]
    pub shell_review_decision: Option<PromptCommandShellReviewDecision>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetExternalToolTargetDecisionRequest {
    pub workspace_path: Option<String>,
    pub approval_key: String,
    pub decision_key: String,
    pub approved: bool,
    pub expected_preference_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalToolDecisionRef {
    pub approval_key: String,
    pub decision_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetExternalToolTargetsEnabledRequest {
    pub workspace_path: Option<String>,
    pub decisions: Vec<ExternalToolDecisionRef>,
    pub enabled: bool,
    pub expected_catalog_generation: u64,
    pub expected_preference_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetExternalToolConflictChoiceRequest {
    pub workspace_path: Option<String>,
    pub conflict_key: String,
    pub candidate_id: String,
    pub expected_preference_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetExternalSubagentActivationRequest {
    pub workspace_path: Option<String>,
    pub candidate_id: String,
    pub approved: bool,
    pub expected_subagent_generation: u64,
    pub expected_preference_revision: u64,
    pub decision_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalCandidateDecisionRef {
    pub candidate_id: String,
    pub decision_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetExternalSubagentsEnabledRequest {
    pub workspace_path: Option<String>,
    pub decisions: Vec<ExternalCandidateDecisionRef>,
    pub enabled: bool,
    pub expected_subagent_generation: u64,
    pub expected_preference_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetExternalSubagentModelBindingRequest {
    pub workspace_path: Option<String>,
    pub binding_key: String,
    pub target: Option<ExternalSubagentModelBindingTarget>,
    pub expected_subagent_generation: u64,
    pub expected_preference_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChooseExternalSubagentConflictRequest {
    pub workspace_path: Option<String>,
    pub conflict_key: String,
    pub candidate_id: String,
    #[serde(default)]
    pub approve_external: bool,
    pub expected_subagent_generation: u64,
    pub expected_preference_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetExternalMcpServerDecisionRequest {
    pub workspace_path: Option<String>,
    pub candidate_id: String,
    pub decision_key: String,
    pub approved: bool,
    pub expected_mcp_generation: u64,
    pub expected_preference_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetExternalMcpServersEnabledRequest {
    pub workspace_path: Option<String>,
    pub decisions: Vec<ExternalCandidateDecisionRef>,
    pub enabled: bool,
    pub expected_mcp_generation: u64,
    pub expected_preference_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChooseExternalMcpConflictRequest {
    pub workspace_path: Option<String>,
    pub conflict_key: String,
    pub candidate_id: String,
    #[serde(default)]
    pub approve_external: bool,
    pub expected_mcp_generation: u64,
    pub expected_preference_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanExternalMcpImportRequest {
    pub workspace_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyExternalMcpImportRequest {
    pub workspace_path: Option<String>,
    pub import_request: ExternalMcpImportApplyRequestV1,
}

pub type ExternalSourceSnapshotResponse = ExternalSourcePublicSnapshot;
pub type ExternalSourceControlResponse = ExternalSourceSurfaceSnapshotV1;
pub type ExpandExternalPromptCommandResponse = PromptCommandInvocationOutcome;
pub type NativePromptCommandConflictsResponse = NativePromptCommandConflictSnapshot;
pub type WorkspaceReferenceResponse = WorkspaceReferenceSnapshot;

#[tauri::command]
pub async fn plan_external_mcp_import_command(
    request: PlanExternalMcpImportRequest,
) -> ExternalSourceOperationResult<ExternalMcpImportPlanV1> {
    let workspace = require_local_workspace(request.workspace_path.as_deref())
        .await?
        .map(Path::to_path_buf);
    openbitfun_core::external_mcp_import::plan_external_mcp_import(workspace).await
}

#[tauri::command]
pub async fn apply_external_mcp_import_command(
    request: ApplyExternalMcpImportRequest,
) -> ExternalSourceOperationResult<ExternalMcpImportApplyResultV1> {
    let workspace = require_local_workspace(request.workspace_path.as_deref())
        .await?
        .map(Path::to_path_buf);
    openbitfun_core::external_mcp_import::apply_external_mcp_import(
        workspace,
        request.import_request,
    )
    .await
}

pub(super) async fn require_local_workspace(
    workspace_path: Option<&str>,
) -> ExternalSourceOperationResult<Option<&Path>> {
    let Some(workspace_path) = workspace_path else {
        return Ok(None);
    };
    if is_remote_path(workspace_path).await {
        return Err(ExternalSourceOperationError::new(
            ExternalSourceOperationErrorCode::HostUnavailable,
            "The remote workspace is not running the external compatibility service",
            true,
        ));
    }
    let path = Path::new(workspace_path);
    if !path.is_absolute() {
        return Err(ExternalSourceOperationError::invalid_request(
            "External AI application sources require an absolute workspace path",
        ));
    }
    Ok(Some(path))
}

#[tauri::command]
pub async fn update_external_integration_policy_command(
    request: UpdateExternalIntegrationPolicyRequest,
) -> ExternalSourceOperationResult<ExternalSourceSnapshotResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    update_external_integration_policy(workspace, request.mutation)
        .await
        .map(Into::into)
        .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn get_external_source_snapshot(
    request: ExternalSourceSnapshotRequest,
) -> ExternalSourceOperationResult<ExternalSourceSnapshotResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    external_source_snapshot(workspace, request.force_refresh)
        .await
        .map(|snapshot| ExternalSourcePublicSnapshot::from(snapshot).into_legacy_v0_compatible())
        .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn get_workspace_reference_snapshot(
    state: State<'_, AppState>,
    request: WorkspaceReferenceSnapshotRequest,
) -> ExternalSourceOperationResult<WorkspaceReferenceResponse> {
    let requested_workspace = Path::new(&request.workspace_path);
    if !requested_workspace.is_absolute() {
        return Err(ExternalSourceOperationError::invalid_request(
            "Workspace references require an absolute workspace path",
        ));
    }
    let workspace_id = request
        .workspace_id
        .as_deref()
        .map(str::trim)
        .filter(|workspace_id| !workspace_id.is_empty());
    let workspace_info = match workspace_id {
        Some(workspace_id) => state.workspace_service.get_workspace(workspace_id).await,
        None => {
            state
                .workspace_service
                .get_workspace_by_path(requested_workspace)
                .await
        }
    };
    ensure_registered_workspace_reference_kind(
        workspace_info
            .as_ref()
            .map(|workspace| &workspace.workspace_kind),
    )?;
    let workspace_info = workspace_info.expect("registered workspace was validated above");
    let path_matches_registered_workspace = if workspace_reference_path_matches_registered_root(
        &workspace_info.root_path,
        requested_workspace,
    ) {
        true
    } else {
        state
            .workspace_service
            .is_live_worktree_root_in_same_repository(
                &workspace_info.root_path,
                requested_workspace,
            )
            .await
            .unwrap_or(false)
    };
    if !path_matches_registered_workspace {
        return Err(ExternalSourceOperationError::invalid_request(
            "Workspace reference path does not match the registered workspace or one of its Git worktrees",
        ));
    }
    let workspace = require_local_workspace(Some(&request.workspace_path))
        .await?
        .ok_or_else(|| {
            ExternalSourceOperationError::invalid_request(
                "Workspace references require a local workspace path",
            )
        })?;
    let native_related_paths = workspace_info.related_paths;
    workspace_reference_snapshot(workspace, &native_related_paths, request.force_refresh)
        .await
        .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

fn ensure_registered_workspace_reference_kind(
    workspace_kind: Option<&WorkspaceKind>,
) -> ExternalSourceOperationResult<()> {
    match workspace_kind {
        None => Err(ExternalSourceOperationError::new(
            ExternalSourceOperationErrorCode::NotFound,
            "Workspace references require a registered workspace",
            false,
        )),
        Some(WorkspaceKind::Remote) => Err(ExternalSourceOperationError::new(
            ExternalSourceOperationErrorCode::HostUnavailable,
            "The remote workspace is not running the external compatibility service",
            true,
        )),
        Some(WorkspaceKind::Normal | WorkspaceKind::Assistant) => Ok(()),
    }
}

fn workspace_reference_path_matches_registered_root(
    registered_root: &Path,
    requested_path: &Path,
) -> bool {
    let Ok((requested_path, _)) = canonicalize_local_workspace_root(requested_path) else {
        return false;
    };
    local_workspace_roots_equal(registered_root, &requested_path)
}

#[tauri::command]
pub async fn reveal_external_source_location(
    request: RevealExternalSourceLocationRequest,
) -> ExternalSourceOperationResult<()> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    let path = external_source_location_for_host_action(workspace, &request.source_key)
        .await
        .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)?;
    super::commands::reveal_local_path_in_explorer(&path, &request.source_key)
        .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn get_external_source_control_snapshot(
    request: ExternalSourceSnapshotRequest,
) -> ExternalSourceOperationResult<ExternalSourceControlResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    core_get_external_source_control_snapshot(
        workspace,
        request.force_refresh,
        ExternalSourceHostCapabilities::local_desktop(),
    )
    .await
}

#[tauri::command]
pub async fn apply_external_source_control_action_command(
    request: ExternalSourceControlCommandRequest,
) -> ExternalSourceOperationResult<ExternalSourceControlResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    apply_external_source_control_action(workspace, request.control).await
}

/// External applications discovered on this host that the user has never been
/// told about. Surfaces use it to show a low-key "something new" affordance.
#[tauri::command]
pub async fn get_external_ecosystem_awareness_command(
    request: ExternalEcosystemAwarenessRequest,
) -> ExternalSourceOperationResult<ExternalEcosystemAwarenessResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    unacknowledged_external_ecosystems(workspace)
        .await
        .map(
            |unacknowledged_ecosystem_ids| ExternalEcosystemAwarenessResponse {
                unacknowledged_ecosystem_ids,
            },
        )
        .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

/// Records that the user has seen these external applications.
///
/// This only clears the "new application" hint. It grants nothing, so it takes
/// no expected preference revision and leaves approvals and policy untouched.
#[tauri::command]
pub async fn acknowledge_external_ecosystems_command(
    request: AcknowledgeExternalEcosystemsRequest,
) -> ExternalSourceOperationResult<()> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    acknowledge_external_ecosystems(workspace, request.ecosystem_ids)
        .await
        .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn set_external_source_enabled_command(
    request: SetExternalSourceEnabledRequest,
) -> ExternalSourceOperationResult<ExternalSourceSnapshotResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    set_external_source_enabled(
        workspace,
        &request.source_key,
        request.enabled,
        request.expected_preference_revision,
    )
    .await
    .map(Into::into)
    .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn set_external_source_conflict_choice_command(
    request: SetExternalSourceConflictChoiceRequest,
) -> ExternalSourceOperationResult<ExternalSourceSnapshotResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    set_external_prompt_command_conflict_choice(
        workspace,
        &request.conflict_key,
        &request.candidate_id,
        request.expected_preference_revision,
    )
    .await
    .map(Into::into)
    .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn get_native_prompt_command_conflicts_command(
    request: NativePromptCommandConflictsRequest,
) -> ExternalSourceOperationResult<NativePromptCommandConflictsResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    native_prompt_command_conflicts(workspace, request.native_commands)
        .await
        .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn set_native_prompt_command_conflict_choice_command(
    request: SetNativePromptCommandConflictChoiceRequest,
) -> ExternalSourceOperationResult<NativePromptCommandConflictsResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    set_native_prompt_command_conflict_choice(
        workspace,
        request.native_commands,
        &request.selected_candidate_id,
        request.expected_preference_revision,
    )
    .await
    .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn expand_external_prompt_command_command(
    request: ExpandExternalPromptCommandRequest,
) -> ExternalSourceOperationResult<ExpandExternalPromptCommandResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    expand_external_prompt_command(
        workspace,
        &request.name,
        &request.arguments,
        request.native_commands,
        Some(&request.candidate_id),
        Some(&request.expected_content_version),
        request.expected_native_conflict_key.as_deref(),
        request.expected_preference_revision,
        request.shell_review_decision.as_ref(),
    )
    .await
    .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn set_external_tool_target_decision_command(
    request: SetExternalToolTargetDecisionRequest,
) -> ExternalSourceOperationResult<ExternalSourceSnapshotResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    set_external_tool_target_decision(
        workspace,
        &request.approval_key,
        &request.decision_key,
        request.approved,
        request.expected_preference_revision,
    )
    .await
    .map(Into::into)
    .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn set_external_tool_targets_enabled_command(
    request: SetExternalToolTargetsEnabledRequest,
) -> ExternalSourceOperationResult<ExternalSourceSnapshotResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    set_external_tool_targets_enabled(
        workspace,
        request
            .decisions
            .into_iter()
            .map(|decision| (decision.approval_key, decision.decision_key))
            .collect(),
        request.enabled,
        request.expected_catalog_generation,
        request.expected_preference_revision,
    )
    .await
    .map(Into::into)
    .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn set_external_tool_conflict_choice_command(
    request: SetExternalToolConflictChoiceRequest,
) -> ExternalSourceOperationResult<ExternalSourceSnapshotResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    set_external_tool_conflict_choice(
        workspace,
        &request.conflict_key,
        &request.candidate_id,
        request.expected_preference_revision,
    )
    .await
    .map(Into::into)
    .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn set_external_subagent_activation_command(
    request: SetExternalSubagentActivationRequest,
) -> ExternalSourceOperationResult<ExternalSourceSnapshotResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    set_external_subagent_activation(
        workspace,
        &request.candidate_id,
        request.approved,
        request.expected_subagent_generation,
        request.expected_preference_revision,
        &request.decision_key,
    )
    .await
    .map(Into::into)
    .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn set_external_subagents_enabled_command(
    request: SetExternalSubagentsEnabledRequest,
) -> ExternalSourceOperationResult<ExternalSourceSnapshotResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    set_external_subagents_enabled(
        workspace,
        request
            .decisions
            .into_iter()
            .map(|decision| (decision.candidate_id, decision.decision_key))
            .collect(),
        request.enabled,
        request.expected_subagent_generation,
        request.expected_preference_revision,
    )
    .await
    .map(Into::into)
    .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn set_external_subagent_model_binding_command(
    request: SetExternalSubagentModelBindingRequest,
) -> ExternalSourceOperationResult<ExternalSourceSnapshotResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    set_external_subagent_model_binding(
        workspace,
        &request.binding_key,
        request.target,
        request.expected_subagent_generation,
        request.expected_preference_revision,
    )
    .await
    .map(Into::into)
    .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn choose_external_subagent_conflict_command(
    request: ChooseExternalSubagentConflictRequest,
) -> ExternalSourceOperationResult<ExternalSourceSnapshotResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    choose_external_subagent_conflict(
        workspace,
        &request.conflict_key,
        &request.candidate_id,
        request.approve_external,
        request.expected_subagent_generation,
        request.expected_preference_revision,
    )
    .await
    .map(Into::into)
    .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn set_external_mcp_server_decision_command(
    request: SetExternalMcpServerDecisionRequest,
) -> ExternalSourceOperationResult<ExternalSourceSnapshotResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    set_external_mcp_server_decision(
        workspace,
        &request.candidate_id,
        &request.decision_key,
        request.approved,
        request.expected_mcp_generation,
        request.expected_preference_revision,
    )
    .await
    .map(Into::into)
    .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn set_external_mcp_servers_enabled_command(
    request: SetExternalMcpServersEnabledRequest,
) -> ExternalSourceOperationResult<ExternalSourceSnapshotResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    set_external_mcp_servers_enabled(
        workspace,
        request
            .decisions
            .into_iter()
            .map(|decision| (decision.candidate_id, decision.decision_key))
            .collect(),
        request.enabled,
        request.expected_mcp_generation,
        request.expected_preference_revision,
    )
    .await
    .map(Into::into)
    .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn choose_external_mcp_conflict_command(
    request: ChooseExternalMcpConflictRequest,
) -> ExternalSourceOperationResult<ExternalSourceSnapshotResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    choose_external_mcp_conflict(
        workspace,
        &request.conflict_key,
        &request.candidate_id,
        request.approve_external,
        request.expected_mcp_generation,
        request.expected_preference_revision,
    )
    .await
    .map(Into::into)
    .map_err(openbitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbitfun_core::external_sources::{
        ExternalSourceCatalogSnapshot, ExternalSourceControlActionV1,
    };

    #[test]
    fn workspace_references_fail_closed_without_registered_local_metadata() {
        let missing = ensure_registered_workspace_reference_kind(None).unwrap_err();
        assert_eq!(missing.code, ExternalSourceOperationErrorCode::NotFound);

        let remote =
            ensure_registered_workspace_reference_kind(Some(&WorkspaceKind::Remote)).unwrap_err();
        assert_eq!(
            remote.code,
            ExternalSourceOperationErrorCode::HostUnavailable
        );

        assert!(ensure_registered_workspace_reference_kind(Some(&WorkspaceKind::Normal)).is_ok());
        assert!(
            ensure_registered_workspace_reference_kind(Some(&WorkspaceKind::Assistant)).is_ok()
        );
    }

    #[test]
    fn workspace_reference_paths_do_not_accept_unrelated_or_stale_local_paths() {
        let directory = tempfile::tempdir().unwrap();
        let registered_root = directory.path().join("registered");
        let unrelated_root = directory.path().join("unrelated");
        for path in [&registered_root, &unrelated_root] {
            std::fs::create_dir_all(path).unwrap();
        }

        assert!(workspace_reference_path_matches_registered_root(
            &registered_root,
            &registered_root
        ));
        assert!(!workspace_reference_path_matches_registered_root(
            &registered_root,
            &unrelated_root
        ));
        assert!(!workspace_reference_path_matches_registered_root(
            &registered_root,
            Path::new("/stale/remote/workspace")
        ));
    }

    #[test]
    fn desktop_snapshot_never_serializes_prompt_templates() {
        let snapshot: ExternalSourceCatalogSnapshot = serde_json::from_value(serde_json::json!({
            "generation": 1,
            "discoveryPending": false,
            "sources": [],
            "commands": [{
                "definition": {
                    "id": {
                        "source": { "providerId": "opencode.commands", "sourceId": "global" },
                        "localId": "review"
                    },
                    "name": "review",
                    "description": "Review changes",
                    "template": "sensitive prompt body",
                    "availability": { "state": "available" },
                    "contentVersion": "v1"
                }
            }],
            "commandConflicts": [],
            "diagnostics": []
        }))
        .unwrap();

        let value = serde_json::to_value(ExternalSourceSnapshotResponse::from(snapshot)).unwrap();

        assert_eq!(
            value["commands"][0]["candidateId"],
            "17:opencode.commands6:global6:review"
        );
        assert_eq!(value["commands"][0]["definition"]["name"], "review");
        assert!(value["commands"][0]["definition"].get("template").is_none());
    }

    #[test]
    fn desktop_control_command_deserializes_the_shared_action() {
        let request: ExternalSourceControlCommandRequest =
            serde_json::from_value(serde_json::json!({
                "workspacePath": null,
                "control": {
                    "schemaVersion": 1,
                    "operationId": "desktop-safe-mode",
                    "expectedPreferenceRevision": 7,
                    "action": { "type": "set_safe_mode", "enabled": true }
                }
            }))
            .unwrap();

        assert!(matches!(
            request.control.action,
            ExternalSourceControlActionV1::SetSafeMode { enabled: true }
        ));
    }

    #[test]
    fn desktop_subagent_model_binding_request_keeps_the_target_typed_and_nullable() {
        let set: SetExternalSubagentModelBindingRequest =
            serde_json::from_value(serde_json::json!({
                "workspacePath": "D:/workspace/project",
                "bindingKey": "external_subagent_model_binding:review",
                "target": { "kind": "model", "modelId": "glm-project" },
                "expectedSubagentGeneration": 5,
                "expectedPreferenceRevision": 8
            }))
            .unwrap();
        assert_eq!(
            set.target,
            Some(ExternalSubagentModelBindingTarget::Model {
                model_id: "glm-project".to_string(),
            })
        );

        let clear: SetExternalSubagentModelBindingRequest =
            serde_json::from_value(serde_json::json!({
                "workspacePath": null,
                "bindingKey": "external_subagent_model_binding:review",
                "target": null,
                "expectedSubagentGeneration": 5,
                "expectedPreferenceRevision": 8
            }))
            .unwrap();
        assert_eq!(clear.target, None);
    }

    #[test]
    fn desktop_prompt_expansion_request_requires_guarded_candidate_identity() {
        let request: ExpandExternalPromptCommandRequest =
            serde_json::from_value(serde_json::json!({
                "workspacePath": "D:/workspace/project",
                "name": "review",
                "arguments": "focus on auth",
                "nativeCommands": [{
                    "commandName": "review",
                    "candidateId": "openbitfun.desktop:action:review",
                    "behaviorVersion": "action:review:v1"
                }],
                "candidateId": "claude-code.commands:project:review",
                "expectedContentVersion": "behavior-v1"
            }))
            .unwrap();

        assert_eq!(request.name, "review");
        assert_eq!(request.arguments, "focus on auth");
        assert_eq!(request.native_commands.len(), 1);
        assert_eq!(request.candidate_id, "claude-code.commands:project:review");
        assert!(request.shell_review_decision.is_none());

        let approved: ExpandExternalPromptCommandRequest =
            serde_json::from_value(serde_json::json!({
                "workspacePath": "D:/workspace/project",
                "name": "review",
                "arguments": "",
                "nativeCommands": [],
                "candidateId": "opencode.commands:project:review",
                "expectedContentVersion": "behavior-v2",
                "shellReviewDecision": {
                    "planFingerprint": "sha256:plan-v2",
                    "mode": "run_once",
                    "expectedPreferenceRevision": 9
                }
            }))
            .unwrap();
        assert_eq!(
            approved
                .shell_review_decision
                .as_ref()
                .map(|decision| decision.plan_fingerprint.as_str()),
            Some("sha256:plan-v2")
        );
        assert!(
            serde_json::from_value::<ExpandExternalPromptCommandRequest>(serde_json::json!({
                "name": "review",
                "arguments": "",
                "candidateId": "claude-code.commands:project:review",
                "expectedContentVersion": "behavior-v1"
            }))
            .is_err()
        );
    }

    #[test]
    fn desktop_external_mcp_import_requests_use_structured_sanitized_shapes() {
        let plan: PlanExternalMcpImportRequest = serde_json::from_value(serde_json::json!({
            "workspacePath": "D:/workspace/project"
        }))
        .unwrap();
        assert_eq!(plan.workspace_path.as_deref(), Some("D:/workspace/project"));

        let apply: ApplyExternalMcpImportRequest = serde_json::from_value(serde_json::json!({
            "workspacePath": null,
            "importRequest": {
                "schemaVersion": 1,
                "planFingerprint": "sha256:plan-v1",
                "selections": [{
                    "candidateId": "external_mcp:17:opencode.commands6:global4:docs",
                    "requestedNativeId": "docs"
                }]
            }
        }))
        .unwrap();
        assert_eq!(apply.import_request.selections.len(), 1);

        assert!(
            serde_json::from_value::<PlanExternalMcpImportRequest>(serde_json::json!({
                "workspacePath": null,
                "rawSource": { "args": ["secret"] }
            }))
            .is_err()
        );
    }
}
