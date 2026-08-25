#![cfg(feature = "function-agents")]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use bitfun_services_integrations::function_agents::FunctionAgentGitService;

struct TestTempDir {
    path: PathBuf,
}

impl TestTempDir {
    fn new(label: &str) -> Self {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "bitfun-function-agent-service-{}-{}-{}",
            label,
            std::process::id(),
            suffix
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
async fn git_service_builds_commit_snapshot_from_staged_diff_without_unstaged_content() {
    let repo = TestTempDir::new("commit-snapshot");
    init_git_repo(repo.path());
    fs::write(repo.path().join("tracked.txt"), "base\n").unwrap();
    git(repo.path(), &["add", "tracked.txt"]);
    git(repo.path(), &["commit", "-m", "initial"]);

    fs::write(repo.path().join("tracked.txt"), "base\nunstaged only\n").unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::write(repo.path().join("src/lib.rs"), "pub fn staged() {}\n").unwrap();
    git(repo.path(), &["add", "src/lib.rs"]);

    let snapshot = FunctionAgentGitService::git_commit_snapshot(repo.path().to_path_buf())
        .await
        .unwrap();

    assert_eq!(snapshot.staged_paths, vec!["src/lib.rs".to_string()]);
    assert_eq!(snapshot.staged_count, 1);
    assert_eq!(snapshot.unstaged_count, 1);
    assert!(snapshot.diff_content.contains("src/lib.rs"));
    assert!(snapshot.diff_content.contains("pub fn staged()"));
    assert!(!snapshot.diff_content.contains("unstaged only"));
}

fn init_git_repo(repo: &Path) {
    git(repo, &["init", "-b", "main"]);
    git(repo, &["config", "user.email", "test@example.com"]);
    git(repo, &["config", "user.name", "BitFun Test"]);
}

fn git(repo: &Path, args: &[&str]) {
    git_with_env(repo, args, &[]);
}

fn git_with_env(repo: &Path, args: &[&str], envs: &[(&str, &str)]) {
    let mut command = bitfun_services_core::process_manager::create_command("git");
    command.args(args).current_dir(repo);
    for (key, value) in envs {
        command.env(key, value);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout={}\nstderr={}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
