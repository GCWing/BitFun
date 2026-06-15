//! Function-agent concrete integration services.
//!
//! Product-domain crates own prompt, parser, and facade policy. This module
//! owns concrete Git snapshots for function agents without depending on
//! `bitfun-core`.

use std::path::{Path, PathBuf};

use bitfun_product_domains::function_agents::common::{AgentError, AgentResult};
use bitfun_product_domains::function_agents::git_func_agent::ContextAnalyzer;
use bitfun_product_domains::function_agents::ports::{
    GitCommitSnapshot, StartchatGitSnapshot, StartchatTimeSnapshot,
};
use bitfun_product_domains::function_agents::startchat_func_agent::AheadBehind;
use bitfun_services_core::process_manager;

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

    pub async fn startchat_git_snapshot(repo_path: PathBuf) -> AgentResult<StartchatGitSnapshot> {
        let current_branch = git_stdout_lenient(&repo_path, &["branch", "--show-current"])?
            .trim()
            .to_string();
        let status_porcelain = git_stdout_lenient(&repo_path, &["status", "--porcelain"])?;
        let unstaged_diff = git_stdout_lenient(&repo_path, &["diff", "HEAD"])?;
        let staged_diff = git_stdout_lenient(&repo_path, &["diff", "--cached"])?;
        let unpushed_commits = git_unpushed_commits(&repo_path);
        let ahead_behind = git_ahead_behind(&repo_path);
        let last_commit_timestamp = git_last_commit_timestamp(&repo_path);

        Ok(StartchatGitSnapshot {
            current_branch,
            status_porcelain,
            unstaged_diff,
            staged_diff,
            unpushed_commits,
            ahead_behind,
            last_commit_timestamp,
        })
    }

    pub fn startchat_time_snapshot(repo_path: &Path) -> StartchatTimeSnapshot {
        StartchatTimeSnapshot {
            last_commit_timestamp: git_last_commit_timestamp(repo_path),
        }
    }
}

fn git_stdout_lenient(repo_path: &Path, args: &[&str]) -> AgentResult<String> {
    let output = process_manager::create_command("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| AgentError::git_error(format!("Failed to run git {:?}: {}", args, e)))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_unpushed_commits(repo_path: &Path) -> u32 {
    let output = process_manager::create_command("git")
        .arg("log")
        .arg("@{u}..")
        .arg("--oneline")
        .current_dir(repo_path)
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            return String::from_utf8_lossy(&output.stdout).lines().count() as u32;
        }
    }

    0
}

fn git_ahead_behind(repo_path: &Path) -> Option<AheadBehind> {
    let output = process_manager::create_command("git")
        .arg("rev-list")
        .arg("--left-right")
        .arg("--count")
        .arg("HEAD...@{u}")
        .current_dir(repo_path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let result = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = result.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    Some(AheadBehind {
        ahead: parts[0].parse().unwrap_or(0),
        behind: parts[1].parse().unwrap_or(0),
    })
}

fn git_last_commit_timestamp(repo_path: &Path) -> Option<i64> {
    let output = process_manager::create_command("git")
        .arg("log")
        .arg("-1")
        .arg("--format=%ct")
        .current_dir(repo_path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<i64>()
        .ok()
}
