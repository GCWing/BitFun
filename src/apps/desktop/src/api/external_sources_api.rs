//! Desktop host API for ecosystem-neutral external AI application sources.

use bitfun_core::external_sources::{
    apply_external_source_control_action, choose_external_mcp_conflict,
    choose_external_subagent_conflict, expand_external_prompt_command,
    external_source_location_for_host_action, external_source_snapshot,
    get_external_source_control_snapshot as core_get_external_source_control_snapshot,
    native_prompt_command_conflicts, set_external_mcp_server_decision,
    set_external_prompt_command_conflict_choice, set_external_source_enabled,
    set_external_subagent_activation, set_external_tool_conflict_choice,
    set_external_tool_target_decision, set_native_prompt_command_conflict_choice,
    update_external_integration_policy, ExpandedPromptCommand, ExternalIntegrationPolicyMutation,
    ExternalSourceControlRequestV1, ExternalSourceHostCapabilities, ExternalSourceOperationError,
    ExternalSourceOperationErrorCode, ExternalSourceOperationResult, ExternalSourcePublicSnapshot,
    ExternalSourceSurfaceSnapshotV1, NativePromptCommandConflictSnapshot,
    NativePromptCommandDescriptor,
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
pub type ExternalSourceControlResponse = ExternalSourceSurfaceSnapshotV1;
pub type ExpandExternalPromptCommandResponse = ExpandedPromptCommand;
pub type NativePromptCommandConflictsResponse = NativePromptCommandConflictSnapshot;

pub(super) async fn require_local_workspace(
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
        .map(|snapshot| ExternalSourcePublicSnapshot::from(snapshot).into_legacy_v0_compatible())
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
pub async fn get_native_prompt_command_conflicts_command(
    request: NativePromptCommandConflictsRequest,
) -> ExternalSourceOperationResult<NativePromptCommandConflictsResponse> {
    let workspace = require_local_workspace(request.workspace_path.as_deref()).await?;
    native_prompt_command_conflicts(workspace, request.native_commands)
        .await
        .map_err(bitfun_core::external_sources::sanitize_external_source_operation_error)
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
    .map_err(bitfun_core::external_sources::sanitize_external_source_operation_error)
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
    )
    .await
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
    use bitfun_core::external_sources::{
        ExternalSourceCatalogSnapshot, ExternalSourceControlActionV1,
    };

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
    fn desktop_prompt_expansion_request_requires_guarded_candidate_identity() {
        let request: ExpandExternalPromptCommandRequest =
            serde_json::from_value(serde_json::json!({
                "workspacePath": "D:/workspace/project",
                "name": "review",
                "arguments": "focus on auth",
                "nativeCommands": [{
                    "commandName": "review",
                    "candidateId": "bitfun.desktop:action:review",
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
}
