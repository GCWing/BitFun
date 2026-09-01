//! Snapshot Service API

use bitfun_core::product_runtime::{
    CoreSessionMaintenancePermit, CoreSessionMutationPermit, CoreSessionReadPermit,
};
use bitfun_core::service::remote_ssh::workspace_state::{
    is_remote_path, workspace_session_identity,
};
use bitfun_core::service::snapshot::{
    ensure_snapshot_manager_for_workspace, get_snapshot_manager_for_workspace,
    initialize_snapshot_manager_for_workspace, open_snapshot_history_for_workspace,
    open_snapshot_manager_for_view, FileChangeEntry, OperationType, SnapshotManager,
};
use bitfun_runtime_ports::{AgentSessionWorkspaceLocation, SessionStoragePathRequest};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::{path::PathBuf, sync::Arc};
use tauri::{AppHandle, Emitter, State};

use crate::runtime::{DesktopRuntimeContext, DesktopSessionScopeRequest};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapshotRemoteScope {
    #[serde(default, alias = "remoteConnectionId")]
    pub remote_connection_id: Option<String>,
    #[serde(default, alias = "remoteSshHost")]
    pub remote_ssh_host: Option<String>,
}

impl SnapshotRemoteScope {
    fn declares_remote(&self) -> bool {
        self.remote_connection_id
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
            || self
                .remote_ssh_host
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
    }
}

async fn ensure_local_runtime_ownership(
    runtime: &DesktopRuntimeContext,
    workspace_path: &str,
) -> Result<(), String> {
    runtime
        .session_application()
        .ensure_workspace_runtime_ownership(DesktopSessionScopeRequest {
            workspace_path: workspace_path.to_string(),
            remote_connection_id: None,
            remote_ssh_host: None,
        })
        .await
        .map_err(|error| error.to_string())
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
    #[serde(flatten)]
    pub remote_scope: SnapshotRemoteScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptSessionRequest {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
    #[serde(flatten)]
    pub remote_scope: SnapshotRemoteScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptFileRequest {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "filePath")]
    pub file_path: String,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
    #[serde(flatten)]
    pub remote_scope: SnapshotRemoteScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionFilesRequest {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
    #[serde(flatten)]
    pub remote_scope: SnapshotRemoteScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionTurnsRequest {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
    #[serde(flatten)]
    pub remote_scope: SnapshotRemoteScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTurnFilesRequest {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "turnIndex")]
    pub turn_index: usize,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
    #[serde(flatten)]
    pub remote_scope: SnapshotRemoteScope,
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
    #[serde(flatten)]
    pub remote_scope: SnapshotRemoteScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBaselineSnapshotDiffRequest {
    #[serde(rename = "filePath")]
    pub file_path: String,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
    #[serde(flatten)]
    pub remote_scope: SnapshotRemoteScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOperationDiffRequest {
    pub sessionId: String,
    pub filePath: String,
    #[serde(default)]
    pub operationId: Option<String>,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
    #[serde(flatten)]
    pub remote_scope: SnapshotRemoteScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionFileDiffStatsRequest {
    pub sessionId: String,
    pub filePath: String,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
    #[serde(flatten)]
    pub remote_scope: SnapshotRemoteScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOperationSummaryRequest {
    pub sessionId: String,
    pub operationId: String,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
    #[serde(flatten)]
    pub remote_scope: SnapshotRemoteScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionStatsRequest {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
    #[serde(flatten)]
    pub remote_scope: SnapshotRemoteScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFileChangeHistoryRequest {
    #[serde(alias = "filePath")]
    pub file_path: String,
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
    #[serde(flatten)]
    pub remote_scope: SnapshotRemoteScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAllModifiedFilesRequest {
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
    #[serde(flatten)]
    pub remote_scope: SnapshotRemoteScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotWorkspaceRequest {
    #[serde(alias = "workspacePath")]
    pub workspace_path: String,
    #[serde(flatten)]
    pub remote_scope: SnapshotRemoteScope,
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

async fn ensure_snapshot_manager_ready_for(
    workspace_path: &str,
    caller: &str,
) -> Result<Arc<SnapshotManager>, String> {
    let started_at = std::time::Instant::now();
    // Remote workspaces don't support the snapshot system
    if is_remote_path(workspace_path).await {
        return Err(format!(
            "Snapshot system not supported for remote workspace: {}",
            workspace_path
        ));
    }

    let workspace_dir = resolve_workspace_dir(workspace_path).await?;

    if let Some(manager) = get_snapshot_manager_for_workspace(&workspace_dir) {
        let duration_ms = started_at.elapsed().as_millis();
        if duration_ms >= 20 {
            log::debug!(
                "Snapshot manager ready: caller={}, workspace={}, source=cache, duration_ms={}",
                caller,
                workspace_dir.display(),
                duration_ms
            );
        }
        return Ok(manager);
    }

    info!(
        "Snapshot manager missing, initializing lazily: caller={}, workspace={}",
        caller,
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

    let manager = ensure_snapshot_manager_for_workspace(&workspace_dir)
        .map_err(|e| format!("Failed to get snapshot manager: {}", e))?;
    log::debug!(
        "Snapshot manager ready: caller={}, workspace={}, source=lazy_init, duration_ms={}",
        caller,
        workspace_dir.display(),
        started_at.elapsed().as_millis()
    );
    Ok(manager)
}

async fn ensure_local_snapshot_mutation_path(
    workspace_path: &str,
    remote_scope: &SnapshotRemoteScope,
) -> Result<(), String> {
    if remote_scope.declares_remote() || is_remote_path(workspace_path).await {
        return Err(format!(
            "Snapshot system not supported for remote workspace: {}",
            workspace_path
        ));
    }
    Ok(())
}

async fn ensure_complete_rollback_supported(
    workspace_path: &str,
    remote_scope: &SnapshotRemoteScope,
    explicit_location: Option<AgentSessionWorkspaceLocation>,
) -> Result<(), String> {
    let is_remote = remote_scope.declares_remote()
        || explicit_location == Some(AgentSessionWorkspaceLocation::Remote)
        || (explicit_location.is_none() && is_remote_path(workspace_path).await);
    if is_remote {
        return Err(format!(
            "Complete rollback is not supported for remote workspaces because complete file snapshot coverage is unavailable. No workspace files or session messages were changed: {workspace_path}"
        ));
    }
    Ok(())
}

async fn snapshot_manager_for_view(
    workspace_path: &str,
    remote_scope: &SnapshotRemoteScope,
) -> Result<Arc<SnapshotManager>, String> {
    if remote_scope.declares_remote() || is_remote_path(workspace_path).await {
        return Err(format!(
            "Snapshot view unavailable (snapshot_remote_workspace_unavailable): remote workspace {} has no local snapshot runtime",
            workspace_path
        ));
    }
    let workspace_dir = resolve_workspace_dir(workspace_path).await?;
    open_snapshot_manager_for_view(&workspace_dir)
        .await
        .map_err(|error| format!("Failed to open snapshot view: {error}"))
}

/// Only immutable, persisted operation facts are available without a live
/// workspace. The full Session rollback and current-file view keep their own
/// coverage requirements.
async fn snapshot_manager_for_recorded_operation(
    workspace_path: &str,
    remote_scope: &SnapshotRemoteScope,
) -> Result<Arc<SnapshotManager>, String> {
    if !remote_scope.declares_remote() {
        return snapshot_manager_for_view(workspace_path, remote_scope).await;
    }
    let connection_id = remote_scope
        .remote_connection_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let hostname = remote_scope
        .remote_ssh_host
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if connection_id.is_none() || hostname.is_none() {
        return Err("Snapshot operation view unavailable: the remote Session requires its saved connection id and host; no local fallback was attempted".into());
    }
    let identity =
        workspace_session_identity(workspace_path, connection_id, hostname).ok_or_else(|| {
            "Snapshot operation view unavailable: invalid remote workspace identity".to_string()
        })?;
    open_snapshot_history_for_workspace(&identity)
        .await
        .map_err(|error| format!("Snapshot operation view unavailable: {error}"))
}

struct SnapshotHistoryMutation {
    _maintenance: CoreSessionMaintenancePermit,
    _mutation: CoreSessionMutationPermit,
    _storage_path: PathBuf,
}

async fn begin_snapshot_history_read(
    runtime: &DesktopRuntimeContext,
    workspace_path: &str,
    session_id: &str,
) -> Result<CoreSessionReadPermit, String> {
    begin_snapshot_history_read_for_scope(
        runtime,
        workspace_path,
        session_id,
        &SnapshotRemoteScope::default(),
    )
    .await
}

async fn begin_snapshot_history_read_for_scope(
    runtime: &DesktopRuntimeContext,
    workspace_path: &str,
    session_id: &str,
    remote_scope: &SnapshotRemoteScope,
) -> Result<CoreSessionReadPermit, String> {
    // Warm the snapshot view before taking the exclusive session read permit.
    // The first view open for a workspace loads the entire snapshot index from
    // disk; holding the session mutation lock across that load stalls every
    // waiter on the same session, including the next dialog-turn start. After
    // warming, the caller's in-permit view lookup is a cache hit.
    if remote_scope.declares_remote() {
        snapshot_manager_for_recorded_operation(workspace_path, remote_scope).await?;
    } else if !is_remote_path(workspace_path).await {
        let workspace_dir = resolve_workspace_dir(workspace_path).await?;
        open_snapshot_manager_for_view(&workspace_dir)
            .await
            .map_err(|error| format!("Failed to open snapshot view: {error}"))?;
    }
    let compatibility = runtime.session_application().compatibility();
    let storage_path = compatibility
        .resolve_persisted_session_storage_path(SessionStoragePathRequest {
            workspace_path: PathBuf::from(workspace_path),
            remote_connection_id: remote_scope.remote_connection_id.clone(),
            remote_ssh_host: remote_scope.remote_ssh_host.clone(),
        })
        .await
        .map_err(|error| {
            format!("Failed to resolve session storage before snapshot view: {error}")
        })?;
    let read = if remote_scope.declares_remote() {
        let identity = workspace_session_identity(
            workspace_path,
            remote_scope.remote_connection_id.as_deref(),
            remote_scope.remote_ssh_host.as_deref(),
        )
        .ok_or_else(|| {
            "Snapshot operation view unavailable: invalid Session identity".to_string()
        })?;
        compatibility
            .begin_persisted_session_read_for_workspace(&storage_path, session_id, &identity)
            .await
    } else {
        compatibility
            .begin_persisted_session_read(&storage_path, session_id)
            .await
    };
    read.map_err(|error| format!("Failed to open a consistent snapshot view: {error}"))
}

async fn begin_snapshot_history_mutation(
    runtime: &DesktopRuntimeContext,
    workspace_path: &str,
    session_id: &str,
) -> Result<SnapshotHistoryMutation, String> {
    let compatibility = runtime.session_application().compatibility();
    let storage_path = compatibility
        .resolve_persisted_session_storage_path(SessionStoragePathRequest {
            workspace_path: PathBuf::from(workspace_path),
            remote_connection_id: None,
            remote_ssh_host: None,
        })
        .await
        .map_err(|error| {
            format!("Failed to resolve session storage before snapshot mutation: {error}")
        })?;
    compatibility
        .ensure_session_loaded_from_storage_path(&storage_path, session_id, false)
        .await
        .map_err(|error| format!("Failed to load session before snapshot mutation: {error}"))?;
    let maintenance = compatibility
        .begin_session_maintenance(&storage_path, session_id, 2_000)
        .await
        .map_err(|error| format!("Failed to quiesce session before snapshot mutation: {error}"))?;
    let mutation = compatibility
        .begin_persisted_session_mutation(&storage_path, session_id)
        .await
        .map_err(|error| format!("Failed to lock snapshot mutation: {error}"))?;
    compatibility
        .commit_session_revert_before_snapshot_mutation(&mutation)
        .await
        .map_err(|error| {
            format!("Failed to commit the staged Session undo before snapshot mutation: {error}")
        })?;
    Ok(SnapshotHistoryMutation {
        _maintenance: maintenance,
        _mutation: mutation,
        _storage_path: storage_path,
    })
}

#[tauri::command]
pub async fn rollback_session(
    app_handle: AppHandle,
    runtime: State<'_, DesktopRuntimeContext>,
    request: RollbackSessionRequest,
) -> Result<Vec<String>, String> {
    ensure_complete_rollback_supported(&request.workspace_path, &request.remote_scope, None)
        .await?;
    ensure_local_runtime_ownership(runtime.inner(), &request.workspace_path).await?;
    let _history_mutation = begin_snapshot_history_mutation(
        runtime.inner(),
        &request.workspace_path,
        &request.session_id,
    )
    .await?;

    let manager =
        ensure_snapshot_manager_ready_for(&request.workspace_path, "rollback_session").await?;

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
pub async fn rollback_session_to_turn(
    runtime: State<'_, DesktopRuntimeContext>,
    request: bitfun_runtime_ports::AgentSessionRollbackToTurnRequest,
) -> Result<bitfun_runtime_ports::AgentSessionRollbackToTurnOutcome, String> {
    let remote_scope = SnapshotRemoteScope {
        remote_connection_id: request.remote_connection_id.clone(),
        remote_ssh_host: request.remote_ssh_host.clone(),
    };
    ensure_complete_rollback_supported(
        &request.workspace_path,
        &remote_scope,
        request.explicit_workspace_location(),
    )
    .await?;
    ensure_local_runtime_ownership(runtime.inner(), &request.workspace_path).await?;
    runtime
        .session_application()
        .rollback_session_to_turn(request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn accept_session(
    app_handle: AppHandle,
    runtime: State<'_, DesktopRuntimeContext>,
    request: AcceptSessionRequest,
) -> Result<serde_json::Value, String> {
    ensure_local_snapshot_mutation_path(&request.workspace_path, &request.remote_scope).await?;
    ensure_local_runtime_ownership(runtime.inner(), &request.workspace_path).await?;
    let _history_mutation = begin_snapshot_history_mutation(
        runtime.inner(),
        &request.workspace_path,
        &request.session_id,
    )
    .await?;
    let manager =
        ensure_snapshot_manager_ready_for(&request.workspace_path, "accept_session").await?;

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
    runtime: State<'_, DesktopRuntimeContext>,
    request: AcceptFileRequest,
) -> Result<serde_json::Value, String> {
    ensure_local_snapshot_mutation_path(&request.workspace_path, &request.remote_scope).await?;
    ensure_local_runtime_ownership(runtime.inner(), &request.workspace_path).await?;
    let _history_mutation = begin_snapshot_history_mutation(
        runtime.inner(),
        &request.workspace_path,
        &request.session_id,
    )
    .await?;
    let manager = ensure_snapshot_manager_ready_for(&request.workspace_path, "accept_file").await?;

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
    runtime: State<'_, DesktopRuntimeContext>,
    request: AcceptFileRequest,
) -> Result<serde_json::Value, String> {
    ensure_local_snapshot_mutation_path(&request.workspace_path, &request.remote_scope).await?;
    ensure_local_runtime_ownership(runtime.inner(), &request.workspace_path).await?;
    let _history_mutation = begin_snapshot_history_mutation(
        runtime.inner(),
        &request.workspace_path,
        &request.session_id,
    )
    .await?;
    let manager = ensure_snapshot_manager_ready_for(&request.workspace_path, "reject_file").await?;

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
pub async fn get_session_files(
    runtime: State<'_, DesktopRuntimeContext>,
    request: GetSessionFilesRequest,
) -> Result<Vec<String>, String> {
    if request.remote_scope.declares_remote() || is_remote_path(&request.workspace_path).await {
        return Ok(vec![]);
    }
    let read = begin_snapshot_history_read(
        runtime.inner(),
        &request.workspace_path,
        &request.session_id,
    )
    .await?;
    let manager = snapshot_manager_for_view(&request.workspace_path, &request.remote_scope).await?;
    manager
        .get_session_files_before(&request.session_id, read.visible_turn_end())
        .await
        .map(|files| {
            files
                .into_iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect()
        })
        .map_err(|error| format!("Failed to get session files: {error}"))
}

#[tauri::command]
pub async fn get_session_turns(
    runtime: State<'_, DesktopRuntimeContext>,
    request: GetSessionTurnsRequest,
) -> Result<Vec<usize>, String> {
    if request.remote_scope.declares_remote() || is_remote_path(&request.workspace_path).await {
        return Ok(vec![]);
    }
    let read = begin_snapshot_history_read(
        runtime.inner(),
        &request.workspace_path,
        &request.session_id,
    )
    .await?;
    let manager = snapshot_manager_for_view(&request.workspace_path, &request.remote_scope).await?;
    manager
        .get_session_turns_before(&request.session_id, read.visible_turn_end())
        .await
        .map_err(|error| format!("Failed to get session turns: {error}"))
}

#[tauri::command]
pub async fn get_turn_files(
    runtime: State<'_, DesktopRuntimeContext>,
    request: GetTurnFilesRequest,
) -> Result<Vec<String>, String> {
    if request.remote_scope.declares_remote() || is_remote_path(&request.workspace_path).await {
        return Ok(vec![]);
    }
    let read = begin_snapshot_history_read(
        runtime.inner(),
        &request.workspace_path,
        &request.session_id,
    )
    .await?;
    let manager = snapshot_manager_for_view(&request.workspace_path, &request.remote_scope).await?;
    let files = manager
        .get_turn_files_before(
            &request.session_id,
            request.turn_index,
            read.visible_turn_end(),
        )
        .await
        .map_err(|e| format!("Failed to get turn files: {}", e))?;

    Ok(files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

#[tauri::command]
pub async fn get_file_diff(
    runtime: State<'_, DesktopRuntimeContext>,
    request: GetFileDiffRequest,
) -> Result<serde_json::Value, String> {
    ensure_local_snapshot_mutation_path(&request.workspace_path, &request.remote_scope).await?;
    let read = begin_snapshot_history_read(
        runtime.inner(),
        &request.workspace_path,
        &request.session_id,
    )
    .await?;
    let manager = snapshot_manager_for_view(&request.workspace_path, &request.remote_scope).await?;

    let diff = manager
        .get_file_diff_before(
            &request.session_id,
            &request.file_path,
            request.operation_id.as_deref(),
            read.visible_turn_end(),
        )
        .await
        .map_err(|e| format!("Failed to get file diff: {}", e))?;

    Ok(diff)
}

#[tauri::command]
pub async fn get_operation_diff(
    runtime: State<'_, DesktopRuntimeContext>,
    request: GetOperationDiffRequest,
) -> Result<serde_json::Value, String> {
    if request.remote_scope.declares_remote()
        && request
            .operationId
            .as_deref()
            .is_none_or(|id| id.trim().is_empty())
    {
        return Err("Remote snapshot views require a recorded operationId; a current-workspace diff is not available offline".into());
    }
    let read = begin_snapshot_history_read_for_scope(
        runtime.inner(),
        &request.workspace_path,
        &request.sessionId,
        &request.remote_scope,
    )
    .await?;
    let manager =
        snapshot_manager_for_recorded_operation(&request.workspace_path, &request.remote_scope)
            .await?;

    let diff = match request.operationId.as_deref() {
        Some(operation_id) => {
            manager
                .get_operation_diff_before(
                    &request.sessionId,
                    &request.filePath,
                    operation_id,
                    read.visible_turn_end(),
                )
                .await
        }
        None => {
            manager
                .get_file_diff_before(
                    &request.sessionId,
                    &request.filePath,
                    None,
                    read.visible_turn_end(),
                )
                .await
        }
    }
    .map_err(|error| format!("Failed to get operation diff: {error}"))?;

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
pub async fn get_session_file_diff_stats(
    runtime: State<'_, DesktopRuntimeContext>,
    request: GetSessionFileDiffStatsRequest,
) -> Result<serde_json::Value, String> {
    ensure_local_snapshot_mutation_path(&request.workspace_path, &request.remote_scope).await?;
    let read =
        begin_snapshot_history_read(runtime.inner(), &request.workspace_path, &request.sessionId)
            .await?;
    let manager = snapshot_manager_for_view(&request.workspace_path, &request.remote_scope).await?;

    let stats = manager
        .get_session_file_diff_stats_before(
            &request.sessionId,
            &request.filePath,
            read.visible_turn_end(),
        )
        .await
        .map_err(|e| format!("Failed to get session file diff stats: {}", e))?;

    serde_json::to_value(&stats).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_operation_summary(
    runtime: State<'_, DesktopRuntimeContext>,
    request: GetOperationSummaryRequest,
) -> Result<serde_json::Value, String> {
    let read = begin_snapshot_history_read_for_scope(
        runtime.inner(),
        &request.workspace_path,
        &request.sessionId,
        &request.remote_scope,
    )
    .await?;
    let manager =
        snapshot_manager_for_recorded_operation(&request.workspace_path, &request.remote_scope)
            .await?;

    let summary = manager
        .get_operation_summary_before(
            &request.sessionId,
            &request.operationId,
            read.visible_turn_end(),
        )
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
    runtime: State<'_, DesktopRuntimeContext>,
    request: GetSessionFilesRequest,
) -> Result<serde_json::Value, String> {
    if request.remote_scope.declares_remote() || is_remote_path(&request.workspace_path).await {
        return Ok(serde_json::Value::Array(Vec::new()));
    }
    let read = begin_snapshot_history_read(
        runtime.inner(),
        &request.workspace_path,
        &request.session_id,
    )
    .await?;
    let manager = snapshot_manager_for_view(&request.workspace_path, &request.remote_scope).await?;
    let session = manager
        .get_session_before(&request.session_id, read.visible_turn_end())
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
    runtime: State<'_, DesktopRuntimeContext>,
    request: GetOperationSummaryRequest,
) -> Result<serde_json::Value, String> {
    ensure_local_snapshot_mutation_path(&request.workspace_path, &request.remote_scope).await?;
    ensure_local_runtime_ownership(runtime.inner(), &request.workspace_path).await?;
    let _history_mutation = begin_snapshot_history_mutation(
        runtime.inner(),
        &request.workspace_path,
        &request.sessionId,
    )
    .await?;
    let manager =
        ensure_snapshot_manager_ready_for(&request.workspace_path, "accept_operation").await?;

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
    runtime: State<'_, DesktopRuntimeContext>,
    request: GetOperationSummaryRequest,
) -> Result<serde_json::Value, String> {
    ensure_local_snapshot_mutation_path(&request.workspace_path, &request.remote_scope).await?;
    ensure_local_runtime_ownership(runtime.inner(), &request.workspace_path).await?;
    let _history_mutation = begin_snapshot_history_mutation(
        runtime.inner(),
        &request.workspace_path,
        &request.sessionId,
    )
    .await?;
    let manager =
        ensure_snapshot_manager_ready_for(&request.workspace_path, "reject_operation").await?;

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
    runtime: State<'_, DesktopRuntimeContext>,
    request: GetSessionStatsRequest,
) -> Result<serde_json::Value, String> {
    if request.remote_scope.declares_remote() || is_remote_path(&request.workspace_path).await {
        return Ok(serde_json::json!({
            "session_id": request.session_id,
            "total_files": 0,
            "total_turns": 0,
            "total_changes": 0
        }));
    }
    let read = begin_snapshot_history_read(
        runtime.inner(),
        &request.workspace_path,
        &request.session_id,
    )
    .await?;
    let manager = snapshot_manager_for_view(&request.workspace_path, &request.remote_scope).await?;
    manager
        .get_session_stats_before(&request.session_id, read.visible_turn_end())
        .await
        .map_err(|error| format!("Failed to get session stats: {error}"))
}

#[tauri::command]
pub async fn get_snapshot_system_stats(
    request: SnapshotWorkspaceRequest,
) -> Result<serde_json::Value, String> {
    let manager = snapshot_manager_for_view(&request.workspace_path, &request.remote_scope).await?;

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
    if request.remote_scope.declares_remote() || is_remote_path(&request.workspace_path).await {
        return Ok(vec![]);
    }
    let manager = snapshot_manager_for_view(&request.workspace_path, &request.remote_scope).await?;

    manager
        .list_sessions()
        .await
        .map_err(|e| format!("Failed to list snapshot sessions: {}", e))
}

#[tauri::command]
pub async fn check_git_isolation(
    request: SnapshotWorkspaceRequest,
) -> Result<serde_json::Value, String> {
    let manager = snapshot_manager_for_view(&request.workspace_path, &request.remote_scope).await?;

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
    runtime: State<'_, DesktopRuntimeContext>,
    request: GetFileChangeHistoryRequest,
) -> Result<serde_json::Value, String> {
    if request.remote_scope.declares_remote() || is_remote_path(&request.workspace_path).await {
        return Ok(serde_json::Value::Array(Vec::new()));
    }
    let manager = snapshot_manager_for_view(&request.workspace_path, &request.remote_scope).await?;

    let file_path = PathBuf::from(&request.file_path);
    let session_ids = manager
        .list_sessions()
        .await
        .map_err(|error| format!("Failed to list snapshot sessions: {error}"))?;
    let mut changes = Vec::new();
    for session_id in session_ids {
        let read =
            begin_snapshot_history_read(runtime.inner(), &request.workspace_path, &session_id)
                .await?;
        let session = manager
            .get_session_before(&session_id, read.visible_turn_end())
            .await
            .map_err(|error| format!("Failed to get file change history: {error}"))?;
        changes.extend(
            session
                .operations
                .into_iter()
                .filter(|operation| operation.file_path == file_path)
                .map(|operation| FileChangeEntry {
                    session_id: operation.session_id,
                    turn_index: operation.turn_index,
                    snapshot_id: operation
                        .before_snapshot_id
                        .unwrap_or_else(|| format!("empty_snapshot_{}", operation.operation_id)),
                    timestamp: operation.timestamp,
                    operation_type: operation.operation_type,
                    tool_name: operation.tool_context.tool_name,
                }),
        );
        drop(read);
    }
    changes.sort_by_key(|entry| (entry.session_id.clone(), entry.turn_index, entry.timestamp));

    serde_json::to_value(changes).map_err(|e| format!("Serialization failed: {}", e))
}

#[tauri::command]
pub async fn get_all_modified_files(
    runtime: State<'_, DesktopRuntimeContext>,
    request: GetAllModifiedFilesRequest,
) -> Result<Vec<String>, String> {
    if request.remote_scope.declares_remote() || is_remote_path(&request.workspace_path).await {
        return Ok(vec![]);
    }
    let manager = snapshot_manager_for_view(&request.workspace_path, &request.remote_scope).await?;

    let session_ids = manager
        .list_sessions()
        .await
        .map_err(|error| format!("Failed to list snapshot sessions: {error}"))?;
    let mut files = BTreeSet::new();
    for session_id in session_ids {
        let read =
            begin_snapshot_history_read(runtime.inner(), &request.workspace_path, &session_id)
                .await?;
        files.extend(
            manager
                .get_session_files_before(&session_id, read.visible_turn_end())
                .await
                .map_err(|error| format!("Failed to get modified files: {error}"))?,
        );
        drop(read);
    }

    Ok(files
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect())
}

#[tauri::command]
pub async fn get_baseline_snapshot_diff(
    request: GetBaselineSnapshotDiffRequest,
) -> Result<serde_json::Value, String> {
    let manager = snapshot_manager_for_view(&request.workspace_path, &request.remote_scope).await?;

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

#[cfg(test)]
mod tests {
    use super::{
        ensure_complete_rollback_supported, ensure_local_snapshot_mutation_path,
        get_snapshot_manager_for_workspace, snapshot_manager_for_view, SnapshotRemoteScope,
    };

    #[tokio::test]
    async fn incomplete_remote_operation_scope_never_opens_colliding_local_history() {
        let workspace = tempfile::tempdir().unwrap();
        for scope in [
            SnapshotRemoteScope {
                remote_connection_id: Some("connection-1".into()),
                remote_ssh_host: None,
            },
            SnapshotRemoteScope {
                remote_connection_id: None,
                remote_ssh_host: Some("host-1".into()),
            },
        ] {
            let result = super::snapshot_manager_for_recorded_operation(
                &workspace.path().to_string_lossy(),
                &scope,
            )
            .await;
            let error = result.err().expect("incomplete identity must fail closed");
            assert!(error.contains("saved connection id and host"));
            assert!(get_snapshot_manager_for_workspace(workspace.path()).is_none());
            assert_eq!(std::fs::read_dir(workspace.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn operation_scope_keeps_legacy_payload_compatibility_and_round_trips_identity() {
        let legacy: super::GetOperationSummaryRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "session-1", "operationId": "operation-1", "workspacePath": "/srv/project",
        }))
        .unwrap();
        assert!(!legacy.remote_scope.declares_remote());
        let remote: super::GetOperationSummaryRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "session-1", "operationId": "operation-1", "workspacePath": "/srv/project",
            "remoteConnectionId": "ssh:user@host:22", "remoteSshHost": "host",
        }))
        .unwrap();
        let restored: super::GetOperationSummaryRequest =
            serde_json::from_value(serde_json::to_value(remote).unwrap()).unwrap();
        assert_eq!(
            restored.remote_scope.remote_connection_id.as_deref(),
            Some("ssh:user@host:22")
        );
        assert_eq!(
            restored.remote_scope.remote_ssh_host.as_deref(),
            Some("host")
        );
        assert_eq!(restored.workspace_path, "/srv/project");
    }

    #[test]
    fn targeted_rollback_request_preserves_stable_identity_and_remote_facts() {
        let request: bitfun_runtime_ports::AgentSessionRollbackToTurnRequest =
            serde_json::from_value(serde_json::json!({
                "sessionId": "remote-session",
                "targetTurnId": "turn-7",
                "expectedStorageTurnIndex": 7,
                "expectedCatalogRevision": "catalog-3",
                "workspacePath": "/srv/project",
                "remoteConnectionId": "ssh:user@example.com:22",
                "remoteSshHost": "example.com"
            }))
            .expect("deserialize targeted rollback request");

        assert_eq!(request.target_turn_id, "turn-7");
        assert_eq!(request.expected_storage_turn_index, Some(7));
        assert_eq!(
            request.expected_catalog_revision.as_deref(),
            Some("catalog-3")
        );
        assert_eq!(
            request.remote_connection_id.as_deref(),
            Some("ssh:user@example.com:22")
        );
        assert_eq!(request.remote_ssh_host.as_deref(), Some("example.com"));
    }

    #[tokio::test]
    async fn remote_snapshot_mutation_is_rejected_before_writer_initialization() {
        let workspace = tempfile::tempdir().expect("create workspace");
        let workspace_path = workspace.path().to_string_lossy().to_string();
        let remote =
            bitfun_core::service::remote_ssh::workspace_state::init_remote_workspace_manager();
        remote
            .register_remote_workspace(
                workspace_path.clone(),
                "snapshot-remote-test".to_string(),
                "Snapshot remote test".to_string(),
                "snapshot-test-host".to_string(),
            )
            .await;

        let error = ensure_local_snapshot_mutation_path(&workspace_path, &Default::default())
            .await
            .expect_err("remote mutation must fail closed");

        assert!(error.contains("not supported for remote workspace"));
        assert!(get_snapshot_manager_for_workspace(workspace.path()).is_none());
        assert_eq!(
            std::fs::read_dir(workspace.path())
                .expect("workspace remains readable")
                .count(),
            0
        );
        remote
            .unregister_remote_workspace("snapshot-remote-test", &workspace_path)
            .await;

        let disconnected_scope = SnapshotRemoteScope {
            remote_connection_id: Some("snapshot-test-connection".to_string()),
            remote_ssh_host: Some("snapshot-test-host".to_string()),
        };
        let disconnected_error =
            ensure_local_snapshot_mutation_path(&workspace_path, &disconnected_scope)
                .await
                .expect_err("structured session facts remain remote after registry removal");
        assert!(disconnected_error.contains("not supported for remote workspace"));
        assert!(get_snapshot_manager_for_workspace(workspace.path()).is_none());
    }

    #[tokio::test]
    async fn remote_complete_rollback_reports_missing_file_snapshot_coverage() {
        let scope = SnapshotRemoteScope {
            remote_connection_id: Some("connection-1".to_string()),
            remote_ssh_host: Some("example.com".to_string()),
        };

        let error = ensure_complete_rollback_supported("/root/repos", &scope, None)
            .await
            .expect_err("remote rollback must fail before changing files or history");

        assert_eq!(
            error,
            "Complete rollback is not supported for remote workspaces because complete file snapshot coverage is unavailable. No workspace files or session messages were changed: /root/repos"
        );
    }

    #[tokio::test]
    async fn explicit_local_rollback_identity_wins_over_a_remote_path_collision() {
        let workspace = tempfile::tempdir().expect("create local workspace");
        let workspace_path = workspace.path().to_string_lossy().to_string();
        let remote =
            bitfun_core::service::remote_ssh::workspace_state::init_remote_workspace_manager();
        remote
            .register_remote_workspace(
                workspace_path.clone(),
                "rollback-path-collision".to_string(),
                "Rollback collision test".to_string(),
                "remote.example".to_string(),
            )
            .await;

        let legacy_error =
            ensure_complete_rollback_supported(&workspace_path, &Default::default(), None)
                .await
                .expect_err("legacy path-only requests must retain remote fallback behavior");
        assert!(legacy_error.contains("not supported for remote workspaces"));

        ensure_complete_rollback_supported(
            &workspace_path,
            &Default::default(),
            Some(bitfun_runtime_ports::AgentSessionWorkspaceLocation::Local),
        )
        .await
        .expect("explicit local identity must disambiguate the registered remote path");

        remote
            .unregister_remote_workspace("rollback-path-collision", &workspace_path)
            .await;
    }

    #[test]
    fn rollback_commands_reject_remote_workspaces_before_local_side_effects() {
        let source = include_str!("snapshot_service.rs");
        let rollback_session = source
            .split_once("pub async fn rollback_session")
            .expect("rollback_session remains present")
            .1
            .split_once("pub async fn rollback_session_to_turn")
            .expect("targeted rollback remains present")
            .0;
        let targeted_rollback = source
            .split_once("pub async fn rollback_session_to_turn")
            .expect("targeted rollback remains present")
            .1
            .split_once("pub async fn accept_session")
            .expect("accept_session remains present")
            .0;

        let assert_remote_guard_precedes = |body: &str, side_effect: &str| {
            let guard = body
                .find("ensure_complete_rollback_supported")
                .expect("complete rollback guard remains present");
            let effect = body
                .find(side_effect)
                .unwrap_or_else(|| panic!("expected side effect remains present: {side_effect}"));
            assert!(guard < effect, "remote guard must precede {side_effect}");
        };

        assert_remote_guard_precedes(rollback_session, "ensure_local_runtime_ownership");
        assert_remote_guard_precedes(rollback_session, "ensure_snapshot_manager_ready_for");
        assert_remote_guard_precedes(targeted_rollback, "ensure_local_runtime_ownership");
        assert_remote_guard_precedes(targeted_rollback, "rollback_session_to_turn");
    }

    #[test]
    fn snapshot_mutators_share_session_revert_admission() {
        let source = include_str!("snapshot_service.rs");
        for (command, next_command) in [
            ("rollback_session", "rollback_session_to_turn"),
            ("accept_session", "accept_file"),
            ("accept_file", "reject_file"),
            ("reject_file", "get_session_files"),
            ("accept_operation", "reject_operation"),
            ("reject_operation", "get_session_stats"),
        ] {
            let body = source
                .split_once(&format!("pub async fn {command}"))
                .unwrap_or_else(|| panic!("{command} remains present"))
                .1
                .split_once(&format!("pub async fn {next_command}"))
                .unwrap_or_else(|| panic!("{next_command} remains present"))
                .0;
            assert!(
                body.contains("begin_snapshot_history_mutation"),
                "{command} must commit staged Session undo under the shared mutation owner"
            );
        }

        let targeted_rollback = source
            .split_once("pub async fn rollback_session_to_turn")
            .expect("targeted rollback remains present")
            .1
            .split_once("pub async fn accept_session")
            .expect("accept_session remains present")
            .0;
        assert!(
            targeted_rollback.contains(".rollback_session_to_turn(request)"),
            "targeted rollback must delegate admission to the Agent Session transaction"
        );
        assert!(
            !targeted_rollback.contains("begin_snapshot_history_mutation"),
            "targeted rollback must not reacquire the Session mutation owned by the Agent Session transaction"
        );
    }

    #[tokio::test]
    async fn snapshot_view_does_not_initialize_a_writer() {
        let workspace = tempfile::tempdir().expect("create workspace");
        assert!(get_snapshot_manager_for_workspace(workspace.path()).is_none());

        snapshot_manager_for_view(
            &workspace.path().to_string_lossy(),
            &SnapshotRemoteScope::default(),
        )
        .await
        .expect("an empty read-only view remains available");
        assert!(get_snapshot_manager_for_workspace(workspace.path()).is_none());
    }

    #[tokio::test]
    async fn snapshot_view_rejects_structured_remote_scope_after_registry_disconnect() {
        let workspace = tempfile::tempdir().expect("create colliding local workspace");
        let scope = SnapshotRemoteScope {
            remote_connection_id: Some("connection-1".to_string()),
            remote_ssh_host: Some("host-1".to_string()),
        };

        let error = match snapshot_manager_for_view(&workspace.path().to_string_lossy(), &scope)
            .await
        {
            Ok(_) => panic!("structured remote scope must not read the colliding local Snapshot"),
            Err(error) => error,
        };

        assert!(error.contains("snapshot_remote_workspace_unavailable"));
        assert!(get_snapshot_manager_for_workspace(workspace.path()).is_none());
    }
}
