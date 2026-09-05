//! Workspace-scoped static tool permission rules.

use crate::infrastructure::get_path_manager_arc;
use crate::util::errors::{OpenBitFunError, OpenBitFunResult};
use openbitfun_core_types::product_identity::hidden_data_directory;
use openbitfun_runtime_ports::{PermissionRule, WorkspaceFileSystem};
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
        "{}/{}/config/{}",
        remote_root.trim_end_matches('/'),
        hidden_data_directory(),
        PROJECT_PERMISSION_FILE_NAME
    )
}

pub fn deserialize_project_permission_config(
    content: &str,
) -> OpenBitFunResult<ProjectPermissionConfig> {
    let value: Value = serde_json::from_str(content).map_err(|error| {
        OpenBitFunError::config(format!(
            "Failed to parse project permission config: {error}"
        ))
    })?;

    if value.is_array() {
        Err(OpenBitFunError::config(
            "Project permission config uses the pre-OpenBitFun array format; use the explicit data migration tool instead",
        ))
    } else {
        serde_json::from_value(value).map_err(|error| {
            OpenBitFunError::config(format!("Invalid project permission config: {error}"))
        })
    }
}

pub async fn load_project_permission_config_local(
    workspace_root: &Path,
) -> OpenBitFunResult<ProjectPermissionConfig> {
    let path = project_permission_file_path(workspace_root);
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => deserialize_project_permission_config(&content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(ProjectPermissionConfig::default())
        }
        Err(error) => Err(OpenBitFunError::config(format!(
            "Failed to read project permission config '{}': {error}",
            path.display()
        ))),
    }
}

pub async fn load_project_permission_config_remote(
    fs: &dyn WorkspaceFileSystem,
    remote_root: &str,
) -> OpenBitFunResult<ProjectPermissionConfig> {
    let path = project_permission_file_path_for_remote(remote_root);
    if !fs.exists(&path).await.unwrap_or(false) {
        return Ok(ProjectPermissionConfig::default());
    }

    let content = fs.read_file_text(&path).await.map_err(|error| {
        OpenBitFunError::config(format!(
            "Failed to read remote project permission config '{}': {error}",
            path
        ))
    })?;
    deserialize_project_permission_config(&content)
}

#[cfg(test)]
mod tests {
    use super::{deserialize_project_permission_config, project_permission_file_path_for_remote};
    use openbitfun_runtime_ports::{PermissionEffect, PermissionRule};

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
    fn rejects_pre_openbitfun_array_permission_config() {
        let error = deserialize_project_permission_config(
            r#"[{"action":"read","resource":"secrets/*","effect":"deny"}]"#,
        )
        .expect_err("pre-OpenBitFun array config must require explicit migration");

        assert!(error.to_string().contains("pre-OpenBitFun"), "{error}");
        assert!(
            error.to_string().contains("explicit data migration tool"),
            "{error}"
        );
    }

    #[test]
    fn remote_permission_path_is_workspace_scoped() {
        assert_eq!(
            project_permission_file_path_for_remote("/home/user/project/"),
            "/home/user/project/.openbitfun/config/tool_permissions.json"
        );
    }
}
