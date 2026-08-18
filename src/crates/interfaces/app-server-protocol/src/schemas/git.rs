//! Git-domain App Server wire schemas.

use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "git/isRepository", response = GitIsRepositoryResponse)]
pub struct GitIsRepositoryMessage(pub GitRepositoryPathRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct GitIsRepositoryResponse(pub bool);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "git/getStatus", response = GitGetStatusResponse)]
pub struct GitGetStatusMessage(pub GitRepositoryPathRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct GitGetStatusResponse(pub GitStatus);

/// Read-only ownership-trust probe.
///
/// Granting trust is deliberately absent from this surface: it writes the
/// server user's global Git configuration, and a browser client is not the
/// machine that owns the repository. The probe is what lets such a client name
/// the folder and hand over the exact command instead of a dead end.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "git/getRepositoryTrust", response = GitGetRepositoryTrustResponse)]
pub struct GitGetRepositoryTrustMessage(pub GitRepositoryPathRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct GitGetRepositoryTrustResponse(pub GitTrustReport);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "git/getBranches", response = GitGetBranchesResponse)]
pub struct GitGetBranchesMessage(pub GitBranchesRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct GitGetBranchesResponse {
    pub branches: Vec<GitBranch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct GitRepositoryPathRequest {
    pub repository_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct GitBranchesRequest {
    pub repository_path: String,
    #[serde(default)]
    pub include_remote: Option<bool>,
}

/// Whether Git accepts a repository for the user this host runs as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum GitTrustState {
    Trusted,
    TrustRequired,
    NotARepository,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct GitTrustReport {
    pub state: GitTrustState,
    pub repository_path: Option<String>,
    /// Git's own diagnostic, for surfaces that show manual steps.
    pub detail: Option<String>,
    /// Command the user can run on this host to resolve it themselves.
    pub manual_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatus {
    pub staged: Vec<GitFileStatus>,
    pub unstaged: Vec<GitFileStatus>,
    pub untracked: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct GitBranchStats {
    pub commit_count: i32,
    pub contributor_count: i32,
    pub file_changes: i32,
    pub lines_changed: GitLinesChanged,
    pub activity_score: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct GitLinesChanged {
    pub additions: i32,
    pub deletions: i32,
}
