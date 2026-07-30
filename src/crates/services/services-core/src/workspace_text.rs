//! Safe bounded reads for explicitly referenced local workspace text files.

use crate::bounded_fs::{is_symlink_or_reparse, read_bounded_text, BoundedTextRead};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceTextFile {
    pub relative_path: String,
    pub content: String,
    pub byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkspaceTextReadError {
    #[error("workspace root must be an absolute local directory")]
    InvalidWorkspaceRoot,
    #[error("file reference must be a safe workspace-relative path")]
    InvalidRelativePath,
    #[error("referenced workspace file was not found")]
    NotFound,
    #[error("referenced workspace path is not a regular file")]
    NotRegularFile,
    #[error("referenced workspace path must not contain symlinks")]
    Symlink,
    #[error("referenced file resolves outside the workspace")]
    OutsideWorkspace,
    #[error("referenced workspace file exceeds the {max_bytes} byte limit")]
    TooLarge { max_bytes: usize },
    #[error("referenced workspace file is not valid UTF-8")]
    InvalidUtf8,
    #[error("failed to read referenced workspace file: {0}")]
    Io(String),
}

pub async fn read_workspace_relative_text_bounded(
    workspace_root: &Path,
    relative_path: &str,
    max_bytes: usize,
) -> Result<WorkspaceTextFile, WorkspaceTextReadError> {
    let normalized_path = normalize_workspace_relative_path(relative_path)?;
    let components = normalized_path
        .split('/')
        .map(str::to_string)
        .collect::<Vec<_>>();
    let workspace_root = workspace_root.to_path_buf();
    let resolved =
        tokio::task::spawn_blocking(move || resolve_workspace_file(&workspace_root, &components))
            .await
            .map_err(|error| WorkspaceTextReadError::Io(error.to_string()))??;

    let bounded = tokio::task::spawn_blocking(move || read_bounded_text(&resolved, max_bytes))
        .await
        .map_err(|error| WorkspaceTextReadError::Io(error.to_string()))?
        .map_err(map_read_error)?;
    let content = match bounded {
        BoundedTextRead::Content(content) => content,
        BoundedTextRead::TooLarge => return Err(WorkspaceTextReadError::TooLarge { max_bytes }),
        BoundedTextRead::InvalidUtf8 => return Err(WorkspaceTextReadError::InvalidUtf8),
    };
    let byte_len = content.len();
    Ok(WorkspaceTextFile {
        relative_path: normalized_path,
        content,
        byte_len,
    })
}

pub fn normalize_workspace_relative_path(value: &str) -> Result<String, WorkspaceTextReadError> {
    if value.is_empty()
        || value.contains('\0')
        || value.starts_with('~')
        || value.starts_with('/')
        || value.starts_with('\\')
        || value.contains("://")
    {
        return Err(WorkspaceTextReadError::InvalidRelativePath);
    }

    let normalized = value.replace('\\', "/");
    let mut components = Vec::new();
    for component in normalized.split('/') {
        match component {
            "" | "." => {}
            ".." => return Err(WorkspaceTextReadError::InvalidRelativePath),
            component if component.contains(':') => {
                return Err(WorkspaceTextReadError::InvalidRelativePath)
            }
            component => components.push(component.to_string()),
        }
    }
    if components.is_empty() {
        return Err(WorkspaceTextReadError::InvalidRelativePath);
    }
    Ok(components.join("/"))
}

fn resolve_workspace_file(
    workspace_root: &Path,
    components: &[String],
) -> Result<PathBuf, WorkspaceTextReadError> {
    if !workspace_root.is_absolute() {
        return Err(WorkspaceTextReadError::InvalidWorkspaceRoot);
    }
    let canonical_root = std::fs::canonicalize(workspace_root).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            WorkspaceTextReadError::InvalidWorkspaceRoot
        } else {
            WorkspaceTextReadError::Io(error.to_string())
        }
    })?;
    if !std::fs::metadata(&canonical_root)
        .map_err(|error| WorkspaceTextReadError::Io(error.to_string()))?
        .is_dir()
    {
        return Err(WorkspaceTextReadError::InvalidWorkspaceRoot);
    }

    let mut current = canonical_root.clone();
    for (index, component) in components.iter().enumerate() {
        current.push(component);
        let metadata = std::fs::symlink_metadata(&current).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                WorkspaceTextReadError::NotFound
            } else {
                WorkspaceTextReadError::Io(error.to_string())
            }
        })?;
        if is_symlink_or_reparse(&metadata) {
            return Err(WorkspaceTextReadError::Symlink);
        }
        let is_last = index + 1 == components.len();
        if is_last && !metadata.is_file() {
            return Err(WorkspaceTextReadError::NotRegularFile);
        }
        if !is_last && !metadata.is_dir() {
            return Err(WorkspaceTextReadError::NotFound);
        }
    }

    let canonical_file = std::fs::canonicalize(&current).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            WorkspaceTextReadError::NotFound
        } else {
            WorkspaceTextReadError::Io(error.to_string())
        }
    })?;
    if !canonical_file.starts_with(&canonical_root) {
        return Err(WorkspaceTextReadError::OutsideWorkspace);
    }
    Ok(canonical_file)
}

fn map_read_error(error: std::io::Error) -> WorkspaceTextReadError {
    if error.kind() == std::io::ErrorKind::NotFound {
        WorkspaceTextReadError::NotFound
    } else {
        WorkspaceTextReadError::Io(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{read_workspace_relative_text_bounded, WorkspaceTextReadError};
    use std::fs;
    use std::path::Path;

    #[tokio::test]
    async fn reads_nested_utf8_text_with_a_normalized_relative_path() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("src/lib.rs");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "pub fn answer() -> u8 { 42 }").unwrap();

        let result = read_workspace_relative_text_bounded(temp.path(), "src/./lib.rs", 1024)
            .await
            .unwrap();

        assert_eq!(result.relative_path, "src/lib.rs");
        assert_eq!(result.content, "pub fn answer() -> u8 { 42 }");
        assert_eq!(result.byte_len, 28);
    }

    #[tokio::test]
    async fn rejects_non_workspace_relative_syntax() {
        let temp = tempfile::tempdir().unwrap();
        let invalid = [
            "",
            ".",
            "../secret.md",
            "src/../../secret.md",
            "/etc/passwd",
            r"\server\share\secret.md",
            r"C:\secret.md",
            "~/secret.md",
            "http://example.test/secret.md",
            "file:///secret.md",
            "src/bad\0name.md",
        ];

        for path in invalid {
            let error = read_workspace_relative_text_bounded(temp.path(), path, 1024)
                .await
                .unwrap_err();
            assert_eq!(
                error,
                WorkspaceTextReadError::InvalidRelativePath,
                "{path:?}"
            );
        }
    }

    #[tokio::test]
    async fn distinguishes_missing_directories_oversize_and_invalid_utf8() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("docs")).unwrap();
        fs::write(temp.path().join("large.txt"), b"12345").unwrap();
        fs::write(temp.path().join("binary.txt"), [0xff, 0xfe]).unwrap();

        assert_eq!(
            read_workspace_relative_text_bounded(temp.path(), "missing.txt", 4)
                .await
                .unwrap_err(),
            WorkspaceTextReadError::NotFound
        );
        assert_eq!(
            read_workspace_relative_text_bounded(temp.path(), "docs", 4)
                .await
                .unwrap_err(),
            WorkspaceTextReadError::NotRegularFile
        );
        assert_eq!(
            read_workspace_relative_text_bounded(temp.path(), "large.txt", 4)
                .await
                .unwrap_err(),
            WorkspaceTextReadError::TooLarge { max_bytes: 4 }
        );
        assert_eq!(
            read_workspace_relative_text_bounded(temp.path(), "binary.txt", 4)
                .await
                .unwrap_err(),
            WorkspaceTextReadError::InvalidUtf8
        );
    }

    #[tokio::test]
    async fn rejects_symlinked_references_even_when_the_target_is_inside() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("target.txt"), "inside").unwrap();
        let link = temp.path().join("link.txt");
        if !create_file_symlink(Path::new("target.txt"), &link) {
            return;
        }

        assert_eq!(
            read_workspace_relative_text_bounded(temp.path(), "link.txt", 1024)
                .await
                .unwrap_err(),
            WorkspaceTextReadError::Symlink
        );
    }

    #[cfg(unix)]
    fn create_file_symlink(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn create_file_symlink(target: &Path, link: &Path) -> bool {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }
}
