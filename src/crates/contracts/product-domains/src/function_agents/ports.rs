//! Function-agent service ports for future runtime migration.
//!
//! The current core implementation still owns Git commands, AI clients,
//! provider acquisition, and AI transport error mapping. Product-domain modules
//! own prompt templates, JSON extraction, and domain error mapping policy; these
//! ports define the runtime boundary that future adapters must satisfy before
//! concrete Git/AI implementations move.

use crate::function_agents::common::{AgentError, AgentResult};
use crate::function_agents::git_func_agent::{
    assemble_commit_message, build_changes_summary_from_paths, AICommitAnalysis, CommitMessage,
    CommitMessageOptions, ProjectContext,
};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::PathBuf;
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

pub trait FunctionAgentGitPort: Send + Sync {
    fn git_commit_snapshot(&self, repo_path: PathBuf)
        -> FunctionAgentFuture<'_, GitCommitSnapshot>;
}

/// Future AI boundary for function agents.
///
/// Core still owns AI client selection, provider acquisition, and AI transport
/// error mapping. Product call sites may route through this trait only after
/// focused equivalence tests cover the specific facade path.
pub trait FunctionAgentAiPort: Send + Sync {
    fn analyze_commit(
        &self,
        request: CommitAiAnalysisRequest,
    ) -> FunctionAgentFuture<'_, AICommitAnalysis>;
}

/// Port-backed function-agent facade for future runtime owner migration.
///
/// It owns only pure orchestration over function-agent ports and DTO helpers.
/// Core still owns Git/AI service calls, provider acquisition, and AI transport
/// errors.
pub struct FunctionAgentRuntimeFacade<'a> {
    git: &'a dyn FunctionAgentGitPort,
    ai: &'a dyn FunctionAgentAiPort,
}

impl<'a> FunctionAgentRuntimeFacade<'a> {
    pub fn new(git: &'a dyn FunctionAgentGitPort, ai: &'a dyn FunctionAgentAiPort) -> Self {
        Self { git, ai }
    }

    pub async fn generate_commit_message(
        &self,
        repo_path: PathBuf,
        options: CommitMessageOptions,
    ) -> AgentResult<CommitMessage> {
        let snapshot = self.git.git_commit_snapshot(repo_path).await?;
        if snapshot.staged_paths.is_empty() {
            return Err(AgentError::invalid_input(
                "Staging area is empty, please stage files first",
            ));
        }
        if snapshot.diff_content.trim().is_empty() {
            return Err(AgentError::invalid_input("Diff content is empty"));
        }

        let ai_analysis = self
            .ai
            .analyze_commit(CommitAiAnalysisRequest {
                diff_content: snapshot.diff_content,
                project_context: snapshot.project_context,
                options,
            })
            .await?;

        let changes_summary = build_changes_summary_from_paths(
            &snapshot.staged_paths,
            snapshot.staged_count,
            snapshot.unstaged_count,
        );
        let full_message = assemble_commit_message(
            &ai_analysis.title,
            &ai_analysis.body,
            &ai_analysis.breaking_changes,
        );

        Ok(CommitMessage {
            title: ai_analysis.title,
            body: ai_analysis.body,
            footer: ai_analysis.breaking_changes,
            full_message,
            commit_type: ai_analysis.commit_type,
            scope: ai_analysis.scope,
            confidence: ai_analysis.confidence,
            changes_summary,
        })
    }
}
