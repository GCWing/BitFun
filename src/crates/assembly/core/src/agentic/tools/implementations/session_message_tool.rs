use super::session_control_tool::{
    get_available_agent_type_ids_for_creation, resolve_session_mutation_authorization,
    SessionControlWorkspaceTarget, SessionMutationAuthOptions, SessionWorktreeCreateResult,
};
use super::util::normalize_path;
use crate::agentic::agents::AcpAgent;
use crate::agentic::coordination::plan_todo_binding::{
    PLAN_FILE_METADATA_KEY, TODO_ID_METADATA_KEY,
};
use crate::agentic::coordination::{
    get_global_coordinator, get_global_scheduler, ConversationCoordinator, DialogScheduler,
    DialogSubmissionPolicy, DialogTriggerSource,
};
use crate::agentic::events::AgenticEvent;
use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::workspace_paths::posix_style_path_is_absolute;
use crate::service::workspace::get_global_workspace_service;
use crate::service::worktree::WorktreeService;
use crate::service_agent_runtime::CoreServiceAgentRuntime;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_core_types::{SessionExecutionTarget, WorktreeSessionOptions};
use bitfun_runtime_ports::{
    AcpClientBitfunMessageRequest, AcpClientMessageRequest, AcpClientMessageResult, AcpClientPort,
    AcpClientStreamChunk, AcpClientStreamChunkSink, AgentDialogPrependedReminder,
    AgentDialogSteerRequest, AgentDialogTurnPort, AgentDialogTurnRequest,
    AgentSessionCreateRequest, AgentSessionListRequest, AgentSessionReplyRoute,
    AgentSessionSummary, AgentSessionWorkspaceBinding, AgentSessionWorkspaceRequest, PortResult,
};
use log::{info, warn};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

/// Primary channel for legion communication. With a session_id, messages can be sent and received across conversations.
/// Obtain session_id via Task spawn or SessionControl list_tasks.
pub struct SessionMessageTool;

#[derive(Debug, Clone)]
struct SessionMessageWorkspaceTarget {
    workspace_path: String,
    project_workspace_path: String,
    execution_target: Option<SessionExecutionTarget>,
    workspace_id: Option<String>,
    remote_connection_id: Option<String>,
    remote_ssh_host: Option<String>,
}

/// Source-session facts and global runtime handles shared by a single
/// dispatch and by every batch item. Built once per tool call so a batch
/// dispatch performs a single resource setup.
struct DispatchShared {
    source_session_id: String,
    source_workspace: String,
    source_remote_connection_id: Option<String>,
    source_remote_ssh_host: Option<String>,
    coordinator: Arc<ConversationCoordinator>,
    scheduler: Arc<DialogScheduler>,
    runtime: bitfun_agent_runtime::sdk::AgentRuntime,
}

/// Result of one create+send (or send-to-existing) dispatch.
struct DispatchOutcome {
    target_session_id: String,
    target_agent_type: String,
    created_session_id: Option<String>,
    workspace_path: String,
    delivery: &'static str,
    result_text: String,
    /// External response of the ACP direct path; `None` for local dispatches.
    /// The ACP direct path now runs asynchronously, so this is always `None`
    /// for ACP targets (the response streams back through events and the
    /// follow-up reply instead).
    acp_response: Option<String>,
}

/// Bounded window for background ACP direct deliveries (seconds). The old
/// direct path passed `timeout_seconds: None` (unbounded), which could hold
/// the tool call open indefinitely; the async delivery runs in a background
/// task with this 30-minute window instead (external agent long tasks such as
/// review/repair need the wider bound, while it stays bounded to avoid hangs).
const ACP_DIRECT_TIMEOUT_SECONDS: u64 = 1800;

/// Resolve the configured ACP direct-delivery window
/// (`ai.thresholds.acp_timeout.direct_secs`), falling back to
/// `ACP_DIRECT_TIMEOUT_SECONDS = 1800` when unset or invalid.
async fn configured_acp_direct_timeout_secs() -> u64 {
    let Ok(config_service) = crate::service::config::get_global_config_service().await else {
        return ACP_DIRECT_TIMEOUT_SECONDS;
    };
    let Ok(thresholds) = config_service
        .get_config::<crate::service::config::types::AiThresholdsConfig>(Some("ai.thresholds"))
        .await
    else {
        return ACP_DIRECT_TIMEOUT_SECONDS;
    };
    let secs = thresholds.acp_timeout.direct_secs;
    if secs == 0 {
        return ACP_DIRECT_TIMEOUT_SECONDS;
    }
    secs
}

/// COORD-03 流会话注册表元数据键（权威源：interfaces/acp/src/client/
/// session_persistence.rs:11-16 —— AcpSessionPersistence 创建流会话记录时
/// 写入 provider/acpClientId 自定义元数据）。core 不依赖 ACP crate，以
/// 字面量消费同一持久化契约。
const ACP_FLOW_METADATA_PROVIDER_KEY: &str = "provider";
const ACP_FLOW_METADATA_PROVIDER_VALUE: &str = "acp";
const ACP_FLOW_METADATA_CLIENT_ID_KEY: &str = "acpClientId";

/// COORD-03 流会话注册表判定结果：会话 id 形状（`acp_<client>_<uuid>`）
/// 只作线索，注册表记录才是「是否为活跃外部 ACP 流会话」的权威事实。
#[derive(Debug, Clone, PartialEq, Eq)]
enum AcpFlowSessionRegistryStatus {
    /// 注册表记录在册且 provider=acp：活跃外部 ACP 流会话（附记录中的
    /// client id，与形状解析出的 client id 必须一致）。
    Active { client_id: String },
    /// 注册表有记录但不是 ACP 流会话（例如内部会话的 id 恰巧命中形状）。
    NotAcpFlow,
    /// 注册表中无记录：会话已被回收（delete_session_record）或从未创建。
    Missing,
}

/// One of the two ACP direct send shapes: a flow session
/// (`acp_<client>_<uuid>` addressed via `send_message`) or an internal
/// `acp__<client>` session addressed via `send_message_to_bitfun_session`.
enum AcpDirectSendOp {
    Flow(AcpClientMessageRequest),
    Bitfun(AcpClientBitfunMessageRequest),
}

/// Source-session facts captured for the follow-up reply of an ACP direct
/// delivery (AgentSessionReplyRoute semantics: the external response is
/// delivered back to the sender session as a follow-up).
#[derive(Debug, Clone)]
struct AcpDirectReplySource {
    source_session_id: String,
    source_workspace: String,
    source_remote_connection_id: Option<String>,
    source_remote_ssh_host: Option<String>,
}

impl Default for SessionMessageTool {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionMessageTool {
    pub fn new() -> Self {
        Self
    }

    fn validate_session_id(session_id: &str) -> Result<(), String> {
        bitfun_core_types::validate_session_id(session_id)
    }

    /// Group-chat correlation of the calling (member) session context
    /// (R-GC-36). The coordinator forwards the turn's `groupId` from
    /// user_message_metadata into tool custom_data ("groupId", camelCase
    /// matching the group_room metadata contract). A non-group caller carries
    /// no such key → `GroupChatForwardMetadata::default()` (None, no fallback).
    fn group_context_from_custom_data(
        custom_data: &std::collections::HashMap<String, Value>,
    ) -> GroupChatForwardMetadata {
        match custom_data.get("groupId") {
            Some(Value::String(group_id)) if !group_id.trim().is_empty() => {
                GroupChatForwardMetadata {
                    group_id: Some(group_id.clone()),
                    group_message_id: None,
                    group_author: None,
                }
            }
            _ => GroupChatForwardMetadata::default(),
        }
    }

    fn forwarded_user_input_metadata(
        context: &ToolUseContext,
        sender: &SenderIdentity,
        group: &GroupChatForwardMetadata,
    ) -> serde_json::Map<String, Value> {
        use bitfun_agent_runtime::user_questions::USER_INPUT_AVAILABLE_CONTEXT_KEY;

        let mut metadata = serde_json::Map::new();
        if let Some(value @ (Value::Bool(_) | Value::String(_))) =
            context.custom_data.get(USER_INPUT_AVAILABLE_CONTEXT_KEY)
        {
            let is_boolean_fact = matches!(value, Value::Bool(_))
                || matches!(value, Value::String(text) if matches!(text.as_str(), "true" | "false"));
            if is_boolean_fact {
                metadata.insert(USER_INPUT_AVAILABLE_CONTEXT_KEY.to_string(), value.clone());
            }
        }
        // Sender identity triple for UI badges on forwarded agent messages
        // (R-23): every field degrades gracefully when unknown, so the badge
        // renders with whatever is available and never blocks delivery.
        metadata.insert("senderSessionId".to_string(), json!(sender.session_id));
        if let Some(role) = &sender.role {
            metadata.insert("senderRole".to_string(), json!(role));
        }
        if let Some(depth) = sender.depth {
            metadata.insert("senderDepth".to_string(), json!(depth));
        }
        if let Some(name) = sender
            .name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            metadata.insert("senderName".to_string(), json!(name));
        }
        // Group chat reply correlation keys (R-GC-11, contract §1.4): only
        // written when present so non-group dispatch stays zero-pollution.
        if let Some(group_id) = &group.group_id {
            metadata.insert("groupId".to_string(), json!(group_id));
        }
        if let Some(group_message_id) = &group.group_message_id {
            metadata.insert("groupMessageId".to_string(), json!(group_message_id));
        }
        if let Some(group_author) = &group.group_author {
            metadata.insert("groupAuthor".to_string(), json!(group_author));
        }
        metadata
    }

    fn resolve_workspace(&self, workspace: &str, context: &ToolUseContext) -> BitFunResult<String> {
        let workspace = workspace.trim();
        if workspace.is_empty() {
            return Err(BitFunError::tool(
                "workspace is required and cannot be empty".to_string(),
            ));
        }

        if context.is_remote() {
            if !posix_style_path_is_absolute(workspace) {
                return Err(BitFunError::tool(
                    "workspace must be an absolute POSIX path on the remote host".to_string(),
                ));
            }
            return context.resolve_workspace_tool_path(workspace);
        }

        let path = Path::new(workspace);
        if !path.is_absolute() {
            return Err(BitFunError::tool(
                "workspace must be an absolute path".to_string(),
            ));
        }

        let resolved = normalize_path(workspace);
        let path = Path::new(&resolved);
        if !path.exists() {
            return Err(BitFunError::tool(format!(
                "Workspace does not exist: {}",
                resolved
            )));
        }
        if !path.is_dir() {
            return Err(BitFunError::tool(format!(
                "Workspace is not a directory: {}",
                resolved
            )));
        }
        Ok(resolved)
    }

    fn validate_workspace_shape(
        &self,
        workspace: &str,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let workspace = workspace.trim();
        if workspace.is_empty() {
            return ValidationResult {
                result: false,
                message: Some("workspace is required and cannot be empty".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        match context {
            Some(context) => {
                let ws_ok = if context.is_remote() {
                    posix_style_path_is_absolute(workspace)
                } else {
                    Path::new(workspace).is_absolute()
                };
                if !ws_ok {
                    return ValidationResult {
                        result: false,
                        message: Some("workspace must be an absolute path".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                }
            }
            None => {
                if !Path::new(workspace).is_absolute() && !posix_style_path_is_absolute(workspace) {
                    return ValidationResult {
                        result: false,
                        message: Some("workspace must be an absolute path".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                }
            }
        }

        ValidationResult::default()
    }

    fn sender_session_id<'a>(&self, context: &'a ToolUseContext) -> BitFunResult<&'a str> {
        context.session_id.as_deref().ok_or_else(|| {
            BitFunError::tool("SessionMessage requires a source session".to_string())
        })
    }

    fn sender_workspace(&self, context: &ToolUseContext) -> BitFunResult<String> {
        context
            .workspace_root()
            .map(|path| path.to_string_lossy().to_string())
            .ok_or_else(|| {
                BitFunError::tool("SessionMessage requires a source workspace".to_string())
            })
    }

    fn creator_session_marker(&self, context: &ToolUseContext) -> BitFunResult<String> {
        let creator_session_id = context.session_id.as_ref().ok_or_else(|| {
            BitFunError::tool("SessionMessage requires a source session".to_string())
        })?;
        Ok(format!("session-{}", creator_session_id))
    }

    fn workspace_target_from_context(
        &self,
        workspace_path: String,
        context: &ToolUseContext,
    ) -> SessionMessageWorkspaceTarget {
        let binding = context.workspace.as_ref();
        let inherits_current_target = binding.is_some_and(|binding| {
            normalize_path(&binding.root_path_string()) == normalize_path(&workspace_path)
        });
        let remote_connection_id =
            binding.and_then(|workspace| workspace.connection_id().map(ToOwned::to_owned));
        let remote_ssh_host = binding
            .filter(|workspace| workspace.is_remote())
            .map(|workspace| workspace.session_identity.hostname.clone())
            .filter(|value| !value.trim().is_empty());
        let project_workspace_path = if inherits_current_target {
            binding
                .map(|workspace| normalize_path(&workspace.project_root_path_string()))
                .unwrap_or_else(|| workspace_path.clone())
        } else {
            workspace_path.clone()
        };
        SessionMessageWorkspaceTarget {
            workspace_path,
            project_workspace_path,
            execution_target: binding
                .filter(|_| inherits_current_target)
                .and_then(|workspace| workspace.execution_target.clone()),
            workspace_id: binding
                .filter(|_| inherits_current_target)
                .and_then(|workspace| workspace.workspace_id.clone()),
            remote_connection_id,
            remote_ssh_host,
        }
    }

    fn workspace_target_from_binding(
        &self,
        binding: AgentSessionWorkspaceBinding,
    ) -> SessionMessageWorkspaceTarget {
        let project_workspace_path = binding
            .project_workspace_path
            .clone()
            .unwrap_or_else(|| binding.workspace_path.clone());
        SessionMessageWorkspaceTarget {
            workspace_path: binding.workspace_path,
            project_workspace_path,
            execution_target: binding.execution_target,
            workspace_id: binding.workspace_id,
            remote_connection_id: binding.remote_connection_id,
            remote_ssh_host: binding.remote_ssh_host,
        }
    }

    fn same_workspace_identity(
        left: &SessionMessageWorkspaceTarget,
        right: &SessionMessageWorkspaceTarget,
    ) -> bool {
        left.workspace_path == right.workspace_path
            && left.remote_connection_id == right.remote_connection_id
            && left.remote_ssh_host == right.remote_ssh_host
    }

    fn target_agent_type_from_resolution(agent_type: Option<String>) -> Option<String> {
        agent_type.filter(|value| !value.trim().is_empty())
    }

    fn target_agent_type_from_sessions(
        sessions: &[AgentSessionSummary],
        target_session_id: &str,
    ) -> Option<String> {
        sessions
            .iter()
            .find(|session| {
                session.session_id == target_session_id && !session.agent_type.trim().is_empty()
            })
            .map(|session| session.agent_type.clone())
    }

    /// Best-effort identity of the sending session: session-tree depth (R-19),
    /// and display name (session name, else agent type). Every field degrades
    /// gracefully when unknown, so a forwarding send never fails because
    /// identity data is missing.
    #[allow(clippy::too_many_arguments)]
    async fn resolve_sender_identity(
        &self,
        runtime: &bitfun_agent_runtime::sdk::AgentRuntime,
        context: &ToolUseContext,
        source_session_id: &str,
        source_workspace: &str,
        source_remote_connection_id: Option<&str>,
        source_remote_ssh_host: Option<&str>,
        coordinator: &ConversationCoordinator,
    ) -> SenderIdentity {
        let role = None;
        let depth = coordinator.session_tree().get_depth(source_session_id);
        let session_name = runtime
            .list_sessions(AgentSessionListRequest {
                workspace_path: source_workspace.to_string(),
                remote_connection_id: source_remote_connection_id.map(ToOwned::to_owned),
                remote_ssh_host: source_remote_ssh_host.map(ToOwned::to_owned),
                include_hidden: false,
            })
            .await
            .ok()
            .and_then(|sessions| {
                sessions
                    .into_iter()
                    .find(|summary| summary.session_id == source_session_id)
                    .map(|summary| summary.session_name)
            })
            .filter(|name| !name.trim().is_empty());
        let name = session_name.or_else(|| {
            context
                .agent_type
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(ToOwned::to_owned)
        });
        SenderIdentity {
            session_id: source_session_id.to_string(),
            role,
            depth,
            name,
        }
    }

    fn format_forwarded_message(
        &self,
        message: &str,
        sender: &SenderIdentity,
    ) -> (String, Vec<AgentDialogPrependedReminder>) {
        let mut lines = vec![
            format!(
                "This request was sent by {} (session {}), not the human user. Do not use interactive tools for this request. In particular, do not call AskUserQuestion.",
                sender.display_label(),
                sender.session_id
            ),
            format!("From session: {}", sender.session_id),
            format!("From role: {}", sender.role.as_deref().unwrap_or("Agent")),
        ];
        if let Some(depth) = sender.depth {
            lines.push(format!("From depth: {depth}"));
        }
        if let Some(name) = sender
            .name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            lines.push(format!("From agent: {name}"));
        }
        (
            message.to_string(),
            vec![AgentDialogPrependedReminder {
                kind: "session_message_request".to_string(),
                text: lines.join("\n"),
            }],
        )
    }
}

/// Identity of the session that sent a forwarded message.
#[derive(Debug, Clone, PartialEq)]
struct SenderIdentity {
    /// Session id of the sender; always present.
    session_id: String,
    /// RBAC role display label (e.g. "Commander"), when registered.
    role: Option<String>,
    /// Session-tree depth (0 means the root level L0), when known.
    depth: Option<u32>,
    /// Session name, or the agent type fallback, when available.
    name: Option<String>,
}

/// Optional group-chat reply correlation keys forwarded with a dispatched turn
/// (R-GC-11, contract §1.4: groupId / groupMessageId / groupAuthor).
///
/// All fields are optional: a non-group dispatch carries the default empty
/// metadata and never writes the keys (zero pollution).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct GroupChatForwardMetadata {
    /// The group chat room id the message belongs to.
    pub group_id: Option<String>,
    /// The group chat message id being replied to.
    pub group_message_id: Option<String>,
    /// Sender identifier: `__master__` or a member session id.
    pub group_author: Option<String>,
}

impl SenderIdentity {
    /// "[Commander L0]" when role and depth are known; "[Commander]" with role
    /// only; "[Agent]" when no role is registered. Depth is omitted when unknown.
    fn role_label(&self) -> String {
        let role = self.role.as_deref().unwrap_or("Agent");
        match self.depth {
            Some(depth) => format!("[{role} L{depth}]"),
            None => format!("[{role}]"),
        }
    }

    /// "[Commander L0] Name (session abc)" or "[Agent] (session abc)" when the
    /// display name is unavailable.
    fn display_label(&self) -> String {
        let mut label = self.role_label();
        if let Some(name) = self
            .name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            label.push(' ');
            label.push_str(name);
        }
        label
    }
}

/// Lightweight UUID shape check (8-4-4-4-12, 36 chars) for the trailing
/// segment of an ACP flow session id (`acp_<client_id>_<uuid>`). Single
/// authoritative implementation lives in `bitfun_runtime_ports` (d3-P2-2) so
/// core, desktop and Task layers share the same判定. Kept only for the
/// local regression test; production code calls the port directly.
#[cfg(test)]
fn looks_like_uuid(segment: &str) -> bool {
    bitfun_runtime_ports::looks_like_uuid(segment)
}

use bitfun_runtime_ports::AgentType;

#[derive(Debug, Clone, Deserialize)]
struct SessionMessageInput {
    workspace: Option<String>,
    session_id: Option<String>,
    session_name: Option<String>,
    /// Top-level message for single-target dispatch. Mutually exclusive with
    /// `batch`: when batch is present this field must be omitted or empty.
    #[serde(default)]
    message: Option<String>,
    agent_type: Option<AgentType>,
    /// When true, deliver as an urgent mid-turn correction: if the target session
    /// is currently processing, the message is injected into its running turn via
    /// the UserSteering channel instead of starting a new turn. Falls back to
    /// normal delivery when the target session is not processing.
    #[serde(default)]
    urgent: bool,
    /// Optional plan-todo binding: when creating a new session, the dispatched
    /// turn carries planFile/todoId in the forwarded metadata so the scheduler
    /// auto-marks the plan todo (in_progress at turn start, completed when the
    /// turn finishes with a Completed outcome). Only allowed when session_id is
    /// omitted; both fields must be provided together.
    #[serde(default)]
    plan_file: Option<String>,
    #[serde(default)]
    todo_id: Option<String>,
    /// Optional worktree options for create: when present (and session_id is
    /// omitted), a managed worktree is created together with the session via
    /// WorktreeService and the session is bound to it. `None` keeps the legacy
    /// behavior (session runs in the project checkout). Rejected for remote
    /// workspaces and for session_id-based sends.
    #[serde(default)]
    worktree: Option<WorktreeSessionOptions>,
    /// Batch dispatch: perform multiple create+send (or send-to-existing)
    /// operations in a single tool call. All items are validated up front (the
    /// whole batch is rejected when any item is structurally invalid), then each
    /// item executes sequentially and independently: a failed item never rolls
    /// back already-succeeded items and never stops later items. The top-level
    /// session fields (session_id/session_name/agent_type/urgent/plan_file/
    /// todo_id) must stay empty when batch is used; the top-level workspace is
    /// shared by every item that creates a new session.
    #[serde(default)]
    batch: Option<Vec<BatchItem>>,
}

/// One create+send (or send-to-existing-session) operation inside a batch
/// dispatch. Fields mirror the top-level SessionMessageInput semantics, except
/// that the workspace is shared from the top level.
#[derive(Debug, Clone, Deserialize)]
struct BatchItem {
    /// Optional target session ID. Omit it to create a new session (requires
    /// session_name and agent_type; the top-level workspace is used).
    session_id: Option<String>,
    /// Display name for a new session. Required when session_id is omitted.
    session_name: Option<String>,
    /// Message to send to the target session.
    message: String,
    /// Agent type for a new session. Required when session_id is omitted.
    agent_type: Option<AgentType>,
    /// Per-item urgent delivery flag (same semantics as the top-level flag).
    #[serde(default)]
    urgent: bool,
    /// Per-item plan-todo binding (only when session_id is omitted, and
    /// requires todo_id).
    #[serde(default)]
    plan_file: Option<String>,
    /// Per-item todo id within plan_file (only when session_id is omitted, and
    /// requires plan_file).
    #[serde(default)]
    todo_id: Option<String>,
    /// Per-item worktree options for a new session (only when session_id is
    /// omitted; rejected for remote workspaces). Same semantics as the
    /// top-level worktree field.
    #[serde(default)]
    worktree: Option<WorktreeSessionOptions>,
}

/// Delivery decision for an urgent message against a target session.
#[derive(Debug, Clone, PartialEq)]
enum UrgentDelivery {
    /// Target session is processing a turn; steer into the running turn.
    Steer { turn_id: String },
    /// Target session is idle (or the turn ended); use normal submission.
    NormalSubmit,
}

fn resolve_urgent_delivery(processing_turn_id: Option<String>) -> UrgentDelivery {
    match processing_turn_id {
        Some(turn_id) => UrgentDelivery::Steer { turn_id },
        None => UrgentDelivery::NormalSubmit,
    }
}

/// Dual-channel redundancy decision for urgent messages:
/// only attempt the steering channel when the message is urgent AND the target
/// session already exists (a brand-new session has no running turn to steer
/// into) AND the dispatch does not carry a plan-todo binding (the steering
/// channel carries no binding metadata, so a bound message falls back to the
/// normal submission channel that preserves the binding and the reply route —
/// COORD-01). Every other case uses the normal submission channel. When
/// steering is attempted but rejected, the caller falls back to the normal
/// channel, so one of the two channels always delivers the message.
fn should_attempt_steering(
    urgent: bool,
    created_session_id: Option<&str>,
    has_plan_todo_binding: bool,
) -> bool {
    urgent && created_session_id.is_none() && !has_plan_todo_binding
}

#[async_trait]
impl Tool for SessionMessageTool {
    fn name(&self) -> &str {
        "SessionMessage"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Asynchronously send a message to another agent session. When the target session finishes, its result is automatically sent back to you as a follow-up message.

Usage:
- Create a new session and send: omit "session_id", and provide "workspace", "session_name", "agent_type", and "message".
- Reusing an existing session: provide "session_id" and "message". You may omit "workspace"; the tool will resolve it from the target session when possible.
- Urgent correction: set "urgent" to true to inject the message into the target session's running turn instead of waiting for a new turn. Requires "session_id".

Use SessionControl (list) to discover existing sessions before sending messages.
Use SessionHistory to export a transcript of any session.
Use Task to spawn subagent sessions that can receive messages.

Allowed agent types when creating a session are dynamically resolved from the available agent registry (common values include "agentic", "Plan", "Cowork", "DeepResearch", and any custom/external subagent types).
- "agentic": Coding-focused agent for implementation, debugging, and code changes.
- "Plan": Planning agent for clarifying requirements and producing an implementation plan before coding.
- "Cowork": Collaborative agent for office-style work such as research, documentation, presentations, etc.
- "DeepResearch": Research agent for systematic investigation and evidence-driven reports.
"#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Send a message to another agent session and receive the result asynchronously.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "workspace": {
                    "type": "string",
                    "description": "Required absolute target workspace path when creating a new session. Optional when session_id is provided."
                },
                "session_id": {
                    "type": "string",
                    "description": "Optional target session ID. Omit it to create a new session and send the message there."
                },
                "session_name": {
                    "type": "string",
                    "description": "Required when session_id is omitted. Display name for the new session."
                },
                "message": {
                    "type": "string",
                    "description": "Message to send to the target session."
                },
                "agent_type": {
                    "type": "string",
                    "description": "Required when session_id is omitted. Valid values are dynamically resolved from the available agent registry."
                },
                "urgent": {
                    "type": "boolean",
                    "description": "When true, deliver as an urgent mid-turn correction: if the target session is processing, inject into its running turn via the UserSteering channel; otherwise fall back to normal delivery. Requires session_id."
                },
                "plan_file": {
                    "type": "string",
                    "description": "Optional plan-todo binding for a created session (only when session_id is omitted, and requires todo_id): the plan file name or absolute path whose todo is auto-marked in_progress when the dispatched turn starts and completed when it finishes with a Completed outcome."
                },
                "todo_id": {
                    "type": "string",
                    "description": "Optional todo id within plan_file for a created session (only when session_id is omitted, and requires plan_file)."
                },
                "worktree": {
                    "type": "object",
                    "description": "Optional worktree options for a created session (only when session_id is omitted; not supported for remote workspaces): creates a managed Git worktree together with the session and binds the session to it. Shape: {baseRef?, copyLocalChanges?}.",
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
                },
                "batch": {
                    "type": "array",
                    "description": "Batch dispatch: perform multiple create+send (or send-to-existing) operations in one tool call. Mutually exclusive with the top-level message and session fields; the top-level workspace is shared by items that create a session. All items validate up front; each item then runs independently (a failed item never rolls back succeeded ones). Item shape: {session_id?, session_name?, message, agent_type?, plan_file?, todo_id?, urgent?, worktree?}.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "session_id": {
                                "type": "string",
                                "description": "Optional target session ID. Omit it to create a new session."
                            },
                            "session_name": {
                                "type": "string",
                                "description": "Required when session_id is omitted. Display name for the new session."
                            },
                            "message": {
                                "type": "string",
                                "description": "Message to send to the target session."
                            },
                            "agent_type": {
                                "type": "string",
                                "description": "Required when session_id is omitted. Agent type for the new session."
                            },
                            "urgent": {
                                "type": "boolean",
                                "description": "Per-item urgent delivery flag (same semantics as the top-level flag). Requires session_id."
                            },
                            "plan_file": {
                                "type": "string",
                                "description": "Per-item plan-todo binding (only when session_id is omitted, and requires todo_id)."
                            },
                            "todo_id": {
                                "type": "string",
                                "description": "Per-item todo id within plan_file (only when session_id is omitted, and requires plan_file)."
                            },
                            "worktree": {
                                "type": "object",
                                "description": "Per-item worktree options for a new session (only when session_id is omitted; not supported for remote workspaces). Shape: {baseRef?, copyLocalChanges?}.",
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
                        "required": ["message"],
                        "additionalProperties": false
                    }
                }
            },
            "required": [],
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
                "workspace": {
                    "type": "string",
                    "description": "Required absolute target workspace path when creating a new session. Optional when session_id is provided."
                },
                "session_id": {
                    "type": "string",
                    "description": "Optional target session ID. Omit it to create a new session and send the message there."
                },
                "session_name": {
                    "type": "string",
                    "description": "Required when session_id is omitted. Display name for the new session."
                },
                "message": {
                    "type": "string",
                    "description": "Message to send to the target session."
                },
                "agent_type": {
                    "type": "string",
                    "enum": agent_type_enum,
                    "description": "Required when session_id is omitted. Not allowed when sending to an existing session."
                },
                "urgent": {
                    "type": "boolean",
                    "description": "When true, deliver as an urgent mid-turn correction: if the target session is processing, inject into its running turn via the UserSteering channel; otherwise fall back to normal delivery. Requires session_id."
                },
                "plan_file": {
                    "type": "string",
                    "description": "Optional plan-todo binding for a created session (only when session_id is omitted, and requires todo_id): the plan file name or absolute path whose todo is auto-marked in_progress when the dispatched turn starts and completed when it finishes with a Completed outcome."
                },
                "todo_id": {
                    "type": "string",
                    "description": "Optional todo id within plan_file for a created session (only when session_id is omitted, and requires plan_file)."
                },
                "worktree": {
                    "type": "object",
                    "description": "Optional worktree options for a created session (only when session_id is omitted; not supported for remote workspaces): creates a managed Git worktree together with the session and binds the session to it. Shape: {baseRef?, copyLocalChanges?}.",
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
                },
                "batch": {
                    "type": "array",
                    "description": "Batch dispatch: perform multiple create+send (or send-to-existing) operations in one tool call. Mutually exclusive with the top-level message and session fields; the top-level workspace is shared by items that create a session. All items validate up front; each item then runs independently (a failed item never rolls back succeeded ones). Item shape: {session_id?, session_name?, message, agent_type?, plan_file?, todo_id?, urgent?, worktree?}.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "session_id": {
                                "type": "string",
                                "description": "Optional target session ID. Omit it to create a new session."
                            },
                            "session_name": {
                                "type": "string",
                                "description": "Required when session_id is omitted. Display name for the new session."
                            },
                            "message": {
                                "type": "string",
                                "description": "Message to send to the target session."
                            },
                            "agent_type": {
                                "type": "string",
                                "description": "Required when session_id is omitted. Agent type for the new session."
                            },
                            "urgent": {
                                "type": "boolean",
                                "description": "Per-item urgent delivery flag (same semantics as the top-level flag). Requires session_id."
                            },
                            "plan_file": {
                                "type": "string",
                                "description": "Per-item plan-todo binding (only when session_id is omitted, and requires todo_id)."
                            },
                            "todo_id": {
                                "type": "string",
                                "description": "Per-item todo id within plan_file (only when session_id is omitted, and requires plan_file)."
                            },
                            "worktree": {
                                "type": "object",
                                "description": "Per-item worktree options for a new session (only when session_id is omitted; not supported for remote workspaces). Shape: {baseRef?, copyLocalChanges?}.",
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
                        "required": ["message"],
                        "additionalProperties": false
                    }
                }
            },
            "required": [],
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
        let parsed: SessionMessageInput = match serde_json::from_value(input.clone()) {
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

        // Batch mode: the whole batch is validated up front — any structurally
        // invalid item rejects the entire batch before anything executes.
        if let Some(batch) = parsed.batch.as_ref() {
            return self.validate_batch(&parsed, batch, context).await;
        }

        let message = parsed.message.as_deref().unwrap_or_default();
        if message.trim().is_empty() {
            return ValidationResult {
                result: false,
                message: Some("message cannot be empty".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        match parsed.session_id.as_deref() {
            Some(session_id) => {
                if let Err(message) = Self::validate_session_id(session_id) {
                    return ValidationResult {
                        result: false,
                        message: Some(message),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                if parsed.session_name.is_some() {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "session_name is only allowed when session_id is omitted".to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                if parsed.agent_type.is_some() {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "agent_type override is not allowed when session_id is provided"
                                .to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                if parsed.plan_file.is_some() || parsed.todo_id.is_some() {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "plan_file/todo_id binding is only allowed when session_id is omitted"
                                .to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                if parsed.worktree.is_some() {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "worktree is only allowed when session_id is omitted".to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                if let Some(workspace) = parsed.workspace.as_deref() {
                    let workspace_validation = self.validate_workspace_shape(workspace, context);
                    if !workspace_validation.result {
                        return workspace_validation;
                    }
                }
            }
            None => {
                if parsed.plan_file.is_some() != parsed.todo_id.is_some() {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "plan_file and todo_id must be provided together".to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                if parsed
                    .session_name
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
                {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "session_name is required when session_id is omitted".to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                if parsed.agent_type.is_none() {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "agent_type is required when session_id is omitted".to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                if let Some(worktree) = parsed.worktree.as_ref() {
                    if worktree
                        .base_ref
                        .as_deref()
                        .is_some_and(|base_ref| base_ref.trim().is_empty())
                    {
                        return ValidationResult {
                            result: false,
                            message: Some(
                                "worktree.base_ref must not be empty when provided".to_string(),
                            ),
                            error_code: Some(400),
                            meta: None,
                        };
                    }
                    if context.is_some_and(|context| context.is_remote()) {
                        return ValidationResult {
                            result: false,
                            message: Some(
                                "worktree is not supported for remote workspaces".to_string(),
                            ),
                            error_code: Some(400),
                            meta: None,
                        };
                    }
                    // worktree 与 ACP 真会话（agent_type `acp__<client>`）互斥：
                    // ACP 会话是外部进程记录，不承载本地 worktree
                    // execution_target，同时携带会导致 worktree 成为孤儿。
                    if parsed
                        .agent_type
                        .as_ref()
                        .is_some_and(|agent_type| agent_type.as_str().starts_with("acp__"))
                    {
                        return ValidationResult {
                            result: false,
                            message: Some(
                                "worktree is not supported with acp__ agent types".to_string(),
                            ),
                            error_code: Some(400),
                            meta: None,
                        };
                    }
                }

                let Some(workspace) = parsed.workspace.as_deref() else {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "workspace is required when session_id is omitted".to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                };
                let workspace_validation = self.validate_workspace_shape(workspace, context);
                if !workspace_validation.result {
                    return workspace_validation;
                }
            }
        }

        let Some(context) = context else {
            return ValidationResult::default();
        };

        let Some(source_session_id) = context.session_id.as_deref() else {
            return ValidationResult {
                result: false,
                message: Some(
                    "SessionMessage requires a source session in tool context".to_string(),
                ),
                error_code: Some(400),
                meta: None,
            };
        };

        if let Some(target_session_id) = parsed.session_id.as_deref() {
            if source_session_id == target_session_id {
                return ValidationResult {
                    result: false,
                    message: Some(
                        "SessionMessage cannot send a message to the same session".to_string(),
                    ),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        ValidationResult::default()
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let workspace = input
            .get("workspace")
            .and_then(|value| value.as_str())
            .unwrap_or("resolved workspace");
        if let Some(batch) = input.get("batch").and_then(|value| value.as_array()) {
            return format!("Batch dispatch {} message(s) in {}", batch.len(), workspace);
        }
        if let Some(session_id) = input.get("session_id").and_then(|value| value.as_str()) {
            format!("Send message to session {} in {}", session_id, workspace)
        } else {
            let session_name = input
                .get("session_name")
                .and_then(|value| value.as_str())
                .unwrap_or("new session");
            format!(
                "Create session {} in {} and send message",
                session_name, workspace
            )
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: SessionMessageInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::tool(format!("Invalid input: {}", e)))?;
        let shared = self.build_dispatch_shared(context).await?;

        if let Some(batch) = params.batch.as_ref() {
            return self.call_batch(&params, batch, &shared, context).await;
        }

        let outcome = self.dispatch_single(params, &shared, context).await?;
        let mut data = json!({
            "success": true,
            "target_workspace": outcome.workspace_path,
            "target_session_id": outcome.target_session_id,
            "target_agent_type": outcome.target_agent_type,
            "created_session_id": outcome.created_session_id,
            "delivery": outcome.delivery,
        });
        // ACP direct path: the external response is exposed verbatim on the
        // result payload so programmatic callers can consume it.
        if let Some(response) = outcome.acp_response.as_ref() {
            data["response"] = json!(response);
        }
        Ok(vec![ToolResult::Result {
            data,
            result_for_assistant: Some(outcome.result_text),
            image_attachments: None,
        }])
    }
}

/// Build the follow-up message injected into the sender session when an ACP
/// direct delivery succeeds (COORD-15). The full external reply stays in the
/// target ACP stream session history (retrievable via SessionHistory); only
/// the notice is injected so the sender context is not inflated with the
/// full reply text.
fn acp_direct_response_notice(_full_response: &str, session_id: &str) -> String {
    format!(
        "External ACP session '{}' responded; use SessionHistory to view the full reply.",
        session_id
    )
}

/// Current unix time in milliseconds (fallback 0 on clock failure; never
/// panics).
fn acp_direct_delivery_now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// The target workspace of an ACP direct delivery, used to resolve the
/// session storage directory for backend persistence.
fn acp_direct_delivery_workspace_path(op: &AcpDirectSendOp) -> Option<&str> {
    match op {
        AcpDirectSendOp::Flow(request) => request.workspace_path.as_deref(),
        AcpDirectSendOp::Bitfun(request) => request.workspace_path.as_deref(),
    }
}

/// Build the persisted `DialogTurnData` for one ACP direct delivery
/// (a19 后端同构落盘；镜像前端 convertDialogTurnToBackendFormat 的
/// user_message + 单 model_round text_items 结构)。
#[allow(clippy::too_many_arguments)]
fn build_acp_direct_delivery_turn(
    turn_id: &str,
    turn_index: usize,
    session_id: &str,
    user_input: &str,
    round_id: &str,
    round_started_at_ms: u64,
    response: &str,
    status: crate::service::session::TurnStatus,
    error: Option<String>,
) -> crate::service::session::DialogTurnData {
    use crate::service::session::{
        DialogTurnData, ModelRoundData, TextItemData, TurnStatus, UserMessageData,
    };
    let mut turn = DialogTurnData::new(
        turn_id.to_string(),
        turn_index,
        session_id.to_string(),
        UserMessageData {
            id: Uuid::new_v4().to_string(),
            content: user_input.to_string(),
            timestamp: round_started_at_ms,
            metadata: None,
        },
    );
    turn.start_time = round_started_at_ms;
    let mut round = ModelRoundData {
        id: round_id.to_string(),
        turn_id: turn_id.to_string(),
        round_index: 0,
        round_group_id: None,
        timestamp: round_started_at_ms,
        text_items: Vec::new(),
        tool_items: Vec::new(),
        thinking_items: Vec::new(),
        start_time: round_started_at_ms,
        end_time: None,
        duration_ms: None,
        provider_id: None,
        model_config_id: None,
        effective_model_name: None,
        first_chunk_ms: None,
        first_visible_output_ms: None,
        stream_duration_ms: None,
        attempt_count: None,
        attempt_diagnostics: Vec::new(),
        failure_category: None,
        token_details: None,
        status: "completed".to_string(),
    };
    if !response.trim().is_empty() {
        round.text_items.push(TextItemData {
            id: Uuid::new_v4().to_string(),
            content: response.to_string(),
            is_streaming: false,
            timestamp: round_started_at_ms,
            is_markdown: true,
            order_index: Some(0),
            is_subagent_item: None,
            parent_task_tool_id: None,
            subagent_session_id: None,
            status: Some("completed".to_string()),
            attempt_id: None,
            attempt_index: None,
        });
    }
    turn.model_rounds.push(round);
    turn.error = error;
    match status {
        TurnStatus::Completed => turn.mark_completed(),
        TurnStatus::Cancelled | TurnStatus::Error => {
            turn.status = status;
            turn.end_time = Some(acp_direct_delivery_now_unix_ms());
        }
        TurnStatus::InProgress => {}
    }
    turn
}

/// Persist one ACP direct delivery turn through the injected persistence
/// manager. Backend persistence is independent of the frontend event stream;
/// the turn index derives from the session metadata `turn_count` (matching
/// the frontend `indexOf` semantics for a contiguous history). A turn already
/// saved by the frontend at that index is a no-op; an index collision with a
/// different turn id is skipped with a warning. Failures are logged, never
/// propagated, so persistence can never break the notification path.
#[allow(clippy::too_many_arguments)]
async fn persist_acp_direct_delivery_turn(
    persistence: &crate::agentic::persistence::PersistenceManager,
    storage_path: &Path,
    session_id: &str,
    turn_id: &str,
    user_input: &str,
    round_id: &str,
    round_started_at_ms: u64,
    response: &str,
    status: crate::service::session::TurnStatus,
    error: Option<String>,
) {
    let Ok(Some(metadata)) = persistence
        .load_session_metadata(storage_path, session_id)
        .await
    else {
        warn!(
            "ACP direct delivery persistence skipped: session metadata not found: session_id={}",
            session_id
        );
        return;
    };
    // 幂等：同 turn_id 已在会话任意索引落盘 → no-op（不重复追加）。
    let known_turn_count = metadata.turn_count;
    for index in 0..known_turn_count {
        if let Ok(Some(existing)) = persistence
            .load_dialog_turn(storage_path, session_id, index)
            .await
        {
            if existing.turn_id == turn_id {
                return;
            }
        }
    }
    // P-19 全文落盘原则：计算索引（metadata.turn_count）可能被前端/并发写者
    // 已落盘的既有 turn 占用而元数据未同步（实证「SessionHistory 导出仍只有
    // turn 0」）。此时不得静默丢弃投递 turn——从 turn_count 起向后扫描第一个
    // 空闲索引追加，保证 reply 全文始终可经 SessionHistory 检索。
    let mut turn_index = known_turn_count;
    loop {
        match persistence
            .load_dialog_turn(storage_path, session_id, turn_index)
            .await
        {
            Ok(Some(existing)) if existing.turn_id == turn_id => {
                return;
            }
            Ok(Some(_)) => {
                turn_index += 1;
            }
            _ => break,
        }
    }
    let turn = build_acp_direct_delivery_turn(
        turn_id,
        turn_index,
        session_id,
        user_input,
        round_id,
        round_started_at_ms,
        response,
        status,
        error,
    );
    if let Err(save_error) = persistence.save_dialog_turn(storage_path, &turn).await {
        warn!(
            "Failed to persist ACP direct delivery turn: session_id={} turn_id={} error={}",
            session_id, turn_id, save_error
        );
    }
}

/// Production wrapper for ACP direct delivery persistence: resolve the
/// workspace session storage path and build the global persistence manager,
/// then persist the turn.
#[allow(clippy::too_many_arguments)]
async fn persist_acp_direct_delivery_to_workspace(
    workspace_path: &str,
    session_id: &str,
    turn_id: &str,
    user_input: &str,
    round_id: &str,
    round_started_at_ms: u64,
    response: &str,
    status: crate::service::session::TurnStatus,
    error: Option<String>,
) {
    use crate::agentic::persistence::PersistenceManager;
    use crate::infrastructure::get_path_manager_arc;
    use crate::service::remote_ssh::workspace_state::get_effective_session_path;

    let storage_path = get_effective_session_path(workspace_path, None, None).await;
    let persistence = match PersistenceManager::new(get_path_manager_arc()) {
        Ok(persistence) => persistence,
        Err(init_error) => {
            warn!(
                "ACP direct delivery persistence skipped: failed to initialize PersistenceManager: {}",
                init_error
            );
            return;
        }
    };
    persist_acp_direct_delivery_turn(
        &persistence,
        &storage_path,
        session_id,
        turn_id,
        user_input,
        round_id,
        round_started_at_ms,
        response,
        status,
        error,
    )
    .await;
}

impl SessionMessageTool {
    /// Validates a batch payload up front. Structural rules mirror the
    /// single-target shape, applied per item with `batch[N]` prefixes; any
    /// invalid item rejects the whole batch before anything executes.
    async fn validate_batch(
        &self,
        parsed: &SessionMessageInput,
        batch: &[BatchItem],
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        if batch.is_empty() {
            return Self::invalid("batch cannot be empty");
        }
        if parsed
            .message
            .as_deref()
            .is_some_and(|message| !message.trim().is_empty())
        {
            return Self::invalid("message cannot be combined with batch");
        }
        if parsed.session_id.is_some()
            || parsed.session_name.is_some()
            || parsed.agent_type.is_some()
            || parsed.plan_file.is_some()
            || parsed.todo_id.is_some()
            || parsed.urgent
        {
            return Self::invalid(
                "session fields must be provided per batch item when batch is used",
            );
        }

        // The shared workspace must be present (and well-formed) when any item
        // creates a new session; when present it is always shape-checked.
        if let Some(workspace) = parsed.workspace.as_deref() {
            let workspace_validation = self.validate_workspace_shape(workspace, context);
            if !workspace_validation.result {
                return workspace_validation;
            }
        } else if batch.iter().any(|item| item.session_id.is_none()) {
            return Self::invalid("workspace is required when a batch item omits session_id");
        }

        let source_session_id = context.and_then(|context| context.session_id.as_deref());
        for (index, item) in batch.iter().enumerate() {
            let field = |name: &str| format!("batch[{index}].{name}");
            if item.message.trim().is_empty() {
                return Self::invalid(format!("{} cannot be empty", field("message")));
            }
            match item.session_id.as_deref() {
                Some(session_id) => {
                    if let Err(message) = Self::validate_session_id(session_id) {
                        return Self::invalid(format!("{}: {message}", field("session_id")));
                    }
                    if item.session_name.is_some() {
                        return Self::invalid(format!(
                            "{} is only allowed when session_id is omitted",
                            field("session_name")
                        ));
                    }
                    if item.agent_type.is_some() {
                        return Self::invalid(format!(
                            "{} override is not allowed when session_id is provided",
                            field("agent_type")
                        ));
                    }
                    if item.plan_file.is_some() || item.todo_id.is_some() {
                        return Self::invalid(format!(
                            "{} binding is only allowed when session_id is omitted",
                            field("plan_file/todo_id")
                        ));
                    }
                    if item.worktree.is_some() {
                        return Self::invalid(format!(
                            "{} is only allowed when session_id is omitted",
                            field("worktree")
                        ));
                    }
                    if let Some(source_session_id) = source_session_id {
                        if source_session_id == session_id {
                            return Self::invalid(format!(
                                "{} cannot send a message to the same session",
                                field("session_id")
                            ));
                        }
                    }
                }
                None => {
                    if item.plan_file.is_some() != item.todo_id.is_some() {
                        return Self::invalid(format!(
                            "{} and {} must be provided together",
                            field("plan_file"),
                            field("todo_id")
                        ));
                    }
                    if item
                        .session_name
                        .as_deref()
                        .is_none_or(|value| value.trim().is_empty())
                    {
                        return Self::invalid(format!(
                            "{} is required when session_id is omitted",
                            field("session_name")
                        ));
                    }
                    if item.agent_type.is_none() {
                        return Self::invalid(format!(
                            "{} is required when session_id is omitted",
                            field("agent_type")
                        ));
                    }
                    if let Some(worktree) = item.worktree.as_ref() {
                        if worktree
                            .base_ref
                            .as_deref()
                            .is_some_and(|base_ref| base_ref.trim().is_empty())
                        {
                            return Self::invalid(format!(
                                "{} must not be empty when provided",
                                field("worktree.base_ref")
                            ));
                        }
                        if context.is_some_and(|context| context.is_remote()) {
                            return Self::invalid(format!(
                                "{} is not supported for remote workspaces",
                                field("worktree")
                            ));
                        }
                        if item
                            .agent_type
                            .as_ref()
                            .is_some_and(|agent_type| agent_type.as_str().starts_with("acp__"))
                        {
                            return Self::invalid(format!(
                                "{} is not supported with acp__ agent types",
                                field("worktree")
                            ));
                        }
                    }
                }
            }
        }

        let Some(context) = context else {
            return ValidationResult::default();
        };
        let Some(_source_session_id) = context.session_id.as_deref() else {
            return Self::invalid("SessionMessage requires a source session in tool context");
        };
        ValidationResult::default()
    }

    fn invalid(message: impl Into<String>) -> ValidationResult {
        ValidationResult {
            result: false,
            message: Some(message.into()),
            error_code: Some(400),
            meta: None,
        }
    }

    /// Resolves the source-session facts and the global coordinator, scheduler
    /// and runtime once per tool call, so a batch dispatch shares one resource
    /// setup instead of re-resolving globals for every item.
    async fn build_dispatch_shared(
        &self,
        context: &ToolUseContext,
    ) -> BitFunResult<DispatchShared> {
        let source_session_id = self.sender_session_id(context)?.to_string();
        let source_workspace = self.sender_workspace(context)?;
        let source_remote_connection_id = context
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.connection_id().map(ToOwned::to_owned));
        let source_remote_ssh_host = context
            .workspace
            .as_ref()
            .filter(|workspace| workspace.is_remote())
            .map(|workspace| workspace.session_identity.hostname.clone())
            .filter(|value| !value.trim().is_empty());
        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;
        let scheduler = get_global_scheduler()
            .ok_or_else(|| BitFunError::tool("scheduler not initialized".to_string()))?;
        let runtime = CoreServiceAgentRuntime::agent_runtime_with_dialog_turns(
            coordinator.clone(),
            scheduler.clone(),
        )
        .map_err(BitFunError::tool)?;
        Ok(DispatchShared {
            source_session_id,
            source_workspace,
            source_remote_connection_id,
            source_remote_ssh_host,
            coordinator,
            scheduler,
            runtime,
        })
    }

    /// The ACP client id when the target agent type is an ACP bridge agent
    /// (`acp__<client_id>`; see AcpAgent::agent_id_for), otherwise `None`.
    /// ACP targets bypass the local model entirely: SessionMessage forwards
    /// the message through the ACP client port instead of submitting a local
    /// dialog turn, so no bridge re-translation (and no double billing) can
    /// happen.
    fn acp_client_id_from_agent_type(agent_type: &str) -> Option<&str> {
        agent_type
            .strip_prefix(AcpAgent::agent_id_prefix())
            .filter(|client_id| !client_id.trim().is_empty())
    }

    /// The ACP client id when `session_id` is a flow session id of the shape
    /// `acp_<client_id>_<uuid>` (created by the frontend `create_acp_flow_session`,
    /// `acp_control` create, or the SessionControl `acp__` path; see
    /// interfaces/acp session_persistence.rs:44). Flow sessions live in the ACP
    /// persistence store, not the internal session store, so they are detected
    /// by id shape instead of a registry lookup. The trailing UUID segment is
    /// shape-checked so an internal session id that happens to start with
    /// `acp_` is never mistaken for a flow session. Single authoritative
    /// implementation lives in `bitfun_runtime_ports` (d3-P2-2).
    fn acp_flow_client_id_from_session_id(session_id: &str) -> Option<&str> {
        bitfun_runtime_ports::acp_flow_client_id_from_session_id(session_id).and_then(|_| {
            // 借用指向传入 session_id 的子串：权威实现已校验形状，
            // 这里把所有权转换回借用，保持调用点签名不变。
            // 用 get() 安全切片（权威实现已保证形状，边界必然合法，
            // 但防御性 get() 避免 panic）。
            let start = 4; // "acp_" 前缀长度
            let end = session_id.len().checked_sub(37)?; // 尾段 "_<36 字符 uuid>" 长度
            session_id.get(start..end)
        })
    }

    /// COORD-03 权威判定：查 ACP 流会话注册表（workspace 会话存储中的持久
    /// 化记录）。流会话记录由 `AcpClientPort::create_session` 写入（provider=
    /// acp + acpClientId 元数据），回收（`delete_session_record`）后记录被
    /// 删除，因此记录状态是「是否活跃外部 ACP 流会话」的权威事实：
    /// - `Active`：记录在册且 provider=acp，附记录中的 client id；
    /// - `NotAcpFlow`：记录在册但不是 ACP 流会话（内部会话命中形状）；
    /// - `Missing`：无记录（已回收或从未创建）——派发前存活校验失败。
    ///
    /// 同一存储目录（`get_effective_session_path`）同时承载内部会话与 ACP
    /// 流会话记录，provider 标记负责区分；与 desktop `AcpClientPort` 的
    /// `session_storage_path` 解析一致（本地 workspace，不涉及 remote）。
    async fn acp_flow_session_registry_status(
        workspace_path: &str,
        session_id: &str,
    ) -> BitFunResult<AcpFlowSessionRegistryStatus> {
        use crate::agentic::persistence::PersistenceManager;
        use crate::infrastructure::get_path_manager_arc;
        use crate::service::remote_ssh::workspace_state::get_effective_session_path;

        let storage_path = get_effective_session_path(workspace_path, None, None).await;
        let persistence = PersistenceManager::new(get_path_manager_arc())
            .map_err(|error| BitFunError::tool(error.to_string()))?;
        let Some(metadata) = persistence
            .load_session_metadata(&storage_path, session_id)
            .await
            .map_err(|error| BitFunError::tool(error.to_string()))?
        else {
            return Ok(AcpFlowSessionRegistryStatus::Missing);
        };
        let Some(custom) = metadata.custom_metadata.as_ref() else {
            return Ok(AcpFlowSessionRegistryStatus::NotAcpFlow);
        };
        if custom
            .get(ACP_FLOW_METADATA_PROVIDER_KEY)
            .and_then(Value::as_str)
            != Some(ACP_FLOW_METADATA_PROVIDER_VALUE)
        {
            return Ok(AcpFlowSessionRegistryStatus::NotAcpFlow);
        }
        let client_id = custom
            .get(ACP_FLOW_METADATA_CLIENT_ID_KEY)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        match client_id {
            Some(client_id) => Ok(AcpFlowSessionRegistryStatus::Active { client_id }),
            // provider=acp 但 client id 缺失/为空：异常记录，无法确认归属，
            // 按非 ACP 流会话拒绝（不路由）。
            None => Ok(AcpFlowSessionRegistryStatus::NotAcpFlow),
        }
    }

    /// Forward one ACP direct message through the real channel with streaming.
    /// Text chunks are pushed into `chunk_sink` as they arrive and the full
    /// external response is returned; failures are port errors.
    async fn acp_direct_send_stream(
        port: &dyn AcpClientPort,
        op: AcpDirectSendOp,
        chunk_sink: AcpClientStreamChunkSink,
    ) -> PortResult<AcpClientMessageResult> {
        match op {
            AcpDirectSendOp::Flow(request) => port.send_message_stream(request, chunk_sink).await,
            AcpDirectSendOp::Bitfun(request) => {
                port.send_message_to_bitfun_session_stream(request, chunk_sink)
                    .await
            }
        }
    }

    /// Async ACP direct delivery: spawn a background task that forwards the
    /// message through the port and, once the external turn completes, streams
    /// the response back through `agentic://` turn events for the target
    /// session and delivers the response to the sender session as a follow-up.
    ///
    /// The tool call itself returns immediately with an acceptance text; it no
    /// longer blocks on the external agent's full turn.
    fn spawn_acp_direct_delivery(
        port: Arc<dyn AcpClientPort>,
        op: AcpDirectSendOp,
        coordinator: Arc<ConversationCoordinator>,
        scheduler: Arc<DialogScheduler>,
        target_session_id: String,
        user_input: String,
        source: AcpDirectReplySource,
    ) {
        tokio::spawn(async move {
            Self::run_acp_direct_delivery(
                port.as_ref(),
                op,
                coordinator.as_ref(),
                scheduler.as_ref(),
                &target_session_id,
                &user_input,
                &source,
            )
            .await;
        });
    }

    /// Completion path of one ACP direct delivery: stream the external reply
    /// back through per-chunk turn events for the target session and route the
    /// external response back to the sender session (follow-up), or emit a
    /// failure event on port error. Turn event order is preserved:
    /// `DialogTurnStarted` → [`ModelRoundStarted`] → zero or more `TextChunk`
    /// → [`ModelRoundCompleted`] → `DialogTurnCompleted`. Round events are
    /// emitted only when the reply produces text (mirroring the non-streaming
    /// path); the `ModelRoundCompleted` is emitted first when the port fails
    /// after a partial reply, so no round is left dangling.
    async fn run_acp_direct_delivery(
        port: &dyn AcpClientPort,
        op: AcpDirectSendOp,
        coordinator: &ConversationCoordinator,
        scheduler: &DialogScheduler,
        target_session_id: &str,
        user_input: &str,
        source: &AcpDirectReplySource,
    ) {
        let turn_id = Uuid::new_v4().to_string();
        let round_id = Uuid::new_v4().to_string();
        let started_at = Instant::now();
        // a19 后端落盘时间基准：事件流内无法再次取时（事件不携带时间戳）。
        let turn_started_at_ms = acp_direct_delivery_now_unix_ms();
        // a19 后端落盘目标工作区：在 `op` 被 move 进发送 future 前提取。
        let target_workspace_path = acp_direct_delivery_workspace_path(&op).map(ToOwned::to_owned);
        coordinator
            .emit_event(AgenticEvent::DialogTurnStarted {
                session_id: target_session_id.to_string(),
                turn_id: turn_id.clone(),
                turn_index: 0,
                user_input: user_input.to_string(),
                original_user_input: Some(user_input.to_string()),
                user_message_metadata: None,
            })
            .await;

        // Stream the external reply: the port pushes text chunks into the
        // channel while the recv loop emits one `TextChunk` turn event per
        // chunk, so the frontend renders the reply incrementally instead of
        // receiving the whole response in a single chunk. `join!` keeps the
        // recv loop running concurrently with the port call; the channel
        // closes when the port call finishes, ending the loop.
        let (chunk_tx, mut chunk_rx) = tokio::sync::mpsc::unbounded_channel();
        let send_future = Self::acp_direct_send_stream(port, op, chunk_tx);
        let stream_turn_events = async {
            let mut round_started = false;
            while let Some(chunk) = chunk_rx.recv().await {
                if let AcpClientStreamChunk::Text { text } = chunk {
                    if !round_started {
                        // 与 coordinator.rs 既有模式一致：TextChunk 前先补发
                        // ModelRoundStarted，让前端正常建立 round 容器，再流式输出文本。
                        coordinator
                            .emit_event(AgenticEvent::ModelRoundStarted {
                                session_id: target_session_id.to_string(),
                                turn_id: turn_id.clone(),
                                round_id: round_id.clone(),
                                round_group_id: None,
                                round_index: 0,
                                model_config_id: String::new(),
                                effective_model_name: String::new(),
                            })
                            .await;
                        round_started = true;
                    }
                    coordinator
                        .emit_event(AgenticEvent::TextChunk {
                            session_id: target_session_id.to_string(),
                            turn_id: turn_id.clone(),
                            round_id: round_id.clone(),
                            attempt_id: None,
                            attempt_index: None,
                            text,
                        })
                        .await;
                }
            }
            round_started
        };
        let (sent, round_started) = tokio::join!(send_future, stream_turn_events);
        let duration_ms = started_at.elapsed().as_millis() as u64;

        match sent {
            Ok(sent) => {
                if round_started {
                    coordinator
                        .emit_event(AgenticEvent::ModelRoundCompleted {
                            session_id: target_session_id.to_string(),
                            turn_id: turn_id.clone(),
                            round_id: round_id.clone(),
                            has_tool_calls: false,
                            duration_ms: Some(duration_ms),
                            provider_id: None,
                            model_config_id: String::new(),
                            effective_model_name: String::new(),
                            first_chunk_ms: None,
                            first_visible_output_ms: None,
                            stream_duration_ms: None,
                            attempt_count: None,
                            failure_category: None,
                            token_details: None,
                        })
                        .await;
                }
                coordinator
                    .emit_event(AgenticEvent::DialogTurnCompleted {
                        session_id: target_session_id.to_string(),
                        turn_id: turn_id.clone(),
                        total_rounds: 1,
                        total_tools: 0,
                        duration_ms,
                        partial_recovery_reason: None,
                        success: Some(true),
                        // "complete" 是前端 NORMAL_FINISH_REASONS 内的正常终止码，
                        // 避免误报「非标准方式结束」横幅。
                        finish_reason: Some("complete".to_string()),
                        has_final_response: Some(true),
                    })
                    .await;
                // a19 后端同构落盘：外部回复直接写入目标 ACP 会话的持久化 turn
                // 文件，不依赖前端事件流（前端未打开/事件流中断时 SessionHistory
                // 仍可读）。失败仅告警，不破坏通知式路径（COORD-15 follow-up
                // 照常投递）。
                if let Some(workspace_path) = target_workspace_path.as_deref() {
                    persist_acp_direct_delivery_to_workspace(
                        workspace_path,
                        target_session_id,
                        &turn_id,
                        user_input,
                        &round_id,
                        turn_started_at_ms,
                        &sent.response,
                        crate::service::session::TurnStatus::Completed,
                        None,
                    )
                    .await;
                }
                // AgentSessionReplyRoute semantics: deliver the external
                // response back to the sender session as a follow-up.
                //
                // COORD-15：事件流（DialogTurnStarted → TextChunk →
                // DialogTurnCompleted）已在目标会话完成流式渲染，是外部回复的
                // 唯一完整呈现；follow-up 的 content/display 均只注入通知句
                // （完成回执），全文保留在 ACP 流会话历史，发起方用
                // SessionHistory 自查，避免 ACP 直通事件流与本地 follow-up
                // 双重呈现、也避免全文膨胀发起方上下文。
                let content = acp_direct_response_notice(&sent.response, target_session_id);
                let display = format!(
                    "External ACP session '{}' responded; the full reply is streamed in that session's chat view.",
                    target_session_id
                );
                if let Err(error) = scheduler
                    .deliver_background_result(
                        source.source_session_id.clone(),
                        String::new(),
                        Some(source.source_workspace.clone()),
                        source.source_remote_connection_id.clone(),
                        source.source_remote_ssh_host.clone(),
                        content,
                        Some(display),
                        None,
                    )
                    .await
                {
                    warn!(
                        "Failed to deliver ACP direct response back to source: source_session_id={}, target_session_id={}, error={}",
                        source.source_session_id, target_session_id, error
                    );
                }
            }
            Err(error) => {
                if round_started {
                    coordinator
                        .emit_event(AgenticEvent::ModelRoundCompleted {
                            session_id: target_session_id.to_string(),
                            turn_id: turn_id.clone(),
                            round_id: round_id.clone(),
                            has_tool_calls: false,
                            duration_ms: Some(duration_ms),
                            provider_id: None,
                            model_config_id: String::new(),
                            effective_model_name: String::new(),
                            first_chunk_ms: None,
                            first_visible_output_ms: None,
                            stream_duration_ms: None,
                            attempt_count: None,
                            failure_category: None,
                            token_details: None,
                        })
                        .await;
                }
                coordinator
                    .emit_event(AgenticEvent::DialogTurnFailed {
                        session_id: target_session_id.to_string(),
                        turn_id: turn_id.clone(),
                        error: format!(
                            "ACP direct delivery failed for session '{}': {}",
                            target_session_id, error
                        ),
                        error_category: None,
                        error_detail: None,
                    })
                    .await;
                let error_text = format!(
                    "ACP direct delivery failed for session '{}': {}",
                    target_session_id, error
                );
                // a19 后端同构落盘：失败 turn 也写入持久化存储（与前端在
                // DialogTurnFailed 时保存 error turn 的行为同构）。
                if let Some(workspace_path) = target_workspace_path.as_deref() {
                    persist_acp_direct_delivery_to_workspace(
                        workspace_path,
                        target_session_id,
                        &turn_id,
                        user_input,
                        &round_id,
                        turn_started_at_ms,
                        "",
                        crate::service::session::TurnStatus::Error,
                        Some(error_text.clone()),
                    )
                    .await;
                }
                if let Err(delivery_error) = scheduler
                    .deliver_background_result(
                        source.source_session_id.clone(),
                        String::new(),
                        Some(source.source_workspace.clone()),
                        source.source_remote_connection_id.clone(),
                        source.source_remote_ssh_host.clone(),
                        error_text,
                        None,
                        None,
                    )
                    .await
                {
                    warn!(
                        "Failed to deliver ACP direct failure back to source: source_session_id={}, error={}",
                        source.source_session_id, delivery_error
                    );
                }
            }
        }
    }

    /// Performs one create+send (or send-to-existing) dispatch and returns the
    /// resolved outcome. Shared by the single-target call and every batch item.
    async fn dispatch_single(
        &self,
        params: SessionMessageInput,
        shared: &DispatchShared,
        context: &ToolUseContext,
    ) -> BitFunResult<DispatchOutcome> {
        let message = params
            .message
            .clone()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| BitFunError::tool("message cannot be empty".to_string()))?;
        let source_session_id = &shared.source_session_id;
        let source_workspace = &shared.source_workspace;
        let source_remote_connection_id = shared.source_remote_connection_id.as_deref();
        let source_remote_ssh_host = shared.source_remote_ssh_host.as_deref();
        let coordinator = &shared.coordinator;
        let scheduler = &shared.scheduler;
        let runtime = &shared.runtime;

        let (target_session_id, target_agent_type, created_session_id, workspace_target) =
            if let Some(target_session_id) = params.session_id.clone() {
                if source_session_id == &target_session_id {
                    return Err(BitFunError::tool(
                        "SessionMessage cannot send a message to the same session".to_string(),
                    ));
                }

                // ACP 流会话直通：session_id 形状 `acp_<client_id>_<uuid>`（前端
                // create_acp_flow_session / acp_control / SessionControl acp__ 创建的
                // 真外部 ACP 会话）。流会话不在内部 session store，无法走 workspace
                // binding / list_sessions 解析；直接经 AcpClientPort::send_message 真
                // 通道转发（与 acp_message 同通道，无本地模型 turn）。投递即返回，
                // 外部响应经事件流 + follow-up 回传。
                //
                // COORD-03：形状只作线索，ACP 流会话注册表才是权威判定。命中形状
                // 后先查注册表（派发前存活校验）：记录在册且 provider=acp 且
                // acpClientId 与形状 client id 一致 → 直通；内部会话命中形状 /
                // 记录已回收 / 记录归属 client 不一致 → 显式拒绝而非路由，杜绝
                // 误分流与回收竞态（回收后形状仍命中会把消息发向已释放的会话）。
                if let Some(flow_client_id) =
                    Self::acp_flow_client_id_from_session_id(&target_session_id)
                {
                    // 注册表查询需要 workspace 定位会话存储目录；缺失时无法
                    // 完成权威判定，显式拒绝（不静默直通未校验的会话）。
                    let workspace_path = params.workspace.clone().or_else(|| {
                        context
                            .workspace_root()
                            .map(|path| path.to_string_lossy().to_string())
                    });
                    let registry_status = Self::acp_flow_session_registry_status(
                        workspace_path.as_deref().ok_or_else(|| {
                            BitFunError::tool(format!(
                                "workspace is required to verify the target session '{}'",
                                target_session_id
                            ))
                        })?,
                        &target_session_id,
                    )
                    .await?;
                    let registry_client_id = match registry_status {
                        AcpFlowSessionRegistryStatus::Active { client_id } => client_id,
                        AcpFlowSessionRegistryStatus::NotAcpFlow => {
                            return Err(BitFunError::tool(format!(
                                "session '{}' is not an ACP flow session (its persisted record is not an ACP session record); refusing to route it through the external ACP direct path",
                                target_session_id
                            )));
                        }
                        AcpFlowSessionRegistryStatus::Missing => {
                            return Err(BitFunError::tool(format!(
                                "ACP flow session '{}' was not found in the flow-session registry; it may have been recycled or never created",
                                target_session_id
                            )));
                        }
                    };
                    if registry_client_id != flow_client_id {
                        return Err(BitFunError::tool(format!(
                            "ACP flow session '{}' is registered for client '{}', not '{}'; refusing to route",
                            target_session_id, registry_client_id, flow_client_id
                        )));
                    }
                    let port = coordinator.acp_client_port().ok_or_else(|| {
                        BitFunError::tool(
                            "ACP client port is not available; the desktop host did not inject it"
                                .to_string(),
                        )
                    })?;
                    // Resolve before the move below: the flow client id borrows
                    // from `target_session_id`, which is moved into the outcome.
                    let target_agent_type = format!("acp:{}", flow_client_id);
                    let resolved_workspace = workspace_path.clone().unwrap_or_default();
                    let result_text = format!(
                        "Message accepted for external ACP session '{}' in workspace '{}' using agent type '{}'. The external agent response will stream back once it completes.",
                        target_session_id, resolved_workspace, target_agent_type
                    );
                    let source = AcpDirectReplySource {
                        source_session_id: source_session_id.clone(),
                        source_workspace: source_workspace.clone(),
                        source_remote_connection_id: source_remote_connection_id
                            .map(ToOwned::to_owned),
                        source_remote_ssh_host: source_remote_ssh_host.map(ToOwned::to_owned),
                    };
                    Self::spawn_acp_direct_delivery(
                        port,
                        AcpDirectSendOp::Flow(AcpClientMessageRequest {
                            session_id: target_session_id.clone(),
                            message: message.clone(),
                            workspace_path: workspace_path.clone(),
                            timeout_seconds: Some(configured_acp_direct_timeout_secs().await),
                        }),
                        coordinator.clone(),
                        scheduler.clone(),
                        target_session_id.clone(),
                        message.clone(),
                        source,
                    );
                    return Ok(DispatchOutcome {
                        target_session_id,
                        target_agent_type,
                        created_session_id: None,
                        workspace_path: resolved_workspace,
                        delivery: "acp_direct",
                        result_text,
                        acp_response: None,
                    });
                }

                let workspace_target = runtime
                    .resolve_session_workspace_binding(AgentSessionWorkspaceRequest {
                        session_id: target_session_id.clone(),
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?;
                let workspace_target = workspace_target.ok_or_else(|| {
                    BitFunError::NotFound(format!(
                        "Workspace for session '{}' could not be resolved",
                        target_session_id
                    ))
                })?;
                let workspace_target = self.workspace_target_from_binding(workspace_target);

                if let Some(workspace) = params.workspace.as_deref() {
                    let requested_workspace = self.resolve_workspace(workspace, context)?;
                    let requested_target =
                        self.workspace_target_from_context(requested_workspace.clone(), context);
                    if !Self::same_workspace_identity(&requested_target, &workspace_target) {
                        return Err(BitFunError::NotFound(format!(
                            "Session '{}' not found in workspace '{}'",
                            target_session_id, requested_target.workspace_path
                        )));
                    }
                }

                let visible_sessions = runtime
                    .list_sessions(AgentSessionListRequest {
                        workspace_path: workspace_target.project_workspace_path.clone(),
                        remote_connection_id: workspace_target.remote_connection_id.clone(),
                        remote_ssh_host: workspace_target.remote_ssh_host.clone(),
                        include_hidden: true,
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?;
                let listed_agent_type =
                    Self::target_agent_type_from_sessions(&visible_sessions, &target_session_id);
                let resolved_agent_type = if listed_agent_type.is_none() {
                    Self::target_agent_type_from_resolution(
                        runtime
                            .resolve_session_agent_type(&target_session_id)
                            .await
                            .map_err(|error| {
                                BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(
                                    error,
                                ))
                            })?,
                    )
                } else {
                    None
                };
                let target_agent_type =
                    listed_agent_type.or(resolved_agent_type).ok_or_else(|| {
                        BitFunError::NotFound(format!("Session '{}' not found", target_session_id))
                    })?;

                (target_session_id, target_agent_type, None, workspace_target)
            } else {
                let workspace = self.resolve_workspace(
                    params.workspace.as_deref().ok_or_else(|| {
                        BitFunError::tool(
                            "workspace is required when session_id is omitted".to_string(),
                        )
                    })?,
                    context,
                )?;
                let workspace_target = self.workspace_target_from_context(workspace, context);
                // R-WF-23: 会话创建层级权限链——create 默认继承创建者工作区
                // （跨区 create 拒绝）+ L0/L1/L2 层级校验。与 SessionControl
                // create 同款封装，复用 same_session_storage_dir /
                // caller_is_owner_session / 会话树 depth（不新造校验函数）。
                {
                    let caller_session_id = context.session_id.as_deref().ok_or_else(|| {
                        BitFunError::tool(
                            "create requires a caller session in tool context".to_string(),
                        )
                    })?;
                    let caller_workspace_path = context.workspace_root().ok_or_else(|| {
                        BitFunError::tool(
                            "create requires a caller workspace in tool context".to_string(),
                        )
                    })?;
                    super::session_control_tool::enforce_session_create_workspace_hierarchy(
                        coordinator.get_session_manager(),
                        coordinator.session_tree(),
                        caller_session_id,
                        caller_workspace_path,
                        std::path::Path::new(&workspace_target.workspace_path),
                    )
                    .await?;
                }
                let session_name = params
                    .session_name
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        BitFunError::tool(
                            "session_name is required when session_id is omitted".to_string(),
                        )
                    })?;
                let agent_type = params
                    .agent_type
                    .as_ref()
                    .ok_or_else(|| {
                        BitFunError::tool(
                            "agent_type is required when session_id is omitted".to_string(),
                        )
                    })?
                    .as_str()
                    .to_string();

                // W9: remote 互斥拒绝（SessionMessage create 与
                // SessionControl create 同一语义）。
                let mut created_worktree: Option<SessionWorktreeCreateResult> = None;
                if let Some(worktree) = &params.worktree {
                    super::session_control_tool::ensure_worktree_not_remote(context)?;
                    let worktree_options = worktree;
                    let request_id = context
                        .tool_call_id
                        .as_deref()
                        .map(|tool_call_id| format!("session-message:{tool_call_id}:worktree"))
                        .unwrap_or_else(|| {
                            format!("session-message:{}:worktree", uuid::Uuid::new_v4())
                        });
                    created_worktree = Some(
                        super::session_control_tool::create_worktree_for_session(
                            &request_id,
                            &SessionControlWorkspaceTarget {
                                display_workspace: workspace_target.workspace_path.clone(),
                                project_workspace: workspace_target.project_workspace_path.clone(),
                                execution_target: workspace_target.execution_target.clone(),
                                workspace_id: workspace_target.workspace_id.clone(),
                                remote_connection_id: workspace_target.remote_connection_id.clone(),
                                remote_ssh_host: workspace_target.remote_ssh_host.clone(),
                            },
                            worktree_options,
                            context,
                        )
                        .await?,
                    );
                }

                let created_by = self.creator_session_marker(context)?;
                let mut metadata = serde_json::Map::new();
                metadata.insert("createdBy".to_string(), json!(created_by));
                // A2（幽灵会话删除修复）：SessionMessage create 补 lineage 元数据，
                // 对齐 SessionControl create 链——parentSessionId/subagentType/subagent
                // 使创建路径产出 Subagent kind（coordinator 读取这些键），随后在下方
                // 持久化 SessionRelationship 并注册内存树，根治「只写 createdBy 不挂树」
                // 的孤儿源头（幽灵会话删除根因 A 同根源头）。
                metadata.insert(
                    "parentSessionId".to_string(),
                    json!(context.session_id.clone()),
                );
                metadata.insert("subagentType".to_string(), json!(agent_type.clone()));
                metadata.insert("subagent".to_string(), json!(true));
                // Persistent copy of the plan-todo binding on the created
                // session record (the turn-channel copy is injected at submit).
                if let Some(plan_file) = params.plan_file.as_deref() {
                    metadata.insert(PLAN_FILE_METADATA_KEY.to_string(), json!(plan_file));
                }
                if let Some(todo_id) = params.todo_id.as_deref() {
                    metadata.insert(TODO_ID_METADATA_KEY.to_string(), json!(todo_id));
                }
                let session = match runtime
                    .create_session(AgentSessionCreateRequest {
                        session_name,
                        agent_type: agent_type.clone(),
                        workspace_path: Some(
                            created_worktree
                                .as_ref()
                                .map(|wt| wt.execution_target.root_path.clone())
                                .unwrap_or_else(|| workspace_target.workspace_path.clone()),
                        ),
                        project_workspace_path: Some(
                            created_worktree
                                .as_ref()
                                .map(|wt| wt.project_workspace_path.clone())
                                .unwrap_or_else(|| workspace_target.project_workspace_path.clone()),
                        ),
                        execution_target: created_worktree
                            .as_ref()
                            .map(|wt| wt.execution_target.clone())
                            .or_else(|| workspace_target.execution_target.clone()),
                        workspace_id: created_worktree
                            .as_ref()
                            .and_then(|wt| wt.tracked_workspace_id.clone())
                            .or_else(|| workspace_target.workspace_id.clone()),
                        remote_connection_id: workspace_target.remote_connection_id.clone(),
                        remote_ssh_host: workspace_target.remote_ssh_host.clone(),
                        model_id: None,
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

                // A2（幽灵会话删除修复）：创建后挂树——持久化 SessionRelationship 并
                // 注册内存树，对齐 SessionControl create 的 lineage 写入（R-001/R-002/R-003）。
                // lineage 持久化失败回滚已创建的会话（同 SessionControl create 的失败回滚，
                // 见 session_control_tool.rs 的 persist_session_lineage 失败回滚），确保不留下
                // 无父子关系记录的孤儿会话；回滚自身失败仍要上报（绝不静默降级）。
                if let Some(parent_session_id) = context.session_id.as_ref() {
                    use bitfun_services_core::session::types::{
                        SessionRelationship, SessionRelationshipKind,
                    };
                    let parent_depth = coordinator
                        .session_manager
                        .load_session_metadata(
                            &std::path::PathBuf::from(&workspace_target.project_workspace_path),
                            parent_session_id,
                        )
                        .await
                        .ok()
                        .flatten()
                        .and_then(|m| m.relationship.and_then(|r| r.depth))
                        .unwrap_or(0u32);
                    let child_depth = parent_depth + 1;
                    let relationship = SessionRelationship {
                        kind: Some(SessionRelationshipKind::Subagent),
                        parent_session_id: Some(parent_session_id.clone()),
                        depth: Some(child_depth),
                        ..Default::default()
                    };
                    if let Err(error) = coordinator
                        .session_manager
                        .persist_session_lineage(&session.session_id, relationship)
                        .await
                    {
                        log::warn!(
                            "SessionMessage create: lineage persist failed for {}, retrying once: {:?}",
                            session.session_id,
                            error
                        );
                        // 重试一次以吸收瞬时 IO 故障（同 SessionControl create 模式）。
                        let relationship = SessionRelationship {
                            kind: Some(SessionRelationshipKind::Subagent),
                            parent_session_id: Some(parent_session_id.clone()),
                            depth: Some(child_depth),
                            ..Default::default()
                        };
                        if let Err(retry_error) = coordinator
                            .session_manager
                            .persist_session_lineage(&session.session_id, relationship)
                            .await
                        {
                            // 回滚创建：删除刚创建的会话；回滚自身失败时仍要上报。
                            if let Err(rollback_error) = coordinator
                                .delete_session(
                                    std::path::Path::new(&workspace_target.project_workspace_path),
                                    &session.session_id,
                                )
                                .await
                            {
                                log::error!(
                                    "SessionMessage create: lineage persist failed for {} ({:?}), rollback of session also failed: {:?}",
                                    session.session_id, retry_error, rollback_error
                                );
                            }
                            return Err(BitFunError::tool(format!(
                                "failed to persist session lineage for {} after retry: {}",
                                session.session_id, retry_error
                            )));
                        }
                    }
                    // 内存树注册是 best-effort（R-003 语义，同 SessionControl create）：
                    // 注册失败只 warn，lineage 已持久化，重启后由 list 重建树。
                    if let Err(error) = coordinator.session_tree().register_child(
                        parent_session_id,
                        &session.session_id,
                        child_depth,
                    ) {
                        log::warn!(
                            "SessionMessage create: failed to register child {} under {} in tree: {:?}",
                            session.session_id,
                            parent_session_id,
                            error
                        );
                    }
                }

                (
                    session.session_id.clone(),
                    session.agent_type.clone(),
                    Some(session.session_id),
                    workspace_target,
                )
            };

        // ACP direct path: `acp__<client>` targets are external agents.
        // Forward the message through the ACP client port (addressed by the
        // internal BitFun session id, same identity the AcpAgentTool bridge
        // uses) — no local model turn, no bridge re-translation. Delivery
        // returns immediately; the external response streams back through
        // `agentic://` turn events and a follow-up reply to the sender.
        // When the port is unavailable the dispatch fails loudly instead of
        // falling back to the local model (a fallback would re-introduce the
        // double-billing path).
        //
        // COORD-03：agent_type 前缀 `acp__` 只作线索，ACP client 注册表才是
        // 权威判定。内部会话命中形状但 client 未注册（历史壳会话 / 用户自定义
        // 类型）时显式拒绝而非路由到外部，防误分流；client 已注册时直通（会话
        // 级外部进程绑定由发送端口兜底，失败经事件流 + follow-up 回传）。
        if let Some(client_id) = Self::acp_client_id_from_agent_type(&target_agent_type) {
            let port = coordinator.acp_client_port().ok_or_else(|| {
                BitFunError::tool(
                    "ACP client port is not available; the desktop host did not inject it"
                        .to_string(),
                )
            })?;
            let listed_clients = port.list_clients().await.map_err(|error| {
                BitFunError::tool(format!(
                    "failed to verify the ACP client registry for agent type '{}': {}",
                    target_agent_type, error.message
                ))
            })?;
            if !listed_clients
                .clients
                .iter()
                .any(|client| client.client_id == client_id)
            {
                return Err(BitFunError::tool(format!(
                    "session '{}' uses agent type '{}' but ACP client '{}' is not registered; refusing to route to a non-existent external agent",
                    target_session_id, target_agent_type, client_id
                )));
            }
            let result_text = format!(
                "Message accepted for external ACP session '{}' in workspace '{}' using agent type '{}'. The external agent response will stream back once it completes.",
                target_session_id, workspace_target.workspace_path, target_agent_type
            );
            let source = AcpDirectReplySource {
                source_session_id: source_session_id.clone(),
                source_workspace: source_workspace.clone(),
                source_remote_connection_id: source_remote_connection_id.map(ToOwned::to_owned),
                source_remote_ssh_host: source_remote_ssh_host.map(ToOwned::to_owned),
            };
            Self::spawn_acp_direct_delivery(
                port,
                AcpDirectSendOp::Bitfun(AcpClientBitfunMessageRequest {
                    client_id: client_id.to_string(),
                    bitfun_session_id: target_session_id.clone(),
                    message: message.clone(),
                    workspace_path: Some(workspace_target.workspace_path.clone()),
                    timeout_seconds: Some(configured_acp_direct_timeout_secs().await),
                }),
                coordinator.clone(),
                scheduler.clone(),
                target_session_id.clone(),
                message.clone(),
                source,
            );
            return Ok(DispatchOutcome {
                target_session_id,
                target_agent_type,
                created_session_id,
                workspace_path: workspace_target.workspace_path,
                delivery: "acp_direct",
                result_text,
                acp_response: None,
            });
        }

        // PR #2139 #5: delivery authorization gate. The target session is
        // resolved (exists) and not an ACP direct path (both ACP direct paths
        // above returned after registry verification); only local delivery is
        // handled here (steer_dialog_turn / submit_dialog_turn). Shares the R4
        // authorization verdict with SessionControl delete/cancel:
        // daemon session interception (R-A.04), owner (Commander role
        // or RBAC off) exemption, created_by matching
        // (`session-<caller>` marker, written by creator_session_marker when
        // creating a new session), ancestor authorization (in-memory tree fast
        // path + persisted metadata chain fallback). The new-session branch
        // (created_session_id.is_some()) is a self-created session and skips
        // the gate.
        if created_session_id.is_none() {
            resolve_session_mutation_authorization(
                coordinator.get_session_manager(),
                coordinator.session_tree(),
                source_session_id,
                &target_session_id,
                std::path::Path::new(&workspace_target.project_workspace_path),
                "deliver to",
                SessionMutationAuthOptions::deliver(),
            )
            .await?;
        }

        let sender_identity = self
            .resolve_sender_identity(
                runtime,
                context,
                source_session_id,
                source_workspace,
                source_remote_connection_id,
                source_remote_ssh_host,
                coordinator,
            )
            .await;
        let (forwarded_message, prepended_messages) =
            self.format_forwarded_message(&message, &sender_identity);

        // Urgent delivery: when the target session is currently processing a turn,
        // inject the message into that running turn via the UserSteering channel
        // (interrupts after the current atomic unit) instead of starting a new turn.
        // Honest fallback: when the target session is not processing, or the steering
        // is rejected (the turn ended between the state query and the submit), deliver
        // through the normal submission path so the message is never dropped.
        let mut steering_turn_id: Option<String> = None;
        let has_plan_todo_binding = params.plan_file.is_some() || params.todo_id.is_some();
        if should_attempt_steering(
            params.urgent,
            created_session_id.as_deref(),
            has_plan_todo_binding,
        ) {
            match resolve_urgent_delivery(scheduler.current_processing_turn_id(&target_session_id))
            {
                UrgentDelivery::Steer { turn_id } => {
                    match scheduler
                        .steer_dialog_turn(AgentDialogSteerRequest {
                            session_id: target_session_id.clone(),
                            turn_id: turn_id.clone(),
                            content: forwarded_message.clone(),
                            display_content: Some(message.clone()),
                            prepended_reminders: prepended_messages.clone(),
                            attachments: Vec::new(),
                            metadata: serde_json::Map::new(),
                        })
                        .await
                    {
                        Ok(_outcome) => {
                            steering_turn_id = Some(turn_id.clone());
                            // R-ASYNC-01（项2）+ urgent-reply-01（方案 B）：
                            // urgent 引导注入成功后标记目标 turn 及注入方
                            // source_session_id——完成时抑制自动回传（双回复
                            // 根除）。注入消息的回复由注入通道交付，注入 turn
                            // 再自动回传即产生双回复（该 turn 的 reply_route 是
                            // 发起方等待回传时设定的）。但当 reply_route 指向
                            // 注入方时（注入通道无回传能力，自动回传是唯一
                            // 通道）不抑制——见 scheduler 消费点匹配。
                            scheduler.mark_injected_turn_reply_suppressed(
                                &target_session_id,
                                &turn_id,
                                source_session_id,
                            );
                            info!(
                                "Urgent SessionMessage steered into running turn: source_session_id={}, target_session_id={}, turn_id={}",
                                source_session_id, target_session_id, turn_id
                            );
                        }
                        Err(error) => {
                            warn!(
                                "Urgent SessionMessage steering rejected, falling back to normal submit: target_session_id={}, turn_id={}, error={}",
                                target_session_id, turn_id, error
                            );
                        }
                    }
                }
                UrgentDelivery::NormalSubmit => {}
            }
        }

        if steering_turn_id.is_none() {
            // Turn-channel binding injection: when the caller bound the
            // dispatched session to a plan todo, carry planFile/todoId in the
            // forwarded turn metadata so the scheduler can auto-mark the todo
            // (in_progress at turn start, completed on a Completed outcome).
            //
            // Group chat correlation (R-GC-36): when the calling member session
            // runs inside a group context, the coordinator forwards the group
            // session id into tool custom_data ("groupId", camelCase to match the
            // group_room metadata contract). Re-attach it to the forwarded turn so
            // the relayed message keeps the group id; a non-group caller has no
            // such key and stays None (zero pollution, no fallback).
            let group_context = Self::group_context_from_custom_data(&context.custom_data);
            let mut forwarded_metadata =
                Self::forwarded_user_input_metadata(context, &sender_identity, &group_context);
            if let Some(plan_file) = params.plan_file.as_deref() {
                forwarded_metadata.insert(PLAN_FILE_METADATA_KEY.to_string(), json!(plan_file));
            }
            if let Some(todo_id) = params.todo_id.as_deref() {
                forwarded_metadata.insert(TODO_ID_METADATA_KEY.to_string(), json!(todo_id));
            }
            runtime
                .submit_dialog_turn(AgentDialogTurnRequest {
                    session_id: target_session_id.clone(),
                    message: forwarded_message,
                    original_message: Some(message.clone()),
                    turn_id: None,
                    execution: Default::default(),
                    agent_type: target_agent_type.clone(),
                    workspace_path: Some(workspace_target.workspace_path.clone()),
                    remote_connection_id: workspace_target.remote_connection_id.clone(),
                    remote_ssh_host: workspace_target.remote_ssh_host.clone(),
                    policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
                    reply_route: Some(AgentSessionReplyRoute {
                        source_session_id: source_session_id.clone(),
                        source_workspace_path: source_workspace.clone(),
                        source_remote_connection_id: source_remote_connection_id
                            .map(ToOwned::to_owned),
                        source_remote_ssh_host: source_remote_ssh_host.map(ToOwned::to_owned),
                    }),
                    prepended_reminders: prepended_messages,
                    attachments: Vec::new(),
                    metadata: forwarded_metadata,
                })
                .await
                .map_err(|error| {
                    BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                })?;
        }

        let urgent_fell_back =
            params.urgent && steering_turn_id.is_none() && created_session_id.is_none();
        let mut result_text = if let Some(steered_turn_id) = steering_turn_id.as_ref() {
            format!(
                "Urgent message injected into the running turn '{}' of session '{}' in workspace '{}' using agent type '{}'.",
                steered_turn_id, target_session_id, workspace_target.workspace_path, target_agent_type
            )
        } else if let Some(created_session_id) = created_session_id.as_ref() {
            format!(
                "Created session '{}' and accepted the message in workspace '{}' using agent type '{}'.",
                created_session_id, workspace_target.workspace_path, target_agent_type
            )
        } else {
            format!(
                "Message accepted for session '{}' in workspace '{}' using agent type '{}'.",
                target_session_id, workspace_target.workspace_path, target_agent_type
            )
        };
        if urgent_fell_back {
            result_text.push_str(
                " Steering into the running turn was not possible (the target session was idle, its turn had just ended, the queue was congested, or the message carries a plan-todo binding that the steering channel cannot carry), so the urgent message was delivered as a normal submission instead of a mid-turn correction.",
            );
        }

        Ok(DispatchOutcome {
            target_session_id,
            target_agent_type,
            created_session_id,
            workspace_path: workspace_target.workspace_path,
            delivery: if steering_turn_id.is_some() {
                "steered"
            } else {
                "submitted"
            },
            result_text,
            acp_response: None,
        })
    }

    /// Batch dispatch: runs each item sequentially and independently. A failed
    /// item never rolls back already-succeeded items and never stops later
    /// items; the per-item result array keeps every session id so the caller
    /// can skip succeeded items when retrying the failed ones.
    async fn call_batch(
        &self,
        params: &SessionMessageInput,
        items: &[BatchItem],
        shared: &DispatchShared,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let mut results = Vec::with_capacity(items.len());
        for item in items {
            let item_params = SessionMessageInput {
                workspace: params.workspace.clone(),
                session_id: item.session_id.clone(),
                session_name: item.session_name.clone(),
                message: Some(item.message.clone()),
                agent_type: item.agent_type.clone(),
                urgent: item.urgent,
                plan_file: item.plan_file.clone(),
                todo_id: item.todo_id.clone(),
                worktree: item.worktree.clone(),
                batch: None,
            };
            match self.dispatch_single(item_params, shared, context).await {
                Ok(outcome) => {
                    let result_text = outcome.result_text;
                    let mut item_data = json!({
                        "status": "success",
                        "target_session_id": outcome.target_session_id,
                        "target_agent_type": outcome.target_agent_type,
                        "target_workspace": outcome.workspace_path,
                        "created_session_id": outcome.created_session_id,
                        "delivery": outcome.delivery,
                        "result": result_text,
                    });
                    // ACP direct path: expose the external response verbatim.
                    if let Some(response) = outcome.acp_response.as_ref() {
                        item_data["response"] = json!(response);
                    }
                    results.push(item_data);
                }
                Err(error) => {
                    warn!(
                        "Batch SessionMessage item failed (successful items are not rolled back): session_name={:?}, session_id={:?}, error={}",
                        item.session_name, item.session_id, error
                    );
                    results.push(json!({
                        "status": "error",
                        "session_name": item.session_name.clone(),
                        "session_id": item.session_id.clone(),
                        "error": error.to_string(),
                    }));
                }
            }
        }

        let (succeeded, failed, summary) = Self::summarize_batch_results(&results);

        Ok(vec![ToolResult::Result {
            data: json!({
                "success": true,
                "total": results.len(),
                "succeeded": succeeded,
                "failed": failed,
                "results": results,
            }),
            result_for_assistant: Some(summary),
            image_attachments: None,
        }])
    }

    /// Aggregates per-item outcomes into success/failed counts and the summary
    /// text. Successful items are never rolled back; the summary tells the
    /// caller to retry only the failed items using the per-item session ids.
    fn summarize_batch_results(results: &[Value]) -> (usize, usize, String) {
        let succeeded = results
            .iter()
            .filter(|result| result.get("status").and_then(Value::as_str) == Some("success"))
            .count();
        let failed = results.len() - succeeded;
        let mut summary = format!(
            "Batch dispatch of {} message(s): {} succeeded, {} failed. Successful items are not rolled back; retry only the failed items (skip the succeeded session ids below).",
            results.len(),
            succeeded,
            failed
        );
        if failed > 0 {
            summary.push_str(
                " A failed item never rolls back earlier successes, and later items still ran.",
            );
        }
        (succeeded, failed, summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::core::SessionConfig;
    use crate::agentic::events::{EventQueue, EventQueueConfig, EventRouter};
    use crate::agentic::execution::{
        ExecutionEngine, ExecutionEngineConfig, RoundExecutor, StreamProcessor,
    };
    use crate::agentic::persistence::PersistenceManager;
    use crate::agentic::session::{
        compression::{CompressionConfig, ContextCompressor},
        PromptCachePolicy, SessionContextStore, SessionManager, SessionManagerConfig,
    };
    use crate::agentic::tools::framework::ToolUseContext;
    use crate::agentic::tools::registry::ToolRegistry;
    use crate::agentic::tools::{ToolPipeline, ToolStateManager};
    use crate::agentic::WorkspaceBinding;
    use crate::infrastructure::PathManager;
    use bitfun_core_types::{
        SessionExecutionTarget, SessionExecutionTargetKind, WorktreeLifecycle,
    };
    use bitfun_runtime_ports::{
        PortError, PortErrorKind, PortResult, RuntimeServiceCapability, RuntimeServicePort,
    };
    use serde_json::json;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::Duration;
    use tokio::sync::RwLock as TokioRwLock;
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

    fn session_context(session_id: &str) -> ToolUseContext {
        ToolUseContext {
            session_id: Some(session_id.to_string()),
            ..empty_context()
        }
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

    fn workspace_target(
        workspace_path: &str,
        remote_connection_id: Option<&str>,
        remote_ssh_host: Option<&str>,
    ) -> SessionMessageWorkspaceTarget {
        SessionMessageWorkspaceTarget {
            workspace_path: workspace_path.to_string(),
            project_workspace_path: workspace_path.to_string(),
            execution_target: None,
            workspace_id: None,
            remote_connection_id: remote_connection_id.map(ToOwned::to_owned),
            remote_ssh_host: remote_ssh_host.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn session_message_input_parses_worktree_options_and_keeps_legacy_compat() {
        // 旧 payload（无 worktree 字段）解析兼容。
        let legacy: SessionMessageInput = serde_json::from_value(json!({
            "workspace": "/repo",
            "session_name": "legacy",
            "message": "hello",
            "agent_type": "agentic",
        }))
        .expect("legacy payload must parse");
        assert!(legacy.worktree.is_none());

        // 新 payload：worktree 对象解析。
        let with_worktree: SessionMessageInput = serde_json::from_value(json!({
            "workspace": "/repo",
            "session_name": "task-a",
            "message": "hello",
            "agent_type": "agentic",
            "worktree": {
                "baseRef": "main",
                "copyLocalChanges": true
            }
        }))
        .expect("worktree payload must parse");
        assert!(with_worktree.worktree.is_some());
        assert_eq!(
            with_worktree
                .worktree
                .as_ref()
                .and_then(|w| w.base_ref.as_deref()),
            Some("main")
        );
        assert!(with_worktree
            .worktree
            .as_ref()
            .is_some_and(|w| w.copy_local_changes));

        // batch item 的 worktree 解析。
        let batch: SessionMessageInput = serde_json::from_value(json!({
            "workspace": "/repo",
            "batch": [{
                "session_name": "item-a",
                "message": "hi",
                "agent_type": "agentic",
                "worktree": {"copyLocalChanges": false}
            }]
        }))
        .expect("batch payload must parse");
        let item = batch.batch.as_ref().expect("batch").first().expect("item");
        assert!(item.worktree.is_some());
    }

    #[tokio::test]
    async fn session_message_worktree_rejected_for_existing_session_send() {
        // 发送到既有 session_id 时 worktree 被拒绝（create-only 语义）。
        let input = json!({
            "workspace": "/repo",
            "session_id": "existing_1",
            "message": "hello",
            "worktree": {"baseRef": "main"}
        });
        let tool = SessionMessageTool::new();
        let result = tool
            .validate_input(&input, Some(&session_context("caller_1")))
            .await;
        assert!(!result.result);
        assert!(result
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("worktree is only allowed when session_id is omitted"));
    }

    #[test]
    fn creating_in_current_worktree_inherits_project_scope_and_target() {
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
        let binding = WorkspaceBinding::new(None, worktree_path)
            .with_project_root_path(project_path.clone())
            .with_execution_target(Some(execution_target.clone()));
        let mut context = empty_context();
        context.workspace = Some(binding);

        let target = SessionMessageTool::new()
            .workspace_target_from_context("/worktrees/wt-1".to_string(), &context);

        assert_eq!(target.workspace_path, "/worktrees/wt-1");
        assert_eq!(PathBuf::from(target.project_workspace_path), project_path);
        assert_eq!(target.execution_target, Some(execution_target));
    }

    #[test]
    fn workspace_identity_matches_full_remote_tuple() {
        let left = workspace_target("/root/repo", Some("conn-1"), Some("host-a"));
        let right = workspace_target("/root/repo", Some("conn-1"), Some("host-a"));

        assert!(SessionMessageTool::same_workspace_identity(&left, &right));
    }

    #[test]
    fn workspace_identity_rejects_remote_local_parity_mismatch() {
        let requested = workspace_target("/root/repo", None, None);
        let target = workspace_target("/root/repo", Some("conn-1"), Some("host-a"));

        assert!(!SessionMessageTool::same_workspace_identity(
            &requested, &target
        ));
    }

    #[test]
    fn workspace_identity_rejects_remote_host_mismatch() {
        let requested = workspace_target("/root/repo", Some("conn-1"), Some("host-a"));
        let target = workspace_target("/root/repo", Some("conn-1"), Some("host-b"));

        assert!(!SessionMessageTool::same_workspace_identity(
            &requested, &target
        ));
    }

    #[test]
    fn target_agent_type_rejects_empty_agent_type_resolution() {
        assert_eq!(
            SessionMessageTool::target_agent_type_from_resolution(Some(" ".to_string())),
            None
        );
    }

    #[test]
    fn acp_flow_client_id_parses_flow_session_id() {
        assert_eq!(
            SessionMessageTool::acp_flow_client_id_from_session_id(
                "acp_codebuddy_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b"
            ),
            Some("codebuddy")
        );
    }

    #[test]
    fn acp_flow_client_id_parses_client_ids_with_underscores() {
        assert_eq!(
            SessionMessageTool::acp_flow_client_id_from_session_id(
                "acp_claude_code_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b"
            ),
            Some("claude_code")
        );
    }

    #[test]
    fn acp_flow_client_id_rejects_non_flow_session_ids() {
        // Internal session ids are not flow sessions even when they start with
        // "acp_": the trailing segment must be a well-formed UUID.
        assert_eq!(
            SessionMessageTool::acp_flow_client_id_from_session_id("acp_codebuddy"),
            None
        );
        assert_eq!(
            SessionMessageTool::acp_flow_client_id_from_session_id("acp_codebuddy_not-a-uuid"),
            None
        );
        assert_eq!(
            SessionMessageTool::acp_flow_client_id_from_session_id("session-123"),
            None
        );
        assert_eq!(
            SessionMessageTool::acp_flow_client_id_from_session_id(""),
            None
        );
    }

    #[test]
    fn looks_like_uuid_accepts_only_canonical_shape() {
        assert!(looks_like_uuid("7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b"));
        assert!(!looks_like_uuid("7f0e1a2b3c4d4e5f8a9b0c1d2e3f4a5b"));
        assert!(!looks_like_uuid(
            "7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b-extra"
        ));
        assert!(!looks_like_uuid(""));
        assert!(!looks_like_uuid("7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5"));
    }

    #[test]
    fn session_message_forwards_noninteractive_user_input_fact() {
        use bitfun_agent_runtime::user_questions::USER_INPUT_AVAILABLE_CONTEXT_KEY;

        let mut context = empty_context();
        context.custom_data.insert(
            USER_INPUT_AVAILABLE_CONTEXT_KEY.to_string(),
            Value::Bool(false),
        );
        let sender = SenderIdentity {
            session_id: "source-1".to_string(),
            role: Some("Commander".to_string()),
            depth: Some(0),
            name: Some("Assistant".to_string()),
        };

        let metadata = SessionMessageTool::forwarded_user_input_metadata(
            &context,
            &sender,
            &GroupChatForwardMetadata::default(),
        );

        assert_eq!(
            metadata.get(USER_INPUT_AVAILABLE_CONTEXT_KEY),
            Some(&Value::Bool(false))
        );
        assert_eq!(
            metadata.get("senderSessionId"),
            Some(&Value::String("source-1".to_string()))
        );
        assert_eq!(
            metadata.get("senderRole"),
            Some(&Value::String("Commander".to_string()))
        );
        assert_eq!(metadata.get("senderDepth"), Some(&Value::from(0)));
        assert_eq!(
            metadata.get("senderName"),
            Some(&Value::String("Assistant".to_string()))
        );
    }

    #[test]
    fn forwarded_metadata_omits_unknown_sender_fields() {
        let context = empty_context();
        let sender = SenderIdentity {
            session_id: "source-2".to_string(),
            role: None,
            depth: None,
            name: None,
        };

        let metadata = SessionMessageTool::forwarded_user_input_metadata(
            &context,
            &sender,
            &GroupChatForwardMetadata::default(),
        );

        assert_eq!(
            metadata.get("senderSessionId"),
            Some(&Value::String("source-2".to_string()))
        );
        assert!(!metadata.contains_key("senderRole"));
        assert!(!metadata.contains_key("senderDepth"));
        assert!(!metadata.contains_key("senderName"));
    }

    #[test]
    fn forwarded_metadata_carries_group_chat_keys_when_present() {
        let context = empty_context();
        let sender = SenderIdentity {
            session_id: "source-3".to_string(),
            role: None,
            depth: None,
            name: None,
        };
        let group = GroupChatForwardMetadata {
            group_id: Some("room-1".to_string()),
            group_message_id: Some("msg-42".to_string()),
            group_author: Some("__master__".to_string()),
        };

        let metadata = SessionMessageTool::forwarded_user_input_metadata(&context, &sender, &group);

        assert_eq!(
            metadata.get("groupId"),
            Some(&Value::String("room-1".to_string()))
        );
        assert_eq!(
            metadata.get("groupMessageId"),
            Some(&Value::String("msg-42".to_string()))
        );
        assert_eq!(
            metadata.get("groupAuthor"),
            Some(&Value::String("__master__".to_string()))
        );
    }

    #[test]
    fn forwarded_metadata_omits_group_chat_keys_when_absent() {
        let context = empty_context();
        let sender = SenderIdentity {
            session_id: "source-4".to_string(),
            role: None,
            depth: None,
            name: None,
        };

        let metadata = SessionMessageTool::forwarded_user_input_metadata(
            &context,
            &sender,
            &GroupChatForwardMetadata::default(),
        );

        assert!(!metadata.contains_key("groupId"));
        assert!(!metadata.contains_key("groupMessageId"));
        assert!(!metadata.contains_key("groupAuthor"));
    }

    // ── R-GC-36: group id passthrough from the calling (member) context ──
    #[test]
    fn group_context_carries_group_id_when_present() {
        let mut custom_data = std::collections::HashMap::new();
        custom_data.insert("groupId".to_string(), Value::String("room-1".to_string()));

        let group = SessionMessageTool::group_context_from_custom_data(&custom_data);

        assert_eq!(group.group_id.as_deref(), Some("room-1"));
        assert_eq!(group.group_message_id, None);
        assert_eq!(group.group_author, None);
    }

    #[test]
    fn group_context_stays_none_without_group_context() {
        let custom_data = std::collections::HashMap::new();

        let group = SessionMessageTool::group_context_from_custom_data(&custom_data);

        assert_eq!(group.group_id, None);
        assert_eq!(group.group_message_id, None);
        assert_eq!(group.group_author, None);
    }

    #[test]
    fn group_context_ignores_blank_or_non_string_group_id() {
        for custom_data in [
            std::collections::HashMap::new(),
            std::collections::HashMap::from([(
                "groupId".to_string(),
                Value::String("   ".to_string()),
            )]),
            std::collections::HashMap::from([("groupId".to_string(), Value::Bool(true))]),
        ] {
            let group = SessionMessageTool::group_context_from_custom_data(&custom_data);
            assert_eq!(group.group_id, None, "custom_data={custom_data:?}");
        }
    }

    #[test]
    fn target_agent_type_uses_resolved_agent_type() {
        assert_eq!(
            SessionMessageTool::target_agent_type_from_resolution(Some("agentic".to_string()))
                .as_deref(),
            Some("agentic")
        );
    }

    #[test]
    fn target_agent_type_uses_matching_session_agent_type() {
        let sessions = vec![AgentSessionSummary {
            session_id: "worker_1".to_string(),
            session_name: "Worker".to_string(),
            agent_type: "agentic".to_string(),
            model_id: None,
            reasoning_preset: None,
            last_user_dialog_agent_type: None,
            last_submitted_agent_type: None,
            turn_count: 0,
            created_at_ms: 1,
            last_active_at_ms: 2,
            parent_session_id: None,
            status: None,
            display_state: None,
            is_daemon: false,
        }];

        assert_eq!(
            SessionMessageTool::target_agent_type_from_sessions(&sessions, "worker_1").as_deref(),
            Some("agentic")
        );
    }

    #[test]
    fn target_agent_type_rejects_empty_session_agent_type() {
        let sessions = vec![AgentSessionSummary {
            session_id: "worker_1".to_string(),
            session_name: "Worker".to_string(),
            agent_type: " ".to_string(),
            model_id: None,
            reasoning_preset: None,
            last_user_dialog_agent_type: None,
            last_submitted_agent_type: None,
            turn_count: 0,
            created_at_ms: 1,
            last_active_at_ms: 2,
            parent_session_id: None,
            status: None,
            display_state: None,
            is_daemon: false,
        }];

        assert_eq!(
            SessionMessageTool::target_agent_type_from_sessions(&sessions, "worker_1"),
            None
        );
    }

    #[tokio::test]
    async fn validate_existing_session_rejects_agent_type_override() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "session_id": "worker_1",
                    "message": "hello",
                    "agent_type": "DeepResearch",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("agent_type override is not allowed when session_id is provided")
        );
    }

    #[tokio::test]
    async fn validate_new_session_requires_session_name() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "message": "hello",
                    "agent_type": "agentic",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("session_name is required when session_id is omitted")
        );
    }

    #[tokio::test]
    async fn validate_new_session_requires_agent_type() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "message": "hello",
                    "session_name": "Worker Session",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("agent_type is required when session_id is omitted")
        );
    }

    #[tokio::test]
    async fn validate_new_session_accepts_create_and_send_shape() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "message": "hello",
                    "session_name": "Worker Session",
                    "agent_type": "DeepResearch",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_new_session_accepts_plan_todo_binding() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "message": "hello",
                    "session_name": "Worker Session",
                    "agent_type": "agentic",
                    "plan_file": "my_plan_1234.plan.md",
                    "todo_id": "setup-auth",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_new_session_rejects_plan_file_without_todo_id() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "message": "hello",
                    "session_name": "Worker Session",
                    "agent_type": "agentic",
                    "plan_file": "my_plan_1234.plan.md",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("plan_file and todo_id must be provided together")
        );
    }

    #[tokio::test]
    async fn validate_new_session_rejects_todo_id_without_plan_file() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "message": "hello",
                    "session_name": "Worker Session",
                    "agent_type": "agentic",
                    "todo_id": "setup-auth",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("plan_file and todo_id must be provided together")
        );
    }

    #[tokio::test]
    async fn validate_existing_session_rejects_plan_todo_binding() {
        let tool = SessionMessageTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": "C:/work",
                    "session_id": "worker_1",
                    "message": "hello",
                    "plan_file": "my_plan_1234.plan.md",
                    "todo_id": "setup-auth",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("plan_file/todo_id binding is only allowed when session_id is omitted")
        );
    }

    #[test]
    fn session_message_input_parses_plan_todo_binding() {
        let input: SessionMessageInput = serde_json::from_value(json!({
            "workspace": "C:/work",
            "message": "hello",
            "session_name": "Worker Session",
            "agent_type": "agentic",
            "plan_file": "my_plan_1234.plan.md",
            "todo_id": "setup-auth",
        }))
        .expect("payload with plan-todo binding must parse");

        assert_eq!(input.plan_file.as_deref(), Some("my_plan_1234.plan.md"));
        assert_eq!(input.todo_id.as_deref(), Some("setup-auth"));
    }

    #[tokio::test]
    async fn validate_existing_session_allows_missing_workspace() {
        let tool = SessionMessageTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "session_id": "worker_1",
                    "message": "hello",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_new_session_requires_workspace() {
        let tool = SessionMessageTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "message": "hello",
                    "session_name": "Worker Session",
                    "agent_type": "agentic",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("workspace is required when session_id is omitted")
        );
    }

    #[test]
    fn session_message_input_defaults_urgent_to_false_for_backward_compat() {
        let input: SessionMessageInput = serde_json::from_value(json!({
            "session_id": "worker_1",
            "message": "hello",
        }))
        .expect("legacy payload without urgent must parse");

        assert!(!input.urgent);
    }

    #[test]
    fn session_message_input_parses_urgent_flag() {
        let input: SessionMessageInput = serde_json::from_value(json!({
            "session_id": "worker_1",
            "message": "stop what you are doing and correct this",
            "urgent": true,
        }))
        .expect("payload with urgent must parse");

        assert!(input.urgent);
    }

    #[test]
    fn urgent_delivery_steers_into_a_processing_turn() {
        assert_eq!(
            resolve_urgent_delivery(Some("turn-7".to_string())),
            UrgentDelivery::Steer {
                turn_id: "turn-7".to_string()
            }
        );
    }

    #[test]
    fn urgent_delivery_falls_back_to_normal_submit_for_idle_session() {
        assert_eq!(resolve_urgent_delivery(None), UrgentDelivery::NormalSubmit);
    }

    #[test]
    fn urgent_message_to_existing_session_attempts_steering_channel() {
        assert!(should_attempt_steering(true, None, false));
    }

    #[test]
    fn urgent_message_to_new_session_uses_normal_channel_only() {
        assert!(!should_attempt_steering(true, Some("new-session-1"), false));
    }

    #[test]
    fn urgent_message_with_plan_todo_binding_uses_normal_channel_only() {
        // The steering channel carries no plan-todo binding metadata, so a
        // bound dispatch must fall back to the normal submission channel that
        // preserves the binding and the reply route (COORD-01).
        assert!(!should_attempt_steering(true, None, true));
        assert!(!should_attempt_steering(true, Some("new-session-1"), true));
    }

    #[test]
    fn non_urgent_message_never_attempts_steering_channel() {
        assert!(!should_attempt_steering(false, None, false));
        assert!(!should_attempt_steering(
            false,
            Some("new-session-1"),
            false
        ));
        assert!(!should_attempt_steering(false, None, true));
    }

    #[test]
    fn forwarded_reminder_includes_full_sender_identity() {
        let sender = SenderIdentity {
            session_id: "source-1".to_string(),
            role: Some("Commander".to_string()),
            depth: Some(0),
            name: Some("Assistant".to_string()),
        };
        let (message, reminders) =
            SessionMessageTool::new().format_forwarded_message("hello", &sender);
        assert_eq!(message, "hello");
        assert_eq!(reminders.len(), 1);
        let reminder = &reminders[0];
        assert_eq!(reminder.kind, "session_message_request");
        assert!(reminder.text.contains("[Commander L0]"));
        assert!(reminder.text.contains("Assistant"));
        assert!(reminder.text.contains("(session source-1)"));
        assert!(reminder.text.contains("not the human user"));
        assert!(reminder.text.contains("From session: source-1"));
        assert!(reminder.text.contains("From role: Commander"));
        assert!(reminder.text.contains("From depth: 0"));
        assert!(reminder.text.contains("From agent: Assistant"));
    }

    #[test]
    fn forwarded_reminder_falls_back_when_role_is_unregistered() {
        let sender = SenderIdentity {
            session_id: "source-2".to_string(),
            role: None,
            depth: Some(2),
            name: None,
        };
        let (_, reminders) = SessionMessageTool::new().format_forwarded_message("hello", &sender);
        let text = &reminders[0].text;
        assert!(text.contains("[Agent L2]"));
        assert!(text.contains("(session source-2)"));
        assert!(text.contains("From role: Agent"));
        assert!(text.contains("From depth: 2"));
        assert!(!text.contains("From agent:"));
    }

    #[test]
    fn forwarded_reminder_omits_depth_when_unknown() {
        let sender = SenderIdentity {
            session_id: "source-3".to_string(),
            role: Some("Executor".to_string()),
            depth: None,
            name: Some("Worker".to_string()),
        };
        let (_, reminders) = SessionMessageTool::new().format_forwarded_message("hello", &sender);
        assert!(reminders[0]
            .text
            .contains("[Executor] Worker (session source-3)"));
        assert!(!reminders[0].text.contains("From depth:"));
        assert!(reminders[0].text.contains("From agent: Worker"));
    }

    #[test]
    fn forwarded_reminder_always_identifies_session() {
        let sender = SenderIdentity {
            session_id: "source-4".to_string(),
            role: None,
            depth: None,
            name: None,
        };
        let (_, reminders) = SessionMessageTool::new().format_forwarded_message("hello", &sender);
        assert!(reminders[0].text.contains("[Agent] (session source-4)"));
        assert!(reminders[0].text.contains("From session: source-4"));
        assert!(reminders[0].text.contains("From role: Agent"));
        assert!(!reminders[0].text.contains("From depth:"));
        assert!(!reminders[0].text.contains("From agent:"));
    }

    #[test]
    fn session_message_input_parses_batch_items() {
        let input: SessionMessageInput = serde_json::from_value(json!({
            "workspace": "C:/work",
            "batch": [
                {
                    "session_name": "Worker One",
                    "message": "hello one",
                    "agent_type": "agentic"
                },
                {
                    "session_id": "worker_2",
                    "message": "hello two",
                    "urgent": true
                }
            ]
        }))
        .expect("payload with batch must parse");

        let batch = input.batch.expect("batch must be present");
        assert_eq!(batch.len(), 2);
        assert_eq!(batch[0].session_name.as_deref(), Some("Worker One"));
        assert_eq!(batch[0].message, "hello one");
        assert_eq!(
            batch[0].agent_type.as_ref().map(AgentType::as_str),
            Some("agentic")
        );
        assert!(batch[0].session_id.is_none());
        assert!(!batch[0].urgent);
        assert_eq!(batch[1].session_id.as_deref(), Some("worker_2"));
        assert!(batch[1].urgent);
        assert!(batch[1].session_name.is_none());
        assert!(batch[1].agent_type.is_none());
    }

    #[test]
    fn session_message_input_batch_defaults_to_none_for_backward_compat() {
        let input: SessionMessageInput = serde_json::from_value(json!({
            "session_id": "worker_1",
            "message": "hello",
        }))
        .expect("legacy payload without batch must parse");

        assert!(input.batch.is_none());
    }

    #[test]
    fn session_message_input_allows_omitting_top_level_message_for_batch() {
        let input: SessionMessageInput = serde_json::from_value(json!({
            "workspace": "C:/work",
            "batch": [
                {
                    "session_name": "Worker One",
                    "message": "hello",
                    "agent_type": "agentic"
                }
            ]
        }))
        .expect("batch payload without top-level message must parse");

        assert!(input.message.is_none());
        assert_eq!(
            input.batch.as_ref().expect("batch must be present").len(),
            1
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_empty_batch() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(validation.message.as_deref(), Some("batch cannot be empty"));
    }

    #[tokio::test]
    async fn validate_batch_rejects_top_level_message() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "message": "hello",
                    "batch": [
                        {
                            "session_name": "Worker One",
                            "message": "hello one",
                            "agent_type": "agentic"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("message cannot be combined with batch")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_top_level_session_fields() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "session_id": "worker_1",
                    "batch": [
                        {
                            "session_name": "Worker One",
                            "message": "hello one",
                            "agent_type": "agentic"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("session fields must be provided per batch item when batch is used")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_missing_workspace_for_create_item() {
        let tool = SessionMessageTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "batch": [
                        {
                            "session_name": "Worker One",
                            "message": "hello one",
                            "agent_type": "agentic"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("workspace is required when a batch item omits session_id")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_item_missing_session_name() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "message": "hello one",
                            "agent_type": "agentic"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("batch[0].session_name is required when session_id is omitted")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_item_missing_agent_type() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_name": "Worker One",
                            "message": "hello one"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("batch[0].agent_type is required when session_id is omitted")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_item_empty_message() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_name": "Worker One",
                            "message": "   ",
                            "agent_type": "agentic"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("batch[0].message cannot be empty")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_self_session_item() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_id": "source_1",
                            "message": "hello one"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("batch[0].session_id cannot send a message to the same session")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_item_plan_without_todo() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_name": "Worker One",
                            "message": "hello one",
                            "agent_type": "agentic",
                            "plan_file": "my_plan_1234.plan.md"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("batch[0].plan_file and batch[0].todo_id must be provided together")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_item_session_name_with_session_id() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_id": "worker_1",
                            "session_name": "Worker One",
                            "message": "hello one"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("batch[0].session_name is only allowed when session_id is omitted")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_item_agent_type_with_session_id() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_id": "worker_1",
                            "message": "hello one",
                            "agent_type": "agentic"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("batch[0].agent_type override is not allowed when session_id is provided")
        );
    }

    #[tokio::test]
    async fn validate_batch_rejects_item_plan_binding_with_session_id() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_id": "worker_1",
                            "message": "hello one",
                            "plan_file": "my_plan_1234.plan.md",
                            "todo_id": "setup-auth"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("batch[0].plan_file/todo_id binding is only allowed when session_id is omitted")
        );
    }

    #[tokio::test]
    async fn validate_batch_accepts_all_create_items() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_name": "Worker One",
                            "message": "hello one",
                            "agent_type": "agentic"
                        },
                        {
                            "session_name": "Worker Two",
                            "message": "hello two",
                            "agent_type": "Plan"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_batch_accepts_mixed_send_and_create_items() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_id": "worker_1",
                            "message": "hello existing"
                        },
                        {
                            "session_name": "Worker Two",
                            "message": "hello new",
                            "agent_type": "agentic"
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_batch_accepts_item_plan_todo_binding_and_urgent() {
        let tool = SessionMessageTool::new();
        let workspace = TestTempDir::new("bitfun-session-message-tool-test");

        let validation = tool
            .validate_input(
                &json!({
                    "workspace": workspace.as_string(),
                    "batch": [
                        {
                            "session_name": "Worker One",
                            "message": "hello one",
                            "agent_type": "agentic",
                            "plan_file": "my_plan_1234.plan.md",
                            "todo_id": "setup-auth"
                        },
                        {
                            "session_id": "worker_1",
                            "message": "urgent hello",
                            "urgent": true
                        }
                    ],
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[test]
    fn batch_summary_counts_success_and_failure() {
        let results = vec![
            json!({
                "status": "success",
                "target_session_id": "session-1",
                "created_session_id": "session-1",
            }),
            json!({
                "status": "error",
                "error": "session not found",
            }),
            json!({
                "status": "error",
                "error": "workspace mismatch",
            }),
        ];

        let (succeeded, failed, summary) = SessionMessageTool::summarize_batch_results(&results);

        assert_eq!(succeeded, 1);
        assert_eq!(failed, 2);
        assert!(summary.contains("3 message(s): 1 succeeded, 2 failed"));
        assert!(summary.contains("Successful items are not rolled back"));
        assert!(summary.contains("A failed item never rolls back earlier successes"));
    }

    #[test]
    fn batch_summary_omits_partial_failure_note_when_all_succeed() {
        let results = vec![
            json!({
                "status": "success",
                "target_session_id": "session-1",
            }),
            json!({
                "status": "success",
                "target_session_id": "session-2",
            }),
        ];

        let (succeeded, failed, summary) = SessionMessageTool::summarize_batch_results(&results);

        assert_eq!(succeeded, 2);
        assert_eq!(failed, 0);
        assert!(summary.contains("2 message(s): 2 succeeded, 0 failed"));
        assert!(!summary.contains("A failed item never rolls back"));
    }

    /// Minimal ACP port recording `send_message_to_bitfun_session` calls;
    /// the remaining trait methods are not exercised by these tests.
    #[derive(Debug, Default)]
    struct FakeAcpPort {
        bitfun_messages: Mutex<Vec<AcpClientBitfunMessageRequest>>,
        flow_messages: Mutex<Vec<AcpClientMessageRequest>>,
        fail_send: bool,
    }

    impl RuntimeServicePort for FakeAcpPort {
        fn capability(&self) -> RuntimeServiceCapability {
            RuntimeServiceCapability::AcpClient
        }
    }

    #[async_trait]
    impl AcpClientPort for FakeAcpPort {
        async fn create_session(
            &self,
            _request: bitfun_runtime_ports::AcpClientCreateRequest,
        ) -> PortResult<bitfun_runtime_ports::AcpClientCreateResult> {
            Err(PortError::new(
                PortErrorKind::Backend,
                "not exercised by the ACP direct-path tests",
            ))
        }

        async fn list_clients(&self) -> PortResult<bitfun_runtime_ports::AcpClientListResult> {
            Err(PortError::new(
                PortErrorKind::Backend,
                "not exercised by the ACP direct-path tests",
            ))
        }

        async fn release_session(
            &self,
            _request: bitfun_runtime_ports::AcpClientReleaseRequest,
        ) -> PortResult<()> {
            Err(PortError::new(
                PortErrorKind::Backend,
                "not exercised by the ACP direct-path tests",
            ))
        }

        async fn cancel_session(
            &self,
            _request: bitfun_runtime_ports::AcpClientCancelRequest,
        ) -> PortResult<()> {
            Err(PortError::new(
                PortErrorKind::Backend,
                "not exercised by the ACP direct-path tests",
            ))
        }

        async fn send_message(
            &self,
            request: bitfun_runtime_ports::AcpClientMessageRequest,
        ) -> PortResult<bitfun_runtime_ports::AcpClientMessageResult> {
            if self.fail_send {
                return Err(PortError::new(
                    PortErrorKind::Backend,
                    "simulated external agent failure",
                ));
            }
            self.flow_messages.lock().unwrap().push(request.clone());
            Ok(bitfun_runtime_ports::AcpClientMessageResult {
                session_id: request.session_id,
                response: "external response".to_string(),
            })
        }

        async fn send_message_stream(
            &self,
            request: bitfun_runtime_ports::AcpClientMessageRequest,
            chunk_sink: AcpClientStreamChunkSink,
        ) -> PortResult<bitfun_runtime_ports::AcpClientMessageResult> {
            if self.fail_send {
                return Err(PortError::new(
                    PortErrorKind::Backend,
                    "simulated external agent failure",
                ));
            }
            self.flow_messages.lock().unwrap().push(request.clone());
            let _ = chunk_sink.send(AcpClientStreamChunk::Text {
                text: "external response".to_string(),
            });
            let _ = chunk_sink.send(AcpClientStreamChunk::Completed);
            Ok(bitfun_runtime_ports::AcpClientMessageResult {
                session_id: request.session_id,
                response: "external response".to_string(),
            })
        }

        async fn send_message_to_bitfun_session(
            &self,
            request: AcpClientBitfunMessageRequest,
        ) -> PortResult<bitfun_runtime_ports::AcpClientMessageResult> {
            if self.fail_send {
                return Err(PortError::new(
                    PortErrorKind::Backend,
                    "simulated external agent failure",
                ));
            }
            self.bitfun_messages.lock().unwrap().push(request.clone());
            Ok(bitfun_runtime_ports::AcpClientMessageResult {
                session_id: request.bitfun_session_id,
                response: "external response".to_string(),
            })
        }

        async fn send_message_to_bitfun_session_stream(
            &self,
            request: AcpClientBitfunMessageRequest,
            chunk_sink: AcpClientStreamChunkSink,
        ) -> PortResult<bitfun_runtime_ports::AcpClientMessageResult> {
            if self.fail_send {
                return Err(PortError::new(
                    PortErrorKind::Backend,
                    "simulated external agent failure",
                ));
            }
            self.bitfun_messages.lock().unwrap().push(request.clone());
            let _ = chunk_sink.send(AcpClientStreamChunk::Text {
                text: "external response".to_string(),
            });
            let _ = chunk_sink.send(AcpClientStreamChunk::Completed);
            Ok(bitfun_runtime_ports::AcpClientMessageResult {
                session_id: request.bitfun_session_id,
                response: "external response".to_string(),
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
            _request: bitfun_runtime_ports::AcpClientHistoryRequest,
        ) -> PortResult<bitfun_runtime_ports::AcpClientHistoryResult> {
            Err(PortError::new(
                PortErrorKind::Backend,
                "not exercised by the ACP direct-path tests",
            ))
        }
    }

    /// Builds a real coordinator + scheduler harness so the async ACP direct
    /// delivery can be observed end to end (events + port forwarding). Mirrors
    /// the scheduler test harness.
    #[allow(clippy::type_complexity)]
    fn test_acp_delivery_harness() -> (
        Arc<ConversationCoordinator>,
        Arc<DialogScheduler>,
        Arc<SessionManager>,
        Arc<EventQueue>,
        tempfile::TempDir,
    ) {
        let root = tempfile::tempdir().expect("test root");
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let session_manager = Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(
                PersistenceManager::new(Arc::new(PathManager::with_user_root_for_tests(
                    root.path().join("user-root"),
                )))
                .expect("persistence manager"),
            ),
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: false,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ));
        let tool_pipeline = Arc::new(ToolPipeline::new(
            Arc::new(TokioRwLock::new(ToolRegistry::new())),
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
        let coordinator = Arc::new(ConversationCoordinator::new(
            session_manager.clone(),
            execution_engine,
            tool_pipeline,
            event_queue.clone(),
            Arc::new(EventRouter::new()),
            Arc::new(
                crate::runtime_ownership::CoreRuntimeOwnership::embedded_with_facts(
                    std::env::temp_dir().join(format!(
                        "bitfun-session-message-ownership-test-{}",
                        Uuid::new_v4()
                    )),
                    "bitfun".to_string(),
                    "test",
                ),
            ),
        ));
        let scheduler = DialogScheduler::new(coordinator.clone(), session_manager.clone());
        scheduler.set_agent_reply_archive_root(root.path().join("agent-replies"));
        (coordinator, scheduler, session_manager, event_queue, root)
    }

    #[test]
    fn acp_client_id_is_extracted_from_agent_type_prefix() {
        assert_eq!(
            SessionMessageTool::acp_client_id_from_agent_type("acp__codex"),
            Some("codex")
        );
        assert_eq!(
            SessionMessageTool::acp_client_id_from_agent_type("acp__Claude Code"),
            Some("Claude Code")
        );
        assert_eq!(
            SessionMessageTool::acp_client_id_from_agent_type("agentic"),
            None
        );
        assert_eq!(
            SessionMessageTool::acp_client_id_from_agent_type("Plan"),
            None
        );
        // A flow session id (acp_<client>_<uuid>) is not an agent type prefix.
        assert_eq!(
            SessionMessageTool::acp_client_id_from_agent_type("acp_codex_abc123"),
            None
        );
        assert_eq!(SessionMessageTool::acp_client_id_from_agent_type(""), None);
        // A bare prefix with no client id is rejected (empty client id).
        assert_eq!(
            SessionMessageTool::acp_client_id_from_agent_type("acp__"),
            None
        );
    }

    #[tokio::test]
    async fn acp_direct_send_forwards_through_bitfun_port() {
        let port = FakeAcpPort::default();
        let request = AcpClientBitfunMessageRequest {
            client_id: "codex".to_string(),
            bitfun_session_id: "session-internal-1".to_string(),
            message: "hello external agent".to_string(),
            workspace_path: Some("/repo/project".to_string()),
            timeout_seconds: Some(ACP_DIRECT_TIMEOUT_SECONDS),
        };
        let (chunk_tx, mut chunk_rx) = tokio::sync::mpsc::unbounded_channel();
        let response = SessionMessageTool::acp_direct_send_stream(
            &port,
            AcpDirectSendOp::Bitfun(request.clone()),
            chunk_tx,
        )
        .await
        .expect("direct path should succeed");

        let messages = port.bitfun_messages.lock().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].client_id, "codex");
        assert_eq!(messages[0].bitfun_session_id, "session-internal-1");
        assert_eq!(messages[0].message, "hello external agent");
        assert_eq!(messages[0].workspace_path.as_deref(), Some("/repo/project"));
        // The async direct path now carries a bounded window instead of the
        // old unbounded `None`.
        assert_eq!(
            messages[0].timeout_seconds,
            Some(ACP_DIRECT_TIMEOUT_SECONDS)
        );

        // The external response is returned verbatim, no re-translation.
        assert_eq!(response.response, "external response");
        // The response is also streamed as per-chunk text.
        let streamed = chunk_rx.try_recv().expect("streamed text chunk");
        assert!(matches!(
            streamed,
            AcpClientStreamChunk::Text { text } if text == "external response"
        ));
    }

    #[tokio::test]
    async fn acp_direct_send_propagates_port_failure() {
        let port = FakeAcpPort {
            fail_send: true,
            ..FakeAcpPort::default()
        };
        let (chunk_tx, _chunk_rx) = tokio::sync::mpsc::unbounded_channel();
        let error = SessionMessageTool::acp_direct_send_stream(
            &port,
            AcpDirectSendOp::Bitfun(AcpClientBitfunMessageRequest {
                client_id: "codex".to_string(),
                bitfun_session_id: "session-internal-1".to_string(),
                message: "hello".to_string(),
                workspace_path: None,
                timeout_seconds: Some(ACP_DIRECT_TIMEOUT_SECONDS),
            }),
            chunk_tx,
        )
        .await
        .unwrap_err();
        assert!(error.message.contains("simulated external agent failure"));
    }

    #[tokio::test]
    async fn acp_direct_delivery_streams_events_and_forwards_port_call() {
        let (coordinator, _scheduler, session_manager, event_queue, root) =
            test_acp_delivery_harness();
        let source_session_id = "source-session";
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(source_session_id.to_string()),
                "Source".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("create source session");

        let port = Arc::new(FakeAcpPort::default());
        let target_session_id = "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b".to_string();
        let mut event_rx = event_queue.subscribe();

        SessionMessageTool::spawn_acp_direct_delivery(
            port.clone(),
            AcpDirectSendOp::Flow(AcpClientMessageRequest {
                session_id: target_session_id.clone(),
                message: "hello external agent".to_string(),
                workspace_path: Some(workspace.to_string_lossy().into_owned()),
                timeout_seconds: Some(ACP_DIRECT_TIMEOUT_SECONDS),
            }),
            coordinator,
            _scheduler.clone(),
            target_session_id.clone(),
            "hello external agent".to_string(),
            AcpDirectReplySource {
                source_session_id: source_session_id.to_string(),
                source_workspace: workspace.to_string_lossy().into_owned(),
                source_remote_connection_id: None,
                source_remote_ssh_host: None,
            },
        );

        // The delivery runs in a background task; wait for the streamed turn
        // events and the port call (bounded timeout, not the old `None`).
        // Note: the follow-up reply back to the source session is not asserted
        // here because a model-less unit-test host cannot run the follow-up
        // turn; it is covered by `deliver_background_result`'s own tests.
        let mut saw_started = false;
        let mut saw_round_started = false;
        let mut saw_text = false;
        let mut saw_round_completed = false;
        let mut saw_completed = false;
        // "complete" 是前端 NORMAL_FINISH_REASONS 内的正常终止码，非标准方式结束
        // 横幅不会误报（参照 web-ui flow_chat/utils/turnCompletionNotice.ts）。
        let mut saw_complete_finish = false;
        for _ in 0..200 {
            while let Ok(envelope) = event_rx.try_recv() {
                match &envelope.event {
                    AgenticEvent::DialogTurnStarted { session_id, .. }
                        if session_id == &target_session_id =>
                    {
                        saw_started = true;
                    }
                    AgenticEvent::ModelRoundStarted { session_id, .. }
                        if session_id == &target_session_id =>
                    {
                        saw_round_started = true;
                    }
                    AgenticEvent::TextChunk {
                        session_id, text, ..
                    } if session_id == &target_session_id => {
                        saw_text = text == "external response";
                    }
                    AgenticEvent::ModelRoundCompleted { session_id, .. }
                        if session_id == &target_session_id =>
                    {
                        saw_round_completed = true;
                    }
                    AgenticEvent::DialogTurnCompleted {
                        session_id,
                        finish_reason,
                        ..
                    } if session_id == &target_session_id => {
                        saw_completed = true;
                        saw_complete_finish = finish_reason.as_deref() == Some("complete");
                    }
                    _ => {}
                }
            }
            let delivered = {
                let messages = port.flow_messages.lock().unwrap();
                saw_started
                    && saw_round_started
                    && saw_text
                    && saw_round_completed
                    && saw_completed
                    && saw_complete_finish
                    && messages.len() == 1
                    && messages[0].timeout_seconds == Some(ACP_DIRECT_TIMEOUT_SECONDS)
            };
            if delivered {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!(
            "ACP direct delivery did not stream turn events and forward the port call: saw_started={}, saw_round_started={}, saw_text={}, saw_round_completed={}, saw_completed={}, saw_complete_finish={}",
            saw_started, saw_round_started, saw_text, saw_round_completed, saw_completed, saw_complete_finish
        );
    }

    #[test]
    fn acp_direct_response_notice_excludes_full_response() {
        let full_reply = format!("EXTERNAL_REPLY_MARKER_{}", "x".repeat(4096));
        let notice = acp_direct_response_notice(&full_reply, "session-abc");
        assert!(!notice.contains("EXTERNAL_REPLY_MARKER_"));
        assert!(notice.contains("session-abc"));
        assert!(notice.contains("SessionHistory"));
    }

    #[test]
    fn acp_direct_delivery_workspace_path_extracts_from_ops() {
        assert_eq!(
            acp_direct_delivery_workspace_path(&AcpDirectSendOp::Flow(AcpClientMessageRequest {
                session_id: "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b".to_string(),
                message: "m".to_string(),
                workspace_path: Some("/repo/project".to_string()),
                timeout_seconds: None,
            })),
            Some("/repo/project")
        );
        assert_eq!(
            acp_direct_delivery_workspace_path(&AcpDirectSendOp::Bitfun(
                AcpClientBitfunMessageRequest {
                    client_id: "codex".to_string(),
                    bitfun_session_id: "session-internal-1".to_string(),
                    message: "m".to_string(),
                    workspace_path: None,
                    timeout_seconds: None,
                },
            )),
            None
        );
    }

    #[test]
    fn build_acp_direct_delivery_turn_maps_response_and_status() {
        let turn = build_acp_direct_delivery_turn(
            "turn-1",
            3,
            "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b",
            "hello",
            "round-1",
            1000,
            "external response",
            crate::service::session::TurnStatus::Completed,
            None,
        );
        assert_eq!(turn.turn_index, 3);
        assert_eq!(turn.user_message.content, "hello");
        assert_eq!(turn.model_rounds.len(), 1);
        assert_eq!(turn.model_rounds[0].round_index, 0);
        assert_eq!(turn.model_rounds[0].text_items.len(), 1);
        assert_eq!(
            turn.model_rounds[0].text_items[0].content,
            "external response"
        );
        assert_eq!(turn.status, crate::service::session::TurnStatus::Completed);
        assert!(turn.end_time.is_some());
        assert!(turn.error.is_none());

        // 失败 turn：status=Error + error 字段，空回复不产生文本项。
        let failed = build_acp_direct_delivery_turn(
            "turn-2",
            4,
            "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b",
            "hello",
            "round-2",
            2000,
            "",
            crate::service::session::TurnStatus::Error,
            Some("boom".to_string()),
        );
        assert_eq!(failed.status, crate::service::session::TurnStatus::Error);
        assert_eq!(failed.error.as_deref(), Some("boom"));
        assert!(failed.model_rounds[0].text_items.is_empty());
    }

    #[tokio::test]
    async fn acp_direct_delivery_appends_full_reply_even_when_index_occupied() {
        // 防回退（P-19 全文落盘原则）：acp 流会话投递 turn 的 reply 全文必须可经
        // SessionHistory 检索。当 metadata.turn_count 落后（既有 turn 已落盘但元数据
        // 未同步，如前端/并发写者在同一索引先落盘——正是「SessionHistory 导出仍只有
        // turn 0」的实证场景）时，投递 turn 不得在计算索引处与既有 turn 冲突即静默
        // 丢弃，必须追加到下一空闲索引，保证全文不丢。
        use crate::service::session::SessionMetadata;

        let root = tempfile::tempdir().expect("test root");
        let persistence = PersistenceManager::new(Arc::new(PathManager::with_user_root_for_tests(
            root.path().join("user-root"),
        )))
        .expect("persistence manager");
        let storage_path = root.path().join("storage");
        let session_id = "acp_codebuddy_a4f68de7-c4ec-46a8-9aab-7e2bc417c3d0".to_string();
        let metadata = SessionMetadata::new(
            session_id.clone(),
            "codebuddy ACP".to_string(),
            "acp:codebuddy".to_string(),
            "auto".to_string(),
        );
        persistence
            .create_session_metadata_if_absent(&storage_path, &metadata)
            .await
            .expect("metadata should be created");

        // 模拟前端/并发写者已落盘 turn 0（index 0 被占用）。
        persist_acp_direct_delivery_turn(
            &persistence,
            &storage_path,
            &session_id,
            "frontend-turn-0",
            "initial user input",
            "round-0",
            100,
            "pre-existing content",
            crate::service::session::TurnStatus::Completed,
            None,
        )
        .await;
        // 再模拟 metadata.turn_count 落后：落盘后置回 0（前端写者未同步元数据）。
        persistence
            .update_session_metadata(&storage_path, &session_id, |stale| {
                stale.turn_count = 0;
            })
            .await
            .expect("metadata should update");

        // 后端投递（存活测试）：reply 全文为 'alive'，不得因 index=0 冲突而丢弃。
        persist_acp_direct_delivery_turn(
            &persistence,
            &storage_path,
            &session_id,
            "turn-alive",
            "【acp 会话存活测试】只回『alive』",
            "round-1",
            2000,
            "alive",
            crate::service::session::TurnStatus::Completed,
            None,
        )
        .await;

        // 全文必须追加到下一空闲索引（1）并完整可检索（SessionHistory 导出依据）。
        let saved = persistence
            .load_dialog_turn(&storage_path, &session_id, 1)
            .await
            .expect("load should succeed")
            .expect("delivery turn should be persisted, not dropped");
        assert_eq!(
            saved.user_message.content,
            "【acp 会话存活测试】只回『alive』"
        );
        assert_eq!(saved.model_rounds[0].text_items[0].content, "alive");
        assert_eq!(saved.status, crate::service::session::TurnStatus::Completed);
    }

    #[tokio::test]
    async fn persist_acp_direct_delivery_turn_writes_turn_file() {
        use crate::service::session::SessionMetadata;

        let root = tempfile::tempdir().expect("test root");
        let persistence = PersistenceManager::new(Arc::new(PathManager::with_user_root_for_tests(
            root.path().join("user-root"),
        )))
        .expect("persistence manager");
        let storage_path = root.path().join("storage");
        let session_id = "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b".to_string();
        let metadata = SessionMetadata::new(
            session_id.clone(),
            "Codex ACP".to_string(),
            "acp:codex".to_string(),
            "auto".to_string(),
        );
        persistence
            .create_session_metadata_if_absent(&storage_path, &metadata)
            .await
            .expect("metadata should be created");

        persist_acp_direct_delivery_turn(
            &persistence,
            &storage_path,
            &session_id,
            "turn-1",
            "hello",
            "round-1",
            1000,
            "external response",
            crate::service::session::TurnStatus::Completed,
            None,
        )
        .await;

        let saved = persistence
            .load_dialog_turn(&storage_path, &session_id, 0)
            .await
            .expect("load should succeed")
            .expect("turn should be persisted");
        assert_eq!(saved.turn_id, "turn-1");
        assert_eq!(saved.user_message.content, "hello");
        assert_eq!(
            saved.model_rounds[0].text_items[0].content,
            "external response"
        );
        assert_eq!(saved.status, crate::service::session::TurnStatus::Completed);

        // 幂等：同 turn 再次落盘为 no-op（不覆盖已保存内容、不报错）。
        persist_acp_direct_delivery_turn(
            &persistence,
            &storage_path,
            &session_id,
            "turn-1",
            "hello",
            "round-2",
            2000,
            "overwrite attempt",
            crate::service::session::TurnStatus::Completed,
            None,
        )
        .await;
        let saved_again = persistence
            .load_dialog_turn(&storage_path, &session_id, 0)
            .await
            .expect("load should succeed")
            .expect("turn should still exist");
        assert_eq!(
            saved_again.model_rounds[0].text_items[0].content,
            "external response"
        );
    }

    // PR #2139 #5: delivery authorization gate (dispatch_single local delivery
    // to an existing session). Reuses the R4 shared verdict
    // resolve_session_mutation_authorization (daemon interception ->
    // owner exemption -> created_by match -> ancestor traversal), with option
    // deliver(): owner exemption + no ghost ACP allowance.
    // ---------------------------------------------------------------------

    fn delivery_authz_session_manager() -> Arc<SessionManager> {
        Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(
                PersistenceManager::new(Arc::new(PathManager::with_user_root_for_tests(
                    std::env::temp_dir()
                        .join(format!("bitfun-session-message-authz-{}", Uuid::new_v4())),
                )))
                .expect("persistence manager"),
            ),
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
    async fn delivery_authz_rejects_unrelated_caller_without_metadata() {
        // Not owner, target has no created_by, no ancestor relationship
        // -> reject (consistent with delete semantics).
        let session_manager = delivery_authz_session_manager();
        let tree = bitfun_services_core::session::tree::SessionTreeManager::new(8);
        let workspace = TestTempDir::new("bitfun-delivery-authz-unrelated");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);

        let error = resolve_session_mutation_authorization(
            &session_manager,
            &tree,
            "caller-1",
            "target-1",
            workspace_path,
            "deliver to",
            SessionMutationAuthOptions::deliver(),
        )
        .await
        .expect_err("unrelated caller without metadata must be rejected");
        assert!(
            error.to_string().contains("not authorized to deliver to")
                || error
                    .to_string()
                    .contains("cannot verify ancestor relationship"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn delivery_authz_created_by_match_allows_caller() {
        // created_by match: target metadata created_by == session-<caller>
        // -> allow.
        let session_manager = delivery_authz_session_manager();
        let tree = bitfun_services_core::session::tree::SessionTreeManager::new(8);
        let workspace = TestTempDir::new("bitfun-delivery-authz-created-by");
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
        created_metadata.created_by = Some("session-caller-1".to_string());
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
            "deliver to",
            SessionMutationAuthOptions::deliver(),
        )
        .await
        .expect("creator should be authorized to deliver");
    }

    #[tokio::test]
    async fn delivery_authz_ancestor_allows_caller() {
        // Ancestor authorization: caller is an ancestor of the target (tree
        // registered child relationship) -> allow.
        let session_manager = delivery_authz_session_manager();
        let tree = bitfun_services_core::session::tree::SessionTreeManager::new(8);
        tree.register_child("caller-1", "child-1", 1)
            .expect("register child");
        let workspace = TestTempDir::new("bitfun-delivery-authz-ancestor");
        let workspace_string = workspace.as_string();
        let workspace_path = std::path::Path::new(&workspace_string);

        resolve_session_mutation_authorization(
            &session_manager,
            &tree,
            "caller-1",
            "child-1",
            workspace_path,
            "deliver to",
            SessionMutationAuthOptions::deliver(),
        )
        .await
        .expect("ancestor should be authorized to deliver");
    }
}
