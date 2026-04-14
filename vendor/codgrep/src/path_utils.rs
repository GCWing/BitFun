use std::path::{Path, PathBuf};

use crate::error::Result;

pub(crate) fn normalize_existing_path(path: &Path) -> Result<PathBuf> {
    let absolute = normalize_path_from_cwd(path)?;
    Ok(std::fs::canonicalize(&absolute).unwrap_or(absolute))
}

pub(crate) fn normalize_path_from_cwd(path: &Path) -> Result<PathBuf> {
    let current_dir = std::env::current_dir()?;
    Ok(normalize_path(path, &current_dir))
}

pub(crate) fn normalize_path(path: &Path, current_dir: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        current_dir.join(path)
    };
    let normalized = normalize_lexical_path(&joined);
    std::fs::canonicalize(&normalized).unwrap_or(normalized)
}

pub(crate) fn repo_relative_path(path: &Path, repo_root: &Path) -> String {
    let normalized_path = normalize_lexical_path(path);
    let normalized_root = normalize_lexical_path(repo_root);
    normalized_path
        .strip_prefix(&normalized_root)
        .map(Path::to_path_buf)
        .unwrap_or(normalized_path)
        .to_string_lossy()
        .into_owned()
}

pub(crate) fn resolve_repo_path(repo_root: &Path, path: &str) -> PathBuf {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return normalize_lexical_path(candidate);
    }
    normalize_lexical_path(&repo_root.join(candidate))
}

pub(crate) fn normalize_lexical_path(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}
