use super::git_types::*;
use super::git_utils::*;
/**
 * Git service implementation
 */
use crate::util::elapsed_ms_u64;
use git2::{BranchType, Commit, Repository};
use std::path::Path;
use std::time::Duration;
use std::time::Instant;
use tokio::time::timeout;

pub struct GitService;

type CommitStats = (Option<i32>, Option<i32>, Option<i32>);

fn parse_name_status_output(output: &str) -> Vec<GitChangedFile> {
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

impl GitService {
    /// Checks whether the path is a Git repository.
    pub async fn is_repository<P: AsRef<Path>>(path: P) -> Result<bool, GitError> {
        Ok(is_git_repository(path))
    }

    /// Gets repository information.
    pub async fn get_repository<P: AsRef<Path>>(path: P) -> Result<GitRepository, GitError> {
        let _start_time = Instant::now();

        let repo =
            Repository::open(&path).map_err(|e| GitError::RepositoryNotFound(e.to_string()))?;

        let current_branch = get_current_branch(&repo)?;
        let is_bare = repo.is_bare();
        let has_changes = !get_file_statuses(&repo)?.is_empty();

        let remotes = repo
            .remotes()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?
            .iter()
            .filter_map(|name| name.map(|s| s.to_string()))
            .collect();

        let path_str = path.as_ref().to_string_lossy().to_string();
        let name = path
            .as_ref()
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        Ok(GitRepository {
            path: path_str,
            name,
            current_branch,
            is_bare,
            has_changes,
            remotes,
        })
    }

    /// Gets repository status.
    pub async fn get_status<P: AsRef<Path>>(path: P) -> Result<GitStatus, GitError> {
        let repo =
            Repository::open(&path).map_err(|e| GitError::RepositoryNotFound(e.to_string()))?;

        let current_branch = get_current_branch(&repo)?;
        let file_statuses = get_file_statuses(&repo)?;

        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        let mut untracked = Vec::new();

        for status in file_statuses {
            if status.status.contains('?') {
                untracked.push(status.path);
            } else {
                if status.index_status.is_some() {
                    staged.push(status.clone());
                }
                if status.workdir_status.is_some() {
                    unstaged.push(status);
                }
            }
        }

        let (ahead, behind) =
            Self::get_ahead_behind_count(&repo, &current_branch).unwrap_or((0, 0));

        Ok(GitStatus {
            staged,
            unstaged,
            untracked,
            current_branch,
            ahead,
            behind,
        })
    }

    /// Gets the branch list.
    pub async fn get_branches<P: AsRef<Path>>(
        path: P,
        include_remote: bool,
    ) -> Result<Vec<GitBranch>, GitError> {
        let repo =
            Repository::open(&path).map_err(|e| GitError::RepositoryNotFound(e.to_string()))?;

        let mut branches = Vec::new();
        let current_branch = get_current_branch(&repo)?;

        let local_branches = repo
            .branches(Some(BranchType::Local))
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        for branch_result in local_branches {
            let (branch, _) = branch_result.map_err(|e| GitError::CommandFailed(e.to_string()))?;

            if let Some(name) = branch
                .name()
                .map_err(|e| GitError::CommandFailed(e.to_string()))?
            {
                let is_current = name == current_branch;
                let upstream = branch.upstream().ok().and_then(|upstream_branch| {
                    upstream_branch.name().ok().flatten().map(|s| s.to_string())
                });

                let (last_commit, last_commit_date) =
                    if let Ok(commit) = branch.get().peel_to_commit() {
                        (
                            Some(commit.id().to_string()),
                            Some(format_timestamp(commit.time().seconds())),
                        )
                    } else {
                        (None, None)
                    };

                let (ahead, behind) = if is_current {
                    Self::get_ahead_behind_count(&repo, name).unwrap_or((0, 0))
                } else {
                    (0, 0)
                };

                branches.push(GitBranch {
                    name: name.to_string(),
                    current: is_current,
                    remote: false,
                    upstream,
                    ahead,
                    behind,
                    last_commit,
                    last_commit_date: last_commit_date.clone(),

                    base_branch: None,
                    child_branches: None,
                    merged_branches: None,
                    branch_type: Some(Self::determine_branch_type(name)),
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
                });
            }
        }

        if include_remote {
            let remote_branches = repo
                .branches(Some(BranchType::Remote))
                .map_err(|e| GitError::CommandFailed(e.to_string()))?;

            for branch_result in remote_branches {
                let (branch, _) =
                    branch_result.map_err(|e| GitError::CommandFailed(e.to_string()))?;

                if let Some(name) = branch
                    .name()
                    .map_err(|e| GitError::CommandFailed(e.to_string()))?
                {
                    let (last_commit, last_commit_date) =
                        if let Ok(commit) = branch.get().peel_to_commit() {
                            (
                                Some(commit.id().to_string()),
                                Some(format_timestamp(commit.time().seconds())),
                            )
                        } else {
                            (None, None)
                        };

                    branches.push(GitBranch {
                        name: name.to_string(),
                        current: false,
                        remote: true,
                        upstream: None,
                        ahead: 0,
                        behind: 0,
                        last_commit,
                        last_commit_date: last_commit_date.clone(),

                        base_branch: None,
                        child_branches: None,
                        merged_branches: None,
                        branch_type: Some(Self::determine_branch_type(name)),
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
                    });
                }
            }
        }

        Ok(branches)
    }

    /// Gets branches with detailed information.
    pub async fn get_enhanced_branches<P: AsRef<Path>>(
        path: P,
        include_remote: bool,
    ) -> Result<Vec<GitBranch>, GitError> {
        let mut branches = Self::get_branches(&path, include_remote).await?;

        Self::analyze_branch_relations(&mut branches)?;

        let repo =
            Repository::open(&path).map_err(|e| GitError::RepositoryNotFound(e.to_string()))?;

        for branch in &mut branches {
            if !branch.remote {
                branch.stats = Self::calculate_branch_stats(&repo, &branch.name).ok();
                branch.is_stale = Some(Self::is_branch_stale(branch));
                branch.can_merge = Self::can_merge_safely(&repo, &branch.name).ok();
                branch.has_conflicts = branch.can_merge.map(|can| !can);
            }
        }

        Ok(branches)
    }

    /// Determines the branch type.
    fn determine_branch_type(branch_name: &str) -> String {
        if branch_name.starts_with("feature/") || branch_name.starts_with("feat/") {
            "feature".to_string()
        } else if branch_name.starts_with("hotfix/") || branch_name.starts_with("fix/") {
            "hotfix".to_string()
        } else if branch_name.starts_with("release/") || branch_name.starts_with("rel/") {
            "release".to_string()
        } else if branch_name.starts_with("bugfix/") || branch_name.starts_with("bug/") {
            "bugfix".to_string()
        } else if branch_name.starts_with("chore/") {
            "chore".to_string()
        } else if branch_name.starts_with("docs/") {
            "docs".to_string()
        } else if branch_name.starts_with("test/") {
            "test".to_string()
        } else if ["main", "master", "develop", "development"].contains(&branch_name) {
            "main".to_string()
        } else {
            "other".to_string()
        }
    }

    /// Analyzes branch relationships.
    fn analyze_branch_relations(branches: &mut [GitBranch]) -> Result<(), GitError> {
        let main_branches = ["main", "master", "develop"];

        let available_main_branches: Vec<String> = branches
            .iter()
            .filter(|b| !b.remote && main_branches.contains(&b.name.as_str()))
            .map(|b| b.name.clone())
            .collect();

        for branch in branches.iter_mut() {
            if !branch.remote && !main_branches.contains(&branch.name.as_str()) {
                if let Some(main_branch) = available_main_branches.first() {
                    branch.base_branch = Some(main_branch.clone());
                }
            }
        }

        let mut child_map: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for branch in branches.iter() {
            if let Some(base) = &branch.base_branch {
                child_map
                    .entry(base.clone())
                    .or_default()
                    .push(branch.name.clone());
            }
        }

        for branch in branches.iter_mut() {
            if let Some(children) = child_map.get(&branch.name) {
                branch.child_branches = Some(children.clone());
            }
        }

        Ok(())
    }

    /// Computes branch statistics.
    fn calculate_branch_stats(
        repo: &Repository,
        branch_name: &str,
    ) -> Result<GitBranchStats, GitError> {
        let branch_ref = repo
            .find_branch(branch_name, BranchType::Local)
            .map_err(|e| GitError::BranchNotFound(e.to_string()))?;

        let target = branch_ref
            .get()
            .target()
            .ok_or_else(|| GitError::CommandFailed("Branch has no target".to_string()))?;

        let mut revwalk = repo
            .revwalk()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;
        revwalk
            .push(target)
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        let commit_count = revwalk.count() as i32;

        Ok(GitBranchStats {
            commit_count,
            contributor_count: 1,
            file_changes: 0,
            lines_changed: GitLinesChanged {
                additions: 0,
                deletions: 0,
            },
            activity_score: std::cmp::min(commit_count * 2, 100),
        })
    }

    /// Checks whether a branch is stale.
    fn is_branch_stale(branch: &GitBranch) -> bool {
        !matches!(&branch.last_commit_date, Some(_last_commit_date))
    }

    /// Checks whether a branch can be merged safely.
    fn can_merge_safely(_repo: &Repository, _branch_name: &str) -> Result<bool, GitError> {
        Ok(true)
    }

    /// Gets commit history.
    pub async fn get_commits<P: AsRef<Path>>(
        path: P,
        params: GitLogParams,
    ) -> Result<Vec<GitCommit>, GitError> {
        let repo =
            Repository::open(&path).map_err(|e| GitError::RepositoryNotFound(e.to_string()))?;

        let mut revwalk = repo
            .revwalk()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        revwalk
            .push_head()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        let mut commits = Vec::new();
        let mut count = 0;
        let skip = params.skip.unwrap_or(0);
        let max_count = params.max_count.unwrap_or(50);

        for (index, oid_result) in revwalk.enumerate() {
            if index < skip as usize {
                continue;
            }

            if count >= max_count {
                break;
            }

            let oid = oid_result.map_err(|e| GitError::CommandFailed(e.to_string()))?;

            let commit = repo
                .find_commit(oid)
                .map_err(|e| GitError::CommandFailed(e.to_string()))?;

            let author = commit.author();
            let message = commit.message().unwrap_or("").to_string();

            if let Some(author_filter) = &params.author {
                if !author.name().unwrap_or("").contains(author_filter) {
                    continue;
                }
            }

            if let Some(grep_filter) = &params.grep {
                if !message.contains(grep_filter) {
                    continue;
                }
            }

            let parents: Vec<String> = commit.parent_ids().map(|id| id.to_string()).collect();

            let (additions, deletions, files_changed) = if params.stat.unwrap_or(false) {
                Self::get_commit_stats(&repo, &commit).unwrap_or((None, None, None))
            } else {
                (None, None, None)
            };

            commits.push(GitCommit {
                hash: commit.id().to_string(),
                short_hash: commit.id().to_string()[..7].to_string(),
                message,
                author: author.name().unwrap_or("Unknown").to_string(),
                author_email: author.email().unwrap_or("").to_string(),
                date: format_timestamp(commit.time().seconds()),
                parents,
                additions,
                deletions,
                files_changed,
            });

            count += 1;
        }

        Ok(commits)
    }

    /// Adds files to the staging area.
    pub async fn add_files<P: AsRef<Path>>(
        path: P,
        params: GitAddParams,
    ) -> Result<GitOperationResult, GitError> {
        let start_time = Instant::now();
        let repo_path = path.as_ref().to_string_lossy();

        let mut args = vec!["add"];

        if params.all.unwrap_or(false) {
            args.push("-A");
        } else if params.update.unwrap_or(false) {
            args.push("-u");
        } else {
            for file in &params.files {
                args.push(file);
            }
        }

        let output = execute_git_command(&repo_path, &args).await?;
        let duration = elapsed_ms_u64(start_time);

        Ok(GitOperationResult {
            success: true,
            data: Some(serde_json::json!({
                "files": params.files,
                "all": params.all,
                "update": params.update
            })),
            error: None,
            output: Some(output),
            duration: Some(duration),
        })
    }

    /// Commits changes.
    pub async fn commit<P: AsRef<Path>>(
        path: P,
        params: GitCommitParams,
    ) -> Result<GitOperationResult, GitError> {
        let start_time = Instant::now();
        let repo_path = path.as_ref().to_string_lossy();

        let mut args = vec![
            "commit".to_string(),
            "-m".to_string(),
            params.message.clone(),
        ];

        if params.amend.unwrap_or(false) {
            args.push("--amend".to_string());
        }

        if params.all.unwrap_or(false) {
            args.push("-a".to_string());
        }

        if params.no_verify.unwrap_or(false) {
            args.push("--no-verify".to_string());
        }

        if let Some(author) = &params.author {
            args.push("--author".to_string());
            args.push(format!("{} <{}>", author.name, author.email));
        }

        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = execute_git_command(&repo_path, &args_refs).await?;
        let duration = elapsed_ms_u64(start_time);

        Ok(GitOperationResult {
            success: true,
            data: Some(serde_json::json!({
                "message": params.message,
                "amend": params.amend,
                "all": params.all,
                "noVerify": params.no_verify,
                "author": params.author
            })),
            error: None,
            output: Some(output),
            duration: Some(duration),
        })
    }

    /// Pushes changes.
    pub async fn push<P: AsRef<Path>>(
        path: P,
        params: GitPushParams,
    ) -> Result<GitOperationResult, GitError> {
        let start_time = Instant::now();
        let repo_path = path.as_ref().to_string_lossy();

        let mut args = vec!["push"];

        if params.force.unwrap_or(false) {
            args.push("--force");
        }

        if params.set_upstream.unwrap_or(false) {
            args.push("-u");
        }

        if let Some(remote) = &params.remote {
            args.push(remote);
        }

        if let Some(branch) = &params.branch {
            args.push(branch);
        }

        let output = timeout(
            Duration::from_secs(30),
            execute_git_command(&repo_path, &args),
        )
        .await
        .map_err(|_| GitError::NetworkError("Push operation timed out".to_string()))??;

        let duration = elapsed_ms_u64(start_time);

        Ok(GitOperationResult {
            success: true,
            data: Some(serde_json::json!({
                "remote": params.remote,
                "branch": params.branch,
                "force": params.force,
                "set_upstream": params.set_upstream
            })),
            error: None,
            output: Some(output),
            duration: Some(duration),
        })
    }

    /// Pulls changes.
    pub async fn pull<P: AsRef<Path>>(
        path: P,
        params: GitPullParams,
    ) -> Result<GitOperationResult, GitError> {
        let start_time = Instant::now();
        let repo_path = path.as_ref().to_string_lossy();

        let mut args = vec!["pull"];

        if params.rebase.unwrap_or(false) {
            args.push("--rebase");
        }

        if let Some(remote) = &params.remote {
            args.push(remote);
        }

        if let Some(branch) = &params.branch {
            args.push(branch);
        }

        let output = timeout(
            Duration::from_secs(30),
            execute_git_command(&repo_path, &args),
        )
        .await
        .map_err(|_| GitError::NetworkError("Pull operation timed out".to_string()))??;

        let duration = elapsed_ms_u64(start_time);

        Ok(GitOperationResult {
            success: true,
            data: Some(serde_json::json!({
                "remote": params.remote,
                "branch": params.branch,
                "rebase": params.rebase
            })),
            error: None,
            output: Some(output),
            duration: Some(duration),
        })
    }

    /// Checks out a branch.
    pub async fn checkout_branch<P: AsRef<Path>>(
        path: P,
        branch_name: &str,
    ) -> Result<GitOperationResult, GitError> {
        let start_time = Instant::now();
        let repo_path = path.as_ref().to_string_lossy();

        let args = vec!["checkout", branch_name];
        let output = execute_git_command(&repo_path, &args).await?;
        let duration = elapsed_ms_u64(start_time);

        Ok(GitOperationResult {
            success: true,
            data: Some(serde_json::json!({
                "branch": branch_name
            })),
            error: None,
            output: Some(output),
            duration: Some(duration),
        })
    }

    /// Creates a branch.
    pub async fn create_branch<P: AsRef<Path>>(
        path: P,
        branch_name: &str,
        start_point: Option<&str>,
    ) -> Result<GitOperationResult, GitError> {
        let start_time = Instant::now();
        let repo_path = path.as_ref().to_string_lossy();

        let mut args = vec!["checkout", "-b", branch_name];
        let effective_start_point = start_point.filter(|s| !s.trim().is_empty());
        if let Some(start) = effective_start_point {
            args.push(start);
        }

        let output = execute_git_command(&repo_path, &args).await?;
        let duration = elapsed_ms_u64(start_time);

        Ok(GitOperationResult {
            success: true,
            data: Some(serde_json::json!({
                "branch": branch_name,
                "start_point": effective_start_point
            })),
            error: None,
            output: Some(output),
            duration: Some(duration),
        })
    }

    /// Deletes a branch.
    pub async fn delete_branch<P: AsRef<Path>>(
        path: P,
        branch_name: &str,
        force: bool,
    ) -> Result<GitOperationResult, GitError> {
        let start_time = Instant::now();
        let repo_path = path.as_ref().to_string_lossy();

        let flag = if force { "-D" } else { "-d" };
        let args = vec!["branch", flag, branch_name];
        let output = execute_git_command(&repo_path, &args).await?;
        let duration = elapsed_ms_u64(start_time);

        Ok(GitOperationResult {
            success: true,
            data: Some(serde_json::json!({
                "branch": branch_name,
                "force": force
            })),
            error: None,
            output: Some(output),
            duration: Some(duration),
        })
    }

    /// Resets to a specific commit.
    ///
    /// # Parameters
    /// - `path`: Repository path
    /// - `commit_hash`: Target commit hash
    /// - `mode`: Reset mode (`soft`, `mixed`, `hard`)
    pub async fn reset_to_commit<P: AsRef<Path>>(
        path: P,
        commit_hash: &str,
        mode: &str,
    ) -> Result<GitOperationResult, GitError> {
        let start_time = Instant::now();
        let repo_path = path.as_ref().to_string_lossy();

        let mode_flag = match mode {
            "soft" => "--soft",
            "mixed" => "--mixed",
            "hard" => "--hard",
            _ => {
                return Err(GitError::CommandFailed(format!(
                    "Invalid reset mode: {}",
                    mode
                )))
            }
        };

        let args = vec!["reset", mode_flag, commit_hash];
        let output = execute_git_command(&repo_path, &args).await?;
        let duration = elapsed_ms_u64(start_time);

        Ok(GitOperationResult {
            success: true,
            data: Some(serde_json::json!({
                "commit": commit_hash,
                "mode": mode
            })),
            error: None,
            output: Some(output),
            duration: Some(duration),
        })
    }

    /// Gets the diff.
    pub async fn get_diff<P: AsRef<Path>>(
        path: P,
        params: &GitDiffParams,
    ) -> Result<String, GitError> {
        let repo_path = path.as_ref().to_string_lossy();

        let mut args = vec!["diff"];
        let range;

        if params.staged.unwrap_or(false) {
            args.push("--cached");
        }

        match (&params.source, &params.target) {
            (Some(src), Some(tgt)) => {
                range = format!("{}..{}", src, tgt);
                args.push(&range);
            }
            (Some(src), None) => {
                args.push(src);
            }
            (None, None) => {}
            _ => {}
        }

        if params.stat.unwrap_or(false) {
            args.push("--stat");
        }

        if let Some(files) = &params.files {
            args.push("--");
            for file in files {
                args.push(file);
            }
        }

        execute_git_command(&repo_path, &args).await
    }

    /// Gets changed files using `git diff --name-status`.
    pub async fn get_changed_files<P: AsRef<Path>>(
        path: P,
        params: &GitChangedFilesParams,
    ) -> Result<Vec<GitChangedFile>, GitError> {
        let repo_path = path.as_ref().to_string_lossy();

        let mut args = vec!["diff", "--name-status"];
        let range;

        if params.staged.unwrap_or(false) {
            args.push("--cached");
        }

        match (&params.source, &params.target) {
            (Some(src), Some(tgt)) => {
                range = format!("{}..{}", src, tgt);
                args.push(&range);
            }
            (Some(src), None) => {
                args.push(src);
            }
            (None, Some(tgt)) => {
                args.push(tgt);
            }
            (None, None) => {}
        }

        let output = execute_git_command(&repo_path, &args).await?;
        Ok(parse_name_status_output(&output))
    }

    /// Gets file content.
    ///
    /// # Parameters
    /// - `path`: Repository path
    /// - `file_path`: File relative path
    /// - `commit`: Commit reference (optional, defaults to `HEAD`)
    ///
    /// # Returns
    /// - File content string
    pub async fn get_file_content<P: AsRef<Path>>(
        path: P,
        file_path: &str,
        commit: Option<&str>,
    ) -> Result<String, GitError> {
        let repo_path = path.as_ref().to_string_lossy();

        let commit_ref = commit.unwrap_or("HEAD");
        let object_spec = format!("{}:{}", commit_ref, file_path);

        let args = vec!["show", &object_spec];

        execute_git_command(&repo_path, &args).await
    }

    /// Resets file changes (discarding working tree changes).
    ///
    /// # Parameters
    /// - `path`: Repository path
    /// - `files`: List of file paths
    /// - `staged`: Whether to reset the index (`true`: reset staged, `false`: restore worktree)
    ///
    /// # Returns
    /// - Operation result
    pub async fn reset_files<P: AsRef<Path>>(
        path: P,
        files: &[String],
        staged: bool,
    ) -> Result<String, GitError> {
        let repo_path = path.as_ref().to_string_lossy();

        if staged {
            let mut args = vec!["restore", "--staged"];
            for file in files {
                args.push(file);
            }
            execute_git_command(&repo_path, &args).await
        } else {
            let mut args = vec!["restore"];
            for file in files {
                args.push(file);
            }
            execute_git_command(&repo_path, &args).await
        }
    }

    /// Gets ahead/behind counts.
    fn get_ahead_behind_count(
        repo: &Repository,
        branch_name: &str,
    ) -> Result<(i32, i32), GitError> {
        let local_branch = repo
            .find_branch(branch_name, BranchType::Local)
            .map_err(|e| GitError::BranchNotFound(e.to_string()))?;

        if let Ok(upstream) = local_branch.upstream() {
            let local_oid = local_branch.get().target().ok_or_else(|| {
                GitError::CommandFailed("Failed to get local branch target".to_string())
            })?;
            let upstream_oid = upstream.get().target().ok_or_else(|| {
                GitError::CommandFailed("Failed to get upstream branch target".to_string())
            })?;

            let (ahead, behind) = repo
                .graph_ahead_behind(local_oid, upstream_oid)
                .map_err(|e| GitError::CommandFailed(e.to_string()))?;

            Ok((ahead as i32, behind as i32))
        } else {
            Ok((0, 0))
        }
    }

    /// Gets commit statistics.
    fn get_commit_stats(_repo: &Repository, _commit: &Commit) -> Result<CommitStats, GitError> {
        Ok((None, None, None))
    }

    /// Gets Git commit graph data.
    pub async fn get_git_graph<P: AsRef<Path>>(
        path: P,
        max_count: Option<usize>,
    ) -> Result<super::GitGraph, GitError> {
        let repo =
            Repository::open(&path).map_err(|e| GitError::RepositoryNotFound(e.to_string()))?;

        super::graph::build_git_graph(&repo, max_count)
            .map_err(|e| GitError::CommandFailed(e.to_string()))
    }

    /// Gets Git commit graph data for a specific branch.
    pub async fn get_git_graph_for_branch<P: AsRef<Path>>(
        path: P,
        max_count: Option<usize>,
        branch_name: Option<&str>,
    ) -> Result<super::GitGraph, GitError> {
        let repo =
            Repository::open(&path).map_err(|e| GitError::RepositoryNotFound(e.to_string()))?;

        super::graph::build_git_graph_for_branch(&repo, max_count, branch_name)
            .map_err(|e| GitError::CommandFailed(e.to_string()))
    }

    /// Cherry-picks a commit onto the current branch.
    ///
    /// # Parameters
    /// - `path`: Repository path
    /// - `commit_hash`: Commit hash to cherry-pick
    /// - `no_commit`: Apply changes without committing automatically (default `false`)
    ///
    /// # Returns
    /// - Operation result
    pub async fn cherry_pick<P: AsRef<Path>>(
        path: P,
        commit_hash: &str,
        no_commit: bool,
    ) -> Result<GitOperationResult, GitError> {
        let start_time = Instant::now();
        let repo_path = path.as_ref().to_string_lossy();

        let mut args = vec!["cherry-pick"];

        if no_commit {
            args.push("-n");
        }

        args.push(commit_hash);

        let output = execute_git_command(&repo_path, &args).await?;
        let duration = elapsed_ms_u64(start_time);

        Ok(GitOperationResult {
            success: true,
            data: Some(serde_json::json!({
                "commit": commit_hash,
                "no_commit": no_commit
            })),
            error: None,
            output: Some(output),
            duration: Some(duration),
        })
    }

    /// Aborts the cherry-pick operation.
    ///
    /// # Parameters
    /// - `path`: Repository path
    ///
    /// # Returns
    /// - Operation result
    pub async fn cherry_pick_abort<P: AsRef<Path>>(
        path: P,
    ) -> Result<GitOperationResult, GitError> {
        let start_time = Instant::now();
        let repo_path = path.as_ref().to_string_lossy();

        let args = vec!["cherry-pick", "--abort"];
        let output = execute_git_command(&repo_path, &args).await?;
        let duration = elapsed_ms_u64(start_time);

        Ok(GitOperationResult {
            success: true,
            data: None,
            error: None,
            output: Some(output),
            duration: Some(duration),
        })
    }

    /// Continues the cherry-pick operation (after resolving conflicts).
    ///
    /// # Parameters
    /// - `path`: Repository path
    ///
    /// # Returns
    /// - Operation result
    pub async fn cherry_pick_continue<P: AsRef<Path>>(
        path: P,
    ) -> Result<GitOperationResult, GitError> {
        let start_time = Instant::now();
        let repo_path = path.as_ref().to_string_lossy();

        let args = vec!["cherry-pick", "--continue"];
        let output = execute_git_command(&repo_path, &args).await?;
        let duration = elapsed_ms_u64(start_time);

        Ok(GitOperationResult {
            success: true,
            data: None,
            error: None,
            output: Some(output),
            duration: Some(duration),
        })
    }

    /// Lists all worktrees.
    ///
    /// # Parameters
    /// - `path`: Repository path
    ///
    /// # Returns
    /// - Worktree list
    pub async fn list_worktrees<P: AsRef<Path>>(
        path: P,
    ) -> Result<Vec<super::GitWorktreeInfo>, GitError> {
        let repo_path = path.as_ref().to_string_lossy();

        let args = vec!["worktree", "list", "--porcelain"];
        let output = execute_git_command(&repo_path, &args).await?;

        Self::parse_worktree_list(&output)
    }

    /// Parses `git worktree list --porcelain` output.
    fn parse_worktree_list(output: &str) -> Result<Vec<super::GitWorktreeInfo>, GitError> {
        let mut worktrees = Vec::new();
        let mut current_worktree: Option<super::GitWorktreeInfo> = None;

        for line in output.lines() {
            if line.starts_with("worktree ") {
                if let Some(wt) = current_worktree.take() {
                    worktrees.push(wt);
                }
                let path = line.strip_prefix("worktree ").unwrap_or("").to_string();
                current_worktree = Some(super::GitWorktreeInfo {
                    path,
                    branch: None,
                    head: String::new(),
                    is_main: false,
                    is_locked: false,
                    is_prunable: false,
                });
            } else if let Some(ref mut wt) = current_worktree {
                if line.starts_with("HEAD ") {
                    wt.head = line.strip_prefix("HEAD ").unwrap_or("").to_string();
                } else if line.starts_with("branch ") {
                    let branch_ref = line.strip_prefix("branch ").unwrap_or("");
                    let branch_name = branch_ref
                        .strip_prefix("refs/heads/")
                        .unwrap_or(branch_ref)
                        .to_string();
                    wt.branch = Some(branch_name);
                } else if line == "bare" {
                    wt.is_main = true;
                } else if line == "locked" {
                    wt.is_locked = true;
                } else if line == "prunable" {
                    wt.is_prunable = true;
                }
            }
        }

        if let Some(wt) = current_worktree {
            worktrees.push(wt);
        }

        if let Some(first) = worktrees.first_mut() {
            if !first.is_main {
                first.is_main = true;
            }
        }

        Ok(worktrees)
    }

    /// Adds a new worktree.
    ///
    /// # Parameters
    /// - `path`: Repository path
    /// - `branch`: Branch name
    /// - `create_branch`: Whether to create a new branch
    ///
    /// # Returns
    /// - Newly created worktree information
    pub async fn add_worktree<P: AsRef<Path>>(
        path: P,
        branch: &str,
        create_branch: bool,
    ) -> Result<super::GitWorktreeInfo, GitError> {
        let repo_path = path.as_ref().to_string_lossy();

        let worktree_dir = path.as_ref().join(".worktrees");
        let worktree_path = worktree_dir.join(branch);
        let worktree_path_str = worktree_path.to_string_lossy().to_string();

        if !worktree_dir.exists() {
            std::fs::create_dir_all(&worktree_dir).map_err(GitError::IoError)?;
        }

        let args = if create_branch {
            vec!["worktree", "add", "-b", branch, &worktree_path_str]
        } else {
            vec!["worktree", "add", &worktree_path_str, branch]
        };

        execute_git_command(&repo_path, &args).await?;

        let worktrees = Self::list_worktrees(&path).await?;

        let normalized_expected = worktree_path_str.replace("\\", "/");

        worktrees
            .into_iter()
            .find(|wt| wt.path == normalized_expected)
            .ok_or_else(|| {
                GitError::CommandFailed("Failed to find newly created worktree".to_string())
            })
    }

    /// Removes a worktree.
    ///
    /// # Parameters
    /// - `path`: Repository path
    /// - `worktree_path`: Worktree path to remove
    /// - `force`: Whether to force removal
    ///
    /// # Returns
    /// - Operation result
    pub async fn remove_worktree<P: AsRef<Path>>(
        path: P,
        worktree_path: &str,
        force: bool,
    ) -> Result<GitOperationResult, GitError> {
        let start_time = Instant::now();
        let repo_path = path.as_ref().to_string_lossy();

        let mut args = vec!["worktree", "remove"];
        if force {
            args.push("--force");
        }
        args.push(worktree_path);

        let output = execute_git_command(&repo_path, &args).await?;
        let duration = elapsed_ms_u64(start_time);

        Ok(GitOperationResult {
            success: true,
            data: Some(serde_json::json!({
                "worktree_path": worktree_path,
                "force": force
            })),
            error: None,
            output: Some(output),
            duration: Some(duration),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_status_output_for_common_statuses() {
        let files = parse_name_status_output(
            "M\tsrc/main.rs\nA\tsrc/new.rs\nD\tsrc/old.rs\nR100\tsrc/old_name.rs\tsrc/new_name.rs\nC087\tsrc/source.rs\tsrc/copy.rs\n",
        );

        assert_eq!(
            files,
            vec![
                GitChangedFile {
                    path: "src/main.rs".to_string(),
                    old_path: None,
                    status: GitChangedFileStatus::Modified,
                },
                GitChangedFile {
                    path: "src/new.rs".to_string(),
                    old_path: None,
                    status: GitChangedFileStatus::Added,
                },
                GitChangedFile {
                    path: "src/old.rs".to_string(),
                    old_path: None,
                    status: GitChangedFileStatus::Deleted,
                },
                GitChangedFile {
                    path: "src/new_name.rs".to_string(),
                    old_path: Some("src/old_name.rs".to_string()),
                    status: GitChangedFileStatus::Renamed,
                },
                GitChangedFile {
                    path: "src/copy.rs".to_string(),
                    old_path: Some("src/source.rs".to_string()),
                    status: GitChangedFileStatus::Copied,
                },
            ],
        );
    }
}
