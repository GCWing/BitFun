//! Workspace path resolution for agent tools.
//!
//! When BitFun runs on Windows but the open workspace is a **remote SSH** (POSIX) tree,
//! `std::path::Path` treats paths like `/home/user/proj` as non-absolute and joins them
//! incorrectly. Remote sessions must use POSIX path semantics for tool arguments.

use crate::util::errors::{BitFunError, BitFunResult};
pub use bitfun_agent_tools::{
    is_bitfun_runtime_uri, ParsedBitFunRuntimeUri, BITFUN_RUNTIME_URI_PREFIX,
};
use std::path::Path;

pub fn normalize_path(path: &str) -> String {
    bitfun_agent_tools::normalize_host_path(path)
}

pub fn resolve_path_with_workspace(
    path: &str,
    workspace_root: Option<&Path>,
) -> BitFunResult<String> {
    bitfun_agent_tools::resolve_host_path_with_workspace(path, workspace_root)
        .map_err(|error| BitFunError::tool(error.to_string()))
}

pub fn resolve_path(path: &str) -> BitFunResult<String> {
    bitfun_agent_tools::resolve_host_path(path)
        .map_err(|error| BitFunError::tool(error.to_string()))
}

pub fn normalize_runtime_relative_path(path: &str) -> BitFunResult<String> {
    bitfun_agent_tools::normalize_runtime_relative_path(path)
        .map_err(|error| BitFunError::tool(error.to_string()))
}

pub fn parse_bitfun_runtime_uri(path: &str) -> BitFunResult<ParsedBitFunRuntimeUri> {
    bitfun_agent_tools::parse_bitfun_runtime_uri(path)
        .map_err(|error| BitFunError::tool(error.to_string()))
}

pub fn build_bitfun_runtime_uri(
    workspace_scope: &str,
    relative_path: &str,
) -> BitFunResult<String> {
    bitfun_agent_tools::build_bitfun_runtime_uri(workspace_scope, relative_path)
        .map_err(|error| BitFunError::tool(error.to_string()))
}

/// POSIX absolute: after normalizing backslashes, path starts with `/`.
pub fn posix_style_path_is_absolute(path: &str) -> bool {
    bitfun_agent_tools::posix_style_path_is_absolute(path)
}

/// Resolve a path using POSIX rules (for remote SSH workspaces).
pub fn posix_resolve_path_with_workspace(
    path: &str,
    workspace_root: Option<&str>,
) -> BitFunResult<String> {
    bitfun_agent_tools::posix_resolve_path_with_workspace(path, workspace_root)
        .map_err(|error| BitFunError::tool(error.to_string()))
}

/// Unified resolver: POSIX semantics when the workspace is remote SSH; otherwise host `Path`.
pub fn resolve_workspace_tool_path(
    path: &str,
    workspace_root: Option<&str>,
    workspace_is_remote: bool,
) -> BitFunResult<String> {
    bitfun_agent_tools::resolve_workspace_tool_path(path, workspace_root, workspace_is_remote)
        .map_err(|error| BitFunError::tool(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
