//! Function-agent service ports for future runtime migration.
//!
//! The current core implementation still owns Git commands, AI clients, prompt
//! templates, JSON extraction, and error mapping. These ports define the seam
//! that future adapters must satisfy before those implementations move.

use crate::function_agents::common::{AgentResult, Language};
use crate::function_agents::git_func_agent::{
    AICommitAnalysis, CommitMessageOptions, ProjectContext,
};
use crate::function_agents::startchat_func_agent::{
    AIGeneratedAnalysis, AheadBehind, GitWorkState,
};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

pub type FunctionAgentFuture<'a, T> = Pin<Box<dyn Future<Output = AgentResult<T>> + Send + 'a>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitSnapshot {
    pub staged_paths: Vec<String>,
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub diff_content: String,
    pub project_context: ProjectContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitAiAnalysisRequest {
    pub diff_content: String,
    pub project_context: ProjectContext,
    pub options: CommitMessageOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartchatGitSnapshot {
    pub current_branch: String,
    pub status_porcelain: String,
    pub unstaged_diff: String,
    pub staged_diff: String,
    pub unpushed_commits: u32,
    pub ahead_behind: Option<AheadBehind>,
    pub last_commit_timestamp: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkStateAiAnalysisRequest {
    pub git_state: Option<GitWorkState>,
    pub git_diff: String,
    pub language: Language,
}

pub trait FunctionAgentGitPort: Send + Sync {
    fn git_commit_snapshot(&self, repo_path: String) -> FunctionAgentFuture<'_, GitCommitSnapshot>;
    fn startchat_git_snapshot(
        &self,
        repo_path: String,
    ) -> FunctionAgentFuture<'_, StartchatGitSnapshot>;
}

/// Future AI boundary for function agents.
///
/// This PR only defines the contract. Core still owns AI client selection,
/// prompt templates, response parsing, and error mapping; a concrete adapter
/// must add equivalence tests before any call site is wired through this trait.
pub trait FunctionAgentAiPort: Send + Sync {
    fn analyze_commit(
        &self,
        request: CommitAiAnalysisRequest,
    ) -> FunctionAgentFuture<'_, AICommitAnalysis>;
    fn analyze_work_state(
        &self,
        request: WorkStateAiAnalysisRequest,
    ) -> FunctionAgentFuture<'_, AIGeneratedAnalysis>;
}
