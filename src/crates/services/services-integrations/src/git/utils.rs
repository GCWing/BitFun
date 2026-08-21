/**
 * Git utility functions
 */
use super::trust;
pub use super::{
    build_git_changed_files_args, build_git_diff_args, parse_branch_line, parse_git_log_line,
};
use super::{GitCommandOutput, GitError, GitFileStatus};
use bitfun_services_core::process_manager;
use git2::{Repository, Status, StatusOptions};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::time::timeout;

const REVIEW_GIT_TIMEOUT: Duration = Duration::from_secs(30);
const REVIEW_GIT_OUTPUT_LIMIT: usize = 8 * 1024 * 1024;

async fn read_bounded_review_git_stream<R>(reader: R) -> Result<Vec<u8>, GitError>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = Vec::new();
    let mut limited = reader.take((REVIEW_GIT_OUTPUT_LIMIT + 1) as u64);
    limited
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| GitError::CommandFailed(format!("Failed to read Git output: {error}")))?;
    if bytes.len() > REVIEW_GIT_OUTPUT_LIMIT {
        return Err(GitError::CommandFailed(
            "Review Git inspection output exceeded the 8 MiB safety limit".to_string(),
        ));
    }
    Ok(bytes)
}

/// Opens a repository, keeping an ownership rejection distinguishable from a
/// missing repository.
pub fn open_repository<P: AsRef<Path>>(path: P) -> Result<Repository, GitError> {
    let path = path.as_ref();
    Repository::open(path).map_err(|error| trust::classify_repository_open_error(path, &error))
}

/// Discovers the repository containing `path`, keeping an ownership rejection
/// distinguishable from a missing repository.
pub fn discover_repository<P: AsRef<Path>>(path: P) -> Result<Repository, GitError> {
    let path = path.as_ref();
    Repository::discover(path).map_err(|error| trust::classify_repository_open_error(path, &error))
}

/// Returns whether the given path is a Git repository.
///
/// A repository whose owner differs from the current user still is one: Git
/// only refuses to operate on it. Reporting `false` there would hide the real
/// reason behind "not a repository" and strip every recovery affordance.
pub fn is_git_repository<P: AsRef<Path>>(path: P) -> bool {
    match open_repository(path) {
        Ok(_) => true,
        Err(error) => error.untrusted_repository_path().is_some(),
    }
}

/// Returns the repository root directory.
pub fn get_repository_root<P: AsRef<Path>>(path: P) -> Result<String, GitError> {
    let requested = path.as_ref();
    let lexical_start = if requested.is_file() {
        requested.parent().unwrap_or(requested)
    } else {
        requested
    };
    for root in lexical_start
        .ancestors()
        .filter(|ancestor| ancestor.join(".git").exists())
    {
        // An ownership rejection still identifies a real repository root; the
        // operation that needs it will fail with the actionable trust error.
        match open_repository(root) {
            Ok(_) => return Ok(root.to_string_lossy().to_string()),
            Err(error) if error.untrusted_repository_path().is_some() => {
                return Ok(root.to_string_lossy().to_string())
            }
            Err(_) => {}
        }
    }

    let repo = discover_repository(requested)?;

    let workdir = repo
        .workdir()
        .ok_or_else(|| GitError::InvalidPath("Repository has no working directory".to_string()))?;

    Ok(workdir.to_string_lossy().to_string())
}

/// Returns the current branch name.
pub fn get_current_branch(repo: &Repository) -> Result<String, GitError> {
    match repo.head() {
        Ok(head) => {
            if let Ok(branch_name) = head.shorthand() {
                Ok(branch_name.to_string())
            } else {
                Ok("HEAD".to_string())
            }
        }
        Err(e) => {
            if e.code() == git2::ErrorCode::UnbornBranch {
                if let Ok(config) = repo.config() {
                    if let Ok(default_branch) = config.get_string("init.defaultBranch") {
                        return Ok(default_branch);
                    }
                }
                Ok("master".to_string())
            } else {
                Err(GitError::CommandFailed(format!(
                    "Failed to get HEAD: {}",
                    e
                )))
            }
        }
    }
}

/// Converts Git status flags to a short string.
pub fn status_to_string(status: Status) -> String {
    let mut result = Vec::new();

    if status.contains(Status::CONFLICTED) {
        result.push("C");
    }

    if status.contains(Status::INDEX_NEW) {
        result.push("A");
    }
    if status.contains(Status::INDEX_MODIFIED) {
        result.push("M");
    }
    if status.contains(Status::INDEX_DELETED) {
        result.push("D");
    }
    if status.contains(Status::INDEX_RENAMED) {
        result.push("R");
    }
    if status.contains(Status::INDEX_TYPECHANGE) {
        result.push("T");
    }

    if status.contains(Status::WT_NEW) {
        result.push("?");
    }
    if status.contains(Status::WT_MODIFIED) {
        result.push("M");
    }
    if status.contains(Status::WT_DELETED) {
        result.push("D");
    }
    if status.contains(Status::WT_RENAMED) {
        result.push("R");
    }
    if status.contains(Status::WT_TYPECHANGE) {
        result.push("T");
    }

    if result.is_empty() {
        "U".to_string()
    } else {
        result.join("")
    }
}

/// Maximum number of untracked entries before we stop recursing into untracked
/// directories. When the non-recursive scan already reports many untracked
/// top-level entries, recursing would return thousands of paths that bloat IPC
/// payloads and DOM rendering, causing severe UI lag.
const UNTRACKED_RECURSE_THRESHOLD: usize = 200;

/// Collects file statuses from a `StatusOptions` scan.
fn collect_statuses(
    repo: &Repository,
    recurse_untracked: bool,
) -> Result<Vec<GitFileStatus>, GitError> {
    let mut status_options = StatusOptions::new();
    status_options.include_untracked(true);
    status_options.include_ignored(false);
    status_options.recurse_untracked_dirs(recurse_untracked);

    let statuses = repo
        .statuses(Some(&mut status_options))
        .map_err(|e| GitError::CommandFailed(format!("Failed to get statuses: {}", e)))?;

    let mut result = Vec::new();

    for entry in statuses.iter() {
        if let Ok(path) = entry.path() {
            let status = entry.status();
            let status_str = status_to_string(status);

            let index_status = if status.intersects(
                Status::INDEX_NEW
                    | Status::INDEX_MODIFIED
                    | Status::INDEX_DELETED
                    | Status::INDEX_RENAMED
                    | Status::INDEX_TYPECHANGE,
            ) || status.contains(Status::CONFLICTED)
            {
                Some(status_to_string(
                    status
                        & (Status::INDEX_NEW
                            | Status::INDEX_MODIFIED
                            | Status::INDEX_DELETED
                            | Status::INDEX_RENAMED
                            | Status::INDEX_TYPECHANGE
                            | Status::CONFLICTED),
                ))
            } else {
                None
            };

            let workdir_status = if status.intersects(
                Status::WT_NEW
                    | Status::WT_MODIFIED
                    | Status::WT_DELETED
                    | Status::WT_RENAMED
                    | Status::WT_TYPECHANGE,
            ) || status.contains(Status::CONFLICTED)
            {
                Some(status_to_string(
                    status
                        & (Status::WT_NEW
                            | Status::WT_MODIFIED
                            | Status::WT_DELETED
                            | Status::WT_RENAMED
                            | Status::WT_TYPECHANGE
                            | Status::CONFLICTED),
                ))
            } else {
                None
            };

            result.push(GitFileStatus {
                path: path.to_string(),
                status: status_str,
                index_status,
                workdir_status,
            });
        }
    }

    Ok(result)
}

/// Returns file statuses.
///
/// Uses a two-pass strategy to avoid expensive recursive scans when the
/// repository contains many untracked files (e.g. missing .gitignore for
/// build artifacts). First a non-recursive pass counts top-level untracked
/// entries; only when that count is within `UNTRACKED_RECURSE_THRESHOLD` does
/// a second recursive pass run.
pub fn get_file_statuses(repo: &Repository) -> Result<Vec<GitFileStatus>, GitError> {
    // Pass 1: fast non-recursive scan.
    let shallow = collect_statuses(repo, false)?;

    let untracked_count = shallow.iter().filter(|f| f.status.contains('?')).count();

    if untracked_count <= UNTRACKED_RECURSE_THRESHOLD {
        // Few untracked entries – safe to recurse for full detail.
        collect_statuses(repo, true)
    } else {
        // Too many untracked entries – return the shallow result as-is.
        // Untracked directories appear as a single entry (folder name with
        // trailing slash) which is sufficient for the UI.
        Ok(shallow)
    }
}

/// Executes a Git command and returns the raw output including exit code.
///
/// Git diff returns exit code 1 when there are differences (not an error).
/// Callers that need to distinguish this case should inspect `exit_code`.
pub async fn execute_git_command_raw(
    repo_path: &str,
    args: &[&str],
) -> Result<GitCommandOutput, GitError> {
    // Ownership-trust classification matches Git's English diagnostics, so
    // every in-product Git call pins the locale instead of inheriting the
    // user's (possibly localized) one.
    let output = process_manager::create_tokio_command("git")
        .current_dir(repo_path)
        .env("LC_ALL", "C")
        .args(args)
        .output()
        .await
        .map_err(|e| GitError::CommandFailed(format!("Failed to execute git command: {}", e)))?;

    Ok(GitCommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

/// Executes a Git command.
///
/// For most git commands, exit code 0 means success and anything else is an error.
/// However, `git diff` returns exit code 1 when there are differences, which is
/// not an error. Use [`execute_git_command_raw`] if you need to handle that case.
pub async fn execute_git_command(repo_path: &str, args: &[&str]) -> Result<String, GitError> {
    let result = execute_git_command_raw(repo_path, args).await?;

    if result.exit_code == 0 {
        Ok(result.stdout)
    } else {
        let error = if result.stderr.is_empty() {
            result.stdout
        } else {
            result.stderr
        };
        Err(trust::classify_command_failure(repo_path, error))
    }
}

/// Executes a bounded read-only Git command without optional locks or external
/// diff/text-conversion helpers. Callers must still provide a fixed operation
/// and validated arguments rather than forwarding arbitrary command strings.
pub async fn execute_git_readonly_command(
    repo_path: &str,
    args: &[&str],
) -> Result<String, GitError> {
    execute_git_hardened_command_with_env(repo_path, args, &[]).await
}

/// Runs a Git command under the hardened, non-interactive environment every
/// in-product probe shares: no optional locks, no terminal prompts, no lazy
/// fetches, and English diagnostics (`LC_ALL=C`) so [`trust`] can classify the
/// output regardless of the user's locale. Despite the read-only callers, the
/// environment itself makes no such promise: the trust module deliberately
/// reuses it for the confirmed `safe.directory` config write, which needs the
/// same prompt-free, bounded execution.
pub(crate) async fn execute_git_hardened_command_with_env(
    repo_path: &str,
    args: &[&str],
    env: &[(&str, &str)],
) -> Result<String, GitError> {
    let mut command = process_manager::create_tokio_command("git");
    command
        .current_dir(repo_path)
        .env("LC_ALL", "C")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("GIT_LITERAL_PATHSPECS", "1")
        .env_remove("GIT_EXTERNAL_DIFF")
        .args(args)
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        command.env(key, value);
    }
    let mut child = command.spawn().map_err(|error| {
        GitError::CommandFailed(format!("Failed to execute Git inspection: {error}"))
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        GitError::CommandFailed("Failed to capture Git inspection stdout".to_string())
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        GitError::CommandFailed("Failed to capture Git inspection stderr".to_string())
    })?;

    let bounded_output = timeout(REVIEW_GIT_TIMEOUT, async {
        tokio::try_join!(
            read_bounded_review_git_stream(stdout),
            read_bounded_review_git_stream(stderr),
            async {
                child.wait().await.map_err(|error| {
                    GitError::CommandFailed(format!("Failed to wait for Git inspection: {error}"))
                })
            },
        )
    })
    .await;

    let (stdout, stderr, status) = match bounded_output {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            let _ = child.kill().await;
            return Err(error);
        }
        Err(_) => {
            let _ = child.kill().await;
            return Err(GitError::CommandFailed(
                "Review Git inspection timed out".to_string(),
            ));
        }
    };

    if status.success() {
        Ok(String::from_utf8_lossy(&stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&stdout).trim().to_string();
        Err(trust::classify_command_failure(
            repo_path,
            if stderr.is_empty() { stdout } else { stderr },
        ))
    }
}

/// Executes a Git command synchronously and returns the raw output including exit code.
pub fn execute_git_command_sync_raw(
    repo_path: &str,
    args: &[&str],
) -> Result<GitCommandOutput, GitError> {
    // English diagnostics, see execute_git_command_raw.
    let output = process_manager::create_command("git")
        .current_dir(repo_path)
        .env("LC_ALL", "C")
        .args(args)
        .output()
        .map_err(|e| GitError::CommandFailed(format!("Failed to execute git command: {}", e)))?;

    Ok(GitCommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

/// Executes a Git command synchronously, giving up after `timeout`.
///
/// Only for commands whose output is a handful of bytes: stdout and stderr are
/// piped and drained after exit, so a command that filled a pipe buffer while
/// still running would deadlock against this poll loop rather than time out.
///
/// On expiry the child is killed and reaped, so nothing is left behind to hold
/// the repository lock or a worker thread.
pub fn execute_git_command_sync_with_timeout(
    repo_path: &str,
    args: &[&str],
    timeout: std::time::Duration,
) -> Result<String, GitError> {
    use std::process::Stdio;

    let mut child = process_manager::create_command("git")
        .current_dir(repo_path)
        // English diagnostics, see execute_git_command_raw.
        .env("LC_ALL", "C")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| GitError::CommandFailed(format!("Failed to execute git command: {}", e)))?;

    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(e) => {
                return Err(GitError::CommandFailed(format!(
                    "Failed to wait for git command: {}",
                    e
                )))
            }
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(GitError::CommandFailed(format!(
                "git {} timed out after {:?}",
                args.join(" "),
                timeout
            )));
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    let output = child
        .wait_with_output()
        .map_err(|e| GitError::CommandFailed(format!("Failed to read git output: {}", e)))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Err(trust::classify_command_failure(
        repo_path,
        if stderr.is_empty() { stdout } else { stderr },
    ))
}

/// Executes a Git command synchronously.
pub fn execute_git_command_sync(repo_path: &str, args: &[&str]) -> Result<String, GitError> {
    let result = execute_git_command_sync_raw(repo_path, args)?;

    if result.exit_code == 0 {
        Ok(result.stdout)
    } else {
        let error = if result.stderr.is_empty() {
            result.stdout
        } else {
            result.stderr
        };
        Err(trust::classify_command_failure(repo_path, error))
    }
}

/// Formats a timestamp.
pub fn format_timestamp(timestamp: i64) -> String {
    use chrono::{TimeZone, Utc};

    match Utc.timestamp_opt(timestamp, 0) {
        chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        _ => "Invalid date".to_string(),
    }
}

/// Checks whether Git is available.
pub fn check_git_available() -> bool {
    process_manager::create_command("git")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod review_git_output_tests {
    use super::*;

    #[test]
    fn repository_root_ignores_an_invalid_lexical_git_marker() {
        let temp = tempfile::tempdir().expect("tempdir");
        let nested = temp.path().join("nested");
        std::fs::create_dir_all(temp.path().join(".git")).expect("fake git marker");
        std::fs::create_dir_all(&nested).expect("nested directory");

        // The lexical probe walks every ancestor that carries a `.git`
        // entry. On developer machines whose home directory is itself a git
        // checkout, the walk escapes the tempdir and resolves the outer
        // repository, so the strict `Err` expectation cannot hold there. The
        // assertion that matters is that the *invalid* marker is never
        // adopted as the repository root; CI environments (no repository
        // ancestor) keep the strict expectation.
        let host_has_repository_ancestor = std::env::temp_dir()
            .ancestors()
            .any(|ancestor| ancestor.join(".git").exists());
        match get_repository_root(&nested) {
            Ok(root) => {
                assert_ne!(
                    root,
                    temp.path().to_string_lossy().to_string(),
                    "invalid lexical .git marker must not be adopted as the repository root"
                );
                if !host_has_repository_ancestor {
                    panic!("expected an error in an environment without a repository ancestor");
                }
            }
            Err(_) => {
                assert!(
                    !host_has_repository_ancestor,
                    "unexpected error: the host environment provides no repository ancestor either"
                );
            }
        }
    }

    #[test]
    fn repository_root_preserves_a_valid_lexical_worktree_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        Repository::init(temp.path()).expect("repository");
        let nested = temp.path().join("nested");
        std::fs::create_dir_all(&nested).expect("nested directory");

        assert_eq!(
            get_repository_root(&nested).expect("repository root"),
            temp.path().to_string_lossy()
        );
    }

    #[tokio::test]
    async fn bounded_reader_rejects_output_before_unbounded_buffering() {
        let reader = tokio::io::repeat(b'x');
        let error = read_bounded_review_git_stream(reader)
            .await
            .expect_err("unbounded output must be rejected");
        assert!(error.to_string().contains("8 MiB safety limit"));
    }
}
