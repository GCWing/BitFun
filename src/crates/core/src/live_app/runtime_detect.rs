//! Runtime detection — Bun first, Node.js fallback for JS Worker.

use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeKind {
    Bun,
    Node,
}

#[derive(Debug, Clone)]
pub struct DetectedRuntime {
    pub kind: RuntimeKind,
    pub path: PathBuf,
    pub version: String,
}

/// Detect available JS runtime: Bun first, then Node.js. Returns None if neither is available.
pub fn detect_runtime() -> Option<DetectedRuntime> {
    if let Ok(bun_path) = which::which("bun") {
        if let Ok(version) = get_version(&bun_path) {
            return Some(DetectedRuntime {
                kind: RuntimeKind::Bun,
                path: bun_path,
                version,
            });
        }
    }
    if let Ok(node_path) = which::which("node") {
        if let Ok(version) = get_version(&node_path) {
            return Some(DetectedRuntime {
                kind: RuntimeKind::Node,
                path: node_path,
                version,
            });
        }
    }
    None
}

fn get_version(executable: &std::path::Path) -> Result<String, std::io::Error> {
    let out = Command::new(executable).arg("--version").output()?;
    if out.status.success() {
        let v = String::from_utf8_lossy(&out.stdout);
        Ok(v.trim().to_string())
    } else {
        Err(std::io::Error::other("version check failed"))
    }
}
