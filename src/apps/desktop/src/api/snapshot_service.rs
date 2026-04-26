//! Snapshot Service API

use bitfun_core::infrastructure::try_get_path_manager_arc;
use bitfun_core::service::remote_ssh::workspace_state::is_remote_path;
use bitfun_core::service::snapshot::{
    ensure_snapshot_manager_for_workspace, get_snapshot_manager_for_workspace,
    initialize_snapshot_manager_for_workspace, OperationType, SnapshotConfig, SnapshotManager,
};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotInitRequest {
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
    pub config: Option<SnapshotConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordFileChangeRequest {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "turnIndex")]
    pub turn_index: usize,
    #[serde(alias = "filePath")]
    pub file_path: String,
    #[serde(alias = "operationType")]
    pub operation_type: String, // "Create", "Modify", "Delete", "Rename"
    #[serde(alias = "toolName")]
    pub tool_name: String,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackSessionRequest {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(default)]
    #[serde(alias = "deleteSession")]
    pub delete_session: bool, // Whether to also delete the session (default false)
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackTurnRequest {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "turnIndex")]
    pub turn_index: usize,
    #[serde(default)]
    pub delete_turns: bool,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptSessionRequest {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptFileRequest {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "filePath")]
    pub file_path: String,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionFilesRequest {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionTurnsRequest {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTurnFilesRequest {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "turnIndex")]
    pub turn_index: usize,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFileDiffRequest {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "filePath")]
    pub file_path: String,
    #[serde(default)]
    #[serde(alias = "operationId")]
    pub operation_id: Option<String>,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBaselineSnapshotDiffRequest {
    #[serde(rename = "filePath")]
    pub file_path: String,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOperationDiffRequest {
    pub sessionId: String,
    pub filePath: String,
    #[serde(default)]
    pub operationId: Option<String>,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOperationSummaryRequest {
    pub sessionId: String,
    pub operationId: String,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionStatsRequest {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFileChangeHistoryRequest {
    #[serde(alias = "filePath")]
    pub file_path: String,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAllModifiedFilesRequest {
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotWorkspaceRequest {
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
}

#[tauri::command]
pub async fn initialize_snapshot(
    app_handle: AppHandle,
    request: SnapshotInitRequest,
) -> Result<serde_json::Value, String> {
    // Remote workspaces don't support snapshot system
    if is_remote_path(&request.workspace_path).await {
        return Ok(serde_json::json!({
            "success": true,
            "message": "Snapshot system skipped for remote workspace"
        }));
    }

    let workspace_dir = PathBuf::from(&request.workspace_path);

    if !workspace_dir.exists() {
        return Err(format!(
            "Workspace directory does not exist: {}",
            request.workspace_path
        ));
    }

    initialize_snapshot_manager_for_workspace(workspace_dir, request.config)
        .await
        .map_err(|e| format!("Failed to initialize snapshot system: {}", e))?;

    let _ = app_handle.emit(
        "snapshot_initialized",
        serde_json::json!({
            "workspace_path": request.workspace_path,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }),
    );

    Ok(serde_json::json!({
        "success": true,
        "message": "Snapshot system initialized"
    }))
}

async fn resolve_workspace_dir(workspace_path: &str) -> Result<PathBuf, String> {
    if workspace_path.trim().is_empty() {
        return Err("workspacePath is required".to_string());
    }

    let workspace_dir = PathBuf::from(workspace_path);
    // Remote paths don't exist on the local filesystem — skip the existence check
    if !is_remote_path(workspace_path).await && !workspace_dir.exists() {
        return Err(format!(
            "Workspace directory does not exist: {}",
            workspace_path
        ));
    }

    Ok(workspace_dir)
}

async fn ensure_snapshot_manager_ready(
    workspace_path: &str,
) -> Result<Arc<SnapshotManager>, String> {
    // Remote workspaces don't support the snapshot system
    if is_remote_path(workspace_path).await {
        return Err(format!(
            "Snapshot system not supported for remote workspace: {}",
            workspace_path
        ));
    }

    let workspace_dir = resolve_workspace_dir(workspace_path).await?;

    if let Some(manager) = get_snapshot_manager_for_workspace(&workspace_dir) {
        return Ok(manager);
    }

    info!(
        "Snapshot manager missing, initializing lazily: workspace={}",
        workspace_dir.display()
    );

    initialize_snapshot_manager_for_workspace(workspace_dir.clone(), None)
        .await
        .map_err(|e| {
            format!(
                "Failed to initialize snapshot system for workspace {}: {}",
                workspace_dir.display(),
                e
            )
        })?;

    ensure_snapshot_manager_for_workspace(&workspace_dir)
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))
}

#[tauri::command]
pub async fn record_file_change(
    app_handle: AppHandle,
    request: RecordFileChangeRequest,
) -> Result<String, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let operation_type = match request.operation_type.as_str() {
        "Create" => OperationType::Create,
        "Modify" => OperationType::Modify,
        "Delete" => OperationType::Delete,
        "Rename" => OperationType::Rename,
        _ => {
            return Err(format!(
                "Unknown operation type: {}",
                request.operation_type
            ))
        }
    };

    let snapshot_id = manager
        .record_file_change(
            &request.session_id,
            request.turn_index,
            PathBuf::from(&request.file_path),
            operation_type,
            request.tool_name.clone(),
        )
        .await
        .map_err(|e| format!("Failed to record file change: {}", e))?;

    let _ = app_handle.emit(
        "file_change_recorded",
        serde_json::json!({
            "session_id": request.session_id,
            "turn_index": request.turn_index,
            "file_path": request.file_path,
            "snapshot_id": snapshot_id,
        }),
    );

    Ok(snapshot_id)
}

#[tauri::command]
pub async fn rollback_session(
    app_handle: AppHandle,
    request: RollbackSessionRequest,
) -> Result<Vec<String>, String> {
    // Remote workspaces have no local snapshots — nothing to roll back
    if is_remote_path(&request.workspace_path).await {
        return Ok(vec![]);
    }

    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let restored_files = manager
        .rollback_session(&request.session_id)
        .await
        .map_err(|e| format!("Failed to rollback session: {}", e))?;

    let restored_files_str: Vec<String> = restored_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let _ = app_handle.emit(
        "session_rolled_back",
        serde_json::json!({
            "session_id": request.session_id,
            "files_count": restored_files_str.len(),
            "session_deleted": request.delete_session,
        }),
    );

    Ok(restored_files_str)
}

#[tauri::command]
pub async fn rollback_to_turn(
    app_handle: AppHandle,
    request: RollbackTurnRequest,
) -> Result<Vec<String>, String> {
    // Remote workspaces have no local snapshots — nothing to roll back
    if is_remote_path(&request.workspace_path).await {
        return Ok(vec![]);
    }

    {
        use bitfun_core::agentic::coordination::get_global_coordinator;

        if let Some(coordinator) = get_global_coordinator() {
            if let Err(e) = coordinator
                .cancel_active_turn_for_session(&request.session_id, Duration::from_secs(2))
                .await
            {
                warn!(
                    "Failed to cancel active turn before rollback: session_id={}, turn_index={}, error={}",
                    request.session_id, request.turn_index, e
                );
            }
        }
    }

    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let restored_files = manager
        .rollback_to_turn(&request.session_id, request.turn_index)
        .await
        .map_err(|e| format!("Failed to rollback turn: {}", e))?;

    let restored_files_str: Vec<String> = restored_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let mut deleted_turns_count = 0;
    if request.delete_turns {
        let workspace_path = PathBuf::from(&request.workspace_path);
        {
            use bitfun_core::agentic::coordination::get_global_coordinator;

            if let Some(coordinator) = get_global_coordinator() {
                if let Err(e) = coordinator
                    .get_session_manager()
                    .rollback_context_to_turn_start(
                        &workspace_path,
                        &request.session_id,
                        request.turn_index,
                    )
                    .await
                {
                    warn!(
                        "Rollback agentic context failed: session_id={}, turn_index={}, error={}",
                        request.session_id, request.turn_index, e
                    );
                }
            } else {
                warn!("Global coordinator not initialized, skipping agentic context rollback");
            }
        }

        use bitfun_core::agentic::persistence::PersistenceManager;

        match try_get_path_manager_arc() {
            Ok(path_manager) => match PersistenceManager::new(path_manager) {
                Ok(persistence_manager) => {
                    match persistence_manager
                        .delete_turns_from(&workspace_path, &request.session_id, request.turn_index)
                        .await
                    {
                        Ok(count) => {
                            deleted_turns_count = count;
                        }
                        Err(e) => {
                            warn!("Failed to delete conversation turns: session_id={}, turn_index={}, error={}", request.session_id, request.turn_index, e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to create PersistenceManager: error={}", e);
                }
            },
            Err(e) => {
                warn!("Failed to create PathManager: error={}", e);
            }
        }

        let _ = app_handle.emit(
            "conversation_turns_deleted",
            serde_json::json!({
                "session_id": request.session_id,
                "remaining_turns": request.turn_index,
                "deleted_count": deleted_turns_count,
            }),
        );
    }

    let _ = app_handle.emit(
        "turn_rolled_back",
        serde_json::json!({
            "session_id": request.session_id,
            "turn_index": request.turn_index,
            "files_count": restored_files_str.len(),
            "deleted_turns": request.delete_turns,
            "deleted_turns_count": deleted_turns_count,
        }),
    );

    Ok(restored_files_str)
}

#[tauri::command]
pub async fn accept_session(
    app_handle: AppHandle,
    request: AcceptSessionRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    manager
        .accept_session(&request.session_id)
        .await
        .map_err(|e| format!("Failed to accept session: {}", e))?;

    let _ = app_handle.emit(
        "session_accepted",
        serde_json::json!({
            "session_id": request.session_id,
        }),
    );

    Ok(serde_json::json!({
        "success": true,
        "message": "Session changes accepted"
    }))
}

#[tauri::command]
pub async fn accept_file(
    app_handle: AppHandle,
    request: AcceptFileRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    manager
        .accept_file(&request.session_id, &request.file_path)
        .await
        .map_err(|e| format!("Failed to accept file: {}", e))?;

    let _ = app_handle.emit(
        "file_accepted",
        serde_json::json!({
            "session_id": request.session_id,
            "file_path": request.file_path,
        }),
    );

    Ok(serde_json::json!({
        "success": true,
        "message": "File changes accepted"
    }))
}

#[tauri::command]
pub async fn reject_file(
    app_handle: AppHandle,
    request: AcceptFileRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let restored_files = manager
        .reject_file(&request.session_id, &request.file_path)
        .await
        .map_err(|e| format!("Failed to reject file: {}", e))?;

    let restored_files_str: Vec<String> = restored_files
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();

    let _ = app_handle.emit(
        "file_rejected",
        serde_json::json!({
            "session_id": request.session_id,
            "file_path": request.file_path,
            "restored_files": restored_files_str,
        }),
    );

    Ok(serde_json::json!({
        "success": true,
        "message": "File changes rejected"
    }))
}

#[tauri::command]
pub async fn get_session_files(request: GetSessionFilesRequest) -> Result<Vec<String>, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let files = manager
        .get_session_files(&request.session_id)
        .await
        .map_err(|e| format!("Failed to get session files: {}", e))?;

    Ok(files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

#[tauri::command]
pub async fn get_session_turns(
    _app_handle: AppHandle,
    request: GetSessionTurnsRequest,
) -> Result<Vec<usize>, String> {
    use bitfun_core::agentic::persistence::PersistenceManager;

    let workspace_path = PathBuf::from(&request.workspace_path);
    if let Ok(path_manager) = try_get_path_manager_arc() {
        match PersistenceManager::new(path_manager) {
            Ok(persistence_manager) => {
                match persistence_manager
                    .load_session_metadata(&workspace_path, &request.session_id)
                    .await
                {
                    Ok(Some(metadata)) => {
                        let turns: Vec<usize> = (0..metadata.turn_count).collect();
                        return Ok(turns);
                    }
                    Ok(None) => {}
                    Err(e) => {
                        warn!("Failed to load conversation metadata: session_id={}, error={}, falling back to snapshot", request.session_id, e);
                    }
                }
            }
            Err(e) => {
                warn!(
                    "Failed to create PersistenceManager: error={}, falling back to snapshot",
                    e
                );
            }
        }
    }

    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let turns = manager
        .get_session_turns(&request.session_id)
        .await
        .map_err(|e| format!("Failed to get session turns: {}", e))?;

    Ok(turns)
}

#[tauri::command]
pub async fn get_turn_files(request: GetTurnFilesRequest) -> Result<Vec<String>, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let files = manager
        .get_turn_files(&request.session_id, request.turn_index)
        .await
        .map_err(|e| format!("Failed to get turn files: {}", e))?;

    Ok(files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

#[tauri::command]
pub async fn get_file_diff(request: GetFileDiffRequest) -> Result<serde_json::Value, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let diff = manager
        .get_file_diff(
            &request.session_id,
            &request.file_path,
            request.operation_id.as_deref(),
        )
        .await
        .map_err(|e| format!("Failed to get file diff: {}", e))?;

    Ok(diff)
}

#[tauri::command]
pub async fn get_operation_diff(
    request: GetOperationDiffRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let diff = manager
        .get_file_diff(
            &request.sessionId,
            &request.filePath,
            request.operationId.as_deref(),
        )
        .await
        .map_err(|e| format!("Failed to get file diff: {}", e))?;

    let original = diff
        .get("original_content")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let modified = diff
        .get("modified_content")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    log::debug!(
        "get_operation_diff: session_id={} file_path={} operation_id={:?} original_len={} modified_len={} identical={}",
        request.sessionId,
        request.filePath,
        request.operationId,
        original.len(),
        modified.len(),
        original == modified
    );

    Ok(serde_json::json!({
        "filePath": diff.get("file_path").and_then(|v| v.as_str()).unwrap_or(&request.filePath),
        "originalContent": original.to_string(),
        "modifiedContent": modified.to_string(),
        "anchorLine": diff.get("anchor_line").and_then(|v| v.as_u64()),
    }))
}

#[tauri::command]
pub async fn get_operation_summary(
    request: GetOperationSummaryRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let summary = manager
        .get_operation_summary(&request.sessionId, &request.operationId)
        .await
        .map_err(|e| format!("Failed to get operation summary: {}", e))?;

    Ok(serde_json::json!({
        "operationId": summary.get("operation_id").and_then(|v| v.as_str()).unwrap_or(&request.operationId),
        "sessionId": summary.get("session_id").and_then(|v| v.as_str()).unwrap_or(&request.sessionId),
        "turnIndex": summary.get("turn_index").and_then(|v| v.as_u64()),
        "seqInTurn": summary.get("seq_in_turn").and_then(|v| v.as_u64()),
        "filePath": summary.get("file_path").and_then(|v| v.as_str()),
        "operationType": summary.get("operation_type").and_then(|v| v.as_str()),
        "toolName": summary.get("tool_name").and_then(|v| v.as_str()),
        "linesAdded": summary.get("lines_added").and_then(|v| v.as_u64()),
        "linesRemoved": summary.get("lines_removed").and_then(|v| v.as_u64()),
    }))
}

#[tauri::command]
pub async fn get_session_operations(
    request: GetSessionFilesRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let session = manager
        .get_session(&request.session_id)
        .await
        .map_err(|e| format!("Failed to get session operations: {}", e))?;

    let operations: Vec<serde_json::Value> = session
        .operations
        .into_iter()
        .map(|operation| {
            let operation_type = match operation.operation_type {
                OperationType::Create => "create",
                OperationType::Modify => "modify",
                OperationType::Delete => "delete",
                OperationType::Rename => "rename",
            };

            serde_json::json!({
                "operation_id": operation.operation_id,
                "session_id": operation.session_id,
                "turn_index": operation.turn_index,
                "seq_in_turn": operation.seq_in_turn,
                "file_path": operation.file_path.to_string_lossy().to_string(),
                "tool_name": operation.tool_context.tool_name,
                "operation_type": operation_type,
                "status": "applied",
                "timestamp": chrono::DateTime::<chrono::Utc>::from(operation.timestamp).to_rfc3339(),
                "diff_summary": {
                    "lines_added": operation.diff_summary.lines_added,
                    "lines_removed": operation.diff_summary.lines_removed,
                    "blocks_changed": operation.diff_summary.lines_modified,
                }
            })
        })
        .collect();

    Ok(serde_json::Value::Array(operations))
}

#[tauri::command]
pub async fn accept_operation(
    app_handle: AppHandle,
    request: GetOperationSummaryRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let summary = manager
        .get_operation_summary(&request.sessionId, &request.operationId)
        .await
        .map_err(|e| format!("Failed to accept operation: {}", e))?;
    let file_path = summary
        .get("file_path")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "Operation file path not found".to_string())?;

    manager
        .accept_file(&request.sessionId, file_path)
        .await
        .map_err(|e| format!("Failed to accept operation: {}", e))?;

    let _ = app_handle.emit(
        "operation_accepted",
        serde_json::json!({
            "session_id": request.sessionId,
            "operation_id": request.operationId,
            "file_path": file_path,
        }),
    );

    Ok(serde_json::json!({
        "success": true,
        "message": "Operation accepted"
    }))
}

#[tauri::command]
pub async fn reject_operation(
    app_handle: AppHandle,
    request: GetOperationSummaryRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let summary = manager
        .get_operation_summary(&request.sessionId, &request.operationId)
        .await
        .map_err(|e| format!("Failed to reject operation: {}", e))?;
    let file_path = summary
        .get("file_path")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "Operation file path not found".to_string())?;

    let restored_files = manager
        .reject_file(&request.sessionId, file_path)
        .await
        .map_err(|e| format!("Failed to reject operation: {}", e))?;

    let restored_files_str: Vec<String> = restored_files
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();

    let _ = app_handle.emit(
        "operation_rejected",
        serde_json::json!({
            "session_id": request.sessionId,
            "operation_id": request.operationId,
            "file_path": file_path,
            "restored_files": restored_files_str,
        }),
    );

    Ok(serde_json::json!({
        "success": true,
        "message": "Operation rejected"
    }))
}

#[tauri::command]
pub async fn get_session_stats(
    request: GetSessionStatsRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let stats = manager
        .get_session_stats(&request.session_id)
        .await
        .map_err(|e| format!("Failed to get session stats: {}", e))?;

    Ok(stats)
}

#[tauri::command]
pub async fn get_snapshot_system_stats(
    request: SnapshotWorkspaceRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let stats = manager
        .get_system_stats()
        .await
        .map_err(|e| format!("Failed to get system stats: {}", e))?;

    Ok(stats)
}

#[tauri::command]
pub async fn get_snapshot_sessions(
    request: SnapshotWorkspaceRequest,
) -> Result<Vec<String>, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    manager
        .list_sessions()
        .await
        .map_err(|e| format!("Failed to list snapshot sessions: {}", e))
}

#[tauri::command]
pub async fn check_git_isolation(
    request: SnapshotWorkspaceRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let is_isolated = manager
        .check_git_isolation()
        .await
        .map_err(|e| format!("Failed to check git isolation: {}", e))?;

    Ok(serde_json::json!({
        "git_isolated": is_isolated,
        "message": if is_isolated { "Git repository is safely isolated" } else { "Git isolation status abnormal" }
    }))
}

#[tauri::command]
pub async fn get_file_change_history(
    request: GetFileChangeHistoryRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let file_path = PathBuf::from(&request.file_path);
    let changes = manager
        .get_file_change_history(&file_path)
        .await
        .map_err(|e| format!("Failed to get file change history: {}", e))?;

    serde_json::to_value(changes).map_err(|e| format!("Serialization failed: {}", e))
}

#[tauri::command]
pub async fn get_all_modified_files(
    request: GetAllModifiedFilesRequest,
) -> Result<Vec<String>, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let files = manager
        .get_all_modified_files()
        .await
        .map_err(|e| format!("Failed to get modified files: {}", e))?;

    Ok(files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

#[tauri::command]
pub async fn get_baseline_snapshot_diff(
    request: GetBaselineSnapshotDiffRequest,
) -> Result<serde_json::Value, String> {
    let manager = ensure_snapshot_manager_ready(&request.workspace_path).await?;

    let file_path = PathBuf::from(&request.file_path);

    let (baseline_content, current_content) = {
        let snapshot_service = manager.get_snapshot_service();
        let snapshot_service = snapshot_service.read().await;

        match snapshot_service
            .get_baseline_snapshot_diff(&file_path)
            .await
        {
            Ok(diff) => diff,
            Err(e) => {
                warn!(
                    "Failed to get baseline diff: file_path={}, error={}",
                    request.file_path, e
                );
                (String::new(), String::new())
            }
        }
    };

    Ok(serde_json::json!({
        "filePath": request.file_path,
        "originalContent": baseline_content,
        "modifiedContent": current_content,
    }))
}
