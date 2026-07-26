//! Workspace-scoped static tool permission rules.

use crate::infrastructure::get_path_manager_arc;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_runtime_ports::{PermissionRule, WorkspaceFileSystem};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub const PROJECT_PERMISSION_FILE_NAME: &str = "tool_permissions.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ProjectPermissionConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<PermissionRule>,
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
        Ok(ProjectPermissionConfig { rules })
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
    }

    #[test]
    fn remote_permission_path_is_workspace_scoped() {
        assert_eq!(
            project_permission_file_path_for_remote("/home/user/project/"),
            "/home/user/project/.bitfun/config/tool_permissions.json"
        );
    }
}
