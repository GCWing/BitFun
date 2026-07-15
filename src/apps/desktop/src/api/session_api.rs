//! Session persistence API

use crate::api::app_state::AppState;
use crate::api::session_storage_path::desktop_effective_session_storage_path;
use crate::startup_trace::DesktopStartupTrace;
use bitfun_core::agentic::persistence::{
    PersistenceManager, SessionBranchRequest, SessionBranchResult, SessionMetadataPage,
};
use bitfun_core::infrastructure::PathManager;
use bitfun_core::service::session::{
    DialogTurnData, SessionKind, SessionMetadata, SessionStatus, SessionTranscriptExport,
    SessionTranscriptExportOptions,
};
use bitfun_core::service::session_usage::{
    generate_session_usage_report, SessionUsageReport, SessionUsageReportRequest,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPersistedSessionsRequest {
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
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
pub struct SaveSessionMetadataRequest {
    pub metadata: SessionMetadata,
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
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<Vec<SessionMetadata>, String> {
    let workspace_path = desktop_effective_session_storage_path(
        &app_state,
        &request.workspace_path,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

    manager
        .list_session_metadata(&workspace_path)
        .await
        .map_err(|e| format!("Failed to list persisted sessions: {}", e))
}

#[tauri::command]
pub async fn list_persisted_sessions_page(
    request: ListPersistedSessionsPageRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
    startup_trace: State<'_, DesktopStartupTrace>,
) -> Result<SessionMetadataPage, String> {
    let trace_started = Instant::now();
    let result = async {
        let workspace_path = desktop_effective_session_storage_path(
            &app_state,
            &request.workspace_path,
            request.remote_connection_id.as_deref(),
            request.remote_ssh_host.as_deref(),
        )
        .await;
        let manager = PersistenceManager::new(path_manager.inner().clone())
            .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

        manager
            .list_session_metadata_page(&workspace_path, request.cursor.as_deref(), request.limit)
            .await
            .map_err(|e| format!("Failed to list persisted session page: {}", e))
    }
    .await;
    startup_trace.record_tauri_command_elapsed("list_persisted_sessions_page", None, trace_started);
    result
}

#[tauri::command]
pub async fn load_session_turns(
    request: LoadSessionTurnsRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
    startup_trace: State<'_, DesktopStartupTrace>,
) -> Result<Vec<DialogTurnData>, String> {
    let trace_started = Instant::now();
    let trace_target = if request.limit.is_some() {
        "recent"
    } else {
        "full"
    };
    let result = async {
        let workspace_path = desktop_effective_session_storage_path(
            &app_state,
            &request.workspace_path,
            request.remote_connection_id.as_deref(),
            request.remote_ssh_host.as_deref(),
        )
        .await;
        let manager = PersistenceManager::new(path_manager.inner().clone())
            .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

        let turns = if let Some(limit) = request.limit {
            manager
                .load_recent_turns(&workspace_path, &request.session_id, limit)
                .await
        } else {
            manager
                .load_session_turns(&workspace_path, &request.session_id)
                .await
        };

        turns.map_err(|e| format!("Failed to load session turns: {}", e))
    }
    .await;
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
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<SessionUsageReport, String> {
    let storage_workspace_path = desktop_effective_session_storage_path(
        &app_state,
        &request.workspace_path,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

    let mut report = generate_session_usage_report(
        &manager,
        Some(app_state.token_usage_service.as_ref()),
        SessionUsageReportRequest {
            session_id: request.session_id,
            workspace_path: Some(storage_workspace_path.to_string_lossy().to_string()),
            remote_connection_id: request.remote_connection_id.clone(),
            remote_ssh_host: request.remote_ssh_host.clone(),
            include_hidden_subagents: request.include_hidden_subagents,
        },
    )
    .await
    .map_err(|e| format!("Failed to generate session usage report: {}", e))?;

    report.workspace.path_label = Some(request.workspace_path);
    report.workspace.remote_connection_id = request.remote_connection_id;
    report.workspace.remote_ssh_host = request.remote_ssh_host;

    Ok(report)
}

#[tauri::command]
pub async fn save_session_turn(
    request: SaveSessionTurnRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<(), String> {
    let workspace_path = desktop_effective_session_storage_path(
        &app_state,
        &request.workspace_path,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

    manager
        .save_dialog_turn(&workspace_path, &request.turn_data)
        .await
        .map_err(|e| format!("Failed to save session turn: {}", e))?;

    // Notify the auto-sync background task (debounced upload to relay)
    crate::api::remote_connect_api::notify_session_changed(
        &request.turn_data.session_id,
        &request.workspace_path,
    );
    Ok(())
}

#[tauri::command]
pub async fn save_session_metadata(
    request: SaveSessionMetadataRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<(), String> {
    let workspace_path = desktop_effective_session_storage_path(
        &app_state,
        &request.workspace_path,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

    manager
        .save_session_metadata(&workspace_path, &request.metadata)
        .await
        .map_err(|e| format!("Failed to save session metadata: {}", e))?;

    // Notify the auto-sync background task
    crate::api::remote_connect_api::notify_session_changed(
        &request.metadata.session_id,
        &request.workspace_path,
    );
    Ok(())
}

#[tauri::command]
pub async fn export_session_transcript(
    request: ExportSessionTranscriptRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<SessionTranscriptExport, String> {
    let workspace_path = desktop_effective_session_storage_path(
        &app_state,
        &request.workspace_path,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

    manager
        .export_session_transcript(
            &workspace_path,
            &request.session_id,
            &SessionTranscriptExportOptions {
                tools: request.tools,
                tool_inputs: request.tool_inputs,
                thinking: request.thinking,
                turns: request.turns,
            },
        )
        .await
        .map_err(|e| format!("Failed to export session transcript: {}", e))
}

#[tauri::command]
pub async fn delete_persisted_session(
    request: DeletePersistedSessionRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<(), String> {
    let workspace_path = desktop_effective_session_storage_path(
        &app_state,
        &request.workspace_path,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

    manager
        .delete_session(&workspace_path, &request.session_id)
        .await
        .map_err(|e| format!("Failed to delete persisted session: {}", e))?;

    // Notify auto-sync: tombstone this session on the relay
    crate::api::remote_connect_api::notify_session_deleted(&request.session_id);
    Ok(())
}

#[tauri::command]
pub async fn touch_session_activity(
    request: TouchSessionActivityRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
    startup_trace: State<'_, DesktopStartupTrace>,
) -> Result<(), String> {
    let trace_started = Instant::now();
    let result = async {
        let workspace_path = desktop_effective_session_storage_path(
            &app_state,
            &request.workspace_path,
            request.remote_connection_id.as_deref(),
            request.remote_ssh_host.as_deref(),
        )
        .await;
        let manager = PersistenceManager::new(path_manager.inner().clone())
            .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

        manager
            .touch_session(&workspace_path, &request.session_id)
            .await
            .map_err(|e| format!("Failed to update session activity: {}", e))
    }
    .await;
    startup_trace.record_tauri_command_elapsed("touch_session_activity", None, trace_started);
    result
}

#[tauri::command]
pub async fn load_persisted_session_metadata(
    request: LoadPersistedSessionMetadataRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
    startup_trace: State<'_, DesktopStartupTrace>,
) -> Result<Option<SessionMetadata>, String> {
    let trace_started = Instant::now();
    let result = async {
        let workspace_path = desktop_effective_session_storage_path(
            &app_state,
            &request.workspace_path,
            request.remote_connection_id.as_deref(),
            request.remote_ssh_host.as_deref(),
        )
        .await;
        let manager = PersistenceManager::new(path_manager.inner().clone())
            .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

        let metadata = manager
            .load_session_metadata(&workspace_path, &request.session_id)
            .await
            .map_err(|e| format!("Failed to load persisted session metadata: {}", e))?;

        // Direct metadata lookups are used by persistence flows that must be able
        // to read hidden subagent sessions without list-level visibility filtering.
        Ok(metadata)
    }
    .await;
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
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<ForkSessionResponse, String> {
    let workspace_path = desktop_effective_session_storage_path(
        &app_state,
        &request.workspace_path,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

    manager
        .branch_session(
            &workspace_path,
            &SessionBranchRequest {
                source_session_id: request.source_session_id,
                source_turn_id: request.source_turn_id,
            },
        )
        .await
        .map_err(|e| format!("Failed to fork session: {}", e))
}

#[tauri::command]
pub async fn archive_session(
    request: ArchiveSessionRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<(), String> {
    let workspace_path = desktop_effective_session_storage_path(
        &app_state,
        &request.workspace_path,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

    let mut metadata = manager
        .load_session_metadata(&workspace_path, &request.session_id)
        .await
        .map_err(|e| format!("Failed to load session metadata: {}", e))?
        .ok_or_else(|| "Session not found".to_string())?;

    metadata.status = SessionStatus::Archived;

    manager
        .save_session_metadata(&workspace_path, &metadata)
        .await
        .map_err(|e| format!("Failed to save session metadata: {}", e))
}

#[tauri::command]
pub async fn unarchive_session(
    request: UnarchiveSessionRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<(), String> {
    let workspace_path = desktop_effective_session_storage_path(
        &app_state,
        &request.workspace_path,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

    let mut metadata = manager
        .load_session_metadata(&workspace_path, &request.session_id)
        .await
        .map_err(|e| format!("Failed to load session metadata: {}", e))?
        .ok_or_else(|| "Session not found".to_string())?;

    metadata.status = SessionStatus::Active;

    manager
        .save_session_metadata(&workspace_path, &metadata)
        .await
        .map_err(|e| format!("Failed to save session metadata: {}", e))
}

#[tauri::command]
pub async fn archive_all_sessions(
    request: ArchiveAllSessionsRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<u32, String> {
    let workspace_path = desktop_effective_session_storage_path(
        &app_state,
        &request.workspace_path,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

    let sessions = manager
        .list_session_metadata(&workspace_path)
        .await
        .map_err(|e| format!("Failed to list sessions: {}", e))?;

    let mut archived_count: u32 = 0;

    for mut metadata in sessions {
        if metadata.status != SessionStatus::Archived
            && metadata.session_kind == SessionKind::Standard
        {
            metadata.status = SessionStatus::Archived;
            manager
                .save_session_metadata(&workspace_path, &metadata)
                .await
                .map_err(|e| format!("Failed to save session metadata: {}", e))?;
            archived_count += 1;
        }
    }

    Ok(archived_count)
}

#[tauri::command]
pub async fn list_archived_sessions(
    request: ListPersistedSessionsRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<Vec<SessionMetadata>, String> {
    let workspace_path = desktop_effective_session_storage_path(
        &app_state,
        &request.workspace_path,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

    let sessions = manager
        .list_session_metadata(&workspace_path)
        .await
        .map_err(|e| format!("Failed to list sessions: {}", e))?;

    let archived: Vec<SessionMetadata> = sessions
        .into_iter()
        .filter(|s| s.status == SessionStatus::Archived)
        .collect();

    Ok(archived)
}

#[tauri::command]
pub async fn delete_all_archived_sessions(
    request: DeleteAllArchivedSessionsRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<u32, String> {
    let workspace_path = desktop_effective_session_storage_path(
        &app_state,
        &request.workspace_path,
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

    let sessions = manager
        .list_session_metadata(&workspace_path)
        .await
        .map_err(|e| format!("Failed to list sessions: {}", e))?;

    let mut deleted_count: u32 = 0;

    for metadata in sessions {
        if metadata.status == SessionStatus::Archived {
            manager
                .delete_session(&workspace_path, &metadata.session_id)
                .await
                .map_err(|e| format!("Failed to delete session: {}", e))?;
            deleted_count += 1;
        }
    }

    Ok(deleted_count)
}
