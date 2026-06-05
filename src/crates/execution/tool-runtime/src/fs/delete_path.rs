use crate::util::string::shell_single_quote;
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
