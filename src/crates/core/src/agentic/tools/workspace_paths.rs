//! Workspace path resolution for agent tools.
//!
//! When BitFun runs on Windows but the open workspace is a **remote SSH** (POSIX) tree,
//! `std::path::Path` treats paths like `/home/user/proj` as non-absolute and joins them
//! incorrectly. Remote sessions must use POSIX path semantics for tool arguments.

use crate::util::errors::{BitFunError, BitFunResult};
use std::path::{Component, Path, PathBuf};

pub fn normalize_path(path: &str) -> String {
    let path = Path::new(path);
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !components.is_empty() {
                    components.pop();
                }
            }
            c => components.push(c),
        }
    }
    components
        .iter()
        .collect::<PathBuf>()
        .to_string_lossy()
        .to_string()
}

pub fn resolve_path_with_workspace(
    path: &str,
    workspace_root: Option<&Path>,
) -> BitFunResult<String> {
    if Path::new(path).is_absolute() {
        Ok(normalize_path(path))
    } else {
        let base_path = workspace_root.ok_or_else(|| {
            BitFunError::tool(format!(
                "A workspace path is required to resolve relative path: {}",
                path
            ))
        })?;

        Ok(normalize_path(
            base_path.join(path).to_string_lossy().as_ref(),
        ))
    }
}

pub fn resolve_path(path: &str) -> BitFunResult<String> {
    resolve_path_with_workspace(path, None)
}

/// POSIX absolute: after normalizing backslashes, path starts with `/`.
pub fn posix_style_path_is_absolute(path: &str) -> bool {
    let p = path.trim().replace('\\', "/");
    p.starts_with('/')
}

fn posix_normalize_components(path: &str) -> String {
    let path = path.trim().replace('\\', "/");
    let is_abs = path.starts_with('/');
    let mut stack: Vec<String> = Vec::new();
    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            stack.pop();
        } else {
            stack.push(part.to_string());
        }
    }
    let body = stack.join("/");
    if is_abs {
        format!("/{}", body)
    } else {
        body
    }
}

/// Resolve a path using POSIX rules (for remote SSH workspaces).
pub fn posix_resolve_path_with_workspace(
    path: &str,
    workspace_root: Option<&str>,
) -> BitFunResult<String> {
    let path = path.trim();
    if path.is_empty() {
        return Err(BitFunError::tool("path cannot be empty".to_string()));
    }

    let normalized_input = path.replace('\\', "/");

    let combined = if posix_style_path_is_absolute(&normalized_input) {
        normalized_input
    } else {
        let base = workspace_root
            .ok_or_else(|| {
                BitFunError::tool(format!(
                    "A workspace path is required to resolve relative path: {}",
                    path
                ))
            })?
            .trim()
            .replace('\\', "/");
        let base = base.trim_end_matches('/');
        format!("{}/{}", base, normalized_input)
    };

    Ok(posix_normalize_components(&combined))
}

/// Unified resolver: POSIX semantics when the workspace is remote SSH; otherwise host `Path`.
pub fn resolve_workspace_tool_path(
    path: &str,
    workspace_root: Option<&str>,
    workspace_is_remote: bool,
) -> BitFunResult<String> {
    if workspace_is_remote {
        posix_resolve_path_with_workspace(path, workspace_root)
    } else {
        resolve_path_with_workspace(path, workspace_root.map(Path::new))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_relative_paths_from_workspace_root() {
        let resolved = resolve_path_with_workspace("src/main.rs", Some(Path::new("/repo")))
            .expect("path should resolve");

        assert_eq!(resolved, "/repo/src/main.rs");
    }

    #[test]
    fn posix_absolute_starts_with_slash() {
        let r =
            posix_resolve_path_with_workspace("/home/user/file.txt", Some("/should/not/matter"))
                .unwrap();
        assert_eq!(r, "/home/user/file.txt");
    }

    #[test]
    fn posix_relative_joins_workspace() {
        let r = posix_resolve_path_with_workspace("src/main.rs", Some("/home/proj")).unwrap();
        assert_eq!(r, "/home/proj/src/main.rs");
    }
}
