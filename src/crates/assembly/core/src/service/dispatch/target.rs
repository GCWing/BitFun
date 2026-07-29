use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DispatchWorkspaceDeliveryRequest {
    Existing,
    SnapshotExact {
        #[serde(rename = "sourceWorkspacePath")]
        source_workspace_path: String,
        #[serde(rename = "sensitiveFilesConfirmed")]
        sensitive_files_confirmed: bool,
    },
}

impl Default for DispatchWorkspaceDeliveryRequest {
    fn default() -> Self {
        Self::Existing
    }
}

/// The execution location selected while a chat session is being created.
///
/// Dispatch is deliberately orthogonal to `SessionExecutionTarget`: the latter
/// describes a path owned by this process, while non-local dispatch targets are
/// owned by another BitFun process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum DispatchTargetRequest {
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

impl Default for DispatchTargetRequest {
    fn default() -> Self {
        Self::Local
    }
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
    fn exact_snapshot_requires_an_explicit_source_and_confirmation_fact() {
        let value = serde_json::to_value(DispatchWorkspaceDeliveryRequest::SnapshotExact {
            source_workspace_path: "/work/app".to_string(),
            sensitive_files_confirmed: true,
        })
        .expect("serialize delivery");
        assert_eq!(
            value,
            serde_json::json!({
                "kind": "snapshot-exact",
                "sourceWorkspacePath": "/work/app",
                "sensitiveFilesConfirmed": true
            })
        );
    }
}
