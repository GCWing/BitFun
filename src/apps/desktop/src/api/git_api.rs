//! Git API

use crate::api::app_state::AppState;
use bitfun_core::infrastructure::storage::StorageOptions;
use bitfun_core::service::git::{
    GitAddParams, GitCommitParams, GitDiffParams, GitLogParams, GitPullParams, GitPushParams,
    GitService,
};
use bitfun_core::service::git::{
    GitBranch, GitCommit, GitOperationResult, GitRepository, GitStatus,
};
use log::{error, info};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRepositoryRequest {
    pub repository_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBranchesRequest {
    pub repository_path: String,
    pub include_remote: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitsRequest {
    pub repository_path: String,
    pub params: Option<GitLogParams>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitAddFilesRequest {
    pub repository_path: String,
    pub params: GitAddParams,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitRequest {
    pub repository_path: String,
    pub params: GitCommitParams,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPushRequest {
    pub repository_path: String,
    pub params: GitPushParams,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPullRequest {
    pub repository_path: String,
    pub params: GitPullParams,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCheckoutBranchRequest {
    pub repository_path: String,
    pub branch_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCreateBranchRequest {
    pub repository_path: String,
    pub branch_name: String,
    pub start_point: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDeleteBranchRequest {
    pub repository_path: String,
    pub branch_name: String,
    pub force: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffRequest {
    pub repository_path: String,
    pub params: GitDiffParams,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitResetFilesRequest {
    pub repository_path: String,
    pub files: Vec<String>,
    pub staged: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitResetToCommitRequest {
    pub repository_path: String,
    pub commit_hash: String,
    pub mode: String, // "soft", "mixed", or "hard"
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitGetFileContentRequest {
    pub repository_path: String,
    pub file_path: String,
    pub commit: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCherryPickRequest {
    pub repository_path: String,
    pub commit_hash: String,
    pub no_commit: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitAddWorktreeRequest {
    pub repository_path: String,
    pub branch: String,
    pub create_branch: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRemoveWorktreeRequest {
    pub repository_path: String,
    pub worktree_path: String,
    pub force: Option<bool>,
}

#[tauri::command]
pub async fn git_is_repository(
    _state: State<'_, AppState>,
    request: GitRepositoryRequest,
) -> Result<bool, String> {
    GitService::is_repository(&request.repository_path)
        .await
        .map_err(|e| {
            error!(
                "Failed to check Git repository: path={}, error={}",
                request.repository_path, e
            );
            format!("Failed to check Git repository: {}", e)
        })
}

#[tauri::command]
pub async fn git_get_repository(
    _state: State<'_, AppState>,
    request: GitRepositoryRequest,
) -> Result<GitRepository, String> {
    GitService::get_repository(&request.repository_path)
        .await
        .map_err(|e| {
            error!(
                "Failed to get Git repository info: path={}, error={}",
                request.repository_path, e
            );
            format!("Failed to get Git repository info: {}", e)
        })
}

#[tauri::command]
pub async fn git_get_status(
    _state: State<'_, AppState>,
    request: GitRepositoryRequest,
) -> Result<GitStatus, String> {
    GitService::get_status(&request.repository_path)
        .await
        .map_err(|e| {
            error!(
                "Failed to get Git status: path={}, error={}",
                request.repository_path, e
            );
            format!("Failed to get Git status: {}", e)
        })
}

#[tauri::command]
pub async fn git_get_branches(
    _state: State<'_, AppState>,
    request: GitBranchesRequest,
) -> Result<Vec<GitBranch>, String> {
    let include_remote = request.include_remote.unwrap_or(false);
    GitService::get_branches(&request.repository_path, include_remote)
        .await
        .map_err(|e| {
            error!(
                "Failed to get Git branches: path={}, include_remote={}, error={}",
                request.repository_path, include_remote, e
            );
            format!("Failed to get Git branches: {}", e)
        })
}

#[tauri::command]
pub async fn git_get_enhanced_branches(
    _state: State<'_, AppState>,
    request: GitBranchesRequest,
) -> Result<Vec<GitBranch>, String> {
    let include_remote = request.include_remote.unwrap_or(false);
    GitService::get_enhanced_branches(&request.repository_path, include_remote)
        .await
        .map_err(|e| {
            error!(
                "Failed to get enhanced Git branches: path={}, include_remote={}, error={}",
                request.repository_path, include_remote, e
            );
            format!("Failed to get enhanced Git branches: {}", e)
        })
}

#[tauri::command]
pub async fn git_get_commits(
    _state: State<'_, AppState>,
    request: GitCommitsRequest,
) -> Result<Vec<GitCommit>, String> {
    let params = request.params.unwrap_or_default();
    GitService::get_commits(&request.repository_path, params)
        .await
        .map_err(|e| {
            error!(
                "Failed to get Git commits: path={}, error={}",
                request.repository_path, e
            );
            format!("Failed to get Git commits: {}", e)
        })
}

#[tauri::command]
pub async fn git_add_files(
    _state: State<'_, AppState>,
    request: GitAddFilesRequest,
) -> Result<GitOperationResult, String> {
    GitService::add_files(&request.repository_path, request.params)
        .await
        .map_err(|e| {
            error!(
                "Failed to add files: path={}, error={}",
                request.repository_path, e
            );
            format!("Failed to add files: {}", e)
        })
}

#[tauri::command]
pub async fn git_commit(
    _state: State<'_, AppState>,
    request: GitCommitRequest,
) -> Result<GitOperationResult, String> {
    GitService::commit(&request.repository_path, request.params)
        .await
        .map_err(|e| {
            error!(
                "Failed to commit: path={}, error={}",
                request.repository_path, e
            );
            format!("Failed to commit: {}", e)
        })
}

#[tauri::command]
pub async fn git_push(
    _state: State<'_, AppState>,
    request: GitPushRequest,
) -> Result<GitOperationResult, String> {
    GitService::push(&request.repository_path, request.params)
        .await
        .map_err(|e| {
            error!(
                "Failed to push: path={}, error={}",
                request.repository_path, e
            );
            format!("Failed to push: {}", e)
        })
}

#[tauri::command]
pub async fn git_pull(
    _state: State<'_, AppState>,
    request: GitPullRequest,
) -> Result<GitOperationResult, String> {
    GitService::pull(&request.repository_path, request.params)
        .await
        .map_err(|e| {
            error!(
                "Failed to pull: path={}, error={}",
                request.repository_path, e
            );
            format!("Failed to pull: {}", e)
        })
}

#[tauri::command]
pub async fn git_checkout_branch(
    _state: State<'_, AppState>,
    request: GitCheckoutBranchRequest,
) -> Result<GitOperationResult, String> {
    GitService::checkout_branch(&request.repository_path, &request.branch_name)
        .await
        .map_err(|e| {
            error!(
                "Failed to checkout branch: path={}, branch={}, error={}",
                request.repository_path, request.branch_name, e
            );
            format!("Failed to checkout branch: {}", e)
        })
}

#[tauri::command]
pub async fn git_create_branch(
    _state: State<'_, AppState>,
    request: GitCreateBranchRequest,
) -> Result<GitOperationResult, String> {
    GitService::create_branch(
        &request.repository_path,
        &request.branch_name,
        request.start_point.as_deref(),
    )
    .await
    .map_err(|e| {
        error!(
            "Failed to create branch: path={}, branch={}, error={}",
            request.repository_path, request.branch_name, e
        );
        format!("Failed to create branch: {}", e)
    })
}

#[tauri::command]
pub async fn git_delete_branch(
    _state: State<'_, AppState>,
    request: GitDeleteBranchRequest,
) -> Result<GitOperationResult, String> {
    let force = request.force.unwrap_or(false);
    GitService::delete_branch(&request.repository_path, &request.branch_name, force)
        .await
        .map_err(|e| {
            error!(
                "Failed to delete branch: path={}, branch={}, force={}, error={}",
                request.repository_path, request.branch_name, force, e
            );
            format!("Failed to delete branch: {}", e)
        })
}

#[tauri::command]
pub async fn git_get_diff(
    _state: State<'_, AppState>,
    request: GitDiffRequest,
) -> Result<String, String> {
    GitService::get_diff(&request.repository_path, &request.params)
        .await
        .map_err(|e| {
            error!(
                "Failed to get Git diff: path={}, error={}",
                request.repository_path, e
            );
            format!("Failed to get Git diff: {}", e)
        })
}

#[tauri::command]
pub async fn git_reset_files(
    _state: State<'_, AppState>,
    request: GitResetFilesRequest,
) -> Result<GitOperationResult, String> {
    let staged = request.staged.unwrap_or(false);

    info!(
        "Resetting files in '{}' (staged: {}): {:?}",
        request.repository_path, staged, request.files
    );

    GitService::reset_files(&request.repository_path, &request.files, staged)
        .await
        .map(|output| GitOperationResult {
            success: true,
            data: None,
            error: None,
            output: Some(output),
            duration: None,
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_get_file_content(
    _state: State<'_, AppState>,
    request: GitGetFileContentRequest,
) -> Result<String, String> {
    info!(
        "Getting file content for '{}' at commit '{:?}' in repo '{}'",
        request.file_path, request.commit, request.repository_path
    );

    let content = GitService::get_file_content(
        &request.repository_path,
        &request.file_path,
        request.commit.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(content)
}

#[tauri::command]
pub async fn git_reset_to_commit(
    _state: State<'_, AppState>,
    request: GitResetToCommitRequest,
) -> Result<GitOperationResult, String> {
    info!(
        "Resetting to commit '{}' with mode '{}' in repo '{}'",
        request.commit_hash, request.mode, request.repository_path
    );

    GitService::reset_to_commit(
        &request.repository_path,
        &request.commit_hash,
        &request.mode,
    )
    .await
    .map_err(|e| {
        error!(
            "Failed to reset to commit: path={}, commit={}, mode={}, error={}",
            request.repository_path, request.commit_hash, request.mode, e
        );
        format!("Failed to reset: {}", e)
    })
}

#[tauri::command]
pub async fn git_get_graph(
    _state: State<'_, AppState>,
    repository_path: String,
    max_count: Option<usize>,
    branch_name: Option<String>,
) -> Result<bitfun_core::service::git::GitGraph, String> {
    info!(
        "Getting git graph: repository_path={}, max_count={:?}, branch_name={:?}",
        repository_path, max_count, branch_name
    );

    GitService::get_git_graph_for_branch(&repository_path, max_count, branch_name.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_cherry_pick(
    _state: State<'_, AppState>,
    request: GitCherryPickRequest,
) -> Result<GitOperationResult, String> {
    let no_commit = request.no_commit.unwrap_or(false);

    info!(
        "Cherry-picking commit '{}' in repo '{}' (no_commit: {})",
        request.commit_hash, request.repository_path, no_commit
    );

    GitService::cherry_pick(&request.repository_path, &request.commit_hash, no_commit)
        .await
        .map_err(|e| {
            error!(
                "Failed to cherry-pick: path={}, commit={}, no_commit={}, error={}",
                request.repository_path, request.commit_hash, no_commit, e
            );
            format!("Failed to cherry-pick: {}", e)
        })
}

#[tauri::command]
pub async fn git_cherry_pick_abort(
    _state: State<'_, AppState>,
    request: GitRepositoryRequest,
) -> Result<GitOperationResult, String> {
    info!("Aborting cherry-pick in repo '{}'", request.repository_path);

    GitService::cherry_pick_abort(&request.repository_path)
        .await
        .map_err(|e| {
            error!(
                "Failed to abort cherry-pick: path={}, error={}",
                request.repository_path, e
            );
            format!("Failed to abort cherry-pick: {}", e)
        })
}

#[tauri::command]
pub async fn git_cherry_pick_continue(
    _state: State<'_, AppState>,
    request: GitRepositoryRequest,
) -> Result<GitOperationResult, String> {
    info!(
        "Continuing cherry-pick in repo '{}'",
        request.repository_path
    );

    GitService::cherry_pick_continue(&request.repository_path)
        .await
        .map_err(|e| {
            error!(
                "Failed to continue cherry-pick: path={}, error={}",
                request.repository_path, e
            );
            format!("Failed to continue cherry-pick: {}", e)
        })
}

#[tauri::command]
pub async fn git_list_worktrees(
    _state: State<'_, AppState>,
    request: GitRepositoryRequest,
) -> Result<Vec<bitfun_core::service::git::GitWorktreeInfo>, String> {
    info!("Listing worktrees for '{}'", request.repository_path);

    GitService::list_worktrees(&request.repository_path)
        .await
        .map_err(|e| {
            error!(
                "Failed to list worktrees: path={}, error={}",
                request.repository_path, e
            );
            format!("Failed to list worktrees: {}", e)
        })
}

#[tauri::command]
pub async fn git_add_worktree(
    _state: State<'_, AppState>,
    request: GitAddWorktreeRequest,
) -> Result<bitfun_core::service::git::GitWorktreeInfo, String> {
    let create_branch = request.create_branch.unwrap_or(false);
    info!(
        "Adding worktree for branch '{}' in '{}' (create_branch: {})",
        request.branch, request.repository_path, create_branch
    );

    GitService::add_worktree(&request.repository_path, &request.branch, create_branch)
        .await
        .map_err(|e| {
            error!(
                "Failed to add worktree: path={}, branch={}, create_branch={}, error={}",
                request.repository_path, request.branch, create_branch, e
            );
            format!("Failed to add worktree: {}", e)
        })
}

#[tauri::command]
pub async fn git_remove_worktree(
    _state: State<'_, AppState>,
    request: GitRemoveWorktreeRequest,
) -> Result<GitOperationResult, String> {
    let force = request.force.unwrap_or(false);
    info!(
        "Removing worktree '{}' from '{}' (force: {})",
        request.worktree_path, request.repository_path, force
    );

    GitService::remove_worktree(&request.repository_path, &request.worktree_path, force)
        .await
        .map_err(|e| {
            error!(
                "Failed to remove worktree: path={}, worktree_path={}, force={}, error={}",
                request.repository_path, request.worktree_path, force, e
            );
            format!("Failed to remove worktree: {}", e)
        })
}

// MARK: Git Repo History

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRepoHistory {
    pub url: String,
    pub last_used: String,
    pub local_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRepoHistoryData {
    pub repos: Vec<GitRepoHistory>,
    pub saved_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveGitRepoHistoryRequest {
    pub repos: Vec<GitRepoHistory>,
}

#[tauri::command]
pub async fn save_git_repo_history(
    state: State<'_, AppState>,
    request: SaveGitRepoHistoryRequest,
) -> Result<(), String> {
    let workspace_service = &state.workspace_service;
    let persistence = workspace_service.persistence();

    let data = GitRepoHistoryData {
        repos: request.repos,
        saved_at: chrono::Utc::now().to_rfc3339(),
    };

    persistence
        .save_json("git_repo_history", &data, StorageOptions::default())
        .await
        .map_err(|e| {
            error!("Failed to save git repo history: {}", e);
            format!("Failed to save git repo history: {}", e)
        })
}

#[tauri::command]
pub async fn load_git_repo_history(
    state: State<'_, AppState>,
) -> Result<Vec<GitRepoHistory>, String> {
    let workspace_service = &state.workspace_service;
    let persistence = workspace_service.persistence();

    let data: Option<GitRepoHistoryData> = persistence
        .load_json("git_repo_history")
        .await
        .map_err(|e| {
            error!("Failed to load git repo history: {}", e);
            format!("Failed to load git repo history: {}", e)
        })?;

    match data {
        Some(data) => Ok(data.repos),
        None => Ok(Vec::new()),
    }
}
