#![cfg(feature = "taiji")]

use crate::agentic::warden::SHAME_WALL_FILENAME;
use crate::util::errors::{BitFunError, BitFunResult};
pub use bitfun_agent_tools::{
    classify_tool_call, is_miniapp_headless_agent_run, is_remote_posix_path_within_root,
    miniapp_headless_agent_tool_restrictions, tool_restrictions_for_delegation_policy,
    OperationClass, ToolPathOperation, ToolPathPolicy, ToolRestrictionError,
    ToolRuntimeRestrictions, ToolRuntimeRestrictionsPatch,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

/// Agent role enum for RBAC permission templates.
/// Determines the default [`ToolRuntimeRestrictions`] assigned to a session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentRole {
    /// Scheduler: ReadOnly + Communicate + Write (.md only via path_policy)
    Commander,
    /// Executor: WriteFile + ExecuteCode
    Executor,
    /// Reviewer: ReadOnly + Communicate
    Reviewer,
    /// Guardian: ReadOnly + Communicate + SessionHistory
    Warden,
    /// Punishment executor: Write (shame-wall) + SessionControl (lock)
    PunishmentExecutor,
}

/// Role→Permission template mapping table.
///
/// Loaded at first access; Warden may trigger role switches at runtime.
pub type RolePermissionMap = HashMap<AgentRole, ToolRuntimeRestrictions>;

static DEFAULT_ROLE_PERMISSIONS: OnceLock<RolePermissionMap> = OnceLock::new();

fn build_default_role_permissions() -> RolePermissionMap {
    let mut map = RolePermissionMap::new();

    // ── Commander ──────────────────────────────────────────────────
    // Allowed operation classes: ReadOnly + Communicate
    // Allowed tool names: Write (TODO: path_policy should restrict to .md files
    // once ToolPathPolicy supports file-extension patterns)
    {
        let mut allowed_ops = BTreeSet::new();
        allowed_ops.insert(OperationClass::ReadOnly);
        allowed_ops.insert(OperationClass::Communicate);
        let mut allowed_tools = BTreeSet::new();
        allowed_tools.insert("Write".to_string());
        map.insert(
            AgentRole::Commander,
            ToolRuntimeRestrictions {
                allowed_operation_classes: allowed_ops,
                allowed_tool_names: allowed_tools,
                ..Default::default()
            },
        );
    }

    // ── Executor ───────────────────────────────────────────────────
    // Allowed operation classes: WriteFile + ExecuteCode
    {
        let mut allowed_ops = BTreeSet::new();
        allowed_ops.insert(OperationClass::WriteFile);
        allowed_ops.insert(OperationClass::ExecuteCode);
        map.insert(
            AgentRole::Executor,
            ToolRuntimeRestrictions {
                allowed_operation_classes: allowed_ops,
                ..Default::default()
            },
        );
    }

    // ── Reviewer ───────────────────────────────────────────────────
    // Allowed operation classes: ReadOnly + Communicate
    {
        let mut allowed_ops = BTreeSet::new();
        allowed_ops.insert(OperationClass::ReadOnly);
        allowed_ops.insert(OperationClass::Communicate);
        map.insert(
            AgentRole::Reviewer,
            ToolRuntimeRestrictions {
                allowed_operation_classes: allowed_ops,
                ..Default::default()
            },
        );
    }

    // ── Warden ─────────────────────────────────────────────────────
    // Allowed operation classes: ReadOnly + Communicate + ExecuteCode (gbrain search)
    // Allowed tool names: SessionHistory (extra, for cross-session inspection),
    //                     ExecCommand (for gbrain search/query across full knowledge base)
    {
        let mut allowed_ops = BTreeSet::new();
        allowed_ops.insert(OperationClass::ReadOnly);
        allowed_ops.insert(OperationClass::Communicate);
        allowed_ops.insert(OperationClass::ExecuteCode);
        let mut allowed_tools = BTreeSet::new();
        allowed_tools.insert("SessionHistory".to_string());
        allowed_tools.insert("ExecCommand".to_string());
        map.insert(
            AgentRole::Warden,
            ToolRuntimeRestrictions {
                allowed_operation_classes: allowed_ops,
                allowed_tool_names: allowed_tools,
                ..Default::default()
            },
        );
    }

    // ── PunishmentExecutor ─────────────────────────────────────────
    // Allowed tool names: Write (path-policy restricted to .master-framework/shame-wall-registry.json),
    //                     SessionControl (lock capability)
    {
        let mut allowed_tools = BTreeSet::new();
        allowed_tools.insert("Write".to_string());
        allowed_tools.insert("SessionControl".to_string());
        let mut path_policy = ToolPathPolicy::default();
        path_policy.write_roots = vec![SHAME_WALL_FILENAME.to_string()];
        map.insert(
            AgentRole::PunishmentExecutor,
            ToolRuntimeRestrictions {
                allowed_tool_names: allowed_tools,
                path_policy,
                ..Default::default()
            },
        );
    }

    map
}

/// Get the default [`ToolRuntimeRestrictions`] for a given role.
///
/// Templates are lazily built on first call and cached for the lifetime of the process.
pub fn get_default_permissions(role: AgentRole) -> ToolRuntimeRestrictions {
    let map = DEFAULT_ROLE_PERMISSIONS.get_or_init(build_default_role_permissions);
    map.get(&role).cloned().unwrap_or_default()
}

/// Global session-specific tool runtime restrictions.
/// Keyed by session_id. If a session has no entry here, the role-default template is used.
static SESSION_RESTRICTIONS: OnceLock<RwLock<HashMap<String, ToolRuntimeRestrictions>>> =
    OnceLock::new();

fn session_restrictions_map(
) -> &'static RwLock<HashMap<String, ToolRuntimeRestrictions>> {
    SESSION_RESTRICTIONS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Update tool runtime restrictions for a specific session.
///
/// If `role` is `Some`, the session's restrictions are first reset to the role's
/// default template before applying the patch. This allows a caller to assign a
/// role baseline and then apply incremental overrides via the patch.
///
/// When `role` is `None`, only the `patch` fields are applied on top of any
/// existing session restrictions, leaving unrelated values unchanged.
pub fn update_restrictions(
    session_id: &str,
    role: Option<AgentRole>,
    patch: ToolRuntimeRestrictionsPatch,
) -> BitFunResult<()> {
    let mut map = session_restrictions_map().write().map_err(|e| {
        BitFunError::tool(format!("Session restrictions lock poisoned: {e}"))
    })?;
    let restrictions = map
        .entry(session_id.to_string())
        .or_insert_with(ToolRuntimeRestrictions::default);

    // If a role is specified, load its default template first
    if let Some(role) = role {
        *restrictions = get_default_permissions(role);
    }

    restrictions.apply_patch(patch);
    Ok(())
}

/// Retrieve the session-specific restrictions, if any.
/// Returns `None` when no per-session override has been registered.
pub fn get_session_restrictions(session_id: &str) -> Option<ToolRuntimeRestrictions> {
    session_restrictions_map()
        .read()
        .ok()
        .and_then(|map| map.get(session_id).cloned())
}

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

    // ── Role→Permission template tests ─────────────────────────────

    #[test]
    fn commander_gets_readonly_and_communicate() {
        let permissions = get_default_permissions(AgentRole::Commander);
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::ReadOnly),
            "Commander should allow ReadOnly"
        );
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::Communicate),
            "Commander should allow Communicate"
        );
        assert!(
            permissions.allowed_tool_names.contains("Write"),
            "Commander should allow Write tool"
        );
        // WriteFile and ExecuteCode should NOT be in allowed_operation_classes
        assert!(
            !permissions
                .allowed_operation_classes
                .contains(&OperationClass::WriteFile),
            "Commander should not allow WriteFile"
        );
        assert!(
            !permissions
                .allowed_operation_classes
                .contains(&OperationClass::ExecuteCode),
            "Commander should not allow ExecuteCode"
        );
    }

    #[test]
    fn executor_gets_writefile_and_executecode() {
        let permissions = get_default_permissions(AgentRole::Executor);
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::WriteFile),
            "Executor should allow WriteFile"
        );
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::ExecuteCode),
            "Executor should allow ExecuteCode"
        );
        // ReadOnly should NOT be in allowed_operation_classes
        assert!(
            !permissions
                .allowed_operation_classes
                .contains(&OperationClass::ReadOnly),
            "Executor should not allow ReadOnly (not in default allowed set)"
        );
    }

    #[test]
    fn reviewer_gets_readonly_and_communicate() {
        let permissions = get_default_permissions(AgentRole::Reviewer);
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::ReadOnly),
            "Reviewer should allow ReadOnly"
        );
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::Communicate),
            "Reviewer should allow Communicate"
        );
        assert!(
            !permissions
                .allowed_operation_classes
                .contains(&OperationClass::WriteFile),
            "Reviewer should not allow WriteFile"
        );
        assert!(
            !permissions
                .allowed_operation_classes
                .contains(&OperationClass::ExecuteCode),
            "Reviewer should not allow ExecuteCode"
        );
    }

    #[test]
    fn warden_gets_readonly_communicate_exec_and_session_history() {
        let permissions = get_default_permissions(AgentRole::Warden);
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::ReadOnly),
            "Warden should allow ReadOnly"
        );
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::Communicate),
            "Warden should allow Communicate"
        );
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::ExecuteCode),
            "Warden should allow ExecuteCode for gbrain search"
        );
        assert!(
            permissions.allowed_tool_names.contains("SessionHistory"),
            "Warden should allow SessionHistory tool"
        );
        assert!(
            permissions.allowed_tool_names.contains("ExecCommand"),
            "Warden should allow ExecCommand for gbrain search/query"
        );
    }

    #[test]
    fn punishment_executor_gets_write_and_session_control() {
        let permissions = get_default_permissions(AgentRole::PunishmentExecutor);
        assert!(
            permissions.allowed_tool_names.contains("Write"),
            "PunishmentExecutor should allow Write tool"
        );
        assert!(
            permissions.allowed_tool_names.contains("SessionControl"),
            "PunishmentExecutor should allow SessionControl tool"
        );
        // path_policy should restrict Write to shame-wall-registry.json under .master-framework
        assert!(
            permissions.path_policy.write_roots.contains(&SHAME_WALL_FILENAME.to_string()),
            "PunishmentExecutor write_roots should contain {}",
            SHAME_WALL_FILENAME
        );
    }

    #[test]
    fn update_restrictions_with_role_loads_template() {
        // Apply Commander role via update_restrictions
        let session_id = "test-session-role-01";
        let patch = ToolRuntimeRestrictionsPatch::default();
        update_restrictions(session_id, Some(AgentRole::Commander), patch)
            .expect("update_restrictions should succeed");

        let stored = get_session_restrictions(session_id)
            .expect("session restrictions should exist after update");

        assert!(
            stored
                .allowed_operation_classes
                .contains(&OperationClass::ReadOnly),
            "Session should have Commander's ReadOnly after role-based update"
        );
        assert!(
            stored
                .allowed_operation_classes
                .contains(&OperationClass::Communicate),
            "Session should have Commander's Communicate after role-based update"
        );
    }

    #[test]
    fn update_restrictions_patch_overrides_role_template() {
        let session_id = "test-session-role-02";
        // Start with Executor, then patch to add ReadOnly
        let mut patch = ToolRuntimeRestrictionsPatch::default();
        let mut extra_ops = BTreeSet::new();
        extra_ops.insert(OperationClass::ReadOnly);
        patch.allowed_operation_classes = Some(extra_ops);

        update_restrictions(session_id, Some(AgentRole::Executor), patch)
            .expect("update_restrictions with role+patch should succeed");

        let stored = get_session_restrictions(session_id)
            .expect("session restrictions should exist");

        // Executor baseline: WriteFile + ExecuteCode
        assert!(
            stored
                .allowed_operation_classes
                .contains(&OperationClass::WriteFile),
            "Should retain Executor's WriteFile"
        );
        assert!(
            stored
                .allowed_operation_classes
                .contains(&OperationClass::ExecuteCode),
            "Should retain Executor's ExecuteCode"
        );
        // Patch adds ReadOnly (since patch replaces the entire set, it should only contain ReadOnly)
        // Actually, apply_patch replaces the field entirely when Some, so after patch:
        // allowed_operation_classes = {ReadOnly} (replacing {WriteFile, ExecuteCode})
        assert!(
            stored
                .allowed_operation_classes
                .contains(&OperationClass::ReadOnly),
            "Patch should add ReadOnly"
        );
        assert!(
            !stored
                .allowed_operation_classes
                .contains(&OperationClass::WriteFile),
            "Patch replaced operation classes, WriteFile should be gone"
        );
    }
}
