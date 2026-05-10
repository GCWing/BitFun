/**
 * Git-related type definitions
 */
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRepository {
    pub path: String,
    pub name: String,
    pub current_branch: String,
    pub is_bare: bool,
    pub has_changes: bool,
    pub remotes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatus {
    pub staged: Vec<GitFileStatus>,
    pub unstaged: Vec<GitFileStatus>,
    pub untracked: Vec<String>,
    pub current_branch: String,
    pub ahead: i32,
    pub behind: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitFileStatus {
    pub path: String,
    pub status: String,
    pub index_status: Option<String>,
    pub workdir_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitBranch {
    pub name: String,
    pub current: bool,
    pub remote: bool,
    pub upstream: Option<String>,
    pub ahead: i32,
    pub behind: i32,
    pub last_commit: Option<String>,
    pub last_commit_date: Option<String>,

    pub base_branch: Option<String>,
    pub child_branches: Option<Vec<String>>,
    pub merged_branches: Option<Vec<String>>,

    pub branch_type: Option<String>,
    pub has_conflicts: Option<bool>,
    pub can_merge: Option<bool>,
    pub is_stale: Option<bool>,
    pub merge_status: Option<String>,

    pub stats: Option<GitBranchStats>,
    pub created_at: Option<String>,
    pub last_activity_at: Option<String>,

    pub tags: Option<Vec<String>>,
    pub description: Option<String>,
    pub linked_issues: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitBranchStats {
    pub commit_count: i32,
    pub contributor_count: i32,
    pub file_changes: i32,
    pub lines_changed: GitLinesChanged,
    pub activity_score: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLinesChanged {
    pub additions: i32,
    pub deletions: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommit {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub author: String,
    pub author_email: String,
    pub date: String,
    pub parents: Vec<String>,
    pub additions: Option<i32>,
    pub deletions: Option<i32>,
    pub files_changed: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitLogParams {
    pub max_count: Option<i32>,
    pub skip: Option<i32>,
    pub author: Option<String>,
    pub grep: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub stat: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitAddParams {
    pub files: Vec<String>,
    pub all: Option<bool>,
    pub update: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommitParams {
    pub message: String,
    pub amend: Option<bool>,
    pub all: Option<bool>,
    #[serde(rename = "noVerify")]
    pub no_verify: Option<bool>,
    pub author: Option<GitAuthor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitAuthor {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitPushParams {
    pub remote: Option<String>,
    pub branch: Option<String>,
    pub force: Option<bool>,
    pub set_upstream: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitPullParams {
    pub remote: Option<String>,
    pub branch: Option<String>,
    pub rebase: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitMergeParams {
    pub branch: String,
    pub strategy: Option<String>,
    pub message: Option<String>,
    pub no_ff: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStashParams {
    pub message: Option<String>,
    pub include_untracked: Option<bool>,
    pub keep_index: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitDiffParams {
    pub source: Option<String>,
    pub target: Option<String>,
    pub files: Option<Vec<String>>,
    pub staged: Option<bool>,
    pub stat: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitChangedFilesParams {
    pub source: Option<String>,
    pub target: Option<String>,
    pub staged: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitChangedFileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitChangedFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: GitChangedFileStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitOperationResult {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
    pub output: Option<String>,
    pub duration: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDiffResult {
    pub files: Vec<GitDiffFile>,
    pub total_additions: i32,
    pub total_deletions: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDiffFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub additions: i32,
    pub deletions: i32,
    pub diff: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStash {
    pub index: i32,
    pub message: String,
    pub branch: String,
    pub date: String,
    pub hash: String,
}

/// Git worktree information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitWorktreeInfo {
    /// Worktree path
    pub path: String,
    /// Associated branch name
    pub branch: Option<String>,
    /// HEAD commit hash
    pub head: String,
    /// Whether this is the main worktree (the main directory of a bare repository)
    pub is_main: bool,
    /// Whether the worktree is locked
    pub is_locked: bool,
    /// Whether the worktree is prunable
    pub is_prunable: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("Repository not found: {0}")]
    RepositoryNotFound(String),

    #[error("Git command failed: {0}")]
    CommandFailed(String),

    #[error("Invalid repository path: {0}")]
    InvalidPath(String),

    #[error("Branch not found: {0}")]
    BranchNotFound(String),

    #[error("Merge conflict: {0}")]
    MergeConflict(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Git2 error: {0}")]
    Git2Error(#[from] git2::Error),
}

/// Raw result of executing a git command, preserving exit code and both streams.
#[derive(Debug, Clone)]
pub struct GitCommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}
