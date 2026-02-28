/**
 * Git utility functions
 */
use super::git_types::{GitError, GitFileStatus};
use git2::{Repository, Status, StatusOptions};
use std::path::Path;

/// Returns whether the given path is a Git repository.
pub fn is_git_repository<P: AsRef<Path>>(path: P) -> bool {
    Repository::open(path).is_ok()
}

/// Returns the repository root directory.
pub fn get_repository_root<P: AsRef<Path>>(path: P) -> Result<String, GitError> {
    let repo =
        Repository::discover(path).map_err(|e| GitError::RepositoryNotFound(e.to_string()))?;

    let workdir = repo
        .workdir()
        .ok_or_else(|| GitError::InvalidPath("Repository has no working directory".to_string()))?;

    Ok(workdir.to_string_lossy().to_string())
}

/// Returns the current branch name.
pub fn get_current_branch(repo: &Repository) -> Result<String, GitError> {
    match repo.head() {
        Ok(head) => {
            if let Some(branch_name) = head.shorthand() {
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
fn collect_statuses(repo: &Repository, recurse_untracked: bool) -> Result<Vec<GitFileStatus>, GitError> {
    let mut status_options = StatusOptions::new();
    status_options.include_untracked(true);
    status_options.include_ignored(false);
    status_options.recurse_untracked_dirs(recurse_untracked);

    let statuses = repo
        .statuses(Some(&mut status_options))
        .map_err(|e| GitError::CommandFailed(format!("Failed to get statuses: {}", e)))?;

    let mut result = Vec::new();

    for entry in statuses.iter() {
        if let Some(path) = entry.path() {
            let status = entry.status();
            let status_str = status_to_string(status);

            let index_status = if status.intersects(
                Status::INDEX_NEW
                    | Status::INDEX_MODIFIED
                    | Status::INDEX_DELETED
                    | Status::INDEX_RENAMED
                    | Status::INDEX_TYPECHANGE,
            ) {
                Some(status_to_string(
                    status
                        & (Status::INDEX_NEW
                            | Status::INDEX_MODIFIED
                            | Status::INDEX_DELETED
                            | Status::INDEX_RENAMED
                            | Status::INDEX_TYPECHANGE),
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
            ) {
                Some(status_to_string(
                    status
                        & (Status::WT_NEW
                            | Status::WT_MODIFIED
                            | Status::WT_DELETED
                            | Status::WT_RENAMED
                            | Status::WT_TYPECHANGE),
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

/// Executes a Git command.
pub async fn execute_git_command(repo_path: &str, args: &[&str]) -> Result<String, GitError> {
    let output = crate::util::process_manager::create_tokio_command("git")
        .current_dir(repo_path)
        .args(args)
        .output()
        .await
        .map_err(|e| GitError::CommandFailed(format!("Failed to execute git command: {}", e)))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        Err(GitError::CommandFailed(error.to_string()))
    }
}

/// Executes a Git command synchronously.
pub fn execute_git_command_sync(repo_path: &str, args: &[&str]) -> Result<String, GitError> {
    let output = crate::util::process_manager::create_command("git")
        .current_dir(repo_path)
        .args(args)
        .output()
        .map_err(|e| GitError::CommandFailed(format!("Failed to execute git command: {}", e)))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        Err(GitError::CommandFailed(error.to_string()))
    }
}

/// Parses a Git log line.
pub fn parse_git_log_line(line: &str) -> Option<(String, String, String, String, String)> {
    let parts: Vec<&str> = line.splitn(5, '|').collect();
    if parts.len() == 5 {
        Some((
            parts[0].to_string(),
            parts[1].to_string(),
            parts[2].to_string(),
            parts[3].to_string(),
            parts[4].to_string(),
        ))
    } else {
        None
    }
}

/// Parses a Git branch line.
pub fn parse_branch_line(line: &str) -> Option<(String, bool)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with("* ") {
        Some((trimmed[2..].to_string(), true))
    } else if trimmed.starts_with("  ") {
        Some((trimmed[2..].to_string(), false))
    } else {
        Some((trimmed.to_string(), false))
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
    crate::util::process_manager::create_command("git")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
