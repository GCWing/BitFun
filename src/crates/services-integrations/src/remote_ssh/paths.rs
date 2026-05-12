//! Remote SSH workspace path and identity helpers.

use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// SSH host label for local disk workspaces (`Normal` / `Assistant`).
pub const LOCAL_WORKSPACE_SSH_HOST: &str = "localhost";

/// Normalize a remote POSIX workspace path for registry lookup on any client OS.
pub fn normalize_remote_workspace_path(path: &str) -> String {
    let mut s = path.replace('\\', "/");
    while s.contains("//") {
        s = s.replace("//", "/");
    }
    if s == "/" {
        return s;
    }
    s.trim_end_matches('/').to_string()
}

/// Characters invalid in a single Windows path component.
pub fn sanitize_ssh_connection_id_for_local_dir(connection_id: &str) -> String {
    #[cfg(windows)]
    {
        connection_id
            .chars()
            .map(|c| match c {
                '<' | '>' | '"' | ':' | '/' | '\\' | '|' | '?' | '*' => '-',
                c if c.is_control() => '-',
                _ => c,
            })
            .collect()
    }
    #[cfg(not(windows))]
    {
        connection_id.to_string()
    }
}

/// Sanitize a single path component for the local remote-workspace mirror tree.
pub fn sanitize_remote_mirror_path_component(component: &str) -> String {
    let t = component.trim();
    if t.is_empty() {
        return "_".to_string();
    }
    #[cfg(windows)]
    {
        t.chars()
            .map(|c| match c {
                '<' | '>' | '"' | ':' | '/' | '\\' | '|' | '?' | '*' => '-',
                c if c.is_control() => '-',
                _ => c,
            })
            .collect()
    }
    #[cfg(not(windows))]
    {
        t.chars()
            .map(|c| if c == '/' || c == '\0' { '-' } else { c })
            .collect()
    }
}

/// SSH host or alias as a single directory name under `remote_ssh/`.
pub fn sanitize_ssh_hostname_for_mirror(host: &str) -> String {
    sanitize_remote_mirror_path_component(&host.trim().to_lowercase())
}

/// Map normalized remote workspace root to path segments under the host directory.
pub fn remote_root_to_mirror_subpath(remote_root_norm: &str) -> PathBuf {
    let mut pb = PathBuf::new();
    if remote_root_norm == "/" {
        pb.push("_root");
        return pb;
    }
    for seg in remote_root_norm.trim_start_matches('/').split('/') {
        if seg.is_empty() {
            continue;
        }
        pb.push(sanitize_remote_mirror_path_component(seg));
    }
    if pb.as_os_str().is_empty() {
        pb.push("_root");
    }
    pb
}

/// Human-readable logical key: `{host}:{normalized_absolute_root}`.
pub fn workspace_logical_key(ssh_host: &str, root_norm: &str) -> String {
    format!("{}:{}", ssh_host.trim(), root_norm)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn hash_host_and_root(host: &str, root_norm: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(host.trim().to_lowercase().as_bytes());
    hasher.update(b"\n");
    hasher.update(root_norm.as_bytes());
    hex_encode(&hasher.finalize()[..16])
}

/// Stable storage id for a local workspace (`localhost` + canonical absolute root).
pub fn local_workspace_stable_storage_id(canonical_root_norm: &str) -> String {
    format!(
        "local_{}",
        hash_host_and_root(LOCAL_WORKSPACE_SSH_HOST, canonical_root_norm)
    )
}

/// Stable workspace id from SSH host + normalized remote root.
pub fn remote_workspace_stable_id(ssh_host: &str, remote_root_norm: &str) -> String {
    format!("remote_{}", hash_host_and_root(ssh_host, remote_root_norm))
}

/// Stable unresolved-session key used while a remote host cannot be resolved.
pub fn unresolved_remote_session_storage_key(
    connection_id: &str,
    workspace_path_norm: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"unresolved_remote_session\x01");
    hasher.update(connection_id.trim().as_bytes());
    hasher.update(b"\0");
    hasher.update(workspace_path_norm.as_bytes());
    hex_encode(&hasher.finalize()[..12])
}
