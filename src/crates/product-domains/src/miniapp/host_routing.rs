//! MiniApp host-routing string helpers.

use std::path::Path;

const HOST_NAMESPACES: &[&str] = &["fs", "shell", "os", "net"];

/// Returns true when `method` belongs to a namespace served by the host directly.
///
/// `storage.*` is intentionally excluded: it is routed through MiniApp storage
/// from the command layer so it can share locking with the rest of the app.
pub fn is_host_primitive(method: &str) -> bool {
    method
        .split_once('.')
        .map(|(ns, _)| HOST_NAMESPACES.contains(&ns))
        .unwrap_or(false)
}

pub fn command_basename_for_allowlist(command: &str) -> String {
    let file_name = command
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(command);
    Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name)
        .to_lowercase()
}

pub fn command_basename_allowed(allowlist: &[String], basename: &str) -> bool {
    allowlist.is_empty()
        || allowlist
            .iter()
            .any(|allowed| allowed.to_lowercase() == basename)
}

pub fn host_allowed_by_allowlist(allowlist: &[String], host: &str) -> bool {
    allowlist.is_empty()
        || allowlist.iter().any(|allowed| {
            allowed == "*" || host == allowed || host.ends_with(&format!(".{}", allowed))
        })
}
