//! ACP client API

use crate::api::app_state::AppState;
use crate::api::session_storage_path::desktop_effective_session_storage_path;
use crate::startup_trace::DesktopStartupTrace;
use bitfun_acp::client::{
    AcpAvailableCommand, AcpClientInfo, AcpClientPermissionResponse, AcpClientRequirementProbe,
    AcpClientStreamEvent, AcpSessionOptions, CreateAcpFlowSessionRecordResponse,
    SetAcpSessionConfigOptionRequest, SetAcpSessionModelRequest,
    SubmitAcpPermissionResponseRequest,
};
use bitfun_core::agentic::image_analysis::ImageContextData;
use bitfun_core::agentic::persistence::PersistenceManager;
use bitfun_core::infrastructure::PathManager;
use bitfun_core::service::session::{
    DialogTurnData, ModelRoundData, TextItemData, ThinkingItemData, ToolCallData, ToolItemData,
    ToolResultData, TurnStatus, UserMessageData,
};
use bitfun_events::ToolEventData;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientIdRequest {
    pub client_id: String,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAcpFlowSessionRequest {
    pub client_id: String,
    #[serde(default)]
    pub session_name: Option<String>,
    pub workspace_path: String,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
}

pub type CreateAcpFlowSessionResponse = CreateAcpFlowSessionRecordResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartAcpDialogTurnRequest {
    pub session_id: String,
    pub client_id: String,
    pub user_input: String,
    #[serde(default)]
    pub original_user_input: Option<String>,
    pub turn_id: String,
    #[serde(default)]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    /// 图片上下文（L2-P2-1）：前端 ACPClientAPI.startDialogTurn 透传的
    /// imageContexts。此前 Rust 端无此字段，serde 静默忽略导致图片上下文
    /// 在 ACP 直通路径丢失。补字段后经 prompt_agent_stream 转成 ACP 协议
    /// ContentBlock::Image 发送给外部 agent。
    #[serde(default)]
    pub image_contexts: Option<Vec<ImageContextData>>,
    /// 用户消息元数据（L2-P2-1）：前端 userMessageMetadata 透传，经 ACP
    /// PromptRequest._meta 附带；同时随 dialog-turn-started 事件回显前端。
    #[serde(default)]
    pub user_message_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelAcpDialogTurnRequest {
    pub session_id: String,
    pub client_id: String,
    #[serde(default)]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAcpSessionOptionsRequest {
    pub session_id: String,
    pub client_id: String,
    #[serde(default)]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProbeAcpClientRequirementsRequest {
    #[serde(default)]
    pub remote_connection_id: Option<String>,
    #[serde(default)]
    pub force_refresh: bool,
}

fn emit_acp_model_round_completed(
    app_handle: &AppHandle,
    session_id: &str,
    turn_id: &str,
    round_id: String,
    has_tool_calls: bool,
) -> Result<(), bitfun_core::util::errors::BitFunError> {
    app_handle
        .emit(
            "agentic://model-round-completed",
            serde_json::json!({
                "sessionId": session_id,
                "turnId": turn_id,
                "roundId": round_id,
                "hasToolCalls": has_tool_calls,
            }),
        )
        .map_err(|e| bitfun_core::util::errors::BitFunError::service(e.to_string()))
}

/// Current unix time in milliseconds (fallback 0 on clock failure; never
/// panics).
fn acp_now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// In-progress accumulation of one ACP dialog turn's model rounds while the
/// external `prompt_agent_stream` events are being forwarded to the frontend.
///
/// The frontend-only persistence (debounced `saveSessionTurn`) is the
/// authoritative writer while it is online; this accumulator is the backend
/// safety-net copy so a turn is still persisted when the frontend is closed,
/// the session is not open, or the event stream is interrupted.
struct AcpDialogTurnAccumulator {
    current_round: Option<AcpAccumulatedRound>,
    rounds: Vec<AcpAccumulatedRound>,
}

impl Default for AcpDialogTurnAccumulator {
    fn default() -> Self {
        Self {
            current_round: None,
            rounds: Vec::new(),
        }
    }
}

impl AcpDialogTurnAccumulator {
    /// Begin a new model round, closing the previous one first.
    fn start_round(&mut self, round_id: String, round_index: usize) {
        self.finish_current_round();
        self.current_round = Some(AcpAccumulatedRound {
            round_id,
            round_index,
            started_at_ms: acp_now_unix_ms(),
            text_parts: Vec::new(),
            thinking_parts: Vec::new(),
            tool_items: Vec::new(),
            tool_index: HashMap::new(),
        });
    }

    /// Close the current round and append it to the completed rounds.
    fn finish_current_round(&mut self) {
        if let Some(round) = self.current_round.take() {
            self.rounds.push(round);
        }
    }

    /// Merge one ACP tool event into the current round, keyed by tool id so a
    /// Started + Completed (or Failed) pair yields a single tool item.
    fn apply_tool_event(&mut self, event: &ToolEventData) {
        let Some(round) = self.current_round.as_mut() else {
            return;
        };
        let Some(item) = acp_tool_event_to_tool_item(event) else {
            return;
        };
        let tool_id = item.id.clone();
        if let Some(index) = round.tool_index.get(&tool_id).copied() {
            let existing = &mut round.tool_items[index];
            // 保留 Started 时的参数（后续 Completed/Failed 更新不带参数）。
            if let Some(input) = acp_tool_event_started_input(event) {
                existing.tool_call.input = input;
            }
            if let Some(result) = item.tool_result {
                existing.tool_result = Some(result);
                existing.status = item.status;
            }
        } else {
            let index = round.tool_items.len();
            round.tool_index.insert(tool_id, index);
            round.tool_items.push(item);
        }
    }
}

/// One accumulated ACP model round, ready to be converted into
/// `ModelRoundData` when the turn completes.
struct AcpAccumulatedRound {
    round_id: String,
    round_index: usize,
    started_at_ms: u64,
    text_parts: Vec<String>,
    thinking_parts: Vec<String>,
    tool_items: Vec<ToolItemData>,
    tool_index: HashMap<String, usize>,
}

/// The Started-event input of an ACP tool event (`None` for non-Started
/// variants so a completed update never clears the recorded input).
fn acp_tool_event_started_input(event: &ToolEventData) -> Option<serde_json::Value> {
    match event {
        ToolEventData::Started { params, .. } => Some(params.clone()),
        _ => None,
    }
}

/// Map one ACP tool event into a persisted `ToolItemData`.
///
/// Only lifecycle variants that carry content (`Started` / `Completed` /
/// `Failed` / `Cancelled`) are persisted; informational variants
/// (`Progress`, `Streaming`, `Queued`, ...) are skipped.
fn acp_tool_event_to_tool_item(event: &ToolEventData) -> Option<ToolItemData> {
    let (identity, status, tool_result) = match event {
        ToolEventData::Started { identity, .. } => (identity, "in_progress", None),
        ToolEventData::Completed {
            identity,
            result,
            duration_ms,
            ..
        } => (
            identity,
            "completed",
            Some(ToolResultData {
                result: result.clone(),
                success: true,
                result_for_assistant: None,
                image_attachments: None,
                error: None,
                duration_ms: Some(*duration_ms),
            }),
        ),
        ToolEventData::Failed {
            identity,
            error,
            duration_ms,
            ..
        } => (
            identity,
            "failed",
            Some(ToolResultData {
                result: serde_json::Value::Null,
                success: false,
                result_for_assistant: None,
                image_attachments: None,
                error: Some(error.clone()),
                duration_ms: *duration_ms,
            }),
        ),
        ToolEventData::Cancelled {
            identity,
            reason,
            duration_ms,
            ..
        } => (
            identity,
            "cancelled",
            Some(ToolResultData {
                result: serde_json::Value::Null,
                success: false,
                result_for_assistant: None,
                image_attachments: None,
                error: Some(reason.clone()),
                duration_ms: *duration_ms,
            }),
        ),
        _ => return None,
    };
    Some(ToolItemData {
        id: identity.tool_id.clone(),
        tool_name: identity.effective_name().to_string(),
        tool_call: ToolCallData {
            input: acp_tool_event_started_input(event)
                .unwrap_or_else(|| serde_json::json!({})),
            id: identity.tool_id.clone(),
        },
        tool_result,
        ai_intent: None,
        start_time: acp_now_unix_ms(),
        end_time: None,
        duration_ms: None,
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
        status: Some(status.to_string()),
        interruption_reason: None,
    })
}

impl AcpAccumulatedRound {
    /// Convert the accumulated chunks and tool items into a persisted
    /// `ModelRoundData` (mirrors the frontend `convertDialogTurnToBackendFormat`
    /// shape: one text item per round, one thinking item per round, tool items
    /// in arrival order).
    fn into_model_round(self, turn_id: &str) -> ModelRoundData {
        let now_ms = acp_now_unix_ms();
        let mut text_items = Vec::new();
        let text = self.text_parts.concat();
        if !text.trim().is_empty() {
            text_items.push(TextItemData {
                id: uuid::Uuid::new_v4().to_string(),
                content: text,
                is_streaming: false,
                timestamp: self.started_at_ms,
                is_markdown: true,
                order_index: Some(0),
                is_subagent_item: None,
                parent_task_tool_id: None,
                subagent_session_id: None,
                status: Some("completed".to_string()),
                attempt_id: None,
                attempt_index: None,
            });
        }
        let mut thinking_items = Vec::new();
        let thinking = self.thinking_parts.concat();
        if !thinking.trim().is_empty() {
            thinking_items.push(ThinkingItemData {
                id: uuid::Uuid::new_v4().to_string(),
                content: thinking,
                is_streaming: false,
                is_collapsed: true,
                timestamp: self.started_at_ms,
                order_index: Some(0),
                status: Some("completed".to_string()),
                is_subagent_item: None,
                parent_task_tool_id: None,
                subagent_session_id: None,
                attempt_id: None,
                attempt_index: None,
            });
        }
        ModelRoundData {
            id: self.round_id,
            turn_id: turn_id.to_string(),
            round_index: self.round_index,
            round_group_id: None,
            timestamp: self.started_at_ms,
            text_items,
            tool_items: self.tool_items,
            thinking_items,
            start_time: self.started_at_ms,
            end_time: Some(now_ms),
            duration_ms: Some(now_ms.saturating_sub(self.started_at_ms)),
            provider_id: None,
            model_config_id: None,
            effective_model_name: None,
            first_chunk_ms: None,
            first_visible_output_ms: None,
            stream_duration_ms: None,
            attempt_count: None,
            attempt_diagnostics: Vec::new(),
            failure_category: None,
            token_details: None,
            status: "completed".to_string(),
        }
    }
}

/// Build the persisted `DialogTurnData` for one completed ACP dialog turn.
fn build_acp_dialog_turn_data(
    turn_id: &str,
    turn_index: usize,
    session_id: &str,
    user_input: &str,
    start_time_ms: u64,
    rounds: Vec<AcpAccumulatedRound>,
    status: TurnStatus,
    error: Option<String>,
) -> DialogTurnData {
    let mut turn = DialogTurnData::new(
        turn_id.to_string(),
        turn_index,
        session_id.to_string(),
        UserMessageData {
            id: uuid::Uuid::new_v4().to_string(),
            content: user_input.to_string(),
            timestamp: start_time_ms,
            metadata: None,
        },
    );
    turn.start_time = start_time_ms;
    turn.model_rounds = rounds
        .into_iter()
        .map(|round| round.into_model_round(turn_id))
        .collect();
    turn.error = error;
    match status {
        TurnStatus::Completed => turn.mark_completed(),
        TurnStatus::Cancelled | TurnStatus::Error => {
            turn.status = status;
            turn.end_time = Some(acp_now_unix_ms());
        }
        TurnStatus::InProgress => {}
    }
    turn
}

/// Backend safety-net persistence for one ACP dialog turn, independent of the
/// frontend event stream.
///
/// The turn index is derived from the persisted session metadata
/// (`turn_count`), matching the frontend's `indexOf` semantics when the turn
/// history is contiguous. When the frontend (online) already saved the same
/// turn at that index, this is a no-op; a collision with a different turn id
/// is skipped with a warning instead of overwriting foreign data. Failures are
/// logged, never propagated, so persistence can never break the streaming
/// path.
async fn persist_acp_dialog_turn_backend(
    persistence: &PersistenceManager,
    session_storage_path: &Path,
    session_id: &str,
    turn_id: &str,
    user_input: &str,
    start_time_ms: u64,
    rounds: Vec<AcpAccumulatedRound>,
    status: TurnStatus,
    error: Option<String>,
) {
    let Ok(Some(metadata)) = persistence
        .load_session_metadata(session_storage_path, session_id)
        .await
    else {
        log::warn!(
            "ACP turn persistence skipped: session metadata not found: session_id={}",
            session_id
        );
        return;
    };
    let known_turn_count = metadata.turn_count;
    // 幂等对齐直投路径（P-19 铁则）：同 turn_id 已在任意索引落盘 → no-op；
    // 否则从 turn_count 起向后扫描第一个空闲索引追加。单点索引检查在索引
    // 碰撞时静默丢弃回复全文（d3-P1-2/L2-P1-2），SessionHistory 检索不全。
    for index in 0..known_turn_count {
        if let Ok(Some(existing)) = persistence
            .load_dialog_turn(session_storage_path, session_id, index)
            .await
        {
            if existing.turn_id == turn_id {
                return;
            }
        }
    }
    let mut turn_index = known_turn_count;
    loop {
        match persistence
            .load_dialog_turn(session_storage_path, session_id, turn_index)
            .await
        {
            Ok(Some(existing)) if existing.turn_id == turn_id => {
                return;
            }
            Ok(Some(_)) => {
                turn_index += 1;
            }
            _ => break,
        }
    }
    let turn = build_acp_dialog_turn_data(
        turn_id,
        turn_index,
        session_id,
        user_input,
        start_time_ms,
        rounds,
        status,
        error,
    );
    if let Err(error) = persistence.save_dialog_turn(session_storage_path, &turn).await {
        log::warn!(
            "Failed to persist ACP dialog turn: session_id={} turn_id={} error={}",
            session_id,
            turn_id,
            error
        );
    }
}

/// Spawn the backend persistence task for a finished ACP dialog turn.
///
/// Runs off the event-stream path: a missing workspace storage path or a
/// persistence setup failure only logs a warning, never breaks streaming.
fn spawn_acp_turn_backend_persist(
    session_storage_path: Option<PathBuf>,
    session_id: String,
    turn_id: String,
    user_input: String,
    start_time_ms: u64,
    rounds: Vec<AcpAccumulatedRound>,
    status: TurnStatus,
    error: Option<String>,
) {
    let Some(session_storage_path) = session_storage_path else {
        return;
    };
    tokio::spawn(async move {
        let path_manager = match PathManager::new() {
            Ok(path_manager) => std::sync::Arc::new(path_manager),
            Err(error) => {
                log::warn!(
                    "ACP turn persistence skipped: failed to initialize PathManager: {}",
                    error
                );
                return;
            }
        };
        let persistence = match PersistenceManager::new(path_manager) {
            Ok(persistence) => persistence,
            Err(error) => {
                log::warn!(
                    "ACP turn persistence skipped: failed to initialize PersistenceManager: {}",
                    error
                );
                return;
            }
        };
        persist_acp_dialog_turn_backend(
            &persistence,
            &session_storage_path,
            &session_id,
            &turn_id,
            &user_input,
            start_time_ms,
            rounds,
            status,
            error,
        )
        .await;
    });
}

#[tauri::command]
pub async fn initialize_acp_clients(
    state: State<'_, AppState>,
    startup_trace: State<'_, DesktopStartupTrace>,
) -> Result<(), String> {
    let trace_started = Instant::now();
    let result = async {
        let service = state
            .acp_client_service
            .as_ref()
            .ok_or_else(|| "ACP client service not initialized".to_string())?;
        service.initialize_all().await.map_err(|e| e.to_string())
    }
    .await;
    startup_trace.record_tauri_command_elapsed("initialize_acp_clients", None, trace_started);
    result
}

#[tauri::command]
pub async fn get_acp_clients(state: State<'_, AppState>) -> Result<Vec<AcpClientInfo>, String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    service.list_clients().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn probe_acp_client_requirements(
    state: State<'_, AppState>,
    startup_trace: State<'_, DesktopStartupTrace>,
    request: ProbeAcpClientRequirementsRequest,
) -> Result<Vec<AcpClientRequirementProbe>, String> {
    let trace_started = Instant::now();
    let result = async {
        let service = state
            .acp_client_service
            .as_ref()
            .ok_or_else(|| "ACP client service not initialized".to_string())?;
        service
            .probe_client_requirements(
                request.remote_connection_id.as_deref(),
                request.force_refresh,
            )
            .await
            .map_err(|e| e.to_string())
    }
    .await;
    startup_trace.record_tauri_command_elapsed(
        "probe_acp_client_requirements",
        None,
        trace_started,
    );
    result
}

#[tauri::command]
pub async fn predownload_acp_client_adapter(
    state: State<'_, AppState>,
    request: AcpClientIdRequest,
) -> Result<(), String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    service
        .predownload_client_adapter(&request.client_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_acp_client_cli(
    state: State<'_, AppState>,
    request: AcpClientIdRequest,
) -> Result<(), String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    service
        .install_client_cli(&request.client_id, request.remote_connection_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_acp_flow_session(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    request: CreateAcpFlowSessionRequest,
) -> Result<CreateAcpFlowSessionResponse, String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;

    let session_storage_path = desktop_effective_session_storage_path(
        &state,
        &request.workspace_path,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    let response = service
        .create_flow_session_record(
            &session_storage_path,
            &request.workspace_path,
            &request.client_id,
            request.session_name,
        )
        .await
        .map_err(|e| e.to_string())?;
    if let Err(error) = service
        .start_client_for_session(
            &request.client_id,
            &response.session_id,
            Some(&request.workspace_path),
            request.remote_connection_id.as_deref(),
        )
        .await
    {
        if let Err(cleanup_error) = service
            .delete_flow_session_record(&session_storage_path, &response.session_id)
            .await
        {
            log::warn!(
                "Failed to delete ACP session record after client start failure: session_id={}, error={}",
                response.session_id,
                cleanup_error
            );
        }
        return Err(error.to_string());
    }

    let _ = app_handle.emit(
        "agentic://session-created",
        serde_json::json!({
            "sessionId": response.session_id.clone(),
            "sessionName": response.session_name.clone(),
            "agentType": response.agent_type.clone(),
            "workspacePath": request.workspace_path,
            "remoteConnectionId": request.remote_connection_id,
            "remoteSshHost": request.remote_ssh_host,
        }),
    );

    Ok(response)
}

/// Shared implementation for starting an ACP dialog turn.
///
/// Used by both the FlowChat path (`start_acp_dialog_turn` command) and the
/// agentic path (`start_dialog_turn` ACP branch). Emits the standard
/// `agentic://dialog-turn-*` Tauri events while streaming
/// `prompt_agent_stream` output; no internal executor is started.
pub(crate) async fn start_acp_dialog_turn_impl(
    app_handle: AppHandle,
    app_state: &AppState,
    request: StartAcpDialogTurnRequest,
) -> Result<(), String> {
    let service = app_state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?
        .clone();

    let session_id = request.session_id.clone();
    let turn_id = request.turn_id.clone();
    let user_input = request.user_input.clone();
    let original_user_input = request
        .original_user_input
        .clone()
        .unwrap_or_else(|| request.user_input.clone());
    let session_storage_path = match request.workspace_path.as_deref() {
        Some(workspace_path) => Some(
            desktop_effective_session_storage_path(
                app_state,
                workspace_path,
                request.remote_connection_id.as_deref(),
                request.remote_ssh_host.as_deref(),
            )
            .await,
        ),
        None => None,
    };

    let user_message_metadata_for_event = request
        .user_message_metadata
        .clone()
        .unwrap_or(serde_json::Value::Null);
    app_handle
        .emit(
            "agentic://dialog-turn-started",
            serde_json::json!({
                "sessionId": session_id,
                "turnId": turn_id,
                "turnIndex": null,
                "userInput": user_input,
                "originalUserInput": original_user_input,
                "userMessageMetadata": user_message_metadata_for_event,
                "subagentParentInfo": null,
            }),
        )
        .map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        let mut current_round_id: Option<String> = None;
        let mut current_round_has_tool_calls = false;
        // a19 后端兜底落盘：事件流同步累积模型轮次内容，Completed/Cancelled
        // 时经 PersistenceManager 落盘（不依赖前端事件接收）。
        let mut turn_accumulator = AcpDialogTurnAccumulator::default();
        let turn_started_at_ms = acp_now_unix_ms();
        let persist_storage_path = session_storage_path.clone();
        let persist_session_id = request.session_id.clone();
        let persist_turn_id = request.turn_id.clone();
        let persist_user_input = request.user_input.clone();
        let result = service
            .prompt_agent_stream(
                &request.client_id,
                request.user_input,
                request.workspace_path,
                request.remote_connection_id,
                request.session_id.clone(),
                session_storage_path,
                request.timeout_seconds,
                request.image_contexts,
                request.user_message_metadata,
                |event| {
                    match event {
                        AcpClientStreamEvent::ModelRoundStarted {
                            round_id,
                            round_index,
                            disable_explore_grouping,
                        } => {
                            if let Some(previous_round_id) = current_round_id.take() {
                                emit_acp_model_round_completed(
                                    &app_handle,
                                    &request.session_id,
                                    &request.turn_id,
                                    previous_round_id,
                                    current_round_has_tool_calls,
                                )?;
                            }
                            current_round_id = Some(round_id.clone());
                            current_round_has_tool_calls = false;
                            turn_accumulator.start_round(round_id.clone(), round_index);
                            app_handle
                                .emit(
                                    "agentic://model-round-started",
                                    serde_json::json!({
                                        "sessionId": request.session_id,
                                        "turnId": request.turn_id,
                                        "roundId": round_id,
                                        "roundIndex": round_index,
                                        "renderHints": {
                                            "disableExploreGrouping": disable_explore_grouping,
                                        },
                                        "subagentParentInfo": null,
                                    }),
                                )
                                .map_err(|e| {
                                    bitfun_core::util::errors::BitFunError::service(e.to_string())
                                })?;
                        }
                        AcpClientStreamEvent::AgentText(text) => {
                            let round_id = current_round_id.clone().ok_or_else(|| {
                                bitfun_core::util::errors::BitFunError::service(
                                    "ACP text arrived before model round start".to_string(),
                                )
                            })?;
                            if let Some(round) = turn_accumulator.current_round.as_mut() {
                                round.text_parts.push(text.clone());
                            }
                            app_handle
                                .emit(
                                    "agentic://text-chunk",
                                    serde_json::json!({
                                        "sessionId": request.session_id,
                                        "turnId": request.turn_id,
                                        "roundId": round_id,
                                        "text": text,
                                        "subagentParentInfo": null,
                                    }),
                                )
                                .map_err(|e| {
                                    bitfun_core::util::errors::BitFunError::service(e.to_string())
                                })?;
                        }
                        AcpClientStreamEvent::AgentThought(text) => {
                            let round_id = current_round_id.clone().ok_or_else(|| {
                                bitfun_core::util::errors::BitFunError::service(
                                    "ACP thought arrived before model round start".to_string(),
                                )
                            })?;
                            if let Some(round) = turn_accumulator.current_round.as_mut() {
                                round.thinking_parts.push(text.clone());
                            }
                            app_handle
                                .emit(
                                    "agentic://text-chunk",
                                    serde_json::json!({
                                        "sessionId": request.session_id,
                                        "turnId": request.turn_id,
                                        "roundId": round_id,
                                        "text": text,
                                        "contentType": "thinking",
                                        "isThinkingEnd": false,
                                        "subagentParentInfo": null,
                                    }),
                                )
                                .map_err(|e| {
                                    bitfun_core::util::errors::BitFunError::service(e.to_string())
                                })?;
                        }
                        AcpClientStreamEvent::ToolEvent(tool_event) => {
                            let round_id = current_round_id.clone().ok_or_else(|| {
                                bitfun_core::util::errors::BitFunError::service(
                                    "ACP tool event arrived before model round start".to_string(),
                                )
                            })?;
                            current_round_has_tool_calls = true;
                            turn_accumulator.apply_tool_event(&tool_event);
                            app_handle
                                .emit(
                                    "agentic://tool-event",
                                    serde_json::json!({
                                        "sessionId": request.session_id,
                                        "turnId": request.turn_id,
                                        "roundId": round_id,
                                        "toolEvent": tool_event,
                                        "subagentParentInfo": null,
                                    }),
                                )
                                .map_err(|e| {
                                    bitfun_core::util::errors::BitFunError::service(e.to_string())
                                })?;
                        }
                        AcpClientStreamEvent::ContextUsageUpdated(usage) => {
                            app_handle
                                .emit(
                                    "agentic://acp-context-usage-updated",
                                    serde_json::json!({
                                        "sessionId": request.session_id,
                                        "turnId": request.turn_id,
                                        "clientId": request.client_id,
                                        "used": usage.used,
                                        "size": usage.size,
                                        "cost": usage.cost,
                                        "subagentParentInfo": null,
                                    }),
                                )
                                .map_err(|e| {
                                    bitfun_core::util::errors::BitFunError::service(e.to_string())
                                })?;
                        }
                        AcpClientStreamEvent::AvailableCommandsUpdated(commands) => {
                            app_handle
                                .emit(
                                    "agentic://acp-available-commands-updated",
                                    serde_json::json!({
                                        "sessionId": request.session_id,
                                        "clientId": request.client_id,
                                        "commands": commands,
                                    }),
                                )
                                .map_err(|e| {
                                    bitfun_core::util::errors::BitFunError::service(e.to_string())
                                })?;
                        }
                        AcpClientStreamEvent::PlanUpdated(entries) => {
                            app_handle
                                .emit(
                                    "agentic://acp-plan-updated",
                                    serde_json::json!({
                                        "sessionId": request.session_id,
                                        "turnId": request.turn_id,
                                        "clientId": request.client_id,
                                        "entries": entries,
                                    }),
                                )
                                .map_err(|e| {
                                    bitfun_core::util::errors::BitFunError::service(e.to_string())
                                })?;
                        }
                        AcpClientStreamEvent::ConfigOptionsUpdated(_) => {
                            app_handle
                                .emit(
                                    "agentic://acp-session-options-changed",
                                    serde_json::json!({
                                        "sessionId": request.session_id,
                                        "clientId": request.client_id,
                                    }),
                                )
                                .map_err(|e| {
                                    bitfun_core::util::errors::BitFunError::service(e.to_string())
                                })?;
                        }
                        AcpClientStreamEvent::Completed => {
                            if let Some(round_id) = current_round_id.take() {
                                emit_acp_model_round_completed(
                                    &app_handle,
                                    &request.session_id,
                                    &request.turn_id,
                                    round_id,
                                    current_round_has_tool_calls,
                                )?;
                            }
                            turn_accumulator.finish_current_round();
                            spawn_acp_turn_backend_persist(
                                persist_storage_path.clone(),
                                persist_session_id.clone(),
                                persist_turn_id.clone(),
                                persist_user_input.clone(),
                                turn_started_at_ms,
                                std::mem::take(&mut turn_accumulator.rounds),
                                TurnStatus::Completed,
                                None,
                            );
                            app_handle
                                .emit(
                                    "agentic://dialog-turn-completed",
                                    serde_json::json!({
                                        "sessionId": request.session_id,
                                        "turnId": request.turn_id,
                                        "subagentParentInfo": null,
                                        "partialRecoveryReason": null,
                                    }),
                                )
                                .map_err(|e| {
                                    bitfun_core::util::errors::BitFunError::service(e.to_string())
                                })?;
                        }
                        AcpClientStreamEvent::Cancelled => {
                            if let Some(round_id) = current_round_id.take() {
                                emit_acp_model_round_completed(
                                    &app_handle,
                                    &request.session_id,
                                    &request.turn_id,
                                    round_id,
                                    current_round_has_tool_calls,
                                )?;
                            }
                            turn_accumulator.finish_current_round();
                            spawn_acp_turn_backend_persist(
                                persist_storage_path.clone(),
                                persist_session_id.clone(),
                                persist_turn_id.clone(),
                                persist_user_input.clone(),
                                turn_started_at_ms,
                                std::mem::take(&mut turn_accumulator.rounds),
                                TurnStatus::Cancelled,
                                None,
                            );
                            app_handle
                                .emit(
                                    "agentic://dialog-turn-cancelled",
                                    serde_json::json!({
                                        "sessionId": request.session_id,
                                        "turnId": request.turn_id,
                                        "subagentParentInfo": null,
                                    }),
                                )
                                .map_err(|e| {
                                    bitfun_core::util::errors::BitFunError::service(e.to_string())
                                })?;
                        }
                    }
                    Ok(())
                },
            )
            .await;

        if let Err(error) = result {
            // 超时/异常路径兜底（L2-P1-1）：错误时已流式内容必须落盘
            // （TurnStatus::Error）并 emit 终态事件，否则离线场景已流式
            // 回复丢失且前端收不到 dialog-turn-completed/failed 终态。
            turn_accumulator.finish_current_round();
            let finalize_round = std::mem::take(&mut turn_accumulator.rounds);
            if !finalize_round.is_empty() {
                spawn_acp_turn_backend_persist(
                    persist_storage_path.clone(),
                    persist_session_id.clone(),
                    persist_turn_id.clone(),
                    persist_user_input.clone(),
                    turn_started_at_ms,
                    finalize_round,
                    TurnStatus::Error,
                    Some(error.to_string()),
                );
            }
            let _ = app_handle.emit(
                "agentic://dialog-turn-failed",
                serde_json::json!({
                    "sessionId": request.session_id,
                    "turnId": request.turn_id,
                    "error": error.to_string(),
                    "errorCategory": null,
                    "errorDetail": null,
                    "subagentParentInfo": null,
                }),
            );
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn start_acp_dialog_turn(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    request: StartAcpDialogTurnRequest,
) -> Result<(), String> {
    start_acp_dialog_turn_impl(app_handle, &state, request).await
}

#[tauri::command]
pub async fn cancel_acp_dialog_turn(
    state: State<'_, AppState>,
    request: CancelAcpDialogTurnRequest,
) -> Result<(), String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    service
        .cancel_agent_session(
            &request.client_id,
            request.workspace_path,
            request.session_id,
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_acp_session_options(
    state: State<'_, AppState>,
    request: GetAcpSessionOptionsRequest,
) -> Result<AcpSessionOptions, String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    let session_storage_path = match request.workspace_path.as_deref() {
        Some(workspace_path) => Some(
            desktop_effective_session_storage_path(
                &state,
                workspace_path,
                request.remote_connection_id.as_deref(),
                request.remote_ssh_host.as_deref(),
            )
            .await,
        ),
        None => None,
    };
    service
        .get_session_options(
            &request.client_id,
            request.workspace_path,
            request.remote_connection_id,
            session_storage_path,
            request.session_id,
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_acp_session_commands(
    state: State<'_, AppState>,
    request: GetAcpSessionOptionsRequest,
) -> Result<Vec<AcpAvailableCommand>, String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    let session_storage_path = match request.workspace_path.as_deref() {
        Some(workspace_path) => Some(
            desktop_effective_session_storage_path(
                &state,
                workspace_path,
                request.remote_connection_id.as_deref(),
                request.remote_ssh_host.as_deref(),
            )
            .await,
        ),
        None => None,
    };
    service
        .get_session_commands(
            &request.client_id,
            request.workspace_path,
            request.remote_connection_id,
            session_storage_path,
            request.session_id,
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_acp_session_model(
    state: State<'_, AppState>,
    request: SetAcpSessionModelRequest,
) -> Result<AcpSessionOptions, String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    let session_storage_path = match request.workspace_path.as_deref() {
        Some(workspace_path) => Some(
            desktop_effective_session_storage_path(
                &state,
                workspace_path,
                request.remote_connection_id.as_deref(),
                request.remote_ssh_host.as_deref(),
            )
            .await,
        ),
        None => None,
    };
    service
        .set_session_model(request, session_storage_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_acp_session_config_option(
    state: State<'_, AppState>,
    request: SetAcpSessionConfigOptionRequest,
) -> Result<AcpSessionOptions, String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    let session_storage_path = match request.workspace_path.as_deref() {
        Some(workspace_path) => Some(
            desktop_effective_session_storage_path(
                &state,
                workspace_path,
                request.remote_connection_id.as_deref(),
                request.remote_ssh_host.as_deref(),
            )
            .await,
        ),
        None => None,
    };
    service
        .set_session_config_option(request, session_storage_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_acp_client(
    state: State<'_, AppState>,
    request: AcpClientIdRequest,
) -> Result<(), String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    service
        .stop_client(&request.client_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_acp_json_config(state: State<'_, AppState>) -> Result<String, String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    service.load_json_config().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_acp_json_config(
    state: State<'_, AppState>,
    json_config: String,
) -> Result<(), String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    service
        .save_json_config(&json_config)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn submit_acp_permission_response(
    state: State<'_, AppState>,
    request: SubmitAcpPermissionResponseRequest,
) -> Result<AcpClientPermissionResponse, String> {
    let service = state
        .acp_client_service
        .as_ref()
        .ok_or_else(|| "ACP client service not initialized".to_string())?;
    service
        .submit_permission_response(request)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started_event(tool_id: &str) -> ToolEventData {
        ToolEventData::Started {
            identity: bitfun_events::ToolEventIdentity::direct(tool_id, "Bash"),
            params: serde_json::json!({ "command": "echo ok" }),
            timeout_seconds: None,
        }
    }

    fn completed_event(tool_id: &str) -> ToolEventData {
        ToolEventData::Completed {
            identity: bitfun_events::ToolEventIdentity::direct(tool_id, "Bash"),
            result: serde_json::json!({ "success": true }),
            result_for_assistant: None,
            image_attachments: None,
            duration_ms: 12,
            queue_wait_ms: None,
            preflight_ms: None,
            confirmation_wait_ms: None,
            execution_ms: None,
        }
    }

    fn failed_event(tool_id: &str) -> ToolEventData {
        ToolEventData::Failed {
            identity: bitfun_events::ToolEventIdentity::direct(tool_id, "Bash"),
            error: "boom".to_string(),
            duration_ms: None,
            queue_wait_ms: None,
            preflight_ms: None,
            confirmation_wait_ms: None,
            execution_ms: None,
        }
    }

    #[test]
    fn acp_tool_event_maps_lifecycle_variants() {
        let started = acp_tool_event_to_tool_item(&started_event("tool-1"))
            .expect("started maps to an item");
        assert_eq!(started.id, "tool-1");
        assert_eq!(started.tool_name, "Bash");
        assert_eq!(started.status.as_deref(), Some("in_progress"));
        assert_eq!(started.tool_call.input["command"], "echo ok");
        assert!(started.tool_result.is_none());

        let completed = acp_tool_event_to_tool_item(&completed_event("tool-1"))
            .expect("completed maps to an item");
        assert_eq!(completed.status.as_deref(), Some("completed"));
        let result = completed.tool_result.expect("completed has a result");
        assert!(result.success);
        assert_eq!(result.duration_ms, Some(12));

        let failed = acp_tool_event_to_tool_item(&failed_event("tool-1"))
            .expect("failed maps to an item");
        assert_eq!(failed.status.as_deref(), Some("failed"));
        let result = failed.tool_result.expect("failed has a result");
        assert!(!result.success);
        assert_eq!(result.error.as_deref(), Some("boom"));

        // 信息性变体不产生落盘条目。
        assert!(acp_tool_event_to_tool_item(&ToolEventData::Progress {
            identity: bitfun_events::ToolEventIdentity::direct("tool-1", "Bash"),
            message: "working".to_string(),
            percentage: 0.5,
        })
        .is_none());
    }

    #[test]
    fn acp_tool_event_merge_keeps_started_input_and_final_status() {
        let mut accumulator = AcpDialogTurnAccumulator::default();
        accumulator.start_round("round-1".to_string(), 0);
        accumulator.apply_tool_event(&started_event("tool-1"));
        accumulator.apply_tool_event(&completed_event("tool-1"));
        accumulator.finish_current_round();

        assert_eq!(accumulator.rounds.len(), 1);
        let round = &accumulator.rounds[0];
        assert_eq!(round.tool_items.len(), 1);
        assert_eq!(round.tool_items[0].tool_call.input["command"], "echo ok");
        assert_eq!(round.tool_items[0].status.as_deref(), Some("completed"));
        assert!(round.tool_items[0].tool_result.as_ref().unwrap().success);

        // 两次不同 tool id 的事件 → 两个条目。
        accumulator.start_round("round-2".to_string(), 1);
        accumulator.apply_tool_event(&started_event("tool-2"));
        accumulator.apply_tool_event(&failed_event("tool-2"));
        accumulator.finish_current_round();
        assert_eq!(accumulator.rounds[1].tool_items.len(), 1);
        assert_eq!(accumulator.rounds[1].tool_items[0].status.as_deref(), Some("failed"));
    }

    #[test]
    fn build_acp_dialog_turn_data_builds_model_rounds() {
        let mut accumulator = AcpDialogTurnAccumulator::default();
        accumulator.start_round("round-1".to_string(), 0);
        if let Some(round) = accumulator.current_round.as_mut() {
            round.text_parts.push("hello ".to_string());
            round.text_parts.push("world".to_string());
            round.thinking_parts.push("think step".to_string());
        }
        accumulator.apply_tool_event(&started_event("tool-1"));
        accumulator.apply_tool_event(&completed_event("tool-1"));
        accumulator.finish_current_round();

        let turn = build_acp_dialog_turn_data(
            "turn-1",
            2,
            "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b",
            "hello",
            1000,
            accumulator.rounds,
            TurnStatus::Completed,
            None,
        );
        assert_eq!(turn.turn_index, 2);
        assert_eq!(turn.user_message.content, "hello");
        assert_eq!(turn.status, TurnStatus::Completed);
        assert!(turn.end_time.is_some());
        assert!(turn.error.is_none());
        assert_eq!(turn.model_rounds.len(), 1);
        let round = &turn.model_rounds[0];
        assert_eq!(round.round_index, 0);
        assert_eq!(round.text_items.len(), 1);
        assert_eq!(round.text_items[0].content, "hello world");
        assert_eq!(round.thinking_items.len(), 1);
        assert_eq!(round.thinking_items[0].content, "think step");
        assert_eq!(round.tool_items.len(), 1);
        assert_eq!(round.tool_items[0].status.as_deref(), Some("completed"));

        // Cancelled 终态：status=Cancelled + end_time，保留已累积内容。
        let cancelled = build_acp_dialog_turn_data(
            "turn-2",
            3,
            "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b",
            "hello",
            2000,
            Vec::new(),
            TurnStatus::Cancelled,
            None,
        );
        assert_eq!(cancelled.status, TurnStatus::Cancelled);
        assert!(cancelled.end_time.is_some());
        assert!(cancelled.model_rounds.is_empty());

        // Error 终态（d3-P2-1）：desktop 直通失败分支落盘 error text，
        // 与 core 直投路径（session_message_tool 失败分支落 Some(error_text)）对称。
        let failed = build_acp_dialog_turn_data(
            "turn-3",
            4,
            "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b",
            "hello",
            3000,
            Vec::new(),
            TurnStatus::Error,
            Some("ACP agent failed: boom".to_string()),
        );
        assert_eq!(failed.status, TurnStatus::Error);
        assert!(failed.end_time.is_some());
        assert_eq!(failed.error.as_deref(), Some("ACP agent failed: boom"));
    }
}
