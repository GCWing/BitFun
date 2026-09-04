use std::path::{Path, PathBuf};

use openbitfun_services_core::process_manager;
use tokio::task::spawn_blocking;

pub(crate) const DEFAULT_AUTO_INDEX_MIN_FILES: usize = 2_000;
#[derive(Debug, Clone, Copy)]
pub(crate) struct AutoIndexPolicy {
    pub min_indexable_files: usize,
    pub max_file_size: u64,
}

impl Default for AutoIndexPolicy {
    fn default() -> Self {
        Self {
            min_indexable_files: DEFAULT_AUTO_INDEX_MIN_FILES,
            max_file_size: 50 * 1024 * 1024,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AutoIndexDecision {
    /// Counting stops as soon as the threshold is reached, so this is a lower
    /// bound rather than the workspace's real indexable file count.
    Eligible {
        indexable_files_at_least: usize,
    },
    BelowThreshold {
        indexable_files: usize,
    },
    Unsupported {
        reason: String,
    },
}

pub(crate) async fn evaluate(repo_root: PathBuf, policy: AutoIndexPolicy) -> AutoIndexDecision {
    match spawn_blocking(move || evaluate_blocking(&repo_root, policy)).await {
        Ok(decision) => decision,
        Err(error) => AutoIndexDecision::Unsupported {
            reason: format!("file-count worker failed: {error}"),
        },
    }
}

fn evaluate_blocking(repo_root: &Path, policy: AutoIndexPolicy) -> AutoIndexDecision {
    if policy.min_indexable_files == 0 {
        return AutoIndexDecision::Eligible {
            indexable_files_at_least: 0,
        };
    }

    match git_indexable_file_count(repo_root, policy) {
        Ok(count) => decision_for_count(count, policy.min_indexable_files),
        Err(reason) => AutoIndexDecision::Unsupported { reason },
    }
}

fn decision_for_count(count: usize, threshold: usize) -> AutoIndexDecision {
    if count >= threshold {
        AutoIndexDecision::Eligible {
            indexable_files_at_least: count,
        }
    } else {
        AutoIndexDecision::BelowThreshold {
            indexable_files: count,
        }
    }
}

/// Counts indexable files in two passes so that large workspaces never pay for
/// the untracked-file walk.
///
/// `--others` re-scans the whole worktree and dominates the cost: on
/// chromium-src it takes ~4.2 s against ~89 ms for `--cached` alone, which is
/// the same worktree scan the daemon already pays inside `open_repo`. Any
/// workspace that reaches the threshold on tracked files alone therefore skips
/// the second pass entirely; only genuinely small (or freshly cloned, mostly
/// untracked) workspaces need it, and there the walk is cheap.
fn git_indexable_file_count(repo_root: &Path, policy: AutoIndexPolicy) -> Result<usize, String> {
    let worktree_root = git_worktree_root(repo_root)?;
    let tracked = git_ls_files_indexable_count(&worktree_root, &["--cached"], policy, 0)?;
    if tracked >= policy.min_indexable_files {
        return Ok(tracked);
    }
    git_ls_files_indexable_count(
        &worktree_root,
        &["--others", "--exclude-standard"],
        policy,
        tracked,
    )
}

/// Runs one `git ls-files` selector and adds its indexable files to `carried`,
/// stopping as soon as the threshold is reached.
fn git_ls_files_indexable_count(
    worktree_root: &Path,
    selectors: &[&str],
    policy: AutoIndexPolicy,
    carried: usize,
) -> Result<usize, String> {
    let output = process_manager::create_command("git")
        .arg("ls-files")
        .args(selectors)
        .arg("-z")
        .current_dir(worktree_root)
        .output()
        .map_err(|error| format!("git ls-files unavailable: {error}"))?;
    if !output.status.success() {
        return Err(format!("git ls-files exited with {}", output.status));
    }

    let mut count = carried;
    for path in output.stdout.split(|byte| *byte == 0) {
        if path.is_empty() {
            continue;
        }
        let relative_path = String::from_utf8_lossy(path);
        let absolute_path = worktree_root.join(relative_path.as_ref());
        if is_indexable_file(&absolute_path, policy.max_file_size) {
            count += 1;
            if count >= policy.min_indexable_files {
                break;
            }
        }
    }
    Ok(count)
}

fn git_worktree_root(repo_root: &Path) -> Result<PathBuf, String> {
    let output = process_manager::create_command("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(repo_root)
        .output()
        .map_err(|error| format!("git worktree discovery unavailable: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git worktree discovery exited with {}",
            output.status
        ));
    }
    let root = String::from_utf8_lossy(&output.stdout);
    let root = root.trim();
    if root.is_empty() {
        return Err("git worktree discovery returned an empty root".to_string());
    }
    let worktree_root = dunce::canonicalize(root)
        .map_err(|error| format!("cannot canonicalize Git worktree root: {error}"))?;
    let head = process_manager::create_command("git")
        .args(["rev-parse", "--verify", "HEAD^{commit}"])
        .current_dir(&worktree_root)
        .output()
        .map_err(|error| format!("Git HEAD discovery unavailable: {error}"))?;
    if !head.status.success() {
        return Err("flashgrep requires a Git worktree with a HEAD commit".to_string());
    }
    Ok(worktree_root)
}

fn is_indexable_file(path: &Path, max_file_size: u64) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.len() <= max_file_size)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::write;
    use tempfile::TempDir;

    fn policy(min_indexable_files: usize, max_file_size: u64) -> AutoIndexPolicy {
        AutoIndexPolicy {
            min_indexable_files,
            max_file_size,
        }
    }

    #[test]
    fn git_count_uses_visible_tracked_and_untracked_files_and_size_limit() {
        let repo = TempDir::new().expect("temp repo");
        process_manager::create_command("git")
            .args(["init", "--quiet"])
            .current_dir(repo.path())
            .status()
            .expect("git should be available");
        write(repo.path().join("tracked.txt"), "tracked").expect("write tracked");
        write(repo.path().join("untracked.txt"), "untracked").expect("write untracked");
        write(repo.path().join("large.bin"), vec![0_u8; 160]).expect("write large file");
        process_manager::create_command("git")
            .args(["add", "tracked.txt"])
            .current_dir(repo.path())
            .status()
            .expect("git add should work");
        process_manager::create_command("git")
            .args([
                "-c",
                "user.name=OpenBitFun Test",
                "-c",
                "user.email=openbitfun-test@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "initial",
            ])
            .current_dir(repo.path())
            .status()
            .expect("git commit should work");

        assert_eq!(
            git_indexable_file_count(repo.path(), policy(10, 100)),
            Ok(2)
        );
        assert_eq!(git_indexable_file_count(repo.path(), policy(10, 8)), Ok(1));

        // Tracked files alone reach this threshold, so the untracked walk is
        // skipped and `untracked.txt` is never counted.
        assert_eq!(git_indexable_file_count(repo.path(), policy(1, 100)), Ok(1));
    }

    #[test]
    fn non_git_workspaces_are_explicitly_unsupported() {
        let repo = TempDir::new().expect("temp repo");
        let decision = evaluate_blocking(repo.path(), policy(2, 100));
        assert!(matches!(decision, AutoIndexDecision::Unsupported { .. }));
    }

    #[test]
    fn git_workspaces_without_a_head_are_unsupported() {
        let repo = TempDir::new().expect("temp repo");
        process_manager::create_command("git")
            .args(["init", "--quiet"])
            .current_dir(repo.path())
            .status()
            .expect("git should be available");

        let decision = evaluate_blocking(repo.path(), policy(1, 100));
        assert!(matches!(decision, AutoIndexDecision::Unsupported { .. }));
    }
}
