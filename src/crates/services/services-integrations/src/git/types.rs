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

/// Raw result of executing a git command, preserving exit code and both streams.
#[derive(Debug, Clone)]
pub struct GitCommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
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

/// Git graph node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    /// Commit hash.
    pub hash: String,
    /// Commit message (first line).
    pub message: String,
    /// Full commit message.
    pub full_message: String,
    /// Author name.
    pub author_name: String,
    /// Author email.
    pub author_email: String,
    /// Commit time (Unix timestamp).
    pub timestamp: i64,
    /// Parent commit hashes.
    pub parents: Vec<String>,
    /// Child commit hashes (filled when building the graph).
    pub children: Vec<String>,
    /// Associated refs (branches, tags, etc.).
    pub refs: Vec<GraphRef>,
    /// Lane position.
    pub lane: i32,
    /// Lanes that fork out.
    pub forking_lanes: Vec<i32>,
    /// Lanes that merge in.
    pub merging_lanes: Vec<i32>,
    /// Lanes that pass through.
    pub passing_lanes: Vec<i32>,
}

/// Git ref information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRef {
    /// Ref name.
    pub name: String,
    /// Ref type: `branch`, `remote`, `tag`.
    pub ref_type: String,
    /// Whether this is the current branch.
    pub is_current: bool,
    /// Whether this is `HEAD`.
    pub is_head: bool,
}

/// Git graph data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitGraph {
    /// Node list.
    pub nodes: Vec<GraphNode>,
    /// Maximum lane count.
    pub max_lane: i32,
    /// Current branch name.
    pub current_branch: Option<String>,
}
