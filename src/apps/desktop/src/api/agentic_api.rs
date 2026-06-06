//! Agentic API

use log::{debug, warn};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, State};

use crate::api::app_state::AppState;
use crate::api::session_storage_path::desktop_effective_session_storage_path;
use crate::startup_trace::DesktopStartupTrace;
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
use bitfun_core::agentic::session::{SessionViewRestoreRequest, SessionViewRestoreTiming};
use bitfun_core::agentic::tools::image_context::get_image_context;
use bitfun_core::agentic::tools::implementations::exec_command::{
    background_command_output_capture, control_exec_command_session, send_exec_command_input,
    ExecCommandControlAction, ExecCommandControlOrigin, ExecCommandControlRequest,
    ExecCommandInputRequest, ListBackgroundCommandOutputRequest,
    ListBackgroundCommandOutputResponse,
    ReadBackgroundCommandOutputRequest as CoreReadBackgroundCommandOutputRequest,
    ReadBackgroundCommandOutputResponse,
};
use bitfun_core::service::session::{DialogTurnData, SessionRelationship};

const SESSION_VIEW_TOOL_RESULT_TOTAL_CHAR_BUDGET: usize = 512 * 1024;
const SESSION_VIEW_TOOL_RESULT_STRING_CHAR_LIMIT: usize = 16 * 1024;
const SESSION_VIEW_TRUNCATED_MARKER: &str = "\n... Output truncated for session preview";
const SESSION_VIEW_OMITTED_MARKER: &str = "Output omitted from session preview";

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
    pub compression_threshold: Option<f32>,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmToolRequest {
    pub session_id: String,
    pub tool_id: String,
    pub updated_input: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectToolRequest {
    pub session_id: String,
    pub tool_id: String,
    pub reason: Option<String>,
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
    request: CreateSessionRequest,
) -> Result<CreateSessionResponse, String> {
    fn norm_conn(s: Option<String>) -> Option<String> {
        s.map(|x| x.trim().to_string()).filter(|x| !x.is_empty())
    }
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

    let config = request
        .config
        .map(|c| SessionConfig {
            max_context_tokens: c.max_context_tokens.unwrap_or(128128),
            auto_compact: c.auto_compact.unwrap_or(true),
            enable_tools: c.enable_tools.unwrap_or(true),
            safe_mode: c.safe_mode.unwrap_or(true),
            max_turns: c.max_turns.unwrap_or(200),
            enable_context_compression: c.enable_context_compression.unwrap_or(true),
            compression_threshold: c.compression_threshold.unwrap_or(0.8),
            workspace_path: Some(request.workspace_path.clone()),
            workspace_id: request.workspace_id.clone(),
            remote_connection_id: remote_conn.clone(),
            remote_ssh_host: remote_ssh_host.clone(),
            model_id: c.model_name,
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

    Ok(CreateSessionResponse {
        session_id: session.session_id,
        session_name: session.session_name,
        agent_type: session.agent_type,
    })
}

#[tauri::command]
pub async fn update_session_model(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: UpdateSessionModelRequest,
) -> Result<(), String> {
    coordinator
        .update_session_model(&request.session_id, &request.model_name)
        .await
        .map_err(|e| format!("Failed to update session model: {}", e))
}

#[tauri::command]
pub async fn update_session_title(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    app_state: State<'_, AppState>,
    request: UpdateSessionTitleRequest,
) -> Result<String, String> {
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
            .restore_session(&effective, session_id)
            .await
            .map_err(|e| format!("Failed to restore session before renaming: {}", e))?;
    }

    coordinator
        .update_session_title(session_id, &request.title)
        .await
        .map_err(|e| format!("Failed to update session title: {}", e))
}

/// Load the session into the coordinator process when it exists on disk but is not in memory.
/// Uses the same remote→local session path mapping as `restore_session`.
#[tauri::command]
pub async fn ensure_coordinator_session(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    app_state: State<'_, AppState>,
    request: EnsureCoordinatorSessionRequest,
) -> Result<(), String> {
    let session_id = request.session_id.trim();
    if session_id.is_empty() {
        return Err("session_id is required".to_string());
    }
    if coordinator
        .get_session_manager()
        .get_session(session_id)
        .is_some()
    {
        return Ok(());
    }

    let wp = request.workspace_path.trim();
    if wp.is_empty() {
        return Err("workspace_path is required when the session is not loaded".to_string());
    }

    let effective = desktop_effective_session_storage_path(
        &app_state,
        wp,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    let restore_result = if request.include_internal {
        coordinator
            .restore_internal_session(&effective, session_id)
            .await
    } else {
        coordinator.restore_session(&effective, session_id).await
    };
    restore_result.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_dialog_turn(
    _app: AppHandle,
    _coordinator: State<'_, Arc<ConversationCoordinator>>,
    scheduler: State<'_, Arc<DialogScheduler>>,
    request: StartDialogTurnRequest,
) -> Result<StartDialogTurnResponse, String> {
    let StartDialogTurnRequest {
        session_id,
        user_input,
        original_user_input,
        agent_type,
        workspace_path,
        turn_id,
        image_contexts,
        user_message_metadata,
    } = request;

    let policy = DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopUi);
    let resolved_images = if let Some(image_contexts) = image_contexts
        .as_ref()
        .filter(|images| !images.is_empty())
        .cloned()
    {
        Some(resolve_missing_image_payloads(image_contexts)?)
    } else {
        None
    };

    scheduler
        .submit(
            session_id,
            user_input,
            original_user_input,
            turn_id,
            agent_type,
            workspace_path,
            policy,
            None,
            user_message_metadata,
            resolved_images,
        )
        .await
        .map_err(|e| format!("Failed to start dialog turn: {}", e))?;

    Ok(StartDialogTurnResponse {
        success: true,
        message: "Dialog turn started".to_string(),
    })
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
            .restore_session(&effective, session_id)
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
            .restore_session(&effective, session_id)
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
            .restore_session(&effective, session_id)
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

async fn resolve_session_workspace_path_for_thread_goal_read(
    coordinator: &Arc<ConversationCoordinator>,
    app_state: &AppState,
    session_id: &str,
    workspace_path: Option<&str>,
    remote_connection_id: Option<&str>,
    remote_ssh_host: Option<&str>,
) -> Result<PathBuf, String> {
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
        let workspace_path = resolve_session_workspace_path_for_thread_goal_read(
            coordinator.inner(),
            app_state.inner(),
            session_id,
            request.workspace_path.as_deref(),
            request.remote_connection_id.as_deref(),
            request.remote_ssh_host.as_deref(),
        )
        .await?;
        let goal = coordinator
            .get_thread_goal(session_id, workspace_path.as_path())
            .await
            .map_err(|error| error.to_string())?;
        Ok(GetSessionThreadGoalResponse { goal })
    }
    .await;
    startup_trace.record_tauri_command_elapsed("get_session_thread_goal", None, trace_started);
    result
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
            .restore_session(&effective, session_id)
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
    coordinator: State<'_, Arc<ConversationCoordinator>>,
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

    coordinator
        .cancel_dialog_turn(&request.session_id, &request.dialog_turn_id)
        .await
        .map_err(|e| {
            log::error!(
                "Failed to cancel dialog turn: session_id={}, dialog_turn_id={}, error={}",
                request.session_id,
                request.dialog_turn_id,
                e
            );
            format!("Failed to cancel dialog turn: {}", e)
        })
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
    request: ControlBackgroundCommandRequest,
) -> Result<(), String> {
    let session_id = request.exec_session_id;
    let remote = request.remote;
    let action: ExecCommandControlAction = request.action.into();

    control_exec_command_session(ExecCommandControlRequest {
        session_id,
        action,
        origin: ExecCommandControlOrigin::OutOfBand,
        remote,
        yield_time_ms: Some(250),
        max_output_chars: Some(4_096),
    })
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
    request: SendBackgroundCommandInputRequest,
) -> Result<(), String> {
    if request.chars.is_empty() && !request.append_enter {
        return Err("chars or append_enter is required".to_string());
    }

    send_exec_command_input(ExecCommandInputRequest {
        session_id: request.exec_session_id,
        chars: request.chars,
        append_enter: request.append_enter,
        remote: request.remote,
    })
    .await
    .map_err(|e| {
        log::error!(
            "Failed to send input to background command: exec_session_id={}, remote={}, error={}",
            request.exec_session_id,
            request.remote,
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
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    app_state: State<'_, AppState>,
    request: DeleteSessionRequest,
) -> Result<(), String> {
    let effective_path = desktop_effective_session_storage_path(
        &app_state,
        &request.workspace_path,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    if let Some(acp_client_service) = app_state.acp_client_service.as_ref() {
        acp_client_service
            .release_bitfun_session(&request.session_id)
            .await;
    }
    coordinator
        .delete_session(&effective_path, &request.session_id)
        .await
        .map_err(|e| format!("Failed to delete session: {}", e))
}

#[tauri::command]
pub async fn restore_session(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    app_state: State<'_, AppState>,
    request: RestoreSessionRequest,
) -> Result<SessionResponse, String> {
    let effective_path = desktop_effective_session_storage_path(
        &app_state,
        &request.workspace_path,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    let session = if request.include_internal {
        coordinator
            .restore_internal_session(&effective_path, &request.session_id)
            .await
    } else {
        coordinator
            .restore_session(&effective_path, &request.session_id)
            .await
    }
    .map_err(|e| format!("Failed to restore session: {}", e))?;

    Ok(session_to_response(session))
}

#[tauri::command]
pub async fn restore_session_view(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    app_state: State<'_, AppState>,
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
        let path_started_at = Instant::now();
        let effective_path = desktop_effective_session_storage_path(
            &app_state,
            &request.workspace_path,
            request.remote_connection_id.as_deref(),
            request.remote_ssh_host.as_deref(),
        )
    .await;
        debug!(
            "restore_session_view storage path resolved: trace_id={}, session_id={}, duration_ms={}",
            trace_id,
            request.session_id,
            path_started_at.elapsed().as_millis()
        );

        let view_request = SessionViewRestoreRequest {
            workspace_path: effective_path,
            session_id: request.session_id.clone(),
            include_internal: request.include_internal,
            tail_turn_count: request.tail_turn_count,
        };
        let tail_turn_count = view_request
            .tail_turn_count
            .filter(|count| *count > 0)
            .map(|count| count.min(16));
        let (session, mut turns, total_turn_count, timings) =
            if let Some(tail_turn_count) = tail_turn_count {
                if view_request.include_internal {
                    coordinator
                        .restore_internal_session_view_tail_timed(
                            &view_request.workspace_path,
                            &view_request.session_id,
                            tail_turn_count,
                        )
                        .await
                } else {
                    coordinator
                        .restore_session_view_tail_timed(
                            &view_request.workspace_path,
                            &view_request.session_id,
                            tail_turn_count,
                        )
                        .await
                }
            } else if view_request.include_internal {
                coordinator
                    .restore_internal_session_view_timed(
                        &view_request.workspace_path,
                        &view_request.session_id,
                    )
                    .await
                    .map(|(session, turns, timings)| {
                        let total_turn_count = turns.len();
                        (session, turns, total_turn_count, timings)
                    })
            } else {
                coordinator
                    .restore_session_view_timed(&view_request.workspace_path, &view_request.session_id)
                    .await
                    .map(|(session, turns, timings)| {
                        let total_turn_count = turns.len();
                        (session, turns, total_turn_count, timings)
                    })
            }
            .map_err(|e| format!("Failed to restore session view: {}", e))?;
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
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    app_state: State<'_, AppState>,
    request: RestoreSessionRequest,
) -> Result<RestoreSessionWithTurnsResponse, String> {
    let started_at = std::time::Instant::now();
    let trace_id = request.trace_id.as_deref().unwrap_or("none");
    debug!(
        "restore_session_with_turns request received: trace_id={}, session_id={}",
        trace_id, request.session_id
    );
    let path_started_at = std::time::Instant::now();
    let effective_path = desktop_effective_session_storage_path(
        &app_state,
        &request.workspace_path,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    debug!(
        "restore_session_with_turns storage path resolved: trace_id={}, session_id={}, duration_ms={}",
        trace_id,
        request.session_id,
        path_started_at.elapsed().as_millis()
    );
    let (session, turns) = if request.include_internal {
        coordinator
            .restore_internal_session_with_turns(&effective_path, &request.session_id)
            .await
    } else {
        coordinator
            .restore_session_with_turns(&effective_path, &request.session_id)
            .await
    }
    .map_err(|e| format!("Failed to restore session: {}", e))?;

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
pub async fn confirm_tool_execution(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: ConfirmToolRequest,
) -> Result<(), String> {
    coordinator
        .confirm_tool(&request.tool_id, request.updated_input)
        .await
        .map_err(|e| format!("Confirm tool failed: {}", e))
}

#[tauri::command]
pub async fn reject_tool_execution(
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: RejectToolRequest,
) -> Result<(), String> {
    let reason = request
        .reason
        .unwrap_or_else(|| "User rejected".to_string());

    coordinator
        .reject_tool(&request.tool_id, reason)
        .await
        .map_err(|e| format!("Reject tool failed: {}", e))
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
    use serde_json::json;

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
            subagent_model_id: None,
            subagent_model_alias: None,
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
                model_id: None,
                model_alias: None,
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
                model_id: None,
                model_alias: None,
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
                model_id: None,
                model_alias: None,
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
