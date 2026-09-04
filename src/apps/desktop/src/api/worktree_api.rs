//! Thin desktop adapter for product-owned managed worktrees.

use openbitfun_core::service::remote_ssh::lookup_remote_connection;
use openbitfun_core::service::worktree::{
    WorktreeCreateBranchRequest, WorktreeCreateRequest, WorktreeCreateResult, WorktreeListRequest,
    WorktreeMutationResult, WorktreeProjectListRequest, WorktreeProjectSummary,
    WorktreePromoteRequest, WorktreeRecreateRequest, WorktreeRemoveRequest, WorktreeRemoveResult,
    WorktreeService, WorktreeSessionBindingRequest, WorktreeSessionBindingResult,
};
use openbitfun_core_types::{WorktreeError, WorktreeErrorCode, WorktreeSummary};

fn remote_unsupported() -> WorktreeError {
    WorktreeError {
        code: WorktreeErrorCode::RemoteUnsupported,
        message: "Managed worktrees are not supported for remote SSH workspaces yet".to_string(),
        recovery_path: None,
    }
}

async fn ensure_local(project_workspace_path: &str) -> Result<(), WorktreeError> {
    if lookup_remote_connection(project_workspace_path)
        .await
        .is_some()
    {
        Err(remote_unsupported())
    } else {
        Ok(())
    }
}

#[tauri::command]
pub async fn worktree_list(
    request: WorktreeListRequest,
) -> Result<Vec<WorktreeSummary>, WorktreeError> {
    ensure_local(&request.project_workspace_path).await?;
    WorktreeService::list(request).await
}

#[tauri::command]
pub async fn worktree_list_projects(
    request: WorktreeProjectListRequest,
) -> Result<Vec<WorktreeProjectSummary>, WorktreeError> {
    WorktreeService::list_projects(request).await
}

#[tauri::command]
pub async fn worktree_create(
    request: WorktreeCreateRequest,
) -> Result<WorktreeCreateResult, WorktreeError> {
    ensure_local(&request.project_workspace_path).await?;
    WorktreeService::create(request).await
}

#[tauri::command]
pub async fn worktree_create_branch(
    request: WorktreeCreateBranchRequest,
) -> Result<WorktreeMutationResult, WorktreeError> {
    ensure_local(&request.project_workspace_path).await?;
    WorktreeService::create_branch(request).await
}

#[tauri::command]
pub async fn worktree_promote(
    request: WorktreePromoteRequest,
) -> Result<WorktreeMutationResult, WorktreeError> {
    ensure_local(&request.project_workspace_path).await?;
    WorktreeService::promote(request).await
}

#[tauri::command]
pub async fn worktree_remove(
    request: WorktreeRemoveRequest,
) -> Result<WorktreeRemoveResult, WorktreeError> {
    ensure_local(&request.project_workspace_path).await?;
    WorktreeService::remove(request).await
}

/// Toggle worktree isolation for a single session. The optional project path
/// lets the product layer locate view-only persisted sessions; remote checks
/// and repository resolution remain in that shared layer.
#[tauri::command]
pub async fn worktree_bind_session(
    request: WorktreeSessionBindingRequest,
) -> Result<WorktreeSessionBindingResult, WorktreeError> {
    WorktreeService::bind_session(request).await
}

#[tauri::command]
pub async fn worktree_recreate(
    request: WorktreeRecreateRequest,
) -> Result<WorktreeMutationResult, WorktreeError> {
    ensure_local(&request.project_workspace_path).await?;
    WorktreeService::recreate(request).await
}
