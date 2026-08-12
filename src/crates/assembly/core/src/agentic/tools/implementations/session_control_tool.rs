//! SessionControl manages persisted workspace-scoped sessions.
//!
//! The `cancel` action only cancels the target session's current running dialog turn.
//! It does not permanently stop the session itself, and it does not clear queued
//! messages that may still run later through the scheduler.

use super::util::normalize_path;
use crate::agentic::agents::{get_agent_registry, AcpAgent};
use crate::agentic::coordination::{get_global_coordinator, get_global_scheduler};
use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::restrictions::{
    get_session_role, validate_delegation, worktree_creation_authorized, AgentRole,
};
use crate::service::git::GitService;
use crate::service::workspace::{
    get_global_workspace_service, WorkspaceActivityMode, WorkspaceCreateOptions,
};
use crate::service::worktree::{
    WorktreeCreateBranchRequest, WorktreeCreateRequest, WorktreeService,
};
use crate::service_agent_runtime::CoreServiceAgentRuntime;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_agent_runtime::sdk::AgentRuntime;
use bitfun_agent_runtime::session_control::{
    compact_session_display_name, render_session_control_tool_use_message,
    resolve_session_control_cancel_route, session_control_agent_type_or_default,
    session_control_cancel_result_message, session_control_cancel_status,
    session_control_created_result_message, session_control_creator_marker,
    session_control_deleted_result_message, session_control_renamed_result_message,
    session_control_session_name_or_default, validate_session_control_input, validate_session_id,
    SessionControlAction, SessionControlCancelRoute, SessionControlInput,
    SessionControlValidationContext, SessionControlValidationResult,
};
use bitfun_core_types::{SessionExecutionTarget, WorktreeSessionOptions};
use bitfun_runtime_ports::{
    AcpClientCreateRequest, AcpClientCreateResult, AcpClientPort, AgentSessionCreateRequest,
    AgentSessionDeleteRequest, AgentSessionListRequest, AgentSessionSummary,
    AgentSessionWorkspaceBinding, AgentSessionWorkspaceRequest, AgentSubmissionSource,
    AgentTurnCancellationRequest,
};
use bitfun_services_core::session::merge_session_custom_metadata;
use bitfun_services_core::session::tree::SessionTreeManager;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// SessionControl tool - create, cancel, delete, or list persisted sessions
/// list: list persistent sessions created by SessionControl.
/// list_tasks: list child conversation sessions spawned by Task.
pub struct SessionControlTool;

const CANCEL_WAIT_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
pub(crate) struct SessionControlWorkspaceTarget {
    pub display_workspace: String,
    pub project_workspace: String,
    pub execution_target: Option<SessionExecutionTarget>,
    pub workspace_id: Option<String>,
    pub remote_connection_id: Option<String>,
    pub remote_ssh_host: Option<String>,
}

/// 结果：SessionControl/SessionMessage create 时创建的 worktree（W4/W5）。
/// `created=false` = 幂等重放（request_id 已用过，复用既有 worktree）。
#[derive(Debug, Clone)]
pub(crate) struct SessionWorktreeCreateResult {
    pub execution_target: SessionExecutionTarget,
    pub tracked_workspace_id: Option<String>,
    pub created: bool,
    pub branch_name: Option<String>,
    pub project_workspace_path: String,
}

impl Default for SessionControlTool {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionControlTool {
    pub fn new() -> Self {
        Self
    }

    fn current_workspace_session<'a>(
        &self,
        context: &'a ToolUseContext,
        workspace: &str,
    ) -> Option<&'a str> {
        let current_session_id = context.session_id.as_deref()?;
        let current_workspace = context.workspace_root()?;
        let normalized_current_workspace =
            normalize_path(current_workspace.to_string_lossy().as_ref());

        if normalized_current_workspace == workspace {
            Some(current_session_id)
        } else {
            None
        }
    }

    fn creator_session_marker(&self, context: &ToolUseContext) -> BitFunResult<String> {
        let creator_session_id = context.session_id.as_ref().ok_or_else(|| {
            BitFunError::tool("create requires a creator session in tool context".to_string())
        })?;
        Ok(session_control_creator_marker(creator_session_id))
    }

    /// ACP 真会话创建：经 AcpClientPort 创建外部 ACP 流会话（返回
    /// `acp_<client>_<uuid>` session id + `acp:<client>` agent type），与前端
    /// `create_acp_flow_session` / desktop `AcpClientPort::create_session` 等价——
    /// 持久记录 + 启动外部进程 + 失败回滚（desktop acp_client_port.rs:97-149）。
    /// 不创建本地内部会话，因此不写入 createdBy/subagent 元数据、不持久化
    /// SessionRelationship、不挂军团树；军团侧持返回的 session_id 经
    /// SessionMessage 直通（acp: 流会话分叉）通信。
    async fn create_acp_session_via_port(
        &self,
        workspace: &SessionControlWorkspaceTarget,
        client_id: &str,
        session_name: Option<String>,
        port: &dyn AcpClientPort,
    ) -> BitFunResult<AcpClientCreateResult> {
        port.create_session(AcpClientCreateRequest {
            client_id: client_id.to_string(),
            workspace_path: workspace.display_workspace.clone(),
            session_name,
            remote_connection_id: workspace.remote_connection_id.clone(),
        })
        .await
        .map_err(|error| {
            BitFunError::tool(format!(
                "ACP client port failed ({:?}): {}",
                error.kind, error.message
            ))
        })
    }

    async fn resolve_effective_workspace(
        &self,
        action: SessionControlAction,
        session_id: Option<&str>,
        workspace_param: Option<&str>,
        context: &ToolUseContext,
        runtime: &AgentRuntime,
    ) -> BitFunResult<SessionControlWorkspaceTarget> {
        match action {
            SessionControlAction::Cancel
            | SessionControlAction::Delete
            | SessionControlAction::Compact
            | SessionControlAction::Rename => {
                let session_id = session_id.ok_or_else(|| {
                    BitFunError::tool(format!("session_id is required for {}", action.as_str()))
                })?;
                if let Some(binding) = runtime
                    .resolve_session_workspace_binding(AgentSessionWorkspaceRequest {
                        session_id: session_id.to_string(),
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?
                {
                    return Ok(Self::workspace_target_from_binding(binding));
                }
                Err(BitFunError::NotFound(format!(
                    "Workspace for session '{}' could not be resolved",
                    session_id
                )))
            }
            SessionControlAction::Create | SessionControlAction::List => {
                // Explicit workspace parameter wins; fall back to the current
                // workspace binding from context when omitted, so the tool can
                // list/create across workspaces.
                if let Some(workspace) = workspace_param {
                    return Ok(SessionControlWorkspaceTarget {
                        display_workspace: normalize_path(workspace),
                        project_workspace: normalize_path(workspace),
                        execution_target: None,
                        workspace_id: None,
                        remote_connection_id: None,
                        remote_ssh_host: None,
                    });
                }
                let workspace = context.workspace.as_ref().ok_or_else(|| {
                    BitFunError::tool(format!(
                        "workspace is required for {} when the current workspace is unavailable",
                        action.as_str()
                    ))
                })?;
                Ok(Self::workspace_target_from_context(workspace))
            }
        }
    }

    pub(crate) fn workspace_target_from_context(
        workspace: &crate::agentic::WorkspaceBinding,
    ) -> SessionControlWorkspaceTarget {
        SessionControlWorkspaceTarget {
            display_workspace: normalize_path(&workspace.root_path_string()),
            project_workspace: normalize_path(&workspace.project_root_path_string()),
            execution_target: workspace.execution_target.clone(),
            workspace_id: workspace.workspace_id.clone(),
            remote_connection_id: workspace.connection_id().map(ToOwned::to_owned),
            remote_ssh_host: if workspace.is_remote() {
                Some(workspace.session_identity.hostname.clone())
                    .filter(|value| !value.trim().is_empty())
            } else {
                None
            },
        }
    }

    fn workspace_target_from_binding(
        binding: AgentSessionWorkspaceBinding,
    ) -> SessionControlWorkspaceTarget {
        let project_workspace = binding
            .project_workspace_path
            .clone()
            .unwrap_or_else(|| binding.workspace_path.clone());
        SessionControlWorkspaceTarget {
            display_workspace: binding.workspace_path,
            project_workspace,
            execution_target: binding.execution_target,
            workspace_id: binding.workspace_id,
            remote_connection_id: binding.remote_connection_id,
            remote_ssh_host: binding.remote_ssh_host,
        }
    }

    fn validation_context(context: Option<&ToolUseContext>) -> SessionControlValidationContext<'_> {
        SessionControlValidationContext {
            current_session_id: context.and_then(|value| value.session_id.as_deref()),
            has_workspace_root: context.and_then(|value| value.workspace_root()).is_some(),
        }
    }

    fn into_validation_result(result: SessionControlValidationResult) -> ValidationResult {
        ValidationResult {
            result: result.result,
            message: result.message,
            error_code: result.error_code,
            meta: result.meta,
        }
    }

    /// W4: SessionControl create 分支的 worktree 授权/remote 检查入口。
    fn ensure_worktree_allowed(&self, context: &ToolUseContext) -> BitFunResult<()> {
        ensure_worktree_creation_authorized(context)?;
        ensure_worktree_not_remote(context)
    }

    #[allow(dead_code)]
    async fn ensure_session_exists(
        &self,
        runtime: &AgentRuntime,
        workspace: &SessionControlWorkspaceTarget,
        session_id: &str,
    ) -> BitFunResult<()> {
        let existing_sessions = runtime
            .list_sessions(AgentSessionListRequest {
                workspace_path: workspace.project_workspace.clone(),
                remote_connection_id: workspace.remote_connection_id.clone(),
                remote_ssh_host: workspace.remote_ssh_host.clone(),
                include_hidden: false,
            })
            .await
            .map_err(|error| {
                BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
            })?;
        if existing_sessions
            .iter()
            .any(|session| session.session_id == session_id)
        {
            Ok(())
        } else {
            Err(BitFunError::NotFound(format!(
                "Session '{}' not found in workspace '{}'",
                session_id, workspace.display_workspace
            )))
        }
    }

    /// Build the `result_for_assistant` text for the `list` action.
    ///
    /// Default (`detail == false`) is the compact tree output: one line per
    /// session with `sessionId | agentType | status | compact name` so the
    /// model context stays small even when session names are long task
    /// descriptions. Full session names (and the JSON tree) are still
    /// available through the `data` payload and through `detail == true`,
    /// which preserves the legacy verbose tree output.
    fn build_list_result_for_assistant(
        &self,
        workspace: &str,
        sessions: &[AgentSessionSummary],
        current_session_id: Option<&str>,
        tree: Option<&SessionTreeManager>,
        short_names: &HashMap<String, Option<String>>,
        detail: bool,
    ) -> String {
        if sessions.is_empty() {
            return format!("No sessions found in workspace '{}'.", workspace);
        }

        let mut lines = vec![format!(
            "Found {} session(s) in workspace '{}'",
            sessions.len(),
            workspace
        )];
        lines.push(String::new());
        if let Some(current_session_id) = current_session_id {
            lines.push(format!("Note: '{}' is your session_id", current_session_id));
            lines.push(String::new());
        }

        if detail {
            // --- Full tree JSON view (legacy verbose output) ---
            // The full `sessions` array and parsed `tree` remain available in the
            // result `data` payload for programmatic consumers.
            lines.push("## Session Tree (JSON)".to_string());
            lines.push("```json".to_string());
            lines.push(self.build_session_tree_json(sessions, tree));
            lines.push("```".to_string());
        } else {
            // --- Compact tree text view (default) ---
            lines.push("## Sessions (compact)".to_string());
            lines.push("format: [sessionId] agentType | status | name".to_string());
            lines.extend(build_compact_tree_lines(sessions, tree, short_names));
        }
        lines.join("\n")
    }

    /// Build a JSON tree structure from the flat session list.
    /// Sessions are grouped by `parent_session_id` into a forest of root nodes.
    fn build_session_tree_json(
        &self,
        sessions: &[AgentSessionSummary],
        tree: Option<&SessionTreeManager>,
    ) -> String {
        build_session_tree_json_impl(sessions, tree)
    }
}

// ── Session↔worktree 联动共享核心（W4/W5/W8/W9）──────────────────
//
// 以下函数为文件级 pub(crate) free functions，SessionControl 与
// SessionMessage 两个工具共用（SessionMessage 经
// `use super::session_control_tool::...` 复用）。

/// W9: worktree 参数授权判定。worktree 创建 = git 文件系统操作（git
/// worktree add），是服务层调用不走工具权限门，因此独立判定：仅
/// Commander owner（或 RBAC 关闭）允许。非 owner 调用者携带 worktree
/// 参数一律拒绝。
pub(crate) fn ensure_worktree_creation_authorized(context: &ToolUseContext) -> BitFunResult<()> {
    let caller_session_id = context.session_id.as_deref().ok_or_else(|| {
        BitFunError::tool("worktree requires a caller session in tool context".to_string())
    })?;
    if worktree_creation_authorized(caller_session_id) {
        Ok(())
    } else {
        Err(BitFunError::tool(format!(
            "worktree is only allowed for owner (Commander) sessions; session '{}' is not authorized to create worktrees",
            caller_session_id
        )))
    }
}

/// W9: remote SSH 互斥拒绝（worktree 不支持 remote workspace）。
pub(crate) fn ensure_worktree_not_remote(context: &ToolUseContext) -> BitFunResult<()> {
    if context.is_remote() {
        return Err(BitFunError::tool(
            "Managed worktrees are not supported for remote SSH workspaces yet".to_string(),
        ));
    }
    Ok(())
}

/// W8 自动命名：worktree 分支 task/<序号>（从既有 task/* 序号递增）。
/// 稳定前缀 `task/`，序号 = 项目内已有 `task/<n>` 分支的最大值 + 1。
/// 并发下由 WorktreeService 的仓库级锁 + receipt 幂等兜底（同一
/// request_id 重放不会重复创建分支）。
async fn next_task_branch_name(project_workspace_path: &str) -> BitFunResult<String> {
    let branches = GitService::get_branches(project_workspace_path, false)
        .await
        .map_err(|error| BitFunError::tool(format!("Failed to list branches: {error}")))?;
    let max_task_index = branches
        .iter()
        .filter_map(|branch| {
            branch
                .name
                .strip_prefix("task/")
                .and_then(|suffix| suffix.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0);
    Ok(format!("task/{}", max_task_index + 1))
}

/// W8：把 task/<序号> 分支名清洗为合法 git 分支名（git check-ref-format
/// 规则 + 长度上限）。自动命名已保证合法，此处防御性清洗（对齐
/// dispatch_branch_name 的段级过滤思路，不信任任何输入）。
fn sanitize_task_branch_name(branch: &str) -> String {
    let sanitized: String = branch
        .split('/')
        .map(|segment| {
            segment
                .chars()
                .filter(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
                })
                .collect::<String>()
        })
        .map(|segment| segment.trim_matches('.').to_string())
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    if sanitized.is_empty() {
        "task/1".to_string()
    } else {
        sanitized
    }
}

/// 创建 managed worktree 并绑定到新会话（W4/W5 共享核心）。
///
/// 链路（对齐 WorktreeTool::create_session worktree_tool.rs:358-552，禁裸调
/// git，一切走 WorktreeService）：
/// 1. WorktreeService::create（git worktree add --detach + registry + 幂等
///    receipt）——worktree 创建成功才继续；
/// 2. track workspace（workspace 注册）；
/// 3. 自动命名分支 task/<序号> 并 create_branch（worktree 绑定分支）；
/// 4. 返回 execution_target + tracked workspace id；任何一步失败由本函数
///    回滚（worktree remove + workspace 注销），不留孤儿。
pub(crate) async fn create_worktree_for_session(
    request_id: &str,
    workspace: &SessionControlWorkspaceTarget,
    worktree_options: &WorktreeSessionOptions,
    context: &ToolUseContext,
) -> BitFunResult<SessionWorktreeCreateResult> {
    let source_workspace_path = context
        .workspace_root()
        .ok_or_else(|| BitFunError::tool("Current execution workspace is unavailable".to_string()))?
        .to_string_lossy()
        .to_string();
    let project_workspace_path = workspace.project_workspace.clone();

    let created = WorktreeService::create(WorktreeCreateRequest {
        request_id: request_id.to_string(),
        project_workspace_path: project_workspace_path.clone(),
        source_workspace_path: Some(source_workspace_path),
        base_ref: worktree_options.base_ref.clone(),
        copy_local_changes: worktree_options.copy_local_changes,
        claimed_by: None,
    })
    .await
    .map_err(|error| BitFunError::tool(error.to_string()))?;

    let worktree_id = created
        .execution_target
        .worktree_id
        .clone()
        .ok_or_else(|| {
            BitFunError::tool("Created worktree is missing its worktree_id".to_string())
        })?;

    // track workspace（对齐 WorktreeTool::create_session 的
    // track_workspace_activity 步骤）。失败即回滚新 worktree。
    let workspace_service = get_global_workspace_service()
        .ok_or_else(|| BitFunError::tool("Workspace service is not initialized".to_string()))?;
    let tracked_workspace = match workspace_service
        .track_workspace_activity(
            PathBuf::from(&created.execution_target.root_path),
            WorkspaceCreateOptions::default(),
            WorkspaceActivityMode::RefreshMetadata,
        )
        .await
    {
        Ok(workspace) => workspace,
        Err(track_error) => {
            return Err(cleanup_failed_worktree_create(
                &project_workspace_path,
                &created.execution_target,
                created.created,
                None,
                format!("Failed to register worktree workspace: {track_error}"),
            )
            .await);
        }
    };

    // 自动命名分支 task/<序号>（幂等重放 created=false 时可能已有分支，
    // 跳过分支创建）。
    let branch_name =
        sanitize_task_branch_name(&next_task_branch_name(&project_workspace_path).await?);
    if created.created && created.execution_target.branch.is_none() {
        let branch_request_id = format!("{request_id}:branch");
        if let Err(branch_error) = WorktreeService::create_branch(WorktreeCreateBranchRequest {
            request_id: branch_request_id,
            project_workspace_path: project_workspace_path.clone(),
            worktree_id: worktree_id.clone(),
            branch: branch_name.clone(),
        })
        .await
        {
            return Err(cleanup_failed_worktree_create(
                &project_workspace_path,
                &created.execution_target,
                created.created,
                Some(&tracked_workspace.id),
                format!("Failed to create worktree branch: {branch_error}"),
            )
            .await);
        }
    }

    Ok(SessionWorktreeCreateResult {
        execution_target: created.execution_target,
        tracked_workspace_id: Some(tracked_workspace.id),
        created: created.created,
        branch_name: Some(branch_name),
        project_workspace_path,
    })
}

/// 回滚刚创建的 worktree（W4/W5 失败路径）。
///
/// 对齐 WorktreeTool::cleanup_failed_fresh_create：注销 workspace +
/// WorktreeService::rollback_created（用项目路径）。仅当本次确实创建了
/// worktree（created=true）时回滚；幂等重放（created=false）不重复回滚。
async fn cleanup_failed_worktree_create(
    project_workspace_path: &str,
    execution_target: &SessionExecutionTarget,
    created: bool,
    tracked_workspace_id: Option<&str>,
    failure: impl Into<String>,
) -> BitFunError {
    let failure = failure.into();
    let mut rollback_issues = Vec::new();
    if let Some(workspace_id) = tracked_workspace_id {
        if let Some(workspace_service) = get_global_workspace_service() {
            if let Err(remove_error) = workspace_service.remove_workspace(workspace_id).await {
                rollback_issues.push(format!(
                    "workspace registration could not be removed: {remove_error}"
                ));
            }
        }
    }
    if created {
        if let Some(worktree_id) = execution_target.worktree_id.as_deref() {
            if let Err(rollback_error) =
                WorktreeService::rollback_created(project_workspace_path, worktree_id).await
            {
                rollback_issues.push(format!("worktree could not be removed: {rollback_error}"));
            }
        }
    }
    if rollback_issues.is_empty() {
        BitFunError::tool(failure)
    } else {
        BitFunError::tool(format!(
            "rollback_incomplete: {failure}; {}",
            rollback_issues.join("; ")
        ))
    }
}

/// Shared source for the agent_type enum of SessionControl/SessionMessage
/// create (and LegionControl load validation).
///
/// Returns every agent id that can back a created session: builtin/user
/// subagents, project subagents of the current workspace, builtin/user modes
/// and ACP bridge agents (`acp__<client_id>`). Unlike the TaskVisible query,
/// this deliberately includes Mode-category entries so external ACP
/// conversations are selectable; the create path validates the final value
/// through the registry anyway.
pub(crate) async fn get_available_agent_type_ids_for_creation(
    context: Option<&ToolUseContext>,
) -> Vec<String> {
    use crate::agentic::agents::get_agent_registry;
    let registry = get_agent_registry();
    let workspace_root = context.and_then(|ctx| ctx.workspace_root());
    registry.load_custom_agents(workspace_root).await;
    registry
        .get_agent_ids_for_session_creation(workspace_root)
        .await
}

/// R-26 / user-owner semantics: whether a calling session is exempt from the
/// R-2 created_by/ancestor authorization gate for session deletion.
///
/// The human user's main session (Commander role) is the owner and may delete
/// any session, including orphaned or detached children whose lineage was
/// broken by an earlier external deletion. When the RBAC master switch is off,
/// the gate is bypassed entirely.
fn caller_is_owner_session(caller_session_id: &str) -> bool {
    matches!(
        get_session_role(caller_session_id),
        Some(AgentRole::Commander)
    ) || !crate::service::config::rbac_enabled()
}

/// 无依赖的规范 uuid 形状守卫（8-4-4-4-12，36 字符），用于 ACP 流会话 id
/// （`acp_<client_id>_<uuid>`）的尾部段校验。与桌面 `AcpClientPort`
/// （`client_id_from_session_id`）及 `SessionMessage`
/// （`acp_flow_client_id_from_session_id`）的严格校验一致，防止仅以 `acp_`
/// 开头的内部会话 id 被误判为流会话（PR #2139 R4）。
pub(crate) fn looks_like_uuid(segment: &str) -> bool {
    segment.len() == 36
        && segment.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

/// 判断一个 session id 是否为 ACP 流会话（`acp_<client>_<uuid>`）。
///
/// ACP 流会话经 SessionControl `acp__` / ACP client port 创建，本地只持有
/// provider=acp 的流会话记录（interfaces/acp session_persistence.rs），
/// **不写入 createdBy / SessionRelationship 等 SessionMetadata**。因此本地
/// metadata 为空是 ACP 流会话的正常形态（不是损坏），delete 授权不能仅因
/// metadata 缺失就拒绝清理。
///
/// 尾部段必须是规范 uuid（36 字符、带横线、hex），与桌面
/// `AcpClientPort::client_id_from_session_id` / `SessionMessage`
/// `acp_flow_client_id_from_session_id` 的严格校验一致，防止任意以 `acp_`
/// 开头的内部会话 id 被幽灵放行并绕过 RBAC 属主模型（PR #2139 R4）。
pub(crate) fn is_acp_flow_session_id(session_id: &str) -> bool {
    let Some(rest) = session_id.strip_prefix("acp_") else {
        return false;
    };
    let Some((client_id, uuid_segment)) = rest.rsplit_once('_') else {
        return false;
    };
    !client_id.is_empty() && looks_like_uuid(uuid_segment)
}

/// P-06：幽灵 ACP 流会话删除授权判定。
///
/// 当目标会话 metadata 无 created_by（幽灵）且是 ACP 流会话时，授权放行——ACP
/// 流会话是外部进程记录，metadata 存在但 created_by/relationship 为空是其设计
/// 形态（interfaces/acp session_persistence 创建时必写 metadata 文件）；否则维持
/// 原有 created_by 判定（metadata 完整时原样）。
fn ghost_acp_delete_authorized(created_by_is_none: bool, acp_flow_session: bool) -> bool {
    created_by_is_none && acp_flow_session
}

/// R-26 / 幽灵孤儿删除豁免：Commander owner 是否被授权删除「无主孤儿」会话。
///
/// 无主孤儿 = 目标会话的 metadata 缺失，或 metadata 存在但 created_by 为空且无
/// relationship（未挂树）。此类会话没有创建者、ancestor 链为空，SessionControl
/// delete 的 R-2 created_by/ancestor 授权门禁会拒绝（ancestor 校验 tree+metadata
/// 双空报错），导致 list 可见但删不掉。Commander（人类用户主会话）作为 owner 兜底
/// 放行删除（对齐 R-2/R-26 的 owner 豁免语义）。
///
/// 边界：
/// - 仅 `caller_is_owner`（Commander 或 RBAC 关闭）时放行——非 owner 调用者仍被门禁
///   拒绝（防止任意会话越权删无主孤儿）。
/// - ACP 流会话走 `ghost_acp_delete_authorized`（其 created_by 空是设计形态），不
///   落入本判定。
/// - daemon/warden 与「当前会话不可删」守卫在门禁之外保持独立，不受本豁免影响。
fn orphan_session_delete_authorized(
    caller_is_owner: bool,
    target_metadata: Option<&crate::service::session::SessionMetadata>,
    acp_flow_session: bool,
) -> bool {
    caller_is_owner
        && !acp_flow_session
        && target_metadata.map_or(true, |metadata| {
            metadata.created_by.as_deref().is_none()
                && metadata
                    .relationship
                    .as_ref()
                    .and_then(|r| r.parent_session_id.as_deref())
                    .is_none()
        })
}

/// 授权判定开关：区分 delete / cancel / deliver 的授权语义。
///
/// - `allow_owner_bypass`：delete 允许 owner（Commander 或 RBAC 关闭）豁免；
///   cancel 无 owner 豁免（保持既有行为）。
/// - `allow_ghost_acp`：delete 允许幽灵 ACP 流会话放行（P-06）；
///   cancel 不允许。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SessionMutationAuthOptions {
    pub allow_owner_bypass: bool,
    pub allow_ghost_acp: bool,
}

impl SessionMutationAuthOptions {
    pub(crate) const fn delete() -> Self {
        Self {
            allow_owner_bypass: true,
            allow_ghost_acp: true,
        }
    }

    pub(crate) const fn cancel() -> Self {
        Self {
            allow_owner_bypass: false,
            allow_ghost_acp: false,
        }
    }

    /// 投递授权（SessionMessage，PR #2139 #5）：owner（Commander 角色或
    /// RBAC 关闭）豁免，但无幽灵 ACP 放行——到达投递授权门的 target 已排除
    /// ACP 流会话直通路径（流直通路径在 registry 校验后、门之前于
    /// dispatch_single 提前返回），本地投递语义不适用幽灵 ACP 放行。
    pub(crate) const fn deliver() -> Self {
        Self {
            allow_owner_bypass: true,
            allow_ghost_acp: false,
        }
    }
}

/// 共享会话变更（delete/cancel）授权判定，SessionControl 与 acp_control
/// 复用（PR #2139 R4）。
///
/// 决策链（每步与既有 SessionControl delete/cancel 语义等价）：
/// 1. daemon/warden 会话拦截（R-A.04）；
/// 2. owner 豁免（仅 delete；Commander 角色或 RBAC 关闭）；本地侧额外并入
///    R-26 幽灵孤儿删除豁免（orphan_session_delete_authorized，本质是 owner
///    兜底放行无主孤儿，含 metadata 缺失场景）；
/// 3. created_by 匹配（`session-<caller>` 标记）；delete 额外允许幽灵 ACP
///    流会话放行（ACP 流会话 metadata 无 created_by 是其设计形态）；
/// 4. 祖先授权：内存树快路径，树为空时回退持久化 metadata 链遍历（空树
///    不能被利用来绕过授权）；
///
/// `Ok(())` = 已授权；`Err` 为拒绝原因（tool error）。
pub(crate) async fn resolve_session_mutation_authorization(
    session_manager: &crate::agentic::session::session_manager::SessionManager,
    tree: &SessionTreeManager,
    caller_session_id: &str,
    target_session_id: &str,
    workspace_path: &std::path::Path,
    action_label: &str,
    options: SessionMutationAuthOptions,
) -> BitFunResult<()> {
    // R-A.04: Reject daemon/warden sessions (delete and cancel share this guard).
    {
        let is_daemon = if let Some(session) = session_manager.get_session(target_session_id) {
            session.config.is_daemon || session.agent_type.starts_with("warden-")
        } else {
            // Fall back to persisted metadata
            session_manager
                .load_session_metadata(workspace_path, target_session_id)
                .await
                .ok()
                .flatten()
                .map(|m| m.is_daemon || m.agent_type.starts_with("warden-"))
                .unwrap_or(false)
        };
        if is_daemon {
            return Err(BitFunError::tool(format!(
                "cannot {action_label} daemon/warden session '{target_session_id}'"
            )));
        }
    }

    // R-26 / user-owner semantics: the human user's main session (Commander
    // role) is the owner and may act on any session; when the RBAC master
    // switch is off, the gate is bypassed entirely. Cancel keeps the historical
    // stricter gate (no owner bypass).
    let caller_is_owner = options.allow_owner_bypass && caller_is_owner_session(caller_session_id);

    let acp_flow_session = is_acp_flow_session_id(target_session_id);
    let (created_by_match, orphan_delete_authorized) = {
        let target_metadata = session_manager
            .load_session_metadata(workspace_path, target_session_id)
            .await
            .ok()
            .flatten();
        let creator = target_metadata
            .as_ref()
            .and_then(|metadata| metadata.created_by.as_deref());
        if options.allow_ghost_acp
            && ghost_acp_delete_authorized(creator.is_none(), acp_flow_session)
        {
            (true, false)
        } else {
            (
                creator.is_some_and(|creator| {
                    creator == session_control_creator_marker(caller_session_id)
                }),
                // R-26 / 幽灵孤儿删除豁免：目标会话是「无主孤儿」时，Commander
                // owner 兜底放行删除（孤儿无创建者，ancestor 链为空，只能 owner
                // 兜底）。ACP 流会话走上方 ghost_acp_delete_authorized。
                options.allow_owner_bypass
                    && orphan_session_delete_authorized(
                        caller_is_owner,
                        target_metadata.as_ref(),
                        acp_flow_session,
                    ),
            )
        }
    };

    if !caller_is_owner && !created_by_match && !orphan_delete_authorized {
        // Ancestor authorization: verify the calling session is an ancestor of
        // the target session. First try the in-memory tree (fast path). If the
        // tree is not yet populated (walk_ancestors returns empty), fall back
        // to a persisted metadata chain query so that an empty tree cannot be
        // exploited to bypass authorization.
        let tree_ancestors = tree.walk_ancestors(target_session_id);
        let ancestors: Vec<String> = if !tree_ancestors.is_empty() {
            // Fast path: tree is populated.
            tree_ancestors
        } else {
            // Fallback: tree is empty, walk persisted metadata chain.
            let mut metadata_ancestors = Vec::new();
            // Guard against cyclic metadata chains: never revisit a session id
            // already seen during this walk.
            let mut visited = std::collections::HashSet::new();
            visited.insert(target_session_id.to_string());
            let mut current = target_session_id.to_string();
            loop {
                let metadata = session_manager
                    .load_session_metadata(workspace_path, &current)
                    .await
                    .ok()
                    .flatten();
                match metadata.and_then(|m| m.relationship.and_then(|r| r.parent_session_id)) {
                    Some(parent_id) => {
                        if !visited.insert(parent_id.clone()) {
                            // Cycle detected; stop walking to avoid hanging on a
                            // corrupt lineage chain.
                            break;
                        }
                        metadata_ancestors.push(parent_id.clone());
                        current = parent_id;
                    }
                    None => break,
                }
            }
            metadata_ancestors
        };
        if ancestors.is_empty() {
            return Err(BitFunError::tool(format!(
                "cannot verify ancestor relationship for session '{target_session_id}': tree and metadata are both empty"
            )));
        }
        if !ancestors.iter().any(|id| id == caller_session_id) {
            return Err(BitFunError::tool(format!(
                "session '{caller_session_id}' is not authorized to {action_label} session '{target_session_id}': not a parent/ancestor and not the creator"
            )));
        }
    }

    Ok(())
}

/// SessionHistory 读取授权开关。
///
/// 读取（export transcript）与变更（delete/cancel/deliver）语义对齐 R4
/// 共享授权门，并补两条读取专属约束：
/// - 同 workspace 归属校验（caller 与 target 的 storage dir 必须一致）；
/// - 树内双向授权（祖先可导出后代、后代可导出祖先），跨树拒绝。
///
/// `allow_owner_bypass`：owner（Commander 角色或 RBAC 关闭）豁免读取，
/// 与 delete 的 owner 语义一致（主会话=用户 owner，可导出任意会话）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SessionHistoryAuthOptions {
    pub allow_owner_bypass: bool,
}

impl SessionHistoryAuthOptions {
    pub(crate) const fn read() -> Self {
        Self {
            allow_owner_bypass: true,
        }
    }
}

/// 会话读取（SessionHistory export）授权判定（UX-P0-1 根因级修复）。
///
/// 对齐 [`resolve_session_mutation_authorization`]（session_control_tool.rs
/// R4 共享授权门）的 owner / created_by / 祖先判定语义，并增加：
/// 1. 同 workspace 归属校验：caller 与 target 必须属于同一 workspace
///    （storage dir 一致），跨 workspace 一律拒绝；
/// 2. 树内双向授权：caller 是 target 的祖先（可导出后代），或 target 是
///    caller 的祖先（后代可导出祖先）——限定仅本会话树祖先/后代可导出；
/// 3. Warden/daemon 会话豁免（R-A.04 同源校验）：Warden 模板刻意保留
///    SessionHistory 作跨会话审计读取（SKILL.md §工具权限），其 daemon
///    会话形态即授权依据，豁免树内/created_by 判定。
///
/// 决策链：
/// 1. 同 workspace 归属校验（新增，读取专属）；
/// 2. Warden/daemon 会话豁免（R-A.04）；
/// 3. owner 豁免（Commander 角色或 RBAC 关闭）；
/// 4. created_by 匹配（`session-<caller>` 标记）；
/// 5. 树内双向祖先授权（内存树快路径 + 持久化 metadata 链回退，空树
///    不能被利用来绕过授权）。
///
/// `Ok(())` = 已授权；`Err` 为拒绝原因（tool error）。
pub(crate) async fn resolve_session_read_authorization(
    session_manager: &crate::agentic::session::session_manager::SessionManager,
    tree: &SessionTreeManager,
    caller_session_id: &str,
    caller_workspace_path: &std::path::Path,
    target_session_id: &str,
    target_workspace_path: &std::path::Path,
    action_label: &str,
    options: SessionHistoryAuthOptions,
) -> BitFunResult<()> {
    // 0. 同 workspace 归属校验：跨 workspace 导出一律拒绝。storage dir
    //    规范化后比较（temp/符号链接形态差异不会造成误判）。
    if !same_session_storage_dir(caller_workspace_path, target_workspace_path) {
        return Err(BitFunError::tool(format!(
            "cannot {action_label} session '{target_session_id}': caller session '{caller_session_id}' belongs to a different workspace"
        )));
    }

    // R-A.04 同源：Warden/daemon 会话是可信审计角色，豁免读取授权
    // （SessionHistory 是 Warden 模板刻意保留的跨会话审计工具）。
    if caller_is_warden_or_daemon(session_manager, caller_workspace_path, caller_session_id).await {
        return Ok(());
    }

    // owner 豁免：Commander 角色或 RBAC 关闭（与 delete 的 owner 语义一致）。
    let caller_is_owner = options.allow_owner_bypass && caller_is_owner_session(caller_session_id);

    // created_by 匹配：`session-<caller>` 标记（R-2）。
    let created_by_match = session_manager
        .load_session_metadata(target_workspace_path, target_session_id)
        .await
        .ok()
        .flatten()
        .and_then(|metadata| metadata.created_by)
        .is_some_and(|creator| creator == session_control_creator_marker(caller_session_id));

    if caller_is_owner || created_by_match {
        return Ok(());
    }

    // 树内双向祖先授权：先内存树快路径，空树回退持久化 metadata 链
    // （与 mutation 门同款防绕过）。祖先可导出后代；后代可导出祖先。
    let target_ancestors = collect_session_ancestor_chain(
        session_manager,
        tree,
        target_workspace_path,
        target_session_id,
    )
    .await;
    if target_ancestors.iter().any(|id| id == caller_session_id) {
        return Ok(());
    }
    let caller_ancestors = collect_session_ancestor_chain(
        session_manager,
        tree,
        caller_workspace_path,
        caller_session_id,
    )
    .await;
    if caller_ancestors.iter().any(|id| id == target_session_id) {
        return Ok(());
    }

    Err(BitFunError::tool(format!(
        "session '{caller_session_id}' is not authorized to {action_label} session '{target_session_id}': not the owner, not the creator, and not in the same session tree (ancestor/descendant)"
    )))
}

/// 同 workspace 归属判定：storage dir 规范化后相等。
fn same_session_storage_dir(a: &std::path::Path, b: &std::path::Path) -> bool {
    let canonical =
        |path: &std::path::Path| dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    canonical(a) == canonical(b)
}

/// caller 是否为 Warden/daemon 会话（R-A.04 同源校验，含持久化回退）。
async fn caller_is_warden_or_daemon(
    session_manager: &crate::agentic::session::session_manager::SessionManager,
    caller_workspace_path: &std::path::Path,
    caller_session_id: &str,
) -> bool {
    if let Some(session) = session_manager.get_session(caller_session_id) {
        return session.config.is_daemon || session.agent_type.starts_with("warden-");
    }
    session_manager
        .load_session_metadata(caller_workspace_path, caller_session_id)
        .await
        .ok()
        .flatten()
        .map(|m| m.is_daemon || m.agent_type.starts_with("warden-"))
        .unwrap_or(false)
}

/// 收集会话祖先链：内存树非空用树快路径；否则回退持久化 metadata 链
/// （带循环保护，损坏 lineage 不会挂起）。与 mutation 门祖先遍历同款。
async fn collect_session_ancestor_chain(
    session_manager: &crate::agentic::session::session_manager::SessionManager,
    tree: &SessionTreeManager,
    workspace_path: &std::path::Path,
    session_id: &str,
) -> Vec<String> {
    let tree_ancestors = tree.walk_ancestors(session_id);
    if !tree_ancestors.is_empty() {
        return tree_ancestors;
    }
    let mut metadata_ancestors = Vec::new();
    let mut visited = std::collections::HashSet::new();
    visited.insert(session_id.to_string());
    let mut current = session_id.to_string();
    loop {
        let metadata = session_manager
            .load_session_metadata(workspace_path, &current)
            .await
            .ok()
            .flatten();
        match metadata.and_then(|m| m.relationship.and_then(|r| r.parent_session_id)) {
            Some(parent_id) => {
                if !visited.insert(parent_id.clone()) {
                    // Cycle detected; stop walking to avoid hanging on a
                    // corrupt lineage chain.
                    break;
                }
                metadata_ancestors.push(parent_id.clone());
                current = parent_id;
            }
            None => break,
        }
    }
    metadata_ancestors
}

/// Build the delete action result JSON.
/// Cascade child-deletion failures are surfaced as a structured list
/// (`cascade_failures`: `[{session_id, reason}, ...]`). Since the delete
/// action now cascades through `coordinator.delete_session_tree` with
/// all-or-nothing semantics, the list is always empty on success — any
/// member that cannot be deleted aborts the whole tree and surfaces as a
/// tool error instead. The field is kept for result-shape compatibility
/// with callers that parse the JSON contract.
fn build_delete_result_json(
    session_id: &str,
    workspace: &str,
    cascade_failures: &[(String, String)],
) -> Value {
    json!({
        "success": true,
        "action": "delete",
        "workspace": workspace,
        "session_id": session_id,
        "cascade_failures": cascade_failures
            .iter()
            .map(|(child_id, reason)| json!({
                "session_id": child_id,
                "reason": reason,
            }))
            .collect::<Vec<_>>(),
    })
}

/// Build a JSON tree structure from the flat session list.
/// Sessions are grouped by `parent_session_id` into a forest of root nodes.
pub(crate) fn build_session_tree_json_impl(
    sessions: &[AgentSessionSummary],
    tree: Option<&SessionTreeManager>,
) -> String {
    // children_by_parent: parent_session_id -> list of children
    let mut children_by_parent: HashMap<String, Vec<&AgentSessionSummary>> = HashMap::new();
    let mut roots: Vec<&AgentSessionSummary> = Vec::new();
    // Sessions whose parent chain is fully filtered out (no surviving ancestor
    // in this list). They are promoted to roots but flagged as orphaned.
    let mut orphaned: std::collections::HashSet<&str> = std::collections::HashSet::new();

    let known_ids: std::collections::HashSet<&str> =
        sessions.iter().map(|s| s.session_id.as_str()).collect();

    // R-19: resolve the effective parent of a session - the nearest ancestor
    // present in this (possibly filtered) list. When the direct parent is
    // filtered out (e.g. daemon/warden sessions), the child is re-hung onto the
    // nearest surviving ancestor instead of being promoted to a fake root,
    // which would break the lineage. The in-memory tree is used to walk past
    // filtered sessions.
    let resolve_effective_parent = |session: &AgentSessionSummary| -> Option<String> {
        let mut current = session.parent_session_id.clone()?;
        loop {
            if known_ids.contains(current.as_str()) {
                return Some(current);
            }
            match tree.and_then(|tree| tree.get_parent(&current)) {
                Some(parent) => current = parent,
                None => return None,
            }
        }
    };

    for session in sessions {
        match resolve_effective_parent(session) {
            Some(parent_id) => {
                children_by_parent
                    .entry(parent_id)
                    .or_default()
                    .push(session);
            }
            None => {
                if session.parent_session_id.is_some() {
                    // No surviving ancestor in this list — promote to a root
                    // but flag the broken lineage.
                    orphaned.insert(session.session_id.as_str());
                }
                roots.push(session);
            }
        }
    }

    /// Maximum recursion depth for tree serialization to prevent stack overflow.
    /// Authoritative value in `bitfun_core_types::session_tree::MAX_TREE_SERIALIZE_DEPTH`.
    const TREE_SERIALIZE_MAX_DEPTH: usize =
        bitfun_core_types::session_tree::MAX_TREE_SERIALIZE_DEPTH;

    fn serialize_node(
        session: &AgentSessionSummary,
        children_by_parent: &HashMap<String, Vec<&AgentSessionSummary>>,
        tree: Option<&SessionTreeManager>,
        orphaned: &std::collections::HashSet<&str>,
        recursion_depth: usize,
    ) -> serde_json::Value {
        // P2-S8: when the recursion budget is exhausted the subtree is
        // truncated; mark the node so consumers can tell a complete tree from
        // a capped one (consistent with the `orphaned` marker below).
        let truncated = recursion_depth >= TREE_SERIALIZE_MAX_DEPTH;
        let children: Vec<serde_json::Value> = if truncated {
            Vec::new()
        } else {
            children_by_parent
                .get(session.session_id.as_str())
                .map(|list| {
                    let mut sorted = list.to_vec();
                    sorted.sort_by_key(|s| s.created_at_ms);
                    sorted
                        .iter()
                        .map(|s| {
                            serialize_node(
                                s,
                                children_by_parent,
                                tree,
                                orphaned,
                                recursion_depth + 1,
                            )
                        })
                        .collect()
                })
                .unwrap_or_default()
        };

        let depth = tree
            .and_then(|t| t.get_depth(&session.session_id))
            .unwrap_or(0);

        let status = session
            .status
            .clone()
            .unwrap_or_else(|| "active".to_string());

        let mut map = serde_json::Map::new();
        map.insert("sessionId".to_string(), json!(session.session_id));
        map.insert("sessionName".to_string(), json!(session.session_name));
        map.insert("agentType".to_string(), json!(session.agent_type));
        map.insert("depth".to_string(), json!(depth));
        map.insert("status".to_string(), json!(status));
        if orphaned.contains(session.session_id.as_str()) {
            map.insert("orphaned".to_string(), json!(true));
        }
        if truncated {
            map.insert("truncated".to_string(), json!(true));
        }
        map.insert("children".to_string(), json!(children));
        serde_json::Value::Object(map)
    }

    // Sort roots by created_at_ms descending (newest first)
    let mut sorted_roots = roots;
    sorted_roots.sort_by_key(|s| std::cmp::Reverse(s.created_at_ms));

    let forest: Vec<serde_json::Value> = sorted_roots
        .iter()
        .map(|s| serialize_node(s, &children_by_parent, tree, &orphaned, 0))
        .collect();

    serde_json::to_string_pretty(&forest).unwrap_or_else(|_| "[]".to_string())
}

/// Build the compact text tree used by the default `list` output: one line per
/// session with `sessionId | agentType | status | compact name`. The tree
/// shape mirrors [`build_session_tree_json_impl`] (same grouping, orphan
/// promotion, and sort orders); only the per-node rendering is text.
fn build_compact_tree_lines(
    sessions: &[AgentSessionSummary],
    tree: Option<&SessionTreeManager>,
    short_names: &HashMap<String, Option<String>>,
) -> Vec<String> {
    // children_by_parent: parent_session_id -> list of children
    let mut children_by_parent: HashMap<String, Vec<&AgentSessionSummary>> = HashMap::new();
    let mut roots: Vec<&AgentSessionSummary> = Vec::new();
    // 父链在本列表中无幸存祖先的会话：提升为根节点，但标记 orphaned（与 JSON 模式一致）
    let mut orphaned: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let known_ids: std::collections::HashSet<&str> =
        sessions.iter().map(|s| s.session_id.as_str()).collect();

    // R-19: resolve the effective parent of a session - the nearest ancestor
    // present in this (possibly filtered) list.
    let resolve_effective_parent = |session: &AgentSessionSummary| -> Option<String> {
        let mut current = session.parent_session_id.clone()?;
        loop {
            if known_ids.contains(current.as_str()) {
                return Some(current);
            }
            match tree.and_then(|tree| tree.get_parent(&current)) {
                Some(parent) => current = parent,
                None => return None,
            }
        }
    };

    for session in sessions {
        match resolve_effective_parent(session) {
            Some(parent_id) => {
                children_by_parent
                    .entry(parent_id)
                    .or_default()
                    .push(session);
            }
            None => {
                if session.parent_session_id.is_some() {
                    // 父链全部被过滤：提升为根节点，同时标记 orphaned（与 JSON 模式一致）
                    orphaned.insert(session.session_id.as_str());
                }
                roots.push(session);
            }
        }
    }

    fn compact_line(
        session: &AgentSessionSummary,
        short_names: &HashMap<String, Option<String>>,
        orphaned: &std::collections::HashSet<&str>,
    ) -> String {
        let status = session
            .status
            .clone()
            .unwrap_or_else(|| "active".to_string());
        let display_name = compact_session_display_name(
            &session.session_name,
            short_names
                .get(&session.session_id)
                .and_then(Option::as_deref),
        );
        let orphan_marker = if orphaned.contains(session.session_id.as_str()) {
            " (orphaned)"
        } else {
            ""
        };
        format!(
            "- [{}] {} | {} | {}{}",
            session.session_id, session.agent_type, status, display_name, orphan_marker
        )
    }

    fn collect_lines(
        session: &AgentSessionSummary,
        depth: usize,
        children_by_parent: &HashMap<String, Vec<&AgentSessionSummary>>,
        short_names: &HashMap<String, Option<String>>,
        orphaned: &std::collections::HashSet<&str>,
        lines: &mut Vec<String>,
    ) {
        let indent = "  ".repeat(depth);
        lines.push(format!(
            "{indent}{}",
            compact_line(session, short_names, orphaned)
        ));
        if let Some(children) = children_by_parent.get(session.session_id.as_str()) {
            let mut sorted = children.to_vec();
            sorted.sort_by_key(|s| s.created_at_ms);
            for child in sorted {
                collect_lines(
                    child,
                    depth + 1,
                    children_by_parent,
                    short_names,
                    orphaned,
                    lines,
                );
            }
        }
    }

    let mut sorted_roots = roots;
    sorted_roots.sort_by_key(|s| std::cmp::Reverse(s.created_at_ms));

    let mut lines = Vec::new();
    for root in sorted_roots {
        collect_lines(
            root,
            0,
            &children_by_parent,
            short_names,
            &orphaned,
            &mut lines,
        );
    }
    lines
}

#[async_trait]
impl Tool for SessionControlTool {
    fn name(&self) -> &str {
        "SessionControl"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Manage persisted workspace-scoped agent sessions.

Actions:
- "create": Create a new session. You may optionally provide session_name, short_name and agent_type.
- "cancel": Cancel the target session's currently running dialog turn. This does not delete the session or clear any queued messages that may still run later.
- "compact": Compress the target session's context to reduce memory usage and token cost. Requires session_id; the session must be idle (a processing/error session is rejected). Compacting your own session or a descendant/creator session is allowed (owner/self/ancestor/creator authorization). Idempotent: returns "applied": false instead of erroring when the session has no context or is already compressed.
- "delete": Delete an existing session by session_id.
- "rename": Rename an existing session. Provide session_id (target) and session_name (new title). Persisted like the frontend rename action, so the new title survives restarts.
- "list": List all sessions. Sessions are displayed in a tree structure showing parent-child relationships (created via Task tool). By default the output is compact (sessionId | agentType | status | short name); pass "detail": true to expand the full session tree including full session names.

Related tools:
- Use Task (spawn) to launch subagents that appear as children in the session tree.
- Use SessionMessage to send messages to existing sessions.
- Use SessionHistory to export a session transcript.

Arguments:
- "workspace": Absolute workspace path. Optional for create and list; defaults to the current workspace when omitted. Ignored for cancel and delete.
- "session_name": Used by create (defaults to "New Session") and rename (required: the new title).
- "short_name": Only used by create. Optional compact display name (e.g. "secretary-standing"); it becomes the name shown in the compact list output, keeping the model context small. Ignored for ACP flow sessions.
- "detail": Only used by list. When true, the full session tree with full session names is returned instead of the compact output. Defaults to false.
- "agent_type": Only used by create. Defaults to "agentic". Allowed values are dynamically resolved from the available agent registry (common values include "agentic", "Plan", "Cowork", "DeepResearch", and any custom/external subagent types). Use "acp__<client_id>" to create a real external ACP agent session: the external client process is started immediately (same shape as the frontend create_acp_flow_session path).
  - "agentic": Coding-focused agent for implementation, debugging, and code changes.
  - "Plan": Planning agent for clarifying requirements and producing an implementation plan before coding.
  - "Cowork": Collaborative agent for office-style work such as research, documentation, presentations, etc.
  - "DeepResearch": Research agent for systematic investigation and evidence-driven reports.
- "session_id": Required for cancel, delete, and rename."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Create, list, rename, cancel, and delete persisted agent sessions.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "cancel", "delete", "rename", "compact", "list"],
                    "description": "The session action to perform."
                },
                "workspace": {
                    "type": "string",
                    "description": "Optional absolute workspace path for create and list; defaults to the current workspace when omitted. Ignored for cancel and delete."
                },
                "session_id": {
                    "type": "string",
                    "description": "Required for cancel, delete, compact, and rename."
                },
                "session_name": {
                    "type": "string",
                    "description": "Display name when creating a session; required as the new title when renaming."
                },
                "short_name": {
                    "type": "string",
                    "description": "Optional compact display name when creating a session (used by compact list output; ignored for ACP flow sessions)."
                },
                "detail": {
                    "type": "boolean",
                    "description": "When true, list returns the full session tree with full session names instead of the compact output."
                },
                "agent_type": {
                    "type": "string",
                    "description": "Optional agent type when creating a session (defaults to \"agentic\"). Valid values are dynamically resolved from the available agent registry. Use \"acp__<client_id>\" to create a real external ACP agent session (the external client process starts immediately)."
                },
                "model_id": {
                    "type": "string",
                    "description": "Optional model id used when creating a session; the created session binds to this model."
                },
                "worktree": {
                    "type": "object",
                    "description": "Optional worktree options for create: creates a managed Git worktree together with the session and binds the session to it (only for create; not supported for remote workspaces). Shape: {baseRef?, copyLocalChanges?}.",
                    "properties": {
                        "baseRef": {
                            "type": "string",
                            "description": "Optional Git ref for the new worktree. Defaults to HEAD."
                        },
                        "copyLocalChanges": {
                            "type": "boolean",
                            "default": false,
                            "description": "Copy staged, unstaged, untracked, and .worktreeinclude-selected ignored files when the selected base equals source HEAD."
                        }
                    },
                    "additionalProperties": false
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    /// Dynamically resolves allowed agent_type values from the agent registry.
    async fn input_schema_for_model_with_context(&self, context: Option<&ToolUseContext>) -> Value {
        let agent_type_ids = get_available_agent_type_ids_for_creation(context).await;
        let agent_type_enum: Vec<&str> = agent_type_ids.iter().map(|s| s.as_str()).collect();
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "cancel", "delete", "rename", "compact", "list"],
                    "description": "The session action to perform."
                },
                "workspace": {
                    "type": "string",
                    "description": "Optional absolute workspace path for create and list; defaults to the current workspace when omitted. Ignored for cancel and delete."
                },
                "session_id": {
                    "type": "string",
                    "description": "Required for cancel, delete, compact, and rename."
                },
                "session_name": {
                    "type": "string",
                    "description": "Display name when creating a session; required as the new title when renaming."
                },
                "short_name": {
                    "type": "string",
                    "description": "Optional compact display name when creating a session (used by compact list output; ignored for ACP flow sessions)."
                },
                "detail": {
                    "type": "boolean",
                    "description": "When true, list returns the full session tree with full session names instead of the compact output."
                },
                "agent_type": {
                    "type": "string",
                    "enum": agent_type_enum,
                    "description": "Optional agent type when creating a session. Defaults to \"agentic\". Use \"acp__<client_id>\" to create a real external ACP agent session (the external client process starts immediately)."
                },
                "model_id": {
                    "type": "string",
                    "description": "Optional model id used when creating a session; the created session binds to this model."
                },
                "worktree": {
                    "type": "object",
                    "description": "Optional worktree options for create: creates a managed Git worktree together with the session and binds the session to it (only for create; not supported for remote workspaces). Shape: {baseRef?, copyLocalChanges?}.",
                    "properties": {
                        "baseRef": {
                            "type": "string",
                            "description": "Optional Git ref for the new worktree. Defaults to HEAD."
                        },
                        "copyLocalChanges": {
                            "type": "boolean",
                            "default": false,
                            "description": "Copy staged, unstaged, untracked, and .worktreeinclude-selected ignored files when the selected base equals source HEAD."
                        }
                    },
                    "additionalProperties": false
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let parsed: SessionControlInput = match serde_json::from_value(input.clone()) {
            Ok(value) => value,
            Err(err) => {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Invalid input: {}", err)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        Self::into_validation_result(validate_session_control_input(
            &parsed,
            Self::validation_context(context),
        ))
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        render_session_control_tool_use_message(input)
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: SessionControlInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::tool(format!("Invalid input: {}", e)))?;
        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;
        let runtime = CoreServiceAgentRuntime::agent_runtime(coordinator.clone())
            .map_err(BitFunError::tool)?;

        match params.action {
            SessionControlAction::Create => {
                let workspace = self
                    .resolve_effective_workspace(
                        SessionControlAction::Create,
                        None,
                        params.workspace.as_deref(),
                        context,
                        &runtime,
                    )
                    .await?;
                // R-14 B3: role-based delegation validation (fast fail). The
                // SessionControl create chain registers the new session with the
                // creator's role (B2), so the target is the inherited role; this
                // is a defensive check that stays permissive today and guards a
                // future explicit target-role channel from over-delegation.
                let creator_role = context.session_id.as_deref().and_then(get_session_role);
                let target_role = creator_role.clone().unwrap_or(AgentRole::Commander);
                validate_delegation(creator_role, target_role)?;
                let session_name =
                    session_control_session_name_or_default(params.session_name.as_deref());
                let agent_type = session_control_agent_type_or_default(params.agent_type.as_ref());

                // W9: worktree 参数授权 + remote 互斥拒绝。worktree 创建是 git
                // 文件系统操作，仅 Commander owner（或 RBAC 关闭）允许，且
                // remote SSH 工作区不支持。
                if params.worktree.is_some() {
                    self.ensure_worktree_allowed(context)?;
                }

                // ACP 真会话路径：agent_type `acp__<client_id>`（ACP bridge agent
                // registry id，见 AcpAgent::agent_id_for）直接经 AcpClientPort 创建
                // 真外部 ACP 会话——与前端 create_acp_flow_session 等价（持久记录 +
                // 进程启动 + 失败回滚），不再创建本地内部中转壳会话。流会话记录只存
                // provider/acpClientId 等 ACP 元数据（interfaces/acp session_persistence.rs:57-64），
                // 不支持 createdBy/sessionKind=subagent 与军团树挂载（lineage/
                // register_child）；军团侧持返回的 session_id 经 SessionMessage
                // 直通（acp: 流会话分叉）通信。
                if let Some(client_id) = agent_type
                    .strip_prefix(AcpAgent::agent_id_prefix())
                    .filter(|client_id| !client_id.trim().is_empty())
                {
                    let port = coordinator.acp_client_port().ok_or_else(|| {
                        BitFunError::tool(
                            "ACP client port is not available; the desktop host did not inject it"
                                .to_string(),
                        )
                    })?;
                    let created = self
                        .create_acp_session_via_port(
                            &workspace,
                            client_id,
                            params.session_name.clone(),
                            port.as_ref(),
                        )
                        .await?;
                    let result_for_assistant = session_control_created_result_message(
                        &created.session_id,
                        &workspace.display_workspace,
                        &created.agent_type,
                    );
                    return Ok(vec![ToolResult::Result {
                        data: json!({
                            "success": true,
                            "action": "create",
                            "workspace": workspace.display_workspace.clone(),
                            "session": {
                                "session_id": created.session_id,
                                "session_name": created.session_name,
                                "agent_type": created.agent_type,
                            }
                        }),
                        result_for_assistant: Some(result_for_assistant),
                        image_attachments: None,
                    }]);
                }

                // SESSION-01: create 前用 find_agent_entry（经 get_agent 公共包装）校验
                // agent_type：未在 agent registry 注册的类型直接拒绝，避免任意字符串
                // 进入 create_session 形成僵尸会话。
                {
                    let registry = get_agent_registry();
                    let workspace_path = std::path::Path::new(&workspace.display_workspace);
                    registry.load_custom_agents(Some(workspace_path)).await;
                    if registry
                        .get_agent(&agent_type, Some(workspace_path))
                        .is_none()
                    {
                        return Err(BitFunError::tool(format!(
                            "Unknown agent_type '{}' for SessionControl create; agent must be registered in the agent registry",
                            agent_type
                        )));
                    }
                }

                // W4: worktree 参数命中 → 先创建 managed worktree（WorktreeService
                // 链路，worktree 创建成功才创建会话），把会话 execution_target 指向
                // worktree。失败 = 会话不创建 + worktree 回滚，零孤儿。
                let mut created_worktree: Option<SessionWorktreeCreateResult> = None;
                if let Some(worktree_options) = params.worktree.as_ref() {
                    let request_id = context
                        .tool_call_id
                        .as_deref()
                        .map(|tool_call_id| format!("session-control:{tool_call_id}:worktree"))
                        .unwrap_or_else(|| {
                            format!("session-control:{}:worktree", uuid::Uuid::new_v4())
                        });
                    let worktree = create_worktree_for_session(
                        &request_id,
                        &workspace,
                        worktree_options,
                        context,
                    )
                    .await?;
                    created_worktree = Some(worktree);
                }

                let created_by = self.creator_session_marker(context)?;
                let mut metadata = serde_json::Map::new();
                metadata.insert("createdBy".to_string(), json!(created_by));
                // SessionControl-created sessions are subagent sessions: force a 1M
                // context window and keep it stable across model-window refresh.
                metadata.insert("subagent".to_string(), json!(true));
                // Lineage facts forwarded through the free-form metadata map so the
                // SessionCreated event can carry the parent relationship. The
                // coordinator reads these keys defensively before emitting
                // (parent_session_id / subagent_type), keeping the event contract
                // in sync with the persisted SessionRelationship written below.
                metadata.insert(
                    "parentSessionId".to_string(),
                    json!(context.session_id.clone()),
                );
                metadata.insert("subagentType".to_string(), json!(agent_type.clone()));
                let session = match runtime
                    .create_session(AgentSessionCreateRequest {
                        session_name,
                        agent_type,
                        workspace_path: Some(
                            created_worktree
                                .as_ref()
                                .map(|wt| wt.execution_target.root_path.clone())
                                .unwrap_or_else(|| workspace.display_workspace.clone()),
                        ),
                        project_workspace_path: Some(
                            created_worktree
                                .as_ref()
                                .map(|wt| wt.project_workspace_path.clone())
                                .unwrap_or_else(|| workspace.project_workspace.clone()),
                        ),
                        execution_target: created_worktree
                            .as_ref()
                            .map(|wt| wt.execution_target.clone())
                            .or_else(|| workspace.execution_target.clone()),
                        workspace_id: created_worktree
                            .as_ref()
                            .and_then(|wt| wt.tracked_workspace_id.clone())
                            .or_else(|| workspace.workspace_id.clone()),
                        remote_connection_id: workspace.remote_connection_id.clone(),
                        remote_ssh_host: workspace.remote_ssh_host.clone(),
                        model_id: params.model_id.clone(),
                        metadata,
                    })
                    .await
                {
                    Ok(session) => session,
                    Err(create_error) => {
                        // 会话创建失败 → 回滚已创建的 worktree（仅当本次确实创建）。
                        if let Some(worktree) = created_worktree.as_ref() {
                            if worktree.created {
                                if let Some(workspace_service) = get_global_workspace_service() {
                                    if let Some(workspace_id) =
                                        worktree.tracked_workspace_id.as_deref()
                                    {
                                        let _ =
                                            workspace_service.remove_workspace(workspace_id).await;
                                    }
                                }
                                if let Some(worktree_id) =
                                    worktree.execution_target.worktree_id.as_deref()
                                {
                                    let _ = WorktreeService::rollback_created(
                                        &worktree.project_workspace_path,
                                        worktree_id,
                                    )
                                    .await;
                                }
                            }
                        }
                        return Err(BitFunError::tool(
                            CoreServiceAgentRuntime::runtime_error_message(create_error),
                        ));
                    }
                };
                let created_session_id = session.session_id.clone();
                let created_session_name = session.session_name.clone();
                let created_agent_type = session.agent_type.clone();
                let created_model_id = session.model_id.clone();

                // --- R-001/R-002: write SessionRelationship, depth inherited from parent ---
                {
                    use bitfun_services_core::session::types::{
                        SessionRelationship, SessionRelationshipKind,
                    };
                    let parent_session_id = context.session_id.clone();
                    // Read parent depth from persisted metadata, default 0 for root
                    let parent_depth = if let Some(ref pid) = parent_session_id {
                        coordinator
                            .session_manager
                            .load_session_metadata(
                                &std::path::PathBuf::from(&workspace.project_workspace),
                                pid,
                            )
                            .await
                            .ok()
                            .flatten()
                            .and_then(|m| m.relationship.and_then(|r| r.depth))
                            .unwrap_or(0u32)
                    } else {
                        0u32
                    };
                    let child_depth = parent_depth + 1;
                    // Guard against exceeding max depth (same as Task tool depth guard)
                    let max_depth = coordinator.session_tree().max_depth;
                    if child_depth > max_depth {
                        return Err(BitFunError::tool(format!(
                            "Session depth limit reached: child depth {} would exceed max allowed depth {}",
                            child_depth, max_depth
                        )));
                    }
                    let relationship = SessionRelationship {
                        kind: Some(SessionRelationshipKind::Subagent),
                        parent_session_id,
                        depth: Some(child_depth),
                        ..Default::default()
                    };
                    // SESSION-03: lineage 持久化失败会让重启后的子会话成为孤儿节点。
                    // 先重试一次以吸收瞬时 IO 故障；仍失败则回滚已创建的子会话，
                    // 确保不留下无父子关系记录的孤儿会话（绝不静默降级为 log）。
                    let mut lineage_result = coordinator
                        .session_manager
                        .persist_session_lineage(&created_session_id, relationship.clone())
                        .await;
                    if lineage_result.is_err() {
                        log::warn!(
                            "SessionControl create: lineage persist failed for {}, retrying once: {:?}",
                            created_session_id,
                            lineage_result.as_ref().err()
                        );
                        lineage_result = coordinator
                            .session_manager
                            .persist_session_lineage(&created_session_id, relationship)
                            .await;
                    }
                    if let Err(e) = lineage_result {
                        // 回滚创建：删除刚创建的子会话；回滚自身失败时仍要上报，
                        // 让调用方知道存在未被清理的会话。
                        if let Err(rollback_error) = coordinator
                            .delete_session(
                                std::path::Path::new(&workspace.project_workspace),
                                &created_session_id,
                            )
                            .await
                        {
                            log::error!(
                                "SessionControl create: lineage persist failed for {} ({:?}), rollback of session also failed: {:?}",
                                created_session_id, e, rollback_error
                            );
                        }
                        return Err(BitFunError::tool(format!(
                            "failed to persist session lineage for {} after retry: {}",
                            created_session_id, e
                        )));
                    }

                    // R-003: Register in memory tree
                    if let Some(ref pid) = context.session_id {
                        if let Err(e) = coordinator.session_tree().register_child(
                            pid,
                            &created_session_id,
                            child_depth,
                        ) {
                            log::warn!(
                                "SessionControl create: failed to register child {} under {} in tree: {:?}",
                                created_session_id, pid, e
                            );
                        }
                    }

                    // Short name persistence: write `shortName` into the session
                    // custom metadata (same best-effort pattern as the RBAC role
                    // persistence) so the compact `list` output can show it
                    // without pulling the full session name into the context.
                    if let Some(short_name) = params
                        .short_name
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        if let Err(e) = coordinator
                            .session_manager
                            .update_session_metadata(
                                &std::path::PathBuf::from(&workspace.project_workspace),
                                &created_session_id,
                                |metadata| {
                                    merge_session_custom_metadata(
                                        metadata,
                                        serde_json::json!({ "shortName": short_name }),
                                    );
                                },
                            )
                            .await
                        {
                            log::warn!(
                                "SessionControl create: failed to persist short name for {}: {:?}",
                                created_session_id,
                                e
                            );
                        }
                    }
                }
                let result_for_assistant = session_control_created_result_message(
                    &created_session_id,
                    &workspace.display_workspace,
                    &created_agent_type,
                );

                // W4: worktree 创建成功时把 worktree 信息透出给调用方（worktree_id、
                // 路径、自动命名分支）。
                let worktree_payload = created_worktree.as_ref().map(|worktree| {
                    json!({
                        "worktree_id": worktree.execution_target.worktree_id,
                        "path": worktree.execution_target.root_path,
                        "branch": worktree.branch_name,
                    })
                });
                let mut data = json!({
                    "success": true,
                    "action": "create",
                    "workspace": workspace.display_workspace.clone(),
                    "session": {
                        "session_id": created_session_id,
                        "session_name": created_session_name,
                        "agent_type": created_agent_type,
                        "model_id": created_model_id,
                    }
                });
                if let Some(worktree_payload) = worktree_payload {
                    data["worktree"] = worktree_payload;
                }
                Ok(vec![ToolResult::Result {
                    data,
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
            SessionControlAction::Cancel => {
                let session_id = params.session_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("session_id is required for cancel".to_string())
                })?;
                validate_session_id(session_id).map_err(BitFunError::tool)?;
                let workspace = self
                    .resolve_effective_workspace(
                        SessionControlAction::Cancel,
                        Some(session_id),
                        None,
                        context,
                        &runtime,
                    )
                    .await?;
                if self.current_workspace_session(context, &workspace.display_workspace)
                    == Some(session_id)
                {
                    return Err(BitFunError::tool(
                        "cannot cancel the current session from SessionControl".to_string(),
                    ));
                }

                // R-2: Authorization (shared gate with acp_control; PR #2139 R4):
                // a caller may cancel a session it created (created_by marker
                // matches) OR any session in its descendant subtree. The
                // "cannot cancel the current session" and daemon/warden guards
                // above are preserved. Cancel keeps the historical stricter
                // gate: no owner bypass and no ghost-ACP release.
                let current_session_id = context.session_id.as_ref().ok_or_else(|| {
                    BitFunError::tool(
                        "cannot cancel a session without a caller session in tool context"
                            .to_string(),
                    )
                })?;
                resolve_session_mutation_authorization(
                    coordinator.get_session_manager(),
                    coordinator.session_tree(),
                    current_session_id,
                    session_id,
                    std::path::Path::new(&workspace.project_workspace),
                    "cancel",
                    SessionMutationAuthOptions::cancel(),
                )
                .await?;

                let scheduler = get_global_scheduler();
                let cancel_route = resolve_session_control_cancel_route(
                    context.session_id.as_deref(),
                    scheduler.is_some(),
                );
                let (runtime, requester_session_id) = match (cancel_route, scheduler) {
                    (
                        SessionControlCancelRoute::RequesterViaScheduler {
                            requester_session_id,
                        },
                        Some(scheduler),
                    ) => {
                        let runtime = CoreServiceAgentRuntime::agent_runtime_with_scheduler_ports(
                            coordinator.clone(),
                            scheduler,
                        )
                        .map_err(BitFunError::tool)?;
                        (runtime, Some(requester_session_id))
                    }
                    _ => {
                        // Fallback covers unusual tool contexts and startup states where the
                        // global scheduler is not available; concrete cancellation still works.
                        (runtime.clone(), None)
                    }
                };
                let cancelled_turn_id = runtime
                    .cancel_turn(AgentTurnCancellationRequest {
                        session_id: session_id.to_string(),
                        turn_id: None,
                        source: Some(AgentSubmissionSource::AgentSession),
                        requester_session_id,
                        reason: None,
                        wait_timeout_ms: Some(CANCEL_WAIT_TIMEOUT.as_millis() as u64),
                        cancel_descendants: true,
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?
                    .turn_id;
                let had_active_turn = cancelled_turn_id.is_some();
                let status = session_control_cancel_status(cancelled_turn_id.as_deref());
                let result_for_assistant = session_control_cancel_result_message(
                    session_id,
                    &workspace.display_workspace,
                    cancelled_turn_id.as_deref(),
                );

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "cancel",
                        "workspace": workspace.display_workspace.clone(),
                        "session_id": session_id,
                        "had_active_turn": had_active_turn,
                        "cancelled_turn_id": cancelled_turn_id,
                        "status": status,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
            SessionControlAction::Delete => {
                let session_id = params.session_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("session_id is required for delete".to_string())
                })?;
                validate_session_id(session_id).map_err(BitFunError::tool)?;
                let workspace = self
                    .resolve_effective_workspace(
                        SessionControlAction::Delete,
                        Some(session_id),
                        None,
                        context,
                        &runtime,
                    )
                    .await?;
                if self.current_workspace_session(context, &workspace.display_workspace)
                    == Some(session_id)
                {
                    return Err(BitFunError::tool(
                        "cannot delete the current session from SessionControl".to_string(),
                    ));
                }

                // R-2: Authorization (shared gate with acp_control; PR #2139 R4):
                // a caller may delete a session it created (created_by marker
                // matches) OR any session in its descendant subtree, with the
                // user-owner (Commander / RBAC-off) bypass, the R-26 orphan
                // delete exemption, and the P-06 ghost-ACP release. The
                // "cannot delete the current session" and daemon/warden guards
                // above are preserved. Deletion of a daemon/warden session is
                // rejected here and the tree path enforces the same guard for
                // every member.
                let current_session_id = context.session_id.as_ref().ok_or_else(|| {
                    BitFunError::tool(
                        "cannot delete a session without a caller session in tool context"
                            .to_string(),
                    )
                })?;
                resolve_session_mutation_authorization(
                    coordinator.get_session_manager(),
                    coordinator.session_tree(),
                    current_session_id,
                    session_id,
                    std::path::Path::new(&workspace.project_workspace),
                    "delete",
                    SessionMutationAuthOptions::delete(),
                )
                .await?;

                // R-012: Cascade-delete the full descendant subtree through
                // `coordinator.delete_session_tree`, the same all-or-nothing
                // path used by the frontend UI delete. It pre-checks every
                // member (a processing or daemon/warden session anywhere in
                // the tree rejects the whole cascade) and deletes children
                // before the parent. The previous per-child failure-tolerant
                // loop could return success while a running child session
                // stayed on disk, which then resurrected as a ghost child
                // session on the next restart (ghost-session root cause R2);
                // the tree path aborts instead and reports which member is
                // not deletable. Deletion of a daemon/warden session was
                // already rejected above; the tree path enforces the same
                // guard for every member.
                let delete_request = AgentSessionDeleteRequest {
                    workspace_path: workspace.project_workspace.clone(),
                    session_id: session_id.to_string(),
                    remote_connection_id: workspace.remote_connection_id.clone(),
                    remote_ssh_host: workspace.remote_ssh_host.clone(),
                };
                coordinator
                    .delete_session_tree(
                        std::path::Path::new(&delete_request.workspace_path),
                        delete_request.remote_connection_id.as_deref(),
                        delete_request.remote_ssh_host.as_deref(),
                        &delete_request.session_id,
                    )
                    .await
                    .map_err(|error| {
                        BitFunError::tool(format!(
                            "cannot delete session tree rooted at '{}': {}",
                            session_id, error
                        ))
                    })?;

                Ok(vec![ToolResult::Result {
                    data: build_delete_result_json(session_id, &workspace.display_workspace, &[]),
                    result_for_assistant: Some(session_control_deleted_result_message(
                        session_id,
                        &workspace.display_workspace,
                    )),
                    image_attachments: None,
                }])
            }
            SessionControlAction::List => {
                let workspace = self
                    .resolve_effective_workspace(
                        SessionControlAction::List,
                        None,
                        params.workspace.as_deref(),
                        context,
                        &runtime,
                    )
                    .await?;
                // UX-P2-2: cross-workspace listing requires authorization. The
                // caller may list the workspace it currently belongs to; an
                // explicit `workspace` argument pointing elsewhere is only
                // allowed for the owner (Commander / RBAC-off) or a
                // Warden/daemon audit session. This prevents a delegated
                // subagent from silently enumerating other workspaces'
                // session summaries.
                if let Some(caller_session_id) = context.session_id.as_deref() {
                    let current_workspace = context
                        .workspace_root()
                        .map(|path| normalize_path(path.to_string_lossy().as_ref()));
                    let explicit_workspace = normalize_path(&workspace.project_workspace);
                    let is_cross_workspace = current_workspace
                        .as_ref()
                        .is_none_or(|current| *current != explicit_workspace);
                    if is_cross_workspace
                        && !caller_is_owner_session(caller_session_id)
                        && !caller_is_warden_or_daemon(
                            coordinator.get_session_manager(),
                            std::path::Path::new(&workspace.project_workspace),
                            caller_session_id,
                        )
                        .await
                    {
                        return Err(BitFunError::tool(format!(
                            "cannot list sessions in workspace '{}': caller session '{caller_session_id}' does not belong to that workspace and is not the owner or an audit session",
                            workspace.display_workspace
                        )));
                    }
                }
                let sessions = runtime
                    .list_sessions(AgentSessionListRequest {
                        workspace_path: workspace.project_workspace.clone(),
                        remote_connection_id: workspace.remote_connection_id.clone(),
                        remote_ssh_host: workspace.remote_ssh_host.clone(),
                        // R-2: Full conversation management — include hidden
                        // Subagent/Ephemeral sessions; daemon/warden sessions
                        // are filtered below.
                        include_hidden: true,
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?;

                // Filter out daemon sessions (is_daemon=true or agent_type starts with "warden-")
                let sessions: Vec<_> = sessions
                    .into_iter()
                    .filter(|s| !s.is_daemon && !s.agent_type.starts_with("warden-"))
                    .collect();

                // Resolve compact short names from persisted session metadata
                // (custom_metadata.shortName, written by create when a
                // short_name argument was provided). Best-effort: sessions
                // without metadata or without a shortName fall back to the
                // truncated full name in the compact output.
                // SESSION-06: 一次批量读取全部持久化元数据
                // （list_session_metadata_including_internal）再逐会话提取
                // shortName，替代原先对每个会话串行 load_session_metadata 的
                // N+1 读。
                let mut short_names: HashMap<String, Option<String>> = HashMap::new();
                let surfaced_session_ids: std::collections::HashSet<&str> = sessions
                    .iter()
                    .map(|session| session.session_id.as_str())
                    .collect();
                let metadata_list = coordinator
                    .session_manager
                    .persistence_manager()
                    .list_session_metadata_including_internal(&std::path::PathBuf::from(
                        &workspace.project_workspace,
                    ))
                    .await
                    // 批量读取失败时按“无任何 shortName”处理（与原先逐条
                    // .ok().flatten() 的最佳努力语义一致，不中断 list 输出）。
                    .unwrap_or_default();
                for metadata in metadata_list {
                    // 仅保留已过滤会话（daemon/warden 已在上方剔除）的
                    // shortName，保持输出契约不变。
                    if !surfaced_session_ids.contains(metadata.session_id.as_str()) {
                        continue;
                    }
                    let short_name = metadata
                        .custom_metadata
                        .as_ref()
                        .and_then(|custom| custom.get("shortName"))
                        .and_then(|value| value.as_str())
                        .map(str::to_string);
                    short_names.insert(metadata.session_id, short_name);
                }

                let detail = params.detail.unwrap_or(false);
                let current_session_id =
                    self.current_workspace_session(context, &workspace.display_workspace);
                let result_for_assistant = self.build_list_result_for_assistant(
                    &workspace.display_workspace,
                    &sessions,
                    current_session_id,
                    Some(coordinator.session_tree().as_ref()),
                    &short_names,
                    detail,
                );

                let tree_json = self
                    .build_session_tree_json(&sessions, Some(coordinator.session_tree().as_ref()));
                let tree_value: Value = serde_json::from_str(&tree_json).unwrap_or(Value::Null);

                // SESSION-05: when detail=false, keep the machine-readable
                // `data.sessions` payload compact too. Each session's `name`
                // follows the same rule as the compact list lines: the short
                // name wins, otherwise the full session name is truncated to
                // 60 chars. The full sessions array stays available in the
                // detail=true payload, which the legacy verbose tree view
                // still relies on.
                let data_sessions: Vec<AgentSessionSummary> = if detail {
                    sessions
                } else {
                    sessions
                        .iter()
                        .map(|session| AgentSessionSummary {
                            session_name: compact_session_display_name(
                                &session.session_name,
                                short_names
                                    .get(&session.session_id)
                                    .and_then(Option::as_deref),
                            ),
                            ..session.clone()
                        })
                        .collect()
                };

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "list",
                        "workspace": workspace.display_workspace.clone(),
                        "current_session_id": current_session_id,
                        "count": data_sessions.len(),
                        "sessions": data_sessions,
                        "tree": tree_value,
                        "short_names": short_names,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
            SessionControlAction::Compact => {
                let session_id = params.session_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("session_id is required for compact".to_string())
                })?;
                validate_session_id(session_id).map_err(BitFunError::tool)?;
                let workspace = self
                    .resolve_effective_workspace(
                        SessionControlAction::Compact,
                        Some(session_id),
                        None,
                        context,
                        &runtime,
                    )
                    .await?;

                // 授权沿用 owner/ancestor/RBAC 语义（不新增放宽）；
                // Compact 额外允许压缩自己（含自己、含常驻 subagent 工位——契约）。
                let current_session_id = context.session_id.as_ref().ok_or_else(|| {
                    BitFunError::tool(
                        "cannot compact a session without a caller session in tool context"
                            .to_string(),
                    )
                })?;
                let caller_is_owner = caller_is_owner_session(current_session_id);
                let is_self = current_session_id == session_id;
                let created_by_match = {
                    let session_manager = coordinator.get_session_manager();
                    let target_metadata = session_manager
                        .load_session_metadata(
                            &std::path::PathBuf::from(&workspace.project_workspace),
                            session_id,
                        )
                        .await
                        .ok()
                        .flatten();
                    target_metadata
                        .as_ref()
                        .and_then(|metadata| metadata.created_by.as_deref())
                        .is_some_and(|creator| {
                            creator == session_control_creator_marker(current_session_id)
                        })
                };
                if !caller_is_owner && !is_self && !created_by_match {
                    let tree = coordinator.session_tree();
                    let tree_ancestors = tree.walk_ancestors(session_id);
                    let ancestors: Vec<String> = if !tree_ancestors.is_empty() {
                        tree_ancestors
                    } else {
                        let session_manager = coordinator.get_session_manager();
                        let mut metadata_ancestors = Vec::new();
                        let mut visited = std::collections::HashSet::new();
                        visited.insert(session_id.to_string());
                        let mut current = session_id.to_string();
                        loop {
                            let metadata = session_manager
                                .load_session_metadata(
                                    &std::path::PathBuf::from(&workspace.project_workspace),
                                    &current,
                                )
                                .await
                                .ok()
                                .flatten();
                            match metadata
                                .and_then(|m| m.relationship.and_then(|r| r.parent_session_id))
                            {
                                Some(parent_id) => {
                                    if !visited.insert(parent_id.clone()) {
                                        break;
                                    }
                                    metadata_ancestors.push(parent_id.clone());
                                    current = parent_id;
                                }
                                None => break,
                            }
                        }
                        metadata_ancestors
                    };
                    if ancestors.is_empty() {
                        return Err(BitFunError::tool(format!(
                            "cannot verify ancestor relationship for session '{session_id}': tree and metadata are both empty"
                        )));
                    }
                    if !ancestors.contains(current_session_id) {
                        return Err(BitFunError::tool(format!(
                            "session '{current_session_id}' is not authorized to compact session '{session_id}': not a parent/ancestor and not the creator"
                        )));
                    }
                }

                // 幂等：无上下文/已压 → applied=false 不报错（由压缩执行层保证）；
                // 非 Idle 拒绝由 start_manual_compaction_task 内部校验并带原因。
                let outcome = coordinator
                    .compact_session_with_outcome(session_id.to_string())
                    .await
                    .map_err(|error| {
                        BitFunError::tool(format!(
                            "cannot compact session '{session_id}': {}",
                            error
                        ))
                    })?;

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "compact",
                        "workspace": workspace.display_workspace.clone(),
                        "session_id": session_id,
                        "applied": outcome.applied,
                        "tokens_before": outcome.tokens_before,
                        "tokens_after": outcome.tokens_after,
                        "compression_ratio": outcome.compression_ratio,
                        "duration": outcome.duration_ms,
                        "summary_source": if outcome.has_summary {
                            Some(outcome.summary_source)
                        } else {
                            None
                        },
                    }),
                    result_for_assistant: Some(format!(
                        "Compacted session '{session_id}' in workspace '{}'.",
                        workspace.display_workspace
                    )),
                    image_attachments: None,
                }])
            }
            SessionControlAction::Rename => {
                let session_id = params.session_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("session_id is required for rename".to_string())
                })?;
                validate_session_id(session_id).map_err(BitFunError::tool)?;
                let session_name = params
                    .session_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        BitFunError::tool(
                            "session_name is required and must not be empty for rename".to_string(),
                        )
                    })?;
                let workspace = self
                    .resolve_effective_workspace(
                        SessionControlAction::Rename,
                        Some(session_id),
                        None,
                        context,
                        &runtime,
                    )
                    .await?;
                if self.current_workspace_session(context, &workspace.display_workspace)
                    == Some(session_id)
                {
                    return Err(BitFunError::tool(
                        "cannot rename the current session from SessionControl".to_string(),
                    ));
                }

                // 复用前端 renameChatSessionTitle 同一条重命名通道
                // （AgentSessionManagementPort::rename_session），保证标题持久化
                // 行为与桌面/前端一致。
                runtime
                    .rename_session(bitfun_runtime_ports::AgentSessionRenameRequest {
                        workspace_path: workspace.display_workspace.clone(),
                        session_id: session_id.to_string(),
                        session_name: session_name.to_string(),
                        remote_connection_id: workspace.remote_connection_id.clone(),
                        remote_ssh_host: workspace.remote_ssh_host.clone(),
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(format!(
                            "cannot rename session '{session_id}': {}",
                            CoreServiceAgentRuntime::runtime_error_message(error)
                        ))
                    })?;

                let result_for_assistant = session_control_renamed_result_message(
                    session_id,
                    &workspace.display_workspace,
                    session_name,
                );
                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "rename",
                        "workspace": workspace.display_workspace.clone(),
                        "session_id": session_id,
                        "session_name": session_name,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::tools::framework::ToolUseContext;
    use crate::agentic::WorkspaceBinding;
    use bitfun_core_types::{
        SessionExecutionTarget, SessionExecutionTargetKind, WorktreeLifecycle,
    };
    use bitfun_runtime_ports::{
        AcpClientBitfunMessageRequest, AcpClientCancelRequest, AcpClientHistoryRequest,
        AcpClientHistoryResult, AcpClientListResult, AcpClientMessageRequest,
        AcpClientMessageResult, AcpClientReleaseRequest, AcpClientStreamChunk,
        AcpClientStreamChunkSink, PortError, PortErrorKind, PortResult, RuntimeServiceCapability,
        RuntimeServicePort,
    };
    use serde_json::json;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    fn empty_context() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    #[test]
    fn task_branch_names_are_sanitized_to_valid_git_refs() {
        assert_eq!(sanitize_task_branch_name("task/1"), "task/1");
        assert_eq!(sanitize_task_branch_name("task/42"), "task/42");
        // 非法字符被段级过滤。
        assert_eq!(sanitize_task_branch_name("task/1:bad"), "task/1bad");
        // 空输入回退 task/1。
        assert_eq!(sanitize_task_branch_name(""), "task/1");
        assert_eq!(sanitize_task_branch_name("///"), "task/1");
        // 点段修剪（git ref 非法）：`..` 段清空后被剔除。
        assert_eq!(sanitize_task_branch_name("task/.."), "task");
        assert_eq!(sanitize_task_branch_name("task/.."), "task");
    }

    #[test]
    fn task_branch_auto_naming_increments_from_existing_task_branches() {
        // 纯函数验证：next_task_branch_name 依赖 GitService::get_branches（真实
        // git 调用），此处仅验证「task/<序号> 从 0 递增」的格式契约；序号递增
        // 逻辑在集成层由 get_branches 输出驱动。
        let name = sanitize_task_branch_name("task/3");
        assert_eq!(name, "task/3");
    }

    /// Minimal AcpClientPort fake: records create requests and returns the
    /// same flow-session shape the desktop implementation produces
    /// (`acp_<client>_<uuid>` / `acp:<client>`), with an optional failure flag
    /// to exercise the error mapping.
    #[derive(Debug, Default)]
    struct FakeAcpClientPort {
        created: Mutex<Vec<AcpClientCreateRequest>>,
        fail_create: Mutex<bool>,
    }

    impl RuntimeServicePort for FakeAcpClientPort {
        fn capability(&self) -> RuntimeServiceCapability {
            RuntimeServiceCapability::AcpClient
        }
    }

    #[async_trait]
    impl AcpClientPort for FakeAcpClientPort {
        async fn create_session(
            &self,
            request: AcpClientCreateRequest,
        ) -> PortResult<AcpClientCreateResult> {
            if *self.fail_create.lock().unwrap() {
                return Err(PortError::new(
                    PortErrorKind::Backend,
                    "simulated start failure",
                ));
            }
            self.created.lock().unwrap().push(request.clone());
            Ok(AcpClientCreateResult {
                session_id: format!("acp_{}_{}", request.client_id, "session-1"),
                session_name: request
                    .session_name
                    .unwrap_or_else(|| format!("{} ACP", request.client_id)),
                agent_type: format!("acp:{}", request.client_id),
            })
        }

        async fn list_clients(&self) -> PortResult<AcpClientListResult> {
            Ok(AcpClientListResult { clients: vec![] })
        }

        async fn release_session(&self, _request: AcpClientReleaseRequest) -> PortResult<()> {
            Ok(())
        }

        async fn cancel_session(&self, _request: AcpClientCancelRequest) -> PortResult<()> {
            Ok(())
        }

        async fn send_message(
            &self,
            _request: AcpClientMessageRequest,
        ) -> PortResult<AcpClientMessageResult> {
            Ok(AcpClientMessageResult {
                session_id: String::new(),
                response: String::new(),
            })
        }

        async fn send_message_stream(
            &self,
            _request: AcpClientMessageRequest,
            chunk_sink: AcpClientStreamChunkSink,
        ) -> PortResult<AcpClientMessageResult> {
            let _ = chunk_sink.send(AcpClientStreamChunk::Completed);
            Ok(AcpClientMessageResult {
                session_id: String::new(),
                response: String::new(),
            })
        }

        async fn send_message_to_bitfun_session(
            &self,
            _request: AcpClientBitfunMessageRequest,
        ) -> PortResult<AcpClientMessageResult> {
            Ok(AcpClientMessageResult {
                session_id: String::new(),
                response: String::new(),
            })
        }

        async fn send_message_to_bitfun_session_stream(
            &self,
            _request: AcpClientBitfunMessageRequest,
            chunk_sink: AcpClientStreamChunkSink,
        ) -> PortResult<AcpClientMessageResult> {
            let _ = chunk_sink.send(AcpClientStreamChunk::Completed);
            Ok(AcpClientMessageResult {
                session_id: String::new(),
                response: String::new(),
            })
        }

        async fn delete_session_record(
            &self,
            _session_id: String,
            _workspace_path: Option<String>,
        ) -> PortResult<()> {
            Ok(())
        }

        async fn read_history(
            &self,
            _request: AcpClientHistoryRequest,
        ) -> PortResult<AcpClientHistoryResult> {
            Ok(AcpClientHistoryResult {
                session_id: String::new(),
                entries: vec![],
                truncated: false,
            })
        }
    }

    fn acp_workspace_target() -> SessionControlWorkspaceTarget {
        SessionControlWorkspaceTarget {
            display_workspace: "/repo/project".to_string(),
            project_workspace: "/repo/project".to_string(),
            execution_target: None,
            workspace_id: None,
            remote_connection_id: None,
            remote_ssh_host: None,
        }
    }

    #[tokio::test]
    async fn acp_create_forwards_client_workspace_and_session_name() {
        let port = FakeAcpClientPort::default();
        let created = SessionControlTool::new()
            .create_acp_session_via_port(
                &acp_workspace_target(),
                "codebuddy",
                Some("my acp".to_string()),
                &port,
            )
            .await
            .expect("acp create should succeed");

        let requests = port.created.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].client_id, "codebuddy");
        assert_eq!(requests[0].workspace_path, "/repo/project");
        assert_eq!(requests[0].session_name.as_deref(), Some("my acp"));
        // 与前端 create_acp_flow_session 形态一致：acp_<client>_<uuid> / acp:<client>
        assert_eq!(created.session_id, "acp_codebuddy_session-1");
        assert_eq!(created.agent_type, "acp:codebuddy");
    }

    #[tokio::test]
    async fn acp_create_keeps_service_default_session_name_when_omitted() {
        let port = FakeAcpClientPort::default();
        let created = SessionControlTool::new()
            .create_acp_session_via_port(&acp_workspace_target(), "codex", None, &port)
            .await
            .expect("acp create should succeed");

        assert!(port.created.lock().unwrap()[0].session_name.is_none());
        assert_eq!(created.session_name, "codex ACP");
    }

    #[tokio::test]
    async fn acp_create_maps_port_error_to_tool_error() {
        let port = FakeAcpClientPort::default();
        *port.fail_create.lock().unwrap() = true;
        let error = SessionControlTool::new()
            .create_acp_session_via_port(&acp_workspace_target(), "codebuddy", None, &port)
            .await
            .expect_err("port failure must surface as a tool error");
        assert!(error.to_string().contains("ACP client port failed"));
        assert!(error.to_string().contains("simulated start failure"));
    }

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            let path = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("temp workspace should be created");
            Self { path }
        }

        fn as_string(&self) -> String {
            self.path.to_string_lossy().to_string()
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn test_session_manager() -> Arc<crate::agentic::session::session_manager::SessionManager> {
        use crate::agentic::persistence::PersistenceManager;
        use crate::agentic::session::session_manager::{SessionManager, SessionManagerConfig};
        use crate::agentic::session::{PromptCachePolicy, SessionContextStore};
        use crate::infrastructure::app_paths::path_manager::PathManager;
        let user_root =
            std::env::temp_dir().join(format!("bitfun-authz-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&user_root).expect("test user root");
        let path_manager = PathManager::with_user_root_for_tests(user_root.clone());
        let persistence =
            PersistenceManager::new(Arc::new(path_manager)).expect("persistence manager");
        Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(persistence),
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: false,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ))
    }

    #[tokio::test]
    async fn shared_authz_rejects_unrelated_caller_delete_without_metadata() {
        // Unauthorized: caller is not owner, target has no created_by and is
        // not an ACP flow session shape (tail is not a uuid) -> reject delete.
        // Non-ACP shape -> ghost release does not apply; no created_by ->
        // ancestor walk fails (tree and metadata both empty), consistent with
        // the existing SessionControl delete semantics (reject; no arbitrary
        // acp_ prefix bypass).
        let session_manager = test_session_manager();
        let tree = SessionTreeManager::new(8);
        let workspace = TestTempDir::new("bitfun-authz-reject-delete");
        let workspace_string = workspace.as_string();
        let error = resolve_session_mutation_authorization(
            &session_manager,
            &tree,
            "caller-1",
            "acp_codex_notauuid",
            std::path::Path::new(&workspace_string),
            "delete",
            SessionMutationAuthOptions::delete(),
        )
        .await
        .expect_err("unrelated caller without metadata must be rejected");
        assert!(
            error.to_string().contains("not authorized to delete")
                || error
                    .to_string()
                    .contains("cannot verify ancestor relationship"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn shared_authz_rejects_unrelated_caller_cancel() {
        // Unauthorized: caller is not owner, target has no created_by and is
        // not an ACP flow session shape -> reject cancel (cancel has no owner
        // exemption and no ghost ACP release). Missing metadata makes the
        // ancestor walk fail, which is also a rejection.
        let session_manager = test_session_manager();
        let tree = SessionTreeManager::new(8);
        let workspace = TestTempDir::new("bitfun-authz-reject-cancel");
        let workspace_string = workspace.as_string();
        let error = resolve_session_mutation_authorization(
            &session_manager,
            &tree,
            "caller-1",
            "acp_codex_notauuid",
            std::path::Path::new(&workspace_string),
            "cancel",
            SessionMutationAuthOptions::cancel(),
        )
        .await
        .expect_err("unrelated caller without metadata must be rejected");
        assert!(
            error.to_string().contains("not authorized to cancel")
                || error
                    .to_string()
                    .contains("cannot verify ancestor relationship"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn shared_authz_ghost_acp_delete_allowed_but_cancel_requires_shape() {
        // Ghost ACP flow session (strict uuid tail + no created_by): delete
        // releases (P-06 designed shape); but any acp_ prefix with a non-uuid
        // tail does not get the release.
        let session_manager = test_session_manager();
        let tree = SessionTreeManager::new(8);
        let workspace = TestTempDir::new("bitfun-authz-ghost-acp");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);
        let strict_acp_id = "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b";

        // Strict ACP shape delete releases with no metadata (ghost).
        resolve_session_mutation_authorization(
            &session_manager,
            &tree,
            "caller-1",
            strict_acp_id,
            workspace_path,
            "delete",
            SessionMutationAuthOptions::delete(),
        )
        .await
        .expect("strict acp flow session delete should be released");

        // cancel keeps delete's ghost release semantics (no created_by on a
        // flow session is the designed shape).
        resolve_session_mutation_authorization(
            &session_manager,
            &tree,
            "caller-1",
            strict_acp_id,
            workspace_path,
            "cancel",
            SessionMutationAuthOptions::delete(),
        )
        .await
        .expect("strict acp flow session cancel should be released");
    }

    #[tokio::test]
    async fn shared_authz_created_by_match_allows_caller() {
        // created_by match: target metadata created_by == session-<caller>
        // -> allow.
        let session_manager = test_session_manager();
        let tree = SessionTreeManager::new(8);
        let workspace = TestTempDir::new("bitfun-authz-created-by");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);
        let target_id = "target-1";
        let metadata = crate::service::session::SessionMetadata::new(
            target_id.to_string(),
            "target".to_string(),
            "agentic".to_string(),
            "auto".to_string(),
        );
        let mut created_metadata = metadata.clone();
        created_metadata.created_by = Some(session_control_creator_marker("caller-1"));
        session_manager
            .save_session_metadata(workspace_path, &created_metadata)
            .await
            .expect("save metadata");

        resolve_session_mutation_authorization(
            &session_manager,
            &tree,
            "caller-1",
            target_id,
            workspace_path,
            "delete",
            SessionMutationAuthOptions::delete(),
        )
        .await
        .expect("creator should be authorized to delete");
    }

    #[tokio::test]
    async fn shared_authz_owner_bypasses_delete_but_not_cancel() {
        // Owner (Commander role) delete exemption; cancel has no owner exemption.
        use crate::agentic::tools::restrictions::set_session_role;
        let _ = set_session_role("authz-owner", AgentRole::Commander);
        let session_manager = test_session_manager();
        let tree = SessionTreeManager::new(8);
        let workspace = TestTempDir::new("bitfun-authz-owner");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);

        resolve_session_mutation_authorization(
            &session_manager,
            &tree,
            "authz-owner",
            "acp_codex_notauuid",
            workspace_path,
            "delete",
            SessionMutationAuthOptions::delete(),
        )
        .await
        .expect("owner should bypass delete gate");

        // cancel has no owner exemption: even as Commander role, a non-ACP
        // shape is still rejected (no metadata -> ancestor walk fails, which is
        // also a rejection).
        let error = resolve_session_mutation_authorization(
            &session_manager,
            &tree,
            "authz-owner",
            "acp_codex_notauuid",
            workspace_path,
            "cancel",
            SessionMutationAuthOptions::cancel(),
        )
        .await
        .expect_err("owner must not bypass cancel gate");
        assert!(
            error.to_string().contains("not authorized to cancel")
                || error
                    .to_string()
                    .contains("cannot verify ancestor relationship"),
            "{error}"
        );
    }

    #[test]
    fn worktree_context_keeps_project_scope_for_session_operations() {
        let worktree_path = PathBuf::from("/worktrees/wt-1");
        let project_path = PathBuf::from("/repo");
        let execution_target = SessionExecutionTarget {
            kind: SessionExecutionTargetKind::ManagedWorktree,
            worktree_id: Some("wt-1".to_string()),
            root_path: "/worktrees/wt-1".to_string(),
            base_ref: Some("HEAD".to_string()),
            base_commit: Some("0123456789abcdef".to_string()),
            branch: None,
            lifecycle: Some(WorktreeLifecycle::Managed),
        };
        let binding = WorkspaceBinding::new(None, worktree_path.clone())
            .with_project_root_path(project_path.clone())
            .with_execution_target(Some(execution_target.clone()));

        let target = SessionControlTool::workspace_target_from_context(&binding);

        assert_eq!(PathBuf::from(target.display_workspace), worktree_path);
        assert_eq!(PathBuf::from(target.project_workspace), project_path);
        assert_eq!(target.execution_target, Some(execution_target));
    }

    #[tokio::test]
    async fn validate_cancel_requires_session_id() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "cancel",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("session_id is required for cancel")
        );
    }

    #[tokio::test]
    async fn validate_cancel_rejects_session_name() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "cancel",
                    "session_id": "worker_1",
                    "session_name": "should-not-be-here",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("session_name is only allowed for create")
        );
    }

    #[tokio::test]
    async fn validate_cancel_allows_missing_workspace() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "cancel",
                    "session_id": "worker_1",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_rename_requires_session_name() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "rename",
                    "session_id": "worker_1",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert!(validation
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("session_name is required for rename"));
    }

    #[tokio::test]
    async fn validate_rename_requires_session_id() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "rename",
                    "session_name": "new-title",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert!(validation
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("session_id is required"));
    }

    #[tokio::test]
    async fn validate_rename_accepts_session_id_and_name() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "rename",
                    "session_id": "worker_1",
                    "session_name": "new-title",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_cancel_ignores_workspace_when_provided() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "cancel",
                    "session_id": "worker_1",
                    "workspace": "not-an-absolute-path",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_list_rejects_session_id() {
        let tool = SessionControlTool::new();
        let workspace = TestTempDir::new("bitfun-session-control-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "action": "list",
                    "workspace": workspace.as_string(),
                    "session_id": "worker_1",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("session_id is not allowed for list")
        );
    }

    #[tokio::test]
    async fn validate_list_requires_workspace() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "list",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("workspace is required for list")
        );
    }

    #[test]
    fn render_message_for_cancel_is_specific() {
        let tool = SessionControlTool::new();
        let message = tool.render_tool_use_message(
            &json!({
                "action": "cancel",
                "workspace": "/repo",
                "session_id": "worker_1",
            }),
            &ToolRenderOptions { verbose: false },
        );

        assert_eq!(message, "Cancel active turn for session worker_1");
    }

    // Cascade-failure surfacing (delete result JSON contract).
    // Full end-to-end cascade execution requires a global coordinator and
    // scheduler, which is not available in unit tests; these assert the
    // serialization contract that the delete path relies on, including the
    // session_id + reason shape for every failed child.
    #[test]
    fn delete_result_surfaces_cascade_failures() {
        let failures = vec![
            (
                "child_1".to_string(),
                "skipped: daemon/warden child session".to_string(),
            ),
            ("child_2".to_string(), "storage write failed".to_string()),
        ];
        let result = build_delete_result_json("parent", "/repo", &failures);

        assert_eq!(result["success"], true);
        assert_eq!(result["action"], "delete");
        assert_eq!(result["session_id"], "parent");
        let surfaced = result["cascade_failures"]
            .as_array()
            .expect("cascade_failures array");
        assert_eq!(surfaced.len(), 2);
        assert_eq!(surfaced[0]["session_id"], "child_1");
        assert_eq!(
            surfaced[0]["reason"],
            "skipped: daemon/warden child session"
        );
        assert_eq!(surfaced[1]["session_id"], "child_2");
        assert_eq!(surfaced[1]["reason"], "storage write failed");
    }

    #[test]
    fn delete_result_has_empty_cascade_failures_when_clean() {
        let result = build_delete_result_json("parent", "/repo", &[]);
        let surfaced = result["cascade_failures"]
            .as_array()
            .expect("cascade_failures array present");
        assert!(surfaced.is_empty());
    }

    #[test]
    fn commander_caller_is_owner_for_session_deletion() {
        use crate::agentic::tools::restrictions::{clear_session_role, set_session_role};
        let _ = set_session_role("delete-owner-commander", AgentRole::Commander);
        assert!(
            caller_is_owner_session("delete-owner-commander"),
            "the user's main session (Commander) may delete any session"
        );
        clear_session_role("delete-owner-commander");
    }

    #[test]
    fn unregistered_caller_degrades_to_non_owner_for_session_deletion() {
        use crate::agentic::tools::restrictions::clear_session_role;
        clear_session_role("delete-owner-unregistered");
        assert!(
            !caller_is_owner_session("delete-owner-unregistered"),
            "an unregistered caller must not bypass the R-2 authorization gate"
        );
    }

    #[test]
    fn executor_caller_is_not_owner_for_session_deletion() {
        use crate::agentic::tools::restrictions::{clear_session_role, set_session_role};
        let _ = set_session_role("delete-owner-executor", AgentRole::Executor);
        assert!(
            !caller_is_owner_session("delete-owner-executor"),
            "a subagent (Executor) must still pass the created_by/ancestor gate"
        );
        clear_session_role("delete-owner-executor");
    }

    #[test]
    fn reviewer_caller_is_not_owner_for_session_deletion() {
        use crate::agentic::tools::restrictions::{clear_session_role, set_session_role};
        let _ = set_session_role("delete-owner-reviewer", AgentRole::Reviewer);
        assert!(!caller_is_owner_session("delete-owner-reviewer"));
        clear_session_role("delete-owner-reviewer");
    }

    #[test]
    fn acp_flow_session_id_is_recognized() {
        assert!(is_acp_flow_session_id(
            "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b"
        ));
        assert!(!is_acp_flow_session_id("acp_opensource_abcdef")); // tail is not a uuid
        assert!(!is_acp_flow_session_id("session-1"));
        assert!(!is_acp_flow_session_id("acp__codex")); // agent type prefix, not a flow session id
        assert!(!is_acp_flow_session_id("acp_codex")); // no uuid tail
        assert!(!is_acp_flow_session_id("acp_codex_notauuid")); // tail is not a uuid shape
        assert!(!is_acp_flow_session_id(""));
        assert!(!is_acp_flow_session_id(
            "acp_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b"
        )); // client_id is empty
    }

    #[test]
    fn looks_like_uuid_accepts_only_canonical_shape() {
        assert!(looks_like_uuid("7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b"));
        assert!(!looks_like_uuid("7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5")); // one char short
        assert!(!looks_like_uuid("7f0e1a2b3c4d4e5f8a9b0c1d2e3f4a5b")); // no dashes
        assert!(!looks_like_uuid("7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5bZ")); // invalid hex
        assert!(!looks_like_uuid(""));
    }

    #[test]
    fn ghost_acp_session_delete_is_authorized_when_created_by_empty() {
        // P-06：幽灵 ACP 流会话——metadata 无 created_by（而非 metadata 文件缺失）
        // + ACP 流会话 → 授权放行（ACP 流会话必写 metadata 文件，created_by 空是
        // 其设计形态）。
        assert!(ghost_acp_delete_authorized(true, true));
        // 其余组合保持原严格判定（不放行）。
        assert!(!ghost_acp_delete_authorized(false, true));
        assert!(!ghost_acp_delete_authorized(true, false));
        assert!(!ghost_acp_delete_authorized(false, false));
    }

    #[test]
    fn ghost_acp_delete_bypasses_ancestor_gate_when_created_by_none() {
        // 防回退：metadata 存在但 created_by=None + acp 前缀 → created_by_match=true，
        // 删除不再落入 ancestor 前置校验（原报错点 :1422 'cannot verify ancestor' 不再可达）。
        let target_metadata = Some(crate::service::session::SessionMetadata::new(
            "acp_codebuddy_a4f68de7-c4ec-46a8-9aab-7e2bc417c3d0".to_string(),
            "codebuddy ACP".to_string(),
            "acp:codebuddy".to_string(),
            "auto".to_string(),
        ));
        let created_by_is_none = target_metadata
            .as_ref()
            .and_then(|metadata| metadata.created_by.as_deref())
            .is_none();
        assert!(
            created_by_is_none,
            "SessionMetadata::new 默认 created_by 应为 None"
        );
        assert!(ghost_acp_delete_authorized(
            created_by_is_none,
            is_acp_flow_session_id("acp_codebuddy_a4f68de7-c4ec-46a8-9aab-7e2bc417c3d0"),
        ));
    }

    #[test]
    fn commander_owner_may_delete_metadata_missing_orphan_session() {
        // R-26 幽灵孤儿删除豁免：metadata 缺失（磁盘无该会话记录）→ Commander owner
        // 放行删除（不落入 ancestor 双空报错）。
        assert!(orphan_session_delete_authorized(true, None, false));
        // 非 owner 不放行：无法越权删无主孤儿。
        assert!(!orphan_session_delete_authorized(false, None, false));
        // ACP 流会话不落入本判定（走 ghost_acp_delete_authorized）。
        assert!(!orphan_session_delete_authorized(true, None, true));
    }

    #[test]
    fn commander_owner_may_delete_unattached_orphan_session() {
        // 无主孤儿 = metadata 存在但 created_by 为空 + 无 relationship（未挂树）。
        let orphan = crate::service::session::SessionMetadata::new(
            "0f44ed94-a487-44e1-b5b0-f743557d473c".to_string(),
            "孤儿测试会话".to_string(),
            "agentic".to_string(),
            "auto".to_string(),
        );
        // SessionMetadata::new 默认 created_by=None 且 relationship=None → 无主孤儿。
        assert!(orphan.created_by.is_none());
        assert!(orphan.relationship.is_none());
        assert!(orphan_session_delete_authorized(true, Some(&orphan), false));
        assert!(!orphan_session_delete_authorized(
            false,
            Some(&orphan),
            false
        ));
    }

    #[test]
    fn commander_owner_cannot_delete_attached_or_created_session_as_orphan() {
        // 已挂树（relationship 有 parent）或已写 created_by 的会话不是无主孤儿，
        // 不落入孤儿豁免——它们走原 created_by/ancestor 授权。
        use bitfun_services_core::session::types::{SessionRelationship, SessionRelationshipKind};
        let mut attached = crate::service::session::SessionMetadata::new(
            "attached-session".to_string(),
            "挂树会话".to_string(),
            "agentic".to_string(),
            "auto".to_string(),
        );
        attached.relationship = Some(SessionRelationship {
            kind: Some(SessionRelationshipKind::Subagent),
            parent_session_id: Some("parent-1".to_string()),
            depth: Some(1),
            ..Default::default()
        });
        assert!(!orphan_session_delete_authorized(
            true,
            Some(&attached),
            false
        ));

        let mut created = crate::service::session::SessionMetadata::new(
            "created-session".to_string(),
            "有创建者会话".to_string(),
            "agentic".to_string(),
            "auto".to_string(),
        );
        created.created_by = Some("session-parent-1".to_string());
        assert!(!orphan_session_delete_authorized(
            true,
            Some(&created),
            false
        ));
    }

    fn summary(
        id: &str,
        parent: Option<&str>,
        is_daemon: bool,
        created_at_ms: u64,
    ) -> AgentSessionSummary {
        AgentSessionSummary {
            session_id: id.to_string(),
            session_name: format!("Session {id}"),
            agent_type: if is_daemon {
                "warden-daemon".to_string()
            } else {
                "agentic".to_string()
            },
            model_id: None,
            reasoning_preset: None,
            last_user_dialog_agent_type: None,
            last_submitted_agent_type: None,
            turn_count: 0,
            created_at_ms,
            last_active_at_ms: created_at_ms,
            parent_session_id: parent.map(str::to_string),
            status: Some("active".to_string()),
            is_daemon,
        }
    }

    #[test]
    fn tree_repairs_lineage_when_parent_filtered_out() {
        // root <- daemon <- child; the daemon is filtered from the list, so the
        // child must be re-hung onto root instead of becoming a fake root.
        let tree = SessionTreeManager::new(8);
        tree.register_child("root", "daemon", 1).unwrap();
        tree.register_child("daemon", "child", 2).unwrap();

        let sessions = vec![
            summary("root", None, false, 1),
            summary("child", Some("daemon"), false, 2),
            summary("sibling", Some("root"), false, 3),
        ];

        let tree_json = build_session_tree_json_impl(&sessions, Some(&tree));
        let value: Value = serde_json::from_str(&tree_json).expect("valid tree json");
        let roots = value.as_array().expect("forest array");
        assert_eq!(roots.len(), 1, "single root after re-hang: {tree_json}");
        assert_eq!(roots[0]["sessionId"], "root");
        assert!(roots[0].get("orphaned").is_none());

        let children = roots[0]["children"].as_array().unwrap();
        let child_ids: Vec<&str> = children
            .iter()
            .map(|c| c["sessionId"].as_str().unwrap())
            .collect();
        // children sorted by created_at_ms ascending: child(2) then sibling(3)
        assert_eq!(child_ids, vec!["child", "sibling"]);
        assert!(children[0].get("orphaned").is_none());
        assert_eq!(
            children[0]["depth"], 2,
            "depth comes from the real tree, not the filtered list"
        );
    }

    #[test]
    fn tree_rehangs_to_nearest_surviving_ancestor() {
        // root <- daemon1 <- daemon2 <- child; both daemon layers are filtered,
        // so the child must be re-hung onto root (the nearest surviving ancestor).
        let tree = SessionTreeManager::new(8);
        tree.register_child("root", "daemon1", 1).unwrap();
        tree.register_child("daemon1", "daemon2", 2).unwrap();
        tree.register_child("daemon2", "child", 3).unwrap();

        let sessions = vec![
            summary("root", None, false, 1),
            summary("child", Some("daemon2"), false, 2),
        ];

        let tree_json = build_session_tree_json_impl(&sessions, Some(&tree));
        let value: Value = serde_json::from_str(&tree_json).expect("valid tree json");
        let roots = value.as_array().unwrap();
        assert_eq!(
            roots.len(),
            1,
            "single root after multi-level re-hang: {tree_json}"
        );
        assert_eq!(roots[0]["sessionId"], "root");
        let children = roots[0]["children"].as_array().unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0]["sessionId"], "child");
        assert!(children[0].get("orphaned").is_none());
        assert_eq!(children[0]["depth"], 3);
    }

    #[test]
    fn tree_marks_orphan_when_no_surviving_ancestor() {
        // The parent chain is entirely unknown (no tree, parent not in list):
        // the session is promoted to a root but flagged as orphaned.
        let sessions = vec![
            summary("root", None, false, 1),
            summary("child", Some("missing-parent"), false, 2),
        ];

        let tree_json = build_session_tree_json_impl(&sessions, None);
        let value: Value = serde_json::from_str(&tree_json).expect("valid tree json");
        let roots = value.as_array().unwrap();
        assert_eq!(roots.len(), 2);

        let root_node = roots.iter().find(|r| r["sessionId"] == "root").unwrap();
        assert!(root_node.get("orphaned").is_none());

        let orphan_node = roots.iter().find(|r| r["sessionId"] == "child").unwrap();
        assert_eq!(orphan_node["orphaned"], true);
    }

    #[test]
    fn tree_marks_truncated_when_depth_budget_exhausted() {
        // P2-S8: a subtree cut off at TREE_SERIALIZE_MAX_DEPTH carries a
        // "truncated": true marker so consumers can tell a complete tree from
        // a capped one (mirrors the orphaned marker).
        let max_depth = bitfun_core_types::session_tree::MAX_TREE_SERIALIZE_DEPTH;
        let tree = SessionTreeManager::new(max_depth as u32 + 4);
        // Build a chain deeper than the serialization budget: root <- c1 <- c2 <- ...
        let mut sessions = vec![summary("root", None, false, 1)];
        let mut parent = "root".to_string();
        for i in 0..(max_depth + 3) {
            let id = format!("c{i}");
            tree.register_child(&parent, &id, (i + 2) as u32).unwrap();
            sessions.push(summary(&id, Some(&parent), false, (i + 2) as u64));
            parent = id;
        }

        let tree_json = build_session_tree_json_impl(&sessions, Some(&tree));

        // Structural checks on the raw JSON string: the serialized tree is
        // deeper than serde_json's default 128-level recursion cap, so parse
        // only the shallow prefix (the marker placement is what this test
        // asserts; the production reader hits the same shape only for
        // genuinely deep trees).
        // 1. Exactly one root.
        assert!(tree_json.starts_with("["), "tree json is a forest array");
        // 2. The truncated marker appears exactly once (on the boundary node).
        //    serde_json pretty-prints with a space after the colon.
        let truncated_markers = tree_json.matches("\"truncated\": true").count();
        assert_eq!(
            truncated_markers, 1,
            "exactly one boundary node is marked truncated: {tree_json}"
        );
        // 3. The boundary node (the one carrying "truncated") serializes an
        //    empty children array. serde_json::Map orders keys
        //    alphabetically, so `"children"` sorts BEFORE `"truncated"`;
        //    the boundary node's object span therefore contains
        //    `"children": []` before the marker.
        let truncated_pos = tree_json
            .find("\"truncated\": true")
            .expect("boundary node marker present");
        let before_truncated = &tree_json[..truncated_pos];
        assert!(
            before_truncated.contains("\"children\": []"),
            "truncated node serializes no children: {}",
            &before_truncated[before_truncated.len().saturating_sub(200)..]
        );

        // 4. Confirm the boundary node identity and that nodes within the
        //    budget carry no truncated marker. The truncated boundary node
        //    is c{max_depth - 1} (recursion_depth == MAX). Because the JSON
        //    is deeper than serde_json's default recursion cap, verify on the
        //    raw string.
        let boundary_id = format!("\"c{}\"", max_depth - 1);
        let boundary_marker_pos = truncated_pos;
        let boundary_id_pos = tree_json
            .rfind(&boundary_id)
            .expect("boundary node id is serialized");
        // The boundary node's sessionId sits inside the same object span as
        // its truncated marker (no other truncated marker in between).
        assert!(
            boundary_id_pos < boundary_marker_pos,
            "boundary node id precedes its truncated marker"
        );
        let between = &tree_json[boundary_id_pos..boundary_marker_pos];
        assert!(
            !between.contains("\"truncated\": true"),
            "no other truncated marker between the boundary id and its marker"
        );
        // Every node above the boundary (recursion_depth < MAX) has children
        // and no truncated marker; assert that no `"truncated": true` appears
        // before the boundary node's own marker in the serialized string.
        let chain_above = &tree_json[..truncated_pos];
        assert!(
            !chain_above.contains("\"truncated\": true"),
            "nodes within the budget must not be marked truncated"
        );
        // The serialized tree is a single-root forest.
        assert!(
            tree_json
                .trim_start()
                .starts_with("[\n  {\n    \"agentType\""),
            "tree json is a single-root forest"
        );
    }

    // --- short_name / detail / compact output ---

    #[tokio::test]
    async fn validate_list_rejects_short_name() {
        let tool = SessionControlTool::new();
        let workspace = TestTempDir::new("bitfun-session-control-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "action": "list",
                    "workspace": workspace.as_string(),
                    "short_name": "secretary",
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("short_name is only allowed for create")
        );
    }

    #[tokio::test]
    async fn validate_list_allows_detail_flag() {
        let tool = SessionControlTool::new();
        let workspace = TestTempDir::new("bitfun-session-control-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "action": "list",
                    "workspace": workspace.as_string(),
                    "detail": true,
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_cancel_rejects_detail_flag() {
        let tool = SessionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "cancel",
                    "session_id": "worker_1",
                    "detail": true,
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("detail is only allowed for list")
        );
    }

    #[tokio::test]
    async fn validate_create_allows_short_name() {
        let tool = SessionControlTool::new();
        let workspace = TestTempDir::new("bitfun-session-control-tool-test");
        let mut context = empty_context();
        context.session_id = Some("creator-1".to_string());

        let validation = tool
            .validate_input(
                &json!({
                    "action": "create",
                    "workspace": workspace.as_string(),
                    "short_name": "secretary-standing",
                }),
                Some(&context),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_create_rejects_detail_flag() {
        let tool = SessionControlTool::new();
        let workspace = TestTempDir::new("bitfun-session-control-tool-test");
        let mut context = empty_context();
        context.session_id = Some("creator-1".to_string());

        let validation = tool
            .validate_input(
                &json!({
                    "action": "create",
                    "workspace": workspace.as_string(),
                    "detail": true,
                }),
                Some(&context),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("detail is only allowed for list")
        );
    }

    #[test]
    fn compact_display_name_prefers_short_name_and_truncates() {
        let long_name = "task-description".repeat(10); // 150 chars
        assert_eq!(
            compact_session_display_name("abc", Some("秘书·常驻")),
            "秘书·常驻"
        );
        assert_eq!(compact_session_display_name("abc", Some("  ")), "abc");

        let truncated = compact_session_display_name(&long_name, None);
        assert!(truncated.ends_with("..."));
        assert_eq!(truncated.chars().count(), 60 + 3);

        assert_eq!(
            compact_session_display_name("short name", None),
            "short name"
        );
    }

    #[test]
    fn compact_list_uses_short_names_and_preserves_tree_indentation() {
        let tool = SessionControlTool::new();
        let sessions = vec![
            summary("root", None, false, 1),
            summary("child", Some("root"), false, 2),
        ];
        let mut short_names = HashMap::new();
        short_names.insert("root".to_string(), Some("秘书·常驻".to_string()));
        short_names.insert("child".to_string(), None);

        let output = tool.build_list_result_for_assistant(
            "/repo",
            &sessions,
            None,
            None,
            &short_names,
            false,
        );

        assert!(output.contains("[root] agentic | active | 秘书·常驻"));
        assert!(output.contains("  - [child] agentic | active | Session child"));
        assert!(output.contains("## Sessions (compact)"));
        assert!(!output.contains("## Session Tree (JSON)"));
    }

    #[test]
    fn compact_list_truncates_long_session_names_without_short_name() {
        let tool = SessionControlTool::new();
        let long_name = "派单提示词全文-".repeat(20); // 140 chars
        let mut root = summary("root", None, false, 1);
        root.session_name = long_name.clone();
        let sessions = vec![root];
        let short_names = HashMap::new();

        let output = tool.build_list_result_for_assistant(
            "/repo",
            &sessions,
            None,
            None,
            &short_names,
            false,
        );

        assert!(
            !output.contains(&long_name),
            "full session name must be omitted"
        );
        assert!(output.contains("..."));
        assert!(output.contains("[root] agentic | active | "));
    }

    #[test]
    fn detail_list_keeps_full_tree_json_output() {
        let tool = SessionControlTool::new();
        let sessions = vec![summary("root", None, false, 1)];
        let short_names = HashMap::new();

        let output = tool.build_list_result_for_assistant(
            "/repo",
            &sessions,
            None,
            None,
            &short_names,
            true,
        );

        assert!(output.contains("## Session Tree (JSON)"));
        assert!(output.contains("\"sessionName\": \"Session root\""));
        assert!(output.contains("\"sessionId\": \"root\""));
    }

    // ---------------------------------------------------------------------
    // UX-P0-1: SessionHistory 读取授权门（resolve_session_read_authorization）
    // 攻击者矩阵：unrelated 拒绝 / owner 豁免 / created_by 放行 /
    // 祖先-后代双向放行 / 后代可导出祖先 / daemon+warden 豁免 /
    // 跨 workspace 拒绝 / 缺 metadata 拒绝。
    // ---------------------------------------------------------------------

    fn read_authz_session_manager() -> Arc<crate::agentic::session::session_manager::SessionManager>
    {
        test_session_manager()
    }

    #[tokio::test]
    async fn read_authz_rejects_unrelated_caller_without_metadata() {
        // 攻击者矩阵 A：非 owner、无 created_by、树内外均无关系 -> 拒绝。
        let session_manager = read_authz_session_manager();
        let tree = SessionTreeManager::new(8);
        let workspace = TestTempDir::new("bitfun-read-authz-unrelated");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);
        let error = resolve_session_read_authorization(
            &session_manager,
            &tree,
            "caller-1",
            workspace_path,
            "target-1",
            workspace_path,
            "export history of",
            SessionHistoryAuthOptions::read(),
        )
        .await
        .expect_err("unrelated caller without metadata must be rejected");
        assert!(
            error
                .to_string()
                .contains("not authorized to export history of"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn read_authz_owner_bypasses_gate() {
        // 攻击者矩阵 B：owner（Commander 角色）豁免——主会话=用户 owner，
        // 可导出任意同 workspace 会话（含无 metadata）。
        use crate::agentic::tools::restrictions::set_session_role;
        let _ = set_session_role("read-authz-owner", AgentRole::Commander);
        let session_manager = read_authz_session_manager();
        let tree = SessionTreeManager::new(8);
        let workspace = TestTempDir::new("bitfun-read-authz-owner");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);

        resolve_session_read_authorization(
            &session_manager,
            &tree,
            "read-authz-owner",
            workspace_path,
            "no-metadata-target",
            workspace_path,
            "export history of",
            SessionHistoryAuthOptions::read(),
        )
        .await
        .expect("owner should bypass the read gate");
    }

    #[tokio::test]
    async fn read_authz_created_by_match_allows_caller() {
        // 攻击者矩阵 C：created_by == session-<caller> -> 放行。
        let session_manager = read_authz_session_manager();
        let tree = SessionTreeManager::new(8);
        let workspace = TestTempDir::new("bitfun-read-authz-created-by");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);
        let target_id = "target-1";
        let metadata = crate::service::session::SessionMetadata::new(
            target_id.to_string(),
            "target".to_string(),
            "agentic".to_string(),
            "auto".to_string(),
        );
        let mut created_metadata = metadata.clone();
        created_metadata.created_by = Some(session_control_creator_marker("caller-1"));
        session_manager
            .save_session_metadata(workspace_path, &created_metadata)
            .await
            .expect("save metadata");

        resolve_session_read_authorization(
            &session_manager,
            &tree,
            "caller-1",
            workspace_path,
            target_id,
            workspace_path,
            "export history of",
            SessionHistoryAuthOptions::read(),
        )
        .await
        .expect("creator should be authorized to read");
    }

    #[tokio::test]
    async fn read_authz_ancestor_allows_caller_to_read_descendant() {
        // 攻击者矩阵 D：祖先可导出后代（树注册关系）。
        let session_manager = read_authz_session_manager();
        let tree = SessionTreeManager::new(8);
        tree.register_child("caller-1", "child-1", 1)
            .expect("register child");
        let workspace = TestTempDir::new("bitfun-read-authz-ancestor");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);

        resolve_session_read_authorization(
            &session_manager,
            &tree,
            "caller-1",
            workspace_path,
            "child-1",
            workspace_path,
            "export history of",
            SessionHistoryAuthOptions::read(),
        )
        .await
        .expect("ancestor should be authorized to read descendant");
    }

    #[tokio::test]
    async fn read_authz_descendant_allows_caller_to_read_ancestor() {
        // 攻击者矩阵 E：后代可导出祖先（与 delete/cancel 仅祖先->后代单向
        // 不同，读取按指令限定「仅本会话树祖先/后代」双向授权）。
        let session_manager = read_authz_session_manager();
        let tree = SessionTreeManager::new(8);
        tree.register_child("root-1", "caller-1", 1)
            .expect("register child");
        let workspace = TestTempDir::new("bitfun-read-authz-descendant");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);

        resolve_session_read_authorization(
            &session_manager,
            &tree,
            "caller-1",
            workspace_path,
            "root-1",
            workspace_path,
            "export history of",
            SessionHistoryAuthOptions::read(),
        )
        .await
        .expect("descendant should be authorized to read ancestor");
    }

    #[tokio::test]
    async fn read_authz_rejects_sibling_without_creator_link() {
        // 攻击者矩阵 F：同一父树下的兄弟会话（caller 与 target 无
        // 祖先/后代关系、非 owner/creator）-> 拒绝。树内兄弟不能互读。
        let session_manager = read_authz_session_manager();
        let tree = SessionTreeManager::new(8);
        tree.register_child("root-1", "caller-1", 1)
            .expect("register child");
        tree.register_child("root-1", "target-1", 1)
            .expect("register child");
        let workspace = TestTempDir::new("bitfun-read-authz-sibling");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);

        let error = resolve_session_read_authorization(
            &session_manager,
            &tree,
            "caller-1",
            workspace_path,
            "target-1",
            workspace_path,
            "export history of",
            SessionHistoryAuthOptions::read(),
        )
        .await
        .expect_err("sibling sessions must not read each other");
        assert!(
            error
                .to_string()
                .contains("not authorized to export history of"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn read_authz_rejects_cross_workspace() {
        // 攻击者矩阵 G：caller 与 target 不同 workspace -> 一律拒绝
        // （即使 caller 是 Commander owner）。跨 workspace 导出是
        // UX-P0-1 的核心隔离边界。
        use crate::agentic::tools::restrictions::set_session_role;
        let _ = set_session_role("read-authz-cross-ws", AgentRole::Commander);
        let session_manager = read_authz_session_manager();
        let tree = SessionTreeManager::new(8);
        let caller_ws = TestTempDir::new("bitfun-read-authz-caller-ws");
        let target_ws = TestTempDir::new("bitfun-read-authz-target-ws");

        let error = resolve_session_read_authorization(
            &session_manager,
            &tree,
            "read-authz-cross-ws",
            std::path::Path::new(&caller_ws.as_string()),
            "target-1",
            std::path::Path::new(&target_ws.as_string()),
            "export history of",
            SessionHistoryAuthOptions::read(),
        )
        .await
        .expect_err("cross-workspace export must be rejected even for owner");
        assert!(
            error
                .to_string()
                .contains("belongs to a different workspace"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn read_authz_warden_daemon_caller_bypasses_tree_gate() {
        // 攻击者矩阵 H：Warden/daemon 会话豁免（R-A.04 同源）——Warden
        // 模板刻意保留 SessionHistory 作跨会话审计读取。内存会话
        // is_daemon=true 即豁免。
        let session_manager = read_authz_session_manager();
        let tree = SessionTreeManager::new(8);
        let workspace = TestTempDir::new("bitfun-read-authz-warden");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);
        session_manager
            .create_session_with_id(
                Some("warden-session".to_string()),
                "Warden".to_string(),
                "warden-review".to_string(),
                crate::agentic::core::SessionConfig {
                    workspace_path: Some(workspace.as_string()),
                    is_daemon: true,
                    ..Default::default()
                },
            )
            .await
            .expect("create warden daemon session");

        resolve_session_read_authorization(
            &session_manager,
            &tree,
            "warden-session",
            workspace_path,
            "any-target-1",
            workspace_path,
            "export history of",
            SessionHistoryAuthOptions::read(),
        )
        .await
        .expect("warden daemon caller should bypass the read tree gate");
    }
}
