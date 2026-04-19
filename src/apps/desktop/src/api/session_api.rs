//! Session persistence API

use crate::api::app_state::AppState;
use crate::api::session_storage_path::{desktop_effective_session_storage_path, SessionStorageScopeDto};
use bitfun_core::agentic::persistence::PersistenceManager;
use bitfun_core::infrastructure::PathManager;
use bitfun_core::service::session::{
    DialogTurnData, SessionMetadata, SessionTranscriptExport, SessionTranscriptExportOptions,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::State;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPersistedSessionsRequest {
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_scope: Option<SessionStorageScopeDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadSessionTurnsRequest {
    pub session_id: String,
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_scope: Option<SessionStorageScopeDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSessionTurnRequest {
    pub turn_data: DialogTurnData,
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_scope: Option<SessionStorageScopeDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSessionMetadataRequest {
    pub metadata: SessionMetadata,
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_scope: Option<SessionStorageScopeDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSessionTranscriptRequest {
    pub session_id: String,
    pub workspace_path: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_scope: Option<SessionStorageScopeDto>,
}

fn default_tools() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletePersistedSessionRequest {
    pub session_id: String,
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_scope: Option<SessionStorageScopeDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchSessionActivityRequest {
    pub session_id: String,
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_scope: Option<SessionStorageScopeDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadPersistedSessionMetadataRequest {
    pub session_id: String,
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_scope: Option<SessionStorageScopeDto>,
}

fn legacy_dispatcher_workspace_roots(path_manager: &PathManager) -> Vec<PathBuf> {
    let mut roots = vec![
        path_manager.default_assistant_workspace_dir(None),
        path_manager.legacy_default_assistant_workspace_dir(None),
    ];
    for base in [
        path_manager.assistant_workspace_base_dir(None),
        path_manager.legacy_assistant_workspace_base_dir(None),
    ] {
        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
                    continue;
                };
                if name == "workspace" || name.starts_with("workspace-") {
                    roots.push(path);
                }
            }
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

async fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), String> {
    let mut pending = vec![(source.to_path_buf(), target.to_path_buf())];
    while let Some((current_source, current_target)) = pending.pop() {
        fs::create_dir_all(&current_target).await.map_err(|e| {
            format!(
                "Failed to create target directory {}: {}",
                current_target.display(),
                e
            )
        })?;
        let mut entries = fs::read_dir(&current_source).await.map_err(|e| {
            format!(
                "Failed to read directory {}: {}",
                current_source.display(),
                e
            )
        })?;
        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            format!(
                "Failed to read directory entry in {}: {}",
                current_source.display(),
                e
            )
        })? {
            let source_path = entry.path();
            let target_path = current_target.join(entry.file_name());
            let file_type = entry
                .file_type()
                .await
                .map_err(|e| format!("Failed to stat {}: {}", source_path.display(), e))?;
            if file_type.is_dir() {
                pending.push((source_path, target_path));
            } else {
                if let Some(parent) = target_path.parent() {
                    fs::create_dir_all(parent).await.map_err(|e| {
                        format!("Failed to create parent directory {}: {}", parent.display(), e)
                    })?;
                }
                fs::copy(&source_path, &target_path).await.map_err(|e| {
                    format!(
                        "Failed to copy {} to {}: {}",
                        source_path.display(),
                        target_path.display(),
                        e
                    )
                })?;
            }
        }
    }
    Ok(())
}

async fn migrate_legacy_dispatcher_sessions_if_needed(
    manager: &PersistenceManager,
    path_manager: &PathManager,
    agentic_os_root: &Path,
) -> Result<(), String> {
    let existing = manager
        .list_session_metadata(agentic_os_root)
        .await
        .map_err(|e| format!("Failed to inspect Agentic OS sessions: {}", e))?;
    if !existing.is_empty() {
        return Ok(());
    }

    let target_sessions_dir = path_manager.agentic_os_runtime_root().join("sessions");
    fs::create_dir_all(&target_sessions_dir)
        .await
        .map_err(|e| format!("Failed to create Agentic OS sessions dir: {}", e))?;

    for legacy_root in legacy_dispatcher_workspace_roots(path_manager) {
        let legacy_metadata = match manager.list_session_metadata(&legacy_root).await {
            Ok(value) => value,
            Err(_) => continue,
        };
        for metadata in legacy_metadata
            .into_iter()
            .filter(|item| item.agent_type.eq_ignore_ascii_case("dispatcher"))
        {
            let source_dir = path_manager.project_sessions_dir(&legacy_root).join(&metadata.session_id);
            let target_dir = target_sessions_dir.join(&metadata.session_id);
            if target_dir.exists() || !source_dir.exists() {
                continue;
            }
            copy_dir_recursive(&source_dir, &target_dir).await?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn list_persisted_sessions(
    request: ListPersistedSessionsRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<Vec<SessionMetadata>, String> {
    let workspace_path = desktop_effective_session_storage_path(
        &app_state,
        request.workspace_path.as_deref(),
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
        request.storage_scope,
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;
    if matches!(request.storage_scope, Some(SessionStorageScopeDto::AgenticOs)) {
        migrate_legacy_dispatcher_sessions_if_needed(
            &manager,
            path_manager.inner().as_ref(),
            &workspace_path,
        )
        .await?;
    }

    manager
        .list_session_metadata(&workspace_path)
        .await
        .map_err(|e| format!("Failed to list persisted sessions: {}", e))
}

#[tauri::command]
pub async fn load_session_turns(
    request: LoadSessionTurnsRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<Vec<DialogTurnData>, String> {
    let workspace_path = desktop_effective_session_storage_path(
        &app_state,
        request.workspace_path.as_deref(),
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
        request.storage_scope,
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;
    if matches!(request.storage_scope, Some(SessionStorageScopeDto::AgenticOs)) {
        migrate_legacy_dispatcher_sessions_if_needed(
            &manager,
            path_manager.inner().as_ref(),
            &workspace_path,
        )
        .await?;
    }

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

#[tauri::command]
pub async fn save_session_turn(
    request: SaveSessionTurnRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<(), String> {
    let workspace_path = desktop_effective_session_storage_path(
        &app_state,
        request.workspace_path.as_deref(),
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
        request.storage_scope,
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;
    if matches!(request.storage_scope, Some(SessionStorageScopeDto::AgenticOs)) {
        migrate_legacy_dispatcher_sessions_if_needed(
            &manager,
            path_manager.inner().as_ref(),
            &workspace_path,
        )
        .await?;
    }

    manager
        .save_dialog_turn(&workspace_path, &request.turn_data)
        .await
        .map_err(|e| format!("Failed to save session turn: {}", e))
}

#[tauri::command]
pub async fn save_session_metadata(
    request: SaveSessionMetadataRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<(), String> {
    let workspace_path = desktop_effective_session_storage_path(
        &app_state,
        request.workspace_path.as_deref(),
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
        request.storage_scope,
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

    manager
        .save_session_metadata(&workspace_path, &request.metadata)
        .await
        .map_err(|e| format!("Failed to save session metadata: {}", e))
}

#[tauri::command]
pub async fn export_session_transcript(
    request: ExportSessionTranscriptRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<SessionTranscriptExport, String> {
    let workspace_path = desktop_effective_session_storage_path(
        &app_state,
        request.workspace_path.as_deref(),
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
        request.storage_scope,
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
        request.workspace_path.as_deref(),
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
        request.storage_scope,
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

    manager
        .delete_session(&workspace_path, &request.session_id)
        .await
        .map_err(|e| format!("Failed to delete persisted session: {}", e))
}

#[tauri::command]
pub async fn touch_session_activity(
    request: TouchSessionActivityRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<(), String> {
    let workspace_path = desktop_effective_session_storage_path(
        &app_state,
        request.workspace_path.as_deref(),
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
        request.storage_scope,
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

    manager
        .touch_session(&workspace_path, &request.session_id)
        .await
        .map_err(|e| format!("Failed to update session activity: {}", e))
}

#[tauri::command]
pub async fn load_persisted_session_metadata(
    request: LoadPersistedSessionMetadataRequest,
    app_state: State<'_, AppState>,
    path_manager: State<'_, Arc<PathManager>>,
) -> Result<Option<SessionMetadata>, String> {
    let workspace_path = desktop_effective_session_storage_path(
        &app_state,
        request.workspace_path.as_deref(),
        request.remote_connection_id.as_deref(),
        request.remote_ssh_host.as_deref(),
        request.storage_scope,
    )
    .await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("Failed to create persistence manager: {}", e))?;

    let metadata = manager
        .load_session_metadata(&workspace_path, &request.session_id)
        .await
        .map_err(|e| format!("Failed to load persisted session metadata: {}", e))?;

    Ok(metadata.filter(|metadata| !metadata.should_hide_from_user_lists()))
}
