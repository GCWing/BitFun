use crate::util::errors::{BitFunError, BitFunResult};
use std::path::Path;
use std::path::{Component, PathBuf};

pub fn normalize_path(path: &str) -> String {
    let path = Path::new(path);
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {} // Ignore "."
            Component::ParentDir => {
                // Handle ".."
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
    current_working_directory: Option<&Path>,
    workspace_root: Option<&Path>,
) -> BitFunResult<String> {
    if Path::new(path).is_absolute() {
        Ok(normalize_path(path))
    } else {
        let base_path = current_working_directory.or(workspace_root).ok_or_else(|| {
            BitFunError::tool(format!(
                "A current working directory or workspace path is required to resolve relative path: {}",
                path
            ))
        })?;

        Ok(normalize_path(&base_path.join(path).to_string_lossy().to_string()))
    }
}

pub fn resolve_path(path: &str) -> BitFunResult<String> {
    resolve_path_with_workspace(path, None, None)
}

#[cfg(test)]
mod tests {
    use super::resolve_path_with_workspace;
    use std::path::Path;

    #[test]
    fn resolves_relative_paths_from_current_working_directory_first() {
        let resolved = resolve_path_with_workspace(
            "src/main.rs",
            Some(Path::new("/repo/crates/core")),
            Some(Path::new("/repo")),
        )
        .expect("path should resolve");

        assert_eq!(resolved, "/repo/crates/core/src/main.rs");
    }

    #[test]
    fn falls_back_to_workspace_root_when_current_working_directory_missing() {
        let resolved =
            resolve_path_with_workspace("src/main.rs", None, Some(Path::new("/repo")))
                .expect("path should resolve");

        assert_eq!(resolved, "/repo/src/main.rs");
    }
}
