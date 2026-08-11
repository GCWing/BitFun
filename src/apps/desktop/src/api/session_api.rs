//! Session persistence API

use crate::api::app_state::AppState;
use crate::runtime::{
    DesktopRuntimeContext, DesktopSessionApplicationError, DesktopSessionScopeRequest,
    UiSessionMetadataField,
};
use crate::startup_trace::DesktopStartupTrace;
use bitfun_agent_runtime::sdk::AgentSessionLineageSnapshot;
use bitfun_core::agentic::coordination::get_global_scheduler;
use bitfun_core::agentic::persistence::{SessionBranchResult, SessionMetadataPage};
use bitfun_core::service::remote_ssh::normalize_remote_workspace_path;
use bitfun_core::service::session::{
    DialogTurnData, SessionKind, SessionMetadata, SessionStatus, SessionTranscriptExport,
    SessionTranscriptExportOptions, SessionTurnCatalog,
};
use bitfun_core::service::session_usage::SessionUsageReport;
use bitfun_core::service::workspace::WorkspaceKind;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tauri::State;

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

fn desktop_session_error(error: DesktopSessionApplicationError) -> String {
    error.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPersistedSessionsRequest {
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    /// When true, hidden Subagent/Ephemeral sessions are included in the
    /// result (full conversation management).
    #[serde(default)]
    pub include_hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPersistedSessionsPageRequest {
    pub workspace_path: String,
    pub limit: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    /// When true, hidden Subagent/Ephemeral sessions are included in the page
    /// (full conversation management).
    #[serde(default)]
    pub include_hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionLineageRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadSessionTurnsRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSessionTurnRequest {
    pub turn_data: DialogTurnData,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordLocalCommandTurnRequest {
    pub turn_data: DialogTurnData,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordLocalCommandTurnResponse {
    pub turn_id: String,
    pub storage_turn_index: usize,
    pub total_turn_count: usize,
    pub turn_catalog: SessionTurnCatalog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSessionMetadataRequest {
    pub metadata: SessionMetadata,
    pub fields: Vec<UiSessionMetadataField>,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSessionTranscriptRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    #[serde(default = "default_tools")]
    pub tools: bool,
    #[serde(default)]
    pub tool_inputs: bool,
    #[serde(default)]
    pub thinking: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turns: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchReferenceableSessionsRequest {
    pub query: String,
    #[serde(default = "default_session_reference_search_limit")]
    pub limit: usize,
}

fn default_session_reference_search_limit() -> usize {
    30
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionReferenceCandidate {
    pub session_id: String,
    pub session_name: String,
    pub workspace_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    pub workspace_label: String,
    pub last_activity_at: u64,
}

fn default_tools() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletePersistedSessionRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchSessionActivityRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadPersistedSessionMetadataRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionUsageReportRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    #[serde(default = "default_include_hidden_subagents")]
    pub include_hidden_subagents: bool,
}

fn default_include_hidden_subagents() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkSessionRequest {
    pub source_session_id: String,
    pub source_turn_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

pub type ForkSessionResponse = SessionBranchResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveSessionRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnarchiveSessionRequest {
    pub session_id: String,
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveAllSessionsRequest {
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteAllArchivedSessionsRequest {
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[tauri::command]
pub async fn list_persisted_sessions(
    request: ListPersistedSessionsRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<Vec<SessionMetadata>, String> {
    runtime
        .session_application()
        .list_persisted_sessions_with_options(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            request.include_hidden,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to list persisted sessions: {}",
                desktop_session_error(error)
            )
        })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListDeletedSessionIdsRequest {
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

/// List session ids recorded in the workspace deletion tombstone registry.
/// The frontend initialization path pulls this registry to guard against
/// ghost resurrection of deleted subagent sessions after a restart.
#[tauri::command]
pub async fn list_deleted_session_ids(
    request: ListDeletedSessionIdsRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<Vec<String>, String> {
    runtime
        .session_application()
        .list_deleted_session_ids(desktop_session_scope(
            request.workspace_path,
            request.remote_connection_id,
            request.remote_ssh_host,
        ))
        .await
        .map_err(|error| {
            format!(
                "Failed to list deleted session ids: {}",
                desktop_session_error(error)
            )
        })
}

/// Search lightweight persisted metadata across open local and SSH
/// workspaces. This deliberately never loads dialog turns or generates a
/// transcript; that work happens only when the selected message is dispatched.
#[tauri::command]
pub async fn search_referenceable_sessions(
    request: SearchReferenceableSessionsRequest,
    runtime: State<'_, DesktopRuntimeContext>,
    app_state: State<'_, AppState>,
) -> Result<Vec<SessionReferenceCandidate>, String> {
    let query = request.query.trim().to_lowercase();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let limit = request.limit.clamp(1, 30);
    let scheduler = get_global_scheduler();
    let mut workspaces = app_state.workspace_service.get_opened_workspaces().await;
    workspaces.sort_by_key(|workspace| std::cmp::Reverse(workspace.last_accessed));

    let mut candidates = Vec::new();
    for workspace in workspaces {
        let remote_connection_id = workspace.remote_ssh_connection_id().map(ToOwned::to_owned);
        let remote_ssh_host = workspace
            .metadata
            .get("sshHost")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let workspace_path = if workspace.workspace_kind == WorkspaceKind::Remote {
            normalize_remote_workspace_path(&workspace.root_path.to_string_lossy())
        } else {
            workspace.root_path.to_string_lossy().to_string()
        };
        let metadata = runtime
            .session_application()
            .list_persisted_sessions(desktop_session_scope(
                workspace_path.clone(),
                remote_connection_id.clone(),
                remote_ssh_host.clone(),
            ))
            .await
            .map_err(|error| {
                format!(
                    "Failed to list sessions for workspace {}: {}",
                    workspace.name,
                    desktop_session_error(error)
                )
            })?;

        for session in metadata {
            if session.status == SessionStatus::Archived
                || !matches!(session.session_kind, SessionKind::Standard)
                || scheduler.as_ref().is_some_and(|scheduler| {
                    scheduler.is_session_busy_or_queued(&session.session_id)
                })
                || !session.session_name.to_lowercase().contains(&query)
            {
                continue;
            }
            candidates.push(SessionReferenceCandidate {
                session_id: session.session_id,
                session_name: session.session_name,
                workspace_path: workspace_path.clone(),
                remote_connection_id: remote_connection_id.clone(),
                remote_ssh_host: remote_ssh_host.clone(),
                workspace_label: workspace.name.clone(),
                last_activity_at: session.last_active_at,
            });
        }
    }

    candidates.sort_by_key(|right| std::cmp::Reverse(right.last_activity_at));
    candidates.truncate(limit);
    Ok(candidates)
}

#[tauri::command]
pub async fn list_persisted_sessions_page(
    request: ListPersistedSessionsPageRequest,
    runtime: State<'_, DesktopRuntimeContext>,
    startup_trace: State<'_, DesktopStartupTrace>,
) -> Result<SessionMetadataPage, String> {
    let trace_started = Instant::now();
    let result = runtime
        .session_application()
        .list_persisted_sessions_page_with_options(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            request.cursor.as_deref(),
            request.limit,
            request.include_hidden,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to list persisted session page: {}",
                desktop_session_error(error)
            )
        });
    startup_trace.record_tauri_command_elapsed("list_persisted_sessions_page", None, trace_started);
    result
}

#[tauri::command]
pub async fn get_session_lineage(
    request: GetSessionLineageRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<Option<AgentSessionLineageSnapshot>, String> {
    runtime
        .session_application()
        .get_session_lineage(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            &request.session_id,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to load session lineage: {}",
                desktop_session_error(error)
            )
        })
}

#[tauri::command]
pub async fn load_session_turns(
    request: LoadSessionTurnsRequest,
    runtime: State<'_, DesktopRuntimeContext>,
    startup_trace: State<'_, DesktopStartupTrace>,
) -> Result<Vec<DialogTurnData>, String> {
    let trace_started = Instant::now();
    let trace_target = if request.limit.is_some() {
        "recent"
    } else {
        "full"
    };
    let result = runtime
        .session_application()
        .load_session_turns(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            &request.session_id,
            request.limit,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to load session turns: {}",
                desktop_session_error(error)
            )
        });
    startup_trace.record_tauri_command_elapsed(
        "load_session_turns",
        Some(trace_target),
        trace_started,
    );
    result
}

#[tauri::command]
pub async fn get_session_usage_report(
    request: GetSessionUsageReportRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<SessionUsageReport, String> {
    runtime
        .session_application()
        .generate_usage_report(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            request.session_id,
            request.include_hidden_subagents,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to generate session usage report: {}",
                desktop_session_error(error)
            )
        })
}

#[tauri::command]
pub async fn save_session_turn(
    request: SaveSessionTurnRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<(), String> {
    runtime
        .session_application()
        .save_session_turn(
            desktop_session_scope(
                request.workspace_path.clone(),
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            &request.turn_data,
        )
        .await
        .map_err(|error| format!("Failed to save session turn: {error}"))?;

    // Notify the auto-sync background task (debounced upload to relay)
    crate::api::remote_connect_api::notify_session_changed(
        &request.turn_data.session_id,
        &request.workspace_path,
    );
    Ok(())
}

#[tauri::command]
pub async fn record_local_command_turn(
    request: RecordLocalCommandTurnRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<RecordLocalCommandTurnResponse, String> {
    let recorded = runtime
        .session_application()
        .record_local_command_turn(
            desktop_session_scope(
                request.workspace_path.clone(),
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            &request.turn_data,
        )
        .await
        .map_err(|error| format!("Failed to record local command turn: {error}"))?;

    crate::api::remote_connect_api::notify_session_changed(
        &request.turn_data.session_id,
        &request.workspace_path,
    );
    Ok(RecordLocalCommandTurnResponse {
        turn_id: recorded.turn_id,
        storage_turn_index: recorded.storage_turn_index,
        total_turn_count: recorded.total_turn_count,
        turn_catalog: recorded.turn_catalog,
    })
}

#[tauri::command]
pub async fn save_session_metadata(
    request: SaveSessionMetadataRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<(), String> {
    runtime
        .session_application()
        .save_ui_metadata(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            request.metadata,
            request.fields,
        )
        .await
        .map_err(|error| match error {
            DesktopSessionApplicationError::Validation(message) => message,
            error => format!(
                "Failed to save session metadata: {}",
                desktop_session_error(error)
            ),
        })
}

#[tauri::command]
pub async fn export_session_transcript(
    request: ExportSessionTranscriptRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<SessionTranscriptExport, String> {
    runtime
        .session_application()
        .export_session_transcript(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            &request.session_id,
            &SessionTranscriptExportOptions {
                tools: request.tools,
                tool_inputs: request.tool_inputs,
                thinking: request.thinking,
                turns: request.turns,
            },
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to export session transcript: {}",
                desktop_session_error(error)
            )
        })
}

#[tauri::command]
pub async fn delete_persisted_session(
    request: DeletePersistedSessionRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<(), String> {
    // 单会话删除（L4-P2-E 确认合理）：归档会话按定义是顶层（archived
    // 会话不可运行、无活跃子树），单会话 delete_session 足够，无需
    // delete_session_tree 级联。前端 ArchivedSessionsConfig 删除单条归档
    // 走此命令；后端 tombstone 落盘 + 列表过滤兜底防重启复活。
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
        .map_err(|error| {
            format!(
                "Failed to delete persisted session: {}",
                desktop_session_error(error)
            )
        })
}

#[tauri::command]
pub async fn touch_session_activity(
    request: TouchSessionActivityRequest,
    runtime: State<'_, DesktopRuntimeContext>,
    startup_trace: State<'_, DesktopStartupTrace>,
) -> Result<(), String> {
    let trace_started = Instant::now();
    let result = runtime
        .session_application()
        .touch_session(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            &request.session_id,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to update session activity: {}",
                desktop_session_error(error)
            )
        });
    startup_trace.record_tauri_command_elapsed("touch_session_activity", None, trace_started);
    result
}

#[tauri::command]
pub async fn load_persisted_session_metadata(
    request: LoadPersistedSessionMetadataRequest,
    runtime: State<'_, DesktopRuntimeContext>,
    startup_trace: State<'_, DesktopStartupTrace>,
) -> Result<Option<SessionMetadata>, String> {
    let trace_started = Instant::now();
    // Direct metadata lookups are used by persistence flows that must be able
    // to read hidden subagent sessions without list-level visibility filtering.
    let result = runtime
        .session_application()
        .load_session_metadata(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            &request.session_id,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to load persisted session metadata: {}",
                desktop_session_error(error)
            )
        });
    startup_trace.record_tauri_command_elapsed(
        "load_persisted_session_metadata",
        None,
        trace_started,
    );
    result
}

#[tauri::command]
pub async fn fork_session(
    request: ForkSessionRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<ForkSessionResponse, String> {
    runtime
        .session_application()
        .fork_session(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            request.source_session_id,
            request.source_turn_id,
        )
        .await
        .map_err(|error| format!("Failed to fork session: {}", desktop_session_error(error)))
}

#[tauri::command]
pub async fn archive_session(
    request: ArchiveSessionRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<(), String> {
    runtime
        .session_application()
        .set_session_archived(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            request.session_id,
            true,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to save session metadata: {}",
                desktop_session_error(error)
            )
        })
}

#[tauri::command]
pub async fn unarchive_session(
    request: UnarchiveSessionRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<(), String> {
    runtime
        .session_application()
        .set_session_archived(
            desktop_session_scope(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            ),
            request.session_id,
            false,
        )
        .await
        .map_err(|error| {
            format!(
                "Failed to save session metadata: {}",
                desktop_session_error(error)
            )
        })
}

#[tauri::command]
pub async fn archive_all_sessions(
    request: ArchiveAllSessionsRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<u32, String> {
    let scope = desktop_session_scope(
        request.workspace_path,
        request.remote_connection_id,
        request.remote_ssh_host,
    );
    let sessions = runtime
        .session_application()
        .list_persisted_sessions(scope.clone())
        .await
        .map_err(|error| format!("Failed to list sessions: {}", desktop_session_error(error)))?;

    let mut archived_count: u32 = 0;

    for metadata in sessions {
        if metadata.status != SessionStatus::Archived
            && metadata.session_kind == SessionKind::Standard
        {
            runtime
                .session_application()
                .set_session_archived(scope.clone(), metadata.session_id, true)
                .await
                .map_err(|error| {
                    format!(
                        "Failed to save session metadata: {}",
                        desktop_session_error(error)
                    )
                })?;
            archived_count += 1;
        }
    }

    Ok(archived_count)
}

#[tauri::command]
pub async fn list_archived_sessions(
    request: ListPersistedSessionsRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<Vec<SessionMetadata>, String> {
    runtime
        .session_application()
        .list_archived_sessions(desktop_session_scope(
            request.workspace_path,
            request.remote_connection_id,
            request.remote_ssh_host,
        ))
        .await
        .map_err(|error| format!("Failed to list sessions: {}", desktop_session_error(error)))
}

#[tauri::command]
pub async fn delete_all_archived_sessions(
    request: DeleteAllArchivedSessionsRequest,
    runtime: State<'_, DesktopRuntimeContext>,
) -> Result<u32, String> {
    let scope = desktop_session_scope(
        request.workspace_path,
        request.remote_connection_id,
        request.remote_ssh_host,
    );
    let sessions = runtime
        .session_application()
        .list_archived_sessions(scope.clone())
        .await
        .map_err(|error| format!("Failed to list sessions: {}", desktop_session_error(error)))?;

    let mut deleted_count: u32 = 0;

    for metadata in sessions {
        // 归档会话按定义无活跃子树（L4-P2-E），逐个单会话删除而非
        // delete_session_tree 级联；任一删除失败即中止（全有或全无语义）。
        runtime
            .session_application()
            .delete_session(scope.clone(), metadata.session_id)
            .await
            .map_err(|error| {
                format!("Failed to delete session: {}", desktop_session_error(error))
            })?;
        deleted_count += 1;
    }

    Ok(deleted_count)
}

// ---------------------------------------------------------------------------
// Group chat commands (R-GC-12, P2-1: 11 commands unified naming)
// ---------------------------------------------------------------------------

use bitfun_core::agentic::session::session_store_port::CoreSessionStorePort;
use bitfun_core::service::session::GroupChatStore;
use bitfun_runtime_ports::{
    GroupChatActor, GroupChatMember, GroupChatMessage, GroupChatMessagesResponse, GroupChatMode,
    GroupChatRoom, GroupChatSendResult, SessionStoragePathRequest, SessionStorePort,
};

/// Resolves the group-chats root (sibling of the sessions root) for a workspace.
async fn group_chats_root(workspace_path: &str) -> Result<std::path::PathBuf, String> {
    let request = SessionStoragePathRequest {
        workspace_path: std::path::PathBuf::from(workspace_path),
        remote_connection_id: None,
        remote_ssh_host: None,
    };
    let resolution = CoreSessionStorePort::default()
        .resolve_session_storage_path(request)
        .await
        .map_err(|error| format!("Failed to resolve sessions root: {error}"))?;
    let sessions_root = resolution.effective_storage_path;
    let parent = sessions_root
        .parent()
        .ok_or_else(|| "sessions root has no parent directory".to_string())?;
    Ok(parent.join("group-chats"))
}

async fn group_chat_store(workspace_path: &str) -> Result<GroupChatStore, String> {
    let root = group_chats_root(workspace_path).await?;
    Ok(GroupChatStore::new(root))
}

fn group_chat_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[tauri::command]
pub async fn group_chat_list(workspace_path: String) -> Result<Vec<GroupChatRoom>, String> {
    let store = group_chat_store(&workspace_path).await?;
    let (rooms, _) = store
        .list_rooms()
        .await
        .map_err(|error| error.to_string())?;
    Ok(rooms)
}

#[tauri::command]
pub async fn group_chat_load(
    workspace_path: String,
    room_id: String,
) -> Result<GroupChatRoom, String> {
    let store = group_chat_store(&workspace_path).await?;
    store
        .load_room(&room_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn group_chat_members(
    workspace_path: String,
    room_id: String,
) -> Result<Vec<GroupChatMember>, String> {
    let store = group_chat_store(&workspace_path).await?;
    store
        .list_members(&room_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn group_chat_create(
    workspace_path: String,
    name: String,
    owner: GroupChatActor,
    members: Vec<String>,
    mode: Option<GroupChatMode>,
) -> Result<GroupChatRoom, String> {
    let store = group_chat_store(&workspace_path).await?;
    let (rooms, _) = store
        .list_rooms()
        .await
        .map_err(|error| error.to_string())?;
    if rooms.iter().any(|room| room.name == name) {
        return Err(format!("group chat name '{name}' already exists"));
    }
    let now = group_chat_unix_ms();
    let room_id = format!("group-{}", uuid_slug(&name));
    let members_list: Vec<GroupChatMember> = members
        .iter()
        .enumerate()
        .map(|(index, session_id)| GroupChatMember {
            session_id: session_id.clone(),
            role: if index == 0 {
                bitfun_runtime_ports::GroupChatMemberRole::Owner
            } else {
                bitfun_runtime_ports::GroupChatMemberRole::Member
            },
            joined_at: now,
            agent_type: "Claw".to_string(),
            display_name: None,
        })
        .collect();
    let room = GroupChatRoom {
        schema_version: 1,
        room_id: room_id.clone(),
        name,
        owner,
        mode: mode.unwrap_or(GroupChatMode::Free),
        round_robin_cursor: 0,
        created_at: now,
        last_active_at: now,
        status: bitfun_runtime_ports::GroupChatStatus::Active,
        member_limit: 50,
        members: Vec::new(),
    };
    store
        .save_room(&room)
        .await
        .map_err(|error| error.to_string())?;
    store
        .save_members(&room_id, &members_list)
        .await
        .map_err(|error| error.to_string())?;
    Ok(room)
}

#[tauri::command]
pub async fn group_chat_join(
    workspace_path: String,
    room_id: String,
    session_id: String,
    actor: GroupChatActor,
) -> Result<GroupChatRoom, String> {
    let store = group_chat_store(&workspace_path).await?;
    let mut room = store
        .load_room(&room_id)
        .await
        .map_err(|error| error.to_string())?;
    if room
        .members
        .iter()
        .any(|member| member.session_id == session_id)
    {
        return Err(format!("session '{session_id}' is already a member"));
    }
    let is_owner_or_master = match (&room.owner, &actor) {
        (
            GroupChatActor::Claw {
                session_id: owner_id,
                ..
            },
            GroupChatActor::Claw {
                session_id: actor_id,
                ..
            },
        ) => owner_id == actor_id,
        _ => matches!(actor, GroupChatActor::Master),
    };
    if !is_owner_or_master {
        return Err("only the owner or the master can add members".to_string());
    }
    let now = group_chat_unix_ms();
    let mut members = room.members.clone();
    members.push(GroupChatMember {
        session_id: session_id.clone(),
        role: bitfun_runtime_ports::GroupChatMemberRole::Member,
        joined_at: now,
        agent_type: "Claw".to_string(),
        display_name: None,
    });
    store
        .save_members(&room_id, &members)
        .await
        .map_err(|error| error.to_string())?;
    room.members = members;
    Ok(room)
}

#[tauri::command]
pub async fn group_chat_leave(
    workspace_path: String,
    room_id: String,
    session_id: String,
    actor: GroupChatActor,
) -> Result<GroupChatRoom, String> {
    let store = group_chat_store(&workspace_path).await?;
    let room = store
        .load_room(&room_id)
        .await
        .map_err(|error| error.to_string())?;
    let can_leave = matches!(actor, GroupChatActor::Master)
        || match (&room.owner, &actor) {
            (
                GroupChatActor::Claw {
                    session_id: owner_id,
                    ..
                },
                GroupChatActor::Claw {
                    session_id: actor_id,
                    ..
                },
            ) => owner_id == actor_id || actor_id == &session_id,
            _ => match &actor {
                GroupChatActor::Claw {
                    session_id: claw, ..
                } => claw == &session_id,
                _ => false,
            },
        };
    if !can_leave {
        return Err("only the owner, the master, or the member itself can leave".to_string());
    }
    let members: Vec<GroupChatMember> = room
        .members
        .iter()
        .filter(|member| member.session_id != session_id)
        .cloned()
        .collect();
    store
        .save_members(&room_id, &members)
        .await
        .map_err(|error| error.to_string())?;
    let mut updated = room;
    updated.members = members;
    Ok(updated)
}

#[tauri::command]
pub async fn group_chat_delete(
    workspace_path: String,
    room_id: String,
    _actor: GroupChatActor,
) -> Result<(), String> {
    let store = group_chat_store(&workspace_path).await?;
    store
        .delete_room(&room_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn group_chat_set_mode(
    workspace_path: String,
    room_id: String,
    mode: GroupChatMode,
    _actor: GroupChatActor,
) -> Result<GroupChatRoom, String> {
    let store = group_chat_store(&workspace_path).await?;
    let mut room = store
        .load_room(&room_id)
        .await
        .map_err(|error| error.to_string())?;
    room.mode = mode;
    room.round_robin_cursor = 0; // 模式切换时 reset cursor (R-GC-10)
    store
        .save_room(&room)
        .await
        .map_err(|error| error.to_string())?;
    Ok(room)
}

#[tauri::command]
pub async fn group_chat_send(
    workspace_path: String,
    room_id: String,
    author: GroupChatActor,
    content: String,
    mention_targets: Vec<GroupChatActor>,
    urgent: bool,
) -> Result<GroupChatSendResult, String> {
    let store = group_chat_store(&workspace_path).await?;
    let room = store
        .load_room(&room_id)
        .await
        .map_err(|error| error.to_string())?;
    if room.members.is_empty() {
        return Err("group has no members".to_string());
    }
    let now = group_chat_unix_ms();
    let message_id = format!("msg-{}", uuid_slug(&format!("{room_id}-{content}-{now}")));
    let message = GroupChatMessage {
        message_id: message_id.clone(),
        room_id: room_id.clone(),
        author: author.clone(),
        kind: match &author {
            GroupChatActor::Master => bitfun_runtime_ports::GroupChatMessageKind::User,
            GroupChatActor::Claw { .. } => bitfun_runtime_ports::GroupChatMessageKind::Agent,
            GroupChatActor::All => bitfun_runtime_ports::GroupChatMessageKind::System,
        },
        content,
        mention_targets: mention_targets.clone(),
        reply_to_message_id: None,
        timestamp: now,
        status: bitfun_runtime_ports::GroupChatMessageStatus::Pending,
    };
    store
        .append_message(&room_id, &message)
        .await
        .map_err(|error| error.to_string())?;
    let delivered_to: Vec<String> = room
        .members
        .iter()
        .map(|member| member.session_id.clone())
        .collect();
    if !delivered_to.is_empty() {
        store
            .update_message_status(
                &room_id,
                &message_id,
                bitfun_runtime_ports::GroupChatMessageStatus::Delivered,
            )
            .await
            .map_err(|error| error.to_string())?;
    }
    let _ = urgent;
    Ok(GroupChatSendResult {
        message_id,
        delivered_to,
        failed_to: Vec::new(),
    })
}

#[tauri::command]
pub async fn group_chat_messages(
    workspace_path: String,
    room_id: String,
    limit: Option<usize>,
    cursor: Option<usize>,
) -> Result<GroupChatMessagesResponse, String> {
    let store = group_chat_store(&workspace_path).await?;
    let window = store
        .list_messages(&room_id, limit, cursor)
        .await
        .map_err(|error| error.to_string())?;
    Ok(GroupChatMessagesResponse {
        messages: window.messages,
        next_cursor: window.next_cursor.map(|index| index.to_string()),
    })
}

#[tauri::command]
pub async fn group_chat_ingest_reply(
    workspace_path: String,
    room_id: String,
    message_id: String,
    _reply_content: String,
    _author: GroupChatActor,
    _timestamp: i64,
) -> Result<(), String> {
    let store = group_chat_store(&workspace_path).await?;
    store
        .update_message_status(
            &room_id,
            &message_id,
            bitfun_runtime_ports::GroupChatMessageStatus::Replied,
        )
        .await
        .map_err(|error| error.to_string())
}

/// P1-1 修复：超时提醒消费端——扫描全部房间的 Pending/Delivered 超时消息
/// （消费 group_chat.reply_timeout_secs，R-GC-26），返回超时提醒列表。
#[tauri::command]
pub async fn group_chat_scan_timeouts(
    workspace_path: String,
    reply_timeout_secs: u64,
) -> Result<Vec<serde_json::Value>, String> {
    let store = group_chat_store(&workspace_path).await?;
    let (rooms, _) = store
        .list_rooms()
        .await
        .map_err(|error| error.to_string())?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let mut reminders = Vec::new();
    for room in rooms {
        let timed_out = store
            .scan_timed_out_messages(&room.room_id, reply_timeout_secs, now)
            .await
            .map_err(|error| error.to_string())?;
        for message in timed_out {
            reminders.push(serde_json::json!({
                "roomId": room.room_id,
                "messageId": message.message_id,
                "content": message.content,
                "status": "failed",
            }));
        }
    }
    Ok(reminders)
}

fn uuid_slug(seed: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"bitfun-group-chat-v1\0");
    hasher.update(seed.as_bytes());
    let digest = hasher.finalize();
    digest
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
