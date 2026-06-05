use crate::util::errors::{BitFunError, BitFunResult};
pub use bitfun_agent_tools::{
    is_remote_posix_path_within_root, ToolPathOperation, ToolPathPolicy, ToolRestrictionError,
    ToolRuntimeRestrictions,
};
use std::path::{Path, PathBuf};

impl From<ToolRestrictionError> for BitFunError {
    fn from(error: ToolRestrictionError) -> Self {
        BitFunError::tool(error.to_string())
    }
}

pub fn is_local_path_within_root(path: &Path, root: &Path) -> BitFunResult<bool> {
    let canonical_path = canonicalize_best_effort(path)?;
    let canonical_root = canonicalize_best_effort(root)?;
    Ok(canonical_path == canonical_root || canonical_path.starts_with(&canonical_root))
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
            denied_tool_messages: Default::default(),
            path_policy: ToolPathPolicy::default(),
        };

        assert!(!restrictions.is_tool_allowed("Write"));
        assert!(restrictions.is_tool_allowed("Edit"));
    }

    #[test]
    fn custom_deny_message_overrides_generic_runtime_error() {
        let restrictions = ToolRuntimeRestrictions {
            denied_tool_names: ["Task"].into_iter().map(str::to_string).collect(),
            denied_tool_messages: [(
                "Task".to_string(),
                "Recursive subagent delegation is blocked. Use direct tools instead.".to_string(),
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        let error = restrictions
            .ensure_tool_allowed("Task")
            .expect_err("custom deny message should be used");
        assert_eq!(
            error.to_string(),
            "Recursive subagent delegation is blocked. Use direct tools instead."
        );
    }

    #[test]
    fn tool_restriction_errors_map_to_tool_errors() {
        let error: BitFunError = ToolRestrictionError::Denied {
            tool_name: "Task".to_string(),
            message: Some(
                "Recursive subagent delegation is blocked. Use direct tools instead.".to_string(),
            ),
        }
        .into();

        match error {
            BitFunError::Tool(message) => {
                assert_eq!(
                    message,
                    "Recursive subagent delegation is blocked. Use direct tools instead."
                )
            }
            other => panic!("expected tool error, got {:?}", other),
        }
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
