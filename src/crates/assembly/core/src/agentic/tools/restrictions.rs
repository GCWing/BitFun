use crate::util::errors::{BitFunError, BitFunResult};
pub use bitfun_agent_tools::{
    classify_tool_call, is_miniapp_headless_agent_run, is_miniapp_market_strict_agent_run,
    is_remote_posix_path_within_root, miniapp_agent_run_tool_restrictions,
    miniapp_headless_agent_tool_restrictions, miniapp_market_strict_agent_tool_restrictions,
    subagent_tool_restrictions, tool_restrictions_for_delegation_policy, OperationClass,
    ToolPathOperation, ToolPathPolicy, ToolRestrictionError, ToolRuntimeRestrictions,
    ToolRuntimeRestrictionsPatch,
};
use std::path::{Path, PathBuf};

/// Update tool runtime restrictions for a specific session.
///
/// RBAC role system removed (R-WF-01): retained as a no-op stub so existing
/// callers and re-exports keep compiling. Session-level overrides are no longer
/// used; enforcement falls back to the context-level [`ToolRuntimeRestrictions`].
pub fn update_restrictions(
    _session_id: &str,
    _patch: ToolRuntimeRestrictionsPatch,
) -> BitFunResult<()> {
    Ok(())
}

/// Retrieve the session-specific restrictions, if any.
///
/// RBAC role system removed (R-WF-01): no per-session override is ever
/// registered, so this always returns `None` and consumers fall back to the
/// context-level restrictions.
pub fn get_session_restrictions(_session_id: &str) -> Option<ToolRuntimeRestrictions> {
    None
}

/// Remove the session-specific tool restrictions (session-end cleanup).
///
/// RBAC role system removed (R-WF-01): retained as a no-op stub.
pub fn clear_session_restrictions(_session_id: &str) {}

impl From<ToolRestrictionError> for BitFunError {
    fn from(error: ToolRestrictionError) -> Self {
        BitFunError::tool(error.to_string())
    }
}

pub fn is_local_path_within_root(path: &Path, root: &Path) -> BitFunResult<bool> {
    let canonical_path = canonicalize_local_path_best_effort(path)?;
    let canonical_root = canonicalize_local_path_best_effort(root)?;
    Ok(canonical_path == canonical_root || canonical_path.starts_with(&canonical_root))
}

pub(crate) fn canonicalize_local_path_best_effort(path: &Path) -> BitFunResult<PathBuf> {
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

    #[test]
    fn session_restrictions_always_none_after_rbac_removal() {
        // R-WF-01 D2: the RBAC role system is removed, so no per-session
        // override is ever registered. get_session_restrictions must always
        // return None so consumers fall back to the context-level restrictions.
        let session_id = "test-session-restrictions-none-01";
        assert_eq!(get_session_restrictions(session_id), None);
        update_restrictions(session_id, ToolRuntimeRestrictionsPatch::default())
            .expect("no-op update must succeed");
        assert_eq!(get_session_restrictions(session_id), None);
        clear_session_restrictions(session_id);
        assert_eq!(get_session_restrictions(session_id), None);
    }
}
