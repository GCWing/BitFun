//! Git API

use crate::api::app_state::AppState;
use bitfun_core::infrastructure::storage::StorageOptions;
use bitfun_core::service::git::{
    build_git_changed_files_args, build_git_diff_args, GitAddParams, GitChangedFile,
    GitChangedFileStatus, GitChangedFilesParams, GitCommitParams, GitDiffParams, GitFileStatus,
    GitLogParams, GitPullParams, GitPushParams, GitService,
};
use bitfun_core::service::git::{
    GitBranch, GitCommit, GitOperationResult, GitRepository, GitStatus,
};
use bitfun_core::service::remote_ssh::{lookup_remote_connection, normalize_remote_workspace_path};
use log::{error, info};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone)]
struct RemoteGitTarget {
    connection_id: String,
    repository_path: String,
}

#[derive(Debug)]
struct RemoteGitOutput {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

async fn resolve_remote_git_target(repository_path: &str) -> Option<RemoteGitTarget> {
    let entry = lookup_remote_connection(repository_path).await?;
    Some(RemoteGitTarget {
        connection_id: entry.connection_id,
        repository_path: normalize_remote_workspace_path(repository_path),
    })
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '-' | '_' | ':' | '=' | '@'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn build_remote_git_command(repository_path: &str, args: &[String]) -> String {
    let mut parts = vec![
        "git".to_string(),
        "-C".to_string(),
        shell_quote(repository_path),
        "--no-pager".to_string(),
    ];
    parts.extend(args.iter().map(|arg| shell_quote(arg)));
    parts.join(" ")
}

async fn execute_remote_git_command(
    state: &AppState,
    target: &RemoteGitTarget,
    args: &[String],
) -> Result<RemoteGitOutput, String> {
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|e| e.to_string())?;
    let command = build_remote_git_command(&target.repository_path, args);
    let (stdout, stderr, exit_code) = manager
        .execute_command(&target.connection_id, &command)
        .await
        .map_err(|e| e.to_string())?;

    Ok(RemoteGitOutput {
        stdout,
        stderr,
        exit_code,
    })
}

async fn execute_remote_git_success(
    state: &AppState,
    target: &RemoteGitTarget,
    args: &[String],
) -> Result<String, String> {
    let output = execute_remote_git_command(state, target, args).await?;
    if output.exit_code == 0 {
        Ok(output.stdout)
    } else {
        let error = if output.stderr.trim().is_empty() {
            output.stdout
        } else {
            output.stderr
        };
        Err(error.trim().to_string())
    }
}

async fn execute_remote_git_operation(
    state: &AppState,
    target: &RemoteGitTarget,
    args: &[String],
) -> Result<GitOperationResult, String> {
    let output = execute_remote_git_command(state, target, args).await?;
    let success = output.exit_code == 0;
    let error = (!success).then(|| {
        if output.stderr.trim().is_empty() {
            output.stdout.trim().to_string()
        } else {
            output.stderr.trim().to_string()
        }
    });

    Ok(GitOperationResult {
        success,
        data: Some(serde_json::json!({
            "remoteExecution": true,
            "exitCode": output.exit_code,
        })),
        error,
        output: Some(output.stdout),
        duration: None,
    })
}

fn parse_remote_status_line(
    line: &str,
) -> Option<(String, String, Option<String>, Option<String>)> {
    if line.len() < 4 {
        return None;
    }

    let index = line.chars().next()?;
    let worktree = line.chars().nth(1)?;
    let path = line.get(3..)?.trim().to_string();
    if path.is_empty() {
        return None;
    }

    let index_status = (index != ' ' && index != '?').then(|| index.to_string());
    let workdir_status = (worktree != ' ' && worktree != '?').then(|| worktree.to_string());
    let status = if index == '?' && worktree == '?' {
        "?".to_string()
    } else {
        [index, worktree]
            .into_iter()
            .filter(|c| *c != ' ')
            .collect::<String>()
    };

    Some((path, status, index_status, workdir_status))
}

fn parse_remote_git_status(output: &str) -> GitStatus {
    let mut current_branch = "HEAD".to_string();
    let mut ahead = 0;
    let mut behind = 0;
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();

    for line in output.lines() {
        if let Some(branch) = line.strip_prefix("## ") {
            let mut branch_part = branch.split("...").next().unwrap_or(branch).trim();
            if let Some((name, _)) = branch_part.split_once(' ') {
                branch_part = name;
            }
            if !branch_part.is_empty() {
                current_branch = branch_part.to_string();
            }

            if let Some(meta_start) = branch.find('[') {
                if let Some(meta_end) = branch[meta_start + 1..].find(']') {
                    let meta = &branch[meta_start + 1..meta_start + 1 + meta_end];
                    for part in meta.split(',').map(str::trim) {
                        if let Some(value) = part.strip_prefix("ahead ") {
                            ahead = value.parse().unwrap_or(0);
                        } else if let Some(value) = part.strip_prefix("behind ") {
                            behind = value.parse().unwrap_or(0);
                        }
                    }
                }
            }
            continue;
        }

        let Some((path, status, index_status, workdir_status)) = parse_remote_status_line(line)
        else {
            continue;
        };

        if status == "?" {
            untracked.push(path);
            continue;
        }

        let file = GitFileStatus {
            path,
            status,
            index_status,
            workdir_status,
        };

        if file.index_status.is_some() {
            staged.push(file.clone());
        }
        if file.workdir_status.is_some() {
            unstaged.push(file);
        }
    }

    GitStatus {
        staged,
        unstaged,
        untracked,
        current_branch,
        ahead,
        behind,
    }
}

fn parse_remote_branches(output: &str) -> Vec<GitBranch> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(6, '\t');
            let current = fields.next()? == "*";
            let full_ref = fields.next()?;
            let name = fields.next()?.to_string();
            let upstream = fields
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let last_commit = fields
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let last_commit_date = fields
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let remote = full_ref.starts_with("refs/remotes/");

            Some(GitBranch {
                name,
                current,
                remote,
                upstream,
                ahead: 0,
                behind: 0,
                last_commit,
                last_commit_date: last_commit_date.clone(),
                base_branch: None,
                child_branches: None,
                merged_branches: None,
                branch_type: None,
                has_conflicts: None,
                can_merge: None,
                is_stale: None,
                merge_status: None,
                stats: None,
                created_at: None,
                last_activity_at: last_commit_date,
                tags: None,
                description: None,
                linked_issues: None,
            })
        })
        .collect()
}

fn parse_remote_commits(output: &str) -> Vec<GitCommit> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(6, '\t');
            let hash = fields.next()?.to_string();
            let short_hash = fields.next()?.to_string();
            let author = fields.next()?.to_string();
            let author_email = fields.next()?.to_string();
            let date = fields.next()?.to_string();
            let message = fields.next().unwrap_or_default().to_string();
            Some(GitCommit {
                hash,
                short_hash,
                message,
                author,
                author_email,
                date,
                parents: Vec::new(),
                additions: None,
                deletions: None,
                files_changed: None,
            })
        })
        .collect()
}

fn parse_remote_name_status_output(output: &str) -> Vec<GitChangedFile> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let raw_status = parts.next()?.trim();
            if raw_status.is_empty() {
                return None;
            }

            let status = match raw_status.chars().next().unwrap_or_default() {
                'A' => GitChangedFileStatus::Added,
                'M' => GitChangedFileStatus::Modified,
                'D' => GitChangedFileStatus::Deleted,
                'R' => GitChangedFileStatus::Renamed,
                'C' => GitChangedFileStatus::Copied,
                _ => GitChangedFileStatus::Unknown,
            };

            match status {
                GitChangedFileStatus::Renamed | GitChangedFileStatus::Copied => {
                    let old_path = parts.next()?.to_string();
                    let path = parts.next()?.to_string();
                    Some(GitChangedFile {
                        path,
                        old_path: Some(old_path),
                        status,
                    })
                }
                _ => {
                    let path = parts.next()?.to_string();
                    Some(GitChangedFile {
                        path,
                        old_path: None,
                        status,
                    })
                }
            }
        })
        .collect()
}

fn git_log_args(params: &GitLogParams) -> Vec<String> {
    let mut args = vec![
        "log".to_string(),
        format!("--max-count={}", params.max_count.unwrap_or(50)),
        "--format=%H%x09%h%x09%an%x09%ae%x09%ci%x09%s".to_string(),
    ];
    if let Some(skip) = params.skip {
        args.push(format!("--skip={skip}"));
    }
    if let Some(author) = params.author.as_deref().filter(|s| !s.trim().is_empty()) {
        args.push(format!("--author={author}"));
    }
    if let Some(grep) = params.grep.as_deref().filter(|s| !s.trim().is_empty()) {
        args.push(format!("--grep={grep}"));
    }
    if let Some(since) = params.since.as_deref().filter(|s| !s.trim().is_empty()) {
        args.push(format!("--since={since}"));
    }
    if let Some(until) = params.until.as_deref().filter(|s| !s.trim().is_empty()) {
        args.push(format!("--until={until}"));
    }
    args
}

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
pub struct GitChangedFilesRequest {
    pub repository_path: String,
    pub params: GitChangedFilesParams,
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
    state: State<'_, AppState>,
    request: GitRepositoryRequest,
) -> Result<bool, String> {
    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        let output = execute_remote_git_command(
            &state,
            &target,
            &["rev-parse".to_string(), "--is-inside-work-tree".to_string()],
        )
        .await?;
        return Ok(output.exit_code == 0 && output.stdout.trim() == "true");
    }

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
    state: State<'_, AppState>,
    request: GitRepositoryRequest,
) -> Result<GitRepository, String> {
    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        let current_branch = execute_remote_git_success(
            &state,
            &target,
            &["branch".to_string(), "--show-current".to_string()],
        )
        .await
        .map(|s| {
            let branch = s.trim();
            if branch.is_empty() {
                "HEAD".to_string()
            } else {
                branch.to_string()
            }
        })?;
        let remotes_output =
            execute_remote_git_success(&state, &target, &["remote".to_string()]).await?;
        let status = execute_remote_git_success(
            &state,
            &target,
            &["status".to_string(), "--porcelain".to_string()],
        )
        .await?;

        let name = target
            .repository_path
            .rsplit('/')
            .find(|part| !part.is_empty())
            .unwrap_or("/")
            .to_string();

        return Ok(GitRepository {
            path: target.repository_path,
            name,
            current_branch,
            is_bare: false,
            has_changes: !status.trim().is_empty(),
            remotes: remotes_output
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
        });
    }

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
    state: State<'_, AppState>,
    request: GitRepositoryRequest,
) -> Result<GitStatus, String> {
    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        let output = execute_remote_git_success(
            &state,
            &target,
            &[
                "status".to_string(),
                "--porcelain=v1".to_string(),
                "--branch".to_string(),
            ],
        )
        .await?;
        return Ok(parse_remote_git_status(&output));
    }

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
    state: State<'_, AppState>,
    request: GitBranchesRequest,
) -> Result<Vec<GitBranch>, String> {
    let include_remote = request.include_remote.unwrap_or(false);
    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        let mut args = vec![
            "for-each-ref".to_string(),
            "--format=%(if)%(HEAD)%(then)*%(else) %(end)%09%(refname)%09%(refname:short)%09%(upstream:short)%09%(objectname)%09%(committerdate:iso-strict)".to_string(),
            "refs/heads".to_string(),
        ];
        if include_remote {
            args.push("refs/remotes".to_string());
        }
        let output = execute_remote_git_success(&state, &target, &args).await?;
        return Ok(parse_remote_branches(&output));
    }

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
    state: State<'_, AppState>,
    request: GitBranchesRequest,
) -> Result<Vec<GitBranch>, String> {
    let include_remote = request.include_remote.unwrap_or(false);
    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        let mut args = vec![
            "for-each-ref".to_string(),
            "--format=%(if)%(HEAD)%(then)*%(else) %(end)%09%(refname)%09%(refname:short)%09%(upstream:short)%09%(objectname)%09%(committerdate:iso-strict)".to_string(),
            "refs/heads".to_string(),
        ];
        if include_remote {
            args.push("refs/remotes".to_string());
        }
        let output = execute_remote_git_success(&state, &target, &args).await?;
        return Ok(parse_remote_branches(&output));
    }

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
    state: State<'_, AppState>,
    request: GitCommitsRequest,
) -> Result<Vec<GitCommit>, String> {
    let params = request.params.unwrap_or_default();
    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        let output = execute_remote_git_success(&state, &target, &git_log_args(&params)).await?;
        return Ok(parse_remote_commits(&output));
    }

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
    state: State<'_, AppState>,
    request: GitAddFilesRequest,
) -> Result<GitOperationResult, String> {
    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        let mut args = vec!["add".to_string()];
        if request.params.all.unwrap_or(false) {
            args.push("-A".to_string());
        } else if request.params.update.unwrap_or(false) {
            args.push("-u".to_string());
        } else {
            args.extend(request.params.files);
        }
        return execute_remote_git_operation(&state, &target, &args).await;
    }

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
    state: State<'_, AppState>,
    request: GitCommitRequest,
) -> Result<GitOperationResult, String> {
    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        let mut args = vec![
            "commit".to_string(),
            "-m".to_string(),
            request.params.message.clone(),
        ];
        if request.params.amend.unwrap_or(false) {
            args.push("--amend".to_string());
        }
        if request.params.all.unwrap_or(false) {
            args.push("-a".to_string());
        }
        if request.params.no_verify.unwrap_or(false) {
            args.push("--no-verify".to_string());
        }
        if let Some(author) = request.params.author {
            args.push("--author".to_string());
            args.push(format!("{} <{}>", author.name, author.email));
        }
        return execute_remote_git_operation(&state, &target, &args).await;
    }

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
    state: State<'_, AppState>,
    request: GitPushRequest,
) -> Result<GitOperationResult, String> {
    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        let mut args = vec!["push".to_string()];
        if request.params.force.unwrap_or(false) {
            args.push("--force".to_string());
        }
        if request.params.set_upstream.unwrap_or(false) {
            args.push("-u".to_string());
        }
        if let Some(remote) = request.params.remote {
            args.push(remote);
        }
        if let Some(branch) = request.params.branch {
            args.push(branch);
        }
        return execute_remote_git_operation(&state, &target, &args).await;
    }

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
    state: State<'_, AppState>,
    request: GitPullRequest,
) -> Result<GitOperationResult, String> {
    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        let mut args = vec!["pull".to_string()];
        if request.params.rebase.unwrap_or(false) {
            args.push("--rebase".to_string());
        }
        if let Some(remote) = request.params.remote {
            args.push(remote);
        }
        if let Some(branch) = request.params.branch {
            args.push(branch);
        }
        return execute_remote_git_operation(&state, &target, &args).await;
    }

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
    state: State<'_, AppState>,
    request: GitCheckoutBranchRequest,
) -> Result<GitOperationResult, String> {
    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        return execute_remote_git_operation(
            &state,
            &target,
            &["checkout".to_string(), request.branch_name],
        )
        .await;
    }

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
    state: State<'_, AppState>,
    request: GitCreateBranchRequest,
) -> Result<GitOperationResult, String> {
    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        let mut args = vec![
            "checkout".to_string(),
            "-b".to_string(),
            request.branch_name,
        ];
        if let Some(start_point) = request.start_point.filter(|s| !s.trim().is_empty()) {
            args.push(start_point);
        }
        return execute_remote_git_operation(&state, &target, &args).await;
    }

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
    state: State<'_, AppState>,
    request: GitDeleteBranchRequest,
) -> Result<GitOperationResult, String> {
    let force = request.force.unwrap_or(false);
    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        return execute_remote_git_operation(
            &state,
            &target,
            &[
                "branch".to_string(),
                if force { "-D" } else { "-d" }.to_string(),
                request.branch_name,
            ],
        )
        .await;
    }

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
    state: State<'_, AppState>,
    request: GitDiffRequest,
) -> Result<String, String> {
    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        return execute_remote_git_success(&state, &target, &build_git_diff_args(&request.params))
            .await;
    }

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
pub async fn git_get_changed_files(
    state: State<'_, AppState>,
    request: GitChangedFilesRequest,
) -> Result<Vec<GitChangedFile>, String> {
    info!(
        "Getting changed Git files for repository: {}",
        request.repository_path
    );

    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        let output = execute_remote_git_success(
            &state,
            &target,
            &build_git_changed_files_args(&request.params),
        )
        .await?;
        return Ok(parse_remote_name_status_output(&output));
    }

    GitService::get_changed_files(&request.repository_path, &request.params)
        .await
        .map_err(|e| {
            error!("Failed to get changed Git files: {}", e);
            e.to_string()
        })
}

#[tauri::command]
pub async fn git_reset_files(
    state: State<'_, AppState>,
    request: GitResetFilesRequest,
) -> Result<GitOperationResult, String> {
    let staged = request.staged.unwrap_or(false);

    info!(
        "Resetting files in '{}' (staged: {}): {:?}",
        request.repository_path, staged, request.files
    );

    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        let mut args = vec!["restore".to_string()];
        if staged {
            args.push("--staged".to_string());
        }
        args.extend(request.files);
        return execute_remote_git_operation(&state, &target, &args).await;
    }

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
    state: State<'_, AppState>,
    request: GitGetFileContentRequest,
) -> Result<String, String> {
    info!(
        "Getting file content for '{}' at commit '{:?}' in repo '{}'",
        request.file_path, request.commit, request.repository_path
    );

    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        let object_spec = format!(
            "{}:{}",
            request.commit.as_deref().unwrap_or("HEAD"),
            request.file_path
        );
        return execute_remote_git_success(&state, &target, &["show".to_string(), object_spec])
            .await;
    }

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
    state: State<'_, AppState>,
    request: GitResetToCommitRequest,
) -> Result<GitOperationResult, String> {
    info!(
        "Resetting to commit '{}' with mode '{}' in repo '{}'",
        request.commit_hash, request.mode, request.repository_path
    );

    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        let mode_flag = match request.mode.as_str() {
            "soft" => "--soft",
            "mixed" => "--mixed",
            "hard" => "--hard",
            _ => return Err(format!("Invalid reset mode: {}", request.mode)),
        };
        return execute_remote_git_operation(
            &state,
            &target,
            &[
                "reset".to_string(),
                mode_flag.to_string(),
                request.commit_hash,
            ],
        )
        .await;
    }

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

    if resolve_remote_git_target(&repository_path).await.is_some() {
        return Err("Git graph is not supported for remote SSH workspaces yet".to_string());
    }

    GitService::get_git_graph_for_branch(&repository_path, max_count, branch_name.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_cherry_pick(
    state: State<'_, AppState>,
    request: GitCherryPickRequest,
) -> Result<GitOperationResult, String> {
    let no_commit = request.no_commit.unwrap_or(false);

    info!(
        "Cherry-picking commit '{}' in repo '{}' (no_commit: {})",
        request.commit_hash, request.repository_path, no_commit
    );

    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        let mut args = vec!["cherry-pick".to_string()];
        if no_commit {
            args.push("-n".to_string());
        }
        args.push(request.commit_hash);
        return execute_remote_git_operation(&state, &target, &args).await;
    }

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
    state: State<'_, AppState>,
    request: GitRepositoryRequest,
) -> Result<GitOperationResult, String> {
    info!("Aborting cherry-pick in repo '{}'", request.repository_path);

    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        return execute_remote_git_operation(
            &state,
            &target,
            &["cherry-pick".to_string(), "--abort".to_string()],
        )
        .await;
    }

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
    state: State<'_, AppState>,
    request: GitRepositoryRequest,
) -> Result<GitOperationResult, String> {
    info!(
        "Continuing cherry-pick in repo '{}'",
        request.repository_path
    );

    if let Some(target) = resolve_remote_git_target(&request.repository_path).await {
        return execute_remote_git_operation(
            &state,
            &target,
            &["cherry-pick".to_string(), "--continue".to_string()],
        )
        .await;
    }

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

    if resolve_remote_git_target(&request.repository_path)
        .await
        .is_some()
    {
        return Err("Git worktrees are not supported for remote SSH workspaces yet".to_string());
    }

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

    if resolve_remote_git_target(&request.repository_path)
        .await
        .is_some()
    {
        return Err("Git worktrees are not supported for remote SSH workspaces yet".to_string());
    }

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

    if resolve_remote_git_target(&request.repository_path)
        .await
        .is_some()
    {
        return Err("Git worktrees are not supported for remote SSH workspaces yet".to_string());
    }

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
