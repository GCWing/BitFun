use crate::agentic::agents::subagent_default_tools;
use crate::agentic::warden::{SHAME_WALL_FILENAME, WARDEN_AUDIT_WRITE_ROOT};
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
    /// Punishment executor: Write (shame-wall) + SessionControl
    ///
    /// P2-S1: under R-25 (reminder-only discipline) the SessionControl
    /// allowlist entry is effectively list/inspect-only:
    /// - `list` needs no target scope (summary-only, no content).
    /// - `create` registers a new session under the caller tree (delegation
    ///   validated, inherited role).
    /// - `cancel`/`delete` still pass `resolve_session_mutation_authorization`
    ///   (owner/created-by/ancestor gate) before touching any target session.
    /// There is deliberately no freeze/role-change surface (R-25 removed it).
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
    // Allowed operation classes: ReadOnly + Communicate + WriteFile + DeleteFile + ExecuteCode
    // （全工具语义对齐：Commander 主会话 = 全工具执行者，工具已全量白名单，
    //   操作类必须同步全量——Write/Edit=WriteFile、Delete=DeleteFile、
    //   ExecCommand/GetToolSpec/CallDeferredTool=ExecuteCode，否则工具进了
    //   白名单仍被 ensure_operation_allowed 拦截。）
    // Allowed tool names: agentic 全工具（subagent_default_tools() 单源同步，
    // 与 GeneralPurpose 模板同源）——Commander 主会话 = 全工具执行者，
    // 根治「窄白名单系统性缺口」（TodoWrite/Grep/Glob/GetTime/GetToolSpec 等
    // 逐个补永远差一个）。再叠加 ACP 工具族（外部进程桥）与 deferred 工具链
    // 核心（GetToolSpec/CallDeferredTool 解锁全部 deferred 工具）。
    {
        let mut allowed_ops = BTreeSet::new();
        allowed_ops.insert(OperationClass::ReadOnly);
        allowed_ops.insert(OperationClass::Communicate);
        allowed_ops.insert(OperationClass::WriteFile);
        allowed_ops.insert(OperationClass::DeleteFile);
        allowed_ops.insert(OperationClass::ExecuteCode);
        let mut allowed_tools = BTreeSet::new();
        for name in subagent_default_tools() {
            allowed_tools.insert(name);
        }
        // Dedicated ACP tool family mirrors the Session toolset over the real
        // external ACP process channel (true bridge).
        allowed_tools.insert("acp_control".to_string());
        allowed_tools.insert("acp_message".to_string());
        allowed_tools.insert("acp_history".to_string());
        // Deferred 工具链核心：GetToolSpec/CallDeferredTool 不在
        // subagent_default_tools()，但缺失会导致全部 deferred 工具
        // （SessionControl/SessionMessage/Git/Plan 等）无法解锁。
        allowed_tools.insert("GetToolSpec".to_string());
        allowed_tools.insert("CallDeferredTool".to_string());
        // GetTime 不在 subagent_default_tools()，但主会话常用（时间/日期事实，
        // 无参数只读），缺失会被 ensure_tool_allowed 拦截。
        allowed_tools.insert("GetTime".to_string());
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
    // Allowed operation classes: ReadOnly + WriteFile + DeleteFile + ExecuteCode + Communicate
    // （执行者读代码基本能力：Read/Write/Edit/Delete/ExecCommand 配齐；
    //   DeleteFile 必须放行，否则 Delete 工具会被 ensure_operation_allowed 拦截；
    //   Communicate 放行（执行者全工具对齐 agentic）——TodoWrite/
    //   SessionMessage/SessionControl/LegionControl 等会话内通信与任务跟踪
    //   工具归类 Communicate（framework.rs classify_tool_call），缺失会被
    //   ensure_operation_allowed 拦截，导致执行者无法建任务清单/协调子会话）
    // 显式白名单（P1-S1 安全收敛）：Executor 模板不再依赖"白名单空 = 全放行"。
    // 工具白名单 = subagent_default_tools()（agentic 全工具，单源同步）∪
    //   GetToolSpec/CallDeferredTool（deferred 工具链解锁）∪ GetTime +
    //   review 核心工具 GetFileDiff/submit_code_review（review 形态
    //   CodeReview/DeepReview/ReviewWorker/ReviewJudge 走默认 Executor 模板，
    //   白名单缺这两个会让审查流程不可用）。
    // 注意：merge subagent_tool_restrictions()（与 GeneralPurpose 专属模板一致）——
    // 所有 Executor 子代理（含非 GeneralPurpose/agentic 形态：CodeReview/
    // DeepReview/Explore/FileFinder/ResearchSpecialist 等）必须带 subagent deny
    // list（ControlHub/GenerativeUI/ReviewPlatform/MiniApp 生命周期/AgentWait），
    // 否则 session_override 优先时 deny 被绕过（ReviewPlatform 已进 review 全家桶
    // default_tools，实际可触达 = 安全边界缺口）。白名单 + deny 双保险：
    // 新增工具默认不在白名单 → 子代理侧默认禁止（与 MiniApp 白名单哲学对齐）。
    {
        let mut allowed_ops = BTreeSet::new();
        allowed_ops.insert(OperationClass::ReadOnly);
        allowed_ops.insert(OperationClass::WriteFile);
        allowed_ops.insert(OperationClass::DeleteFile);
        allowed_ops.insert(OperationClass::ExecuteCode);
        allowed_ops.insert(OperationClass::Communicate);
        let mut allowed_tools = BTreeSet::new();
        for name in subagent_default_tools() {
            allowed_tools.insert(name);
        }
        // Deferred 工具链核心：GetToolSpec/CallDeferredTool 不在
        // subagent_default_tools()，但缺失会导致全部 deferred 工具
        // （SessionControl/SessionMessage/Git/Plan 等）无法解锁——与
        // Commander 模板同源补充（执行者形态同样需要 deferred 解锁）。
        allowed_tools.insert("GetToolSpec".to_string());
        allowed_tools.insert("CallDeferredTool".to_string());
        // GetTime 不在 subagent_default_tools()，但执行者常用（时间/日期事实，
        // 无参数只读），与 Commander 模板同源。
        allowed_tools.insert("GetTime".to_string());
        // review 核心工具（形态分流：review 形态不命中 is_executor_agent_type，
        // 走默认 Executor 模板；白名单必须显式包含，否则 GetFileDiff/
        // submit_code_review 被 ensure_tool_allowed 拦截，DeepReview 流程不可用）。
        allowed_tools.insert("GetFileDiff".to_string());
        allowed_tools.insert("submit_code_review".to_string());
        // review/探索形态附加只读工具（不在 subagent_default_tools() 内）：
        // LaunchReviewAgent（review 编排入口，deferred）+ LS（目录形态只读）。
        allowed_tools.insert("LaunchReviewAgent".to_string());
        allowed_tools.insert("LS".to_string());
        let mut restrictions = ToolRuntimeRestrictions {
            allowed_operation_classes: allowed_ops,
            allowed_tool_names: allowed_tools,
            ..Default::default()
        };
        restrictions.merge(&subagent_tool_restrictions());
        map.insert(AgentRole::Executor, restrictions);
    }

    // ── Reviewer ───────────────────────────────────────────────────
    // Allowed operation classes: ReadOnly + WriteFile + ExecuteCode (≈ Executor).
    // （审查官读代码审查 + 落盘审查报告：Read/Write/Edit/ExecCommand 三件套配齐）
    // Reviewers must be able to inspect and reproduce findings; the signature
    // now intentionally overlaps Executor, so role identity must come from the
    // persisted session role (SESSION_ROLES), never from template inference.
    // 显式白名单（P1-S1 安全收敛）：与 Executor 同源（subagent_default_tools()
    // ∪ 专有），新增工具默认禁止；deny 双保险（subagent_tool_restrictions）。
    {
        let mut allowed_ops = BTreeSet::new();
        allowed_ops.insert(OperationClass::ReadOnly);
        allowed_ops.insert(OperationClass::WriteFile);
        allowed_ops.insert(OperationClass::ExecuteCode);
        let mut allowed_tools = BTreeSet::new();
        for name in subagent_default_tools() {
            allowed_tools.insert(name);
        }
        // review 核心工具（GetFileDiff/submit_code_review 不在共享工具集）。
        allowed_tools.insert("GetFileDiff".to_string());
        allowed_tools.insert("submit_code_review".to_string());
        // review/探索形态附加只读工具（与 Executor 同源）。
        allowed_tools.insert("LaunchReviewAgent".to_string());
        allowed_tools.insert("LS".to_string());
        // Deferred 工具链核心（与 Executor/Commander 同源）。
        allowed_tools.insert("GetToolSpec".to_string());
        allowed_tools.insert("CallDeferredTool".to_string());
        allowed_tools.insert("GetTime".to_string());
        let mut restrictions = ToolRuntimeRestrictions {
            allowed_operation_classes: allowed_ops,
            allowed_tool_names: allowed_tools,
            ..Default::default()
        };
        restrictions.merge(&subagent_tool_restrictions());
        map.insert(AgentRole::Reviewer, restrictions);
    }

    // ── Warden ─────────────────────────────────────────────────────
    // Allowed operation classes: ReadOnly + WriteFile + Communicate + ExecuteCode
    // （守卫审计也需读/落盘：Read/Write/Edit/ExecCommand 三件套配齐）
    // Allowed tool names: SessionHistory (extra, for cross-session inspection),
    //                     ExecCommand (for gbrain search/query across full knowledge base),
    //                     Write/Edit (audit report landing)
    // P2-S2 纵深收敛：Write/Edit 落盘收敛到审计目录（.bitfun/warden/ 写根，
    // 与 SHAME_WALL_FILENAME 同族；相对路径经 workspace runtime root 解析，
    // 绝对路径经 resolve_tool_path 解析后仍须落在写根内）——提示注入即使
    // 拿到 Write/Edit 也只能写审计目录，不能写任意文件。ExecCommand 保留
    // （gbrain 知识库查询是 Warden 审计能力的一部分），其 ExecuteCode 面由
    // 写根收敛 + Warden 会话为 daemon 白名单形态双重约束。
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
        let path_policy = ToolPathPolicy {
            write_roots: vec![WARDEN_AUDIT_WRITE_ROOT.to_string()],
            edit_roots: vec![WARDEN_AUDIT_WRITE_ROOT.to_string()],
            ..Default::default()
        };
        map.insert(
            AgentRole::Warden,
            ToolRuntimeRestrictions {
                allowed_operation_classes: allowed_ops,
                allowed_tool_names: allowed_tools,
                path_policy,
                ..Default::default()
            },
        );
    }

    // ── PunishmentExecutor ─────────────────────────────────────────
    // Allowed tool names: Write (path-policy restricted to
    //                     ~/.bitfun/warden/violation-registry.json via SHAME_WALL_FILENAME),
    //                     SessionControl (list/inspect scope, P2-S1)
    // P2-S1 范围约束文档（R-25 reminder-only 纪律下实际仅 list/inspect）：
    //   - list：无需目标会话范围（仅摘要，不含内容）；
    //   - create：在调用者树内注册新会话（委托校验 + 继承角色）；
    //   - cancel/delete：仍过 resolve_session_mutation_authorization
    //     （owner/created-by/祖先授权门）才可触碰目标会话；
    //   - 无 freeze/role-change 面（R-25 已移除）。
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
/// 只读类。专属模板允许全部操作类（ReadOnly/WriteFile/DeleteFile/
/// ExecuteCode/Communicate），工具白名单与 subagent_default_tools()
/// （agentic 全工具 + SessionControl）保持单一来源同步，确保执行者
/// 模板加全工具后运行时不被白名单拦掉。通用 subagent deny 列表
/// （subagent_tool_restrictions：ControlHub/GenerativeUI/ReviewPlatform/
/// MiniApp 生命周期等）由 coordinator 在会话创建时 merge，仍然生效。
pub fn general_purpose_tool_restrictions() -> ToolRuntimeRestrictions {
    let mut allowed_ops = BTreeSet::new();
    allowed_ops.insert(OperationClass::ReadOnly);
    allowed_ops.insert(OperationClass::WriteFile);
    allowed_ops.insert(OperationClass::DeleteFile);
    allowed_ops.insert(OperationClass::ExecuteCode);
    allowed_ops.insert(OperationClass::Communicate);
    let mut allowed_tools = BTreeSet::new();
    for name in subagent_default_tools() {
        allowed_tools.insert(name);
    }
    // Deferred 工具链核心：GetToolSpec/CallDeferredTool 不在
    // subagent_default_tools()，但缺失会导致全部 deferred 工具
    // （SessionControl/SessionMessage/Git/Plan 等）无法解锁——与
    // Commander 模板同源补充（执行者形态同样需要 deferred 解锁）。
    allowed_tools.insert("GetToolSpec".to_string());
    allowed_tools.insert("CallDeferredTool".to_string());
    // GetTime 不在 subagent_default_tools()，但主会话/执行者常用
    // （时间/日期事实，无参数只读）。
    allowed_tools.insert("GetTime".to_string());
    let mut restrictions = ToolRuntimeRestrictions {
        allowed_operation_classes: allowed_ops,
        allowed_tool_names: allowed_tools,
        ..Default::default()
    };
    // GeneralPurpose 会话 restore 时通过 set_session_role_with_restrictions
    // 直接注册专属模板（coordinator restore_session_role_best_effort），
    // session override 会优先于 context 级限制，因此必须在此把
    // 通用 subagent deny 列表（ControlHub/GenerativeUI/ReviewPlatform/
    // MiniApp 生命周期/AgentWait）merge 进来，防止全工具白名单绕过
    // subagent 安全边界。
    restrictions.merge(&subagent_tool_restrictions());
    restrictions
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
/// Registering a role also lands the role's default permission
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
/// （含 ReadOnly），覆盖默认 Executor 模板禁只读的设计缺口。由 coordinator
/// restore_session_role_best_effort 在 GeneralPurpose 会话 restore 时调用。
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

/// 注册主会话角色而不落角色默认模板（R3 主会话豁免）。
///
/// 主会话（Standard 类型且无 creator）是终端用户的主流程会话。若像
/// `set_session_role` 那样把 Commander 模板写入 SESSION_RESTRICTIONS，
/// 默认配置下主会话的 Read/Grep/Glob/Edit/ExecCommand 会被
/// `allowed_tool_names` 白名单拒绝，构成主流程严重回归。本函数只记录角色
/// （owner 语义与委托校验依赖 `get_session_role`，不受影响），不写限制模板
/// —— 会话保持上下文级默认限制（白名单空 = 全工具放行）。
pub fn register_main_session(session_id: &str, role: AgentRole) -> BitFunResult<()> {
    session_roles_map()
        .write()
        .map_err(|e| BitFunError::tool(format!("Session role lock poisoned: {e}")))?
        .insert(session_id.to_string(), role);
    Ok(())
}

/// 判定会话是否为"主会话"（Standard 类型且无 creator）。
///
/// 主会话是终端用户直接发起的主流程会话，非任何子代理/委派工作。
/// 子代理（Subagent/EphemeralSubagent）或有 creator 的会话不属于此类，
/// 继续走完整 RBAC 角色模板注册。
pub(crate) fn is_main_session(kind: crate::agentic::core::SessionKind, created_by: Option<&str>) -> bool {
    kind == crate::agentic::core::SessionKind::Standard && created_by.is_none()
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
/// per-session restrictions are cleared too so a recycled id
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
///
/// # Warden / PunishmentExecutor (d1-P2-7)
///
/// These two roles can **never** delegate: the match arm `Some(creator) =>`
/// rejects every target role for them. This asymmetry with Commander is
/// deliberate — Warden and PunishmentExecutor are system roles owned by the
/// warden runtime (see [`WARDEN_RUNTIME_SESSION`]) and must not spawn
/// delegated subagent work; allowing them to create sessions would give a
/// discipline/sanctions surface a second way to materialize sessions. The
/// warden runtime requests penalties through the internal trusted marker, not
/// through a role-based delegation call, so no legitimate path is blocked by
/// this rejection.
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
        // UX-P0-1 收窄：SessionHistory 移出共享工具集（Commander 模板派生自
        // subagent_default_tools()），跨会话 transcript 读取仅 Warden 模板
        // 显式授予 + 工具内授权门兜底。Commander 主会话经 UI/前端历史视图
        // 读取，不走该工具。
        assert!(
            !permissions.allowed_tool_names.contains("SessionHistory"),
            "Commander should NOT allow SessionHistory tool (UX-P0-1 narrow)"
        );
        // 全工具语义（Commander 主会话 = 全工具执行者）：操作类与
        // 工具白名单同步全量，WriteFile/DeleteFile/ExecuteCode 均允许。
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::WriteFile),
            "Commander should allow WriteFile (全工具语义)"
        );
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::DeleteFile),
            "Commander should allow DeleteFile (全工具语义)"
        );
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::ExecuteCode),
            "Commander should allow ExecuteCode (全工具语义)"
        );
        assert!(
            permissions
                .allowed_tool_names
                .contains("GetToolSpec"),
            "Commander should allow GetToolSpec (deferred 工具链解锁)"
        );
        assert!(
            permissions
                .allowed_tool_names
                .contains("CallDeferredTool"),
            "Commander should allow CallDeferredTool (deferred 工具链执行)"
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
        // DeleteFile IS in the default Executor set: executor subagents
        // (GeneralPurpose) run the full agentic tool suite including Delete.
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::DeleteFile),
            "Executor should allow DeleteFile (GeneralPurpose runs full agentic tool suite)"
        );
        // ensure_operation_allowed must accept the Delete tool classification.
        assert!(
            permissions
                .ensure_operation_allowed(OperationClass::DeleteFile, "Delete")
                .is_ok(),
            "Executor must pass ensure_operation_allowed for Delete tool"
        );
        // Communicate IS in the default Executor set (执行者全工具
        // 对齐 agentic): TodoWrite/SessionMessage/SessionControl/LegionControl
        // 归类 Communicate，缺失会被 ensure_operation_allowed 拦截。
        assert!(
            permissions
                .allowed_operation_classes
                .contains(&OperationClass::Communicate),
            "Executor should allow Communicate (full agentic tool suite)"
        );
        assert!(
            permissions
                .ensure_operation_allowed(OperationClass::Communicate, "TodoWrite")
                .is_ok(),
            "Executor must pass ensure_operation_allowed for TodoWrite tool"
        );
    }

    #[test]
    fn executor_template_merges_subagent_deny_list() {
        // P1：默认 Executor 模板必须 merge subagent_tool_restrictions()——
        // 非 GeneralPurpose/agentic 的 Executor 子代理（CodeReview/DeepReview/
        // Explore/FileFinder 等）走默认模板，若 deny list 缺失则 session_override
        // 优先时 ReviewPlatform（已进 review 全家桶 default_tools）可被触达，
        // 安全边界被绕过。
        let permissions = get_default_permissions(AgentRole::Executor);
        assert!(
            permissions.ensure_tool_allowed("ReviewPlatform").is_err(),
            "Executor template must deny ReviewPlatform (subagent deny list)"
        );
        assert!(
            permissions.ensure_tool_allowed("ControlHub").is_err(),
            "Executor template must deny ControlHub (subagent deny list)"
        );
        assert!(
            permissions.ensure_tool_allowed("GenerativeUI").is_err(),
            "Executor template must deny GenerativeUI (subagent deny list)"
        );
        assert!(
            permissions.ensure_tool_allowed("InitMiniApp").is_err(),
            "Executor template must deny InitMiniApp (subagent deny list)"
        );
        // deny 不误伤正常执行者能力。
        assert!(
            permissions.ensure_tool_allowed("ExecCommand").is_ok(),
            "Executor template must still allow ExecCommand"
        );
        assert!(
            permissions.ensure_tool_allowed("Read").is_ok(),
            "Executor template must still allow Read"
        );
    }

    #[test]
    fn default_executor_template_review_shapes_keep_review_tools_visible() {
        // 形态分流（P2 回归修复）：review 形态（CodeReview/DeepReview/
        // ReviewWorker/ReviewJudge/ReviewFixer）不命中 is_executor_agent_type →
        // 走默认 Executor 模板。默认模板**显式白名单**（P1-S1 收敛）必须包含
        // review 核心工具 GetFileDiff/submit_code_review（模型可见 + 可调用），
        // 同时 P1 的 deny list merge 保证 ReviewPlatform 仍被拦截（安全边界保留）。
        let permissions = get_default_permissions(AgentRole::Executor);
        assert!(
            !permissions.allowed_tool_names.is_empty(),
            "默认 Executor 模板白名单必须非空（P1-S1：空 = 全放行已废除）"
        );
        assert!(
            permissions.ensure_tool_allowed("GetFileDiff").is_ok(),
            "review 形态 GetFileDiff 必须可见可用（显式白名单包含）"
        );
        assert!(
            permissions.ensure_tool_allowed("submit_code_review").is_ok(),
            "review 形态 submit_code_review 必须可见可用（显式白名单包含）"
        );
        assert!(
            permissions.ensure_tool_allowed("ReviewPlatform").is_err(),
            "review 形态 ReviewPlatform 仍被 deny（P1 deny list 生效）"
        );
        assert!(
            permissions.ensure_tool_allowed("ControlHub").is_err(),
            "Executor 模板 ControlHub 必须被 deny"
        );
        assert!(
            permissions.ensure_tool_allowed("GenerativeUI").is_err(),
            "Executor 模板 GenerativeUI 必须被 deny"
        );
        assert!(
            permissions.ensure_tool_allowed("InitMiniApp").is_err(),
            "Executor 模板 InitMiniApp 必须被 deny"
        );
        assert!(
            permissions.ensure_tool_allowed("ExecCommand").is_ok(),
            "Executor 模板必须允许 ExecCommand"
        );
        assert!(
            permissions.ensure_tool_allowed("Read").is_ok(),
            "Executor 模板必须允许 Read"
        );
    }

    #[test]
    fn executor_and_reviewer_templates_deny_new_tools_by_default() {
        // P1-S1 回归：显式白名单 = 新增工具默认禁止（与 MiniApp 白名单
        // 「默认关闭」哲学对齐）。任何不在 subagent_default_tools() ∪ 专有
        // 补充集的工具名都必须在 Executor/Reviewer 模板上被拒绝。
        let unknown_tool = "FutureNewAgenticTool";
        let executor = get_default_permissions(AgentRole::Executor);
        assert!(
            executor.ensure_tool_allowed(unknown_tool).is_err(),
            "Executor 模板必须拒绝不在白名单的新增工具"
        );
        let reviewer = get_default_permissions(AgentRole::Reviewer);
        assert!(
            reviewer.ensure_tool_allowed(unknown_tool).is_err(),
            "Reviewer 模板必须拒绝不在白名单的新增工具"
        );
        // 白名单与 deny 双保险：即便未来有人把新工具加进白名单，
        // deny 面（subagent deny 列表）仍必须把高危宿主面关死。
        for permissions in [executor, reviewer] {
            for denied in [
                "ControlHub",
                "GenerativeUI",
                "ReviewPlatform",
                "InitMiniApp",
                "FinalizeMiniApp",
                "PublishMiniApp",
                "PageDeploy",
                "PagePublish",
                "AgentWait",
            ] {
                assert!(
                    permissions.ensure_tool_allowed(denied).is_err(),
                    "{denied} 必须在 Executor/Reviewer 模板被 deny"
                );
            }
        }
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
    fn main_session_registration_records_role_without_landing_template() {
        // R3 主会话豁免：register_main_session 只记录角色、不写限制模板，
        // 因此 get_session_restrictions 为空（强制回落到上下文级默认限制 =
        // 白名单空 = 全工具放行），主会话不会被 Commander 模板锁死。
        let session_id = "test-main-session-exempt-01";
        register_main_session(session_id, AgentRole::Commander).expect("register main session");
        assert_eq!(
            get_session_role(session_id),
            Some(AgentRole::Commander),
            "main session role must be recorded (owner/delegation semantics intact)"
        );
        assert_eq!(
            get_session_restrictions(session_id),
            None,
            "main session must NOT land the Commander default template"
        );

        // 无会话级限制时，默认限制（全放行）让主流程工具通过。
        let unrestricted = ToolRuntimeRestrictions::default();
        assert!(
            unrestricted.ensure_tool_allowed("Read").is_ok()
                && unrestricted.ensure_tool_allowed("Edit").is_ok()
                && unrestricted.ensure_tool_allowed("ExecCommand").is_ok()
                && unrestricted.ensure_tool_allowed("Grep").is_ok()
                && unrestricted.ensure_tool_allowed("Glob").is_ok(),
            "default (empty) restrictions must allow main-flow tools"
        );

        // 会话结束清理：同时清除角色与（此处缺席的）模板。
        clear_session_role(session_id);
        assert_eq!(get_session_role(session_id), None);
        assert_eq!(get_session_restrictions(session_id), None);
    }

    #[test]
    fn is_main_session_matches_standard_without_creator_only() {
        use crate::agentic::core::SessionKind;
        // 主会话：Standard 且无 creator。
        assert!(is_main_session(SessionKind::Standard, None));
        // 子代理 / 有 creator 的会话不是主会话，必须走完整 RBAC 模板注册。
        assert!(!is_main_session(SessionKind::Subagent, None));
        assert!(!is_main_session(SessionKind::EphemeralSubagent, None));
        assert!(!is_main_session(SessionKind::Standard, Some("parent-session")));
        assert!(!is_main_session(SessionKind::Subagent, Some("parent-session")));
    }

    #[test]
    fn session_role_registration_lands_role_template() {
        // Registering a role must land the role's default permission
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
            restrictions.ensure_tool_allowed("TodoWrite").is_ok(),
            "Commander template must allow TodoWrite (role task tracking)"
        );
        assert!(
            restrictions.ensure_tool_allowed("Grep").is_ok(),
            "Commander template must allow Grep (role search capability)"
        );
        assert!(
            restrictions.ensure_tool_allowed("Glob").is_ok(),
            "Commander template must allow Glob (role search capability)"
        );
        assert!(
            restrictions.ensure_tool_allowed("GetTime").is_ok(),
            "Commander template must allow GetTime (主会话基础工具)"
        );
        // Deferred 工具链核心：GetToolSpec/CallDeferredTool 解锁全部 deferred
        // 工具（SessionControl/SessionMessage/Git/Plan 等）——缺失则无法解锁。
        assert!(
            restrictions.ensure_tool_allowed("GetToolSpec").is_ok(),
            "Commander template must allow GetToolSpec (deferred 工具链解锁)"
        );
        assert!(
            restrictions.ensure_tool_allowed("CallDeferredTool").is_ok(),
            "Commander template must allow CallDeferredTool (deferred 工具链执行)"
        );
        // 全工具语义：Commander 主会话 = 全工具执行者，操作类与工具白名单
        // 同步全量（Write/Edit=WriteFile、Delete=DeleteFile、
        // ExecCommand/GetToolSpec/CallDeferredTool=ExecuteCode）。
        assert!(
            restrictions
                .allowed_operation_classes
                .contains(&OperationClass::WriteFile),
            "Commander template should include WriteFile (全工具语义)"
        );
        assert!(
            restrictions
                .allowed_operation_classes
                .contains(&OperationClass::DeleteFile),
            "Commander template should include DeleteFile (全工具语义)"
        );
        assert!(
            restrictions
                .allowed_operation_classes
                .contains(&OperationClass::ExecuteCode),
            "Commander template should include ExecuteCode (全工具语义)"
        );
        // GetToolSpec 归类 ExecuteCode——操作类放行后解锁链不再被拦截。
        assert!(
            restrictions
                .ensure_operation_allowed(
                    bitfun_agent_tools::classify_tool_call(
                        "GetToolSpec",
                        &serde_json::json!({})
                    ),
                    "GetToolSpec"
                )
                .is_ok(),
            "Commander must pass operation-class check for GetToolSpec"
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
    fn delegation_validation_rejects_warden_and_punishment_executor_creators() {
        // Warden/PunishmentExecutor are system roles and must never delegate
        // (d1-P2-7): no target role is accepted from these creators, unlike
        // the commander's permissive baseline. This locks the deliberate
        // asymmetry into the contract.
        for creator in [AgentRole::Warden, AgentRole::PunishmentExecutor] {
            for target in [
                AgentRole::Commander,
                AgentRole::Executor,
                AgentRole::Reviewer,
                AgentRole::Warden,
                AgentRole::PunishmentExecutor,
            ] {
                assert!(
                    validate_delegation(Some(creator.clone()), target.clone()).is_err(),
                    "{creator:?} must never delegate to {target:?}"
                );
            }
        }
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
        // P2-S2: Write/Edit path_policy restricted to the warden audit write root.
        assert!(
            permissions
                .path_policy
                .write_roots
                .contains(&WARDEN_AUDIT_WRITE_ROOT.to_string()),
            "Warden write_roots should contain {}",
            WARDEN_AUDIT_WRITE_ROOT
        );
        assert!(
            permissions
                .path_policy
                .edit_roots
                .contains(&WARDEN_AUDIT_WRITE_ROOT.to_string()),
            "Warden edit_roots should contain {}",
            WARDEN_AUDIT_WRITE_ROOT
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
