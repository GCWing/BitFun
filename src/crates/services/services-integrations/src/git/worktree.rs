use super::types::GitWorktreeInfo;

/// Parses `git worktree list --porcelain` output.
pub fn parse_worktree_list(output: &str) -> Vec<GitWorktreeInfo> {
    let mut worktrees = Vec::new();
    let mut current_worktree: Option<GitWorktreeInfo> = None;

    for line in output.lines() {
        if line.starts_with("worktree ") {
            if let Some(wt) = current_worktree.take() {
                worktrees.push(wt);
            }
            let path = line.strip_prefix("worktree ").unwrap_or("").to_string();
            current_worktree = Some(GitWorktreeInfo {
                path,
                branch: None,
                head: String::new(),
                is_main: false,
                is_locked: false,
                is_prunable: false,
            });
        } else if let Some(ref mut wt) = current_worktree {
            if line.starts_with("HEAD ") {
                wt.head = line.strip_prefix("HEAD ").unwrap_or("").to_string();
            } else if line.starts_with("branch ") {
                let branch_ref = line.strip_prefix("branch ").unwrap_or("");
                let branch_name = branch_ref
                    .strip_prefix("refs/heads/")
                    .unwrap_or(branch_ref)
                    .to_string();
                wt.branch = Some(branch_name);
            } else if line == "bare" {
                wt.is_main = true;
            } else if line == "locked" {
                wt.is_locked = true;
            } else if line == "prunable" {
                wt.is_prunable = true;
            }
        }
    }

    if let Some(wt) = current_worktree {
        worktrees.push(wt);
    }

    if let Some(first) = worktrees.first_mut() {
        if !first.is_main {
            first.is_main = true;
        }
    }

    worktrees
}
