//! Core adapters for product-domain function-agent ports.

use std::path::{Path, PathBuf};

use bitfun_product_domains::function_agents::ports::{
    FunctionAgentFuture, FunctionAgentGitPort, GitCommitSnapshot, StartchatGitSnapshot,
};
use bitfun_product_domains::function_agents::startchat_func_agent::AheadBehind;

use crate::function_agents::common::{AgentError, AgentResult};
use crate::function_agents::git_func_agent::ContextAnalyzer;
use crate::service::git::{GitDiffParams, GitService};

#[derive(Debug, Default, Clone)]
pub struct CoreFunctionAgentGitAdapter;

impl FunctionAgentGitPort for CoreFunctionAgentGitAdapter {
    fn git_commit_snapshot(&self, repo_path: String) -> FunctionAgentFuture<'_, GitCommitSnapshot> {
        Box::pin(async move { Self::build_git_commit_snapshot(PathBuf::from(repo_path)).await })
    }

    fn startchat_git_snapshot(
        &self,
        repo_path: String,
    ) -> FunctionAgentFuture<'_, StartchatGitSnapshot> {
        Box::pin(async move { Self::build_startchat_git_snapshot(PathBuf::from(repo_path)).await })
    }
}

impl CoreFunctionAgentGitAdapter {
    async fn build_git_commit_snapshot(repo_path: PathBuf) -> AgentResult<GitCommitSnapshot> {
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

    async fn build_startchat_git_snapshot(repo_path: PathBuf) -> AgentResult<StartchatGitSnapshot> {
        let current_branch = git_stdout(&repo_path, &["branch", "--show-current"])?
            .trim()
            .to_string();
        let status_porcelain = git_stdout(&repo_path, &["status", "--porcelain"])?;
        let unstaged_diff = git_stdout(&repo_path, &["diff", "HEAD"])?;
        let staged_diff = git_stdout(&repo_path, &["diff", "--cached"])?;
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
}

fn git_stdout(repo_path: &Path, args: &[&str]) -> AgentResult<String> {
    let output = crate::util::process_manager::create_command("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| AgentError::git_error(format!("Failed to run git {:?}: {}", args, e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if !stderr.trim().is_empty() {
            stderr.trim()
        } else {
            stdout.trim()
        };
        return Err(AgentError::git_error(format!(
            "git {:?} failed with status {}: {}",
            args, output.status, detail
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_unpushed_commits(repo_path: &Path) -> u32 {
    let output = crate::util::process_manager::create_command("git")
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
    let output = crate::util::process_manager::create_command("git")
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
    let output = crate::util::process_manager::create_command("git")
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

#[cfg(test)]
mod tests {
    use bitfun_product_domains::function_agents::ports::FunctionAgentGitPort;
    use std::fs;
    use std::process::Command;

    use super::CoreFunctionAgentGitAdapter;

    #[tokio::test]
    async fn git_adapter_builds_commit_snapshot_from_existing_core_git_services() {
        let repo = temp_repo("commit-snapshot");
        init_git_repo(&repo);
        fs::write(repo.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(repo.join("src/lib.rs"), "pub fn demo() {}\n").unwrap();
        git(&repo, &["add", "Cargo.toml", "src/lib.rs"]);

        let adapter = CoreFunctionAgentGitAdapter::default();
        let snapshot = adapter
            .git_commit_snapshot(repo.to_string_lossy().to_string())
            .await
            .unwrap();

        assert!(snapshot.staged_paths.contains(&"Cargo.toml".to_string()));
        assert!(snapshot.staged_paths.contains(&"src/lib.rs".to_string()));
        assert_eq!(snapshot.staged_count, 2);
        assert_eq!(snapshot.unstaged_count, 0);
        assert!(snapshot.diff_content.contains("pub fn demo()"));
        assert_eq!(snapshot.project_context.project_type, "rust-application");
    }

    #[tokio::test]
    async fn git_adapter_builds_startchat_snapshot_without_changing_git_semantics() {
        let repo = temp_repo("startchat-snapshot");
        init_git_repo(&repo);
        fs::write(repo.join("tracked.txt"), "base\n").unwrap();
        git(&repo, &["add", "tracked.txt"]);
        git(&repo, &["commit", "-m", "initial"]);
        fs::write(repo.join("tracked.txt"), "base\nchange\n").unwrap();
        fs::write(repo.join("staged.txt"), "staged\n").unwrap();
        git(&repo, &["add", "staged.txt"]);

        let adapter = CoreFunctionAgentGitAdapter::default();
        let snapshot = adapter
            .startchat_git_snapshot(repo.to_string_lossy().to_string())
            .await
            .unwrap();

        assert_eq!(snapshot.current_branch, "main");
        assert!(snapshot.status_porcelain.contains(" M tracked.txt"));
        assert!(snapshot.status_porcelain.contains("A  staged.txt"));
        assert!(snapshot.unstaged_diff.contains("change"));
        assert!(snapshot.staged_diff.contains("staged.txt"));
        assert_eq!(snapshot.unpushed_commits, 0);
        assert!(snapshot.ahead_behind.is_none());
        assert!(snapshot.last_commit_timestamp.is_some());
    }

    #[tokio::test]
    async fn git_adapter_rejects_startchat_snapshot_when_git_command_fails() {
        let repo = temp_repo("not-a-git-repo");

        let adapter = CoreFunctionAgentGitAdapter::default();
        let result = adapter
            .startchat_git_snapshot(repo.to_string_lossy().to_string())
            .await;

        assert!(result.is_err());
    }

    fn temp_repo(label: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "bitfun-function-agent-port-{}-{}",
            label,
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn init_git_repo(repo: &std::path::Path) {
        git(repo, &["init", "-b", "main"]);
        git(repo, &["config", "user.email", "test@example.com"]);
        git(repo, &["config", "user.name", "BitFun Test"]);
    }

    fn git(repo: &std::path::Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?} failed\nstdout={}\nstderr={}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
