//! Shared desktop resolution and access helpers for local, runtime, and remote paths.

use crate::api::app_state::AppState;
use bitfun_core::agentic::tools::workspace_paths::{
    is_bitfun_runtime_uri, parse_bitfun_runtime_uri,
};
use bitfun_core::infrastructure::get_path_manager_arc;
use bitfun_core::infrastructure::FileOperationOptions;
use bitfun_core::service::remote_ssh::workspace_state::remote_workspace_runtime_root;
use bitfun_core::service::remote_ssh::{
    get_remote_workspace_manager, normalize_remote_workspace_path, RemoteWorkspaceEntry,
};
use bitfun_core::service::workspace::{WorkspaceInfo, WorkspaceKind};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub enum DesktopPathTarget {
    Local {
        requested_path: String,
        resolved_path: PathBuf,
        is_runtime_artifact: bool,
    },
    Remote {
        requested_path: String,
        entry: RemoteWorkspaceEntry,
    },
}

impl DesktopPathTarget {
    pub fn requested_path(&self) -> &str {
        match self {
            Self::Local { requested_path, .. } | Self::Remote { requested_path, .. } => {
                requested_path.as_str()
            }
        }
    }

    pub fn as_local_path(&self) -> Option<&Path> {
        match self {
            Self::Local { resolved_path, .. } => Some(resolved_path.as_path()),
            Self::Remote { .. } => None,
        }
    }

    pub fn is_runtime_artifact(&self) -> bool {
        matches!(
            self,
            Self::Local {
                is_runtime_artifact: true,
                ..
            }
        )
    }
}

fn runtime_root_for_workspace_info(workspace: &WorkspaceInfo) -> Result<PathBuf, String> {
    if workspace.workspace_kind == WorkspaceKind::Remote {
        let ssh_host = workspace
            .metadata
            .get("sshHost")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                format!(
                    "Remote workspace '{}' is missing sshHost metadata",
                    workspace.id
                )
            })?;

        let remote_root = normalize_remote_workspace_path(&workspace.root_path.to_string_lossy());
        return Ok(remote_workspace_runtime_root(ssh_host, &remote_root));
    }

    Ok(get_path_manager_arc().project_runtime_root(&workspace.root_path))
}

async fn resolve_runtime_artifact_path(
    app_state: &AppState,
    raw_path: &str,
) -> Result<Option<PathBuf>, String> {
    if !is_bitfun_runtime_uri(raw_path) {
        return Ok(None);
    }

    let parsed = parse_bitfun_runtime_uri(raw_path).map_err(|e| e.to_string())?;
    let workspace = if parsed.workspace_scope == "current" {
        app_state.workspace_service.get_current_workspace().await
    } else {
        app_state
            .workspace_service
            .list_workspace_infos()
            .await
            .into_iter()
            .find(|workspace| workspace.id == parsed.workspace_scope)
    }
    .ok_or_else(|| {
        format!(
            "Unable to resolve runtime URI scope '{}'",
            parsed.workspace_scope
        )
    })?;

    let mut resolved = runtime_root_for_workspace_info(&workspace)?;
    for segment in parsed.relative_path.split('/') {
        resolved.push(segment);
    }

    Ok(Some(resolved))
}

async fn lookup_remote_entry_for_path(
    app_state: &AppState,
    path: &str,
    request_preferred: Option<&str>,
) -> Option<RemoteWorkspaceEntry> {
    let manager = get_remote_workspace_manager()?;
    let legacy = app_state
        .get_remote_workspace_async()
        .await
        .map(|workspace| workspace.connection_id);
    let preferred = request_preferred.map(|s| s.to_string()).or(legacy);
    manager.lookup_connection(path, preferred.as_deref()).await
}

pub async fn resolve_desktop_path_target(
    app_state: &AppState,
    raw_path: &str,
    preferred_remote_connection_id: Option<&str>,
) -> Result<DesktopPathTarget, String> {
    if let Some(resolved_path) = resolve_runtime_artifact_path(app_state, raw_path).await? {
        return Ok(DesktopPathTarget::Local {
            requested_path: raw_path.to_string(),
            resolved_path,
            is_runtime_artifact: true,
        });
    }

    if let Some(entry) =
        lookup_remote_entry_for_path(app_state, raw_path, preferred_remote_connection_id).await
    {
        return Ok(DesktopPathTarget::Remote {
            requested_path: raw_path.to_string(),
            entry,
        });
    }

    Ok(DesktopPathTarget::Local {
        requested_path: raw_path.to_string(),
        resolved_path: PathBuf::from(raw_path),
        is_runtime_artifact: false,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFileMetadata {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_path: Option<String>,
    pub modified: u64,
    pub size: u64,
    pub is_file: bool,
    pub is_dir: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_remote: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_runtime_artifact: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFileMetadata {
    pub path: String,
    pub modified: u64,
    pub size: u64,
    pub is_file: bool,
    pub is_dir: bool,
    pub is_remote: bool,
}

pub fn stat_local_path_metadata(
    requested_path: &str,
    resolved_path: &Path,
    is_runtime_artifact: bool,
) -> Result<LocalFileMetadata, String> {
    let metadata = std::fs::metadata(resolved_path).map_err(|e| {
        format!(
            "Failed to stat local file '{}': {}",
            resolved_path.display(),
            e
        )
    })?;

    let modified = metadata
        .modified()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    Ok(LocalFileMetadata {
        path: requested_path.to_string(),
        resolved_path: is_runtime_artifact.then(|| resolved_path.to_string_lossy().to_string()),
        modified,
        size: metadata.len(),
        is_file: metadata.is_file(),
        is_dir: metadata.is_dir(),
        is_remote: Some(false),
        is_runtime_artifact: is_runtime_artifact.then_some(true),
    })
}

pub async fn read_text_file(
    app_state: &AppState,
    raw_path: &str,
    preferred_remote_connection_id: Option<&str>,
) -> Result<String, String> {
    match resolve_desktop_path_target(app_state, raw_path, preferred_remote_connection_id).await? {
        DesktopPathTarget::Local { resolved_path, .. } => app_state
            .filesystem_service
            .read_file(&resolved_path.to_string_lossy())
            .await
            .map(|result| result.content)
            .map_err(|e| format!("Failed to read file content: {}", e)),
        DesktopPathTarget::Remote {
            requested_path,
            entry,
        } => {
            let remote_fs = app_state
                .get_remote_file_service_async()
                .await
                .map_err(|e| format!("Remote file service not available: {}", e))?;
            let bytes = remote_fs
                .read_file(&entry.connection_id, &requested_path)
                .await
                .map_err(|e| format!("Failed to read remote file: {}", e))?;
            String::from_utf8(bytes).map_err(|e| format!("File is not valid UTF-8: {}", e))
        }
    }
}

pub async fn write_text_file(
    app_state: &AppState,
    raw_path: &str,
    content: &str,
    preferred_remote_connection_id: Option<&str>,
) -> Result<(), String> {
    match resolve_desktop_path_target(app_state, raw_path, preferred_remote_connection_id).await? {
        DesktopPathTarget::Local { resolved_path, .. } => {
            let options = FileOperationOptions {
                backup_on_overwrite: false,
                ..FileOperationOptions::default()
            };
            app_state
                .filesystem_service
                .write_file_with_options(&resolved_path.to_string_lossy(), content, options)
                .await
                .map(|_| ())
                .map_err(|e| format!("Failed to write file {}: {}", raw_path, e))
        }
        DesktopPathTarget::Remote {
            requested_path,
            entry,
        } => {
            let remote_fs = app_state
                .get_remote_file_service_async()
                .await
                .map_err(|e| format!("Remote file service not available: {}", e))?;
            remote_fs
                .write_file(&entry.connection_id, &requested_path, content.as_bytes())
                .await
                .map_err(|e| format!("Failed to write remote file: {}", e))
        }
    }
}

pub async fn path_exists(app_state: &AppState, raw_path: &str) -> Result<bool, String> {
    match resolve_desktop_path_target(app_state, raw_path, None).await? {
        DesktopPathTarget::Local { resolved_path, .. } => Ok(resolved_path.exists()),
        DesktopPathTarget::Remote {
            requested_path,
            entry,
        } => {
            let remote_fs = app_state
                .get_remote_file_service_async()
                .await
                .map_err(|e| format!("Remote file service not available: {}", e))?;
            remote_fs
                .exists(&entry.connection_id, &requested_path)
                .await
                .map_err(|e| format!("Failed to check remote path: {}", e))
        }
    }
}

pub async fn get_path_metadata(
    app_state: &AppState,
    raw_path: &str,
) -> Result<serde_json::Value, String> {
    match resolve_desktop_path_target(app_state, raw_path, None).await? {
        DesktopPathTarget::Local {
            requested_path,
            resolved_path,
            is_runtime_artifact,
        } => {
            let metadata =
                stat_local_path_metadata(&requested_path, &resolved_path, is_runtime_artifact)?;
            serde_json::to_value(metadata)
                .map_err(|e| format!("Failed to serialize file metadata: {}", e))
        }
        DesktopPathTarget::Remote {
            requested_path,
            entry,
        } => {
            let remote_fs = app_state
                .get_remote_file_service_async()
                .await
                .map_err(|e| format!("Remote file service not available: {}", e))?;

            let stat_entry = remote_fs
                .stat(&entry.connection_id, &requested_path)
                .await
                .map_err(|e| format!("Failed to stat remote file: {}", e))?;

            let (is_file, is_dir, size, modified) = match stat_entry {
                Some(entry) => (
                    entry.is_file,
                    entry.is_dir,
                    entry.size.unwrap_or(0),
                    entry.modified.unwrap_or(0),
                ),
                None => (false, false, 0, 0),
            };

            serde_json::to_value(RemoteFileMetadata {
                path: requested_path,
                modified,
                size,
                is_file,
                is_dir,
                is_remote: true,
            })
            .map_err(|e| format!("Failed to serialize remote file metadata: {}", e))
        }
    }
}

pub async fn rename_path(
    app_state: &AppState,
    old_path: &str,
    new_path: &str,
) -> Result<(), String> {
    match resolve_desktop_path_target(app_state, old_path, None).await? {
        DesktopPathTarget::Local {
            resolved_path: old_resolved_path,
            ..
        } => {
            let new_resolved_path =
                match resolve_desktop_path_target(app_state, new_path, None).await? {
                    DesktopPathTarget::Local { resolved_path, .. } => resolved_path,
                    DesktopPathTarget::Remote { .. } => {
                        return Err(format!(
                            "Cannot rename local path '{}' to remote destination '{}'",
                            old_path, new_path
                        ))
                    }
                };

            app_state
                .filesystem_service
                .move_file(
                    &old_resolved_path.to_string_lossy(),
                    &new_resolved_path.to_string_lossy(),
                )
                .await
                .map_err(|e| format!("Failed to rename file: {}", e))
        }
        DesktopPathTarget::Remote { entry, .. } => {
            let remote_fs = app_state
                .get_remote_file_service_async()
                .await
                .map_err(|e| format!("Remote file service not available: {}", e))?;
            remote_fs
                .rename(&entry.connection_id, old_path, new_path)
                .await
                .map_err(|e| format!("Failed to rename remote file: {}", e))
        }
    }
}

pub async fn delete_file(app_state: &AppState, raw_path: &str) -> Result<(), String> {
    match resolve_desktop_path_target(app_state, raw_path, None).await? {
        DesktopPathTarget::Local { resolved_path, .. } => app_state
            .filesystem_service
            .delete_file(&resolved_path.to_string_lossy())
            .await
            .map_err(|e| format!("Failed to delete file: {}", e)),
        DesktopPathTarget::Remote {
            requested_path,
            entry,
        } => {
            let remote_fs = app_state
                .get_remote_file_service_async()
                .await
                .map_err(|e| format!("Remote file service not available: {}", e))?;
            remote_fs
                .remove_file(&entry.connection_id, &requested_path)
                .await
                .map_err(|e| format!("Failed to delete remote file: {}", e))
        }
    }
}

pub async fn delete_directory(
    app_state: &AppState,
    raw_path: &str,
    recursive: bool,
) -> Result<(), String> {
    match resolve_desktop_path_target(app_state, raw_path, None).await? {
        DesktopPathTarget::Local { resolved_path, .. } => app_state
            .filesystem_service
            .delete_directory(&resolved_path.to_string_lossy(), recursive)
            .await
            .map_err(|e| format!("Failed to delete directory: {}", e)),
        DesktopPathTarget::Remote {
            requested_path,
            entry,
        } => {
            let remote_fs = app_state
                .get_remote_file_service_async()
                .await
                .map_err(|e| format!("Remote file service not available: {}", e))?;
            if recursive {
                remote_fs
                    .remove_dir_all(&entry.connection_id, &requested_path)
                    .await
                    .map_err(|e| format!("Failed to delete remote directory: {}", e))
            } else {
                remote_fs
                    .remove_dir_all(&entry.connection_id, &requested_path)
                    .await
                    .map_err(|e| format!("Failed to delete remote directory: {}", e))
            }
        }
    }
}

pub async fn create_empty_file(app_state: &AppState, raw_path: &str) -> Result<(), String> {
    match resolve_desktop_path_target(app_state, raw_path, None).await? {
        DesktopPathTarget::Local { resolved_path, .. } => {
            let options = FileOperationOptions::default();
            app_state
                .filesystem_service
                .write_file_with_options(&resolved_path.to_string_lossy(), "", options)
                .await
                .map(|_| ())
                .map_err(|e| format!("Failed to create file: {}", e))
        }
        DesktopPathTarget::Remote {
            requested_path,
            entry,
        } => {
            let remote_fs = app_state
                .get_remote_file_service_async()
                .await
                .map_err(|e| format!("Remote file service not available: {}", e))?;
            remote_fs
                .write_file(&entry.connection_id, &requested_path, b"")
                .await
                .map_err(|e| format!("Failed to create remote file: {}", e))
        }
    }
}

pub async fn create_directory(app_state: &AppState, raw_path: &str) -> Result<(), String> {
    match resolve_desktop_path_target(app_state, raw_path, None).await? {
        DesktopPathTarget::Local { resolved_path, .. } => app_state
            .filesystem_service
            .create_directory(&resolved_path.to_string_lossy())
            .await
            .map_err(|e| format!("Failed to create directory: {}", e)),
        DesktopPathTarget::Remote {
            requested_path,
            entry,
        } => {
            let remote_fs = app_state
                .get_remote_file_service_async()
                .await
                .map_err(|e| format!("Remote file service not available: {}", e))?;
            remote_fs
                .create_dir_all(&entry.connection_id, &requested_path)
                .await
                .map_err(|e| format!("Failed to create remote directory: {}", e))
        }
    }
}
