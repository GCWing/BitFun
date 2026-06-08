//! Provider-neutral checkpoint summary planning.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightCheckpoint {
    pub current_branch: Option<String>,
    pub dirty_state_summary: String,
    pub touched_files: Vec<String>,
    pub diff_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LightCheckpointWorkspaceFacts {
    WorkspaceUnavailable,
    RemoteWorkspace,
    LocalWorkspace {
        git_status: Result<GitStatusCheckpointFacts, String>,
        diff_hash: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStatusCheckpointFacts {
    pub current_branch: String,
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub untracked_count: usize,
}

pub fn build_light_checkpoint(
    touched_files: Vec<String>,
    workspace: LightCheckpointWorkspaceFacts,
) -> LightCheckpoint {
    match workspace {
        LightCheckpointWorkspaceFacts::WorkspaceUnavailable => LightCheckpoint {
            current_branch: None,
            dirty_state_summary: "workspace_unavailable".to_string(),
            touched_files,
            diff_hash: None,
        },
        LightCheckpointWorkspaceFacts::RemoteWorkspace => LightCheckpoint {
            current_branch: None,
            dirty_state_summary: "remote_workspace_git_metadata_unavailable".to_string(),
            touched_files,
            diff_hash: None,
        },
        LightCheckpointWorkspaceFacts::LocalWorkspace {
            git_status,
            diff_hash,
        } => {
            let (current_branch, dirty_state_summary) = match git_status {
                Ok(status) => (
                    Some(status.current_branch),
                    format!(
                        "staged={}, unstaged={}, untracked={}",
                        status.staged_count, status.unstaged_count, status.untracked_count
                    ),
                ),
                Err(error) => (None, format!("git_status_unavailable: {}", error)),
            };
            LightCheckpoint {
                current_branch,
                dirty_state_summary,
                touched_files,
                diff_hash,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_local_checkpoint_summary_from_git_status_facts() {
        let checkpoint = build_light_checkpoint(
            vec!["src/lib.rs".to_string()],
            LightCheckpointWorkspaceFacts::LocalWorkspace {
                git_status: Ok(GitStatusCheckpointFacts {
                    current_branch: "main".to_string(),
                    staged_count: 1,
                    unstaged_count: 2,
                    untracked_count: 3,
                }),
                diff_hash: Some("abc".to_string()),
            },
        );

        assert_eq!(checkpoint.current_branch, Some("main".to_string()));
        assert_eq!(
            checkpoint.dirty_state_summary,
            "staged=1, unstaged=2, untracked=3"
        );
        assert_eq!(checkpoint.diff_hash, Some("abc".to_string()));
        assert_eq!(checkpoint.touched_files, vec!["src/lib.rs".to_string()]);
    }

    #[test]
    fn preserves_remote_checkpoint_unavailable_summary() {
        let checkpoint = build_light_checkpoint(
            vec!["/repo/src/lib.rs".to_string()],
            LightCheckpointWorkspaceFacts::RemoteWorkspace,
        );

        assert_eq!(checkpoint.current_branch, None);
        assert_eq!(
            checkpoint.dirty_state_summary,
            "remote_workspace_git_metadata_unavailable"
        );
        assert_eq!(checkpoint.diff_hash, None);
    }
}
