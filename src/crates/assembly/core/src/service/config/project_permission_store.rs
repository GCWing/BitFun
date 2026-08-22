//! Workspace-scoped static tool permission rules.

use crate::infrastructure::get_path_manager_arc;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_runtime_ports::{PermissionRule, WorkspaceFileSystem};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub const PROJECT_PERMISSION_FILE_NAME: &str = "tool_permissions.json";

/// User-configured sensitive-resource markers, split by protection class.
///
/// - `read` markers protect content that must never be read without the user
///   deciding: any operation touching such a resource is escalated (or denied
///   in unattended modes). They are matched case-insensitively as substrings.
/// - `write` markers protect content that must not be written automatically:
///   write-class operations (edit/write/bash) touching them lose aggressive
///   auto-approval. Their names are still visible to the judge.
///
/// Legacy configs stored a plain array under `sensitive_resources`; they are
/// deserialized as `read` markers to preserve the historical behavior (the
/// old list only ever guarded the read path).
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct SensitiveResourceConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub write: Vec<String>,
}

impl SensitiveResourceConfig {
    pub fn is_empty(&self) -> bool {
        self.read.is_empty() && self.write.is_empty()
    }
}

impl<'de> Deserialize<'de> for SensitiveResourceConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Object {
                #[serde(default)]
                read: Vec<String>,
                #[serde(default)]
                write: Vec<String>,
            },
            LegacyArray(Vec<String>),
        }
        match Repr::deserialize(deserializer)? {
            Repr::Object { read, write } => Ok(Self { read, write }),
            Repr::LegacyArray(read) => Ok(Self {
                read,
                write: Vec::new(),
            }),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ProjectPermissionConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<PermissionRule>,
    /// User-configured sensitive-resource markers (case-insensitive substring
    /// match), split into read- and write-sensitive classes. Never exposed to
    /// the AI judge prompt as marker text itself; resource names of write-class
    /// markers may appear when a call is judged, secret values are still
    /// redacted.
    #[serde(default, skip_serializing_if = "SensitiveResourceConfig::is_empty")]
    pub sensitive_resources: SensitiveResourceConfig,
}

pub fn project_permission_file_path(workspace_root: &Path) -> PathBuf {
    get_path_manager_arc().project_permission_file(workspace_root)
}

pub fn project_permission_file_path_for_remote(remote_root: &str) -> String {
    format!(
        "{}/.bitfun/config/{}",
        remote_root.trim_end_matches('/'),
        PROJECT_PERMISSION_FILE_NAME
    )
}

pub fn deserialize_project_permission_config(
    content: &str,
) -> BitFunResult<ProjectPermissionConfig> {
    let value: Value = serde_json::from_str(content).map_err(|error| {
        BitFunError::config(format!(
            "Failed to parse project permission config: {error}"
        ))
    })?;

    if value.is_array() {
        let rules = serde_json::from_value(value).map_err(|error| {
            BitFunError::config(format!("Invalid project permission rules: {error}"))
        })?;
        Ok(ProjectPermissionConfig {
            rules,
            sensitive_resources: SensitiveResourceConfig::default(),
        })
    } else {
        serde_json::from_value(value).map_err(|error| {
            BitFunError::config(format!("Invalid project permission config: {error}"))
        })
    }
}

pub async fn load_project_permission_config_local(
    workspace_root: &Path,
) -> BitFunResult<ProjectPermissionConfig> {
    let path = project_permission_file_path(workspace_root);
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => deserialize_project_permission_config(&content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(ProjectPermissionConfig::default())
        }
        Err(error) => Err(BitFunError::config(format!(
            "Failed to read project permission config '{}': {error}",
            path.display()
        ))),
    }
}

pub async fn load_project_permission_config_remote(
    fs: &dyn WorkspaceFileSystem,
    remote_root: &str,
) -> BitFunResult<ProjectPermissionConfig> {
    let path = project_permission_file_path_for_remote(remote_root);
    if !fs.exists(&path).await.unwrap_or(false) {
        return Ok(ProjectPermissionConfig::default());
    }

    let content = fs.read_file_text(&path).await.map_err(|error| {
        BitFunError::config(format!(
            "Failed to read remote project permission config '{}': {error}",
            path
        ))
    })?;
    deserialize_project_permission_config(&content)
}

#[cfg(test)]
mod tests {
    use super::{deserialize_project_permission_config, project_permission_file_path_for_remote};
    use bitfun_runtime_ports::{PermissionEffect, PermissionRule};

    #[test]
    fn parses_object_permission_config() {
        let config = deserialize_project_permission_config(
            r#"{"rules":[{"action":"edit","resource":"src/*","effect":"deny"}]}"#,
        )
        .expect("object config should parse");

        assert_eq!(
            config.rules,
            vec![PermissionRule::new("edit", "src/*", PermissionEffect::Deny)]
        );
    }

    #[test]
    fn parses_legacy_array_permission_config() {
        let config = deserialize_project_permission_config(
            r#"[{"action":"read","resource":"secrets/*","effect":"deny"}]"#,
        )
        .expect("array config should parse");

        assert_eq!(config.rules.len(), 1);
        assert_eq!(config.rules[0].action, "read");
        assert!(config.sensitive_resources.is_empty());
    }

    #[test]
    fn parses_object_config_with_sensitive_resources() {
        let config = deserialize_project_permission_config(
            r#"{"rules":[{"action":"read","resource":"*","effect":"ask"}],"sensitive_resources":["secrets/",".cursorrules"]}"#,
        )
        .expect("object config with sensitive resources should parse");

        assert_eq!(config.rules.len(), 1);
        assert_eq!(
            config.sensitive_resources.read,
            vec!["secrets/".to_string(), ".cursorrules".to_string()]
        );
        assert!(config.sensitive_resources.write.is_empty());
    }

    #[test]
    fn parses_object_config_with_read_write_sensitive_resources() {
        let config = deserialize_project_permission_config(
            r#"{"rules":[],"sensitive_resources":{"read":["secrets/"],"write":["crash-reports/"]}}"#,
        )
        .expect("object config with read/write sensitive resources should parse");

        assert_eq!(
            config.sensitive_resources.read,
            vec!["secrets/".to_string()]
        );
        assert_eq!(
            config.sensitive_resources.write,
            vec!["crash-reports/".to_string()]
        );
    }

    #[test]
    fn legacy_sensitive_resources_array_maps_to_read_class() {
        let config =
            deserialize_project_permission_config(r#"{"sensitive_resources":["secrets/",".env"]}"#)
                .expect("legacy array should parse");

        assert_eq!(
            config.sensitive_resources.read,
            vec!["secrets/".to_string(), ".env".to_string()]
        );
        assert!(config.sensitive_resources.write.is_empty());
    }

    #[test]
    fn sensitive_resources_object_without_write_defaults_to_empty() {
        let config =
            deserialize_project_permission_config(r#"{"sensitive_resources":{"read":["a/"]}}"#)
                .expect("object config without write should parse");

        assert_eq!(config.sensitive_resources.read, vec!["a/".to_string()]);
        assert!(config.sensitive_resources.write.is_empty());
    }

    #[test]
    fn object_config_without_sensitive_resources_defaults_to_empty() {
        let config = deserialize_project_permission_config(r#"{"rules":[]}"#)
            .expect("object config without sensitive resources should parse");
        assert!(config.sensitive_resources.is_empty());
    }

    #[test]
    fn remote_permission_path_is_workspace_scoped() {
        assert_eq!(
            project_permission_file_path_for_remote("/home/user/project/"),
            "/home/user/project/.bitfun/config/tool_permissions.json"
        );
    }
}
