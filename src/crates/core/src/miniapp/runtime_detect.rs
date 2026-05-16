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

use bitfun_product_domains::miniapp::runtime::{candidate_dirs, version_manager_roots};
pub use bitfun_product_domains::miniapp::runtime::{DetectedRuntime, RuntimeKind};

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
    let home = home_dir();
    for candidate in candidate_dirs(home.as_deref()) {
        let exe = candidate.join(name);
        if is_executable(&exe) {
            return Some(exe);
        }
    }
    // nvm / fnm / volta layouts: <root>/<version>/bin/<name>
    for root in version_manager_roots(home.as_deref()) {
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

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn is_executable(p: &Path) -> bool {
    p.is_file()
}

fn get_version(executable: &std::path::Path) -> Result<String, std::io::Error> {
    let out = crate::util::process_manager::create_command(executable)
        .arg("--version")
        .output()?;
    if out.status.success() {
        let v = String::from_utf8_lossy(&out.stdout);
        Ok(v.trim().to_string())
    } else {
        Err(std::io::Error::other("version check failed"))
    }
}
