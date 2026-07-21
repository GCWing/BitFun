//! Desktop host API for ecosystem-neutral external AI application sources.

use bitfun_core::external_sources::{
    choose_external_mcp_conflict, choose_external_subagent_conflict,
    external_source_location_for_host_action, external_source_snapshot,
    set_external_mcp_server_decision, set_external_prompt_command_conflict_choice,
    set_external_source_enabled, set_external_subagent_activation,
    set_external_tool_conflict_choice, set_external_tool_target_decision,
    update_external_integration_policy, ExternalIntegrationPolicyMutation,
    ExternalSourceOperationError, ExternalSourceOperationErrorCode, ExternalSourceOperationResult,
    ExternalSourcePublicSnapshot,
};
use bitfun_core::service::remote_ssh::workspace_state::is_remote_path;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceSnapshotRequest {
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub force_refresh: bool,
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
pub struct SetExternalToolTargetDecisionRequest {
    pub workspace_path: Option<String>,
    pub approval_key: String,
    pub decision_key: String,
    pub approved: bool,
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
pub struct ChooseExternalMcpConflictRequest {
    pub workspace_path: Option<String>,
    pub conflict_key: String,
    pub candidate_id: String,
    #[serde(default)]
    pub approve_external: bool,
    pub expected_mcp_generation: u64,
    pub expected_preference_revision: u64,
}

pub type ExternalSourceSnapshotResponse = ExternalSourcePublicSnapshot;

async fn require_local_workspace(
    workspace_path: Option<&str>,
) -> ExternalSourceOperationResult<Option<&Path>> {
    let Some(workspace_path) = workspace_path else {
        return Ok(None);
    };
    let path = Path::new(workspace_path);
    if !path.is_absolute() {
        return Err(ExternalSourceOperationError::invalid_request(
            "External AI application sources require an absolute workspace path",
        ));
    }
    if is_remote_path(workspace_path).await {
        return Err(ExternalSourceOperationError::new(
            ExternalSourceOperationErrorCode::HostUnavailable,
            "The remote workspace is not running the external compatibility service",
            true,
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
        .map_err(bitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn get_external_source_snapshot(
    request: ExternalSourceSnapshotRequest,
) -> ExternalSourceOperationResult<ExternalSourceSnapshotResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    external_source_snapshot(workspace, request.force_refresh)
        .await
        .map(Into::into)
        .map_err(bitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[tauri::command]
pub async fn reveal_external_source_location(
    request: RevealExternalSourceLocationRequest,
) -> ExternalSourceOperationResult<()> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    let path = external_source_location_for_host_action(workspace, &request.source_key)
        .await
        .map_err(bitfun_core::external_sources::sanitize_external_source_operation_error)?;
    super::commands::reveal_local_path_in_explorer(&path, &request.source_key)
        .map_err(bitfun_core::external_sources::sanitize_external_source_operation_error)
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
    .map_err(bitfun_core::external_sources::sanitize_external_source_operation_error)
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
    .map_err(bitfun_core::external_sources::sanitize_external_source_operation_error)
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
    .map_err(bitfun_core::external_sources::sanitize_external_source_operation_error)
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
    .map_err(bitfun_core::external_sources::sanitize_external_source_operation_error)
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
    .map_err(bitfun_core::external_sources::sanitize_external_source_operation_error)
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
    .map_err(bitfun_core::external_sources::sanitize_external_source_operation_error)
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
    .map_err(bitfun_core::external_sources::sanitize_external_source_operation_error)
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
    .map_err(bitfun_core::external_sources::sanitize_external_source_operation_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_core::external_sources::ExternalSourceCatalogSnapshot;

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

        assert_eq!(value["commands"][0]["definition"]["name"], "review");
        assert!(value["commands"][0]["definition"].get("template").is_none());
    }
}
