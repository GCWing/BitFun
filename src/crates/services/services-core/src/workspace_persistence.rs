//! Current Workspace registry persistence contract and validation.

use crate::storage_error::{StorageError as OpenBitFunError, StorageResult as OpenBitFunResult};
use crate::workspace_identity::{
    canonicalize_local_workspace_root, local_workspace_stable_storage_id,
    normalize_remote_workspace_path, remote_workspace_stable_id, LOCAL_WORKSPACE_SSH_HOST,
};
use crate::workspace_records::{PrimaryAssistantKey, WorkspaceInfo, WorkspaceKind};
use openbitfun_core_types::product_identity::product_id;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub const WORKSPACE_PERSISTENCE_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePersistenceData {
    #[serde(default)]
    pub format_version: u32,
    #[serde(default)]
    pub product_id: String,
    pub workspaces: HashMap<String, WorkspaceInfo>,
    #[serde(default)]
    pub opened_workspace_ids: Vec<String>,
    pub current_workspace_id: Option<String>,
    #[serde(default)]
    pub recent_workspaces: Vec<String>,
    #[serde(default)]
    pub recent_assistant_workspaces: Vec<String>,
    #[serde(default)]
    pub primary_assistant_key: Option<PrimaryAssistantKey>,
    pub saved_at: chrono::DateTime<chrono::Utc>,
}

pub fn current_workspace_storage_id(workspace: &WorkspaceInfo) -> OpenBitFunResult<String> {
    match workspace.workspace_kind {
        WorkspaceKind::Remote => {
            let ssh_host = workspace
                .metadata
                .get("sshHost")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    unsupported_workspace_persistence(format!(
                        "remote workspace '{}' is missing sshHost",
                        workspace.id
                    ))
                })?;
            workspace.remote_ssh_connection_id().ok_or_else(|| {
                unsupported_workspace_persistence(format!(
                    "remote workspace '{}' is missing connectionId",
                    workspace.id
                ))
            })?;

            let stored_root = workspace.root_path.to_string_lossy().replace('\\', "/");
            let normalized_root = normalize_remote_workspace_path(&stored_root);
            if !normalized_root.starts_with('/') {
                return Err(unsupported_workspace_persistence(format!(
                    "remote workspace '{}' does not use an absolute POSIX root",
                    workspace.id
                )));
            }
            if stored_root != normalized_root {
                return Err(unsupported_workspace_persistence(format!(
                    "remote workspace '{}' rootPath is not normalized",
                    workspace.id
                )));
            }
            Ok(remote_workspace_stable_id(ssh_host, &normalized_root))
        }
        WorkspaceKind::Normal | WorkspaceKind::Assistant => {
            let ssh_host = workspace
                .metadata
                .get("sshHost")
                .and_then(|value| value.as_str())
                .map(str::trim);
            if ssh_host != Some(LOCAL_WORKSPACE_SSH_HOST) {
                return Err(unsupported_workspace_persistence(format!(
                    "local workspace '{}' does not declare sshHost=localhost",
                    workspace.id
                )));
            }
            expected_persisted_local_workspace_id(&workspace.root_path).map_err(|error| {
                unsupported_workspace_persistence(format!(
                    "local workspace '{}' is not canonical: {error}",
                    workspace.id
                ))
            })
        }
    }
}

pub fn validate_workspace_persistence_data(
    data: &WorkspacePersistenceData,
    miniapps_root: &Path,
) -> OpenBitFunResult<()> {
    if data.format_version != WORKSPACE_PERSISTENCE_FORMAT_VERSION {
        return Err(unsupported_workspace_persistence(format!(
            "format_version {} is not supported; expected {}",
            data.format_version, WORKSPACE_PERSISTENCE_FORMAT_VERSION
        )));
    }
    if data.product_id != product_id() {
        return Err(unsupported_workspace_persistence(format!(
            "product_id '{}' does not match '{}'",
            data.product_id,
            product_id()
        )));
    }

    for (storage_id, workspace) in &data.workspaces {
        if storage_id != &workspace.id {
            return Err(unsupported_workspace_persistence(format!(
                "workspace map key '{storage_id}' does not match record id '{}'",
                workspace.id
            )));
        }
        let expected_id = current_workspace_storage_id(workspace)?;
        if storage_id != &expected_id {
            return Err(unsupported_workspace_persistence(format!(
                "workspace id '{storage_id}' is not canonical; expected '{expected_id}'"
            )));
        }
    }
    validate_workspace_reference_list(
        &data.workspaces,
        &data.opened_workspace_ids,
        "opened_workspace_ids",
    )?;
    validate_workspace_reference_list(
        &data.workspaces,
        &data.recent_workspaces,
        "recent_workspaces",
    )?;
    validate_workspace_reference_list(
        &data.workspaces,
        &data.recent_assistant_workspaces,
        "recent_assistant_workspaces",
    )?;

    for id in &data.recent_workspaces {
        let workspace = &data.workspaces[id];
        if workspace.workspace_kind == WorkspaceKind::Assistant {
            return Err(unsupported_workspace_persistence(format!(
                "recent_workspaces contains assistant workspace '{id}'"
            )));
        }
        if workspace.root_path.starts_with(miniapps_root) {
            return Err(unsupported_workspace_persistence(format!(
                "recent_workspaces contains MiniApp-owned workspace '{id}'"
            )));
        }
    }
    for id in &data.recent_assistant_workspaces {
        if data.workspaces[id].workspace_kind != WorkspaceKind::Assistant {
            return Err(unsupported_workspace_persistence(format!(
                "recent_assistant_workspaces contains non-assistant workspace '{id}'"
            )));
        }
    }

    if let Some(current_id) = data.current_workspace_id.as_deref() {
        if !data.workspaces.contains_key(current_id) {
            return Err(unsupported_workspace_persistence(format!(
                "current_workspace_id references unknown workspace id '{current_id}'"
            )));
        }
        if !data.opened_workspace_ids.iter().any(|id| id == current_id) {
            return Err(unsupported_workspace_persistence(format!(
                "current workspace '{current_id}' is not present in opened_workspace_ids"
            )));
        }
    }

    Ok(())
}

fn expected_persisted_local_workspace_id(root_path: &Path) -> Result<String, String> {
    if !root_path.is_absolute() {
        return Err(format!(
            "local workspace rootPath is not absolute: {}",
            root_path.display()
        ));
    }

    let normalized_root = if root_path.exists() {
        let (canonical_root, normalized_root) = canonicalize_local_workspace_root(root_path)?;
        if canonical_root != root_path {
            return Err(format!(
                "local workspace rootPath is not canonical: {}",
                root_path.display()
            ));
        }
        normalized_root
    } else {
        root_path.to_string_lossy().replace('\\', "/")
    };

    Ok(local_workspace_stable_storage_id(&normalized_root))
}

fn validate_workspace_reference_list(
    workspaces: &HashMap<String, WorkspaceInfo>,
    ids: &[String],
    field: &str,
) -> OpenBitFunResult<()> {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id.as_str()) {
            return Err(unsupported_workspace_persistence(format!(
                "{field} contains duplicate workspace id '{id}'"
            )));
        }
        if !workspaces.contains_key(id) {
            return Err(unsupported_workspace_persistence(format!(
                "{field} references unknown workspace id '{id}'"
            )));
        }
    }
    Ok(())
}

pub fn unsupported_workspace_persistence(detail: impl AsRef<str>) -> OpenBitFunError {
    OpenBitFunError::config(format!(
        "Unsupported workspace persistence format: {}. The persisted file was left unchanged; explicit data migration is required",
        detail.as_ref()
    ))
}
