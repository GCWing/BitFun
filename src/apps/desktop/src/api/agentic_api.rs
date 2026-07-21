//! Agentic API

use log::{debug, warn};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, State};

use crate::api::app_state::AppState;
use crate::api::session_storage_path::desktop_effective_session_storage_path;
use crate::runtime::{
    DesktopRuntimeContext, DesktopSessionApplicationError, DesktopSessionScopeRequest,
};
use crate::startup_trace::DesktopStartupTrace;
use bitfun_agent_runtime::sdk::{
    AgentDialogTurnRequest, AgentInputAttachment, AgentSessionModelUpdateRequest,
    AgentSubmissionSource, AgentTurnCancellationRequest, PermissionAuditRecord, PermissionGrant,
    PermissionGrantKey, PermissionReply, PermissionRequest,
};
use bitfun_core::agentic::agents::AgentSource;
use bitfun_core::agentic::coordination::{
    AssistantBootstrapBlockReason, AssistantBootstrapEnsureOutcome, AssistantBootstrapSkipReason,
    ConversationCoordinator, DialogScheduler, DialogSubmissionPolicy, DialogTriggerSource,
    SubagentTimeoutAction,
};
use bitfun_core::agentic::core::*;
use bitfun_core::agentic::deep_review_policy::{
    apply_deep_review_queue_control, default_review_team_definition, DeepReviewQueueControlAction,
    ReviewTeamDefinition,
};
use bitfun_core::agentic::goal_mode::{ThreadGoal, ThreadGoalStatus};
use bitfun_core::agentic::image_analysis::ImageContextData;
use bitfun_core::agentic::memories::{db::MemoryDatabase, workspace::reset_memory_workspace};
use bitfun_core::agentic::session::SessionViewRestoreTiming;
use bitfun_core::agentic::tools::image_context::get_image_context;
use bitfun_core::agentic::tools::implementations::exec_command::{
    background_command_output_capture, control_exec_command_session, send_exec_command_input,
    ExecCommandControlAction, ExecCommandControlOrigin, ExecCommandControlRequest,
    ExecCommandInputRequest, ListBackgroundCommandOutputRequest,
    ListBackgroundCommandOutputResponse,
    ReadBackgroundCommandOutputRequest as CoreReadBackgroundCommandOutputRequest,
    ReadBackgroundCommandOutputResponse,
};
use bitfun_core::service::config::project_permission_store::{
    deserialize_project_permission_config, project_permission_file_path,
    project_permission_file_path_for_remote, ProjectPermissionConfig,
};
use bitfun_core::service::remote_ssh::workspace_state::resolve_workspace_session_identity;
use bitfun_core::service::session::{
    DialogTurnData, SessionMemoryMode, SessionMetadata, SessionRelationship,
    SessionRelationshipKind,
};
use bitfun_core::service::workspace::WorkspaceKind;
use bitfun_product_domains::tool_permissions::PermissionRule;

const SESSION_VIEW_TOOL_RESULT_TOTAL_CHAR_BUDGET: usize = 512 * 1024;
const SESSION_VIEW_TOOL_RESULT_STRING_CHAR_LIMIT: usize = 16 * 1024;
const SESSION_VIEW_TRUNCATED_MARKER: &str = "\n... Output truncated for session preview";
const SESSION_VIEW_OMITTED_MARKER: &str = "Output omitted from session preview";

fn desktop_session_scope(
    workspace_path: String,
    remote_connection_id: Option<String>,
    remote_ssh_host: Option<String>,
) -> DesktopSessionScopeRequest {
    DesktopSessionScopeRequest {
        workspace_path,
        remote_connection_id,
        remote_ssh_host,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    pub session_id: Option<String>,
    pub session_name: String,
    pub agent_type: String,
    pub workspace_path: String,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub session_kind: Option<SessionKind>,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
    #[serde(default)]
    pub relationship: Option<SessionRelationship>,
    #[serde(default)]
    pub deep_review_run_manifest: Option<serde_json::Value>,
    #[serde(default)]
    pub review_target_evidence: Option<serde_json::Value>,
    pub config: Option<SessionConfigDTO>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionConfigDTO {
    pub max_context_tokens: Option<usize>,
    pub auto_compact: Option<bool>,
    pub enable_tools: Option<bool>,
    pub safe_mode: Option<bool>,
    pub max_turns: Option<usize>,
    pub enable_context_compression: Option<bool>,
    pub model_name: Option<String>,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionResponse {
    pub session_id: String,
    pub session_name: String,
    pub agent_type: String,
}

fn existing_session_create_response(
    request: &CreateSessionRequest,
    metadata: &SessionMetadata,
) -> Result<CreateSessionResponse, String> {
    let relationship_matches = match (
        metadata.relationship.as_ref(),
        request.relationship.as_ref(),
    ) {
        (None, None) | (None, Some(_)) => true,
        (Some(existing), Some(requested)) => {
            existing.kind == requested.kind
                && existing.parent_session_id == requested.parent_session_id
                && existing.parent_request_id == requested.parent_request_id
        }
        _ => false,
    };
    let deep_manifest_matches = match (
        metadata.deep_review_run_manifest.as_ref(),
        request.deep_review_run_manifest.as_ref(),
    ) {
        (Some(existing), Some(requested)) => existing == requested,
        (None, Some(_)) | (None, None) => true,
        (Some(_), None) => false,
    };
    let target_evidence_matches = match (
        metadata.review_target_evidence.as_ref(),
        request.review_target_evidence.as_ref(),
    ) {
        (Some(existing), Some(requested)) => existing == requested,
        (None, Some(_)) | (None, None) => true,
        (Some(_), None) => false,
    };
    if metadata.agent_type != request.agent_type
        || !relationship_matches
        || !deep_manifest_matches
        || !target_evidence_matches
    {
        return Err(format!(
            "Session ID {} already exists with different identity",
            metadata.session_id
        ));
    }

    Ok(CreateSessionResponse {
        session_id: metadata.session_id.clone(),
        session_name: metadata.session_name.clone(),
        agent_type: metadata.agent_type.clone(),
    })
}

fn is_idempotent_review_create(request: &CreateSessionRequest) -> bool {
    let Some(relationship) = request.relationship.as_ref() else {
        return false;
    };
    matches!(
        relationship.kind,
        Some(SessionRelationshipKind::Review | SessionRelationshipKind::DeepReview)
    ) && relationship
        .parent_request_id
        .as_deref()
        .is_some_and(|request_id| !request_id.trim().is_empty())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSessionModelRequest {
    pub session_id: String,
    pub model_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSessionTitleRequest {
    pub session_id: String,
    pub title: String,
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDialogTurnRequest {
    pub session_id: String,
    pub user_input: String,
    pub original_user_input: Option<String>,
    pub agent_type: String,
    pub workspace_path: Option<String>,
    pub remote_connection_id: Option<String>,
    pub remote_ssh_host: Option<String>,
    pub turn_id: Option<String>,
    #[serde(default)]
    pub image_contexts: Option<Vec<ImageContextData>>,
    #[serde(default)]
    pub user_message_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDialogTurnResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactSessionRequest {
    pub session_id: String,
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivateSessionGoalRequest {
    pub session_id: String,
    #[serde(default)]
    pub user_hint: Option<String>,
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivateSessionGoalResponse {
    pub success: bool,
    pub goal: ThreadGoal,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSessionThreadGoalRequest {
    pub session_id: String,
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSessionThreadGoalResponse {
    pub goal: Option<ThreadGoal>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetMemoryResponse {
    pub success: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPathsResponse {
    pub memories_root_dir: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSessionMemoryModeRequest {
    pub session_id: String,
    pub mode: String,
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSessionMemoryModeResponse {
    pub success: bool,
    pub mode: SessionMemoryMode,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearSessionThreadGoalRequest {
    pub session_id: String,
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSessionThreadGoalStatusRequest {
    pub session_id: String,
    pub status: String,
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSessionThreadGoalObjectiveRequest {
    pub session_id: String,
    pub objective: String,
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnsureCoordinatorSessionRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
    #[serde(default)]
    pub include_internal: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnsureAssistantBootstrapRequest {
    pub session_id: String,
    pub workspace_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunInitAgentsMdRequest {
    pub session_id: String,
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnsureAssistantBootstrapResponse {
    pub status: String,
    pub reason: String,
    pub session_id: String,
    pub turn_id: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSessionRequest {
    pub session_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResponse {
    pub session_id: String,
    pub session_name: String,
    /// Current/default mode selection for the next dialog turn.
    pub agent_type: String,
    /// Current/default model selection for the next dialog turn.
    pub model_name: Option<String>,
    /// Mode of the last surviving user dialog turn in session history.
    pub last_user_dialog_agent_type: Option<String>,
    /// Mode of the most recent user submission accepted by the scheduler.
    pub last_submitted_agent_type: Option<String>,
    pub state: String,
    pub turn_count: usize,
    pub created_at: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreSessionWithTurnsResponse {
    pub session: SessionResponse,
    pub turns: Vec<DialogTurnData>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreSessionViewResponse {
    pub session: SessionResponse,
    pub turns: Vec<DialogTurnData>,
    pub context_restore_state: String,
    pub is_partial: bool,
    pub loaded_turn_count: usize,
    pub total_turn_count: usize,
    pub timings: SessionViewRestoreTiming,
}

#[derive(Debug, Default)]
struct RestoreTurnPayloadStats {
    tool_result_count: usize,
    raw_result_string_chars: usize,
    result_for_assistant_chars: usize,
    largest_raw_result_chars: usize,
    largest_raw_result_path: String,
    top_raw_results: Vec<RestoreToolOutputStats>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RestoreToolOutputStats {
    tool_name: String,
    path: String,
    raw_result_string_chars: usize,
    result_for_assistant_chars: usize,
}

#[derive(Debug, Default)]
struct JsonStringStats {
    total_chars: usize,
    largest_chars: usize,
    largest_path: String,
}

fn collect_json_string_stats(value: &serde_json::Value, path: &str, stats: &mut JsonStringStats) {
    match value {
        serde_json::Value::String(text) => {
            let char_count = text.chars().count();
            stats.total_chars += char_count;
            if char_count > stats.largest_chars {
                stats.largest_chars = char_count;
                stats.largest_path = path.to_string();
            }
        }
        serde_json::Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_json_string_stats(item, &format!("{}[{}]", path, index), stats);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, item) in map {
                let next_path = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{}.{}", path, key)
                };
                collect_json_string_stats(item, &next_path, stats);
            }
        }
        _ => {}
    }
}

fn push_top_raw_result(stats: &mut RestoreTurnPayloadStats, result: RestoreToolOutputStats) {
    if result.raw_result_string_chars == 0 {
        return;
    }
    stats.top_raw_results.push(result);
    stats.top_raw_results.sort_by(|left, right| {
        right
            .raw_result_string_chars
            .cmp(&left.raw_result_string_chars)
            .then_with(|| left.tool_name.cmp(&right.tool_name))
    });
    stats.top_raw_results.truncate(3);
}

fn format_top_raw_results(results: &[RestoreToolOutputStats]) -> String {
    results
        .iter()
        .map(|item| {
            format!(
                "{}:{}:{}:{}",
                item.tool_name,
                item.raw_result_string_chars,
                item.result_for_assistant_chars,
                item.path
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn restore_turn_payload_stats(turns: &[DialogTurnData]) -> RestoreTurnPayloadStats {
    let mut stats = RestoreTurnPayloadStats::default();
    for turn in turns {
        for (round_index, round) in turn.model_rounds.iter().enumerate() {
            for (tool_index, tool) in round.tool_items.iter().enumerate() {
                let Some(result) = tool.tool_result.as_ref() else {
                    continue;
                };
                stats.tool_result_count += 1;
                let assistant_chars = result
                    .result_for_assistant
                    .as_deref()
                    .map(|text| text.chars().count())
                    .unwrap_or(0);
                stats.result_for_assistant_chars += assistant_chars;
                let mut json_stats = JsonStringStats::default();
                collect_json_string_stats(
                    &result.result,
                    &format!(
                        "turn[{}].round[{}].tool[{}].{}",
                        turn.turn_index, round_index, tool_index, tool.tool_name
                    ),
                    &mut json_stats,
                );
                stats.raw_result_string_chars += json_stats.total_chars;
                if json_stats.largest_chars > stats.largest_raw_result_chars {
                    stats.largest_raw_result_chars = json_stats.largest_chars;
                    stats.largest_raw_result_path = json_stats.largest_path.clone();
                }
                push_top_raw_result(
                    &mut stats,
                    RestoreToolOutputStats {
                        tool_name: tool.tool_name.clone(),
                        path: json_stats.largest_path,
                        raw_result_string_chars: json_stats.total_chars,
                        result_for_assistant_chars: assistant_chars,
                    },
                );
            }
        }
    }
    stats
}

fn truncate_string_for_session_view(text: &str, remaining_budget: &mut usize) -> Option<String> {
    let char_count = text.chars().count();
    let available = SESSION_VIEW_TOOL_RESULT_STRING_CHAR_LIMIT.min(*remaining_budget);

    if char_count <= available {
        *remaining_budget = remaining_budget.saturating_sub(char_count);
        return None;
    }

    let omitted_chars = SESSION_VIEW_OMITTED_MARKER.chars().count();
    if available <= omitted_chars {
        *remaining_budget = remaining_budget.saturating_sub(omitted_chars.min(*remaining_budget));
        return Some(SESSION_VIEW_OMITTED_MARKER.to_string());
    }

    let suffix_chars = SESSION_VIEW_TRUNCATED_MARKER.chars().count();
    if available <= suffix_chars {
        *remaining_budget = remaining_budget.saturating_sub(omitted_chars.min(*remaining_budget));
        return Some(SESSION_VIEW_OMITTED_MARKER.to_string());
    }

    let keep_chars = available - suffix_chars;
    let mut preview = text.chars().take(keep_chars).collect::<String>();
    preview.push_str(SESSION_VIEW_TRUNCATED_MARKER);
    let preview_chars = preview.chars().count();
    *remaining_budget = remaining_budget.saturating_sub(preview_chars);
    Some(preview)
}

fn compact_json_for_session_view(value: &mut serde_json::Value, remaining_budget: &mut usize) {
    match value {
        serde_json::Value::String(text) => {
            if let Some(preview) = truncate_string_for_session_view(text, remaining_budget) {
                *text = preview;
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                compact_json_for_session_view(item, remaining_budget);
            }
        }
        serde_json::Value::Object(map) => {
            for item in map.values_mut() {
                compact_json_for_session_view(item, remaining_budget);
            }
        }
        _ => {}
    }
}

fn compact_tool_results_for_session_view(turns: &mut [DialogTurnData]) {
    let mut remaining_budget = SESSION_VIEW_TOOL_RESULT_TOTAL_CHAR_BUDGET;
    for turn in turns {
        for round in &mut turn.model_rounds {
            for tool in &mut round.tool_items {
                if let Some(result) = tool.tool_result.as_mut() {
                    result.result_for_assistant = None;
                    compact_json_for_session_view(&mut result.result, &mut remaining_budget);
                }
            }
        }
    }
}

#[cfg(test)]
fn omit_assistant_only_tool_results_for_session_view(turns: &mut [DialogTurnData]) {
    compact_tool_results_for_session_view(turns);
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelDialogTurnRequest {
    pub session_id: String,
    pub dialog_turn_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteerDialogTurnRequest {
    pub session_id: String,
    pub dialog_turn_id: String,
    /// Rendered content delivered to the model. When omitted by the caller this
    /// equals the displayed user text.
    pub content: String,
    /// Original user text for UI rendering (defaults to `content`).
    #[serde(default)]
    pub display_content: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteerDialogTurnResponse {
    pub success: bool,
    pub steering_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlDeepReviewQueueRequest {
    pub session_id: String,
    pub dialog_turn_id: String,
    pub tool_id: String,
    pub action: ControlDeepReviewQueueActionDTO,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlDeepReviewQueueActionDTO {
    Pause,
    Continue,
    Cancel,
    SkipOptional,
}

impl From<ControlDeepReviewQueueActionDTO> for DeepReviewQueueControlAction {
    fn from(value: ControlDeepReviewQueueActionDTO) -> Self {
        match value {
            ControlDeepReviewQueueActionDTO::Pause => DeepReviewQueueControlAction::Pause,
            ControlDeepReviewQueueActionDTO::Continue => DeepReviewQueueControlAction::Continue,
            ControlDeepReviewQueueActionDTO::Cancel => DeepReviewQueueControlAction::Cancel,
            ControlDeepReviewQueueActionDTO::SkipOptional => {
                DeepReviewQueueControlAction::SkipOptional
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelSessionRequest {
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelToolRequest {
    pub tool_use_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSessionRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreSessionRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
    #[serde(default)]
    pub include_internal: bool,
    #[serde(default)]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub tail_turn_count: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSessionsRequest {
    pub workspace_path: String,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionResponseRequest {
    pub request_id: String,
    pub reply: PermissionReplyKind,
    pub feedback: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionProjectRequest {
    pub workspace_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovePermissionGrantRequest {
    pub workspace_id: String,
    pub action: String,
    pub resource: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionAuditRequest {
    pub workspace_id: String,
    #[serde(default)]
    pub page: usize,
    #[serde(default = "default_permission_audit_page_size")]
    pub page_size: usize,
}

const fn default_permission_audit_page_size() -> usize {
    50
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionAuditPage {
    pub project_id: String,
    pub records: Vec<PermissionAuditRecord>,
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPermissionRulesResponse {
    pub rules: Vec<PermissionRule>,
    pub revision: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProjectPermissionRulesRequest {
    pub workspace_id: String,
    pub rules: Vec<PermissionRule>,
    pub revision: String,
}

#[derive(Debug, Clone)]
struct ProjectPermissionConfigTarget {
    path: String,
    remote_connection_id: Option<String>,
}

async fn permission_project_id_for_workspace(
    state: &AppState,
    workspace_id: &str,
) -> Result<String, String> {
    let workspace = state
        .workspace_service
        .get_workspace(workspace_id)
        .await
        .ok_or_else(|| format!("Workspace not found: {workspace_id}"))?;
    let remote = workspace.workspace_kind == WorkspaceKind::Remote;
    let connection_id = workspace
        .metadata
        .get("connectionId")
        .and_then(|value| value.as_str());
    let ssh_host = workspace
        .metadata
        .get("sshHost")
        .and_then(|value| value.as_str());
    let identity = resolve_workspace_session_identity(
        &workspace.root_path.to_string_lossy(),
        connection_id,
        ssh_host,
    )
    .await
    .ok_or_else(|| format!("Workspace identity is unavailable: {workspace_id}"))?;
    bitfun_core::agentic::tools::pipeline::permission_project_id_for_workspace_identity(
        &identity, remote,
    )
    .map_err(|error| error.to_string())
}

async fn project_permission_config_target_for_workspace(
    state: &AppState,
    workspace_id: &str,
) -> Result<ProjectPermissionConfigTarget, String> {
    let workspace = state
        .workspace_service
        .get_workspace(workspace_id)
        .await
        .ok_or_else(|| format!("Workspace not found: {workspace_id}"))?;

    if workspace.workspace_kind == WorkspaceKind::Remote {
        let remote_connection_id = workspace
            .metadata
            .get("connectionId")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                format!(
                    "Remote workspace is missing a connection ID: {}",
                    workspace.id
                )
            })?
            .to_string();
        return Ok(ProjectPermissionConfigTarget {
            path: project_permission_file_path_for_remote(&workspace.root_path.to_string_lossy()),
            remote_connection_id: Some(remote_connection_id),
        });
    }

    Ok(ProjectPermissionConfigTarget {
        path: project_permission_file_path(&workspace.root_path)
            .to_string_lossy()
            .to_string(),
        remote_connection_id: None,
    })
}

async fn read_project_permission_config_content(
    state: &AppState,
    target: &ProjectPermissionConfigTarget,
) -> Result<Option<String>, String> {
    let Some(connection_id) = target.remote_connection_id.as_deref() else {
        return match tokio::fs::read_to_string(&target.path).await {
            Ok(content) => Ok(Some(content)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!(
                "Failed to read project permission rules '{}': {error}",
                target.path
            )),
        };
    };

    let remote_fs = state
        .get_remote_file_service_async()
        .await
        .map_err(|error| format!("Remote file service is not available: {error}"))?;
    let exists = remote_fs
        .exists(connection_id, &target.path)
        .await
        .map_err(|error| format!("Failed to check remote project permission rules: {error}"))?;
    if !exists {
        return Ok(None);
    }
    let bytes = remote_fs
        .read_file(connection_id, &target.path)
        .await
        .map_err(|error| format!("Failed to read remote project permission rules: {error}"))?;
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|error| format!("Project permission rules are not valid UTF-8: {error}"))
}

async fn write_project_permission_config_content(
    state: &AppState,
    target: &ProjectPermissionConfigTarget,
    content: &str,
) -> Result<(), String> {
    let Some(connection_id) = target.remote_connection_id.as_deref() else {
        let parent = Path::new(&target.path).parent().ok_or_else(|| {
            format!(
                "Project permission rules path has no parent directory: {}",
                target.path
            )
        })?;
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            format!(
                "Failed to create project permission rules directory '{}': {error}",
                parent.display()
            )
        })?;
        return tokio::fs::write(&target.path, content)
            .await
            .map_err(|error| {
                format!(
                    "Failed to write project permission rules '{}': {error}",
                    target.path
                )
            });
    };

    let remote_fs = state
        .get_remote_file_service_async()
        .await
        .map_err(|error| format!("Remote file service is not available: {error}"))?;
    let parent = target
        .path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or_else(|| {
            format!(
                "Remote project permission rules path has no parent directory: {}",
                target.path
            )
        })?;
    remote_fs
        .create_dir_all(connection_id, parent)
        .await
        .map_err(|error| {
            format!("Failed to create remote project permission rules directory: {error}")
        })?;
    remote_fs
        .write_file(connection_id, &target.path, content.as_bytes())
        .await
        .map_err(|error| format!("Failed to write remote project permission rules: {error}"))
}

fn project_permission_rules_revision(content: Option<&str>) -> String {
    let mut hasher = Sha1::new();
    match content {
        Some(content) => {
            hasher.update(b"present\0");
            hasher.update(content.as_bytes());
        }
        None => hasher.update(b"missing\0"),
    }
    format!("{:x}", hasher.finalize())
}

fn validate_project_permission_rules(rules: &[PermissionRule]) -> Result<(), String> {
    if rules
        .iter()
        .any(|rule| rule.action.trim().is_empty() || rule.resource.trim().is_empty())
    {
        return Err("Project permission rule action and resource must be non-empty".to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn get_project_permission_rules(
    state: State<'_, AppState>,
    request: PermissionProjectRequest,
) -> Result<ProjectPermissionRulesResponse, String> {
    let target =
        project_permission_config_target_for_workspace(&state, &request.workspace_id).await?;
    let content = read_project_permission_config_content(&state, &target).await?;
    let rules = content
        .as_deref()
        .map(deserialize_project_permission_config)
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or_default()
        .rules;
    Ok(ProjectPermissionRulesResponse {
        rules,
        revision: project_permission_rules_revision(content.as_deref()),
    })
}

#[tauri::command]
pub async fn save_project_permission_rules(
    state: State<'_, AppState>,
    request: SaveProjectPermissionRulesRequest,
) -> Result<ProjectPermissionRulesResponse, String> {
    validate_project_permission_rules(&request.rules)?;

    let target =
        project_permission_config_target_for_workspace(&state, &request.workspace_id).await?;
    let current_content = read_project_permission_config_content(&state, &target).await?;
    let current_revision = project_permission_rules_revision(current_content.as_deref());
    if request.revision != current_revision {
        return Err(
            "Project permission rules changed outside BitFun. Reload before saving.".to_string(),
        );
    }

    let content = format!(
        "{}\n",
        serde_json::to_string_pretty(&ProjectPermissionConfig {
            rules: request.rules.clone(),
        })
        .map_err(|error| format!("Failed to serialize project permission rules: {error}"))?
    );
    write_project_permission_config_content(&state, &target, &content).await?;
    Ok(ProjectPermissionRulesResponse {
        rules: request.rules,
        revision: project_permission_rules_revision(Some(&content)),
    })
}

#[tauri::command]
pub async fn list_project_permission_grants(
    state: State<'_, AppState>,
    runtime: State<'_, DesktopRuntimeContext>,
    request: PermissionProjectRequest,
) -> Result<Vec<PermissionGrant>, String> {
    let project_id = permission_project_id_for_workspace(&state, &request.workspace_id).await?;
    runtime
        .agent_runtime()
        .list_project_permission_grants(&project_id)
        .await
        .map_err(|error| error.into_message())
}

#[tauri::command]
pub async fn remove_project_permission_grant(
    state: State<'_, AppState>,
    runtime: State<'_, DesktopRuntimeContext>,
    request: RemovePermissionGrantRequest,
) -> Result<bool, String> {
    let project_id = permission_project_id_for_workspace(&state, &request.workspace_id).await?;
    runtime
        .agent_runtime()
        .remove_project_permission_grant(PermissionGrantKey {
            project_id,
            action: request.action,
            resource: request.resource,
        })
        .await
        .map_err(|error| error.into_message())
}

#[tauri::command]
pub async fn clear_project_permission_grants(
    state: State<'_, AppState>,
    runtime: State<'_, DesktopRuntimeContext>,
    request: PermissionProjectRequest,
) -> Result<usize, String> {
    let project_id = permission_project_id_for_workspace(&state, &request.workspace_id).await?;
    runtime
        .agent_runtime()
        .clear_project_permission_grants(&project_id)
        .await
        .map_err(|error| error.into_message())
}

#[tauri::command]
pub async fn list_project_permission_audit(
    state: State<'_, AppState>,
    runtime: State<'_, DesktopRuntimeContext>,
    request: PermissionAuditRequest,
) -> Result<PermissionAuditPage, String> {
    let project_id = permission_project_id_for_workspace(&state, &request.workspace_id).await?;
    let mut records = runtime
        .agent_runtime()
        .list_project_permission_audit(&project_id)
        .await
        .map_err(|error| error.into_message())?;
    records.sort_by(|left, right| {
        right
            .timestamp_ms
            .cmp(&left.timestamp_ms)
            .then_with(|| right.audit_id.cmp(&left.audit_id))
    });
    let total = records.len();
    let page_size = request.page_size.clamp(1, 100);
    let offset = request.page.saturating_mul(page_size).min(total);
    let records = records.into_iter().skip(offset).take(page_size).collect();
    Ok(PermissionAuditPage {
        project_id,
        records,
        page: request.page,
        page_size,
        total,
    })
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionReplyKind {
    Once,
    Always,
    Reject,
}

fn permission_reply(request: PermissionResponseRequest) -> PermissionReply {
    match request.reply {
        PermissionReplyKind::Once => PermissionReply::Once,
        PermissionReplyKind::Always => PermissionReply::Always,
        PermissionReplyKind::Reject => PermissionReply::Reject {
            feedback: request.feedback,
        },
    }
}

#[tauri::command]
pub fn list_pending_permission_requests(
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<Vec<PermissionRequest>, String> {
    runtime
        .agent_runtime()
        .pending_permission_requests()
        .map_err(|error| error.into_message())
}

#[tauri::command]
pub fn subscribe_permission_requests(
    app: AppHandle,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<(), String> {
    runtime
        .start_permission_event_forwarding(app)
        .map_err(|error| error.into_message())
}

#[tauri::command]
pub async fn respond_permission(
    runtime: State<'_, DesktopRuntimeContext>,
    request: PermissionResponseRequest,
) -> Result<(), String> {
    let request_id = request.request_id.clone();
    let reply = permission_reply(request);
    runtime
        .agent_runtime()
        .respond_permission(&request_id, reply)
        .await
        .map_err(|error| error.into_message())
}

#[tauri::command]
pub async fn respond_permission_batch(
    runtime: State<'_, DesktopRuntimeContext>,
    request: PermissionResponseRequest,
) -> Result<Vec<String>, String> {
    let request_id = request.request_id.clone();
    let reply = permission_reply(request);
    runtime
        .agent_runtime()
        .respond_permission_batch(&request_id, reply)
        .await
        .map_err(|error| error.into_message())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateSessionTitleRequest {
    pub session_id: String,
    pub user_message: String,
    pub max_length: Option<usize>,
}

#[tauri::command]
pub async fn create_session(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    app_state: State<'_, AppState>,
    request: CreateSessionRequest,
) -> Result<CreateSessionResponse, String> {
    fn norm_conn(s: Option<String>) -> Option<String> {
        s.map(|x| x.trim().to_string()).filter(|x| !x.is_empty())
    }
    let wp = request.workspace_path.clone();
    let remote_conn = norm_conn(request.remote_connection_id.clone()).or_else(|| {
        request
            .config
            .as_ref()
            .and_then(|c| norm_conn(c.remote_connection_id.clone()))
    });
    let remote_ssh_host = norm_conn(request.remote_ssh_host.clone()).or_else(|| {
        request
            .config
            .as_ref()
            .and_then(|c| norm_conn(c.remote_ssh_host.clone()))
    });

    if is_idempotent_review_create(&request) {
        let session_id = request
            .session_id
            .as_deref()
            .ok_or_else(|| "Idempotent Review session requires a session ID".to_string())?;
        let effective_path = desktop_effective_session_storage_path(
            &app_state,
            &request.workspace_path,
            remote_conn.as_deref(),
            remote_ssh_host.as_deref(),
        )
        .await;
        let existing = coordinator
            .get_session_manager()
            .load_session_metadata(&effective_path, session_id)
            .await
            .map_err(|error| format!("Failed to check existing session: {error}"))?;
        if let Some(mut metadata) = existing {
            let response = existing_session_create_response(&request, &metadata)?;
            let mut repaired = false;
            if metadata.relationship.is_none() && request.relationship.is_some() {
                metadata.relationship = request.relationship.clone();
                repaired = true;
            }
            if metadata.deep_review_run_manifest.is_none()
                && request.deep_review_run_manifest.is_some()
            {
                metadata.deep_review_run_manifest = request.deep_review_run_manifest.clone();
                repaired = true;
            }
            if metadata.review_target_evidence.is_none() && request.review_target_evidence.is_some()
            {
                metadata.review_target_evidence = request.review_target_evidence.clone();
                repaired = true;
            }
            if repaired {
                let relationship = request.relationship.clone();
                let deep_review_run_manifest = request.deep_review_run_manifest.clone();
                let review_target_evidence = request.review_target_evidence.clone();
                coordinator
                    .get_session_manager()
                    .update_session_metadata(&effective_path, session_id, |current| {
                        if current.relationship.is_none() {
                            current.relationship = relationship;
                        }
                        if current.deep_review_run_manifest.is_none() {
                            current.deep_review_run_manifest = deep_review_run_manifest;
                        }
                        if current.review_target_evidence.is_none() {
                            current.review_target_evidence = review_target_evidence;
                        }
                    })
                    .await
                    .map_err(|error| {
                        format!("Failed to repair Review session metadata: {error}")
                    })?;
            }
            return Ok(response);
        }
    }

    let config = request
        .config
        .map(|c| SessionConfig {
            max_context_tokens: c.max_context_tokens.unwrap_or(128128),
            auto_compact: c.auto_compact.unwrap_or(true),
            enable_tools: c.enable_tools.unwrap_or(true),
            safe_mode: c.safe_mode.unwrap_or(true),
            max_turns: c.max_turns.unwrap_or(200),
            enable_context_compression: c.enable_context_compression.unwrap_or(true),
            workspace_path: Some(request.workspace_path.clone()),
            workspace_id: request.workspace_id.clone(),
            remote_connection_id: remote_conn.clone(),
            remote_ssh_host: remote_ssh_host.clone(),
            model_id: c.model_name,
            ..Default::default()
        })
        .unwrap_or(SessionConfig {
            workspace_path: Some(request.workspace_path.clone()),
            workspace_id: request.workspace_id.clone(),
            remote_connection_id: remote_conn.clone(),
            remote_ssh_host: remote_ssh_host.clone(),
            ..Default::default()
        });

    let session_kind = request.session_kind.unwrap_or_default();
    let session = if matches!(session_kind, SessionKind::Subagent) {
        coordinator
            .create_hidden_subagent_session_with_workspace(
                request.session_id,
                request.session_name.clone(),
                request.agent_type.clone(),
                config,
                request.workspace_path,
                None,
            )
            .await
    } else {
        coordinator
            .create_session_with_workspace(
                request.session_id,
                request.session_name.clone(),
                request.agent_type.clone(),
                config,
                request.workspace_path,
            )
            .await
    }
    .map_err(|e| format!("Failed to create session: {}", e))?;

    if let Some(relationship) = request.relationship {
        coordinator
            .get_session_manager()
            .merge_session_relationship(&session.session_id, relationship)
            .await
            .map_err(|e| format!("Failed to persist session relationship: {}", e))?;
    }

    if let Some(run_manifest) = request.deep_review_run_manifest {
        coordinator
            .get_session_manager()
            .set_session_deep_review_run_manifest(&session.session_id, Some(run_manifest))
            .await
            .map_err(|e| format!("Failed to persist Deep Review run manifest: {}", e))?;
    }

    let session_id = session.session_id.clone();
    // Notify auto-sync: new session created
    crate::api::remote_connect_api::notify_session_changed(&session_id, &wp);

    if let Some(target_evidence) = request.review_target_evidence {
        coordinator
            .get_session_manager()
            .set_session_review_target_evidence(&session.session_id, Some(target_evidence))
            .await
            .map_err(|e| format!("Failed to persist Review target evidence: {}", e))?;
    }

    Ok(CreateSessionResponse {
        session_id: session.session_id,
        session_name: session.session_name,
        agent_type: session.agent_type,
    })
}

#[tauri::command]
pub async fn update_session_model(
    runtime: State<'_, DesktopRuntimeContext>,
    request: UpdateSessionModelRequest,
) -> Result<(), String> {
    runtime
        .agent_runtime()
        .update_session_model(AgentSessionModelUpdateRequest {
            session_id: request.session_id,
            model_id: request.model_name,
        })
        .await
        .map_err(|error| format!("Failed to update session model: {}", error.into_message()))
}

#[tauri::command]
pub async fn update_session_title(
    runtime: State<'_, DesktopRuntimeContext>,
    request: UpdateSessionTitleRequest,
) -> Result<String, String> {
    let session_id = request.session_id.trim();
    if session_id.is_empty() {
        return Err("session_id is required".to_string());
    }

    let scope = request
        .workspace_path
        .filter(|workspace_path| !workspace_path.trim().is_empty())
        .map(|workspace_path| {
            desktop_session_scope(
                workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            )
        });
    runtime
        .session_application()
        .rename_session(scope, session_id.to_string(), request.title)
        .await
        .map_err(|error| match error {
            DesktopSessionApplicationError::Validation(message) => message,
            DesktopSessionApplicationError::RestoreBeforeRename(message) => {
                format!("Failed to restore session before renaming: {message}")
            }
            error => format!("Failed to update session title: {error}"),
        })
}

/// Load the session into the coordinator process when it exists on disk but is not in memory.
/// Uses the same remote→local session path mapping as `restore_session`.
#[tauri::command]
pub async fn ensure_coordinator_session(
    runtime: State<'_, DesktopRuntimeContext>,
    request: EnsureCoordinatorSessionRequest,
) -> Result<(), String> {
    let session_id = request.session_id.trim();
    if session_id.is_empty() {
        return Err("session_id is required".to_string());
    }
    runtime
        .session_application()
        .ensure_session_loaded(
            desktop_session_scope(
                request.workspace_path.clone(),
                request.remote_connection_id.clone(),
                request.remote_ssh_host.clone(),
            ),
            session_id,
            request.include_internal,
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn start_dialog_turn(
    _app: AppHandle,
    runtime: State<'_, DesktopRuntimeContext>,
    request: StartDialogTurnRequest,
) -> Result<StartDialogTurnResponse, String> {
    let runtime_request = desktop_dialog_turn_request(request)?;

    runtime
        .agent_runtime()
        .submit_dialog_turn(runtime_request)
        .await
        .map_err(|error| format!("Failed to start dialog turn: {}", error.into_message()))?;

    Ok(StartDialogTurnResponse {
        success: true,
        message: "Dialog turn started".to_string(),
    })
}

fn desktop_dialog_turn_request(
    request: StartDialogTurnRequest,
) -> Result<AgentDialogTurnRequest, String> {
    let StartDialogTurnRequest {
        session_id,
        user_input,
        original_user_input,
        agent_type,
        workspace_path,
        remote_connection_id,
        remote_ssh_host,
        turn_id,
        image_contexts,
        user_message_metadata,
    } = request;

    let policy = DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopUi);
    let attachments = match image_contexts.filter(|images| !images.is_empty()) {
        Some(images) => resolve_missing_image_payloads(images)?
            .into_iter()
            .map(desktop_image_attachment)
            .collect(),
        None => Vec::new(),
    };
    let metadata = desktop_user_message_metadata(user_message_metadata);

    Ok(AgentDialogTurnRequest {
        session_id,
        message: user_input,
        original_message: original_user_input,
        turn_id,
        agent_type,
        workspace_path,
        remote_connection_id,
        remote_ssh_host,
        policy,
        reply_route: None,
        prepended_reminders: Vec::new(),
        attachments,
        metadata,
    })
}

fn desktop_user_message_metadata(
    metadata: Option<serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    match metadata {
        Some(serde_json::Value::Object(metadata)) => metadata,
        Some(metadata) => serde_json::Map::from_iter([("raw_metadata".to_string(), metadata)]),
        None => serde_json::Map::new(),
    }
}

fn desktop_image_attachment(image: ImageContextData) -> AgentInputAttachment {
    let mut metadata = serde_json::Map::new();
    if let Some(image_path) = image.image_path {
        metadata.insert(
            "imagePath".to_string(),
            serde_json::Value::String(image_path),
        );
    }
    if let Some(data_url) = image.data_url {
        metadata.insert("dataUrl".to_string(), serde_json::Value::String(data_url));
    }
    metadata.insert(
        "mimeType".to_string(),
        serde_json::Value::String(image.mime_type),
    );
    if let Some(image_metadata) = image.metadata {
        metadata.insert("metadata".to_string(), image_metadata);
    }

    AgentInputAttachment {
        kind: "remote_image".to_string(),
        id: image.id,
        metadata,
    }
}

#[tauri::command]
pub async fn compact_session(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    app_state: State<'_, AppState>,
    request: CompactSessionRequest,
) -> Result<StartDialogTurnResponse, String> {
    let session_id = request.session_id.trim();
    if session_id.is_empty() {
        return Err("session_id is required".to_string());
    }

    if coordinator
        .get_session_manager()
        .get_session(session_id)
        .is_none()
    {
        let workspace_path = request
            .workspace_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                "workspace_path is required when the session is not loaded".to_string()
            })?;
        let effective = desktop_effective_session_storage_path(
            &app_state,
            workspace_path,
            request.remote_connection_id.as_deref(),
            request.remote_ssh_host.as_deref(),
        )
        .await;
        coordinator
            .restore_session_from_storage_path(&effective, session_id)
            .await
            .map_err(|e| format!("Failed to restore session before compacting: {}", e))?;
    }

    coordinator
        .compact_session_manually(session_id.to_string())
        .await
        .map_err(|e| format!("Failed to compact session: {}", e))?;

    Ok(StartDialogTurnResponse {
        success: true,
        message: "Session compaction started".to_string(),
    })
}

#[tauri::command]
pub async fn activate_session_goal(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    app_state: State<'_, AppState>,
    request: ActivateSessionGoalRequest,
) -> Result<ActivateSessionGoalResponse, String> {
    let session_id = request.session_id.trim();
    if session_id.is_empty() {
        return Err("session_id is required".to_string());
    }

    if coordinator
        .get_session_manager()
        .get_session(session_id)
        .is_none()
    {
        let workspace_path = request
            .workspace_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                "workspace_path is required when the session is not loaded".to_string()
            })?;
        let effective = desktop_effective_session_storage_path(
            &app_state,
            workspace_path,
            request.remote_connection_id.as_deref(),
            request.remote_ssh_host.as_deref(),
        )
        .await;
        coordinator
            .restore_session_from_storage_path(&effective, session_id)
            .await
            .map_err(|e| format!("Failed to restore session before activating goal mode: {e}"))?;
    }

    let user_hint = request
        .user_hint
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let activation = coordinator
        .activate_session_goal(session_id.to_string(), user_hint)
        .await
        .map_err(|error| error.to_string())?;

    Ok(ActivateSessionGoalResponse {
        success: true,
        goal: activation,
    })
}

async fn ensure_session_for_thread_goal(
    coordinator: &Arc<ConversationCoordinator>,
    app_state: &AppState,
    session_id: &str,
    workspace_path: Option<&str>,
    remote_connection_id: Option<&str>,
    remote_ssh_host: Option<&str>,
) -> Result<PathBuf, String> {
    if coordinator
        .get_session_manager()
        .get_session(session_id)
        .is_none()
    {
        let workspace_path = workspace_path
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                "workspace_path is required when the session is not loaded".to_string()
            })?;
        let effective = desktop_effective_session_storage_path(
            app_state,
            workspace_path,
            remote_connection_id,
            remote_ssh_host,
        )
        .await;
        coordinator
            .restore_session_from_storage_path(&effective, session_id)
            .await
            .map_err(|e| format!("Failed to restore session before thread goal access: {e}"))?;
    }

    coordinator
        .get_session_manager()
        .get_session(session_id)
        .and_then(|session| session.config.workspace_path.clone())
        .map(PathBuf::from)
        .ok_or_else(|| format!("Session workspace_path is missing: {session_id}"))
}

async fn resolve_thread_goal_storage_path(
    coordinator: &Arc<ConversationCoordinator>,
    app_state: &AppState,
    session_id: &str,
    workspace_path: Option<&str>,
    remote_connection_id: Option<&str>,
    remote_ssh_host: Option<&str>,
) -> Result<PathBuf, String> {
    if let Some(storage_path) = coordinator
        .get_session_manager()
        .resolve_session_workspace_binding(session_id)
        .await
        .map(|binding| binding.session_storage_dir())
    {
        return Ok(storage_path);
    }

    if let Some(workspace_path) = coordinator
        .get_session_manager()
        .get_session(session_id)
        .and_then(|session| session.config.workspace_path.clone())
    {
        return Ok(PathBuf::from(workspace_path));
    }

    let workspace_path = workspace_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "workspace_path is required when the session is not loaded".to_string())?;

    Ok(desktop_effective_session_storage_path(
        app_state,
        workspace_path,
        remote_connection_id,
        remote_ssh_host,
    )
    .await)
}

#[tauri::command]
pub async fn get_session_thread_goal(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    app_state: State<'_, AppState>,
    startup_trace: State<'_, DesktopStartupTrace>,
    request: GetSessionThreadGoalRequest,
) -> Result<GetSessionThreadGoalResponse, String> {
    let trace_started = Instant::now();
    let result = async {
        let session_id = request.session_id.trim();
        if session_id.is_empty() {
            return Err("session_id is required".to_string());
        }
        let storage_path = resolve_thread_goal_storage_path(
            coordinator.inner(),
            app_state.inner(),
            session_id,
            request.workspace_path.as_deref(),
            request.remote_connection_id.as_deref(),
            request.remote_ssh_host.as_deref(),
        )
        .await?;
        let goal = coordinator
            .get_thread_goal(session_id, storage_path.as_path())
            .await
            .map_err(|error| error.to_string())?;
        Ok(GetSessionThreadGoalResponse { goal })
    }
    .await;
    startup_trace.record_tauri_command_elapsed("get_session_thread_goal", None, trace_started);
    result
}

#[tauri::command]
pub async fn reset_memory(state: State<'_, AppState>) -> Result<ResetMemoryResponse, String> {
    let path_manager = state.workspace_service.path_manager().clone();
    let db = MemoryDatabase::new(path_manager.clone());

    db.reset_memory_state()
        .await
        .map_err(|error| format!("Failed to reset memory database: {}", error))?;
    reset_memory_workspace(&path_manager.memories_root_dir())
        .await
        .map_err(|error| format!("Failed to reset memory workspace: {}", error))?;

    Ok(ResetMemoryResponse { success: true })
}

#[tauri::command]
pub async fn get_memory_paths(state: State<'_, AppState>) -> Result<MemoryPathsResponse, String> {
    let path_manager = state.workspace_service.path_manager().clone();
    let memories_root_dir = path_manager.memories_root_dir();
    tokio::fs::create_dir_all(&memories_root_dir)
        .await
        .map_err(|error| format!("Failed to create memory directory: {}", error))?;
    Ok(MemoryPathsResponse {
        memories_root_dir: memories_root_dir.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn set_session_memory_mode(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    app_state: State<'_, AppState>,
    request: SetSessionMemoryModeRequest,
) -> Result<SetSessionMemoryModeResponse, String> {
    let session_id = request.session_id.trim();
    if session_id.is_empty() {
        return Err("session_id is required".to_string());
    }
    let mode = match request.mode.trim().to_ascii_lowercase().as_str() {
        "enabled" => SessionMemoryMode::Enabled,
        "disabled" => SessionMemoryMode::Disabled,
        "polluted" => {
            return Err("polluted memory mode is internal and cannot be set directly".to_string())
        }
        other => return Err(format!("unsupported memory mode: {other}")),
    };
    let storage_path = resolve_thread_goal_storage_path(
        coordinator.inner(),
        app_state.inner(),
        session_id,
        request.workspace_path.as_deref(),
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await?;

    coordinator
        .get_session_manager()
        .set_session_memory_mode(&storage_path, session_id, mode)
        .await
        .map_err(|error| error.to_string())?;

    Ok(SetSessionMemoryModeResponse {
        success: true,
        mode,
    })
}

#[tauri::command]
pub async fn clear_session_thread_goal(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    app_state: State<'_, AppState>,
    request: ClearSessionThreadGoalRequest,
) -> Result<(), String> {
    let session_id = request.session_id.trim();
    if session_id.is_empty() {
        return Err("session_id is required".to_string());
    }
    let workspace_path = ensure_session_for_thread_goal(
        coordinator.inner(),
        app_state.inner(),
        session_id,
        request.workspace_path.as_deref(),
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await?;
    coordinator
        .clear_thread_goal(session_id, workspace_path.as_path())
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_session_thread_goal_status(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    app_state: State<'_, AppState>,
    request: SetSessionThreadGoalStatusRequest,
) -> Result<ThreadGoal, String> {
    let session_id = request.session_id.trim();
    if session_id.is_empty() {
        return Err("session_id is required".to_string());
    }
    let status = match request.status.trim().to_ascii_lowercase().as_str() {
        "active" => ThreadGoalStatus::Active,
        "paused" => ThreadGoalStatus::Paused,
        "blocked" => ThreadGoalStatus::Blocked,
        "usageLimited" | "usage_limited" => ThreadGoalStatus::UsageLimited,
        "budgetLimited" | "budget_limited" => ThreadGoalStatus::BudgetLimited,
        "complete" => ThreadGoalStatus::Complete,
        other => return Err(format!("unsupported thread goal status: {other}")),
    };
    let workspace_path = ensure_session_for_thread_goal(
        coordinator.inner(),
        app_state.inner(),
        session_id,
        request.workspace_path.as_deref(),
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await?;
    coordinator
        .set_thread_goal_status(session_id, workspace_path.as_path(), status)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn update_session_thread_goal_objective(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    app_state: State<'_, AppState>,
    request: UpdateSessionThreadGoalObjectiveRequest,
) -> Result<ThreadGoal, String> {
    let session_id = request.session_id.trim();
    if session_id.is_empty() {
        return Err("session_id is required".to_string());
    }
    let objective = request.objective.trim();
    if objective.is_empty() {
        return Err("objective is required".to_string());
    }
    let workspace_path = ensure_session_for_thread_goal(
        coordinator.inner(),
        app_state.inner(),
        session_id,
        request.workspace_path.as_deref(),
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await?;
    coordinator
        .update_thread_goal_objective(session_id, workspace_path.as_path(), objective.to_string())
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn ensure_assistant_bootstrap(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: EnsureAssistantBootstrapRequest,
) -> Result<EnsureAssistantBootstrapResponse, String> {
    let outcome = coordinator
        .ensure_assistant_bootstrap(request.session_id, request.workspace_path)
        .await
        .map_err(|e| format!("Failed to ensure assistant bootstrap: {}", e))?;

    Ok(assistant_bootstrap_outcome_to_response(outcome))
}

#[tauri::command]
pub async fn run_init_agents_md(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    scheduler: State<'_, Arc<DialogScheduler>>,
    app_state: State<'_, AppState>,
    request: RunInitAgentsMdRequest,
) -> Result<StartDialogTurnResponse, String> {
    let session_id = request.session_id.trim();
    if session_id.is_empty() {
        return Err("session_id is required".to_string());
    }

    if coordinator
        .get_session_manager()
        .get_session(session_id)
        .is_none()
    {
        let workspace_path = request
            .workspace_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                "workspace_path is required when the session is not loaded".to_string()
            })?;
        let effective = desktop_effective_session_storage_path(
            &app_state,
            workspace_path,
            request.remote_connection_id.as_deref(),
            request.remote_ssh_host.as_deref(),
        )
        .await;
        coordinator
            .restore_session_from_storage_path(&effective, session_id)
            .await
            .map_err(|e| format!("Failed to restore session before running /init: {e}"))?;
    }

    let workspace_path = request
        .workspace_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    scheduler
        .submit_init_agents_md(
            session_id.to_string(),
            workspace_path,
            request.remote_connection_id.clone(),
            request.remote_ssh_host.clone(),
            DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopUi),
        )
        .await
        .map_err(|e| format!("Failed to run /init: {e}"))?;

    Ok(StartDialogTurnResponse {
        success: true,
        message: "Init dialog turn started".to_string(),
    })
}

fn is_blank_text(value: Option<&String>) -> bool {
    value.map(|s| s.trim().is_empty()).unwrap_or(true)
}

fn resolve_missing_image_payloads(
    image_contexts: Vec<ImageContextData>,
) -> Result<Vec<ImageContextData>, String> {
    let mut resolved = Vec::with_capacity(image_contexts.len());

    for mut image in image_contexts {
        let missing_payload =
            is_blank_text(image.image_path.as_ref()) && is_blank_text(image.data_url.as_ref());
        if !missing_payload {
            resolved.push(image);
            continue;
        }

        let stored = get_image_context(&image.id).ok_or_else(|| {
            format!(
                "Image context not found for image_id={}. It may have expired. Please re-attach the image and retry.",
                image.id
            )
        })?;

        if is_blank_text(image.image_path.as_ref()) {
            image.image_path = stored
                .image_path
                .clone()
                .filter(|s: &String| !s.trim().is_empty());
        }
        if is_blank_text(image.data_url.as_ref()) {
            image.data_url = stored
                .data_url
                .clone()
                .filter(|s: &String| !s.trim().is_empty());
        }
        if image.mime_type.trim().is_empty() {
            image.mime_type = stored.mime_type.clone();
        }

        let mut metadata = image
            .metadata
            .take()
            .unwrap_or_else(|| serde_json::json!({}));
        if !metadata.is_object() {
            metadata = serde_json::json!({ "raw_metadata": metadata });
        }
        if let Some(obj) = metadata.as_object_mut() {
            if !obj.contains_key("name") {
                obj.insert("name".to_string(), serde_json::json!(stored.image_name));
            }
            if !obj.contains_key("width") {
                obj.insert("width".to_string(), serde_json::json!(stored.width));
            }
            if !obj.contains_key("height") {
                obj.insert("height".to_string(), serde_json::json!(stored.height));
            }
            if !obj.contains_key("file_size") {
                obj.insert("file_size".to_string(), serde_json::json!(stored.file_size));
            }
            if !obj.contains_key("source") {
                obj.insert("source".to_string(), serde_json::json!(stored.source));
            }
            obj.insert(
                "resolved_from_upload_cache".to_string(),
                serde_json::json!(true),
            );
        }
        image.metadata = Some(metadata);

        let still_missing =
            is_blank_text(image.image_path.as_ref()) && is_blank_text(image.data_url.as_ref());
        if still_missing {
            return Err(format!(
                "Image context {} is missing image_path/data_url after cache resolution",
                image.id
            ));
        }

        resolved.push(image);
    }

    Ok(resolved)
}

#[tauri::command]
pub async fn cancel_dialog_turn(
    runtime: State<'_, DesktopRuntimeContext>,
    app_state: State<'_, AppState>,
    request: CancelDialogTurnRequest,
) -> Result<(), String> {
    if let Some(acp_client_service) = app_state.acp_client_service.as_ref() {
        match acp_client_service
            .cancel_bitfun_session(&request.session_id)
            .await
        {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(error) => {
                log::error!(
                    "Failed to cancel ACP dialog turn: session_id={}, dialog_turn_id={}, error={}",
                    request.session_id,
                    request.dialog_turn_id,
                    error
                );
                return Err(format!("Failed to cancel ACP dialog turn: {}", error));
            }
        }
    }

    runtime
        .agent_runtime()
        .cancel_turn(AgentTurnCancellationRequest {
            session_id: request.session_id.clone(),
            turn_id: Some(request.dialog_turn_id.clone()),
            source: Some(AgentSubmissionSource::DesktopUi),
            requester_session_id: None,
            reason: None,
            wait_timeout_ms: None,
        })
        .await
        .map_err(|e| {
            log::error!(
                "Failed to cancel dialog turn: session_id={}, dialog_turn_id={}, error={}",
                request.session_id,
                request.dialog_turn_id,
                e
            );
            format!("Failed to cancel dialog turn: {}", e.into_message())
        })
        .map(|_| ())
}

#[tauri::command]
pub async fn steer_dialog_turn(
    scheduler: State<'_, Arc<DialogScheduler>>,
    request: SteerDialogTurnRequest,
) -> Result<SteerDialogTurnResponse, String> {
    let SteerDialogTurnRequest {
        session_id,
        dialog_turn_id,
        content,
        display_content,
    } = request;

    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("Steering content cannot be empty".to_string());
    }

    let outcome = scheduler
        .submit_steering(session_id, dialog_turn_id, content, display_content)
        .await
        .map_err(|e| format!("Failed to steer dialog turn: {}", e))?;

    let steering_id = match outcome {
        bitfun_core::agentic::coordination::DialogSteerOutcome::Buffered {
            steering_id, ..
        } => steering_id,
    };

    Ok(SteerDialogTurnResponse {
        success: true,
        steering_id,
    })
}

#[tauri::command]
pub async fn control_deep_review_queue(
    request: ControlDeepReviewQueueRequest,
) -> Result<(), String> {
    if request.session_id.trim().is_empty() {
        return Err("Missing session_id".to_string());
    }
    if request.dialog_turn_id.trim().is_empty() {
        return Err("Missing dialog_turn_id".to_string());
    }
    if request.tool_id.trim().is_empty() {
        return Err("Missing tool_id".to_string());
    }

    apply_deep_review_queue_control(
        &request.dialog_turn_id,
        &request.tool_id,
        request.action.into(),
    );
    Ok(())
}

#[tauri::command]
pub async fn cancel_session(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: CancelSessionRequest,
) -> Result<(), String> {
    coordinator
        .cancel_active_turn_for_session(&request.session_id, std::time::Duration::from_secs(5))
        .await
        .map_err(|e| {
            log::error!(
                "Failed to cancel session: session_id={}, error={}",
                request.session_id,
                e
            );
            format!("Failed to cancel session: {}", e)
        })?;

    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSubagentTimeoutRequest {
    pub session_id: String,
    pub action: SetSubagentTimeoutActionDTO,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum SetSubagentTimeoutActionDTO {
    Disable,
    Restore,
    Extend { seconds: u64 },
}

impl From<SetSubagentTimeoutActionDTO> for SubagentTimeoutAction {
    fn from(dto: SetSubagentTimeoutActionDTO) -> Self {
        match dto {
            SetSubagentTimeoutActionDTO::Disable => SubagentTimeoutAction::Disable,
            SetSubagentTimeoutActionDTO::Restore => SubagentTimeoutAction::Restore,
            SetSubagentTimeoutActionDTO::Extend { seconds } => {
                SubagentTimeoutAction::Extend { seconds }
            }
        }
    }
}

#[tauri::command]
pub async fn set_subagent_timeout(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: SetSubagentTimeoutRequest,
) -> Result<(), String> {
    let action: SubagentTimeoutAction = request.action.into();
    coordinator
        .set_subagent_timeout(&request.session_id, action)
        .await
        .map_err(|e| {
            log::error!(
                "Failed to set subagent timeout: session_id={}, error={}",
                request.session_id,
                e
            );
            format!("Failed to set subagent timeout: {}", e)
        })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlBackgroundCommandRequest {
    pub exec_session_id: i32,
    pub action: BackgroundCommandControlActionDTO,
    pub remote: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackgroundCommandControlActionDTO {
    Interrupt,
    Kill,
}

impl From<BackgroundCommandControlActionDTO> for ExecCommandControlAction {
    fn from(action: BackgroundCommandControlActionDTO) -> Self {
        match action {
            BackgroundCommandControlActionDTO::Interrupt => ExecCommandControlAction::Interrupt,
            BackgroundCommandControlActionDTO::Kill => ExecCommandControlAction::Kill,
        }
    }
}

#[tauri::command]
pub async fn control_background_command(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: ControlBackgroundCommandRequest,
) -> Result<(), String> {
    let session_id = request.exec_session_id;
    let remote = request.remote;
    let action: ExecCommandControlAction = request.action.into();
    let terminal_port = if remote {
        None
    } else {
        coordinator.inner().terminal_port()
    };
    let remote_exec_port = if remote {
        coordinator.inner().remote_exec_port()
    } else {
        None
    };

    control_exec_command_session(
        ExecCommandControlRequest {
            session_id,
            action,
            origin: ExecCommandControlOrigin::OutOfBand,
            remote,
            yield_time_ms: Some(250),
        },
        terminal_port.as_ref(),
        remote_exec_port.as_ref(),
    )
    .await
    .map(|response| {
        if response.session_id.is_none() {
            let status = match response.completion.map(|completion| completion.status) {
                Some(bitfun_core::agentic::tools::implementations::exec_command::ExecCommandCompletionStatus::Interrupted) => {
                    bitfun_core::agentic::tools::implementations::exec_command::BackgroundCommandOutputStatus::Interrupted
                }
                Some(bitfun_core::agentic::tools::implementations::exec_command::ExecCommandCompletionStatus::Killed) => {
                    bitfun_core::agentic::tools::implementations::exec_command::BackgroundCommandOutputStatus::Killed
                }
                Some(bitfun_core::agentic::tools::implementations::exec_command::ExecCommandCompletionStatus::Pruned) => {
                    bitfun_core::agentic::tools::implementations::exec_command::BackgroundCommandOutputStatus::Pruned
                }
                _ => bitfun_core::agentic::tools::implementations::exec_command::BackgroundCommandOutputStatus::Exited,
            };
            let capture = background_command_output_capture();
            tauri::async_runtime::spawn(async move {
                capture
                    .finish_by_session(remote, session_id, status, response.exit_code)
                    .await;
            });
        }
    })
    .map_err(|e| {
        log::error!(
            "Failed to control background command: exec_session_id={}, remote={}, error={}",
            session_id,
            remote,
            e
        );
        format!("Failed to control background command: {}", e)
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendBackgroundCommandInputRequest {
    pub exec_session_id: i32,
    pub remote: bool,
    pub chars: String,
    pub append_enter: bool,
}

#[tauri::command]
pub async fn send_background_command_input(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: SendBackgroundCommandInputRequest,
) -> Result<(), String> {
    if request.chars.is_empty() && !request.append_enter {
        return Err("chars or append_enter is required".to_string());
    }

    let session_id = request.exec_session_id;
    let remote = request.remote;
    let chars = request.chars;
    let append_enter = request.append_enter;
    let terminal_port = if remote {
        None
    } else {
        coordinator.inner().terminal_port()
    };
    let remote_exec_port = if remote {
        coordinator.inner().remote_exec_port()
    } else {
        None
    };
    send_exec_command_input(
        ExecCommandInputRequest {
            session_id,
            chars,
            append_enter,
            remote,
        },
        terminal_port.as_ref(),
        remote_exec_port.as_ref(),
    )
    .await
    .map_err(|e| {
        log::error!(
            "Failed to send input to background command: exec_session_id={}, remote={}, error={}",
            session_id,
            remote,
            e
        );
        format!("Failed to send input to background command: {}", e)
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadBackgroundCommandOutputRequest {
    pub exec_session_id: i32,
    pub remote: bool,
    pub cursor: Option<u64>,
}

#[tauri::command]
pub async fn read_background_command_output(
    request: ReadBackgroundCommandOutputRequest,
) -> Result<ReadBackgroundCommandOutputResponse, String> {
    background_command_output_capture()
        .read(CoreReadBackgroundCommandOutputRequest {
            exec_session_id: request.exec_session_id,
            remote: request.remote,
            cursor: request.cursor,
        })
        .await
        .ok_or_else(|| {
            format!(
                "Background command output not found: exec_session_id={}, remote={}",
                request.exec_session_id, request.remote
            )
        })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListBackgroundCommandActivitiesRequest {
    pub agent_session_id: Option<String>,
}

#[tauri::command]
pub async fn list_background_command_activities(
    request: ListBackgroundCommandActivitiesRequest,
) -> Result<ListBackgroundCommandOutputResponse, String> {
    let agent_session_id = request.agent_session_id.and_then(|value| {
        let trimmed = value.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    });

    Ok(background_command_output_capture()
        .list(ListBackgroundCommandOutputRequest { agent_session_id })
        .await)
}

#[tauri::command]
pub async fn cancel_tool(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: CancelToolRequest,
) -> Result<(), String> {
    let reason = request
        .reason
        .unwrap_or_else(|| "User cancelled".to_string());

    coordinator
        .cancel_tool(&request.tool_use_id, reason)
        .await
        .map_err(|e| {
            log::error!(
                "Failed to cancel tool execution: tool_use_id={}, error={}",
                request.tool_use_id,
                e
            );
            format!("Failed to cancel tool execution: {}", e)
        })
}

#[tauri::command]
pub async fn delete_session(
    runtime: State<'_, DesktopRuntimeContext>,
    request: DeleteSessionRequest,
) -> Result<(), String> {
    runtime
        .session_application()
        .delete_session(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            request.session_id,
        )
        .await
        .map_err(|error| format!("Failed to delete session: {error}"))
}

#[tauri::command]
pub async fn restore_session(
    runtime: State<'_, DesktopRuntimeContext>,
    request: RestoreSessionRequest,
) -> Result<SessionResponse, String> {
    let session = runtime
        .session_application()
        .restore_session(
            desktop_session_scope(
                request.workspace_path.clone(),
                request.remote_connection_id.clone(),
                request.remote_ssh_host.clone(),
            ),
            &request.session_id,
            request.include_internal,
        )
        .await
        .map_err(|error| format!("Failed to restore session: {error}"))?;

    Ok(session_to_response(session))
}

#[tauri::command]
pub async fn restore_session_view(
    runtime: State<'_, DesktopRuntimeContext>,
    startup_trace: State<'_, DesktopStartupTrace>,
    request: RestoreSessionRequest,
) -> Result<RestoreSessionViewResponse, String> {
    let started_at = Instant::now();
    let result = async {
        let trace_id = request.trace_id.as_deref().unwrap_or("none");
        debug!(
            "restore_session_view request received: trace_id={}, session_id={}",
            trace_id, request.session_id
        );
        let tail_turn_count = request
            .tail_turn_count
            .filter(|count| *count > 0)
            .map(|count| count.min(16));
        let restored = runtime
            .session_application()
            .restore_session_view(
                desktop_session_scope(
                    request.workspace_path.clone(),
                    request.remote_connection_id.clone(),
                    request.remote_ssh_host.clone(),
                ),
                &request.session_id,
                request.include_internal,
                tail_turn_count,
                |resolve_storage_path_duration_ms| {
                    debug!(
                        "restore_session_view storage path resolved: trace_id={}, session_id={}, duration_ms={}",
                        trace_id,
                        request.session_id,
                        resolve_storage_path_duration_ms
                    );
                },
            )
            .await
            .map_err(|error| format!("Failed to restore session view: {error}"))?;
        let session = restored.session;
        let mut turns = restored.turns;
        let total_turn_count = restored.total_turn_count;
        let timings = restored.timings;
        let loaded_turn_count = turns.len();
        let is_partial = loaded_turn_count < total_turn_count;

        if log::log_enabled!(log::Level::Debug) {
            let payload_stats = restore_turn_payload_stats(&turns);
            if payload_stats.raw_result_string_chars >= 1024 * 1024
                || payload_stats.result_for_assistant_chars >= 1024 * 1024
            {
                debug!(
                    "restore_session_view payload diagnostics: trace_id={}, session_id={}, turn_count={}, total_turn_count={}, is_partial={}, tool_result_count={}, raw_result_string_chars={}, result_for_assistant_chars={}, largest_raw_result_chars={}, largest_raw_result_path={}, top_raw_results={}",
                    trace_id,
                    request.session_id,
                    turns.len(),
                    total_turn_count,
                    is_partial,
                    payload_stats.tool_result_count,
                    payload_stats.raw_result_string_chars,
                    payload_stats.result_for_assistant_chars,
                    payload_stats.largest_raw_result_chars,
                    payload_stats.largest_raw_result_path,
                    format_top_raw_results(&payload_stats.top_raw_results)
                );
            }
        }

        compact_tool_results_for_session_view(&mut turns);

        debug!(
            "restore_session_view completed: trace_id={}, session_id={}, turn_count={}, total_turn_count={}, is_partial={}, context_restore_state=pending, duration_ms={}",
            trace_id,
            request.session_id,
            turns.len(),
            total_turn_count,
            is_partial,
            started_at.elapsed().as_millis()
        );

        Ok(RestoreSessionViewResponse {
            session: session_to_response_with_turn_count(session, total_turn_count),
            turns,
            context_restore_state: "pending".to_string(),
            is_partial,
            loaded_turn_count,
            total_turn_count,
            timings,
        })
    }
    .await;
    startup_trace.record_tauri_command_elapsed("restore_session_view", None, started_at);
    result
}

#[tauri::command]
pub async fn restore_session_with_turns(
    runtime: State<'_, DesktopRuntimeContext>,
    request: RestoreSessionRequest,
) -> Result<RestoreSessionWithTurnsResponse, String> {
    let started_at = std::time::Instant::now();
    let trace_id = request.trace_id.as_deref().unwrap_or("none");
    debug!(
        "restore_session_with_turns request received: trace_id={}, session_id={}",
        trace_id, request.session_id
    );
    let restored = runtime
        .session_application()
        .restore_session_with_turns(
            desktop_session_scope(
                request.workspace_path.clone(),
                request.remote_connection_id.clone(),
                request.remote_ssh_host.clone(),
            ),
            &request.session_id,
            request.include_internal,
            |resolve_storage_path_duration_ms| {
                debug!(
                    "restore_session_with_turns storage path resolved: trace_id={}, session_id={}, duration_ms={}",
                    trace_id,
                    request.session_id,
                    resolve_storage_path_duration_ms
                );
            },
        )
        .await
        .map_err(|error| format!("Failed to restore session: {error}"))?;
    let session = restored.session;
    let turns = restored.turns;

    if log::log_enabled!(log::Level::Debug) {
        let payload_stats = restore_turn_payload_stats(&turns);
        if payload_stats.raw_result_string_chars >= 1024 * 1024
            || payload_stats.result_for_assistant_chars >= 1024 * 1024
        {
            debug!(
                "restore_session_with_turns payload diagnostics: trace_id={}, session_id={}, turn_count={}, tool_result_count={}, raw_result_string_chars={}, result_for_assistant_chars={}, largest_raw_result_chars={}, largest_raw_result_path={}, top_raw_results={}",
                trace_id,
                request.session_id,
                turns.len(),
                payload_stats.tool_result_count,
                payload_stats.raw_result_string_chars,
                payload_stats.result_for_assistant_chars,
                payload_stats.largest_raw_result_chars,
                payload_stats.largest_raw_result_path,
                format_top_raw_results(&payload_stats.top_raw_results)
            );
        }
    }

    debug!(
        "restore_session_with_turns completed: trace_id={}, session_id={}, turn_count={}, duration_ms={}",
        trace_id,
        request.session_id,
        turns.len(),
        started_at.elapsed().as_millis()
    );

    Ok(RestoreSessionWithTurnsResponse {
        session: session_to_response(session),
        turns,
    })
}

#[tauri::command]
pub async fn list_sessions(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    app_state: State<'_, AppState>,
    request: ListSessionsRequest,
) -> Result<Vec<SessionResponse>, String> {
    let effective_path = desktop_effective_session_storage_path(
        &app_state,
        &request.workspace_path,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    let summaries = coordinator
        .list_sessions(&effective_path)
        .await
        .map_err(|e| format!("Failed to list sessions: {}", e))?;

    let responses = summaries
        .into_iter()
        .map(|summary| SessionResponse {
            session_id: summary.session_id,
            session_name: summary.session_name,
            agent_type: summary.agent_type,
            model_name: None,
            last_user_dialog_agent_type: summary.last_user_dialog_agent_type,
            last_submitted_agent_type: summary.last_submitted_agent_type,
            state: format!("{:?}", summary.state),
            turn_count: summary.turn_count,
            created_at: system_time_to_unix_secs(summary.created_at),
        })
        .collect();

    Ok(responses)
}

#[tauri::command]
pub async fn generate_session_title(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: GenerateSessionTitleRequest,
) -> Result<String, String> {
    coordinator
        .generate_session_title(
            &request.session_id,
            &request.user_message,
            request.max_length,
        )
        .await
        .map_err(|e| format!("Failed to generate session title: {}", e))
}

#[tauri::command]
pub async fn get_available_modes(
    state: State<'_, AppState>,
    startup_trace: State<'_, DesktopStartupTrace>,
) -> Result<Vec<ModeInfoDTO>, String> {
    let trace_started = Instant::now();
    let mode_infos = state.agent_registry.get_modes_info().await;

    let dtos: Vec<ModeInfoDTO> = mode_infos
        .into_iter()
        .map(|info| {
            let config_profile_id = info
                .config_profile_id
                .clone()
                .unwrap_or_else(|| info.id.clone());
            ModeInfoDTO {
                id: info.id,
                name: info.name,
                description: info.description,
                is_readonly: info.is_readonly,
                tool_count: info.tool_count,
                default_tools: info.default_tools,
                prompt_cache_scope_key: info.prompt_cache_scope_key,
                config_profile_id,
                config_profile_label: info.config_profile_label,
                config_profile_member_mode_ids: info.config_profile_member_mode_ids,
                source: info.source,
                path: info.path,
                model: info.model,
            }
        })
        .collect();

    startup_trace.record_tauri_command_elapsed("get_available_modes", None, trace_started);
    Ok(dtos)
}

#[tauri::command]
pub async fn get_default_review_team_definition() -> Result<ReviewTeamDefinition, String> {
    Ok(default_review_team_definition())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeInfoDTO {
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_readonly: bool,
    pub tool_count: usize,
    pub default_tools: Vec<String>,
    pub prompt_cache_scope_key: String,
    pub config_profile_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_profile_label: Option<String>,
    #[serde(default)]
    pub config_profile_member_mode_ids: Vec<String>,
    pub source: AgentSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

fn assistant_bootstrap_outcome_to_response(
    outcome: AssistantBootstrapEnsureOutcome,
) -> EnsureAssistantBootstrapResponse {
    match outcome {
        AssistantBootstrapEnsureOutcome::Started {
            session_id,
            turn_id,
        } => EnsureAssistantBootstrapResponse {
            status: "started".to_string(),
            reason: "bootstrap_started".to_string(),
            session_id,
            turn_id: Some(turn_id),
            detail: None,
        },
        AssistantBootstrapEnsureOutcome::Skipped { session_id, reason } => {
            EnsureAssistantBootstrapResponse {
                status: "skipped".to_string(),
                reason: assistant_bootstrap_skip_reason_to_str(reason).to_string(),
                session_id,
                turn_id: None,
                detail: None,
            }
        }
        AssistantBootstrapEnsureOutcome::Blocked {
            session_id,
            reason,
            detail,
        } => EnsureAssistantBootstrapResponse {
            status: "blocked".to_string(),
            reason: assistant_bootstrap_block_reason_to_str(reason).to_string(),
            session_id,
            turn_id: None,
            detail: Some(detail),
        },
    }
}

fn assistant_bootstrap_skip_reason_to_str(reason: AssistantBootstrapSkipReason) -> &'static str {
    match reason {
        AssistantBootstrapSkipReason::BootstrapNotRequired => "bootstrap_not_required",
        AssistantBootstrapSkipReason::SessionHasExistingTurns => "session_has_existing_turns",
        AssistantBootstrapSkipReason::SessionNotIdle => "session_not_idle",
    }
}

fn assistant_bootstrap_block_reason_to_str(reason: AssistantBootstrapBlockReason) -> &'static str {
    match reason {
        AssistantBootstrapBlockReason::ModelUnavailable => "model_unavailable",
    }
}

fn session_to_response(session: Session) -> SessionResponse {
    let turn_count = session.dialog_turn_ids.len();
    session_to_response_with_turn_count(session, turn_count)
}

fn session_to_response_with_turn_count(session: Session, turn_count: usize) -> SessionResponse {
    SessionResponse {
        session_id: session.session_id,
        session_name: session.session_name,
        agent_type: session.agent_type,
        model_name: session.config.model_id,
        last_user_dialog_agent_type: session.last_user_dialog_agent_type,
        last_submitted_agent_type: session.last_submitted_agent_type,
        state: format!("{:?}", session.state),
        turn_count,
        created_at: system_time_to_unix_secs(session.created_at),
    }
}

fn system_time_to_unix_secs(time: std::time::SystemTime) -> u64 {
    match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(err) => {
            warn!("Failed to convert SystemTime to unix timestamp: {}", err);
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_core::service::session::{
        ModelRoundData, ToolCallData, ToolItemData, ToolResultData, TurnStatus, UserMessageData,
    };
    use bitfun_product_domains::tool_permissions::{PermissionEffect, PermissionRule};
    use serde_json::json;

    #[test]
    fn project_permission_rule_revisions_distinguish_missing_and_present_files() {
        assert_eq!(
            project_permission_rules_revision(Some("{\"rules\":[]}")),
            project_permission_rules_revision(Some("{\"rules\":[]}"))
        );
        assert_ne!(
            project_permission_rules_revision(None),
            project_permission_rules_revision(Some(""))
        );
    }

    #[test]
    fn project_permission_rule_validation_requires_action_and_resource() {
        assert!(validate_project_permission_rules(&[PermissionRule::new(
            "edit",
            "src/*",
            PermissionEffect::Ask,
        )])
        .is_ok());
        assert!(validate_project_permission_rules(&[PermissionRule::new(
            " ",
            "src/*",
            PermissionEffect::Ask,
        )])
        .is_err());
    }

    #[test]
    fn desktop_dialog_turn_request_preserves_runtime_contract() {
        let request: StartDialogTurnRequest = serde_json::from_value(json!({
            "sessionId": "session-1",
            "userInput": "resolved input",
            "originalUserInput": "original input",
            "agentType": "agentic",
            "workspacePath": "/workspace/project",
            "remoteConnectionId": "connection-1",
            "remoteSshHost": "host-1",
            "turnId": "turn-1",
            "imageContexts": [{
                "id": "image-1",
                "image_path": "/workspace/clip.png",
                "data_url": "data:image/png;base64,abc",
                "mime_type": "image/png",
                "metadata": {
                    "name": "clip.png",
                    "source": "upload"
                }
            }],
            "userMessageMetadata": {
                "surface": "flow_chat",
                "requestId": "request-1"
            }
        }))
        .expect("current Tauri request shape");

        let runtime_request =
            desktop_dialog_turn_request(request).expect("Desktop runtime request");

        assert_eq!(runtime_request.session_id, "session-1");
        assert_eq!(runtime_request.message, "resolved input");
        assert_eq!(
            runtime_request.original_message.as_deref(),
            Some("original input")
        );
        assert_eq!(runtime_request.turn_id.as_deref(), Some("turn-1"));
        assert_eq!(runtime_request.agent_type, "agentic");
        assert_eq!(
            runtime_request.workspace_path.as_deref(),
            Some("/workspace/project")
        );
        assert_eq!(
            runtime_request.remote_connection_id.as_deref(),
            Some("connection-1")
        );
        assert_eq!(runtime_request.remote_ssh_host.as_deref(), Some("host-1"));
        assert_eq!(
            runtime_request.policy.trigger_source,
            AgentSubmissionSource::DesktopUi
        );
        assert_eq!(
            runtime_request.policy,
            DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopUi)
        );
        assert!(runtime_request.reply_route.is_none());
        assert!(runtime_request.prepended_reminders.is_empty());
        assert_eq!(runtime_request.attachments.len(), 1);
        let attachment = &runtime_request.attachments[0];
        assert_eq!(attachment.kind, "remote_image");
        assert_eq!(attachment.id, "image-1");
        assert_eq!(
            attachment.metadata.get("imagePath"),
            Some(&json!("/workspace/clip.png"))
        );
        assert_eq!(
            attachment.metadata.get("dataUrl"),
            Some(&json!("data:image/png;base64,abc"))
        );
        assert_eq!(
            attachment.metadata.get("mimeType"),
            Some(&json!("image/png"))
        );
        assert_eq!(
            attachment
                .metadata
                .get("metadata")
                .and_then(|value| value.get("source")),
            Some(&json!("upload"))
        );
        assert_eq!(
            runtime_request.metadata.get("surface"),
            Some(&json!("flow_chat"))
        );
        assert_eq!(
            runtime_request.metadata.get("requestId"),
            Some(&json!("request-1"))
        );
    }

    #[test]
    fn permission_response_dto_uses_stable_camel_case_wire_shape() {
        let request: PermissionResponseRequest = serde_json::from_value(json!({
            "requestId": "permission-1",
            "reply": "reject",
            "feedback": "Use a read-only path"
        }))
        .expect("permission response request");

        assert_eq!(request.request_id, "permission-1");
        assert!(matches!(request.reply, PermissionReplyKind::Reject));
        assert_eq!(request.feedback.as_deref(), Some("Use a read-only path"));
        assert_eq!(
            permission_reply(request),
            PermissionReply::Reject {
                feedback: Some("Use a read-only path".to_string()),
            }
        );
    }

    #[test]
    fn desktop_interaction_dtos_keep_existing_camel_case_shape() {
        let cancel: CancelDialogTurnRequest = serde_json::from_value(json!({
            "sessionId": "session-1",
            "dialogTurnId": "turn-1"
        }))
        .expect("cancel request");
        assert_eq!(cancel.session_id, "session-1");
        assert_eq!(cancel.dialog_turn_id, "turn-1");
        assert_eq!(
            serde_json::to_value(StartDialogTurnResponse {
                success: true,
                message: "Dialog turn started".to_string(),
            })
            .expect("response"),
            json!({
                "success": true,
                "message": "Dialog turn started"
            })
        );
    }

    #[test]
    fn desktop_dialog_turn_accepts_and_normalizes_legacy_non_object_metadata() {
        let request = serde_json::from_value::<StartDialogTurnRequest>(json!({
            "sessionId": "session-1",
            "userInput": "hello",
            "agentType": "agentic",
            "userMessageMetadata": "not-an-object"
        }))
        .expect("legacy metadata request");
        let runtime_request =
            desktop_dialog_turn_request(request).expect("Desktop runtime request");

        assert_eq!(
            runtime_request.metadata.get("raw_metadata"),
            Some(&json!("not-an-object"))
        );
    }

    fn idempotent_create_request() -> CreateSessionRequest {
        CreateSessionRequest {
            session_id: Some("review_child_request-1".to_string()),
            session_name: "Review fixes".to_string(),
            agent_type: "CodeReview".to_string(),
            workspace_path: "/workspace".to_string(),
            workspace_id: None,
            session_kind: None,
            remote_connection_id: None,
            remote_ssh_host: None,
            relationship: Some(SessionRelationship {
                kind: Some(SessionRelationshipKind::Review),
                parent_session_id: Some("parent-1".to_string()),
                parent_request_id: Some("request-1".to_string()),
                parent_dialog_turn_id: Some("turn-1".to_string()),
                parent_turn_index: Some(1),
                ..Default::default()
            }),
            deep_review_run_manifest: None,
            review_target_evidence: None,
            config: None,
        }
    }

    #[test]
    fn existing_create_session_retry_returns_the_matching_session() {
        let request = idempotent_create_request();
        let mut metadata = SessionMetadata::new(
            "review_child_request-1".to_string(),
            "Review fixes".to_string(),
            "CodeReview".to_string(),
            "auto".to_string(),
        );
        metadata.relationship = request.relationship.clone();
        let relationship = metadata.relationship.as_mut().expect("relationship");
        relationship.parent_dialog_turn_id = Some("turn-2".to_string());
        relationship.parent_turn_index = Some(2);

        let response = existing_session_create_response(&request, &metadata)
            .expect("matching retry should reuse the session");

        assert_eq!(response.session_id, "review_child_request-1");
        assert_eq!(response.agent_type, "CodeReview");
    }

    #[test]
    fn existing_create_session_retry_rejects_identity_mismatch() {
        let request = idempotent_create_request();
        let mut metadata = SessionMetadata::new(
            "review_child_request-1".to_string(),
            "Other session".to_string(),
            "DeepReview".to_string(),
            "auto".to_string(),
        );
        metadata.relationship = request.relationship.clone();

        let error = existing_session_create_response(&request, &metadata)
            .expect_err("a conflicting session id must not be reused");

        assert!(error.contains("different identity"));
    }

    #[test]
    fn existing_create_session_retry_rejects_a_different_parent_request() {
        let request = idempotent_create_request();
        let mut metadata = SessionMetadata::new(
            "review_child_request-1".to_string(),
            "Review fixes".to_string(),
            "CodeReview".to_string(),
            "auto".to_string(),
        );
        let mut relationship = request.relationship.clone().expect("relationship");
        relationship.parent_request_id = Some("request-2".to_string());
        metadata.relationship = Some(relationship);

        assert!(existing_session_create_response(&request, &metadata).is_err());
    }

    #[test]
    fn existing_create_session_retry_rejects_different_target_evidence() {
        let mut request = idempotent_create_request();
        request.review_target_evidence = Some(json!({ "fingerprint": "requested" }));
        let mut metadata = SessionMetadata::new(
            "review_child_request-1".to_string(),
            "Review fixes".to_string(),
            "CodeReview".to_string(),
            "auto".to_string(),
        );
        metadata.relationship = request.relationship.clone();
        metadata.review_target_evidence = Some(json!({ "fingerprint": "existing" }));

        assert!(existing_session_create_response(&request, &metadata).is_err());
    }

    #[test]
    fn existing_create_session_retry_allows_missing_target_evidence_repair() {
        let mut request = idempotent_create_request();
        request.review_target_evidence = Some(json!({ "fingerprint": "requested" }));
        let mut metadata = SessionMetadata::new(
            "review_child_request-1".to_string(),
            "Review fixes".to_string(),
            "CodeReview".to_string(),
            "auto".to_string(),
        );
        metadata.relationship = request.relationship.clone();

        assert!(existing_session_create_response(&request, &metadata).is_ok());
    }

    #[test]
    fn ordinary_explicit_session_ids_do_not_use_review_idempotency() {
        let mut request = idempotent_create_request();
        request.relationship = None;

        assert!(!is_idempotent_review_create(&request));
    }

    fn tool_item(
        tool_name: &str,
        result: serde_json::Value,
        assistant: Option<&str>,
    ) -> ToolItemData {
        ToolItemData {
            id: format!("tool-{}", tool_name),
            tool_name: tool_name.to_string(),
            tool_call: ToolCallData {
                id: format!("call-{}", tool_name),
                input: json!({}),
            },
            tool_result: Some(ToolResultData {
                result,
                success: true,
                result_for_assistant: assistant.map(str::to_string),
                error: None,
                duration_ms: Some(1),
            }),
            ai_intent: None,
            start_time: 1,
            end_time: Some(2),
            duration_ms: Some(1),
            queue_wait_ms: None,
            preflight_ms: None,
            confirmation_wait_ms: None,
            execution_ms: None,
            order_index: None,
            is_subagent_item: None,
            parent_task_tool_id: None,
            subagent_session_id: None,
            subagent_dialog_turn_id: None,
            attempt_id: None,
            attempt_index: None,
            subagent_model_id: None,
            subagent_model_display_name: None,
            status: None,
            interruption_reason: None,
        }
    }

    #[test]
    fn restore_turn_payload_stats_tracks_largest_outputs_without_contents() {
        let turn = DialogTurnData {
            turn_id: "turn-1".to_string(),
            turn_index: 0,
            session_id: "session-1".to_string(),
            timestamp: 1,
            kind: Default::default(),
            agent_type: None,
            user_message: UserMessageData {
                id: "user-1".to_string(),
                content: "hello".to_string(),
                timestamp: 1,
                metadata: None,
            },
            model_rounds: vec![ModelRoundData {
                id: "round-1".to_string(),
                turn_id: "turn-1".to_string(),
                round_index: 0,
                round_group_id: None,
                timestamp: 1,
                text_items: vec![],
                tool_items: vec![
                    tool_item("Read", json!({ "content": "abc" }), Some("assistant")),
                    tool_item("Bash", json!({ "output": "x".repeat(20) }), Some("short")),
                ],
                thinking_items: vec![],
                start_time: 1,
                end_time: Some(2),
                duration_ms: Some(1),
                provider_id: None,
                model_config_id: None,
                effective_model_name: None,
                first_chunk_ms: None,
                first_visible_output_ms: None,
                stream_duration_ms: None,
                attempt_count: None,
                failure_category: None,
                token_details: None,
                status: "completed".to_string(),
            }],
            start_time: 1,
            end_time: Some(2),
            duration_ms: Some(1),
            token_usage: None,
            finish_reason: None,
            has_final_response: None,
            status: TurnStatus::Completed,
        };

        let stats = restore_turn_payload_stats(&[turn]);

        assert_eq!(stats.tool_result_count, 2);
        assert_eq!(stats.raw_result_string_chars, 23);
        assert_eq!(stats.result_for_assistant_chars, 14);
        assert_eq!(stats.largest_raw_result_chars, 20);
        assert_eq!(stats.top_raw_results.len(), 2);
        assert_eq!(stats.top_raw_results[0].tool_name, "Bash");
        assert_eq!(stats.top_raw_results[0].raw_result_string_chars, 20);
        assert_eq!(stats.top_raw_results[0].result_for_assistant_chars, 5);
        assert_eq!(stats.top_raw_results[1].tool_name, "Read");
        assert_eq!(stats.top_raw_results[1].raw_result_string_chars, 3);
        assert!(!stats.top_raw_results[0].path.contains(&"x".repeat(20)));
    }

    #[test]
    fn session_view_response_can_report_total_turn_count_for_tail_view() {
        let mut session = Session::new_with_id(
            "session-1".to_string(),
            "Tail view".to_string(),
            "agentic".to_string(),
            SessionConfig::default(),
        );
        session.dialog_turn_ids = vec!["turn-49".to_string(), "turn-50".to_string()];

        let response = session_to_response_with_turn_count(session, 50);

        assert_eq!(response.turn_count, 50);
    }

    #[test]
    fn omit_assistant_only_tool_results_preserves_visible_results() {
        let mut turns = vec![DialogTurnData {
            turn_id: "turn-1".to_string(),
            turn_index: 0,
            session_id: "session-1".to_string(),
            timestamp: 1,
            kind: Default::default(),
            agent_type: None,
            user_message: UserMessageData {
                id: "user-1".to_string(),
                content: "hello".to_string(),
                timestamp: 1,
                metadata: None,
            },
            model_rounds: vec![ModelRoundData {
                id: "round-1".to_string(),
                turn_id: "turn-1".to_string(),
                round_index: 0,
                round_group_id: None,
                timestamp: 1,
                text_items: vec![],
                tool_items: vec![tool_item(
                    "Bash",
                    json!({ "output": "visible output" }),
                    Some("assistant-only payload"),
                )],
                thinking_items: vec![],
                start_time: 1,
                end_time: Some(2),
                duration_ms: Some(1),
                provider_id: None,
                model_config_id: None,
                effective_model_name: None,
                first_chunk_ms: None,
                first_visible_output_ms: None,
                stream_duration_ms: None,
                attempt_count: None,
                failure_category: None,
                token_details: None,
                status: "completed".to_string(),
            }],
            start_time: 1,
            end_time: Some(2),
            duration_ms: Some(1),
            token_usage: None,
            finish_reason: None,
            has_final_response: None,
            status: TurnStatus::Completed,
        }];

        omit_assistant_only_tool_results_for_session_view(&mut turns);

        let tool_result = turns[0].model_rounds[0].tool_items[0]
            .tool_result
            .as_ref()
            .expect("tool result should remain present");
        assert_eq!(tool_result.result["output"], "visible output");
        assert_eq!(tool_result.result_for_assistant, None);
    }

    #[test]
    fn session_view_tool_result_compaction_truncates_large_visible_results() {
        let large_output = "x".repeat(80 * 1024);
        let mut turns = vec![DialogTurnData {
            turn_id: "turn-1".to_string(),
            turn_index: 0,
            session_id: "session-1".to_string(),
            timestamp: 1,
            kind: Default::default(),
            agent_type: None,
            user_message: UserMessageData {
                id: "user-1".to_string(),
                content: "hello".to_string(),
                timestamp: 1,
                metadata: None,
            },
            model_rounds: vec![ModelRoundData {
                id: "round-1".to_string(),
                turn_id: "turn-1".to_string(),
                round_index: 0,
                round_group_id: None,
                timestamp: 1,
                text_items: vec![],
                tool_items: vec![tool_item(
                    "Bash",
                    json!({ "output": large_output, "exit_code": 0 }),
                    Some("assistant-only payload"),
                )],
                thinking_items: vec![],
                start_time: 1,
                end_time: Some(2),
                duration_ms: Some(1),
                provider_id: None,
                model_config_id: None,
                effective_model_name: None,
                first_chunk_ms: None,
                first_visible_output_ms: None,
                stream_duration_ms: None,
                attempt_count: None,
                failure_category: None,
                token_details: None,
                status: "completed".to_string(),
            }],
            start_time: 1,
            end_time: Some(2),
            duration_ms: Some(1),
            token_usage: None,
            finish_reason: None,
            has_final_response: None,
            status: TurnStatus::Completed,
        }];

        omit_assistant_only_tool_results_for_session_view(&mut turns);

        let tool_result = turns[0].model_rounds[0].tool_items[0]
            .tool_result
            .as_ref()
            .expect("tool result should remain present");
        let output = tool_result.result["output"]
            .as_str()
            .expect("output should remain a visible string preview");
        assert!(output.len() < 80 * 1024);
        assert!(!output.contains("[truncated for session view]"));
        assert!(output.contains("Output truncated for session preview"));
        assert_eq!(tool_result.result["exit_code"], 0);
        assert_eq!(tool_result.result_for_assistant, None);
    }

    #[test]
    fn deserializes_set_subagent_timeout_disable_without_payload_field() {
        let request: SetSubagentTimeoutRequest =
            serde_json::from_str(r#"{"sessionId":"subagent-session","action":{"type":"Disable"}}"#)
                .expect("disable action without payload should deserialize");
        assert_eq!(request.session_id, "subagent-session");
        assert!(matches!(
            request.action,
            SetSubagentTimeoutActionDTO::Disable
        ));
    }

    #[test]
    fn deserializes_set_subagent_timeout_disable_with_null_payload() {
        let request: SetSubagentTimeoutRequest = serde_json::from_str(
            r#"{"sessionId":"subagent-session","action":{"type":"Disable","payload":null}}"#,
        )
        .expect("disable action with null payload should deserialize");
        assert_eq!(request.session_id, "subagent-session");
        assert!(matches!(
            request.action,
            SetSubagentTimeoutActionDTO::Disable
        ));
    }
}
