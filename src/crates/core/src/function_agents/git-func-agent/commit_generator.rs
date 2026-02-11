use super::ai_service::AIAnalysisService;
use super::context_analyzer::ContextAnalyzer;
use super::types::*;
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

        let changes_summary = Self::build_changes_summary(&status, &changed_files);

        let full_message = Self::assemble_full_message(
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

    fn build_changes_summary(
        status: &crate::service::git::GitStatus,
        changed_files: &[String],
    ) -> ChangesSummary {
        let total_additions = status.staged.iter().map(|_| 10u32).sum::<u32>()
            + status.unstaged.iter().map(|_| 10u32).sum::<u32>();
        let total_deletions = status.staged.iter().map(|_| 5u32).sum::<u32>()
            + status.unstaged.iter().map(|_| 5u32).sum::<u32>();

        let file_changes: Vec<FileChange> = changed_files
            .iter()
            .map(|path| {
                let file_type = super::utils::infer_file_type(path);
                FileChange {
                    path: path.clone(),
                    change_type: FileChangeType::Modified, // Simplified handling
                    additions: 10,
                    deletions: 5,
                    file_type,
                }
            })
            .collect();

        let affected_modules: Vec<String> = changed_files
            .iter()
            .filter_map(|path| super::utils::extract_module_name(path))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .take(3)
            .collect();

        let change_patterns = super::utils::detect_change_patterns(&file_changes);

        ChangesSummary {
            total_additions,
            total_deletions,
            files_changed: changed_files.len() as u32,
            file_changes,
            affected_modules,
            change_patterns,
        }
    }

    fn assemble_full_message(
        title: &str,
        body: &Option<String>,
        footer: &Option<String>,
    ) -> String {
        let mut parts = vec![title.to_string()];

        if let Some(body_text) = body {
            if !body_text.is_empty() {
                parts.push(String::new());
                parts.push(body_text.clone());
            }
        }

        if let Some(footer_text) = footer {
            if !footer_text.is_empty() {
                parts.push(String::new());
                parts.push(footer_text.clone());
            }
        }

        parts.join("\n")
    }
}
