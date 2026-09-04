use serde::{Deserialize, Serialize};

/// How a dispatch reaches the target: a Git worktree of the controller's own
/// repository, checked out on the target at the same commit.
///
/// This replaced three file-snapshot delivery modes. A snapshot had no common
/// ancestor with the controller, so results could only be applied by overwriting
/// paths. A shared commit makes the result an ordinary branch the user can
/// fetch, review, merge, or discard.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchWorkspaceDelivery {
    /// Controller-side source checkout whose HEAD/local changes seeded the
    /// baseline. This may itself be a linked worktree and therefore is not a
    /// stable registry lookup path after that checkout is removed.
    pub source_workspace_path: String,
    /// Canonical main-project workspace that owns the managed-worktree
    /// registry. Claim release must use this path rather than the possibly
    /// short-lived source checkout above.
    #[serde(default)]
    pub project_workspace_path: String,
    /// Managed worktree created on the controller as this dispatch's baseline.
    pub baseline_worktree_id: String,
    /// Immutable commit both sides check out. Never a ref name: a ref can move
    /// between the controller resolving it and the target fetching it.
    pub base_commit: String,
    /// Branch the target commits onto, and the branch the controller fetches
    /// back during sync.
    pub branch: String,
    /// Git remote the target clones from. Absent when the repository has no
    /// remote, in which case every object is carried by bundle instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,
    /// Fold the baseline worktree's uncommitted changes into `base_commit`.
    ///
    /// Only reaches what `git add -A` would stage — unlike the old exact
    /// snapshot, ignored files never cross the wire.
    #[serde(default)]
    pub include_uncommitted: bool,
}

/// The execution location selected while a chat session is being created.
///
/// Dispatch is deliberately orthogonal to `SessionExecutionTarget`: the latter
/// describes a path owned by this process, while non-local dispatch targets are
/// owned by another OpenBitFun process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[derive(Default)]
pub enum DispatchTargetRequest {
    #[default]
    Local,
    Ssh {
        #[serde(rename = "connectionId")]
        connection_id: String,
        #[serde(rename = "workspacePath")]
        workspace_path: String,
    },
    Device {
        #[serde(rename = "deviceId")]
        device_id: String,
        #[serde(rename = "workspacePath")]
        workspace_path: String,
    },
}

impl DispatchTargetRequest {
    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }

    pub fn workspace_path(&self) -> Option<&str> {
        match self {
            Self::Local => None,
            Self::Ssh { workspace_path, .. } | Self::Device { workspace_path, .. } => {
                Some(workspace_path)
            }
        }
    }
}

/// Resolved dispatch target persisted with an outbound observer record.
///
/// `display_name` is presentation-only. Stable routing always uses the
/// connection/device id and never the label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum DispatchTarget {
    Local,
    Ssh {
        #[serde(rename = "connectionId")]
        connection_id: String,
        #[serde(rename = "workspacePath")]
        workspace_path: String,
        #[serde(rename = "displayName")]
        display_name: String,
    },
    Device {
        #[serde(rename = "deviceId")]
        device_id: String,
        #[serde(rename = "workspacePath")]
        workspace_path: String,
        #[serde(rename = "displayName")]
        display_name: String,
    },
}

impl DispatchTarget {
    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }

    pub fn workspace_path(&self) -> Option<&str> {
        match self {
            Self::Local => None,
            Self::Ssh { workspace_path, .. } | Self::Device { workspace_path, .. } => {
                Some(workspace_path)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_target_shape_is_tagged_and_camel_case() {
        let value = serde_json::to_value(DispatchTargetRequest::Ssh {
            connection_id: "server-a".to_string(),
            workspace_path: "/srv/app".to_string(),
        })
        .expect("serialize target");

        assert_eq!(
            value,
            serde_json::json!({
                "kind": "ssh",
                "connectionId": "server-a",
                "workspacePath": "/srv/app"
            })
        );
    }

    #[test]
    fn workspace_delivery_pins_an_immutable_commit_in_camel_case() {
        let value = serde_json::to_value(DispatchWorkspaceDelivery {
            source_workspace_path: "/work/app".to_string(),
            project_workspace_path: "/work/app-main".to_string(),
            baseline_worktree_id: "wt-1".to_string(),
            base_commit: "0123456789abcdef0123456789abcdef01234567".to_string(),
            branch: "openbitfun/dispatch/1a2b3c4d".to_string(),
            remote_url: Some("git@example.com:acme/app.git".to_string()),
            include_uncommitted: true,
        })
        .expect("serialize delivery");

        assert_eq!(
            value,
            serde_json::json!({
                "sourceWorkspacePath": "/work/app",
                "projectWorkspacePath": "/work/app-main",
                "baselineWorktreeId": "wt-1",
                "baseCommit": "0123456789abcdef0123456789abcdef01234567",
                "branch": "openbitfun/dispatch/1a2b3c4d",
                "remoteUrl": "git@example.com:acme/app.git",
                "includeUncommitted": true
            })
        );
    }

    #[test]
    fn workspace_delivery_omits_the_remote_for_a_repository_without_one() {
        let value = serde_json::to_value(DispatchWorkspaceDelivery {
            source_workspace_path: "/work/app".to_string(),
            project_workspace_path: "/work/app-main".to_string(),
            baseline_worktree_id: "wt-1".to_string(),
            base_commit: "0123456789abcdef0123456789abcdef01234567".to_string(),
            branch: "openbitfun/dispatch/1a2b3c4d".to_string(),
            remote_url: None,
            include_uncommitted: false,
        })
        .expect("serialize delivery");

        assert!(value.get("remoteUrl").is_none());
    }
}
