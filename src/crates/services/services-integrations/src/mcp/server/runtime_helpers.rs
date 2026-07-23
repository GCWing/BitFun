//! MCP server runtime helper contracts.

use super::{MCPRuntimeError, MCPRuntimeResult};
use bitfun_services_core::managed_runtime::{
    ManagedRuntimeResolver, ResolvedCommand, RuntimeSource,
};
use std::collections::HashMap;
use std::path::PathBuf;

const AUTHORIZATION_KEYS: [&str; 3] = ["Authorization", "authorization", "AUTHORIZATION"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MCPLocalCommandResolution {
    pub command: String,
    pub source_label: &'static str,
}

pub fn resolve_mcp_local_command(
    command: &str,
    runtime_root: impl Into<PathBuf>,
) -> MCPRuntimeResult<MCPLocalCommandResolution> {
    resolve_mcp_local_command_with_resolver(command, ManagedRuntimeResolver::new(runtime_root))
}

pub(crate) fn resolve_mcp_local_command_with_resolver(
    command: &str,
    resolver: ManagedRuntimeResolver,
) -> MCPRuntimeResult<MCPLocalCommandResolution> {
    let resolved = resolver.resolve_command(command).ok_or_else(|| {
        MCPRuntimeError::process(format!(
            "MCP server command '{}' not found in system PATH or BitFun managed runtimes at {}",
            command,
            resolver.runtime_root_display()
        ))
    })?;

    Ok(MCPLocalCommandResolution {
        source_label: mcp_local_command_source_label(&resolved),
        command: resolved.command,
    })
}

fn mcp_local_command_source_label(resolved: &ResolvedCommand) -> &'static str {
    match resolved.source {
        RuntimeSource::System => "system",
        RuntimeSource::Managed => "managed",
    }
}

pub fn is_mcp_auth_error_message(message: &str) -> bool {
    let msg = message.to_ascii_lowercase();
    let patterns = [
        "unauthorized",
        "forbidden",
        "auth required",
        "authorization required",
        "authentication required",
        "authentication failed",
        "oauth authorization required",
        "oauth token refresh failed",
        "token refresh failed",
        "www-authenticate",
        "invalid token",
        "token expired",
        "access token expired",
        "refresh token",
        "session expired",
        "status code: 401",
        "status code: 403",
        " 401 ",
        " 403 ",
    ];
    patterns.iter().any(|p| msg.contains(p))
}

pub fn merge_mcp_remote_headers(
    headers: &HashMap<String, String>,
    env: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut merged_headers = headers.clone();
    if AUTHORIZATION_KEYS
        .iter()
        .all(|key| !merged_headers.contains_key(*key))
    {
        // Backward compatibility: older BitFun configs store `Authorization` under `env`.
        if let Some(value) = AUTHORIZATION_KEYS.iter().find_map(|key| env.get(*key)) {
            merged_headers.insert("Authorization".to_string(), value.clone());
        }
    }

    merged_headers
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn create_test_file(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"test").unwrap();
    }

    fn temp_runtime_root() -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "bitfun-mcp-local-command-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        p
    }

    #[test]
    fn local_command_resolution_reports_resolver_source() {
        let root = temp_runtime_root();
        let node_path = root.join("node").join("current").join("bin").join("node");
        create_test_file(&node_path);

        let resolved = resolve_mcp_local_command_with_resolver(
            "node",
            ManagedRuntimeResolver::new(root.clone()),
        )
        .expect("managed node command");

        if resolved.command == node_path.to_string_lossy() {
            assert_eq!(resolved.source_label, "managed");
        } else {
            assert_eq!(resolved.source_label, "system");
            assert!(resolved.command.to_ascii_lowercase().contains("node"));
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn local_command_resolution_reports_missing_command_as_process_error() {
        let root = temp_runtime_root();
        let error = resolve_mcp_local_command_with_resolver(
            "definitely-missing-bitfun-command",
            ManagedRuntimeResolver::new(root.clone()),
        )
        .expect_err("missing command");

        assert!(error
            .to_string()
            .contains("definitely-missing-bitfun-command"));
        assert!(error
            .to_string()
            .contains(&root.to_string_lossy().to_string()));

        let _ = fs::remove_dir_all(root);
    }
}
