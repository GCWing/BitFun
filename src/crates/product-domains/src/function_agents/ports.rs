//! Function-agent service ports for future runtime migration.
//!
//! The current core implementation still owns Git commands, AI clients, prompt
//! templates, JSON extraction, and error mapping. These ports define the seam
//! that future adapters must satisfy before those implementations move.

use crate::function_agents::common::{AgentError, AgentResult, Language};
use crate::function_agents::git_func_agent::{
    assemble_commit_message, build_changes_summary_from_paths, AICommitAnalysis, CommitMessage,
    CommitMessageOptions, ProjectContext,
};
use crate::function_agents::startchat_func_agent::{
    combine_git_diffs, parse_git_status_porcelain, time_of_day_for_hour, AIGeneratedAnalysis,
    AheadBehind, CurrentWorkState, GitWorkState, GreetingMessage, TimeInfo, WorkStateAnalysis,
    WorkStateOptions,
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
pub struct StartchatTimeSnapshot {
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
    fn git_commit_snapshot(&self, repo_path: PathBuf)
        -> FunctionAgentFuture<'_, GitCommitSnapshot>;
    fn startchat_git_snapshot(
        &self,
        repo_path: PathBuf,
    ) -> FunctionAgentFuture<'_, StartchatGitSnapshot>;
    fn startchat_time_snapshot(
        &self,
        repo_path: PathBuf,
    ) -> FunctionAgentFuture<'_, StartchatTimeSnapshot>;
}

/// Future AI boundary for function agents.
///
/// Core still owns AI client selection, prompt templates, response parsing, and
/// error mapping. Product call sites may route through this trait only after
/// focused equivalence tests cover the specific facade path.
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

/// Port-backed function-agent facade for future runtime owner migration.
///
/// It owns only pure orchestration over function-agent ports and DTO helpers.
/// Core still owns Git/AI service calls, prompt templates, JSON extraction,
/// and concrete error mapping. Startchat product-path rewiring depends on core
/// adapters preserving legacy Git state, diff fallback, time-info, and
/// `analyzed_at` timing semantics.
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

    pub async fn analyze_work_state(
        &self,
        repo_path: PathBuf,
        options: WorkStateOptions,
        now_timestamp: i64,
        current_hour: u32,
        analyzed_at: String,
    ) -> AgentResult<WorkStateAnalysis> {
        let time_snapshot = self
            .git
            .startchat_time_snapshot(repo_path.clone())
            .await
            .ok();
        let snapshot = if options.analyze_git {
            self.git.startchat_git_snapshot(repo_path).await.ok()
        } else {
            None
        };
        let git_state = snapshot.as_ref().map(git_work_state_from_snapshot);
        let git_diff = if git_state
            .as_ref()
            .is_some_and(|state| state.unstaged_files > 0 || state.staged_files > 0)
        {
            snapshot
                .as_ref()
                .map(|snapshot| combine_git_diffs(&snapshot.unstaged_diff, &snapshot.staged_diff))
                .unwrap_or_default()
        } else {
            String::new()
        };
        let time_info =
            time_info_from_snapshot(time_snapshot.as_ref(), now_timestamp, current_hour);

        let ai_analysis = self
            .ai
            .analyze_work_state(WorkStateAiAnalysisRequest {
                git_state: git_state.clone(),
                git_diff,
                language: options.language.clone(),
            })
            .await?;

        Ok(WorkStateAnalysis {
            greeting: GreetingMessage {
                title: String::new(),
                subtitle: String::new(),
                tagline: None,
            },
            current_state: CurrentWorkState {
                summary: ai_analysis.summary,
                git_state,
                ongoing_work: ai_analysis.ongoing_work,
                time_info,
            },
            predicted_actions: if options.predict_next_actions {
                ai_analysis.predicted_actions
            } else {
                Vec::new()
            },
            quick_actions: if options.include_quick_actions {
                ai_analysis.quick_actions
            } else {
                Vec::new()
            },
            analyzed_at,
        })
    }
}

pub fn git_work_state_from_snapshot(snapshot: &StartchatGitSnapshot) -> GitWorkState {
    let (unstaged_files, staged_files, modified_files) =
        parse_git_status_porcelain(&snapshot.status_porcelain);
    GitWorkState {
        current_branch: snapshot.current_branch.clone(),
        unstaged_files,
        staged_files,
        unpushed_commits: snapshot.unpushed_commits,
        ahead_behind: snapshot.ahead_behind.clone(),
        modified_files,
    }
}

pub fn time_info_from_snapshot(
    snapshot: Option<&StartchatTimeSnapshot>,
    now_timestamp: i64,
    current_hour: u32,
) -> TimeInfo {
    let minutes_since_last_commit = snapshot
        .and_then(|snapshot| snapshot.last_commit_timestamp)
        .map(|timestamp| (now_timestamp - timestamp) / 60)
        .map(|minutes| minutes as u64);

    TimeInfo {
        minutes_since_last_commit,
        last_commit_time_desc: None,
        time_of_day: time_of_day_for_hour(current_hour),
    }
}
