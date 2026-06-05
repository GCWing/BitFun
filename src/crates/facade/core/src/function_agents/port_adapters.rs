//! Core adapters for product-domain function-agent ports.

use std::path::PathBuf;
use std::sync::Arc;

use bitfun_product_domains::function_agents::ports::{
    CommitAiAnalysisRequest, FunctionAgentAiPort, FunctionAgentFuture, FunctionAgentGitPort,
    GitCommitSnapshot, StartchatGitSnapshot, StartchatTimeSnapshot, WorkStateAiAnalysisRequest,
};
use bitfun_product_domains::function_agents::{
    git_func_agent::AICommitAnalysis, startchat_func_agent::AIGeneratedAnalysis,
};
use bitfun_services_integrations::function_agents::FunctionAgentGitService;

use crate::infrastructure::ai::AIClientFactory;

#[derive(Debug, Default, Clone)]
pub struct CoreFunctionAgentGitAdapter;

impl FunctionAgentGitPort for CoreFunctionAgentGitAdapter {
    fn git_commit_snapshot(
        &self,
        repo_path: PathBuf,
    ) -> FunctionAgentFuture<'_, GitCommitSnapshot> {
        Box::pin(async move { FunctionAgentGitService::git_commit_snapshot(repo_path).await })
    }

    fn startchat_git_snapshot(
        &self,
        repo_path: PathBuf,
    ) -> FunctionAgentFuture<'_, StartchatGitSnapshot> {
        Box::pin(async move { FunctionAgentGitService::startchat_git_snapshot(repo_path).await })
    }

    fn startchat_time_snapshot(
        &self,
        repo_path: PathBuf,
    ) -> FunctionAgentFuture<'_, StartchatTimeSnapshot> {
        Box::pin(async move { Ok(FunctionAgentGitService::startchat_time_snapshot(&repo_path)) })
    }
}

#[derive(Clone)]
pub struct CoreFunctionAgentAiAdapter {
    factory: Arc<AIClientFactory>,
}

impl CoreFunctionAgentAiAdapter {
    pub fn new(factory: Arc<AIClientFactory>) -> Self {
        Self { factory }
    }
}

impl FunctionAgentAiPort for CoreFunctionAgentAiAdapter {
    fn analyze_commit(
        &self,
        request: CommitAiAnalysisRequest,
    ) -> FunctionAgentFuture<'_, AICommitAnalysis> {
        let factory = self.factory.clone();
        Box::pin(async move {
            let service = crate::function_agents::runtime_services::CoreCommitAiAnalysisService::new_with_agent_config(
                factory,
                "git-func-agent",
            )
            .await?;
            service
                .generate_commit_message_ai(
                    &request.diff_content,
                    &request.project_context,
                    &request.options,
                )
                .await
        })
    }

    fn analyze_work_state(
        &self,
        request: WorkStateAiAnalysisRequest,
    ) -> FunctionAgentFuture<'_, AIGeneratedAnalysis> {
        let factory = self.factory.clone();
        Box::pin(async move {
            let service = crate::function_agents::runtime_services::CoreWorkStateAiAnalysisService::new_with_agent_config(
                factory,
                "startchat-func-agent",
            )
            .await?;
            service
                .generate_complete_analysis(
                    &request.git_state,
                    &request.git_diff,
                    &request.language,
                )
                .await
        })
    }
}

#[cfg(test)]
mod tests {
    use bitfun_product_domains::function_agents::ports::FunctionAgentGitPort;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use crate::product_domain_runtime::CoreProductDomainRuntime;

    use super::CoreFunctionAgentGitAdapter;

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "bitfun-function-agent-port-{}-{}",
                label,
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[tokio::test]
    async fn git_adapter_builds_commit_snapshot_from_existing_core_git_services() {
        let repo = TestTempDir::new("commit-snapshot");
        init_git_repo(repo.path());
        fs::write(
            repo.path().join("Cargo.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .unwrap();
        fs::create_dir_all(repo.path().join("src")).unwrap();
        fs::write(repo.path().join("src/lib.rs"), "pub fn demo() {}\n").unwrap();
        git(repo.path(), &["add", "Cargo.toml", "src/lib.rs"]);

        let adapter = CoreFunctionAgentGitAdapter::default();
        let snapshot = adapter
            .git_commit_snapshot(repo.path().to_path_buf())
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
    async fn git_adapter_commit_snapshot_keeps_staged_diff_and_unstaged_count_separate() {
        let repo = TestTempDir::new("commit-snapshot-boundary");
        init_git_repo(repo.path());
        fs::write(repo.path().join("tracked.txt"), "base\n").unwrap();
        git(repo.path(), &["add", "tracked.txt"]);
        git(repo.path(), &["commit", "-m", "initial"]);

        fs::write(repo.path().join("tracked.txt"), "base\nunstaged only\n").unwrap();
        fs::write(repo.path().join("staged.txt"), "staged only\n").unwrap();
        git(repo.path(), &["add", "staged.txt"]);

        let adapter = CoreFunctionAgentGitAdapter::default();
        let snapshot = adapter
            .git_commit_snapshot(repo.path().to_path_buf())
            .await
            .unwrap();

        assert_eq!(snapshot.staged_paths, vec!["staged.txt".to_string()]);
        assert_eq!(snapshot.staged_count, 1);
        assert_eq!(snapshot.unstaged_count, 1);
        assert!(snapshot.diff_content.contains("staged.txt"));
        assert!(snapshot.diff_content.contains("staged only"));
        assert!(!snapshot.diff_content.contains("unstaged only"));
    }

    #[tokio::test]
    async fn git_adapter_builds_startchat_snapshot_without_changing_git_semantics() {
        let repo = TestTempDir::new("startchat-snapshot");
        init_git_repo(repo.path());
        fs::write(repo.path().join("tracked.txt"), "base\n").unwrap();
        git(repo.path(), &["add", "tracked.txt"]);
        git(repo.path(), &["commit", "-m", "initial"]);
        fs::write(repo.path().join("tracked.txt"), "base\nchange\n").unwrap();
        fs::write(repo.path().join("staged.txt"), "staged\n").unwrap();
        git(repo.path(), &["add", "staged.txt"]);

        let adapter = CoreFunctionAgentGitAdapter::default();
        let snapshot = adapter
            .startchat_git_snapshot(repo.path().to_path_buf())
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
    async fn git_adapter_startchat_snapshot_preserves_git_state_when_diff_has_no_head() {
        let repo = TestTempDir::new("startchat-no-head-diff");
        init_git_repo(repo.path());
        fs::write(repo.path().join("new.txt"), "new\n").unwrap();

        let adapter = CoreFunctionAgentGitAdapter::default();
        let snapshot = adapter
            .startchat_git_snapshot(repo.path().to_path_buf())
            .await
            .unwrap();

        assert_eq!(snapshot.current_branch, "main");
        assert!(snapshot.status_porcelain.contains("?? new.txt"));
        assert!(snapshot.unstaged_diff.is_empty());
        assert!(snapshot.staged_diff.is_empty());
        assert_eq!(snapshot.unpushed_commits, 0);
        assert!(snapshot.ahead_behind.is_none());
        assert!(snapshot.last_commit_timestamp.is_none());
    }

    #[tokio::test]
    async fn git_adapter_startchat_snapshot_matches_legacy_empty_state_when_not_git_repo() {
        let repo = TestTempDir::new("not-a-git-repo");

        let adapter = CoreFunctionAgentGitAdapter::default();
        let snapshot = adapter
            .startchat_git_snapshot(repo.path().to_path_buf())
            .await
            .unwrap();

        assert!(snapshot.current_branch.is_empty());
        assert!(snapshot.status_porcelain.is_empty());
        assert!(snapshot.unstaged_diff.is_empty());
        assert!(snapshot.staged_diff.is_empty());
        assert_eq!(snapshot.unpushed_commits, 0);
        assert!(snapshot.ahead_behind.is_none());
        assert!(snapshot.last_commit_timestamp.is_none());
    }

    #[test]
    fn core_product_domain_runtime_owner_constructs_function_agent_git_adapter() {
        let _adapter = CoreProductDomainRuntime::function_agent_git_adapter();
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
