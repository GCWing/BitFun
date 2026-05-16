use crate::util::errors::{BitFunError, BitFunResult};
pub use bitfun_agent_tools::{
    ToolPathOperation, ToolPathPolicy, ToolRestrictionError, ToolRuntimeRestrictions,
};
use std::path::{Path, PathBuf};

impl From<ToolRestrictionError> for BitFunError {
    fn from(error: ToolRestrictionError) -> Self {
        BitFunError::validation(error.to_string())
    }
}

pub fn is_local_path_within_root(path: &Path, root: &Path) -> BitFunResult<bool> {
    let canonical_path = canonicalize_best_effort(path)?;
    let canonical_root = canonicalize_best_effort(root)?;
    Ok(canonical_path == canonical_root || canonical_path.starts_with(&canonical_root))
}

pub fn is_remote_posix_path_within_root(path: &str, root: &str) -> bool {
    let normalized_path = normalize_absolute_posix_path(path);
    let normalized_root = normalize_absolute_posix_path(root);

    if !normalized_path.starts_with('/') || !normalized_root.starts_with('/') {
        return false;
    }

    if normalized_root == "/" {
        return true;
    }

    normalized_path == normalized_root
        || normalized_path
            .strip_prefix(&normalized_root)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn canonicalize_best_effort(path: &Path) -> BitFunResult<PathBuf> {
    if path.exists() {
        return dunce::canonicalize(path).map_err(|err| {
            BitFunError::validation(format!(
                "Failed to canonicalize path '{}': {}",
                path.display(),
                err
            ))
        });
    }

    let mut missing_tail: Vec<PathBuf> = Vec::new();
    let mut current = path;

    loop {
        if current.exists() {
            let mut canonical = dunce::canonicalize(current).map_err(|err| {
                BitFunError::validation(format!(
                    "Failed to canonicalize path '{}': {}",
                    current.display(),
                    err
                ))
            })?;

            for suffix in missing_tail.iter().rev() {
                canonical.push(suffix);
            }

            return Ok(canonical);
        }

        let file_name = current.file_name().ok_or_else(|| {
            BitFunError::validation(format!(
                "Path '{}' cannot be normalized for restriction checks",
                path.display()
            ))
        })?;
        missing_tail.push(PathBuf::from(file_name));

        current = current.parent().ok_or_else(|| {
            BitFunError::validation(format!(
                "Path '{}' cannot be normalized for restriction checks",
                path.display()
            ))
        })?;
    }
}

fn normalize_absolute_posix_path(path: &str) -> String {
    let normalized = path.trim().replace('\\', "/");
    let is_absolute = normalized.starts_with('/');
    let mut segments = Vec::new();

    for segment in normalized.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if !segments.is_empty() {
                    segments.pop();
                }
            }
            value => segments.push(value.to_string()),
        }
    }

    let body = segments.join("/");
    if is_absolute {
        if body.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", body)
        }
    } else {
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_restrictions_allow_all_when_empty() {
        let restrictions = ToolRuntimeRestrictions::default();

        assert!(restrictions.is_tool_allowed("Write"));
        assert!(restrictions.ensure_tool_allowed("Write").is_ok());
    }

    #[test]
    fn denied_tool_names_override_allow_list() {
        let restrictions = ToolRuntimeRestrictions {
            allowed_tool_names: ["Write", "Edit"].into_iter().map(str::to_string).collect(),
            denied_tool_names: ["Write"].into_iter().map(str::to_string).collect(),
            path_policy: ToolPathPolicy::default(),
        };

        assert!(!restrictions.is_tool_allowed("Write"));
        assert!(restrictions.is_tool_allowed("Edit"));
    }

    #[test]
    fn remote_posix_roots_require_true_containment() {
        assert!(is_remote_posix_path_within_root(
            "/workspace/src/lib.rs",
            "/workspace/src"
        ));
        assert!(!is_remote_posix_path_within_root(
            "/workspace/src2/lib.rs",
            "/workspace/src"
        ));
    }

    #[test]
    fn local_path_containment_handles_missing_children() {
        let root =
            std::env::temp_dir().join(format!("bitfun-restrictions-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("allowed")).expect("create temp root");

        let allowed_child = root.join("allowed").join("nested").join("file.txt");
        let sibling = root.join("blocked").join("file.txt");

        assert!(is_local_path_within_root(&allowed_child, &root.join("allowed")).unwrap());
        assert!(!is_local_path_within_root(&sibling, &root.join("allowed")).unwrap());

        let _ = std::fs::remove_dir_all(&root);
    }
}
