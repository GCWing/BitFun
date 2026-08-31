use crate::util::string::shell_single_quote;
use bitfun_runtime_ports::{WorkspaceFileSystem, WorkspacePathKind};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalDeleteTarget {
    pub exists: bool,
    pub is_directory: bool,
    pub is_empty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteLocalPathRequest {
    pub logical_path: String,
    pub resolved_path: PathBuf,
    pub recursive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteLocalPathOutcome {
    pub logical_path: String,
    pub is_directory: bool,
    pub recursive: bool,
}

pub fn delete_path_success_message(path: &str, is_directory: bool) -> String {
    let type_name = if is_directory { "directory" } else { "file" };
    format!("Successfully deleted {} at: {}", type_name, path)
}

/// Inspect through the bound workspace, without following a symlink into its target.
pub async fn inspect_workspace_delete_target(
    fs: &dyn WorkspaceFileSystem,
    path: &str,
) -> Result<LocalDeleteTarget, String> {
    let kind = fs
        .path_kind_no_follow(path)
        .await
        .map_err(|error| format!("{error:#}"))?;
    let is_directory = kind == Some(WorkspacePathKind::Directory);
    let is_empty = if is_directory {
        let listing = fs
            .read_dir_bounded(path, 1)
            .await
            .map_err(|error| format!("{error:#}"))?;
        listing.is_empty()
    } else {
        false
    };
    Ok(LocalDeleteTarget {
        exists: kind.is_some(),
        is_directory,
        is_empty,
    })
}

pub async fn delete_workspace_path(
    fs: &dyn WorkspaceFileSystem,
    logical_path: &str,
    resolved_path: &str,
    recursive: bool,
) -> Result<DeleteLocalPathOutcome, String> {
    let target = inspect_workspace_delete_target(fs, resolved_path).await?;
    if !target.exists {
        return Err(format!("Path does not exist: {logical_path}"));
    }
    if target.is_directory {
        if !recursive && !target.is_empty {
            return Err(format!("Directory is not empty: {logical_path}. Set recursive=true to delete non-empty directories"));
        }
        fs.remove_dir(resolved_path, recursive).await
    } else {
        fs.remove_file(resolved_path).await
    }.map_err(|error| format!("Failed to delete {logical_path}: {error:#}"))?;
    Ok(DeleteLocalPathOutcome {
        logical_path: logical_path.into(),
        is_directory: target.is_directory,
        recursive,
    })
}

pub fn inspect_local_delete_target(path: &Path) -> Result<LocalDeleteTarget, String> {
    if !path.exists() {
        return Ok(LocalDeleteTarget {
            exists: false,
            is_directory: false,
            is_empty: false,
        });
    }

    let is_directory = path.is_dir();
    let is_empty = if is_directory {
        fs::read_dir(path)
            .map_err(|error| format!("Failed to read directory: {}", error))?
            .next()
            .is_none()
    } else {
        false
    };

    Ok(LocalDeleteTarget {
        exists: true,
        is_directory,
        is_empty,
    })
}

pub fn delete_local_path(
    request: DeleteLocalPathRequest,
) -> Result<DeleteLocalPathOutcome, String> {
    let target = inspect_local_delete_target(&request.resolved_path)?;
    if !target.exists {
        return Err(format!("Path does not exist: {}", request.logical_path));
    }

    if target.is_directory {
        if request.recursive {
            fs::remove_dir_all(&request.resolved_path)
                .map_err(|error| format!("Failed to delete directory: {}", error))?;
        } else {
            fs::remove_dir(&request.resolved_path)
                .map_err(|error| format!("Failed to delete directory: {}", error))?;
        }
    } else {
        fs::remove_file(&request.resolved_path)
            .map_err(|error| format!("Failed to delete file: {}", error))?;
    }

    Ok(DeleteLocalPathOutcome {
        logical_path: request.logical_path,
        is_directory: target.is_directory,
        recursive: request.recursive,
    })
}

pub fn build_remote_delete_command(resolved_path: &str, recursive: bool) -> String {
    if recursive {
        format!("rm -rf {}", shell_single_quote(resolved_path))
    } else {
        format!("rm -f {}", shell_single_quote(resolved_path))
    }
}

#[cfg(test)]
mod tests {
    use super::delete_path_success_message;

    #[test]
    fn delete_path_success_message_reports_target_type() {
        assert_eq!(
            delete_path_success_message("old.txt", false),
            "Successfully deleted file at: old.txt"
        );
        assert_eq!(
            delete_path_success_message("tmp", true),
            "Successfully deleted directory at: tmp"
        );
    }
}
