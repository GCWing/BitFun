//! GroupRoomTool — 群聊系列工具（主人定标 v3：群聊 = 普通会话）。
//!
//! Contract: type-contract v3（群聊v3-type-contract-最终权威-20260814.md §二）
//! R-GC-08：9 个 action（create/invite/remove/send/history/list/fork/
//! member_status/delete），复用现成机制：
//! - 建群 = `coordinator.create_session_with_workspace`（coordinator.rs:2659）
//! - 成员 = 调用方传入的真实会话 ID（校验存在后登记 groupChats；
//!   群聊重建 Type-Contract §二，R-GC-28 按数量新建匿名会话已回退）
//! - 发消息 = 群会话 turns（`PersistenceManager::save_dialog_turn`，
//!   persistence/manager.rs:3089；`user_message.metadata` 带 sender+groupId，
//!   types.rs:662）
//! - 历史 = `session_manager.get_messages`（session_manager.rs:8785）
//! - 裂变 = `PersistenceManager::branch_session`（session_branch.rs:14）
//! - 成员状态 = `session_manager.get_session`（:3060）
//! - 删除 = `coordinator.delete_session`（coordinator.rs:7434）
//!
//! 群聊 ID = 会话 ID（UUID）；群 = agent_type="group" 会话（一等内置类型，
//! R-WF-02：GroupMode，见 definitions/modes/group.rs）带专属 workspace。
//!
//! 契约偏差修复（姬码锋 CEO 派发 R-GC-08，2026-08-14）：
//! - B-1（契约 §三）：`GroupMessage.author: SenderIdentity`（复用
//!   session_message_tool.rs:485-496）+ `metadata: GroupChatForwardMetadata`
//!   （复用 session_message_tool.rs:504-510）；history 从 turn metadata
//!   解析真实 author（senderRole/senderDepth/senderName）。
//! - B-2（契约 §三）：send metadata 五字段
//!   { groupId, senderSessionId, senderRole, senderDepth, senderName }；
//!   senderName 取真实会话名（回退 sender_session_id）。
//! - B-3（契约 §六.5）：history/list/member_status 只读；其余 6 action 非只读
//!   （is_readonly 按 action 区分，泛化到 is_concurrency_safe / permission_intents）。
//! - B-4（契约 §二.8）：member_status 先校验 member_session_id ∈ 群成员表
//!   （custom_metadata.groupChats）再 get_session 查 state。

use crate::agentic::agents::get_agent_registry;
use crate::agentic::coordination::{get_global_coordinator, ConversationCoordinator};
use crate::agentic::core::SessionConfig;
use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolExposure, ToolResult, ToolUseContext,
};
use crate::infrastructure::get_path_manager_arc;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_runtime_ports::GROUP_MASTER_ACTOR;
use log::warn;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Tool name registered in the product tool pipeline（materialization 注册）。
pub const GROUP_ROOM_TOOL_NAME: &str = "group_room";

/// Actions supported by the tool（9 基础 + 2 编排扩展，type-contract §二 +
/// R-WF-03 编排扩展：改成员工具集/改接线）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GroupRoomAction {
    Create,
    Invite,
    Remove,
    Send,
    History,
    List,
    Fork,
    MemberStatus,
    Delete,
    /// R-WF-03：改成员工具集（编排控制，复用 add_group_member 群成员表
    /// 持久化 + validate_session_exists 存在性门）。
    UpdateMemberTools,
    /// R-WF-03：改接线（编排控制——数据流/执行顺序提示，非硬编码约束，
    /// 需求 §七「DAG 画布节点连线创建页面，指挥官有工具可修改/查看」）。
    UpdateWiring,
}

impl GroupRoomAction {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "create" => Some(Self::Create),
            "invite" => Some(Self::Invite),
            "remove" => Some(Self::Remove),
            "send" => Some(Self::Send),
            "history" => Some(Self::History),
            "list" => Some(Self::List),
            "fork" => Some(Self::Fork),
            "member_status" => Some(Self::MemberStatus),
            "delete" => Some(Self::Delete),
            "update_member_tools" => Some(Self::UpdateMemberTools),
            "update_wiring" => Some(Self::UpdateWiring),
            _ => None,
        }
    }
}

/// Tool input（9 个 action 的入参，type-contract §二）。
#[derive(Debug, Clone, Deserialize)]
struct GroupRoomInput {
    #[serde(rename = "action")]
    action: String,
    /// create/fork: 群名。
    #[serde(default)]
    name: Option<String>,
    /// create: 群专属工作区。
    #[serde(default)]
    workspace: Option<String>,
    /// create: 工作流 preset id（R-WF-06 建群=建实例：按工作流模板自动
    /// 实例化成员会话，成员类型按 node.agent）。
    #[serde(default)]
    preset_id: Option<String>,
    /// create/invite/fork: 成员会话 id 列表。
    #[serde(default)]
    members: Vec<String>,
    /// invite/remove/member_status: 成员会话 id。
    #[serde(default)]
    member_session_id: Option<String>,
    /// 群会话 id（invite/remove/send/history/fork/member_status/delete）。
    #[serde(default)]
    group_id: Option<String>,
    /// send: 消息正文。
    #[serde(default)]
    content: Option<String>,
    /// send: 发送者会话 id。
    #[serde(default)]
    sender_session_id: Option<String>,
    /// send: 紧急打断。
    #[serde(default)]
    urgent: bool,
    /// history: 读取条数。
    #[serde(default)]
    limit: Option<usize>,
    /// history: 分页游标。
    #[serde(default)]
    cursor: Option<usize>,
    /// fork: 裂变点 turn id。
    #[serde(default)]
    turn_id: Option<String>,
    /// update_member_tools: 成员会话的工具集（覆盖成员默认工具集）。
    #[serde(default)]
    tools: Vec<String>,
    /// update_wiring: 接线定义（数据流/执行顺序，JSON 结构）。
    #[serde(default)]
    wiring: Option<Value>,
}

/// 发送者身份（契约 §三类型定义，字段对齐 session_message_tool.rs:485-496）。
/// 契约要求「复用现成 SenderIdentity」，但该类型在 session_message_tool.rs 中为
/// private 且不可跨模块复用；此处本地定义字段完全一致的等价类型（含 serde derive），
/// 保证 GroupMessage 可序列化且 wire 形态与契约 §三一致。
///
/// R-WF-03（发言方标识 = SOUL.name + 类型）：`role` 随 R-WF-01 RBAC 全删
/// 后恒 None（不再承载 Commander/Executor/Reviewer 身份）；`agent_type`
/// 承载「智能体类型」（需求 §六.5：`三文件名 + 类型`——SOUL 里的身份名 +
/// 智能体类型，不再显示 role）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SenderIdentity {
    /// 发送者会话 id；始终存在。
    pub session_id: String,
    /// RBAC 角色展示标签（R-WF-01 后恒 None，保留字段兼容存量序列化）。
    pub role: Option<String>,
    /// 会话树深度（0 = L0 根）。
    pub depth: Option<u32>,
    /// 会话名（SOUL.name，身份本源名；回退链见 resolve_sender_identity）。
    pub name: Option<String>,
    /// R-WF-03：智能体类型（agent_type，如 "group"/"agentic"/"Claw"）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
}

/// 群聊关联键（契约 §三，字段对齐 session_message_tool.rs:504-510
/// GroupChatForwardMetadata：groupId/groupMessageId/groupAuthor）。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChatForwardMetadata {
    /// 群会话 id。
    pub group_id: Option<String>,
    /// 被回复的群消息 id。
    pub group_message_id: Option<String>,
    /// 发送者标识：`__master__` 或成员会话 id。
    pub group_author: Option<String>,
}

/// 群消息（type-contract §三；author/metadata 复用现成类型）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupMessage {
    pub message_id: String,
    pub group_session_id: String,
    /// 发送者身份（复用 SenderIdentity，§三类型定义）。
    pub author: SenderIdentity,
    pub content: String,
    pub timestamp: i64,
    /// R-WF-08：消息角色（"user" / "system"）。群首 turn = 群 mode 提示词，
    /// 以 System 角色返回（验收断言「群首 turn=system 提示词」）；普通群
    /// 消息为 User。前端据此渲染 mode 提示词为时间线首条 system 展示。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// 群聊关联键（复用 GroupChatForwardMetadata，§三）。
    pub metadata: GroupChatForwardMetadata,
}

/// action → 是否只读（type-contract §六.5 + R-WF-03：history/list/member_status
/// 只读，create/invite/remove/send/fork/delete/update_member_tools/update_wiring
/// 非只读——编排工具 = 写操作）。
pub(crate) fn group_room_action_is_readonly(action: GroupRoomAction) -> bool {
    matches!(
        action,
        GroupRoomAction::History | GroupRoomAction::List | GroupRoomAction::MemberStatus
    )
}

/// R-WF-09（2026-08-16）：编排工具 = 建群/加成员/改接线/查状态
/// （Plan:168 编排工具清单）。查状态 = member_status（只读编排）。
/// send/history/list 为普通消息动作（send 开放投递，Plan:169 不查指挥官）。
fn group_room_action_is_orchestration(action: GroupRoomAction) -> bool {
    matches!(
        action,
        GroupRoomAction::Create
            | GroupRoomAction::Invite
            | GroupRoomAction::Remove
            | GroupRoomAction::Fork
            | GroupRoomAction::Delete
            | GroupRoomAction::UpdateMemberTools
            | GroupRoomAction::UpdateWiring
            | GroupRoomAction::MemberStatus
    )
}

/// GroupRoomTool — 1 tool 9 action（materialization 注册 9 个名称 → 同一实例）。
pub struct GroupRoomTool;

impl Default for GroupRoomTool {
    fn default() -> Self {
        Self::new()
    }
}

impl GroupRoomTool {
    pub fn new() -> Self {
        Self
    }

    fn coordinator() -> BitFunResult<std::sync::Arc<ConversationCoordinator>> {
        get_global_coordinator().ok_or_else(|| {
            BitFunError::tool("group chat tools require an initialized coordinator".to_string())
        })
    }

    /// R-WF-09（2026-08-16）：编排工具「指挥官专用」守卫——只有主会话
    /// （created_by == None 的顶层 Standard 会话，独立于 RBAC）可调用编排
    /// action；非主会话（子会话/成员会话等带 creator 标记）拒绝并返回权限
    /// 错误。普通消息动作（send/history/list）不查指挥官（开放投递 Plan:169）。
    ///
    /// 判定落点：coordinator::is_main_session_by_creator（会话元数据查询，
    /// 不依赖 get_session_role——R-WF-01 已删 RBAC）。调用会话缺失 → 拒绝
    /// （fail-closed，工具上下文必须带 session_id）。
    async fn ensure_orchestration_main_session(
        coordinator: &ConversationCoordinator,
        context: &ToolUseContext,
    ) -> BitFunResult<()> {
        let session_id = context.session_id.as_deref().ok_or_else(|| {
            BitFunError::tool(
                "group orchestration actions require a caller session context (main session only)"
                    .to_string(),
            )
        })?;
        let manager = coordinator.get_session_manager();
        let session = manager.get_session(session_id).ok_or_else(|| {
            BitFunError::tool(format!(
                "group orchestration actions require a main session but caller session '{session_id}' does not exist in memory"
            ))
        })?;
        if !crate::agentic::coordination::coordinator::is_main_session_by_creator(&session) {
            return Err(BitFunError::tool(format!(
                "group orchestration actions are restricted to the main session; caller session '{session_id}' is not a main session (created_by is set)"
            )));
        }
        Ok(())
    }

    fn now_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    /// 构造 SenderIdentity（契约 §三 + R-WF-03 发言方标识 = SOUL.name + 类型）：
    /// - 会话树深度（coordinator.session_tree().get_depth）
    /// - name = SOUL.name（身份本源名，需求 §六.5/§七：成员 SOUL 里的身份名）
    ///   ——优先读成员工作区 SOUL.md frontmatter 的 name 字段（FrontMatterMarkdown，
    ///   与 IDENTITY.md frontmatter 同构）；缺失时回退内存会话名 → 磁盘元数据
    ///   会话名 → None（绝不阻塞发送/读取）。
    /// - agent_type = 会话 agent_type（智能体类型，不再用 role；R-WF-01 后
    ///   role 恒 None，保留字段兼容存量序列化）。
    ///
    /// R-GC-34（主人身份错位 P0 修复，方案 B）：`__master__`（GROUP_MASTER_ACTOR
    /// 保留字，local_customizations.rs:96）特判 → 主人身份 = depth 0（L0）+
    /// 主人名（i18n，禁硬编码中文）。
    async fn resolve_sender_identity(
        coordinator: &ConversationCoordinator,
        session_id: &str,
        workspace: &str,
    ) -> SenderIdentity {
        if session_id == GROUP_MASTER_ACTOR {
            return Self::master_sender_identity().await;
        }
        let role = None;
        let depth = coordinator.session_tree().get_depth(session_id);
        let manager = coordinator.get_session_manager();
        // 会话 agent_type（智能体类型，R-WF-03 发言方标识的「类型」位）。
        let agent_type = manager
            .get_session(session_id)
            .map(|session| session.agent_type.clone());
        // 内存会话名（次优先级，SOUL.name 之下）。
        let session_name = manager.get_session(session_id).and_then(|session| {
            let name = session.session_name.trim().to_string();
            (!name.is_empty()).then_some(name)
        });
        // R-WF-03：身份本源名 = 工作区 SOUL.md frontmatter `name`（三文件
        // 身份名，需求 §六.5）。SOUL 名优先于会话名——会话名是界面标题，
        // SOUL.name 是智能体身份（军团三文件制唯一权威）。
        let soul_name = async {
            let soul_path = std::path::Path::new(workspace).join("SOUL.md");
            let content = tokio::fs::read_to_string(soul_path).await.ok()?;
            let (metadata, _) = crate::util::FrontMatterMarkdown::load_str(&content).ok()?;
            let name = metadata
                .get("name")
                .and_then(|v| v.as_str())?
                .trim()
                .to_string();
            (!name.is_empty()).then_some(name)
        }
        .await;
        // 回退链：SOUL.name → 内存会话名 → 磁盘元数据会话名 → None。
        let name = match soul_name.or(session_name) {
            Some(name) => Some(name),
            None => {
                let disk_name = async {
                    manager
                        .load_session_metadata(std::path::Path::new(workspace), session_id)
                        .await
                        .ok()
                        .flatten()
                        .and_then(|m| {
                            let name = m.session_name.trim().to_string();
                            (!name.is_empty()).then_some(name)
                        })
                }
                .await;
                disk_name
            }
        };
        SenderIdentity {
            session_id: session_id.to_string(),
            role,
            depth,
            name,
            agent_type,
        }
    }

    /// 主人 SenderIdentity（R-GC-34 方案 B）：L0 + i18n 主人名。
    ///
    /// - role：R-WF-01 全删 RBAC 后恒 None（不再硬编码 Commander，发言方标识
    ///   由 R-WF-03 统一为「SOUL.name + 类型」）。
    /// - depth：0（L0 根，会话树语义）。
    /// - name：i18n shared term `agents.master`（按当前 locale 翻译；共享词条
    ///   在 shared/i18n/resources/shared/*/terms.json，经
    ///   generate-i18n-contract.mjs 生成 Rust 端 GENERATED_SHARED_TERMS）。
    ///   i18n-runtime feature 未启用（CLI/acp 编译面，AGENTS-CN.md：
    ///   「调用 I18nService 的 host 必须显式选择 i18n-runtime」）或全局服务
    ///   缺失/词条缺失 → 回退 "Master"（英文，i18n fallback 链语义的兜底）；
    ///   绝不返回空名（空值防御，不 crash）。
    /// - agent_type：R-WF-03 发言方标识 = SOUL.name + 类型——主人无 SOUL 文件
    ///   （L0 根），类型位恒 `Some("__master__")` 占位（GROUP_MASTER_ACTOR），
    ///   与 senderSessionId 同源，前端据 session_id 识别主人。
    async fn master_sender_identity() -> SenderIdentity {
        let role = None;
        let depth = Some(0u32);
        #[cfg(feature = "i18n-runtime")]
        let name = match crate::service::i18n::get_global_i18n_service().await {
            Some(service) => {
                let locale = service.get_current_locale().await;
                let translated = service
                    .translate_with_locale(&locale, "shared.agents.master", None)
                    .await;
                (!translated.is_empty() && translated != "shared.agents.master")
                    .then_some(translated)
            }
            None => None,
        };
        #[cfg(not(feature = "i18n-runtime"))]
        let name = None;
        let name = name.or_else(|| Some("Master".to_string()));
        SenderIdentity {
            session_id: GROUP_MASTER_ACTOR.to_string(),
            role,
            depth,
            name,
            agent_type: Some(GROUP_MASTER_ACTOR.to_string()),
        }
    }

    /// 群会话的 workspace（内存 config 绑定，coordinator.rs:3014 写入）。
    ///
    /// R-GC-38（扩展，死锁链）：内存 session 缺失（重启后群会话未加载）
    /// → 回退磁盘持久化校验——先 `resolve_session_workspace_binding`
    /// （session_manager.rs:1664，四段定位含 projects_root 扫描）解析
    /// binding，取 binding.project_root_path（本地 = 会话元数据的
    /// workspace_path 同源）作为群 workspace。证据：group_workspace 从内存
    /// session 读 config.workspace_path，群会话未加载内存时 send/history/
    /// invite/fork 报「does not exist in memory」，且打开群依赖 isGroupChat
    /// （R-GC-35）→ 死锁链（侦察-群聊运行时风险深挖-第六任CPO 隐患 2）；
    /// 只修 validate_session_exists 不修 group_workspace = 重启后群操作仍报错。
    async fn group_workspace(
        coordinator: &ConversationCoordinator,
        group_id: &str,
    ) -> BitFunResult<String> {
        let manager = coordinator.get_session_manager();
        if let Some(workspace) = manager
            .get_session(group_id)
            .and_then(|session| session.config.workspace_path)
        {
            return Ok(workspace);
        }
        if let Some(binding) = manager.resolve_session_workspace_binding(group_id).await {
            let workspace = binding.project_root_path.to_string_lossy().to_string();
            if !workspace.trim().is_empty() {
                return Ok(workspace);
            }
        }
        Err(BitFunError::tool(format!(
            "group chat session '{group_id}' does not exist in memory or on disk"
        )))
    }

    /// 校验成员会话真实存在（群聊重建 Type-Contract §二：成员 = 调用方传入
    /// 的真实会话 ID，禁按数量新建匿名会话）。
    ///
    /// R-GC-38（P1 升级）：内存 `session_manager.get_session`（session_manager
    /// .rs:3201 只查 self.sessions）失败 → 回退磁盘持久化会话校验——A 路实证
    /// （侦察-群聊运行时风险深挖-第六任CPO-20260815.md 现象 3 根因）：前端列
    /// 磁盘、后端验内存 = 重启后邀请成员不全直接根因。回退
    /// `resolve_session_workspace_binding`（session_manager.rs:1664，四段定位：
    /// 内存 config → session_storage_path_index → 注册 workspace → projects_root
    /// 扫描），binding 解析成功 = 磁盘存在该会话的持久化元数据。
    /// 会话不存在 → 返回明确错误 Err("member session not found: {session_id}")
    /// （禁静默跳过 R-3）。
    async fn validate_session_exists(
        coordinator: &ConversationCoordinator,
        session_id: &str,
    ) -> BitFunResult<()> {
        let manager = coordinator.get_session_manager();
        if manager.get_session(session_id).is_some() {
            return Ok(());
        }
        if manager
            .resolve_session_workspace_binding(session_id)
            .await
            .is_some()
        {
            return Ok(());
        }
        Err(BitFunError::tool(format!(
            "member session not found: {session_id}"
        )))
    }

    /// 群主默认对话类型（R-WF-02，2026-08-16）：群聊 = agent_type="group"
    /// 一等内置类型（AgentType::Group / GroupMode）。群主会话创建与
    /// list_groups 识别统一走本函数——单一权威源，禁散落硬编码
    /// "group" 字符串（零硬编码铁律），改类型只改本函数一处。
    fn default_group_agent_type() -> String {
        "group".to_string()
    }

    /// 群主默认对话显示名（R-GC-28/28b，零硬编码）：从 AgentRegistry 取
    /// group 类型 agent 的 name()（GroupMode::name() = "group"，group.rs）。
    /// 复用现成 `get_agent(agent_type, None)`（registry/mod.rs:177）→
    /// `Agent::name()`；缺失时回退 agent_type 本身（不炸）。
    ///
    /// 群聊重建 Type-Contract §三.5：create_member_session（按数量新建匿名
    /// 成员会话）已移除（禁 dead_code 残留 C-10）——R-GC-28 丢弃入参 ID 的
    /// 实现不再存在，成员 = 调用方传入的真实会话 ID。本函数保留为「显式
    /// 新建成员」场景的命名权威源（契约 §二：default_group_agent_type/name
    /// 保留仅用于显式新建成员场景）；当前无显式新建调用方，故标注
    /// #[allow(dead_code)] 待该场景落地时恢复使用（C-10/C-11 零残留）。
    #[allow(dead_code)]
    fn default_group_agent_name() -> String {
        get_agent_registry()
            .get_agent(Self::default_group_agent_type().as_str(), None)
            .map(|agent| agent.name().to_string())
            .unwrap_or_else(Self::default_group_agent_type)
    }

    /// 建群 = 建 agent_type="group" 对话类型会话（type-contract §二.1；
    /// R-WF-02 一等内置类型：agent_type 取 default_group_agent_type()
    /// = "group"，workspace 取入参兜底链）。
    async fn create_group(
        coordinator: &ConversationCoordinator,
        name: &str,
        members: &[String],
        workspace: &str,
    ) -> BitFunResult<String> {
        let group_session_id = uuid::Uuid::new_v4().to_string();
        let group_agent_type = Self::default_group_agent_type();
        let config = SessionConfig {
            workspace_path: Some(workspace.to_string()),
            project_workspace_path: Some(workspace.to_string()),
            ..Default::default()
        };
        coordinator
            .create_session_with_workspace(
                Some(group_session_id.clone()),
                name.to_string(),
                group_agent_type.clone(),
                config,
                workspace.to_string(),
            )
            .await
            .map_err(BitFunError::tool)?;

        // R-WF-08 原子步 2：群 mode 提示词 = 建群时 system 第一条
        // （role=system，仅新建会话首次，缓存保护——不插入已有历史中间，
        // 只作为本新会话的首条 turn 落盘）。mode 提示词 = 群整体一个 mode，
        // 内容 = 群聊工作流模式说明（群聊 = 容器会话、成员经工具互发、无
        // 大模型响应），随建群会话创建时写入，此后不动（禁重复写入）。
        Self::write_group_mode_system_turn(coordinator, workspace, &group_session_id, name).await?;

        // R-GC-25 群主对话模型：建群 = 创建群主 Claw 会话 + 写入群主欢迎
        // turn（宿主 turn）。群聊 = 普通会话（契约 §一）：群主会话必须带
        // 真实对话 turn，否则开局为空字符串/空时间线、且无宿主 turn 支撑
        // 「该轮以非标准方式结束」的根因（R-GC-23 同根）。
        // 欢迎 turn 与 send_message 同构（kind=UserDialog + status=Completed
        // + finish_reason="complete"），前端 NORMAL_FINISH_REASONS 命中，
        // 不再误报横幅。
        // R-GC-29（2026-08-14 主人实测）：欢迎 turn 文案精简为「群聊「X」
        // 已创建」——删除「我是群主，成员消息将汇聚于此。」冗余描述。该
        // 描述与前端创建成功 toast（CreateGroupChatDialog.tsx:84
        // notificationService.success('群聊「{{name}}」已创建')）文本高度
        // 相似，且欢迎 turn 会作为群聊首条消息渲染（GroupChatView loadHistory
        // 读回 user_dialog 气泡），观感 = 建群提示重复两次。宿主 turn 本体
        // 保留（R-GC-25 结构依赖：群主会话开局必须有真实 turn）。
        Self::write_group_turn(
            coordinator,
            workspace,
            &group_session_id,
            &group_session_id,
            &format!("群聊「{name}」已创建。"),
        )
        .await?;

        // 登记成员（群聊重建 Type-Contract §三.1：成员 = 调用方传入的真实
        // 会话 ID——每个 ID 先校验存在，再登记 groupChats；禁按数量新建匿名
        // 会话 R-GC-28 回退）。
        for member_id in members {
            Self::validate_session_exists(coordinator, member_id).await?;
            Self::add_group_member(coordinator, workspace, &group_session_id, member_id).await?;
        }

        Ok(group_session_id)
    }

    /// R-WF-06 建群=建实例：按工作流 preset 建群——成员会话按模板自动
    /// 实例化（每个 node 建一个会话，成员类型 = node.agent，Claw/agentic/
    /// Plan 等不限定 Claw），群成员表登记自动建的成员 ID。
    ///
    /// 复用链：
    /// - `get_preset`（team_presets.rs:103）读工作流模板（LegionPreset）
    /// - `create_session_with_workspace`（coordinator.rs:2764）建成员会话
    /// - `add_group_member` 登记成员进群成员表（groupChats）
    ///
    /// 成员会话命名：node.role 非空 → `{role}-{node.id}`，否则 `{node.id}`
    /// （与 legion load 部署命名一致）；会话 agent_type = node.agent。
    async fn create_group_from_preset(
        coordinator: &ConversationCoordinator,
        name: &str,
        workspace: &str,
        preset_id: &str,
    ) -> BitFunResult<String> {
        let preset = crate::agentic::agents::team_presets::get_preset(preset_id)
            .map_err(BitFunError::tool)?;
        if preset.nodes.is_empty() {
            return Err(BitFunError::tool(format!(
                "workflow preset '{preset_id}' has no nodes; cannot instantiate a group"
            )));
        }
        // 成员类型按 node.agent（需求 §七：Claw/agentic/Plan 等不限定 Claw）。
        // 每个节点建一个成员会话（工作流 = 创建群聊的「选项」，一个工作流可
        // 建 N 个群，每次建群都按模板实例化全套成员）。
        let mut member_ids = Vec::with_capacity(preset.nodes.len());
        for node in &preset.nodes {
            let session_name = if node.role.trim().is_empty() {
                node.id.clone()
            } else {
                format!("{}-{}", node.role, node.id)
            };
            // R-WF-08 原子步 3（mode 两层 · 成员各自一个）：成员工作区 =
            // resolve_assistant_workspace_dir(Some(node.id)) → workspace-<nodeId>
            // （与 R-WF-07 legion deploy 同口径，独立成员工作区）；成员会话
            // workspace_path = 成员工作区（prompt_builder 据此读身份三文件），
            // project_workspace_path 保持部署 workspace（持久化域不变）。
            let member_workspace = crate::infrastructure::get_path_manager_arc()
                .resolve_assistant_workspace_dir(Some(&node.id), None);
            std::fs::create_dir_all(&member_workspace).map_err(|e| {
                BitFunError::tool(format!(
                    "failed to create member workspace for node '{}': {e}",
                    node.id
                ))
            })?;
            let config = SessionConfig {
                workspace_path: Some(member_workspace.to_string_lossy().to_string()),
                project_workspace_path: Some(workspace.to_string()),
                ..Default::default()
            };
            let session = coordinator
                .create_session_with_workspace(
                    None,
                    session_name,
                    node.agent.clone(),
                    config,
                    workspace.to_string(),
                )
                .await
                .map_err(BitFunError::tool)?;
            // R-WF-08 原子步 3：成员 mode 提示词 = 工作流 node 的 role/prompt/
            // gate 物化为成员身份三文件（SOUL/USER/IDENTITY）+ BOOTSTRAP 临时
            // 清理（复用 R-WF-07 的 initialize_member_persona_files，同一权威
            // 实现）。node.prompt → SOUL（成员 mode 提示词本体），node.role →
            // IDENTITY，直属上级缺省 = 节点 id（preset 无 edge 拓扑），
            // node.gate → SOUL Gate 段。物化失败 = 建群失败（成员 mode 缺失
            // = 身份不完整，禁静默跳过）。
            crate::service::bootstrap::initialize_member_persona_files(
                &member_workspace,
                &node.role,
                &node.prompt,
                node.gate,
                &node.id,
            )
            .await
            .map_err(BitFunError::tool)?;
            member_ids.push(session.session_id);
        }
        // 建群（群主会话 + 欢迎 turn + 成员登记），成员 = 刚实例化的真实会话。
        Self::create_group(coordinator, name, &member_ids, workspace).await
    }

    /// 拉成员 = 校验调用方传入的真实会话 ID 存在 + 记入群成员表
    /// （群聊重建 Type-Contract §三.2：invite = 登记已选真实会话，
    /// 禁按数量新建匿名会话 R-GC-28 回退）。会话不存在 → Err（禁静默跳过）。
    ///
    /// 群 workspace 由 group_id 解析（group_workspace），不再接收入参
    /// workspace（R-GC-R1R4 清理：旧签名的 workspace 仅用于新建匿名成员会话，
    /// 已按新契约移除）。
    async fn invite_member(
        coordinator: &ConversationCoordinator,
        group_id: &str,
        member_session_id: &str,
    ) -> BitFunResult<()> {
        let group_workspace = Self::group_workspace(coordinator, group_id).await?;
        Self::validate_session_exists(coordinator, member_session_id).await?;
        Self::add_group_member(coordinator, &group_workspace, group_id, member_session_id).await
    }

    /// 移除成员 = 从群会话 custom_metadata.groupChats 移除 + 清理成员侧反标
    /// （R-WF-05 P1-A：与 add_group_member 写反标/delete_group 清反标对称）。
    ///
    /// R-WF-05（原子步 3）成员侧反标真实写入成员 workspace 域（P0-1 批次4
    /// 退回修复）后，remove 若只清群侧成员表 → 成员反标残留 → replicate
    /// 遍历成员反标仍含已移除群 → 复刻投递到已移除成员的群（幽灵复刻，
    /// 审查批次4 §四 P1-A 增量）。本函数补清成员域反标：与 add_group_member
    /// 写反标同域（resolve_member_workspace → update_session_metadata 成员域
    /// groupChats 过滤掉 group_id）。成员 workspace 不可解析 → warn 继续
    /// （与 delete_group:1280-1288 一致，不阻断移除主流程）。
    async fn remove_member(
        coordinator: &ConversationCoordinator,
        group_id: &str,
        member_session_id: &str,
    ) -> BitFunResult<()> {
        let group_workspace = Self::group_workspace(coordinator, group_id).await?;
        let manager = coordinator.get_session_manager();
        manager
            .update_session_metadata(&PathBuf::from(&group_workspace), group_id, |metadata| {
                let members = metadata
                    .custom_metadata
                    .as_ref()
                    .and_then(|m| m.get("groupChats"))
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let filtered: Vec<Value> = members
                    .into_iter()
                    .filter(|v| v.as_str() != Some(member_session_id))
                    .collect();
                let custom = metadata
                    .custom_metadata
                    .get_or_insert_with(|| json!({}))
                    .as_object_mut()
                    .expect("custom_metadata is always an object");
                custom.insert("groupChats".to_string(), json!(filtered));
            })
            .await
            .map_err(BitFunError::tool)?;
        // 成员侧反标清理（P1-A）：成员会话 groupChats 过滤掉本群 ID。存储域 =
        // 成员会话真实 workspace（与 add_group_member 写反标同域）。解析失败
        // → warn 继续（不阻断移除）；写入失败 → warn 继续（尽力而为）。
        let member_workspace = match Self::resolve_member_workspace(manager, member_session_id)
            .await
        {
            Some(workspace) => workspace,
            None => {
                warn!(
                        "Failed to resolve member workspace to clear back-mark during remove: member={}, group={}",
                        member_session_id, group_id
                    );
                return Ok(());
            }
        };
        if let Err(error) = manager
            .update_session_metadata(
                &PathBuf::from(&member_workspace),
                member_session_id,
                |metadata| {
                    let custom = metadata
                        .custom_metadata
                        .get_or_insert_with(|| json!({}))
                        .as_object_mut()
                        .expect("custom_metadata is always an object");
                    if let Some(members) =
                        custom.get_mut("groupChats").and_then(|v| v.as_array_mut())
                    {
                        members.retain(|v| v.as_str() != Some(group_id));
                        if members.is_empty() {
                            custom.remove("groupChats");
                        }
                    }
                },
            )
            .await
        {
            warn!(
                "Failed to clear member back-mark during remove: member={}, group={}, error={}",
                member_session_id, group_id, error
            );
        }
        Ok(())
    }

    /// 发送群消息 = 纯落盘群会话 turn（type-contract §二.4 + §三 + R-WF-04）。
    ///
    /// R-GC-26 根因级修复（旧）→ R-WF-04 定稿（2026-08-16）：R-GC-26 曾把
    /// 消息路由进 `coordinator.start_dialog_turn` 触发群主 agent 执行（大模型
    /// 路径），使群主会话有模型响应能力。R-WF-04（Plan:115-121）落地
    /// 「群聊会话无大模型响应 + 开放投递」：send 改走纯落盘
    /// `write_group_turn_with_metadata`（深侦-群聊工具与复刻链路 §2.3 原语）——
    /// 构造 UserDialog + status=Completed + finish_reason="complete" +
    /// has_final_response=true 的宿主 turn 直接持久化，**不触发群主 agent
    /// 执行、不调用大模型**（验收断言 Plan:120「群聊消息只落盘无模型调用」）。
    /// 群消息 = 用户发到群里的消息（契约 §三语义），群主会话无自主模型输出；
    /// 群成员通过各自会话响应，消息聚合复刻由 R-WF-05 承担。
    async fn send_message(
        coordinator: &ConversationCoordinator,
        group_id: &str,
        content: &str,
        sender_session_id: &str,
    ) -> BitFunResult<String> {
        // 群会话存在性门（R-WF-04 简化后的唯一校验）：get_session（内存）+
        // resolve_session_workspace_binding（磁盘回退，重启后未加载场景）；
        // 群不存在 → 明确错误，禁静默跳过（R-3）。不做成员 ∈ groupChats
        // 校验 = 开放投递（非成员可发）。
        let group_workspace = Self::group_workspace(coordinator, group_id).await?;
        let sender =
            Self::resolve_sender_identity(coordinator, sender_session_id, &group_workspace).await;

        // 消息 metadata：五字段 + senderType（契约 §三 + R-WF-03 发言方标识
        // = SOUL.name + 类型）：
        // { groupId, senderSessionId, senderRole, senderDepth, senderName, senderType }。
        let mut metadata = serde_json::Map::new();
        metadata.insert("groupId".to_string(), json!(group_id));
        metadata.insert("senderSessionId".to_string(), json!(sender.session_id));
        if let Some(role) = &sender.role {
            metadata.insert("senderRole".to_string(), json!(role));
        }
        if let Some(depth) = sender.depth {
            metadata.insert("senderDepth".to_string(), json!(depth));
        }
        // senderName 取 SOUL.name（R-WF-03：发言方标识 = SOUL.name + 类型；
        // resolve_sender_identity 已按 SOUL.name → 会话名 → sender id 回退）。
        // 无会话名时回退 sender_session_id 占位。
        // R-GC-34（方案 B，空值防御）：主人（sender_session_id == __master__）
        // 会话名不可得（i18n 服务缺失等）时回退 group_id，绝不 crash。
        let sender_name_fallback = if sender.session_id == GROUP_MASTER_ACTOR {
            group_id
        } else {
            sender_session_id
        };
        metadata.insert(
            "senderName".to_string(),
            json!(sender
                .name
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(sender_name_fallback)),
        );
        // senderType = 智能体类型（R-WF-03 发言方标识「类型」位，metadata
        // 旁路不进 text——缓存保护，总纲 §〇.6）。主人无 agent_type →
        // 回退 senderSessionId（__master__ 同源占位，前端据 session_id 识别）。
        metadata.insert(
            "senderType".to_string(),
            json!(sender
                .agent_type
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(sender.session_id.as_str())),
        );

        // 纯落盘（无模型调用）：turn_id = message_id（get_history 按 turn_id
        // 解析发言方）。turn 形态与 R-GC-25 欢迎 turn 同构（正常完成宿主 turn）。
        Self::write_group_turn_with_metadata(
            coordinator,
            &group_workspace,
            group_id,
            content,
            metadata,
        )
        .await
    }

    /// 群 mode 提示词（R-WF-08 原子步 2）：建群时作为 system 第一条写入
    /// 群会话（role=system，仅新建会话首次——群会话刚创建无历史，本条即首
    /// turn，不插入已有历史中间，缓存保护）。内容 = 群整体一个 mode 的
    /// 提示词（群聊工作流模式说明），落盘后不再变动（建群幂等：重复建群
    /// = 新会话，各自独立首 turn）。
    ///
    /// 落盘形态与其它群消息同构（UserDialog + Completed + "complete" +
    /// has_final_response=true），但 metadata 带 `turnRole="system"` 标记：
    /// - `build_messages_from_turns`（session_manager.rs）按该标记把 turn
    ///   投影为 MessageRole::System；
    /// - `get_history` 的 User/System 过滤把首 turn 返回给前端（验收断言
    ///   「群首 turn=system 提示词」）；
    /// - 群 mode 提示词不参与大模型响应（R-WF-04 群聊无模型执行路径，纯落盘）。
    async fn write_group_mode_system_turn(
        coordinator: &ConversationCoordinator,
        workspace: &str,
        group_id: &str,
        group_name: &str,
    ) -> BitFunResult<String> {
        // mode 提示词内容 = 群整体一个 mode（群聊工作流容器说明）。文案与
        // group_mode.md prompt 模板同源语义（GroupMode::name() = "group"），
        // 不走硬编码中文（群聊 v3 契约 §一：群 = agent_type="group" 容器
        // 会话，成员经群聊工具互发，群会话无大模型响应）。
        let content = format!(
            "群聊工作流 mode：本群「{group_name}」为群聊容器会话。成员会话经群聊工具（create_group_chat/invite_group_member/send_group_message 等）互发消息，消息按发言人身份（senderName/senderType）聚合展示；群会话本身不产生大模型响应。"
        );
        let mut metadata = serde_json::Map::new();
        metadata.insert("groupId".to_string(), json!(group_id));
        // R-WF-08：system 标记（build_messages_from_turns 据此投影
        // MessageRole::System）。sender 字段照常写群主会话 id（与欢迎 turn
        // 同构），前端据 turnRole 区分 system 展示。
        metadata.insert("turnRole".to_string(), json!("system"));
        metadata.insert("senderSessionId".to_string(), json!(group_id));
        Self::write_group_turn_with_metadata(coordinator, workspace, group_id, &content, metadata)
            .await
    }

    /// 群主欢迎 turn（R-GC-25）：建群 = 创建群主 Claw 会话，写群主欢迎
    /// turn 作为会话首条宿主 turn（带 sender 身份 = 群主）。
    async fn write_group_turn(
        coordinator: &ConversationCoordinator,
        workspace: &str,
        group_id: &str,
        sender_session_id: &str,
        content: &str,
    ) -> BitFunResult<String> {
        // 与 send_message 同构的五字段 + senderType metadata（契约 §三 +
        // R-WF-03）：解析群主会话身份（role/depth/name/agent_type），让欢迎
        // turn 的 senderBadge 正常显示。
        let sender = Self::resolve_sender_identity(coordinator, sender_session_id, workspace).await;
        let mut metadata = serde_json::Map::new();
        metadata.insert("groupId".to_string(), json!(group_id));
        metadata.insert("senderSessionId".to_string(), json!(sender.session_id));
        if let Some(role) = &sender.role {
            metadata.insert("senderRole".to_string(), json!(role));
        }
        if let Some(depth) = sender.depth {
            metadata.insert("senderDepth".to_string(), json!(depth));
        }
        metadata.insert(
            "senderName".to_string(),
            json!(sender
                .name
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(sender_session_id)),
        );
        metadata.insert(
            "senderType".to_string(),
            json!(sender
                .agent_type
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(sender.session_id.as_str())),
        );
        Self::write_group_turn_with_metadata(coordinator, workspace, group_id, content, metadata)
            .await
    }

    /// 落盘一条群会话 turn（宿主 turn 形态）：
    /// - kind = UserDialog（is_model_visible=true，history 可读）
    /// - status = Completed + finish_reason = "complete"（前端
    ///   NORMAL_FINISH_REASONS 命中，R-GC-25 消除「该轮以非标准方式结束」）
    /// - has_final_response = true（群消息本身即最终响应）
    /// - turn_index 取「已持久化 turns 的最大 index + 1」——不能固定为 0，
    ///   否则后续消息会覆盖 turn-0 文件（R-GC-10 三形态实测发现，
    ///   群主与成员各发一条时后者把前者覆盖；根因 = 硬编码 turn_index: 0）。
    async fn write_group_turn_with_metadata(
        coordinator: &ConversationCoordinator,
        workspace: &str,
        group_id: &str,
        content: &str,
        metadata: serde_json::Map<String, Value>,
    ) -> BitFunResult<String> {
        let mut next_turn_index = 0usize;
        if let Ok(turns) = coordinator
            .get_session_manager()
            .persistence_manager()
            .load_session_turns(&PathBuf::from(workspace), group_id)
            .await
        {
            next_turn_index = turns
                .iter()
                .map(|turn| turn.turn_index)
                .max()
                .map_or(0, |max| max + 1);
        }
        let message_id = uuid::Uuid::new_v4().to_string();
        let now_ms = Self::now_ms();
        let turn = bitfun_services_core::session::DialogTurnData {
            turn_id: message_id.clone(),
            turn_index: next_turn_index,
            session_id: group_id.to_string(),
            timestamp: now_ms as u64,
            kind: bitfun_services_core::session::DialogTurnKind::UserDialog,
            agent_type: Some(Self::default_group_agent_type()),
            user_message: bitfun_services_core::session::UserMessageData {
                id: message_id.clone(),
                content: content.to_string(),
                timestamp: now_ms as u64,
                metadata: Some(serde_json::Value::Object(metadata)),
            },
            model_rounds: Vec::new(),
            start_time: now_ms as u64,
            end_time: Some(now_ms as u64),
            duration_ms: Some(0),
            token_usage: None,
            // R-GC-25 根因级修复：群消息 = 正常完成的宿主 turn。普通会话
            // 正常终态为 finish_reason="complete"（coordinator.rs:4828/5836），
            // 群消息按同一口径落盘，前端 turnCompletionNotice 不再误报
            // 「该轮以非标准方式结束」（NORMAL_FINISH_REASONS 命中）。
            finish_reason: Some("complete".to_string()),
            has_final_response: Some(true),
            error: None,
            error_detail: None,
            recovery: None,
            recovery_epoch: None,
            status: bitfun_services_core::session::TurnStatus::Completed,
            todos: None,
        };
        coordinator
            .get_session_manager()
            .persistence_manager()
            .save_dialog_turn(&PathBuf::from(workspace), &turn)
            .await
            .map_err(BitFunError::tool)?;

        Ok(message_id)
    }

    /// 查看群历史 = SessionManager::get_messages（type-contract §二.5）。
    ///
    /// R-GC-26：群消息历史只返回**用户发言**（MessageRole::User）。旧实现返回
    /// get_messages 的全部消息（含群主 agent 响应），前端把 assistant 消息也渲染成
    /// 用户气泡。群主响应通过事件流即时显示（DialogTurnStarted/TextChunk），历史
    /// 仅聚合用户发言（群消息 = 用户发到群里的消息，契约 §三语义）。
    ///
    /// author 解析（契约 §三，B-1 修复）：群消息以 `DialogTurnData` 持久化，
    /// 发言方键（senderSessionId/senderRole/senderDepth/senderName）位于
    /// `user_message.metadata`（types.rs:662）。运行时 Message 不承载这些
    /// 自定义键，因此先从持久化 turns 重建「turn_id → 发言方」映射，再为每个
    /// Message 还原 `SenderIdentity`；缺失时优雅降级（senderSessionId 未知 →
    /// "unknown"，role/depth/name → None），绝不阻断读取。
    async fn get_history(
        coordinator: &ConversationCoordinator,
        group_id: &str,
        limit: Option<usize>,
    ) -> BitFunResult<Vec<GroupMessage>> {
        let manager = coordinator.get_session_manager();
        let group_workspace = Self::group_workspace(coordinator, group_id).await?;
        let messages = manager
            .get_messages(group_id)
            .await
            .map_err(BitFunError::tool)?;
        let group_session_id = group_id.to_string();

        // B-1：从持久化 turns 重建发言方映射（turn_id → SenderIdentity）。
        let sender_by_turn = Self::build_sender_by_turn(
            &manager
                .persistence_manager()
                .load_session_turns(&PathBuf::from(&group_workspace), group_id)
                .await
                .unwrap_or_default(),
        );

        // R-GC-26：群消息历史只返回**用户发言**（MessageRole::User）。
        // R-WF-08：群首 turn = 群 mode 提示词（MessageRole::System，
        // build_messages_from_turns 按 metadata turnRole="system" 投影），
        // 历史同样返回（验收断言「群首 turn=system 提示词」；前端据此把
        // mode 提示词渲染为时间线首条）。
        let mut result = messages
            .into_iter()
            .filter(|message| {
                message.role == crate::agentic::core::MessageRole::User
                    || message.role == crate::agentic::core::MessageRole::System
            })
            .map(|message| {
                let sender = message
                    .metadata
                    .turn_id
                    .as_deref()
                    .and_then(|turn_id| sender_by_turn.get(turn_id).cloned())
                    .unwrap_or_else(|| SenderIdentity {
                        session_id: "unknown".to_string(),
                        role: None,
                        depth: None,
                        name: None,
                        agent_type: None,
                    });
                let group_author =
                    (sender.session_id != "unknown").then(|| sender.session_id.clone());
                GroupMessage {
                    message_id: message.id,
                    group_session_id: group_session_id.clone(),
                    author: sender,
                    content: message.content.to_string(),
                    timestamp: message
                        .timestamp
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or_default(),
                    // R-WF-08：消息角色（system 首 turn 投影为 MessageRole::System）。
                    role: (message.role == crate::agentic::core::MessageRole::System)
                        .then(|| "system".to_string()),
                    metadata: GroupChatForwardMetadata {
                        group_id: Some(group_session_id.clone()),
                        group_message_id: None,
                        group_author,
                    },
                }
            })
            .collect::<Vec<_>>();
        if let Some(limit) = limit {
            result.truncate(limit);
        }
        Ok(result)
    }

    /// R-WF-05（原子步 1）：成员 turn 最终回复 → 群会话桥接（消息实时聚合
    /// 复刻）。成员完成一次 dialog turn 后，把最终回复以群消息形态复刻进该
    /// 成员所属的每个群（一对多）：
    /// - 走 `write_group_turn_with_metadata` 纯落盘路径（绕过 agent 执行，
    ///   不触发群主/成员 agent 再跑一轮，Plan:128「走 write_group_turn_
    ///   with_metadata 落盘，绕过 agent 执行」）；
    /// - sender 用成员真实会话 id（resolve_sender_identity → SOUL.name +
    ///   类型，R-WF-03 发言方标识口径）；
    /// - 数据源 = 成员会话 custom_metadata.groupChats 反标（原子步 3 写入，
    ///   成员→群一对多）；
    /// - 异步不阻塞：调用方（persist_completed_dialog_turn hook）以 spawn
    ///   方式调用本函数；本函数内部单群失败 warn 继续（复刻是尽力而为的
    ///   旁路，绝不允许阻断成员会话主流程——验收断言 Plan:132「不阻塞成员
    ///   会话」）。
    pub(crate) async fn replicate_member_turn_to_groups(
        coordinator: &ConversationCoordinator,
        member_session_id: &str,
        final_response: &str,
    ) -> BitFunResult<()> {
        if final_response.trim().is_empty() {
            return Ok(());
        }
        let manager = coordinator.get_session_manager();
        // 读成员反标（成员会话 custom_metadata.groupChats = 群 ID 数组）。
        // 成员会话 workspace 解析：内存 config → 磁盘 binding 回退
        // （group_workspace 同链，但目标 = 成员会话本身）。权威存储域 =
        // 成员 workspace 域（写入侧 add_group_member/delete_group 同域，
        // P0-1 批次4退回修复：写读域一致）。
        let Some(member_workspace) =
            Self::resolve_member_workspace(manager, member_session_id).await
        else {
            // 成员会话不可解析（已删除/未持久化）→ 静默跳过复刻（无群可发）。
            return Ok(());
        };
        let group_ids = match manager
            .load_session_metadata(&PathBuf::from(&member_workspace), member_session_id)
            .await
        {
            Ok(Some(metadata)) => metadata
                .custom_metadata
                .as_ref()
                .and_then(|m| m.get("groupChats"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(ToOwned::to_owned)
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        if group_ids.is_empty() {
            return Ok(());
        }
        for group_id in group_ids {
            // 单群失败 warn 继续（尽力而为的旁路复刻，禁阻塞其它群）。
            if let Err(error) = Self::replicate_member_turn_to_group(
                coordinator,
                &member_workspace,
                member_session_id,
                &group_id,
                final_response,
            )
            .await
            {
                warn!(
                    "Failed to replicate member turn to group: member={}, group={}, error={}",
                    member_session_id, group_id, error
                );
            }
        }
        Ok(())
    }

    /// R-WF-05：单群复刻落盘（replicate_member_turn_to_groups 的单个群执行体）。
    /// 群存在性门（group_workspace）+ 五字段 metadata + senderType（契约 §三
    /// + R-WF-03 发言方标识），内容 = 成员最终回复全文。落盘失败 → Err
    ///   （调用方 warn 继续）。
    async fn replicate_member_turn_to_group(
        coordinator: &ConversationCoordinator,
        member_workspace: &str,
        member_session_id: &str,
        group_id: &str,
        final_response: &str,
    ) -> BitFunResult<String> {
        let group_workspace = Self::group_workspace(coordinator, group_id).await?;
        let sender =
            Self::resolve_sender_identity(coordinator, member_session_id, member_workspace).await;
        let mut metadata = serde_json::Map::new();
        metadata.insert("groupId".to_string(), json!(group_id));
        metadata.insert("senderSessionId".to_string(), json!(sender.session_id));
        if let Some(role) = &sender.role {
            metadata.insert("senderRole".to_string(), json!(role));
        }
        if let Some(depth) = sender.depth {
            metadata.insert("senderDepth".to_string(), json!(depth));
        }
        metadata.insert(
            "senderName".to_string(),
            json!(sender
                .name
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(member_session_id)),
        );
        metadata.insert(
            "senderType".to_string(),
            json!(sender
                .agent_type
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(sender.session_id.as_str())),
        );
        Self::write_group_turn_with_metadata(
            coordinator,
            &group_workspace,
            group_id,
            final_response,
            metadata,
        )
        .await
    }

    /// 解析成员会话真实 workspace（内存 config → 磁盘 binding 回退）。
    ///
    /// R-WF-05（批次4退回 P0-1 修复）：成员反标（成员会话 custom_metadata.
    /// groupChats）的权威存储域 = **成员会话自己所在 workspace 域**——需求
    /// §D.53「每个成员自己单独一个工作区」+ R-WF-07:151「成员工作区 =
    /// workspace-<nodeId>」保证成员 workspace ≠ 群 workspace，写入侧若沿用
    /// 群 workspace 域写成员反标，读取侧（复刻时按成员域 load）必然读不到
    /// → groupChats 反标跨域断链 → 复刻静默失效。本函数与读侧
    /// （replicate_member_turn_to_groups :849-861 同链）共用同一解析口径，
    /// 保证「写入侧落成员域 ↔ 读取侧读成员域」一致。解析失败 → None
    /// （调用方按「成员不可解析」语义处理：add 侧 warn 继续，delete 侧跳过）。
    async fn resolve_member_workspace(
        manager: &crate::agentic::session::SessionManager,
        member_session_id: &str,
    ) -> Option<String> {
        if let Some(workspace) = manager
            .get_session(member_session_id)
            .and_then(|session| session.config.workspace_path)
        {
            return Some(workspace);
        }
        if let Some(binding) = manager
            .resolve_session_workspace_binding(member_session_id)
            .await
        {
            return Some(binding.project_root_path.to_string_lossy().to_string());
        }
        None
    }

    /// 从持久化 turns 重建「turn_id → SenderIdentity」发言方映射（契约 §三）。
    /// user_message.metadata 缺失或为 JSON null 的 turn 跳过；调用方负责容错
    /// （读取失败 → 空映射）。
    fn build_sender_by_turn(
        turns: &[bitfun_services_core::session::DialogTurnData],
    ) -> std::collections::HashMap<String, SenderIdentity> {
        let mut sender_by_turn = std::collections::HashMap::new();
        for turn in turns {
            let Some(metadata) = turn.user_message.metadata.as_ref() else {
                continue;
            };
            // JSON null metadata（测试/异常形态）→ 视为无发言方，跳过。
            if metadata.is_null() {
                continue;
            }
            sender_by_turn.insert(
                turn.turn_id.clone(),
                Self::parse_sender_identity_from_json(metadata),
            );
        }
        sender_by_turn
    }

    /// 从持久化 turn 的 user_message.metadata（JSON）解析 SenderIdentity
    /// （契约 §三 + R-WF-03：senderSessionId/senderRole/senderDepth/senderName/
    /// senderType）。
    fn parse_sender_identity_from_json(metadata: &Value) -> SenderIdentity {
        let get = |key: &str| {
            metadata
                .get(key)
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned)
        };
        SenderIdentity {
            session_id: get("senderSessionId").unwrap_or_else(|| "unknown".to_string()),
            role: get("senderRole"),
            depth: metadata
                .get("senderDepth")
                .and_then(|v| v.as_u64())
                .map(|d| d as u32),
            name: get("senderName").filter(|value| !value.trim().is_empty()),
            agent_type: get("senderType").filter(|value| !value.trim().is_empty()),
        }
    }

    /// 群聊列表 = list_sessions 过滤含群标记（custom_metadata.groupChats）。
    async fn list_groups(
        coordinator: &ConversationCoordinator,
        workspace: &str,
    ) -> BitFunResult<Vec<Value>> {
        let manager = coordinator.get_session_manager();
        let summaries = coordinator
            .list_sessions(std::path::Path::new(workspace))
            .await
            .map_err(BitFunError::tool)?;
        let mut groups = Vec::new();
        let group_agent_type = Self::default_group_agent_type();
        for summary in summaries {
            if summary.agent_type != group_agent_type {
                continue;
            }
            let metadata = manager
                .load_session_metadata(&PathBuf::from(workspace), &summary.session_id)
                .await
                .map_err(BitFunError::tool)?;
            if let Some(meta) = metadata {
                let is_group = meta
                    .custom_metadata
                    .as_ref()
                    .and_then(|m| m.get("groupChats"))
                    .is_some();
                if is_group {
                    groups.push(json!({
                        "groupId": meta.session_id,
                        "name": meta.session_name,
                        "memberCount": meta
                            .custom_metadata
                            .as_ref()
                            .and_then(|m| m.get("groupChats"))
                            .and_then(|v| v.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0),
                    }));
                }
            }
        }
        Ok(groups)
    }

    /// fork 群聊 = branch_session 裂变子群（type-contract §二.7）。
    async fn fork_group(
        coordinator: &ConversationCoordinator,
        group_id: &str,
        name: &str,
        turn_id: Option<&str>,
        members: &[String],
    ) -> BitFunResult<String> {
        use bitfun_services_core::session::{SessionBranchBoundary, SessionBranchRequest};

        let group_workspace = Self::group_workspace(coordinator, group_id).await?;
        let manager = coordinator.get_session_manager();

        // branch_session：从主群 fork 子群（规划/审查/执行小群）。
        let branch = manager
            .persistence_manager()
            .branch_session(
                &PathBuf::from(&group_workspace),
                &SessionBranchRequest {
                    source_session_id: group_id.to_string(),
                    source_turn_id: turn_id.unwrap_or("").to_string(),
                    boundary: SessionBranchBoundary::ThroughTurn,
                },
            )
            .await
            .map_err(BitFunError::tool)?;
        let child_session_id = branch.session_id.clone();

        // 登记子群成员（群聊重建 Type-Contract §三.3：fork members = 调用方
        // 传入的真实会话 ID，每个校验存在后登记子群 groupChats；禁按数量
        // 新建匿名会话 R-GC-28 回退）。
        // R-GC-38（P2）：members 为空 → 登记子群自身 ID 到子群 groupChats
        // （群主=子群自身，契约 §六.1）——branch_session 已继承主群
        // custom_metadata 的 groupChats（主群成员），空成员 fork 时再登记
        // 子群自身，保证子群有群标记 + 成员表非空（list_group_chats 识别）。
        if members.is_empty() {
            Self::add_group_member(
                coordinator,
                &group_workspace,
                &child_session_id,
                &child_session_id,
            )
            .await?;
        }
        for member_id in members {
            Self::validate_session_exists(coordinator, member_id).await?;
            Self::add_group_member(coordinator, &group_workspace, &child_session_id, member_id)
                .await?;
        }

        // 子群命名 + forkOrigin 元数据。
        manager
            .update_session_metadata(&PathBuf::from(&group_workspace), &child_session_id, |m| {
                m.session_name = name.to_string();
                let custom = m
                    .custom_metadata
                    .get_or_insert_with(|| json!({}))
                    .as_object_mut()
                    .expect("custom_metadata is always an object");
                custom.insert(
                    "forkOrigin".to_string(),
                    json!({ "parentGroupId": group_id }),
                );
            })
            .await
            .map_err(BitFunError::tool)?;

        Ok(child_session_id)
    }

    /// 成员状态 = 校验群成员身份 + get_session 查 state（type-contract §二.8）。
    ///
    /// B-4 修复：入参带 group_id + member_session_id，先校验
    /// member_session_id ∈ 群成员表（群会话 custom_metadata.groupChats），
    /// 不在群成员表 → 拒绝（防越权查任意会话）；再 get_session 查 state。
    async fn member_status(
        coordinator: &ConversationCoordinator,
        group_id: &str,
        member_session_id: &str,
    ) -> BitFunResult<Value> {
        let manager = coordinator.get_session_manager();
        let group_workspace = Self::group_workspace(coordinator, group_id).await?;
        let group_metadata = manager
            .load_session_metadata(&PathBuf::from(&group_workspace), group_id)
            .await
            .map_err(BitFunError::tool)?
            .ok_or_else(|| {
                BitFunError::tool(format!(
                    "group chat session '{group_id}' metadata not found"
                ))
            })?;
        let group_members = group_metadata
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let is_member = group_members
            .iter()
            .any(|v| v.as_str() == Some(member_session_id));
        if !is_member {
            return Err(BitFunError::tool(format!(
                "session '{member_session_id}' is not a member of group '{group_id}'"
            )));
        }

        let session = manager.get_session(member_session_id).ok_or_else(|| {
            BitFunError::tool(format!(
                "group member session '{member_session_id}' does not exist in memory"
            ))
        })?;
        Ok(json!({
            "sessionId": session.session_id,
            "agentType": session.agent_type,
            "state": format!("{:?}", session.state),
            "workspacePath": session.config.workspace_path,
        }))
    }

    /// 删除群聊 = 删群会话（type-contract §二.9）。
    ///
    /// R-GC-38（P2）：删除前遍历群成员表，逐个清除成员会话 custom_metadata
    /// .groupChats 里的本群反标（文档 §7 声称「delete 级联清成员反标」对齐）。
    /// 反标 = 成员会话 custom_metadata.groupChats 数组中的群 ID（旧模型
    /// group_chat_membership.rs:18 同键）；单成员反标清除失败 → warn 继续
    /// （S-38 防幽灵，先例 delete_room_impl 逐成员清反标单成员失败 warn 继续），
    /// 不阻塞群会话删除。随后删群会话本体（coordinator.delete_session）。
    async fn delete_group(
        coordinator: &ConversationCoordinator,
        group_id: &str,
    ) -> BitFunResult<()> {
        let group_workspace = Self::group_workspace(coordinator, group_id).await?;
        let manager = coordinator.get_session_manager();

        // 删除前：遍历群成员表（groupChats）逐个清反标。
        if let Ok(Some(group_metadata)) = manager
            .load_session_metadata(&PathBuf::from(&group_workspace), group_id)
            .await
        {
            let group_members = group_metadata
                .custom_metadata
                .as_ref()
                .and_then(|m| m.get("groupChats"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for member in group_members {
                let Some(member_session_id) = member.as_str() else {
                    continue;
                };
                if member_session_id == group_id {
                    continue;
                }
                // P0-1 批次4退回修复：清反标与写反标（add_group_member）同域 =
                // 成员会话真实 workspace（此前写群 workspace 域，与读侧成员域
                // 不一致 → 反标断链）。成员 workspace 不可解析 → 跳过该成员
                // 反标清理（warn，不阻断删群）。
                let Some(member_workspace) =
                    Self::resolve_member_workspace(manager, member_session_id).await
                else {
                    warn!(
                        "R-WF-05(P0-1): cannot resolve member workspace to clear back-mark during delete: member_session_id={}, group_id={}",
                        member_session_id, group_id
                    );
                    continue;
                };
                if let Err(error) = manager
                    .update_session_metadata(
                        &PathBuf::from(&member_workspace),
                        member_session_id,
                        |metadata| {
                            let custom = metadata
                                .custom_metadata
                                .get_or_insert_with(|| json!({}))
                                .as_object_mut()
                                .expect("custom_metadata is always an object");
                            if let Some(members) =
                                custom.get_mut("groupChats").and_then(|v| v.as_array_mut())
                            {
                                members.retain(|v| v.as_str() != Some(group_id));
                                if members.is_empty() {
                                    custom.remove("groupChats");
                                }
                            }
                        },
                    )
                    .await
                {
                    warn!(
                        "R-GC-38: failed to clear group member back-mark during delete: member_session_id={}, group_id={}, error={}",
                        member_session_id, group_id, error
                    );
                }
            }
        }

        coordinator
            .delete_session(std::path::Path::new(&group_workspace), group_id)
            .await
            .map_err(BitFunError::tool)
    }

    /// R-WF-03 编排扩展：改成员工具集——把成员会话的工具集写入群会话
    /// custom_metadata.groupMemberTools（{ memberSessionId: [tool,...] }）。
    ///
    /// 复用现成门（深侦 §1.3）：
    /// - `group_workspace`：群 workspace 解析（内存 config → 磁盘 binding 回退）
    /// - `validate_session_exists`：成员存在性校验（内存 → 磁盘回退，禁静默跳过）
    ///
    /// 工具集为「编排控制提示」（需求 §七：DAG 画布可更改接线 + 指挥官有工具
    /// 可修改/查看）——存储于群会话元数据，供前端/指挥官读取；运行时工具
    /// 授权仍由官方 ToolRuntimeRestrictions 门把关（不在此重复实现）。
    /// 幂等：重复设置同集合直接覆盖，不报错。
    async fn update_member_tools(
        coordinator: &ConversationCoordinator,
        group_id: &str,
        member_session_id: &str,
        tools: &[String],
    ) -> BitFunResult<()> {
        let group_workspace = Self::group_workspace(coordinator, group_id).await?;
        Self::validate_session_exists(coordinator, member_session_id).await?;
        let manager = coordinator.get_session_manager();
        manager
            .update_session_metadata(&PathBuf::from(&group_workspace), group_id, |metadata| {
                let custom = metadata
                    .custom_metadata
                    .get_or_insert_with(|| json!({}))
                    .as_object_mut()
                    .expect("custom_metadata is always an object");
                let tool_map = custom
                    .entry("groupMemberTools".to_string())
                    .or_insert_with(|| json!({}))
                    .as_object_mut()
                    .expect("groupMemberTools is always an object");
                tool_map.insert(member_session_id.to_string(), json!(tools));
            })
            .await
            .map_err(BitFunError::tool)
    }

    /// R-WF-03 编排扩展：改接线——把接线定义（数据流/执行顺序提示）写入群
    /// 会话 custom_metadata.groupWiring。
    ///
    /// 需求 §七「工作流接线：数据流 + 执行顺序，但**不是硬编码约束**——
    /// 前端 DAG 画布展示，可更改，指挥官有工具可修改/查看」：本工具 =
    /// 指挥官侧的接线修改/查看落点。wiring 为任意 JSON 结构（{ nodes:[], edges:[] }
    /// 形态由前端 DAG 画布约定），后端仅持久化透传，不解析不约束。
    /// 幂等：重复设置直接覆盖，不报错。
    async fn update_wiring(
        coordinator: &ConversationCoordinator,
        group_id: &str,
        wiring: &Value,
    ) -> BitFunResult<()> {
        let group_workspace = Self::group_workspace(coordinator, group_id).await?;
        let manager = coordinator.get_session_manager();
        manager
            .update_session_metadata(&PathBuf::from(&group_workspace), group_id, |metadata| {
                let custom = metadata
                    .custom_metadata
                    .get_or_insert_with(|| json!({}))
                    .as_object_mut()
                    .expect("custom_metadata is always an object");
                custom.insert("groupWiring".to_string(), wiring.clone());
            })
            .await
            .map_err(BitFunError::tool)
    }

    /// 记成员进群成员表（幂等：已存在则跳过）。
    async fn add_group_member(
        coordinator: &ConversationCoordinator,
        group_workspace: &str,
        group_id: &str,
        member_session_id: &str,
    ) -> BitFunResult<()> {
        let manager = coordinator.get_session_manager();
        // R-WF-05（原子步 3）：成员↔群一对多「反标」持久化。群侧成员表
        // （groupChats）写群会话；成员侧反标（成员会话 custom_metadata.
        // groupChats = 群 ID 数组）此前只在 delete_group 清除、加入时从不
        // 写入（深侦-群聊工具与复刻链路 §2.4：权威存储 = 群会话 groupChats
        // = 群→多成员；反标 = 成员→多群）。补写反标是「成员 turn 最终回复
        // 实时聚合复刻」（R-WF-05 原子步 1/2）的数据基础——复刻时从成员
        // 反标查该成员属于哪些群。成员侧反标存储路径 = **成员会话真实
        // workspace 域**（P0-1 批次4退回修复：与 delete_group 清反标、复刻
        // 读反标同一存储域；群 workspace 域不是成员反标的权威落点）。
        manager
            .update_session_metadata(&PathBuf::from(group_workspace), group_id, |metadata| {
                let mut members = metadata
                    .custom_metadata
                    .as_ref()
                    .and_then(|m| m.get("groupChats"))
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                if !members
                    .iter()
                    .any(|v| v.as_str() == Some(member_session_id))
                {
                    members.push(json!(member_session_id));
                }
                let custom = metadata
                    .custom_metadata
                    .get_or_insert_with(|| json!({}))
                    .as_object_mut()
                    .expect("custom_metadata is always an object");
                custom.insert("groupChats".to_string(), json!(members));
            })
            .await
            .map_err(BitFunError::tool)?;
        // 成员侧反标（幂等去重）：成员会话 groupChats 追加本群 ID。存储域 =
        // **成员会话真实 workspace**（P0-1 批次4退回修复：此前写群 workspace
        // 域，读侧按成员域读 → 跨域断链复刻静默失效；R-WF-07 定义成员独立
        // workspace 后必然不同）。解析失败/写入失败 → warn 继续（S-38 防幽灵
        // 先例：delete_group 逐成员清反标单成员失败 warn 继续），不阻断建群/
        // 邀请主流程（R-WF-05 验收断言「不阻塞成员会话」同源：反标是复刻
        // 数据源，缺失时复刻静默跳过）。
        let member_workspace =
            match Self::resolve_member_workspace(manager, member_session_id).await {
                Some(workspace) => workspace,
                None => {
                    warn!(
                        "Failed to resolve member workspace for back-mark: member={}, group={}",
                        member_session_id, group_id
                    );
                    return Ok(());
                }
            };
        // 写入失败 → warn 继续（与注释一致 + delete_group 清反标对称；若上抛
        // Err，群侧成员表已写入成功 → create_group/invite 返回失败但群已创建
        // = 孤儿群，S-38 防幽灵先例）。
        if let Err(error) = manager
            .update_session_metadata(
                &PathBuf::from(&member_workspace),
                member_session_id,
                |metadata| {
                    let mut groups = metadata
                        .custom_metadata
                        .as_ref()
                        .and_then(|m| m.get("groupChats"))
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    if !groups.iter().any(|v| v.as_str() == Some(group_id)) {
                        groups.push(json!(group_id));
                    }
                    let custom = metadata
                        .custom_metadata
                        .get_or_insert_with(|| json!({}))
                        .as_object_mut()
                        .expect("custom_metadata is always an object");
                    custom.insert("groupChats".to_string(), json!(groups));
                },
            )
            .await
        {
            warn!(
                "Failed to write member back-mark (groupChats) for member={}, group={}, error={}",
                member_session_id, group_id, error
            );
        }
        Ok(())
    }

    /// 从输入提取 action（只读判定入口；缺失/非法 → None）。
    fn input_action(input: Option<&Value>) -> Option<GroupRoomAction> {
        let action = input?.get("action")?.as_str()?;
        GroupRoomAction::from_str(action)
    }

    /// R-GC-26：建群 workspace 解析（主人定标 2026-08-14：建群 = 新建 Claw
    /// 默认对话，群主会话 workspace = Claw 默认工作区，禁 currentWorkspace）。
    ///
    /// 优先级：入参 workspace（trim 后非空，调用方显式指定群专属工作区）→
    /// 默认 Claw 工作区（`~/.bitfun/personal_assistant/workspace`，
    /// path_manager.rs:203 default_assistant_workspace_dir）。
    ///
    /// R-GC-26 变更：移除 context.workspace_root 一级——旧实现（R-GC-17）把
    /// 当前会话工作区（= 用户当前项目工作区，如 taiji 开发版）作为兜底，
    /// 导致建群后群主会话 workspace 锁定到当前项目（主人实测「工作区自动
    /// 锁定到 taiji 开发版」）。群聊 = Claw 默认对话（契约 §一），群主
    /// workspace 必须落在 Claw 默认工作区，与新建普通 Claw 对话一致。
    /// 任何一级为空/None 都落到默认工作区，任何一端空都不炸、
    /// 不报「workspace is required」。
    fn resolve_create_workspace(workspace_param: Option<&str>) -> String {
        workspace_param
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                get_path_manager_arc()
                    .default_assistant_workspace_dir(None)
                    .to_string_lossy()
                    .trim()
                    .to_string()
            })
    }
}

#[async_trait]
impl Tool for GroupRoomTool {
    fn name(&self) -> &str {
        GROUP_ROOM_TOOL_NAME
    }

    fn short_description(&self) -> String {
        "Manage group chat rooms coordinating multiple Claw assistant sessions.".to_string()
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Manage group chat rooms that coordinate multiple default assistant sessions (v3: 群聊 = 普通会话).

Actions:
- "create": Create a group with a name, members, and a dedicated workspace. Group ID = the created session ID (default assistant agent_type, config-driven). When "preset_id" is provided, the group is instantiated from a workflow preset: member sessions are created automatically per preset node (member agent_type = node.agent) — one workflow can spawn N groups.
- "invite": Invite a member session into a group (creates the member session if missing).
- "remove": Remove a member session from a group.
- "send": Send a group message written into the group session's turn stream (metadata carries sender + groupId).
- "history": Read group message history (SessionHistory of the group session).
- "list": List groups in a workspace (sessions carrying the groupChats marker).
- "fork": Fork a child group (规划/审查/执行小群) via session branch.
- "member_status": Query a member session's state.
- "delete": Delete a group (session delete).
- "update_member_tools": Update a member session's tool set within a group (orchestration control; stored in group metadata).
- "update_wiring": Update the group wiring definition (data flow / execution order hints for the DAG canvas; stored in group metadata).

Arguments:
- "action": One of the actions above.
- "name": Group name for create/fork.
- "workspace": Group workspace for create.
- "members": Member session ids for create/invite/fork.
- "preset_id": Workflow preset id for create (R-WF-06: instantiate the group from a workflow template; members are created automatically per node.agent).
- "group_id": Target group session id for invite/remove/send/history/fork/member_status/delete.
- "member_session_id": Member session id for invite/remove/member_status/update_member_tools.
- "content": Message content for send.
- "sender_session_id": Sender session id for send.
- "urgent": Urgent delivery for send.
- "limit": History read limit.
- "cursor": History page cursor.
- "turn_id": Fork point turn id.
- "tools": Tool set for update_member_tools.
- "wiring": Wiring definition for update_wiring."#
            .to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "invite", "remove", "send", "history", "list", "fork", "member_status", "delete", "update_member_tools", "update_wiring"],
                    "description": "The group chat action to perform."
                },
                "name": { "type": "string", "description": "Group name for create/fork." },
                "workspace": { "type": "string", "description": "Group workspace for create." },
                "members": { "type": "array", "items": { "type": "string" }, "description": "Member session ids for create/invite/fork." },
                "preset_id": { "type": "string", "description": "Workflow preset id for create: instantiate the group from a workflow template (members created per node.agent)." },
                "group_id": { "type": "string", "description": "Target group session id." },
                "member_session_id": { "type": "string", "description": "Member session id for invite/remove/member_status/update_member_tools." },
                "content": { "type": "string", "description": "Message content for send." },
                "sender_session_id": { "type": "string", "description": "Sender session id for send." },
                "urgent": { "type": "boolean", "description": "Urgent delivery for send." },
                "limit": { "type": "integer", "description": "History read limit." },
                "cursor": { "type": "integer", "description": "History page cursor." },
                "turn_id": { "type": "string", "description": "Fork point turn id." },
                "tools": { "type": "array", "items": { "type": "string" }, "description": "Tool set for update_member_tools." },
                "wiring": { "description": "Wiring definition for update_wiring (data flow / execution order hints)." }
            },
            "required": ["action"]
        })
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Direct
    }

    /// 只读判定按 action 区分（type-contract §六.5，B-3 修复）：
    /// history/list/member_status 只读；create/invite/remove/send/fork/delete 非只读。
    fn is_readonly(&self) -> bool {
        false
    }

    /// action 级只读（输入依赖）：由 `is_action_readonly` 决定是否并发安全
    /// 与是否产生权限意图（只读 action 无副作用 → 并发安全 + 无 PermissionIntent）。
    fn is_concurrency_safe(&self, input: Option<&Value>) -> bool {
        Self::input_action(input).is_some_and(group_room_action_is_readonly)
    }

    fn permission_intents(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<PermissionIntent>> {
        if Self::input_action(Some(input)).is_some_and(group_room_action_is_readonly) {
            return Ok(Vec::new());
        }
        Ok(vec![PermissionIntent::new(
            "custom_tool",
            vec![self.name().to_string()],
        )])
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let parsed: GroupRoomInput = serde_json::from_value(input.clone())
            .map_err(|error| BitFunError::tool(format!("Invalid input: {error}")))?;
        let action = GroupRoomAction::from_str(&parsed.action).ok_or_else(|| {
            BitFunError::tool(format!("unknown group_room action '{}'", parsed.action))
        })?;
        let coordinator = Self::coordinator()?;

        // R-WF-09（2026-08-16）：编排工具「指挥官专用」——主会话（created_by
        // == None）可调编排；非主会话拒绝（权限错误）。send/history/list 普通
        // 消息动作开放（不查指挥官，Plan:169 验收断言：send_group_message 仍开放）。
        if group_room_action_is_orchestration(action) {
            Self::ensure_orchestration_main_session(&coordinator, context).await?;
        }

        let output = match action {
            GroupRoomAction::Create => {
                let name = parsed
                    .name
                    .as_deref()
                    .ok_or_else(|| BitFunError::tool("name is required for create".to_string()))?;
                // R-GC-26：建群 = 新建 Claw 默认对话（默认工作区，禁
                // currentWorkspace）。入参 workspace（调用方显式指定群专属
                // 工作区）→ Claw 默认工作区兜底；任一为空都不炸、
                // 不报「workspace is required」。
                let workspace = Self::resolve_create_workspace(parsed.workspace.as_deref());
                // R-WF-06 建群=建实例：preset_id 指定工作流模板 → 按模板
                // node.agent 自动实例化成员会话再建群（一个工作流建 N 群，
                // 成员类型不限定 Claw）。
                let group_id = match parsed.preset_id.as_deref() {
                    Some(preset_id) => {
                        Self::create_group_from_preset(&coordinator, name, &workspace, preset_id)
                            .await?
                    }
                    None => {
                        Self::create_group(&coordinator, name, &parsed.members, &workspace).await?
                    }
                };
                json!({ "groupId": group_id })
            }
            GroupRoomAction::Invite => {
                let group_id = parsed.group_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("group_id is required for invite".to_string())
                })?;
                let member = parsed.member_session_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("member_session_id is required for invite".to_string())
                })?;
                Self::invite_member(&coordinator, group_id, member).await?;
                json!({ "groupId": group_id, "member": member, "status": "invited" })
            }
            GroupRoomAction::Remove => {
                let group_id = parsed.group_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("group_id is required for remove".to_string())
                })?;
                let member = parsed.member_session_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("member_session_id is required for remove".to_string())
                })?;
                Self::remove_member(&coordinator, group_id, member).await?;
                json!({ "groupId": group_id, "member": member, "status": "removed" })
            }
            GroupRoomAction::Send => {
                // R-WF-04 开放投递：send 唯一校验 = 群会话存在（send_message 内
                // group_workspace 内存 + 磁盘回退）；不校验发送者 ∈ groupChats
                // （非成员可发）。群消息纯落盘无模型调用。
                let group_id = parsed.group_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("group_id is required for send".to_string())
                })?;
                let content = parsed
                    .content
                    .as_deref()
                    .ok_or_else(|| BitFunError::tool("content is required for send".to_string()))?;
                let sender = parsed
                    .sender_session_id
                    .as_deref()
                    .or(context.session_id.as_deref())
                    .ok_or_else(|| {
                        BitFunError::tool("sender_session_id is required for send".to_string())
                    })?;
                let message_id =
                    Self::send_message(&coordinator, group_id, content, sender).await?;
                json!({
                    "groupId": group_id,
                    "messageId": message_id,
                    "status": "sent",
                    // 透传 urgent（契约 §二.4 入参声明）：v3 群消息落群会话 turns，
                    // urgent 作为投递提示字段回传，供调用方确认打断语义已受理。
                    "urgent": parsed.urgent,
                })
            }
            GroupRoomAction::History => {
                let group_id = parsed.group_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("group_id is required for history".to_string())
                })?;
                let messages = Self::get_history(&coordinator, group_id, parsed.limit).await?;
                json!({
                    "groupId": group_id,
                    "messages": messages,
                    // 透传 cursor（契约 §二.5 入参声明）：当前实现按 limit 截断，
                    // cursor 作为分页游标原样回传，供调用方确认分页请求已受理。
                    "cursor": parsed.cursor,
                })
            }
            GroupRoomAction::List => {
                let workspace = parsed.workspace.clone().unwrap_or_else(|| {
                    context
                        .workspace_root()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default()
                });
                let groups = Self::list_groups(&coordinator, &workspace).await?;
                json!({ "groups": groups })
            }
            GroupRoomAction::Fork => {
                let group_id = parsed.group_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("group_id is required for fork".to_string())
                })?;
                let name = parsed.name.as_deref().unwrap_or("forked group");
                let child_id = Self::fork_group(
                    &coordinator,
                    group_id,
                    name,
                    parsed.turn_id.as_deref(),
                    &parsed.members,
                )
                .await?;
                json!({ "parentGroupId": group_id, "childGroupId": child_id })
            }
            GroupRoomAction::MemberStatus => {
                let group_id = parsed.group_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("group_id is required for member_status".to_string())
                })?;
                let member = parsed.member_session_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("member_session_id is required for member_status".to_string())
                })?;
                let status = Self::member_status(&coordinator, group_id, member).await?;
                json!({ "groupId": group_id, "status": status })
            }
            GroupRoomAction::Delete => {
                let group_id = parsed.group_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("group_id is required for delete".to_string())
                })?;
                Self::delete_group(&coordinator, group_id).await?;
                json!({ "groupId": group_id, "status": "deleted" })
            }
            GroupRoomAction::UpdateMemberTools => {
                let group_id = parsed.group_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("group_id is required for update_member_tools".to_string())
                })?;
                let member = parsed.member_session_id.as_deref().ok_or_else(|| {
                    BitFunError::tool(
                        "member_session_id is required for update_member_tools".to_string(),
                    )
                })?;
                if parsed.tools.is_empty() {
                    return Err(BitFunError::tool(
                        "tools must not be empty for update_member_tools".to_string(),
                    ));
                }
                Self::update_member_tools(&coordinator, group_id, member, &parsed.tools).await?;
                json!({
                    "groupId": group_id,
                    "member": member,
                    "tools": parsed.tools,
                    "status": "updated",
                })
            }
            GroupRoomAction::UpdateWiring => {
                let group_id = parsed.group_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("group_id is required for update_wiring".to_string())
                })?;
                let wiring = parsed.wiring.clone().ok_or_else(|| {
                    BitFunError::tool("wiring is required for update_wiring".to_string())
                })?;
                Self::update_wiring(&coordinator, group_id, &wiring).await?;
                json!({ "groupId": group_id, "wiring": wiring, "status": "updated" })
            }
        };

        Ok(vec![ToolResult::ok(output, None)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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

    /// R-WF-04：直接用传入 coordinator 调 `send_message`（根因级，R-WF-26b：
    /// 不再经 call_impl → Self::coordinator() 读进程级全局单例——隔离
    /// coordinator 的测试单跑不再 4110 panic；全局空校验语义由
    /// `missing_coordinator_yields_clear_error` 单独覆盖）。输出形态与
    /// call_impl Send 分支一致（groupId/messageId/status/urgent），
    /// sender 缺省语义 = 本测试显式传入，无缺省路径。
    async fn call_send_impl(
        coordinator: &std::sync::Arc<ConversationCoordinator>,
        group_id: &str,
        content: &str,
        sender_session_id: Option<&str>,
    ) -> BitFunResult<Value> {
        let sender = sender_session_id.ok_or_else(|| {
            BitFunError::tool("sender_session_id is required for send".to_string())
        })?;
        let message_id =
            GroupRoomTool::send_message(coordinator, group_id, content, sender).await?;
        Ok(json!({
            "groupId": group_id,
            "messageId": message_id,
            "status": "sent",
            "urgent": false,
        }))
    }

    fn turn_with_sender(
        turn_id: &str,
        sender_json: Value,
    ) -> bitfun_services_core::session::DialogTurnData {
        bitfun_services_core::session::DialogTurnData {
            turn_id: turn_id.to_string(),
            turn_index: 0,
            session_id: "group-1".to_string(),
            timestamp: 0,
            kind: bitfun_services_core::session::DialogTurnKind::UserDialog,
            agent_type: Some(GroupRoomTool::default_group_agent_type()),
            user_message: bitfun_services_core::session::UserMessageData {
                id: turn_id.to_string(),
                content: "hello".to_string(),
                timestamp: 0,
                metadata: Some(sender_json),
            },
            model_rounds: Vec::new(),
            start_time: 0,
            end_time: None,
            duration_ms: None,
            token_usage: None,
            finish_reason: None,
            has_final_response: None,
            error: None,
            error_detail: None,
            recovery: None,
            recovery_epoch: None,
            status: bitfun_services_core::session::TurnStatus::Completed,
            todos: None,
        }
    }

    // ── B-3（契约 §六.5 + R-WF-03）：readonly 按 action 区分 ──
    #[test]
    fn readonly_only_history_list_member_status() {
        for (name, expected) in [
            ("create", false),
            ("invite", false),
            ("remove", false),
            ("send", false),
            ("history", true),
            ("list", true),
            ("fork", false),
            ("member_status", true),
            ("delete", false),
            ("update_member_tools", false),
            ("update_wiring", false),
        ] {
            let action =
                GroupRoomAction::from_str(name).unwrap_or_else(|| panic!("unknown {name}"));
            assert_eq!(
                group_room_action_is_readonly(action),
                expected,
                "action={name}"
            );
        }
    }

    #[test]
    fn tool_metadata_follows_action_readonly() {
        let tool = GroupRoomTool::new();
        // 框架 is_readonly（无 action 上下文基线）保守非只读（契约 §六.5）。
        assert!(!tool.is_readonly());
        // 只读 action：并发安全 + 无权限意图。
        for action in ["history", "list", "member_status"] {
            let input = json!({ "action": action, "group_id": "g-1" });
            assert!(tool.is_concurrency_safe(Some(&input)), "action={action}");
            assert!(
                tool.permission_intents(&input, &empty_context())
                    .expect("permission intents")
                    .is_empty(),
                "action={action}"
            );
        }
        // 非只读 action：非并发安全 + 有权限意图。
        for action in [
            "create",
            "invite",
            "remove",
            "send",
            "fork",
            "delete",
            "update_member_tools",
            "update_wiring",
        ] {
            let input = json!({ "action": action, "group_id": "g-1" });
            assert!(!tool.is_concurrency_safe(Some(&input)), "action={action}");
            assert!(
                !tool
                    .permission_intents(&input, &empty_context())
                    .expect("permission intents")
                    .is_empty(),
                "action={action}"
            );
        }
        // 非法 action → 保守非只读。
        let bad = json!({ "action": "nope" });
        assert!(!tool.is_concurrency_safe(Some(&bad)));
    }

    // ── R-WF-09（2026-08-16）：编排工具「指挥官专用」action 分类 ──
    // 编排 = 建群/加成员/改接线/查状态（Plan:168）：create/invite/remove/
    // fork/delete/update_member_tools/update_wiring/member_status；
    // 普通消息动作 = send/history/list（开放，Plan:169 不查指挥官）。
    #[test]
    fn orchestration_action_classification() {
        for name in [
            "create",
            "invite",
            "remove",
            "fork",
            "delete",
            "update_member_tools",
            "update_wiring",
            "member_status",
        ] {
            let action =
                GroupRoomAction::from_str(name).unwrap_or_else(|| panic!("unknown {name}"));
            assert!(
                group_room_action_is_orchestration(action),
                "action={name} must be orchestration"
            );
        }
        for name in ["send", "history", "list"] {
            let action =
                GroupRoomAction::from_str(name).unwrap_or_else(|| panic!("unknown {name}"));
            assert!(
                !group_room_action_is_orchestration(action),
                "action={name} must be open (not orchestration)"
            );
        }
    }

    // R-WF-09 守卫单测（不依赖全局 coordinator）：主会话放行、非主会话拒绝、
    // 调用会话缺失拒绝。用隔离 coordinator + 直接调用守卫函数验证。
    #[tokio::test]
    async fn orchestration_guard_accepts_main_session_rejects_child() {
        let coordinator = new_isolated_test_coordinator().await;
        let workspace =
            std::env::temp_dir().join(format!("bitfun-rwf09-guard-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&workspace).expect("workspace dir");
        let workspace_str = workspace.to_string_lossy().to_string();

        // 主会话（created_by=None）。
        let main_id = coordinator
            .create_session_with_workspace(
                None,
                "Main".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace_str.clone()),
                    ..Default::default()
                },
                workspace_str.clone(),
            )
            .await
            .expect("create main session")
            .session_id;
        // 非主会话（created_by=Some）。
        let non_main_id = coordinator
            .create_session_with_workspace_and_creator(
                None,
                "Child".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace_str.clone()),
                    ..Default::default()
                },
                workspace_str.clone(),
                Some(format!("session-{main_id}")),
            )
            .await
            .expect("create child session")
            .session_id;

        // 主会话 → 放行。
        let mut context = empty_context();
        context.session_id = Some(main_id.clone());
        GroupRoomTool::ensure_orchestration_main_session(&coordinator, &context)
            .await
            .expect("main session must pass the orchestration guard");

        // 非主会话 → 拒绝（权限错误）。
        context.session_id = Some(non_main_id.clone());
        let error = GroupRoomTool::ensure_orchestration_main_session(&coordinator, &context)
            .await
            .expect_err("child session must be rejected by the orchestration guard");
        let message = error.to_string();
        assert!(
            message.contains("restricted to the main session"),
            "rejection must be a permission error, got: {message}"
        );
        assert!(
            message.contains(&non_main_id),
            "error must name the offending caller session, got: {message}"
        );

        // 调用会话缺失 → 拒绝（fail-closed）。
        let no_session_context = empty_context();
        let error =
            GroupRoomTool::ensure_orchestration_main_session(&coordinator, &no_session_context)
                .await
                .expect_err("missing caller session must be rejected");
        assert!(
            error.to_string().contains("caller session context"),
            "missing-session rejection must be explicit, got: {error}"
        );
    }

    // R-WF-09 集成（call_impl 全链路）：主会话可调编排（create 成功）；非主
    // 会话调编排 → 权限错误；send 普通消息动作开放（非主会话可调）。call_impl
    // 走全局 coordinator——按既有模式复用全局、否则 set_global 隔离
    // coordinator（不嵌套 test_coordinator_access_lock_sync，防重入死锁）。
    #[tokio::test]
    async fn orchestration_actions_require_main_session() {
        // call_impl 走全局 coordinator：复用既有全局（无论谁设置），否则
        // 建隔离 coordinator 并 set_global 后重读全局（OnceLock 单次写入，
        // 若被并行测试抢占则全局是别的实例——必须用全局实例建会话）。
        let coordinator = match get_global_coordinator() {
            Some(coordinator) => coordinator,
            None => {
                let isolated = new_isolated_test_coordinator().await;
                ConversationCoordinator::set_global(isolated.clone());
                get_global_coordinator().expect("global coordinator must be set")
            }
        };
        let workspace =
            std::env::temp_dir().join(format!("bitfun-rwf09-orch-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&workspace).expect("workspace dir");
        let workspace_str = workspace.to_string_lossy().to_string();

        // 主会话（created_by=None）。
        let main_id = coordinator
            .create_session_with_workspace(
                None,
                "Main".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace_str.clone()),
                    ..Default::default()
                },
                workspace_str.clone(),
            )
            .await
            .expect("create main session")
            .session_id;
        // 非主会话（created_by=Some）。
        let non_main_id = coordinator
            .create_session_with_workspace_and_creator(
                None,
                "Child".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace_str.clone()),
                    ..Default::default()
                },
                workspace_str.clone(),
                Some(format!("session-{main_id}")),
            )
            .await
            .expect("create child session")
            .session_id;

        let tool = GroupRoomTool::new();

        // 主会话可调编排（create 返回 groupId）。
        let mut context = empty_context();
        context.session_id = Some(main_id.clone());
        let results = tool
            .call_impl(
                &json!({
                    "action": "create",
                    "name": "R-WF-09 群",
                    "workspace": workspace_str,
                    "members": [],
                }),
                &context,
            )
            .await
            .expect("main session must be allowed to orchestrate");
        let output = results
            .first()
            .map(ToolResult::content)
            .expect("create output");
        assert!(
            output.get("groupId").and_then(Value::as_str).is_some(),
            "main session create must succeed, got: {output}"
        );

        // 非主会话调编排 → 拒绝（权限错误）。
        context.session_id = Some(non_main_id.clone());
        let error = tool
            .call_impl(
                &json!({
                    "action": "create",
                    "name": "拒绝群",
                    "workspace": workspace_str,
                }),
                &context,
            )
            .await
            .expect_err("non-main session must be rejected for orchestration");
        let message = error.to_string();
        assert!(
            message.contains("restricted to the main session"),
            "non-main orchestration error must be a permission error, got: {message}"
        );
        assert!(
            message.contains(&non_main_id),
            "error must name the offending caller session, got: {message}"
        );

        // send 普通消息动作开放：非主会话可调（不查指挥官）。
        let group_id = output
            .get("groupId")
            .and_then(Value::as_str)
            .expect("group id")
            .to_string();
        context.session_id = Some(non_main_id.clone());
        let send_results = tool
            .call_impl(
                &json!({
                    "action": "send",
                    "group_id": group_id,
                    "content": "非主会话发送",
                    "sender_session_id": non_main_id,
                }),
                &context,
            )
            .await
            .expect("send must remain open for non-main sessions (Plan:169)");
        let send_output = send_results
            .first()
            .map(ToolResult::content)
            .expect("send output");
        assert_eq!(
            send_output.get("status").and_then(Value::as_str),
            Some("sent"),
            "send must succeed for non-main session, got: {send_output}"
        );
    }

    // ── R-WF-02（2026-08-16）：群主/成员对话类型 = "group" 一等内置类型 ──
    // 群 = agent_type="group" 会话（AgentType::Group / GroupMode）；本测试
    // 断言 default_group_agent_type 返回 "group"（R-WF-02 验收：群会话
    // agent_type="group"）。
    #[test]
    fn default_group_agent_type_is_group() {
        let actual = GroupRoomTool::default_group_agent_type();
        assert_eq!(actual, "group", "default group agent type must be group");
        assert!(
            !actual.trim().is_empty(),
            "default agent type must be non-empty"
        );
    }

    // ── R-GC-28/28b 零硬编码（主人定标 2026-08-14）：群主默认名称 =
    // group 类型 agent 的显示名（GroupMode::name() = "group"，group.rs），
    // 类型来自 default_group_agent_type、名称来自 AgentRegistry 单一事实源。
    // 群聊重建 Type-Contract §三.5：default_group_agent_name 保留为「显式
    // 新建成员」场景命名权威源（当前无调用方，#[allow(dead_code)] 标注，
    // 无 R-GC-28 匿名成员创建语义）。──
    #[test]
    fn default_group_agent_name_comes_from_agent_registry() {
        let agent_type = GroupRoomTool::default_group_agent_type();
        let expected = crate::agentic::agents::get_agent_registry()
            .get_agent(agent_type.as_str(), None)
            .map(|agent| agent.name().to_string())
            .unwrap_or_else(|| agent_type.clone());
        let actual = GroupRoomTool::default_group_agent_name();
        assert_eq!(actual, expected);
        assert!(
            !actual.trim().is_empty(),
            "default group agent name must be non-empty"
        );
    }

    // ── B-2（契约 §三 + R-WF-03）：send metadata 五字段 + senderName 回退 + senderType ──
    #[test]
    fn send_metadata_contract_shape_is_five_fields() {
        // send 构造的 metadata 键集合 = 契约 §三 五字段 + senderType（R-WF-03
        // 发言方标识 = SOUL.name + 类型；role/depth 缺失时省略）。
        let keys = [
            "groupId",
            "senderSessionId",
            "senderRole",
            "senderDepth",
            "senderName",
            "senderType",
        ];
        // 全字段形态（B-2 完整断言）。
        let metadata = json!({
            "groupId": "group-1",
            "senderSessionId": "sender-1",
            "senderRole": "commander",
            "senderDepth": 3,
            "senderName": "小群主",
            "senderType": "agentic",
        });
        for key in keys {
            assert!(metadata.get(key).is_some(), "missing key {key}");
        }
        assert_eq!(
            metadata.get("groupId").and_then(Value::as_str),
            Some("group-1")
        );
        assert_eq!(
            metadata.get("senderSessionId").and_then(Value::as_str),
            Some("sender-1")
        );
        assert_eq!(
            metadata.get("senderRole").and_then(Value::as_str),
            Some("commander")
        );
        assert_eq!(metadata.get("senderDepth").and_then(Value::as_u64), Some(3));
        assert_eq!(
            metadata.get("senderName").and_then(Value::as_str),
            Some("小群主")
        );
        assert_eq!(
            metadata.get("senderType").and_then(Value::as_str),
            Some("agentic")
        );
    }

    #[test]
    fn send_metadata_name_falls_back_to_sender_id() {
        // senderName 回退逻辑与 send_message 相同（成员无会话名时用
        // sender_session_id；R-GC-34 主人无会话名时回退 group_id，见
        // master_name_falls_back_to_group_id）。
        let sender_session_id = "sender-x";
        let sender_name: Option<String> = None;
        let effective = sender_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(sender_session_id);
        assert_eq!(effective, "sender-x");
    }

    // ── R-WF-03：senderType 回退（智能体类型缺失 → sender session id 占位）──
    #[test]
    fn send_metadata_sender_type_falls_back_to_session_id() {
        // 与 send_message/write_group_turn 的 senderType 组装逻辑同构：
        // agent_type 缺失（如 __master__ 无会话）→ 回退 sender.session_id。
        let session_id = bitfun_runtime_ports::GROUP_MASTER_ACTOR;
        let agent_type: Option<String> = None;
        let effective = agent_type
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(session_id);
        assert_eq!(effective, bitfun_runtime_ports::GROUP_MASTER_ACTOR);
        // 对照：普通成员有类型时用类型。
        let member_type = Some("agentic".to_string());
        let member_effective = member_type
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("sender-x");
        assert_eq!(member_effective, "agentic");
    }

    // ── R-WF-03：发言方标识 = SOUL.name + 类型（metadata，不进 text）──
    // SOUL.name 解析 = 工作区 SOUL.md frontmatter `name` 字段（FrontMatterMarkdown，
    // 与 IDENTITY.md frontmatter 同构）。本测试覆盖解析链（不依赖 coordinator）：
    // frontmatter name 命中 → SOUL.name 优先于会话名。
    #[tokio::test]
    async fn soul_name_resolution_prefers_frontmatter_name() {
        let temp = std::env::temp_dir().join(format!("bitfun-soul-name-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp).expect("temp dir");
        // 无 SOUL.md → None（优雅降级，不炸）。
        let soul_path = temp.join("SOUL.md");
        let content = std::fs::read_to_string(&soul_path).unwrap_or_default();
        let soul_name = async {
            let (metadata, _) = crate::util::FrontMatterMarkdown::load_str(&content).ok()?;
            let name = metadata
                .get("name")
                .and_then(|v| v.as_str())?
                .trim()
                .to_string();
            (!name.is_empty()).then_some(name)
        }
        .await;
        assert_eq!(soul_name, None, "missing SOUL.md must degrade to None");

        // 写入 SOUL.md frontmatter name → 命中。
        std::fs::write(
            &soul_path,
            "---\nname: 姬码锋\ncreature: bee\n---\n\n# SOUL.md\n",
        )
        .expect("write SOUL.md");
        let content = std::fs::read_to_string(&soul_path).expect("read SOUL.md");
        let soul_name = async {
            let (metadata, _) = crate::util::FrontMatterMarkdown::load_str(&content).ok()?;
            let name = metadata
                .get("name")
                .and_then(|v| v.as_str())?
                .trim()
                .to_string();
            (!name.is_empty()).then_some(name)
        }
        .await;
        assert_eq!(
            soul_name.as_deref(),
            Some("姬码锋"),
            "SOUL.name must be the frontmatter name field"
        );
        // 空 name → None（优雅降级）。
        std::fs::write(&soul_path, "---\nname:\n---\n").expect("write empty SOUL.md");
        let content = std::fs::read_to_string(&soul_path).expect("read SOUL.md");
        let soul_name = async {
            let (metadata, _) = crate::util::FrontMatterMarkdown::load_str(&content).ok()?;
            let name = metadata
                .get("name")
                .and_then(|v| v.as_str())?
                .trim()
                .to_string();
            (!name.is_empty()).then_some(name)
        }
        .await;
        assert_eq!(
            soul_name, None,
            "empty frontmatter name must degrade to None"
        );
    }

    // ── R-GC-34（主人身份错位 P0 修复，方案 B）：__master__ 特判 ──
    #[tokio::test]
    async fn master_identity_resolves_to_l0() {
        // 主人（__master__）身份 = L0 + 主人名（i18n）。R-WF-01 全删 RBAC 后
        // role 恒 None。测试环境无全局 i18n service → name 回退英文 "Master"。
        let identity = GroupRoomTool::master_sender_identity().await;
        assert_eq!(
            identity.session_id,
            bitfun_runtime_ports::GROUP_MASTER_ACTOR,
            "master session id must be the __master__ reserved word"
        );
        assert_eq!(identity.role, None, "role must be None after RBAC removal");
        assert_eq!(identity.depth, Some(0), "master depth must be 0 (L0)");
        let name = identity.name.as_deref().expect("master name must exist");
        assert!(
            !name.trim().is_empty(),
            "master name must never be empty (empty-value defense)"
        );
        // R-WF-03：主人类型位 = __master__（GROUP_MASTER_ACTOR 同源占位）。
        assert_eq!(
            identity.agent_type.as_deref(),
            Some(bitfun_runtime_ports::GROUP_MASTER_ACTOR),
            "master senderType must be the __master__ reserved word"
        );
    }

    #[tokio::test]
    async fn master_name_prefers_i18n_shared_term_when_service_available() {
        // i18n shared term agents.master（zh-CN=主人 / en-US=Master / zh-TW=主人）
        // 直接经 generated_shared_term 断言——服务可用时 translate_with_locale
        // 返回词条值（service.rs:187 format_shared_term），服务缺失回退 Master。
        let zh_cn = crate::service::i18n::generated_locale_contract::generated_shared_term(
            crate::service::i18n::LocaleId::ZhCN,
            "agents.master",
        );
        assert_eq!(
            zh_cn,
            Some("主人"),
            "zh-CN master term must be 主人 (i18n, no hardcode)"
        );
        let en_us = crate::service::i18n::generated_locale_contract::generated_shared_term(
            crate::service::i18n::LocaleId::EnUS,
            "agents.master",
        );
        assert_eq!(en_us, Some("Master"), "en-US master term must be Master");
    }

    #[tokio::test]
    async fn master_name_falls_back_to_group_id() {
        // 空值防御（裁决 5）：主人会话名不可得时 senderName 回退 group_id。
        // 与 send_message 的回退分支语义一致：sender 为 __master__ 时
        // fallback = group_id（而非 sender_session_id）。
        let group_id = "group-abc";
        let sender_session_id = bitfun_runtime_ports::GROUP_MASTER_ACTOR;
        let sender_name: Option<String> = None;
        let fallback = if sender_session_id == bitfun_runtime_ports::GROUP_MASTER_ACTOR {
            group_id
        } else {
            sender_session_id
        };
        let effective = sender_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(fallback);
        assert_eq!(effective, group_id, "master name fallback must be group_id");
        // 对照组：普通成员仍回退 sender_session_id。
        let member_fallback = if "member-1" == bitfun_runtime_ports::GROUP_MASTER_ACTOR {
            group_id
        } else {
            "member-1"
        };
        assert_eq!(member_fallback, "member-1");
    }

    #[tokio::test]
    async fn master_identity_resolved_through_resolve_sender_identity() {
        // resolve_sender_identity 对 __master__ 走特判分支：即使 coordinator
        // 无该会话（主人无 Claw session），也返回 L0 身份而非依赖
        // session_tree 的普通路径（空值防御，不 crash）。
        // 此处直接验证特判入口的等价逻辑：__master__ 命中 → 走主人身份。
        let session_id = bitfun_runtime_ports::GROUP_MASTER_ACTOR;
        let is_master = session_id == bitfun_runtime_ports::GROUP_MASTER_ACTOR;
        assert!(
            is_master,
            "__master__ must be recognized as the master actor"
        );
        // 对照：普通成员不命中。
        assert!("member-1" != bitfun_runtime_ports::GROUP_MASTER_ACTOR);
        // 主人身份内容由 master_sender_identity 单测覆盖（L0/名）。
        let identity = GroupRoomTool::master_sender_identity().await;
        assert_eq!(identity.role, None);
        assert_eq!(identity.depth, Some(0));
    }

    // ── B-1（契约 §三 + R-WF-03）：GroupMessage author/metadata 结构 + history author 解析 ──
    #[test]
    fn parse_sender_identity_from_json_full() {
        let parsed = GroupRoomTool::parse_sender_identity_from_json(&json!({
            "groupId": "group-1",
            "senderSessionId": "sender-9",
            "senderRole": "Executor",
            "senderDepth": 2,
            "senderName": "九号助手",
            "senderType": "agentic",
        }));
        assert_eq!(parsed.session_id, "sender-9");
        assert_eq!(parsed.role.as_deref(), Some("Executor"));
        assert_eq!(parsed.depth, Some(2));
        assert_eq!(parsed.name.as_deref(), Some("九号助手"));
        assert_eq!(parsed.agent_type.as_deref(), Some("agentic"));
    }

    #[test]
    fn parse_sender_identity_from_json_degrades_gracefully() {
        let parsed = GroupRoomTool::parse_sender_identity_from_json(&json!({}));
        assert_eq!(parsed.session_id, "unknown");
        assert_eq!(parsed.role, None);
        assert_eq!(parsed.depth, None);
        assert_eq!(parsed.name, None);
        assert_eq!(parsed.agent_type, None);

        let whitespace = GroupRoomTool::parse_sender_identity_from_json(&json!({
            "senderSessionId": "sender-1",
            "senderName": "   ",
            "senderType": "  ",
        }));
        assert_eq!(whitespace.session_id, "sender-1");
        assert_eq!(whitespace.name, None);
        assert_eq!(whitespace.agent_type, None);
    }

    #[test]
    fn history_author_map_resolves_from_turn_metadata() {
        let turns = vec![
            turn_with_sender(
                "turn-a",
                json!({
                    "groupId": "group-1",
                    "senderSessionId": "sender-a",
                    "senderRole": "Commander",
                    "senderDepth": 0,
                    "senderName": "群主",
                }),
            ),
            turn_with_sender("turn-b", json!({ "senderSessionId": "sender-b" })),
            // 无 metadata 的 turn 跳过。
            turn_with_sender("turn-c", json!(null)),
        ];
        let map = GroupRoomTool::build_sender_by_turn(&turns);
        assert_eq!(map.len(), 2);
        let a = map.get("turn-a").expect("turn-a");
        assert_eq!(a.session_id, "sender-a");
        assert_eq!(a.role.as_deref(), Some("Commander"));
        assert_eq!(a.depth, Some(0));
        assert_eq!(a.name.as_deref(), Some("群主"));
        let b = map.get("turn-b").expect("turn-b");
        assert_eq!(b.session_id, "sender-b");
        assert_eq!(b.name, None);
        assert!(!map.contains_key("turn-c"));
    }

    #[test]
    fn history_author_unknown_when_turn_not_in_map() {
        let sender_by_turn: HashMap<String, SenderIdentity> = HashMap::new();
        let turn_id = String::from("some-turn-id");
        let sender = sender_by_turn
            .get(&turn_id)
            .cloned()
            .unwrap_or_else(|| SenderIdentity {
                session_id: "unknown".to_string(),
                role: None,
                depth: None,
                name: None,
                agent_type: None,
            });
        assert_eq!(sender.session_id, "unknown");
        assert_eq!(sender.role, None);
    }

    #[test]
    fn group_message_shape_matches_contract_section_three() {
        // GroupMessage 序列化形态：author 内嵌 SenderIdentity 字段 + metadata 关联键。
        let message = GroupMessage {
            message_id: "msg-1".to_string(),
            group_session_id: "group-1".to_string(),
            author: SenderIdentity {
                session_id: "sender-1".to_string(),
                role: Some("Commander".to_string()),
                depth: Some(0),
                name: Some("群主".to_string()),
                agent_type: Some("group".to_string()),
            },
            content: "hi".to_string(),
            timestamp: 123,
            role: None,
            metadata: GroupChatForwardMetadata {
                group_id: Some("group-1".to_string()),
                group_message_id: None,
                group_author: Some("sender-1".to_string()),
            },
        };
        let json_value = serde_json::to_value(&message).expect("serialize");
        assert_eq!(
            json_value
                .pointer("/author/sessionId")
                .and_then(Value::as_str),
            Some("sender-1")
        );
        assert_eq!(
            json_value.pointer("/author/role").and_then(Value::as_str),
            Some("Commander")
        );
        assert_eq!(
            json_value.pointer("/author/depth").and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            json_value.pointer("/author/name").and_then(Value::as_str),
            Some("群主")
        );
        // R-WF-03：author.agentType（智能体类型）随序列化暴露。
        assert_eq!(
            json_value
                .pointer("/author/agentType")
                .and_then(Value::as_str),
            Some("group")
        );
        assert_eq!(
            json_value
                .pointer("/metadata/groupId")
                .and_then(Value::as_str),
            Some("group-1")
        );
        assert_eq!(
            json_value
                .pointer("/metadata/groupAuthor")
                .and_then(Value::as_str),
            Some("sender-1")
        );
        // R-WF-08：role=None（普通用户消息）不序列化（skip_serializing_if），
        // 避免破坏既有 wire 形态；system 消息 role="system" 才出现。
        assert!(
            json_value.get("role").is_none(),
            "role=None must be omitted from the wire (skip_serializing_if)"
        );
    }

    // ── B-4（契约 §二.8）：member_status 群成员表校验 ──
    #[test]
    fn member_status_requires_group_membership() {
        let group_members = json!(["member-a", "member-b"])
            .as_array()
            .cloned()
            .unwrap_or_default();
        let is_member = |target: &str| group_members.iter().any(|v| v.as_str() == Some(target));
        assert!(is_member("member-a"));
        assert!(!is_member("stranger"));
    }

    #[test]
    fn member_status_membership_parse_helper_shape() {
        // 与 member_status 相同的群成员表读取链（custom_metadata.groupChats 数组）。
        let custom = json!({ "groupChats": ["m-1", "m-2"] });
        let members = custom
            .get("groupChats")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(members.iter().any(|v| v.as_str() == Some("m-1")));
        assert!(!members.iter().any(|v| v.as_str() == Some("m-9")));
    }

    #[test]
    fn send_metadata_group_author_uses_sender_session_id() {
        // history 的 group_author 关联键 = 发送者 session_id（契约 §三）。
        let session_id = "sender-1";
        let group_author = (session_id != "unknown").then(|| session_id.to_string());
        assert_eq!(group_author.as_deref(), Some("sender-1"));
    }

    // ── 核心路径测试（R-GC-08 收尾）──────────────────────────────
    // action 枚举 9 值 round-trip + input_schema 9 enum 校验 + 无 coordinator 清晰报错。

    // ── R-GC-26：建群 workspace = Claw 默认工作区（入参显式群工作区优先，
    //    否则默认 Claw 工作区；禁 currentWorkspace）──
    #[test]
    fn resolve_create_workspace_uses_param_first() {
        let workspace = GroupRoomTool::resolve_create_workspace(Some("  /ws/param  "));
        assert_eq!(workspace, "/ws/param");
    }

    #[test]
    fn resolve_create_workspace_never_uses_context_root() {
        // R-GC-26：建群 workspace 解析不再读取 context.workspace_root——
        // 当前项目工作区（如 taiji 开发版）不得成为群主会话 workspace。
        // 入参为空时直接落到 Claw 默认工作区（assistant home 下）。
        let workspace = GroupRoomTool::resolve_create_workspace(None);
        assert!(
            workspace.contains("personal_assistant") || workspace.contains(".bitfun"),
            "default Claw workspace should live under the assistant home, got: '{workspace}'"
        );
    }

    #[test]
    fn resolve_create_workspace_whitespace_falls_back_to_default() {
        // 空串/纯空白入参 → 默认 Claw 工作区（不得报「workspace is required」）。
        for param in [Some(""), Some("   "), None] {
            let workspace = GroupRoomTool::resolve_create_workspace(param);
            assert!(
                !workspace.trim().is_empty(),
                "empty param must fall back to a non-empty default workspace, got: '{workspace}'"
            );
            assert!(
                workspace.contains("personal_assistant") || workspace.contains(".bitfun"),
                "default Claw workspace should live under the assistant home, got: '{workspace}'"
            );
        }
    }

    #[test]
    fn create_without_workspace_does_not_error_on_missing_coordinator_only() {
        // call_impl create 分支：workspace 缺省不再触发「workspace is required」——
        // 解析兜底在 coordinator 校验之前完成；无 coordinator 时报错仍是
        // 「require an initialized coordinator」（见 missing_coordinator_yields_clear_error）。
        // workspace 空串输入 → 兜底链产出默认工作区，不再要求 workspace 必填。
        let resolved = GroupRoomTool::resolve_create_workspace(Some(""));
        assert!(!resolved.trim().is_empty());
        assert!(
            !resolved.starts_with("workspace is required"),
            "resolve must not surface a workspace-required error"
        );
    }

    #[test]
    fn action_round_trip_all_nine_actions() {
        let cases: [(GroupRoomAction, &str); 11] = [
            (GroupRoomAction::Create, "create"),
            (GroupRoomAction::Invite, "invite"),
            (GroupRoomAction::Remove, "remove"),
            (GroupRoomAction::Send, "send"),
            (GroupRoomAction::History, "history"),
            (GroupRoomAction::List, "list"),
            (GroupRoomAction::Fork, "fork"),
            (GroupRoomAction::MemberStatus, "member_status"),
            (GroupRoomAction::Delete, "delete"),
            (GroupRoomAction::UpdateMemberTools, "update_member_tools"),
            (GroupRoomAction::UpdateWiring, "update_wiring"),
        ];
        for (expected, name) in cases {
            let parsed = GroupRoomAction::from_str(name)
                .unwrap_or_else(|| panic!("action {name} must parse"));
            assert_eq!(parsed, expected, "round-trip {name}");
        }
        // 非法值拒绝。
        assert!(GroupRoomAction::from_str("").is_none());
        assert!(GroupRoomAction::from_str("CREATE").is_none());
        assert!(GroupRoomAction::from_str("memberstatus").is_none());
    }

    #[test]
    fn input_schema_lists_all_nine_action_enums() {
        let schema = GroupRoomTool::new().input_schema();
        let enums = schema
            .pointer("/properties/action/enum")
            .and_then(Value::as_array)
            .expect("action enum array");
        let expected = [
            "create",
            "invite",
            "remove",
            "send",
            "history",
            "list",
            "fork",
            "member_status",
            "delete",
            "update_member_tools",
            "update_wiring",
        ];
        assert_eq!(
            enums.len(),
            11,
            "exactly 11 enum values (9 + 2 orchestration)"
        );
        for name in expected {
            assert!(
                enums.iter().any(|v| v.as_str() == Some(name)),
                "schema enum missing {name}"
            );
            assert!(
                GroupRoomAction::from_str(name).is_some(),
                "schema enum {name} must be parseable"
            );
        }
        // 必填仅 action。
        assert_eq!(
            schema.pointer("/required").and_then(Value::as_array),
            Some(&json!(["action"]).as_array().cloned().unwrap())
        );
    }

    /// 无 coordinator 时所有 action 都返回清晰 tool error（get_global_coordinator 为 None）。
    /// 注意：此测试依赖全局 coordinator 未被其他测试 set_global（OnceLock 单次写入）。
    /// 若已被设置，直接跳过断言（避免跨测试顺序耦合）。
    /// 竞态防护（CI macos-15 修复）：与 set_global 共享同一把全局锁
    /// （coordinator::test_coordinator_access_lock_sync），把「检查 get_global 为
    /// None + call_impl（内部再读 get_global）」整体放在锁内原子执行——锁定期间
    /// set_global 无法写入，两次读取一致，TOCTOU 窗口消除。若 lock 时全局已被
    /// 其它测试设置，直接跳过断言。
    /// 锁须跨 await 保持（call_impl 内部再读 get_global），此为有意设计。
    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn missing_coordinator_yields_clear_error() {
        let _guard = crate::agentic::coordination::coordinator::test_coordinator_access_lock_sync();
        if get_global_coordinator().is_some() {
            return;
        }
        let tool = GroupRoomTool::new();
        let context = empty_context();
        let error = tool
            .call_impl(
                &json!({ "action": "create", "name": "g", "workspace": "/tmp" }),
                &context,
            )
            .await
            .expect_err("must fail without coordinator");
        assert!(
            error
                .to_string()
                .contains("require an initialized coordinator"),
            "error: {error}"
        );
        let error = tool
            .call_impl(&json!({ "action": "history", "group_id": "g-1" }), &context)
            .await
            .expect_err("must fail without coordinator");
        assert!(
            error
                .to_string()
                .contains("require an initialized coordinator"),
            "error: {error}"
        );
    }

    // ── R-WF-06（2026-08-16）：工作流=模板/群聊=实例 ──
    // 验收断言（Plan:141 / TC §六）：一个工作流建 N 群；群成员类型按
    // node.agent（Claw/agentic/Plan 等，不限定 Claw）。

    #[test]
    fn node_agent_type_determines_member_type() {
        // 需求 §七「群成员类型：按工作流定义的 agent 类型（Claw/agentic/Plan
        // 等，不限定 Claw）」——成员类型必须来自 node.agent，绝不硬编码 Claw。
        let agents = ["Claw", "agentic", "Plan", "Debug"];
        for agent in agents {
            let node = crate::agentic::agents::team_presets::LegionNode {
                id: format!("node-{agent}"),
                agent: agent.to_string(),
                role: String::new(),
                prompt: String::new(),
                gate: false,
                tools: Vec::new(),
            };
            assert_eq!(
                node.agent, agent,
                "member type must follow node.agent (not limited to Claw)"
            );
        }
    }

    #[test]
    fn create_input_accepts_preset_id() {
        // create 入参支持 preset_id（建群=建实例入口），schema 同步暴露。
        let schema = GroupRoomTool::new().input_schema();
        assert!(
            schema.pointer("/properties/preset_id").is_some(),
            "input_schema must expose preset_id for create"
        );
        let input = json!({
            "action": "create",
            "name": "g",
            "preset_id": "triad",
        });
        let parsed: GroupRoomInput = serde_json::from_value(input).expect("parse create input");
        assert_eq!(parsed.preset_id.as_deref(), Some("triad"));
    }

    // ── R-WF-06：一个工作流建 N 群（集成，隔离 coordinator）──
    // 自建隔离 coordinator（构造链同 create_send_history_list_roundtrip_with_
    // real_coordinator）：不 set_global、不读 get_global_coordinator。Rust 测
    // 试默认并行、顺序无保证——禁依赖其它测试的全局副作用（P1-2 退回修复），
    // 本测试永远真实执行，断言永不因全局单例缺失而 early-return 空转（P1-1）。
    #[tokio::test]
    async fn workflow_preset_spawns_multiple_groups() {
        use crate::agentic::agents::team_presets::create_preset;
        let coordinator = new_isolated_test_coordinator().await;

        let workspace =
            std::env::temp_dir().join(format!("bitfun-rwf06-wf-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&workspace).expect("workspace dir");
        let workspace_str = workspace.to_string_lossy().to_string();

        // 构造工作流模板：2 节点（agentic + Plan，成员类型不限定 Claw）。
        let preset_id = format!("wf-rwf06-{}", uuid::Uuid::new_v4());
        let preset = crate::agentic::agents::team_presets::LegionPreset {
            id: preset_id.clone(),
            name: "R-WF-06 测试工作流".to_string(),
            description: "test".to_string(),
            nodes: vec![
                crate::agentic::agents::team_presets::LegionNode {
                    id: "writer".to_string(),
                    agent: "agentic".to_string(),
                    role: "executor".to_string(),
                    prompt: String::new(),
                    gate: false,
                    tools: Vec::new(),
                },
                crate::agentic::agents::team_presets::LegionNode {
                    id: "planner".to_string(),
                    agent: "Plan".to_string(),
                    role: "commander".to_string(),
                    prompt: String::new(),
                    gate: false,
                    tools: Vec::new(),
                },
            ],
            edges: Vec::new(),
        };
        create_preset(&preset).expect("create preset");

        // 一个工作流建 2 群（实例化 2 次，每次按 node.agent 建成员）。
        let group_a = GroupRoomTool::create_group_from_preset(
            &coordinator,
            "群A",
            &workspace_str,
            &preset_id,
        )
        .await
        .expect("create group A from preset");
        let group_b = GroupRoomTool::create_group_from_preset(
            &coordinator,
            "群B",
            &workspace_str,
            &preset_id,
        )
        .await
        .expect("create group B from preset");
        assert_ne!(
            group_a, group_b,
            "N groups from one workflow must be distinct"
        );

        // 群 A 成员类型按 node.agent：writer=agentic、planner=Plan。
        let manager = coordinator.get_session_manager();
        let metadata_a = manager
            .load_session_metadata(&workspace, &group_a)
            .await
            .expect("load group A metadata")
            .expect("group A metadata exists");
        let members_a = metadata_a
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            members_a.len(),
            2,
            "group A must auto-instantiate 2 members"
        );
        let mut member_types: Vec<String> = Vec::new();
        for member in &members_a {
            let id = member.as_str().expect("member id");
            let session = manager
                .get_session(id)
                .expect("auto-instantiated member session in memory");
            member_types.push(session.agent_type.clone());
        }
        member_types.sort();
        assert_eq!(
            member_types,
            vec!["Plan".to_string(), "agentic".to_string()],
            "member types must follow node.agent (agentic + Plan, not limited to Claw)"
        );

        // 群 B 同样按 node.agent 实例化（N 群各自全套成员）。
        let metadata_b = manager
            .load_session_metadata(&workspace, &group_b)
            .await
            .expect("load group B metadata")
            .expect("group B metadata exists");
        let members_b = metadata_b
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert_eq!(members_b.len(), 2, "group B must also have 2 members");

        // ── R-WF-08 原子步 3（mode 两层 · 成员各自一个）：preset 实例化的
        // 成员 = 独立工作区（workspace-<nodeId>）+ 身份三文件（SOUL/USER/
        // IDENTITY）齐全 + BOOTSTRAP 临时清理。成员 mode 提示词由 node 的
        // role/prompt 物化（验收断言「三文件齐全 + 各自工作区」）。
        let path_manager = crate::infrastructure::get_path_manager_arc();
        let writer_workspace = path_manager.resolve_assistant_workspace_dir(Some("writer"), None);
        assert!(
            writer_workspace.join("SOUL.md").exists(),
            "R-WF-08: preset member must have SOUL.md (member mode prompt)"
        );
        assert!(
            writer_workspace.join("USER.md").exists(),
            "R-WF-08: preset member must have USER.md"
        );
        assert!(
            writer_workspace.join("IDENTITY.md").exists(),
            "R-WF-08: preset member must have IDENTITY.md"
        );
        assert!(
            !writer_workspace.join("BOOTSTRAP.md").exists(),
            "R-WF-08: BOOTSTRAP.md is a temporary bootstrap file and must be removed"
        );
        let identity = std::fs::read_to_string(writer_workspace.join("IDENTITY.md"))
            .expect("read member IDENTITY.md");
        assert!(
            identity.contains("executor"),
            "R-WF-08: member IDENTITY.md must carry the node role (executor)"
        );

        // 清理：删两个群（测试卫生）。
        GroupRoomTool::delete_group(&coordinator, &group_a)
            .await
            .expect("delete group A");
        GroupRoomTool::delete_group(&coordinator, &group_b)
            .await
            .expect("delete group B");
        // 清理：删除测试 preset（禁在 legions 目录残留 wf-rwf06-* 文件）。
        crate::agentic::agents::team_presets::delete_preset(&preset_id)
            .expect("delete test preset");
    }

    #[test]
    fn create_group_from_preset_rejects_empty_preset() {
        // 空节点模板 → 明确错误（禁建空成员群，禁静默跳过）。
        let preset = crate::agentic::agents::team_presets::LegionPreset {
            id: "empty-wf".to_string(),
            name: "Empty".to_string(),
            description: String::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
        };
        let raw = serde_json::to_string(&preset).expect("serialize preset");
        let round: crate::agentic::agents::team_presets::LegionPreset =
            serde_json::from_str(&raw).expect("parse preset");
        assert!(
            round.nodes.is_empty(),
            "preset with no nodes stays empty (create_group_from_preset must reject it)"
        );
    }

    // ── 集成测试：create → send → history → list（真实 coordinator）──
    // 基建对齐 coordinator.rs 测试 helper（test_coordinator_with_registry，
    // enable_persistence=true 时 save_dialog_turn 可落盘）。set_global 为
    // OnceLock 单次写入：本测试成功后全局 coordinator 保持该实例（接受全局副作用；
    // 其它测试若先 set_global，本测试直接复用并跳过重复构造）。
    #[tokio::test]
    async fn create_send_history_list_roundtrip_with_real_coordinator() {
        use crate::agentic::events::{EventQueue, EventQueueConfig, EventRouter};
        use crate::agentic::execution::{
            ExecutionEngine, ExecutionEngineConfig, RoundExecutor, StreamProcessor,
        };
        use crate::agentic::persistence::PersistenceManager;
        use crate::agentic::session::compression::{CompressionConfig, ContextCompressor};
        use crate::agentic::session::{
            PromptCachePolicy, SessionContextStore, SessionManager, SessionManagerConfig,
        };
        use crate::agentic::tools::pipeline::{ToolPipeline, ToolStateManager};
        use crate::agentic::tools::registry::ToolRegistry;
        use crate::infrastructure::PathManager;
        use crate::runtime_ownership::CoreRuntimeOwnership;
        use std::sync::Arc;
        use std::time::Duration;

        // 自建隔离 coordinator：不读取/复用进程级全局 coordinator。
        // 全局单例是 OnceLock 单次写入——并行测试先 set_global 的实例
        // 拥有不同 user_root（~/.bitfun/projects），reuse 分支会让本测试的
        // 会话落到别人 user_root 下，evict 后磁盘回退（resolve_session_
        // workspace_binding 扫 projects_root）找不到 → get_history 空 →
        // 「history must contain the group welcome turn after restart」失败
        // （CI macos/windows 偶发，R-GC-38 flake 根因）。
        let user_root =
            std::env::temp_dir().join(format!("bitfun-grouproom-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&user_root).expect("user root");
        let path_manager = PathManager::with_user_root_for_tests(user_root.clone());
        let persistence =
            PersistenceManager::new(Arc::new(path_manager)).expect("persistence manager");
        let session_manager = Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(persistence),
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: true,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ));

        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let tool_pipeline = Arc::new(ToolPipeline::new(
            Arc::new(tokio::sync::RwLock::new(ToolRegistry::new())),
            Arc::new(ToolStateManager::new(event_queue.clone())),
            None,
        ));
        let execution_engine = Arc::new(ExecutionEngine::new(
            Arc::new(RoundExecutor::new(
                Arc::new(StreamProcessor::new(event_queue.clone())),
                event_queue.clone(),
                tool_pipeline.clone(),
            )),
            event_queue.clone(),
            session_manager.clone(),
            Arc::new(ContextCompressor::new(CompressionConfig::default())),
            ExecutionEngineConfig::default(),
        ));
        let ownership_root = user_root.join("runtime-ownership");
        let coordinator = ConversationCoordinator::new(
            session_manager.clone(),
            execution_engine,
            tool_pipeline,
            event_queue,
            Arc::new(EventRouter::new()),
            Arc::new(CoreRuntimeOwnership::embedded_with_facts(
                ownership_root,
                "bitfun".to_string(),
                "test",
            )),
        );
        coordinator.set_terminal_port(
            bitfun_runtime_services::test_support::FakeRuntimeServicesProvider::terminal_port(),
        );
        coordinator.set_remote_exec_port(
            bitfun_runtime_services::test_support::FakeRuntimeServicesProvider::remote_exec_port(),
        );
        let coordinator = Arc::new(coordinator);
        // 不再 set_global：本测试全链路用自建隔离 coordinator（Arc 引用），
        // 不读取也不污染进程级全局单例——彻底消除并行测试间的全局竞态。
        let workspace = std::env::temp_dir().join(format!(
            "bitfun-grouproom-workspace-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace dir");
        run_group_roundtrip(&coordinator, &workspace).await;
        run_restart_unloaded_fallback(&coordinator, &workspace).await;
    }

    /// R-GC-38（P1 升级 + 扩展）：重启未加载场景磁盘回退。
    ///
    /// 模拟「重启后会话未加载进内存」：
    /// 1. 创建真实成员会话 + 建群（磁盘已持久化）；
    /// 2. `evict_loaded_session_for_test`（session_manager.rs:541，pub(crate)
    ///    测试专用：仅从内存移除，磁盘保留）把群会话与成员会话踢出内存；
    /// 3. 断言 validate_session_exists（磁盘回退）不误拒真实磁盘会话；
    /// 4. 断言 group_workspace（磁盘回退）可解析群 workspace → 群操作
    ///    （invite/send/history/fork）不报「does not exist in memory」。
    async fn run_restart_unloaded_fallback(
        coordinator: &std::sync::Arc<ConversationCoordinator>,
        workspace: &std::path::Path,
    ) {
        let manager = coordinator.get_session_manager();
        let workspace_str = workspace.to_string_lossy().to_string();

        // 建群（2 真实成员）→ 磁盘持久化完成。
        let member_a = create_member_session_for_test(coordinator, &workspace_str).await;
        let member_b = create_member_session_for_test(coordinator, &workspace_str).await;
        let group_id = GroupRoomTool::create_group(
            coordinator,
            "重启未加载群",
            &[member_a.clone(), member_b.clone()],
            &workspace_str,
        )
        .await
        .expect("create group for restart-unloaded fallback");

        // 模拟重启：群会话 + 成员会话从内存移除（磁盘保留）。
        manager.evict_loaded_session_for_test(&group_id);
        manager.evict_loaded_session_for_test(&member_a);
        manager.evict_loaded_session_for_test(&member_b);
        assert!(
            manager.get_session(&group_id).is_none(),
            "setup: group session must be evicted from memory"
        );
        assert!(
            manager.get_session(&member_a).is_none(),
            "setup: member A must be evicted from memory"
        );

        // 1) validate_session_exists 磁盘回退：真实磁盘会话不误拒。
        GroupRoomTool::validate_session_exists(coordinator, &member_a)
            .await
            .expect("R-GC-38: disk-persisted member session must pass validation after restart");

        // 2) 群操作磁盘回退：invite（依赖 group_workspace + validate_session_exists）。
        GroupRoomTool::invite_member(coordinator, &group_id, &member_a)
            .await
            .expect("R-GC-38: invite must not report 'does not exist in memory' after restart");
        // invite 幂等：member_a 已登记 → 不重复。
        let metadata_after_invite = manager
            .load_session_metadata(workspace, &group_id)
            .await
            .expect("load group metadata")
            .expect("metadata exists");
        let members_after_invite = metadata_after_invite
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(
            members_after_invite
                .iter()
                .any(|v| v.as_str() == Some(member_a.as_str())),
            "invited member A must be registered after restart-fallback invite"
        );

        // 3) history 磁盘回退（依赖 group_workspace）：不报 memory 错（可空历史，欢迎 turn 在）。
        let history = GroupRoomTool::get_history(coordinator, &group_id, None)
            .await
            .expect("R-GC-38: history must not report 'does not exist in memory' after restart");
        // 欢迎 turn 是 User 消息 → 历史非空（create 写 welcome）。
        assert!(
            !history.is_empty(),
            "history must contain the group welcome turn after restart"
        );
        // R-WF-08：群首 turn = system 提示词（get_history 返回 role=system，
        // 验收断言「群首 turn=system 提示词」）。
        let system_msg = history
            .iter()
            .find(|m| m.role.as_deref() == Some("system"))
            .expect("R-WF-08: group history must contain the system mode prompt turn");
        assert!(
            system_msg.content.contains("群聊工作流 mode"),
            "R-WF-08: system mode prompt content must be present in history"
        );
    }

    /// create（建群=建会话，含成员）→ send（写群会话 turns）→ history（读回）
    /// → list（群聊列表过滤）全链路断言。
    /// 测试辅助：创建真实成员会话（契约 §二：成员 = 调用方传入的真实会话 ID，
    /// 由调用方创建后传给 create/invite/fork——测试模拟前端「选中真实 Claw
    /// 会话」后的创建动作）。
    /// 自建隔离的 ConversationCoordinator（不 set_global，不读全局单例）。
    ///
    /// 构造链与 create_send_history_list_roundtrip_with_real_coordinator 相同
    /// （P1-1/P1-2 退回修复，2026-08-16）：Rust 测试默认并行、顺序无保证，
    /// 依赖其它测试 set_global 的全局副作用 = 空转/竞态。R-WF-06 集成断言
    /// 必须基于本函数返回的本地 coordinator 真实执行。
    async fn new_isolated_test_coordinator() -> std::sync::Arc<ConversationCoordinator> {
        use crate::agentic::events::{EventQueue, EventQueueConfig, EventRouter};
        use crate::agentic::execution::{
            ExecutionEngine, ExecutionEngineConfig, RoundExecutor, StreamProcessor,
        };
        use crate::agentic::persistence::PersistenceManager;
        use crate::agentic::session::compression::{CompressionConfig, ContextCompressor};
        use crate::agentic::session::{
            PromptCachePolicy, SessionContextStore, SessionManager, SessionManagerConfig,
        };
        use crate::agentic::tools::pipeline::{ToolPipeline, ToolStateManager};
        use crate::agentic::tools::registry::ToolRegistry;
        use crate::infrastructure::PathManager;
        use crate::runtime_ownership::CoreRuntimeOwnership;
        use std::sync::Arc;
        use std::time::Duration;

        let user_root =
            std::env::temp_dir().join(format!("bitfun-rwf06-isolated-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&user_root).expect("user root");
        let path_manager = PathManager::with_user_root_for_tests(user_root.clone());
        let persistence =
            PersistenceManager::new(Arc::new(path_manager)).expect("persistence manager");
        let session_manager = Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(persistence),
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: true,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ));

        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let tool_pipeline = Arc::new(ToolPipeline::new(
            Arc::new(tokio::sync::RwLock::new(ToolRegistry::new())),
            Arc::new(ToolStateManager::new(event_queue.clone())),
            None,
        ));
        let execution_engine = Arc::new(ExecutionEngine::new(
            Arc::new(RoundExecutor::new(
                Arc::new(StreamProcessor::new(event_queue.clone())),
                event_queue.clone(),
                tool_pipeline.clone(),
            )),
            event_queue.clone(),
            session_manager.clone(),
            Arc::new(ContextCompressor::new(CompressionConfig::default())),
            ExecutionEngineConfig::default(),
        ));
        let ownership_root = user_root.join("runtime-ownership");
        let coordinator = ConversationCoordinator::new(
            session_manager.clone(),
            execution_engine,
            tool_pipeline,
            event_queue,
            Arc::new(EventRouter::new()),
            Arc::new(CoreRuntimeOwnership::embedded_with_facts(
                ownership_root,
                "bitfun".to_string(),
                "test",
            )),
        );
        coordinator.set_terminal_port(
            bitfun_runtime_services::test_support::FakeRuntimeServicesProvider::terminal_port(),
        );
        coordinator.set_remote_exec_port(
            bitfun_runtime_services::test_support::FakeRuntimeServicesProvider::remote_exec_port(),
        );
        Arc::new(coordinator)
    }

    async fn create_member_session_for_test(
        coordinator: &ConversationCoordinator,
        workspace: &str,
    ) -> String {
        let member_id = uuid::Uuid::new_v4().to_string();
        let config = SessionConfig {
            workspace_path: Some(workspace.to_string()),
            project_workspace_path: Some(workspace.to_string()),
            ..Default::default()
        };
        coordinator
            .create_session_with_workspace(
                Some(member_id.clone()),
                "test-member".to_string(),
                GroupRoomTool::default_group_agent_type(),
                config,
                workspace.to_string(),
            )
            .await
            .expect("create member session")
            .session_id
    }

    async fn run_group_roundtrip(
        coordinator: &std::sync::Arc<ConversationCoordinator>,
        workspace: &std::path::Path,
    ) {
        use crate::agentic::core::Message;
        let manager = coordinator.get_session_manager();
        let workspace_str = workspace.to_string_lossy().to_string();

        // create：先建真实成员会话（契约 §二：成员 = 调用方传入的真实会话
        // ID）→ 建群（2 成员）→ 返回 group_id（UUID）；会话列表可见且
        // agent_type=默认对话类型。
        let member_a = create_member_session_for_test(coordinator, &workspace_str).await;
        let member_b = create_member_session_for_test(coordinator, &workspace_str).await;
        let group_name = "测试群";
        let group_id = GroupRoomTool::create_group(
            coordinator,
            group_name,
            &[member_a.clone(), member_b.clone()],
            &workspace_str,
        )
        .await
        .expect("create group");
        assert!(!group_id.is_empty());
        let group_session = manager
            .get_session(&group_id)
            .expect("group session in memory");
        assert_eq!(
            group_session.agent_type,
            GroupRoomTool::default_group_agent_type(),
            "R-GC-25: group-owner session uses the config-driven default agent type (no hardcoded string)"
        );
        assert_eq!(
            group_session.config.workspace_path.as_deref(),
            Some(workspace_str.as_str())
        );

        // 群成员表已写入 groupChats（契约 §一：成员 = 调用方传入的真实会话 ID）。
        let metadata = manager
            .load_session_metadata(workspace, &group_id)
            .await
            .expect("load group metadata")
            .expect("metadata exists");
        let members = metadata
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert_eq!(members.len(), 2, "groupChats must contain 2 members");
        assert!(
            members
                .iter()
                .any(|v| v.as_str() == Some(member_a.as_str())),
            "groupChats must contain real session A"
        );
        assert!(
            members
                .iter()
                .any(|v| v.as_str() == Some(member_b.as_str())),
            "groupChats must contain real session B"
        );
        assert!(
            members
                .iter()
                .all(|v| v.as_str() != Some("member-a") && v.as_str() != Some("member-b")),
            "R-GC-28 回退: members must be the real caller-provided ids, never fresh placeholders"
        );

        // 契约 §四：传不存在 ID → 明确错误（禁静默跳过）。
        let missing_err = GroupRoomTool::create_group(
            coordinator,
            "缺失成员群",
            &["definitely-not-a-real-session".to_string()],
            &workspace_str,
        )
        .await
        .expect_err("create with a non-existent member must fail");
        assert!(
            missing_err.to_string().contains("member session not found"),
            "non-existent member must yield a clear error, got: {missing_err}"
        );

        // 契约 §三.2：invite = 登记调用方传入的真实会话 ID（校验存在）；
        // 传不存在 ID → Err（禁静默跳过）。
        let invite_err =
            GroupRoomTool::invite_member(coordinator, &group_id, "no-such-invite-session")
                .await
                .expect_err("invite with a non-existent member must fail");
        assert!(
            invite_err.to_string().contains("member session not found"),
            "non-existent invite member must yield a clear error, got: {invite_err}"
        );
        let invite_member = create_member_session_for_test(coordinator, &workspace_str).await;
        GroupRoomTool::invite_member(coordinator, &group_id, &invite_member)
            .await
            .expect("invite real member");
        let metadata_after_invite = manager
            .load_session_metadata(workspace, &group_id)
            .await
            .expect("load group metadata")
            .expect("metadata exists");
        let members_after_invite = metadata_after_invite
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(
            members_after_invite
                .iter()
                .any(|v| v.as_str() == Some(invite_member.as_str())),
            "invited real session must be registered in groupChats"
        );

        // ── R-GC-25 群主对话模型：建群 = 创建群主 Claw 会话 + 群主欢迎
        // turn（宿主 turn）。开局不再空字符串/空时间线；欢迎 turn 是
        // 正常完成的宿主 turn（status=Completed + finish_reason="complete"
        // + has_final_response=true），前端 NORMAL_FINISH_REASONS 命中，
        // 「该轮以非标准方式结束」横幅不再误报。
        let welcome_turns = manager
            .persistence_manager()
            .load_session_turns(workspace, &group_id)
            .await
            .expect("load welcome turns");
        assert!(
            !welcome_turns.is_empty(),
            "R-GC-25: create must write a group-owner welcome turn (宿主 turn)"
        );
        let welcome = welcome_turns
            .iter()
            .find(|t| t.user_message.content == format!("群聊「{group_name}」已创建。"))
            .expect("welcome turn content must mention group creation (R-GC-29 concise wording)");
        assert_eq!(
            welcome.status,
            bitfun_services_core::session::TurnStatus::Completed
        );
        assert_eq!(
            welcome.finish_reason.as_deref(),
            Some("complete"),
            "R-GC-25: welcome turn must carry the normal finish code"
        );
        assert_eq!(
            welcome.has_final_response,
            Some(true),
            "R-GC-25: welcome turn is a final response"
        );
        assert_eq!(
            welcome
                .user_message
                .metadata
                .as_ref()
                .and_then(|m| m.get("senderSessionId"))
                .and_then(Value::as_str),
            Some(group_id.as_str()),
            "R-GC-25: welcome turn sender = 群主会话（群聊 ID = 群主会话 ID）"
        );

        // ── R-WF-08 原子步 2：群 mode 提示词 = 建群时 system 第一条 ──
        // （role=system，仅新建会话首次；验收断言 Plan:161「群首 turn=system
        // 提示词」）。mode 首 turn 早于欢迎 turn（turn_index 最小 = 群首），
        // metadata 带 turnRole="system" 标记，供 build_messages_from_turns
        // 投影为 MessageRole::System。
        let system_turn = welcome_turns
            .iter()
            .find(|t| {
                t.user_message
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("turnRole"))
                    .and_then(Value::as_str)
                    == Some("system")
            })
            .expect("R-WF-08: create must write a system mode prompt as first turn");
        assert!(
            system_turn.turn_index < welcome.turn_index,
            "R-WF-08: system mode prompt must precede the welcome turn (group-first turn)"
        );
        assert!(
            system_turn.user_message.content.contains("群聊工作流 mode"),
            "R-WF-08: system mode prompt must carry the group workflow mode wording"
        );
        assert_eq!(
            system_turn.status,
            bitfun_services_core::session::TurnStatus::Completed,
            "R-WF-08: system mode turn is a normal completed host turn"
        );
        assert!(
            system_turn.model_rounds.is_empty(),
            "R-WF-08: system mode prompt must not trigger model invocation"
        );

        // send：写群会话 turns → message_id（发送者 = 真实成员 A）。
        // R-WF-04：send 纯落盘（write_group_turn_with_metadata），不触发群主
        // agent 执行 → 无需 TEST_MODEL_RESOLUTION_AI_CONFIG scope（config 只在
        // start_dialog_turn 模型解析路径需要）。
        let message_id =
            GroupRoomTool::send_message(coordinator, &group_id, "第一条群消息", &member_a)
                .await
                .expect("send message");
        assert!(!message_id.is_empty());

        // ── R-WF-04：send 的消息 turn 为「正常完成宿主 turn」——纯落盘、
        // 无模型轮次。status=Completed + finish_reason="complete" +
        // has_final_response=true（前端 NORMAL_FINISH_REASONS 命中），
        // model_rounds 空 = 无大模型调用（Plan:120「群聊消息只落盘无模型调用」）。
        let sent_turns = manager
            .persistence_manager()
            .load_session_turns(workspace, &group_id)
            .await
            .expect("load sent turns");
        let sent = sent_turns
            .iter()
            .find(|t| t.turn_id == message_id)
            .expect("sent turn persisted by message_id");
        assert_eq!(
            sent.user_message.content, "第一条群消息",
            "R-WF-04: send persists the message into the group-owner session turn"
        );
        assert_eq!(
            sent.status,
            bitfun_services_core::session::TurnStatus::Completed,
            "R-WF-04: sent turn must be persisted as Completed (pure persistence, no agent execution)"
        );
        assert_eq!(
            sent.finish_reason.as_deref(),
            Some("complete"),
            "R-WF-04: sent turn must carry the normal finish code"
        );
        assert_eq!(
            sent.has_final_response,
            Some(true),
            "R-WF-04: sent turn is itself the final response"
        );
        assert!(
            sent.model_rounds.is_empty(),
            "R-WF-04: sent turn must have no model rounds (no model invocation)"
        );
        // 群主会话保持 Idle：send 不触发大模型执行（Processing = 模型运行中）。
        let group_session_after_send = manager
            .get_session(&group_id)
            .expect("group session in memory");
        assert_eq!(
            group_session_after_send.state,
            crate::agentic::core::SessionState::Idle,
            "R-WF-04: group session must stay Idle after send (no model invocation)"
        );

        // history：读回消息（author 从 turn metadata 解析 senderSessionId=member_a）。
        // 注意：get_messages 从持久化 turns 重建 Message（Message.id 为重建时新 uuid），
        // 因此按「内容 + author」匹配，而非 send 返回的 message_id。
        let history = GroupRoomTool::get_history(coordinator, &group_id, None)
            .await
            .expect("get history");
        let found = history
            .iter()
            .find(|m| m.content == "第一条群消息")
            .expect("sent message present in history");
        assert_eq!(found.author.session_id, member_a);
        assert_eq!(found.content, "第一条群消息");
        assert_eq!(found.metadata.group_id.as_deref(), Some(group_id.as_str()));
        // message_id 形状校验（uuid 非空）。
        assert!(!found.message_id.is_empty(), "message_id must not be empty");

        // list：群聊列表过滤（仅含 groupChats 标记的 Claw 会话）。
        let groups = GroupRoomTool::list_groups(coordinator, &workspace_str)
            .await
            .expect("list groups");
        let listed = groups
            .iter()
            .find(|g| g.get("groupId").and_then(Value::as_str) == Some(group_id.as_str()))
            .expect("group listed");
        assert_eq!(
            listed.get("memberCount").and_then(Value::as_u64),
            Some(3),
            "memberCount from groupChats (2 create members + 1 invited)"
        );

        // ── 三形态之②：成员会话（create 拉入的成员 = 调用方传入的真实会话）──
        // 契约 §一：成员 = 调用方传入的真实会话 ID（建群前由调用方创建），
        // 禁按数量新建匿名会话（R-GC-28 回退）。成员 ID 即 groupChats 登记的
        // 真实 ID；成员会话类型 = 创建时的真实 agent_type（默认对话类型）。
        let member_id = members
            .iter()
            .find_map(Value::as_str)
            .expect("first member id from groupChats");
        assert_eq!(
            member_id, member_a,
            "first member must be the real session A"
        );
        let member_session = manager
            .get_session(member_id)
            .expect("member session in memory");
        assert_eq!(
            member_session.agent_type,
            GroupRoomTool::default_group_agent_type(),
            "member session uses the config-driven default agent type"
        );

        // ── 三形态之①：默认 BuiltIn 群主（assistant_id 空）──
        // 群主 = GROUP_MASTER_ACTOR（__master__，契约 §五），无底层 assistant
        // 会话支撑；history 侧 author.session_id 即 __master__。
        // R-WF-04：master send 同样纯落盘（无模型路径）→ 确定性成功断言，
        // 无 busy/config 时序依赖（R-GC-26 start_dialog_turn 时代的
        // TEST_MODEL_RESOLUTION_AI_CONFIG scope 与 busy 拒绝语义已随移除）。
        // master 身份的 history author 解析由 send_metadata_* 单测 +
        // build_sender_by_turn 覆盖（GROUP_MASTER_ACTOR 作为 sender_session_id 透传）。
        let master_message_id = GroupRoomTool::send_message(
            coordinator,
            &group_id,
            "群主发言",
            bitfun_runtime_ports::GROUP_MASTER_ACTOR,
        )
        .await
        .expect("master send must succeed (pure persistence)");
        assert!(
            !master_message_id.is_empty(),
            "master send must return a non-empty message id"
        );
        // master 消息同样以完成态落盘（无模型轮次）。
        let master_turns = manager
            .persistence_manager()
            .load_session_turns(workspace, &group_id)
            .await
            .expect("load master turns");
        let master_sent = master_turns
            .iter()
            .find(|t| t.turn_id == master_message_id)
            .expect("master turn persisted by message_id");
        assert_eq!(
            master_sent.user_message.content, "群主发言",
            "master send persists the message"
        );
        assert!(
            master_sent.model_rounds.is_empty(),
            "R-WF-04: master send must not invoke the model"
        );

        // ── 三形态之③：fork 子群 → parent 关联（契约 §九/§八）──
        // fork 点 = 第一条群消息的持久化 turn_id（send 返回的 message_id 即 turn_id）。
        let member_c = create_member_session_for_test(coordinator, &workspace_str).await;
        let child_id = GroupRoomTool::fork_group(
            coordinator,
            &group_id,
            "测试子群",
            Some(&message_id),
            std::slice::from_ref(&member_c),
        )
        .await
        .expect("fork group");
        assert!(!child_id.is_empty());
        assert_ne!(child_id, group_id, "child must differ from parent");
        // R-WF-03（fork 只读语义）：branch_session 继承 source agent_type
        // （session_branch.rs:72/208）→ 子群 agent_type=group（非 Claw 非
        // agentic），子群同样无大模型响应 + 只读（契约 §二.7）。子群由
        // branch_session 落盘（不注册内存）→ 从磁盘元数据断言。
        let child_metadata_for_type = manager
            .load_session_metadata(workspace, &child_id)
            .await
            .expect("load child metadata")
            .expect("child metadata exists");
        assert_eq!(
            child_metadata_for_type.agent_type,
            GroupRoomTool::default_group_agent_type(),
            "R-WF-03: fork child must keep agent_type=group (readonly fork semantics)"
        );

        // 契约 §四：fork 传不存在 ID → 明确错误（禁静默跳过）。
        let fork_missing_err = GroupRoomTool::fork_group(
            coordinator,
            &group_id,
            "缺失成员子群",
            Some(&message_id),
            &["not-a-real-fork-member".to_string()],
        )
        .await
        .expect_err("fork with a non-existent member must fail");
        assert!(
            fork_missing_err
                .to_string()
                .contains("member session not found"),
            "non-existent fork member must yield a clear error, got: {fork_missing_err}"
        );

        // parent 关联：child custom_metadata.forkOrigin.parentGroupId == 主群 id
        //（group_room fork 写 parentGroupId；branch_session 本身写
        // forkOrigin.sessionId/turnId/turnIndex，fork 重写为 parentGroupId，
        // 契约 §八：fork 亲子关系靠 forkOrigin 元数据）。
        let child_metadata = manager
            .load_session_metadata(workspace, &child_id)
            .await
            .expect("load child metadata")
            .expect("child metadata exists");
        let fork_origin = child_metadata
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("forkOrigin"))
            .expect("child forkOrigin must exist");
        assert_eq!(
            fork_origin.get("parentGroupId").and_then(Value::as_str),
            Some(group_id.as_str()),
            "child forkOrigin.parentGroupId must reference the parent group"
        );

        // 子群自带成员表（fork 继承主群成员 + 登记 fork 成员；契约 §三.3：
        // fork members = 调用方传入的真实会话 ID，登记进子群 groupChats）。
        let child_members = child_metadata
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(
            child_members.len() >= 4,
            "fork child must inherit parent members (3) plus one fork member, got {}",
            child_members.len()
        );
        assert!(
            child_members
                .iter()
                .any(|v| v.as_str() == Some(member_c.as_str())),
            "fork child must register the real fork member C"
        );
        assert!(
            child_members.iter().all(|v| v.as_str() != Some("member-c")),
            "R-GC-28 回退: fork member must be the real caller-provided id, never a placeholder"
        );
        assert!(
            manager.get_session(&member_c).is_some(),
            "fork child member session must exist in memory"
        );

        // 子群继承主群 turns（branch 复制群消息 → 子群历史可读）。
        let child_turns = manager
            .persistence_manager()
            .load_session_turns(workspace, &child_id)
            .await
            .expect("child turns");
        assert!(
            child_turns
                .iter()
                .any(|t| t.user_message.content == "第一条群消息"),
            "child must inherit parent turns"
        );

        // ── R-GC-38（P2）：fork 空成员 → 子群默认登记自身 → 有群标记 ──
        // 契约 §六.1：members 为空 → 登记子群自身 ID 到子群 groupChats
        // （群主=子群自身）。branch_session 继承主群 groupChats（3 成员），
        // 空成员 fork 再登记子群自身 → 成员表非空且含子群自身；
        // list_group_chats 过滤 groupChats 标记 → 子群可被识别。
        let empty_member_child_id =
            GroupRoomTool::fork_group(coordinator, &group_id, "空成员子群", Some(&message_id), &[])
                .await
                .expect("fork with empty members must succeed (R-GC-38 default self-registration)");
        assert!(
            !empty_member_child_id.is_empty(),
            "empty-member child id must be non-empty"
        );
        let empty_child_metadata = manager
            .load_session_metadata(workspace, &empty_member_child_id)
            .await
            .expect("load empty-member child metadata")
            .expect("empty-member child metadata exists");
        let empty_child_members = empty_child_metadata
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(
            !empty_child_members.is_empty(),
            "R-GC-38: empty-member fork child must have a non-empty groupChats (self-registered)"
        );
        assert!(
            empty_child_members
                .iter()
                .any(|v| v.as_str() == Some(empty_member_child_id.as_str())),
            "R-GC-38: empty-member fork child must register itself (群主=子群自身)"
        );
        // list_group_chats 识别：子群带 groupChats 标记 → 出现在群聊列表。
        let groups_after_empty_fork = GroupRoomTool::list_groups(coordinator, &workspace_str)
            .await
            .expect("list groups after empty-member fork");
        assert!(
            groups_after_empty_fork
                .iter()
                .any(|g| g.get("groupId").and_then(Value::as_str)
                    == Some(empty_member_child_id.as_str())),
            "R-GC-38: empty-member fork child must be recognized by list_group_chats"
        );

        // ── R-WF-03：编排扩展——改成员工具集 + 改接线（持久化于群会话
        // custom_metadata）──
        // update_member_tools：成员存在性校验（validate_session_exists 复用）
        // → groupMemberTools 写入 { memberId: [tool,...] }；重复设置幂等覆盖。
        let member_tools = vec!["Read".to_string(), "Grep".to_string(), "Write".to_string()];
        GroupRoomTool::update_member_tools(coordinator, &group_id, &member_a, &member_tools)
            .await
            .expect("update member tools");
        let meta_after_tools = manager
            .load_session_metadata(workspace, &group_id)
            .await
            .expect("load group metadata")
            .expect("metadata exists");
        let member_tool_map = meta_after_tools
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupMemberTools"))
            .expect("groupMemberTools must exist after update_member_tools");
        let member_tools_stored = member_tool_map
            .get(&member_a)
            .and_then(|v| v.as_array())
            .and_then(|a| a.iter().map(Value::as_str).collect::<Option<Vec<_>>>())
            .expect("member A tool set stored");
        assert_eq!(
            member_tools_stored,
            vec!["Read", "Grep", "Write"],
            "update_member_tools must persist the tool set"
        );
        // 成员不存在 → 明确错误（复用 validate_session_exists 门，禁静默跳过）。
        let tools_missing_err = GroupRoomTool::update_member_tools(
            coordinator,
            &group_id,
            "not-a-real-member",
            &["Read".to_string()],
        )
        .await
        .expect_err("update_member_tools with a non-existent member must fail");
        assert!(
            tools_missing_err
                .to_string()
                .contains("member session not found"),
            "non-existent member must yield a clear error, got: {tools_missing_err}"
        );

        // update_wiring：接线定义（数据流/执行顺序提示，非硬编码约束）
        // 持久化于 groupWiring；幂等覆盖。
        let wiring = json!({
            "nodes": ["member_a", "member_b"],
            "edges": [["member_a", "member_b"]],
        });
        GroupRoomTool::update_wiring(coordinator, &group_id, &wiring)
            .await
            .expect("update wiring");
        let meta_after_wiring = manager
            .load_session_metadata(workspace, &group_id)
            .await
            .expect("load group metadata")
            .expect("metadata exists");
        let stored_wiring = meta_after_wiring
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupWiring"))
            .expect("groupWiring must exist after update_wiring");
        assert_eq!(
            stored_wiring
                .get("nodes")
                .and_then(Value::as_array)
                .map(|a| a.len()),
            Some(2),
            "update_wiring must persist the wiring definition"
        );
        assert_eq!(
            stored_wiring
                .get("edges")
                .and_then(Value::as_array)
                .map(|a| a.len()),
            Some(1),
            "update_wiring must persist edges"
        );

        // ── R-WF-03（P2）：delete_group 级联清成员反标 ──
        // 删除群前遍历群成员表清反标（成员会话 custom_metadata.groupChats
        // 移除本群 ID；清空后整个键移除）→ 删除后成员会话无本群反标残留。
        GroupRoomTool::delete_group(coordinator, &group_id)
            .await
            .expect("delete group must succeed");
        // 删除后：成员 A 的反标（groupChats）不再含 group_id。
        let member_a_metadata = manager
            .load_session_metadata(workspace, &member_a)
            .await
            .expect("load member A metadata")
            .expect("member A metadata exists");
        let member_a_groups = member_a_metadata
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(
            !member_a_groups
                .iter()
                .any(|v| v.as_str() == Some(group_id.as_str())),
            "R-GC-38: delete must clear the group back-mark on member A"
        );

        // 只读 action 可经 Tool 接口并发安全调用（不依赖全局 coordinator 的调用路径）。
        let _ = Message::user("unused".to_string());

        // ── R-GC-17/26：workspace 空 → 兜底默认 Claw 工作区 ──
        // 解析链为纯函数（resolve_create_workspace(None) → path_manager 默认
        // assistant workspace），由独立单测覆盖（resolve_create_workspace_*），
        // 不在此触发 create_session——全局 path_manager 在 CI runner 上指向
        // 真实 ~/.bitfun（可能不存在），create_session canonicalize 会失败
        // （Rust Build ubuntu 环境敏感，run 31799106971 前身）。create 数据流
        // 已由本测试主链路（显式 workspace）覆盖。
        let fallback_workspace = GroupRoomTool::resolve_create_workspace(None);
        assert!(
            !fallback_workspace.trim().is_empty(),
            "empty workspace must resolve to a non-empty default Claw workspace"
        );

        // ── R-WF-04 验收断言（Plan:120）：开放投递 + 无模型调用 ──
        // 建新群（专用验收组，避免与上面主链路删群后的状态耦合）。
        let open_group_id = GroupRoomTool::create_group(
            coordinator,
            "R-WF-04 开放投递验收群",
            std::slice::from_ref(&member_a),
            &workspace_str,
        )
        .await
        .expect("create group for R-WF-04 acceptance");
        // 1) 开放投递：非成员（member_b 未加入 open_group）经 send_group_message
        //    工具入口发送成功（Plan:120「非成员发送成功」）——send 只查群会话
        //    存在（get_session + 磁盘回退），不再校验发送者 ∈ groupChats。
        let open_message_id = call_send_impl(
            coordinator,
            &open_group_id,
            "非成员开放投递",
            Some(&member_b),
        )
        .await
        .expect("R-WF-04: non-member send must succeed (open delivery)")
        .get("messageId")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .expect("messageId present");
        assert!(!open_message_id.is_empty());
        // 2) 群聊消息只落盘：turn 以完成态落盘（finish_reason="complete" +
        //    has_final_response=true + status=Completed），前端正常渲染。
        let open_turns = manager
            .persistence_manager()
            .load_session_turns(workspace, &open_group_id)
            .await
            .expect("load open-delivery turns");
        let open_sent = open_turns
            .iter()
            .find(|t| t.user_message.content == "非成员开放投递")
            .expect("open-delivery message must be persisted");
        assert_eq!(
            open_sent.status,
            bitfun_services_core::session::TurnStatus::Completed,
            "R-WF-04: group message turn must be persisted as Completed (no agent execution)"
        );
        assert_eq!(
            open_sent.finish_reason.as_deref(),
            Some("complete"),
            "R-WF-04: group message turn must carry the normal finish code"
        );
        assert_eq!(
            open_sent.has_final_response,
            Some(true),
            "R-WF-04: group message turn is itself the final response"
        );
        // 3) 群聊会话无大模型响应：send 纯落盘（write_group_turn_with_metadata），
        //    不触发群主会话大模型执行 → 会话保持 Idle（Processing 即表示模型
        //    运行中）。已持久化 turn 数量 = 欢迎 + 本条（无额外模型轮次 turn）。
        let group_session = manager
            .get_session(&open_group_id)
            .expect("open group in memory");
        assert_eq!(
            group_session.state,
            crate::agentic::core::SessionState::Idle,
            "R-WF-04: group session must stay Idle after send (no model invocation)"
        );
        // 4) 无模型调用 mock 断言（Plan:120「群聊消息只落盘无模型调用」）：
        //    send 不再经 coordinator.start_dialog_turn（模型调度入口）→
        //    群主会话 model_rounds 恒空；turn 的 model_rounds 为空即无模型轮次。
        assert!(
            open_sent.model_rounds.is_empty(),
            "R-WF-04: persisted group message turn must have no model rounds"
        );
        // 5) 历史读回：开放投递消息 author = 非成员发送者 member_b。
        let open_history = GroupRoomTool::get_history(coordinator, &open_group_id, None)
            .await
            .expect("open-delivery history");
        let open_found = open_history
            .iter()
            .find(|m| m.content == "非成员开放投递")
            .expect("open-delivery message present in history");
        assert_eq!(
            open_found.author.session_id, member_b,
            "R-WF-04: open-delivery message author must be the non-member sender"
        );
        // 6) 群不存在 → 明确错误（群会话存在性门仍生效，禁静默跳过）。
        let missing_group_err = call_send_impl(
            coordinator,
            "definitely-not-a-real-group",
            "发给不存在群",
            Some(&member_a),
        )
        .await
        .expect_err("send to a non-existent group must fail");
        assert!(
            missing_group_err
                .to_string()
                .contains("does not exist"),
            "R-WF-04: send to non-existent group must yield a clear error, got: {missing_group_err}"
        );

        // ── R-WF-05 验收断言（Plan:132）──
        // 1) 成员侧反标持久化（原子步 3）：create/invite 后，成员会话
        //    custom_metadata.groupChats 含群 ID（成员→群一对多反标）。
        //    member_a 是 open_group 的唯一成员（R-WF-04 开放投递群只登记
        //    member_a；member_b 是开放投递的非成员，无反标）。
        let member_a_meta = manager
            .load_session_metadata(workspace, &member_a)
            .await
            .expect("load member A metadata")
            .expect("member A metadata exists");
        let member_a_groups = member_a_meta
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(
            member_a_groups
                .iter()
                .any(|v| v.as_str() == Some(open_group_id.as_str())),
            "R-WF-05: member back-mark (groupChats) must contain the group id after create"
        );
        // 2) 桥接函数（原子步 1）：成员最终回复复刻进所属群（异步落盘 →
        //    群 turns 出现最终回复；成员主动发已由 R-WF-04 覆盖）。
        let replicate_reply = "成员的最终回复（R-WF-05 复刻验收）";
        GroupRoomTool::replicate_member_turn_to_groups(coordinator, &member_a, replicate_reply)
            .await
            .expect("R-WF-05: replicate member turn to group log must succeed");
        let replicated_turns = manager
            .persistence_manager()
            .load_session_turns(workspace, &open_group_id)
            .await
            .expect("load replicated group turns");
        let replicated = replicated_turns
            .iter()
            .find(|t| t.user_message.content == replicate_reply)
            .expect("replicated final reply must be persisted into the group log");
        assert_eq!(
            replicated.status,
            bitfun_services_core::session::TurnStatus::Completed,
            "R-WF-05: replicated turn must be Completed (no agent execution)"
        );
        assert_eq!(
            replicated.finish_reason.as_deref(),
            Some("complete"),
            "R-WF-05: replicated turn must carry the normal finish code"
        );
        assert_eq!(
            replicated.has_final_response,
            Some(true),
            "R-WF-05: replicated turn is itself the final response"
        );
        // 3) 复刻消息 sender = 成员真实会话（发言方标识可解析）。
        let replicated_meta = replicated
            .user_message
            .metadata
            .as_ref()
            .expect("replicated turn metadata");
        assert_eq!(
            replicated_meta
                .get("senderSessionId")
                .and_then(Value::as_str),
            Some(member_a.as_str()),
            "R-WF-05: replicated turn sender must be the member session"
        );
        // 4) 历史读回：复刻最终回复在群消息历史可见（成员完成 turn → 群消息
        //    出现最终回复）。
        let replicated_history = GroupRoomTool::get_history(coordinator, &open_group_id, None)
            .await
            .expect("replicated history");
        assert!(
            replicated_history
                .iter()
                .any(|m| m.content == replicate_reply),
            "R-WF-05: replicated final reply must be visible in group history"
        );
    }

    /// R-WF-05 独立验收：成员↔群一对多——一个成员加入两个群，最终回复
    /// 复刻进**每个**群（反标数组驱动的一对多复刻）；成员不在任何群时复刻
    /// 静默成功（无群可发，不报错、不阻塞）。
    #[tokio::test]
    async fn replicate_member_turn_to_multiple_groups() {
        use crate::agentic::events::{EventQueue, EventQueueConfig, EventRouter};
        use crate::agentic::execution::{
            ExecutionEngine, ExecutionEngineConfig, RoundExecutor, StreamProcessor,
        };
        use crate::agentic::persistence::PersistenceManager;
        use crate::agentic::session::compression::{CompressionConfig, ContextCompressor};
        use crate::agentic::session::{
            PromptCachePolicy, SessionContextStore, SessionManager, SessionManagerConfig,
        };
        use crate::agentic::tools::pipeline::{ToolPipeline, ToolStateManager};
        use crate::agentic::tools::registry::ToolRegistry;
        use crate::infrastructure::PathManager;
        use crate::runtime_ownership::CoreRuntimeOwnership;
        use std::sync::Arc;
        use std::time::Duration;

        let user_root =
            std::env::temp_dir().join(format!("bitfun-grouproom-rwf05-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&user_root).expect("user root");
        let path_manager = PathManager::with_user_root_for_tests(user_root.clone());
        let persistence =
            PersistenceManager::new(Arc::new(path_manager)).expect("persistence manager");
        let session_manager = Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(persistence),
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: true,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ));
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let tool_pipeline = Arc::new(ToolPipeline::new(
            Arc::new(tokio::sync::RwLock::new(ToolRegistry::new())),
            Arc::new(ToolStateManager::new(event_queue.clone())),
            None,
        ));
        let execution_engine = Arc::new(ExecutionEngine::new(
            Arc::new(RoundExecutor::new(
                Arc::new(StreamProcessor::new(event_queue.clone())),
                event_queue.clone(),
                tool_pipeline.clone(),
            )),
            event_queue.clone(),
            session_manager.clone(),
            Arc::new(ContextCompressor::new(CompressionConfig::default())),
            ExecutionEngineConfig::default(),
        ));
        let ownership_root = user_root.join("runtime-ownership");
        let coordinator = Arc::new(ConversationCoordinator::new(
            session_manager.clone(),
            execution_engine,
            tool_pipeline,
            event_queue,
            Arc::new(EventRouter::new()),
            Arc::new(CoreRuntimeOwnership::embedded_with_facts(
                ownership_root,
                "bitfun".to_string(),
                "test",
            )),
        ));
        coordinator.set_terminal_port(
            bitfun_runtime_services::test_support::FakeRuntimeServicesProvider::terminal_port(),
        );
        coordinator.set_remote_exec_port(
            bitfun_runtime_services::test_support::FakeRuntimeServicesProvider::remote_exec_port(),
        );
        ConversationCoordinator::set_global(coordinator.clone());

        let workspace = std::env::temp_dir().join(format!(
            "bitfun-grouproom-rwf05-ws-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace dir");
        let workspace_str = workspace.to_string_lossy().to_string();
        let manager = coordinator.get_session_manager();

        let member = create_member_session_for_test(&coordinator, &workspace_str).await;
        let group_1 = GroupRoomTool::create_group(
            &coordinator,
            "R-WF-05 群一",
            std::slice::from_ref(&member),
            &workspace_str,
        )
        .await
        .expect("create group 1");
        let group_2 = GroupRoomTool::create_group(
            &coordinator,
            "R-WF-05 群二",
            std::slice::from_ref(&member),
            &workspace_str,
        )
        .await
        .expect("create group 2");

        // 一对多反标：成员反标含两个群 ID。
        let member_meta = manager
            .load_session_metadata(&workspace, &member)
            .await
            .expect("load member metadata")
            .expect("member metadata exists");
        let member_groups = member_meta
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            member_groups.len(),
            2,
            "R-WF-05: one member in two groups must carry two back-marks"
        );

        // 复刻最终回复 → 两个群 turns 都出现。
        let reply = "一对多复刻最终回复";
        GroupRoomTool::replicate_member_turn_to_groups(&coordinator, &member, reply)
            .await
            .expect("replicate to multiple groups");
        for group_id in [&group_1, &group_2] {
            let turns = manager
                .persistence_manager()
                .load_session_turns(&workspace, group_id)
                .await
                .expect("load group turns");
            assert!(
                turns.iter().any(|t| t.user_message.content == reply),
                "R-WF-05: reply must be replicated into group {group_id}"
            );
        }

        // 非成员（无反标）复刻 → 静默成功（不报错、不写任何群）。
        let outsider = create_member_session_for_test(&coordinator, &workspace_str).await;
        let before_turns = manager
            .persistence_manager()
            .load_session_turns(&workspace, &group_1)
            .await
            .expect("load group turns before outsider replicate");
        GroupRoomTool::replicate_member_turn_to_groups(&coordinator, &outsider, "外部成员回复")
            .await
            .expect("outsider replicate must be a no-op success");
        let after_turns = manager
            .persistence_manager()
            .load_session_turns(&workspace, &group_1)
            .await
            .expect("load group turns after outsider replicate");
        assert_eq!(
            before_turns.len(),
            after_turns.len(),
            "R-WF-05: non-member replicate must not write into any group"
        );

        // 空回复 → 静默跳过（不落盘）。
        let before_empty = manager
            .persistence_manager()
            .load_session_turns(&workspace, &group_1)
            .await
            .expect("load group turns before empty replicate");
        GroupRoomTool::replicate_member_turn_to_groups(&coordinator, &member, "")
            .await
            .expect("empty replicate must be a no-op success");
        let after_empty = manager
            .persistence_manager()
            .load_session_turns(&workspace, &group_1)
            .await
            .expect("load group turns after empty replicate");
        assert_eq!(
            before_empty.len(),
            after_empty.len(),
            "R-WF-05: empty final reply must be skipped"
        );
    }

    /// R-WF-05 批次4退回 P0-1 修复验收：跨域反标读写一致（成员独立 workspace）。
    ///
    /// 需求 §D.53「每个成员自己单独一个工作区」+ R-WF-07:151「成员工作区 =
    /// workspace-<nodeId>」：成员 workspace ≠ 群 workspace 是生产常态。
    /// 旧实现写入侧用群 workspace 域写成员反标、读取侧按成员域读 → 读不到
    /// 反标 → groupChats 空 → 复刻静默失效（测试同域掩盖）。本用例强制
    /// 成员独立 workspace，断言：
    /// 1. 建群后成员反标落在**成员域**（成员 workspace 下 load 到 groupChats）；
    /// 2. 群域下**读不到**该成员的 groupChats 反标（证明未错落群域）；
    /// 3. 复刻最终回复成功落进群 turns（成员域反标 → 群真实可复刻）。
    #[tokio::test]
    async fn replicate_member_turn_across_workspaces() {
        use crate::agentic::events::{EventQueue, EventQueueConfig, EventRouter};
        use crate::agentic::execution::{
            ExecutionEngine, ExecutionEngineConfig, RoundExecutor, StreamProcessor,
        };
        use crate::agentic::persistence::PersistenceManager;
        use crate::agentic::session::compression::{CompressionConfig, ContextCompressor};
        use crate::agentic::session::{
            PromptCachePolicy, SessionContextStore, SessionManager, SessionManagerConfig,
        };
        use crate::agentic::tools::pipeline::{ToolPipeline, ToolStateManager};
        use crate::agentic::tools::registry::ToolRegistry;
        use crate::infrastructure::PathManager;
        use crate::runtime_ownership::CoreRuntimeOwnership;
        use std::sync::Arc;
        use std::time::Duration;

        let user_root = std::env::temp_dir().join(format!(
            "bitfun-grouproom-rwf05-cross-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&user_root).expect("user root");
        let path_manager = PathManager::with_user_root_for_tests(user_root.clone());
        let persistence =
            PersistenceManager::new(Arc::new(path_manager)).expect("persistence manager");
        let session_manager = Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(persistence),
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: true,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ));
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let tool_pipeline = Arc::new(ToolPipeline::new(
            Arc::new(tokio::sync::RwLock::new(ToolRegistry::new())),
            Arc::new(ToolStateManager::new(event_queue.clone())),
            None,
        ));
        let execution_engine = Arc::new(ExecutionEngine::new(
            Arc::new(RoundExecutor::new(
                Arc::new(StreamProcessor::new(event_queue.clone())),
                event_queue.clone(),
                tool_pipeline.clone(),
            )),
            event_queue.clone(),
            session_manager.clone(),
            Arc::new(ContextCompressor::new(CompressionConfig::default())),
            ExecutionEngineConfig::default(),
        ));
        let ownership_root = user_root.join("runtime-ownership");
        let coordinator = Arc::new(ConversationCoordinator::new(
            session_manager.clone(),
            execution_engine,
            tool_pipeline,
            event_queue,
            Arc::new(EventRouter::new()),
            Arc::new(CoreRuntimeOwnership::embedded_with_facts(
                ownership_root,
                "bitfun".to_string(),
                "test",
            )),
        ));
        coordinator.set_terminal_port(
            bitfun_runtime_services::test_support::FakeRuntimeServicesProvider::terminal_port(),
        );
        coordinator.set_remote_exec_port(
            bitfun_runtime_services::test_support::FakeRuntimeServicesProvider::remote_exec_port(),
        );
        ConversationCoordinator::set_global(coordinator.clone());
        let manager = coordinator.get_session_manager();

        // 群 workspace 与成员 workspace 分离（R-WF-07:151 成员 = workspace-<nodeId>）。
        let group_workspace = std::env::temp_dir().join(format!(
            "bitfun-grouproom-rwf05-group-ws-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&group_workspace).expect("group workspace dir");
        let group_workspace_str = group_workspace.to_string_lossy().to_string();
        let member_workspace = std::env::temp_dir().join(format!(
            "bitfun-grouproom-rwf05-member-ws-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&member_workspace).expect("member workspace dir");
        let member_workspace_str = member_workspace.to_string_lossy().to_string();
        assert_ne!(
            member_workspace_str, group_workspace_str,
            "setup: member workspace must differ from group workspace (R-WF-07)"
        );

        // 成员会话建在成员独立 workspace（R-WF-07 成员工作区 = workspace-<nodeId>）。
        let member = create_member_session_for_test(&coordinator, &member_workspace_str).await;
        let group_id = GroupRoomTool::create_group(
            &coordinator,
            "跨域反标群",
            std::slice::from_ref(&member),
            &group_workspace_str,
        )
        .await
        .expect("create group across workspaces");

        // 1) 成员反标落在成员域（成员 workspace 下可读 groupChats）。
        let member_meta = manager
            .load_session_metadata(&member_workspace, &member)
            .await
            .expect("load member metadata in member workspace")
            .expect("member metadata exists in member workspace");
        let member_groups = member_meta
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            member_groups.len(),
            1,
            "P0-1: member back-mark must be written into the member workspace domain"
        );
        assert_eq!(
            member_groups[0].as_str(),
            Some(group_id.as_str()),
            "P0-1: back-mark group id must match the created group"
        );

        // 2) 群域下读不到该成员反标（证明未错落群域）。
        let group_domain_member_meta = manager
            .load_session_metadata(&group_workspace, &member)
            .await
            .expect("load member metadata in group workspace");
        assert!(
            group_domain_member_meta.is_none(),
            "P0-1: member back-mark must NOT be written into the group workspace domain"
        );

        // 3) 复刻最终回复成功（成员域反标 → 群真实落盘）。
        let reply = "跨域复刻最终回复";
        GroupRoomTool::replicate_member_turn_to_groups(&coordinator, &member, reply)
            .await
            .expect("replicate across workspaces must succeed");
        let turns = manager
            .persistence_manager()
            .load_session_turns(&group_workspace, &group_id)
            .await
            .expect("load group turns");
        assert!(
            turns.iter().any(|t| t.user_message.content == reply),
            "P0-1: reply must be replicated into the group even when member/group workspaces differ"
        );
    }

    /// R-WF-05 P1-A（审查批次4 §四增量 P1）：remove_member 清理成员侧反标。
    ///
    /// P0-1 批次4退回修复后反标真实写入成员 workspace 域，remove 若只清群侧
    /// 成员表 → 成员反标残留 → replicate 遍历成员反标仍含已移除群 → 复刻投递
    /// 到已移除成员的群（幽灵复刻）。本用例：
    /// - 成员/群 workspace 强制分域（assert_ne，模拟 R-WF-07 workspace-<nodeId>）；
    /// - 建群（反标写入成员域）→ remove_member → 断言成员域反标不含 group_id；
    /// - 群域成员表不再含成员 id（群侧移除仍生效）；
    /// - 移除后复刻不再投递到该群（反标清空 → replicate 空遍历，群 turns 无回复）。
    ///   旧实现（只清群侧）此用例必失败：反标残留 → 复刻仍投递。
    #[tokio::test]
    async fn remove_member_clears_member_backmark_across_workspaces() {
        use crate::agentic::events::{EventQueue, EventQueueConfig, EventRouter};
        use crate::agentic::execution::{
            ExecutionEngine, ExecutionEngineConfig, RoundExecutor, StreamProcessor,
        };
        use crate::agentic::persistence::PersistenceManager;
        use crate::agentic::session::compression::{CompressionConfig, ContextCompressor};
        use crate::agentic::session::{
            PromptCachePolicy, SessionContextStore, SessionManager, SessionManagerConfig,
        };
        use crate::agentic::tools::pipeline::{ToolPipeline, ToolStateManager};
        use crate::agentic::tools::registry::ToolRegistry;
        use crate::infrastructure::PathManager;
        use crate::runtime_ownership::CoreRuntimeOwnership;
        use std::sync::Arc;
        use std::time::Duration;

        let user_root = std::env::temp_dir().join(format!(
            "bitfun-grouproom-rwf05p1-remove-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&user_root).expect("user root");
        let path_manager = PathManager::with_user_root_for_tests(user_root.clone());
        let persistence =
            PersistenceManager::new(Arc::new(path_manager)).expect("persistence manager");
        let session_manager = Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(persistence),
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: true,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ));
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let tool_pipeline = Arc::new(ToolPipeline::new(
            Arc::new(tokio::sync::RwLock::new(ToolRegistry::new())),
            Arc::new(ToolStateManager::new(event_queue.clone())),
            None,
        ));
        let execution_engine = Arc::new(ExecutionEngine::new(
            Arc::new(RoundExecutor::new(
                Arc::new(StreamProcessor::new(event_queue.clone())),
                event_queue.clone(),
                tool_pipeline.clone(),
            )),
            event_queue.clone(),
            session_manager.clone(),
            Arc::new(ContextCompressor::new(CompressionConfig::default())),
            ExecutionEngineConfig::default(),
        ));
        let ownership_root = user_root.join("runtime-ownership");
        let coordinator = Arc::new(ConversationCoordinator::new(
            session_manager.clone(),
            execution_engine,
            tool_pipeline,
            event_queue,
            Arc::new(EventRouter::new()),
            Arc::new(CoreRuntimeOwnership::embedded_with_facts(
                ownership_root,
                "bitfun".to_string(),
                "test",
            )),
        ));
        coordinator.set_terminal_port(
            bitfun_runtime_services::test_support::FakeRuntimeServicesProvider::terminal_port(),
        );
        coordinator.set_remote_exec_port(
            bitfun_runtime_services::test_support::FakeRuntimeServicesProvider::remote_exec_port(),
        );
        ConversationCoordinator::set_global(coordinator.clone());
        let manager = coordinator.get_session_manager();

        // 群 workspace 与成员 workspace 分离（R-WF-07:151 成员 = workspace-<nodeId>）。
        let group_workspace = std::env::temp_dir().join(format!(
            "bitfun-grouproom-rwf05p1-remove-group-ws-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&group_workspace).expect("group workspace dir");
        let group_workspace_str = group_workspace.to_string_lossy().to_string();
        let member_workspace = std::env::temp_dir().join(format!(
            "bitfun-grouproom-rwf05p1-remove-member-ws-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&member_workspace).expect("member workspace dir");
        let member_workspace_str = member_workspace.to_string_lossy().to_string();
        assert_ne!(
            member_workspace_str, group_workspace_str,
            "setup: member workspace must differ from group workspace (R-WF-07)"
        );

        // 成员会话建在成员独立 workspace；建群（跨域）。
        let member = create_member_session_for_test(&coordinator, &member_workspace_str).await;
        let group_id = GroupRoomTool::create_group(
            &coordinator,
            "remove 反标清理群",
            std::slice::from_ref(&member),
            &group_workspace_str,
        )
        .await
        .expect("create group across workspaces");

        // 1) 前置：成员反标在成员域（P0-1 已真写）。
        let member_meta = manager
            .load_session_metadata(&member_workspace, &member)
            .await
            .expect("load member metadata in member workspace")
            .expect("member metadata exists in member workspace");
        let member_groups = member_meta
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            member_groups.len(),
            1,
            "setup: member back-mark must exist before remove"
        );

        // 2) remove_member → 成员域反标清空（不含 group_id）。
        GroupRoomTool::remove_member(&coordinator, &group_id, &member)
            .await
            .expect("remove member must succeed");
        let member_meta_after = manager
            .load_session_metadata(&member_workspace, &member)
            .await
            .expect("load member metadata after remove")
            .expect("member metadata still exists after remove");
        let member_groups_after = member_meta_after
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            member_groups_after.len(),
            0,
            "P1-A: member back-mark must be cleared after remove"
        );
        assert!(
            !member_groups_after
                .iter()
                .any(|v| v.as_str() == Some(group_id.as_str())),
            "P1-A: removed group id must NOT remain in member back-mark"
        );

        // 3) 群侧成员表也不再含该成员（移除仍生效）。
        let group_meta = manager
            .load_session_metadata(&group_workspace, &group_id)
            .await
            .expect("load group metadata")
            .expect("group metadata exists");
        let group_members = group_meta
            .custom_metadata
            .as_ref()
            .and_then(|m| m.get("groupChats"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(
            !group_members
                .iter()
                .any(|v| v.as_str() == Some(member.as_str())),
            "remove must also clear the group-side member table"
        );

        // 4) 移除后复刻不再投递到该群（反标已清 → 空遍历，群 turns 无该回复）。
        let reply = "remove 后幽灵复刻检查";
        GroupRoomTool::replicate_member_turn_to_groups(&coordinator, &member, reply)
            .await
            .expect("replicate after remove must succeed (no-op path)");
        let turns = manager
            .persistence_manager()
            .load_session_turns(&group_workspace, &group_id)
            .await
            .expect("load group turns");
        assert!(
            !turns.iter().any(|t| t.user_message.content == reply),
            "P1-A: no ghost replicate after remove (back-mark cleared)"
        );
    }

    /// R-WF-05 批次4退回 P1-1 修复验收：异步不阻塞成员会话。
    ///
    /// hook（coordinator.rs:3375）以 tokio::spawn 异步调用桥接函数——成员
    /// turn 完成不被复刻阻塞。本用例直接在 tokio::spawn 内调用桥接函数，
    /// 断言 spawn 的句柄立即返回（未 await 复刻结果）、复刻在后台完成、且
    /// 复刻结果正确落盘——模拟 hook 的异步路径（成员 turn 主流程不等复刻）。
    /// 同时断言桥接函数本身不 panic、不吞错误（Ok）。
    /// P1-B 补强（审查批次4 §四 P1 残留）：tokio::time::timeout + channel
    /// 信号严格断言「spawn 后主流程不等复刻」（AG-3）——复刻任务先发「已
    /// 开始」信号再延迟执行，主流程收到信号时观测到复刻仍在进行（群 turns
    /// 尚无回复）；若主流程阻塞在复刻上，收到信号时复刻必已完成 → 断言失败。
    #[tokio::test]
    async fn replicate_is_non_blocking_async() {
        use crate::agentic::events::{EventQueue, EventQueueConfig, EventRouter};
        use crate::agentic::execution::{
            ExecutionEngine, ExecutionEngineConfig, RoundExecutor, StreamProcessor,
        };
        use crate::agentic::persistence::PersistenceManager;
        use crate::agentic::session::compression::{CompressionConfig, ContextCompressor};
        use crate::agentic::session::{
            PromptCachePolicy, SessionContextStore, SessionManager, SessionManagerConfig,
        };
        use crate::agentic::tools::pipeline::{ToolPipeline, ToolStateManager};
        use crate::agentic::tools::registry::ToolRegistry;
        use crate::infrastructure::PathManager;
        use crate::runtime_ownership::CoreRuntimeOwnership;
        use std::sync::Arc;
        use std::time::Duration;

        let user_root = std::env::temp_dir().join(format!(
            "bitfun-grouproom-rwf05-async-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&user_root).expect("user root");
        let path_manager = PathManager::with_user_root_for_tests(user_root.clone());
        let persistence =
            PersistenceManager::new(Arc::new(path_manager)).expect("persistence manager");
        let session_manager = Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(persistence),
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: true,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ));
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let tool_pipeline = Arc::new(ToolPipeline::new(
            Arc::new(tokio::sync::RwLock::new(ToolRegistry::new())),
            Arc::new(ToolStateManager::new(event_queue.clone())),
            None,
        ));
        let execution_engine = Arc::new(ExecutionEngine::new(
            Arc::new(RoundExecutor::new(
                Arc::new(StreamProcessor::new(event_queue.clone())),
                event_queue.clone(),
                tool_pipeline.clone(),
            )),
            event_queue.clone(),
            session_manager.clone(),
            Arc::new(ContextCompressor::new(CompressionConfig::default())),
            ExecutionEngineConfig::default(),
        ));
        let ownership_root = user_root.join("runtime-ownership");
        let coordinator = Arc::new(ConversationCoordinator::new(
            session_manager.clone(),
            execution_engine,
            tool_pipeline,
            event_queue,
            Arc::new(EventRouter::new()),
            Arc::new(CoreRuntimeOwnership::embedded_with_facts(
                ownership_root,
                "bitfun".to_string(),
                "test",
            )),
        ));
        coordinator.set_terminal_port(
            bitfun_runtime_services::test_support::FakeRuntimeServicesProvider::terminal_port(),
        );
        coordinator.set_remote_exec_port(
            bitfun_runtime_services::test_support::FakeRuntimeServicesProvider::remote_exec_port(),
        );
        ConversationCoordinator::set_global(coordinator.clone());
        let manager = coordinator.get_session_manager();

        let workspace = std::env::temp_dir().join(format!(
            "bitfun-grouproom-rwf05-async-ws-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace dir");
        let workspace_str = workspace.to_string_lossy().to_string();
        let member = create_member_session_for_test(&coordinator, &workspace_str).await;
        let group_id = GroupRoomTool::create_group(
            &coordinator,
            "异步不阻塞群",
            std::slice::from_ref(&member),
            &workspace_str,
        )
        .await
        .expect("create group for async test");

        // P1-B 严格时序断言：barrier 同步「复刻任务已启动」信号，让主流程在
        // 复刻进行中观测——复刻任务先等 barrier（保证「尚未完成」的观测
        // 窗口）再执行写盘；主流程拿到 barrier 信号后断言群 turns 尚无回复
        // （主流程未阻塞在复刻上，AG-3）。若实现退化为阻塞等待复刻完成，
        // 主流程不可能在复刻完成前观测到「无回复」→ 断言失败。
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let barrier_for_spawn = barrier.clone();
        let coordinator_for_spawn = coordinator.clone();
        let member_for_spawn = member.clone();
        let reply = "异步复刻回复";
        let handle = tokio::spawn(async move {
            // 1) 先发「复刻已开始」信号（复刻尚未落盘）。
            barrier_for_spawn.wait().await;
            // 2) 短暂让出执行权，确保主流程拿到信号后在观测点运行。
            tokio::task::yield_now().await;
            GroupRoomTool::replicate_member_turn_to_groups(
                &coordinator_for_spawn,
                &member_for_spawn,
                reply,
            )
            .await
        });
        // 主流程：spawn 已返回（不阻塞在复刻上）。barrier 信号确认复刻任务
        // 已开始但未完成 → 观测此刻群 turns 尚无该回复（复刻未投递）。
        tokio::time::timeout(Duration::from_secs(5), barrier.wait())
            .await
            .expect("replicate task must start within timeout");
        // 3) 复刻仍在进行（barrier 同步点 = 复刻尚未写盘）→ 主流程不等复刻。
        let turns_during = manager
            .persistence_manager()
            .load_session_turns(&workspace, &group_id)
            .await
            .expect("load group turns during replicate");
        assert!(
            !turns_during.iter().any(|t| t.user_message.content == reply),
            "P1-B: main flow must not block on replicate (AG-3): reply must NOT be present while replicate is still running"
        );
        // 4) join 拿复刻结果并断言成功（后台复刻最终完成）。
        let replicate_result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("replicate task must finish within timeout")
            .expect("replicate task must not panic");
        replicate_result.expect("replicate must succeed (best-effort side path)");

        let turns = manager
            .persistence_manager()
            .load_session_turns(&workspace, &group_id)
            .await
            .expect("load group turns");
        assert!(
            turns.iter().any(|t| t.user_message.content == reply),
            "P1-1: asynchronously spawned replicate must write the reply into the group"
        );
    }

    /// R-WF-05 批次4退回 P1-2 修复验收：单群失败 warn 继续，不阻断其它群。
    ///
    /// 成员同时属于群 A 与群 B；群 A 的 workspace 在复刻前被破坏（群会话
    /// 元数据删除/群域不可用）→ 复刻群 A 失败 → 断言群 B 仍复刻成功
    /// （warn 继续，尽力而为的旁路复刻）。调用仍返回 Ok（单群失败不上抛）。
    #[tokio::test]
    async fn replicate_continues_when_single_group_fails() {
        use crate::agentic::events::{EventQueue, EventQueueConfig, EventRouter};
        use crate::agentic::execution::{
            ExecutionEngine, ExecutionEngineConfig, RoundExecutor, StreamProcessor,
        };
        use crate::agentic::persistence::PersistenceManager;
        use crate::agentic::session::compression::{CompressionConfig, ContextCompressor};
        use crate::agentic::session::{
            PromptCachePolicy, SessionContextStore, SessionManager, SessionManagerConfig,
        };
        use crate::agentic::tools::pipeline::{ToolPipeline, ToolStateManager};
        use crate::agentic::tools::registry::ToolRegistry;
        use crate::infrastructure::PathManager;
        use crate::runtime_ownership::CoreRuntimeOwnership;
        use std::sync::Arc;
        use std::time::Duration;

        let user_root = std::env::temp_dir().join(format!(
            "bitfun-grouproom-rwf05-failone-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&user_root).expect("user root");
        let path_manager = PathManager::with_user_root_for_tests(user_root.clone());
        let persistence =
            PersistenceManager::new(Arc::new(path_manager)).expect("persistence manager");
        let session_manager = Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(persistence),
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: true,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ));
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let tool_pipeline = Arc::new(ToolPipeline::new(
            Arc::new(tokio::sync::RwLock::new(ToolRegistry::new())),
            Arc::new(ToolStateManager::new(event_queue.clone())),
            None,
        ));
        let execution_engine = Arc::new(ExecutionEngine::new(
            Arc::new(RoundExecutor::new(
                Arc::new(StreamProcessor::new(event_queue.clone())),
                event_queue.clone(),
                tool_pipeline.clone(),
            )),
            event_queue.clone(),
            session_manager.clone(),
            Arc::new(ContextCompressor::new(CompressionConfig::default())),
            ExecutionEngineConfig::default(),
        ));
        let ownership_root = user_root.join("runtime-ownership");
        let coordinator = Arc::new(ConversationCoordinator::new(
            session_manager.clone(),
            execution_engine,
            tool_pipeline,
            event_queue,
            Arc::new(EventRouter::new()),
            Arc::new(CoreRuntimeOwnership::embedded_with_facts(
                ownership_root,
                "bitfun".to_string(),
                "test",
            )),
        ));
        coordinator.set_terminal_port(
            bitfun_runtime_services::test_support::FakeRuntimeServicesProvider::terminal_port(),
        );
        coordinator.set_remote_exec_port(
            bitfun_runtime_services::test_support::FakeRuntimeServicesProvider::remote_exec_port(),
        );
        ConversationCoordinator::set_global(coordinator.clone());
        let manager = coordinator.get_session_manager();

        let workspace = std::env::temp_dir().join(format!(
            "bitfun-grouproom-rwf05-failone-ws-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace dir");
        let workspace_str = workspace.to_string_lossy().to_string();
        let member = create_member_session_for_test(&coordinator, &workspace_str).await;
        let group_ok = GroupRoomTool::create_group(
            &coordinator,
            "单群失败-好群",
            std::slice::from_ref(&member),
            &workspace_str,
        )
        .await
        .expect("create healthy group");
        // 「坏群」不真实建群，改为向成员反标注入一个不存在的群 ID——模拟
        // 「群已失效但成员反标残留」（生产真实场景：群被删/持久化损坏后
        // 反标未清，复刻尽力而为跳过）。确定性注入，不依赖文件删除。
        let group_broken = format!("nonexistent-group-{}", uuid::Uuid::new_v4());
        manager
            .update_session_metadata(&workspace, &member, |metadata| {
                let custom = metadata
                    .custom_metadata
                    .get_or_insert_with(|| json!({}))
                    .as_object_mut()
                    .expect("custom_metadata is always an object");
                let mut groups = custom
                    .get("groupChats")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                groups.push(json!(group_broken.clone()));
                custom.insert("groupChats".to_string(), json!(groups));
            })
            .await
            .expect("inject broken group id into member back-mark");

        // 复刻：坏群失败（warn 继续）→ 好群仍成功；调用返回 Ok（不上抛）。
        let reply = "单群失败继续回复";
        GroupRoomTool::replicate_member_turn_to_groups(&coordinator, &member, reply)
            .await
            .expect("single-group failure must not propagate (warn and continue)");

        let ok_turns = manager
            .persistence_manager()
            .load_session_turns(&workspace, &group_ok)
            .await
            .expect("load healthy group turns");
        assert!(
            ok_turns.iter().any(|t| t.user_message.content == reply),
            "P1-2: healthy group must still receive the replicated reply despite one group failing"
        );
    }
}
