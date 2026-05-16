use super::ai_service::AIAnalysisService;
use super::context_analyzer::ContextAnalyzer;
use super::types::*;
use crate::function_agents::common::{AgentError, AgentResult};
use crate::infrastructure::ai::AIClientFactory;
use crate::service::git::{GitDiffParams, GitService};
/**
 * Git Function Agent - commit message generator
 *
 * Uses AI to deeply analyze code changes and generate compliant commit messages
 */
use log::{debug, info};
use std::path::Path;
use std::sync::Arc;

pub struct CommitGenerator;

impl CommitGenerator {
    pub async fn generate_commit_message(
        repo_path: &Path,
        options: CommitMessageOptions,
        factory: Arc<AIClientFactory>,
    ) -> AgentResult<CommitMessage> {
        info!(
            "Generating commit message (AI-driven): repo_path={:?}",
            repo_path
        );

        let status = GitService::get_status(repo_path)
            .await
            .map_err(|e| AgentError::git_error(format!("Failed to get Git status: {}", e)))?;

        let changed_files: Vec<String> = status.staged.iter().map(|f| f.path.clone()).collect();

        if changed_files.is_empty() {
            return Err(AgentError::invalid_input(
                "Staging area is empty, please stage files first",
            ));
        }

        debug!(
            "Staged files: count={}, files={:?}",
            changed_files.len(),
            changed_files
        );

        let diff_content = Self::get_full_diff(repo_path).await?;

        if diff_content.trim().is_empty() {
            return Err(AgentError::invalid_input("Diff content is empty"));
        }

        let project_context = ContextAnalyzer::analyze_project_context(repo_path)
            .await
            .unwrap_or_default(); // Fallback to default on failure

        debug!(
            "Project context: type={}, tech_stack={:?}",
            project_context.project_type, project_context.tech_stack
        );

        let ai_service =
            AIAnalysisService::new_with_agent_config(factory, "git-func-agent").await?;

        let ai_analysis = ai_service
            .generate_commit_message_ai(&diff_content, &project_context, &options)
            .await?;

        debug!(
            "AI analysis completed: commit_type={:?}, confidence={}",
            ai_analysis.commit_type, ai_analysis.confidence
        );

        let changes_summary = super::utils::build_changes_summary_from_paths(
            &changed_files,
            status.staged.len(),
            status.unstaged.len(),
        );

        let full_message = super::utils::assemble_commit_message(
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

    async fn get_full_diff(repo_path: &Path) -> AgentResult<String> {
        let diff_params = GitDiffParams {
            staged: Some(true),
            stat: Some(false),
            files: None,
            ..Default::default()
        };

        let diff = GitService::get_diff(repo_path, &diff_params)
            .await
            .map_err(|e| AgentError::git_error(format!("Failed to get diff: {}", e)))?;

        debug!("Got staged diff: length={} chars", diff.len());
        Ok(diff)
    }
}
