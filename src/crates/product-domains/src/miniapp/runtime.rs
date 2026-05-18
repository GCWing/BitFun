//! MiniApp runtime detection contracts and pure search-plan helpers.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

pub fn runtime_lookup_order() -> &'static [&'static str] {
    &["bun", "node"]
}

pub fn runtime_kind_for_executable(name: &str) -> Option<RuntimeKind> {
    match name {
        "bun" => Some(RuntimeKind::Bun),
        "node" => Some(RuntimeKind::Node),
        _ => None,
    }
}

pub fn candidate_executable_path(dir: impl AsRef<Path>, name: &str) -> PathBuf {
    dir.as_ref().join(name)
}

pub fn versioned_executable_candidate(version_dir: impl AsRef<Path>, name: &str) -> PathBuf {
    version_dir.as_ref().join("bin").join(name)
}

/// Common executable directories checked after PATH lookup.
pub fn candidate_dirs(home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ];
    if let Some(home) = home {
        dirs.push(home.join(".bun").join("bin"));
        dirs.push(home.join(".volta").join("bin"));
        dirs.push(home.join(".local").join("bin"));
        dirs.push(home.join(".cargo").join("bin"));
        dirs.push(home.join(".asdf").join("shims"));
    }
    dirs
}

/// Version-manager roots that contain `<version>/bin/<runtime>` layouts.
pub fn version_manager_roots(home: Option<&Path>) -> Vec<PathBuf> {
    let Some(home) = home else {
        return Vec::new();
    };
    vec![
        home.join(".nvm").join("versions").join("node"),
        home.join(".fnm").join("node-versions"),
        home.join("Library")
            .join("Application Support")
            .join("fnm")
            .join("node-versions"),
    ]
}
