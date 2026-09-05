//! Function-agent concrete integration services.
//!
//! Product-domain crates own prompt, parser, and facade policy. This module
//! owns concrete Git snapshots for function agents without depending on
//! `openbitfun-core`.

use std::path::PathBuf;

use openbitfun_product_domains::function_agents::common::{AgentError, AgentResult};
use openbitfun_product_domains::function_agents::git_func_agent::ContextAnalyzer;
use openbitfun_product_domains::function_agents::ports::GitCommitSnapshot;

use crate::git::{GitDiffParams, GitService};

#[derive(Debug, Default, Clone)]
pub struct FunctionAgentGitService;

impl FunctionAgentGitService {
    pub async fn git_commit_snapshot(repo_path: PathBuf) -> AgentResult<GitCommitSnapshot> {
        let status = GitService::get_status(&repo_path)
            .await
            .map_err(|e| AgentError::git_error(format!("Failed to get Git status: {}", e)))?;

        let diff_params = GitDiffParams {
            staged: Some(true),
            stat: Some(false),
            files: None,
            ..Default::default()
        };
        let diff_content = GitService::get_diff(&repo_path, &diff_params)
            .await
            .map_err(|e| AgentError::git_error(format!("Failed to get diff: {}", e)))?;

        let project_context = ContextAnalyzer::analyze_project_context(&repo_path)
            .await
            .unwrap_or_default();

        Ok(GitCommitSnapshot {
            staged_paths: status.staged.iter().map(|file| file.path.clone()).collect(),
            staged_count: status.staged.len(),
            unstaged_count: status.unstaged.len(),
            diff_content,
            project_context,
        })
    }
}
