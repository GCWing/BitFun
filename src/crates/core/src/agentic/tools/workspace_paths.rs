//! Workspace path resolution for agent tools.
//!
//! When BitFun runs on Windows but the open workspace is a **remote SSH** (POSIX) tree,
//! `std::path::Path` treats paths like `/home/user/proj` as non-absolute and joins them
//! incorrectly. Remote sessions must use POSIX path semantics for tool arguments.

use crate::util::errors::{BitFunError, BitFunResult};
use std::path::{Component, Path, PathBuf};

pub const BITFUN_RUNTIME_URI_PREFIX: &str = "bitfun://runtime/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedBitFunRuntimeUri {
    pub workspace_scope: String,
    pub relative_path: String,
}

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

pub fn is_bitfun_runtime_uri(path: &str) -> bool {
    path.trim().starts_with(BITFUN_RUNTIME_URI_PREFIX)
}

pub fn normalize_runtime_relative_path(path: &str) -> BitFunResult<String> {
    let normalized = path.trim().replace('\\', "/");
    let trimmed = normalized.trim_matches('/');
    if trimmed.is_empty() {
        return Err(BitFunError::tool(
            "Runtime artifact path cannot be empty".to_string(),
        ));
    }

    let mut segments = Vec::new();
    for part in trimmed.split('/') {
        match part {
            "" | "." => continue,
            ".." => {
                return Err(BitFunError::tool(
                    "Runtime artifact path cannot escape its root".to_string(),
                ))
            }
            value => segments.push(value.to_string()),
        }
    }

    if segments.is_empty() {
        return Err(BitFunError::tool(
            "Runtime artifact path cannot be empty".to_string(),
        ));
    }

    Ok(segments.join("/"))
}

pub fn parse_bitfun_runtime_uri(path: &str) -> BitFunResult<ParsedBitFunRuntimeUri> {
    let trimmed = path.trim();
    let suffix = trimmed
        .strip_prefix(BITFUN_RUNTIME_URI_PREFIX)
        .ok_or_else(|| BitFunError::tool(format!("Unsupported runtime URI: {}", path)))?;

    let mut parts = suffix.splitn(2, '/');
    let workspace_scope = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| BitFunError::tool("Runtime URI is missing workspace scope".to_string()))?
        .to_string();
    let relative_path = parts
        .next()
        .ok_or_else(|| BitFunError::tool("Runtime URI is missing artifact path".to_string()))?;

    Ok(ParsedBitFunRuntimeUri {
        workspace_scope,
        relative_path: normalize_runtime_relative_path(relative_path)?,
    })
}

pub fn build_bitfun_runtime_uri(
    workspace_scope: &str,
    relative_path: &str,
) -> BitFunResult<String> {
    let scope = workspace_scope.trim();
    if scope.is_empty() {
        return Err(BitFunError::tool(
            "Runtime URI workspace scope cannot be empty".to_string(),
        ));
    }

    Ok(format!(
        "{}{}/{}",
        BITFUN_RUNTIME_URI_PREFIX,
        scope,
        normalize_runtime_relative_path(relative_path)?
    ))
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

        assert_eq!(
            PathBuf::from(resolved),
            Path::new("/repo").join("src/main.rs")
        );
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

    #[test]
    fn runtime_uri_round_trips_and_normalizes_separators() {
        let uri = build_bitfun_runtime_uri("workspace-123", r"plans\demo.plan.md").unwrap();
        assert_eq!(uri, "bitfun://runtime/workspace-123/plans/demo.plan.md");

        let parsed = parse_bitfun_runtime_uri(&uri).unwrap();
        assert_eq!(parsed.workspace_scope, "workspace-123");
        assert_eq!(parsed.relative_path, "plans/demo.plan.md");
    }

    #[test]
    fn runtime_uri_rejects_parent_directory_escape() {
        let err = build_bitfun_runtime_uri("workspace-123", "../secret.txt")
            .expect_err("runtime URI should reject parent directory escape");

        assert!(err.to_string().contains("cannot escape"));
    }
}
