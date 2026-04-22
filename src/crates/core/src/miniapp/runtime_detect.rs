//! Runtime detection — Bun first, Node.js fallback for JS Worker.
//!
//! On macOS, GUI apps launched from the Finder/Dock inherit a minimal PATH
//! (`/usr/bin:/bin:/usr/sbin:/sbin`) and miss the user's shell-managed
//! installs of Bun / Node (Homebrew, nvm, fnm, volta, asdf, .bun/bin, …).
//! `which::which` only consults `$PATH`, so detection silently fails in the
//! bundled `.app` even though it works fine under `pnpm run desktop:dev`.
//!
//! To make detection work in both contexts we:
//!   1. Try `which::which` (covers shell-launched and Linux/Windows cases).
//!   2. Fall back to a curated list of common install locations.
//!   3. Glob nvm / fnm / volta version directories so any installed Node is
//!      picked up regardless of the active version.

use std::path::{Path, PathBuf};
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
    if let Some(p) = find_executable("bun") {
        if let Ok(version) = get_version(&p) {
            return Some(DetectedRuntime {
                kind: RuntimeKind::Bun,
                path: p,
                version,
            });
        }
    }
    if let Some(p) = find_executable("node") {
        if let Ok(version) = get_version(&p) {
            return Some(DetectedRuntime {
                kind: RuntimeKind::Node,
                path: p,
                version,
            });
        }
    }
    None
}

fn find_executable(name: &str) -> Option<PathBuf> {
    if let Ok(p) = which::which(name) {
        return Some(p);
    }
    for candidate in candidate_dirs() {
        let exe = candidate.join(name);
        if is_executable(&exe) {
            return Some(exe);
        }
    }
    // nvm / fnm / volta layouts: <root>/<version>/bin/<name>
    for root in version_manager_roots() {
        if let Ok(read) = std::fs::read_dir(&root) {
            for entry in read.flatten() {
                let exe = entry.path().join("bin").join(name);
                if is_executable(&exe) {
                    return Some(exe);
                }
            }
        }
    }
    None
}

fn candidate_dirs() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ];
    if let Some(home) = home_dir() {
        v.push(home.join(".bun").join("bin"));
        v.push(home.join(".volta").join("bin"));
        v.push(home.join(".local").join("bin"));
        v.push(home.join(".cargo").join("bin"));
        v.push(home.join(".asdf").join("shims"));
    }
    v
}

fn version_manager_roots() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(home) = home_dir() {
        v.push(home.join(".nvm").join("versions").join("node"));
        v.push(home.join(".fnm").join("node-versions"));
        v.push(
            home.join("Library")
                .join("Application Support")
                .join("fnm")
                .join("node-versions"),
        );
    }
    v
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn is_executable(p: &Path) -> bool {
    p.is_file()
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
