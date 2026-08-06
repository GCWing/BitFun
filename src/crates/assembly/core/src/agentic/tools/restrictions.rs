use crate::agentic::warden::SHAME_WALL_FILENAME;
use crate::util::errors::{BitFunError, BitFunResult};
pub use bitfun_agent_tools::{
    classify_tool_call, is_miniapp_headless_agent_run, is_miniapp_market_strict_agent_run,
    is_remote_posix_path_within_root, miniapp_agent_run_tool_restrictions,
    miniapp_headless_agent_tool_restrictions, miniapp_market_strict_agent_tool_restrictions,
    subagent_tool_restrictions, tool_restrictions_for_delegation_policy, OperationClass,
    ToolPathOperation, ToolPathPolicy, ToolRestrictionError, ToolRuntimeRestrictions,
    ToolRuntimeRestrictionsPatch,
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
    /// Executor: ReadOnly + WriteFile + ExecuteCode
    Executor,
    /// Reviewer: ReadOnly + WriteFile + ExecuteCode
    Reviewer,
    /// Guardian: ReadOnly + WriteFile + Communicate + ExecuteCode + SessionHistory
    Warden,
    /// Punishment executor: Write (shame-wall) + SessionControl (lock)
    PunishmentExecutor,
}

impl AgentRole {
    /// Stable lowercase key persisted with session metadata (R-14 B2).
    ///
    /// Used instead of the serde variant name so metadata survives enum
    /// renames without a migration.
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentRole::Commander => "commander",
            AgentRole::Executor => "executor",
            AgentRole::Reviewer => "reviewer",
            AgentRole::Warden => "warden",
            AgentRole::PunishmentExecutor => "punishment_executor",
        }
    }

    /// Parse a persisted role key. Unknown keys yield `None` so stale metadata
    /// degrades to the commander (permissive) baseline instead of erroring.
    pub fn from_str_key(key: &str) -> Option<AgentRole> {
        match key {
            "commander" => Some(AgentRole::Commander),
            "executor" => Some(AgentRole::Executor),
            "reviewer" => Some(AgentRole::Reviewer),
            "warden" => Some(AgentRole::Warden),
            "punishment_executor" => Some(AgentRole::PunishmentExecutor),
            _ => None,
        }
    }
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
    // once ToolPathPolicy supports file-extension patterns) + the Session
    // toolset (SessionControl/SessionMessage/SessionHistory) used to dispatch
    // and observe delegated sessions.
    {
        let mut allowed_ops = BTreeSet::new();
        allowed_ops.insert(OperationClass::ReadOnly);
        allowed_ops.insert(OperationClass::Communicate);
        let mut allowed_tools = BTreeSet::new();
        allowed_tools.insert("Write".to_string());
        allowed_tools.insert("SessionControl".to_string());
        allowed_tools.insert("SessionMessage".to_string());
        allowed_tools.insert("SessionHistory".to_string());
        // Dedicated ACP tool family mirrors the Session toolset over the real
        // external ACP process channel (true bridge).
        allowed_tools.insert("acp_control".to_string());
        allowed_tools.insert("acp_message".to_string());
        allowed_tools.insert("acp_history".to_string());
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
    // Allowed operation classes: ReadOnly + WriteFile + ExecuteCode
    // （执行者读代码基本能力：Read/Write/Edit/ExecCommand 三件套配齐）
    {
        let mut allowed_ops = BTreeSet::new();
        allowed_ops.insert(OperationClass::ReadOnly);
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
    // Allowed operation classes: ReadOnly + WriteFile + ExecuteCode (≈ Executor).
    // （审查官读代码审查 + 落盘审查报告：Read/Write/Edit/ExecCommand 三件套配齐）
    // Reviewers must be able to inspect and reproduce findings; the signature
    // now intentionally overlaps Executor, so role identity must come from the
    // persisted session role (SESSION_ROLES), never from template inference.
    {
        let mut allowed_ops = BTreeSet::new();
        allowed_ops.insert(OperationClass::ReadOnly);
        allowed_ops.insert(OperationClass::WriteFile);
        allowed_ops.insert(OperationClass::ExecuteCode);
        map.insert(
            AgentRole::Reviewer,
            ToolRuntimeRestrictions {
                allowed_operation_classes: allowed_ops,
                ..Default::default()
            },
        );
    }

    // ── Warden ─────────────────────────────────────────────────────
    // Allowed operation classes: ReadOnly + WriteFile + Communicate + ExecuteCode
    // （守卫审计也需读/落盘：Read/Write/Edit/ExecCommand 三件套配齐）
    // Allowed tool names: SessionHistory (extra, for cross-session inspection),
    //                     ExecCommand (for gbrain search/query across full knowledge base),
    //                     Write/Edit (audit report landing)
    {
        let mut allowed_ops = BTreeSet::new();
        allowed_ops.insert(OperationClass::ReadOnly);
        allowed_ops.insert(OperationClass::WriteFile);
        allowed_ops.insert(OperationClass::Communicate);
        allowed_ops.insert(OperationClass::ExecuteCode);
        let mut allowed_tools = BTreeSet::new();
        allowed_tools.insert("SessionHistory".to_string());
        allowed_tools.insert("ExecCommand".to_string());
        allowed_tools.insert("Write".to_string());
        allowed_tools.insert("Edit".to_string());
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
        let path_policy = ToolPathPolicy {
            write_roots: vec![SHAME_WALL_FILENAME.to_string()],
            ..Default::default()
        };
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

/// GeneralPurpose 专属权限模板（P-01 方案 2）。
///
/// GeneralPurpose 是只读侦察 + 执行混合的子代理：需要 Read/Glob/Grep
/// 等只读工具，而默认 Executor 模板只允许 {WriteFile, ExecuteCode} 会禁掉
/// 只读类。专属模板允许 {ReadOnly, WriteFile, ExecuteCode, Communicate}，
/// 工具集与 general_purpose.rs:17-35 保持一致。
pub fn general_purpose_tool_restrictions() -> ToolRuntimeRestrictions {
    let mut allowed_ops = BTreeSet::new();
    allowed_ops.insert(OperationClass::ReadOnly);
    allowed_ops.insert(OperationClass::WriteFile);
    allowed_ops.insert(OperationClass::ExecuteCode);
    allowed_ops.insert(OperationClass::Communicate);
    let mut allowed_tools = BTreeSet::new();
    for name in GENERAL_PURPOSE_DEFAULT_TOOLS {
        allowed_tools.insert(name.to_string());
    }
    ToolRuntimeRestrictions {
        allowed_operation_classes: allowed_ops,
        allowed_tool_names: allowed_tools,
        ..Default::default()
    }
}

/// GeneralPurpose 默认工具集（与 general_purpose.rs:17-35 保持一致）。
const GENERAL_PURPOSE_DEFAULT_TOOLS: &[&str] = &[
    "Read",
    "view_image",
    "analyze_image",
    "Glob",
    "Grep",
    "Write",
    "Edit",
    "Delete",
    "ExecCommand",
    "WriteStdin",
    "ExecControl",
    "WebSearch",
    "WebFetch",
    "Skill",
    "Task",
];

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

fn session_restrictions_map() -> &'static RwLock<HashMap<String, ToolRuntimeRestrictions>> {
    SESSION_RESTRICTIONS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Global session→role registry (R-14).
///
/// The role is assigned when a session is created (or inherited from its
/// creator) and persisted with the session metadata; this in-memory map is the
/// fast, synchronous path for RBAC decisions such as delegation validation and
/// demotion. It must be treated as authoritative over signature inference,
/// because role templates may share the same tool/operation shape.
static SESSION_ROLES: OnceLock<RwLock<HashMap<String, AgentRole>>> = OnceLock::new();

fn session_roles_map() -> &'static RwLock<HashMap<String, AgentRole>> {
    SESSION_ROLES.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Assign the RBAC role for a session.
///
/// LEGION-05: registering a role also lands the role's default permission
/// template into the session restrictions registry. `register_session_role`
/// and `restore_session_role_best_effort` (coordinator) both go through this
/// function, so this single chokepoint turns the role templates into the
/// session's effective tool runtime restrictions — previously the templates
/// were defined but never applied, and enforcement fell back to the
/// context-level profile for every session.
pub fn set_session_role(session_id: &str, role: AgentRole) -> BitFunResult<()> {
    session_roles_map()
        .write()
        .map_err(|e| BitFunError::tool(format!("Session role lock poisoned: {e}")))?
        .insert(session_id.to_string(), role.clone());
    update_restrictions(session_id, Some(role), ToolRuntimeRestrictionsPatch::default())
}

/// 注册角色并直接设置指定权限模板（不加载角色默认模板）。
///
/// P-01 方案 2：GeneralPurpose 子代理的角色仍是 Executor，但应用专属模板
/// （含 ReadOnly），覆盖默认 Executor 模板禁只读的设计缺口。
pub fn set_session_role_with_restrictions(
    session_id: &str,
    role: AgentRole,
    restrictions: ToolRuntimeRestrictions,
) -> BitFunResult<()> {
    session_roles_map()
        .write()
        .map_err(|e| BitFunError::tool(format!("Session role lock poisoned: {e}")))?
        .insert(session_id.to_string(), role.clone());
    session_restrictions_map()
        .write()
        .map_err(|e| BitFunError::tool(format!("Session restrictions lock poisoned: {e}")))?
        .insert(session_id.to_string(), restrictions);
    Ok(())
}

/// Retrieve the assigned RBAC role for a session, if any.
pub fn get_session_role(session_id: &str) -> Option<AgentRole> {
    session_roles_map()
        .read()
        .ok()
        .and_then(|map| map.get(session_id).cloned())
}

/// Remove the assigned RBAC role for a session (session-end cleanup).
///
/// Called when a session is deleted or discarded so a recycled session id
/// cannot inherit a stale role through the in-memory registry. Best-effort:
/// a poisoned lock only skips the removal, never blocks deletion. The
/// per-session restrictions are cleared too (LEGION-05) so a recycled id
/// cannot inherit a stale role template either.
pub fn clear_session_role(session_id: &str) {
    if let Ok(mut map) = session_roles_map().write() {
        map.remove(session_id);
    }
    clear_session_restrictions(session_id);
}

/// Validate a role-based delegation (R-14 B3).
///
/// The commander may delegate to any role; executor and reviewer sessions may
/// only delegate to their own role. An unknown creator (no registered role) is
/// treated as the permissive commander baseline so sessions outside the RBAC
/// registry are never blocked. Fails fast with a tool error — no retry, no
/// waiting, no human round-trip (R-15 hook rule).
pub fn validate_delegation(
    creator_role: Option<AgentRole>,
    target_role: AgentRole,
) -> BitFunResult<()> {
    match creator_role {
        None | Some(AgentRole::Commander) => Ok(()),
        Some(AgentRole::Executor) if target_role == AgentRole::Executor => Ok(()),
        Some(AgentRole::Reviewer) if target_role == AgentRole::Reviewer => Ok(()),
        Some(creator) => Err(BitFunError::tool(format!(
            "Delegation rejected: role '{}' may only delegate to '{}', not '{}'",
            creator.as_str(),
            creator.as_str(),
            target_role.as_str()
        ))),
    }
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
    let mut map = session_restrictions_map()
        .write()
        .map_err(|e| BitFunError::tool(format!("Session restrictions lock poisoned: {e}")))?;
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

/// Remove the session-specific tool restrictions (session-end cleanup).
///
/// Best-effort: a poisoned lock only skips the removal, never blocks deletion.
pub fn clear_session_restrictions(session_id: &str) {
    if let Ok(mut map) = session_restrictions_map().write() {
        map.remove(session_id);
    }
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
        assert!(
            permissions.allowed_tool_names.contains("SessionControl"),
            "Commander should allow SessionControl tool"
        );
        assert!(
            permissions.allowed_tool_names.contains("SessionMessage"),
            "Commander should allow SessionMessage tool"
        );
        assert!(
            permissions.allowed_tool_names.contains("SessionHistory"),
            "Commander should allow SessionHistory tool"
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
        // ReadOnly IS in the default Executor set (read code before acting).
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::ReadOnly),
            "Executor should allow ReadOnly (default allowed set)"
        );
    }

    #[test]
    fn reviewer_gets_writefile_and_executecode_like_executor() {
        let permissions = get_default_permissions(AgentRole::Reviewer);
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::WriteFile),
            "Reviewer should allow WriteFile"
        );
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::ExecuteCode),
            "Reviewer should allow ExecuteCode"
        );
        // ReadOnly IS in the Reviewer default set: reviewers read code and
        // reproduce findings (≈ Executor), they are not read-only shells.
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::ReadOnly),
            "Reviewer should allow ReadOnly (default allowed set)"
        );
    }

    #[test]
    fn session_role_registry_roundtrips() {
        let session_id = "test-session-role-registry-01";
        assert_eq!(get_session_role(session_id), None);
        set_session_role(session_id, AgentRole::Reviewer).expect("set role should succeed");
        assert_eq!(get_session_role(session_id), Some(AgentRole::Reviewer));
        // Reassignment overwrites.
        set_session_role(session_id, AgentRole::Commander).expect("set role should succeed");
        assert_eq!(get_session_role(session_id), Some(AgentRole::Commander));
    }

    #[test]
    fn session_role_registration_lands_role_template() {
        // LEGION-05: registering a role must land the role's default permission
        // template into the session restrictions, otherwise the templates are
        // dead config and enforcement silently falls back to the context-level
        // profile for every session.
        let session_id = "test-session-role-template-01";
        set_session_role(session_id, AgentRole::Commander).expect("set role should succeed");
        let restrictions = get_session_restrictions(session_id)
            .expect("role registration must land the template");
        assert!(
            restrictions
                .allowed_operation_classes
                .contains(&OperationClass::ReadOnly),
            "Commander template should include ReadOnly"
        );
        assert!(
            restrictions
                .allowed_operation_classes
                .contains(&OperationClass::Communicate),
            "Commander template should include Communicate"
        );
        assert!(
            !restrictions
                .allowed_operation_classes
                .contains(&OperationClass::WriteFile),
            "Commander template should not include WriteFile"
        );

        // Re-registering with a stricter role replaces the landed template.
        set_session_role(session_id, AgentRole::Executor).expect("reassign role should succeed");
        let restrictions = get_session_restrictions(session_id)
            .expect("re-registered role must re-land its template");
        assert!(
            restrictions
                .allowed_operation_classes
                .contains(&OperationClass::WriteFile),
            "Executor template should include WriteFile"
        );

        // Session-end cleanup clears both the role and the landed template so a
        // recycled session id cannot inherit stale restrictions.
        clear_session_role(session_id);
        assert_eq!(get_session_role(session_id), None, "role must be unregistered");
        assert_eq!(
            get_session_restrictions(session_id),
            None,
            "landed template must be cleared with the role"
        );
    }

    #[test]
    fn session_role_cleanup_removes_registry_entry() {
        let session_id = "test-session-role-cleanup-01";
        set_session_role(session_id, AgentRole::Executor).expect("set role should succeed");
        assert_eq!(get_session_role(session_id), Some(AgentRole::Executor));
        clear_session_role(session_id);
        assert_eq!(get_session_role(session_id), None, "role must be unregistered");
        // Clearing a missing entry is a no-op (idempotent).
        clear_session_role(session_id);
    }

    #[test]
    fn session_restrictions_cleanup_removes_registry_entry() {
        let session_id = "test-session-restrictions-cleanup-01";
        update_restrictions(session_id, None, ToolRuntimeRestrictionsPatch::default())
            .expect("set restrictions");
        assert!(
            get_session_restrictions(session_id).is_some(),
            "restrictions should be retrievable after update"
        );
        clear_session_restrictions(session_id);
        assert_eq!(
            get_session_restrictions(session_id),
            None,
            "restrictions must be unregistered"
        );
        // Clearing a missing entry is a no-op (idempotent).
        clear_session_restrictions(session_id);
    }

    #[test]
    fn delegation_validation_gates_executor_and_reviewer() {
        // Executor may only delegate to executor.
        assert!(validate_delegation(Some(AgentRole::Executor), AgentRole::Executor).is_ok());
        assert!(validate_delegation(Some(AgentRole::Executor), AgentRole::Commander).is_err());
        assert!(validate_delegation(Some(AgentRole::Executor), AgentRole::Reviewer).is_err());
        // Reviewer may only delegate to reviewer.
        assert!(validate_delegation(Some(AgentRole::Reviewer), AgentRole::Reviewer).is_ok());
        assert!(validate_delegation(Some(AgentRole::Reviewer), AgentRole::Executor).is_err());
        assert!(validate_delegation(Some(AgentRole::Reviewer), AgentRole::Commander).is_err());
        // Commander may delegate to any role.
        for role in [
            AgentRole::Commander,
            AgentRole::Executor,
            AgentRole::Reviewer,
            AgentRole::Warden,
            AgentRole::PunishmentExecutor,
        ] {
            assert!(
                validate_delegation(Some(AgentRole::Commander), role).is_ok(),
                "Commander should delegate to any role"
            );
        }
        // Unregistered creator degrades to the permissive commander baseline.
        assert!(validate_delegation(None, AgentRole::Commander).is_ok());
        assert!(validate_delegation(None, AgentRole::Executor).is_ok());
    }

    #[test]
    fn agent_role_str_key_roundtrips() {
        for role in [
            AgentRole::Commander,
            AgentRole::Executor,
            AgentRole::Reviewer,
            AgentRole::Warden,
            AgentRole::PunishmentExecutor,
        ] {
            let key = role.as_str();
            let parsed = AgentRole::from_str_key(key);
            assert_eq!(
                parsed.as_ref(),
                Some(&role),
                "key {key:?} should roundtrip to {role:?}"
            );
        }
        // Unknown keys degrade to None (stale metadata => permissive baseline),
        // never to an error or a mis-mapped role.
        assert_eq!(AgentRole::from_str_key("commander-v2"), None);
        assert_eq!(AgentRole::from_str_key(""), None);
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
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::WriteFile),
            "Warden should allow WriteFile for audit report landing"
        );
        assert!(
            permissions.allowed_tool_names.contains("Write"),
            "Warden should allow Write tool for audit report landing"
        );
        assert!(
            permissions.allowed_tool_names.contains("Edit"),
            "Warden should allow Edit tool for audit report landing"
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
            permissions
                .path_policy
                .write_roots
                .contains(&SHAME_WALL_FILENAME.to_string()),
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

        let stored =
            get_session_restrictions(session_id).expect("session restrictions should exist");

        // apply_patch replaces the field entirely when Some, so after the patch
        // allowed_operation_classes = {ReadOnly}, replacing the Executor
        // baseline {WriteFile, ExecuteCode} rather than extending it.
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
        assert!(
            !stored
                .allowed_operation_classes
                .contains(&OperationClass::ExecuteCode),
            "Patch replaced operation classes, ExecuteCode should be gone"
        );
    }
}
