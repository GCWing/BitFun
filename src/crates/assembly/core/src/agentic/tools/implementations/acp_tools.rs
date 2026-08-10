//! Dedicated ACP tool family (real external process channel).
//!
//! These tools mirror SessionControl / SessionMessage / SessionHistory but
//! drive the true ACP bridge: every call forwards to the external ACP client
//! process through the coordinator-injected `AcpClientPort` (implemented by
//! the desktop host over `AcpClientService`). Core never depends on the ACP
//! crate; the port is the architecture boundary.
//!
//! - `acp_control`: create / list / delete / cancel real external ACP sessions.
//! - `acp_message`: forward one message through the real channel and return
//!   the external agent's response synchronously.
//! - `acp_history`: read the persisted transcript of an ACP session.

use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::implementations::session_control_tool::{
    resolve_session_mutation_authorization, SessionMutationAuthOptions,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_runtime_ports::{
    AcpClientCancelRequest, AcpClientCreateRequest, AcpClientHistoryRequest, AcpClientMessageRequest,
    AcpClientPort,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

/// `acp_control` input.
///
/// Field names are snake_case on the wire, matching the tool `input_schema`
/// and the SessionControl/SessionMessage input contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AcpControlInput {
    pub action: String,
    pub client_id: Option<String>,
    pub workspace_path: Option<String>,
    pub session_name: Option<String>,
    pub session_id: Option<String>,
}

/// `acp_message` input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AcpMessageInput {
    pub session_id: String,
    pub message: String,
    pub workspace_path: Option<String>,
    pub timeout_seconds: Option<u64>,
}

/// `acp_history` input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AcpHistoryInput {
    pub session_id: String,
    pub workspace_path: Option<String>,
}

/// Resolve the ACP client port injected by the desktop host.
fn resolve_acp_client_port() -> BitFunResult<Arc<dyn AcpClientPort>> {
    let coordinator = get_global_coordinator()
        .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;
    coordinator.acp_client_port().ok_or_else(|| {
        BitFunError::tool(
            "ACP client port is not available; the desktop host did not inject it".to_string(),
        )
    })
}

/// Map a port-level failure to a tool error with its kind surfaced.
fn port_error(error: bitfun_runtime_ports::PortError) -> BitFunError {
    BitFunError::tool(format!(
        "ACP client port failed ({:?}): {}",
        error.kind, error.message
    ))
}

fn required_session_id(value: Option<&str>, action: &str) -> BitFunResult<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            BitFunError::tool(format!("session_id is required for {}", action))
        })
}

fn workspace_or_context(
    workspace_param: Option<&str>,
    context: &ToolUseContext,
) -> BitFunResult<String> {
    if let Some(workspace) = workspace_param
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(workspace.to_string());
    }
    context
        .workspace_root()
        .map(|path| path.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            BitFunError::tool(
                "workspace_path is required when the current workspace is unavailable".to_string(),
            )
        })
}

/// 授权门（PR #2139 R4）：触碰外部 ACP port 前，acp_control delete/cancel 复用
/// SessionControl 的共享授权决策链（daemon/warden 拦截 + owner/created_by +
/// 幽灵 ACP 流会话 + 祖先遍历）。无全局 coordinator 或无 caller session 时
/// 保守拒绝。
async fn authorize_acp_session_mutation(
    context: &ToolUseContext,
    workspace_path: &str,
    session_id: &str,
    action_label: &str,
    options: SessionMutationAuthOptions,
) -> BitFunResult<()> {
    let caller_session_id = context.session_id.as_ref().ok_or_else(|| {
        BitFunError::tool(format!(
            "cannot {action_label} an ACP session without a caller session in tool context"
        ))
    })?;
    let coordinator = get_global_coordinator()
        .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;
    resolve_session_mutation_authorization(
        coordinator.get_session_manager(),
        coordinator.session_tree(),
        caller_session_id,
        session_id,
        std::path::Path::new(workspace_path),
        action_label,
        options,
    )
    .await
}

/// Execute one `acp_control` action against the real ACP port.
pub(crate) async fn run_acp_control(
    port: &dyn AcpClientPort,
    input: &Value,
    context: &ToolUseContext,
) -> BitFunResult<Vec<ToolResult>> {
    let params: AcpControlInput = serde_json::from_value(input.clone())
        .map_err(|error| BitFunError::tool(format!("Invalid input: {}", error)))?;

    match params.action.as_str() {
        "create" => {
            let client_id = params
                .client_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| BitFunError::tool("client_id is required for create".to_string()))?
                .to_string();
            // d3-P2-3：create 会启动外部 ACP 进程，必须封堵模型任意指定工作
            // 目录的注入面。显式 workspace_path 只允许指向当前会话已注册的
            // 工作区（root 或 project root）；否则拒绝，杜绝"模型把外部进程
            // spawn 到任意目录"的路径。
            let workspace_path = match params.workspace_path.as_deref() {
                Some(explicit) => {
                    let explicit = explicit.trim();
                    let allowed = context
                        .workspace_root()
                        .map(|path| path.to_string_lossy())
                        .into_iter()
                        .chain(
                            context
                                .project_workspace_root()
                                .map(|path| path.to_string_lossy()),
                        )
                        .any(|path| path == explicit);
                    if !allowed {
                        return Err(BitFunError::tool(format!(
                            "workspace_path '{}' is not the current session workspace; external ACP processes can only be started in the registered session workspace (injection guard, d3-P2-3)",
                            explicit
                        )));
                    }
                    explicit.to_string()
                }
                None => workspace_or_context(params.workspace_path.as_deref(), context)?,
            };
            // d3-P2-3：readonly 客户端不允许模型启动外部 ACP 会话进程。
            // readonly 是管理员配置的"该客户端仅可读"标志，模型不可绕过。
            let listed = port.list_clients().await.map_err(port_error)?;
            if listed
                .clients
                .iter()
                .any(|client| client.client_id == client_id && client.readonly)
            {
                return Err(BitFunError::tool(format!(
                    "ACP client '{}' is configured as readonly; it cannot be started by the model (readonly guard, d3-P2-3)",
                    client_id
                )));
            }
            let created_workspace = workspace_path.clone();
            let created = port
                .create_session(AcpClientCreateRequest {
                    client_id,
                    workspace_path,
                    session_name: params.session_name,
                    remote_connection_id: None,
                })
                .await
                .map_err(port_error)?;
            let result_for_assistant = format!(
                "Started external ACP session '{}' (agent '{}') for workspace '{}'.",
                created.session_name, created.agent_type, created_workspace
            );
            Ok(vec![ToolResult::Result {
                data: json!({
                    "success": true,
                    "action": "create",
                    "session": {
                        "session_id": created.session_id,
                        "session_name": created.session_name,
                        "agent_type": created.agent_type,
                    }
                }),
                result_for_assistant: Some(result_for_assistant),
                image_attachments: None,
            }])
        }
        "list" => {
            let listed = port.list_clients().await.map_err(port_error)?;
            let result_for_assistant = if listed.clients.is_empty() {
                "No ACP clients are registered.".to_string()
            } else {
                format!("Found {} ACP client(s):", listed.clients.len())
            };
            let clients = listed
                .clients
                .iter()
                .map(|client| {
                    json!({
                        "client_id": client.client_id,
                        "name": client.name,
                        "status": client.status,
                        "session_count": client.session_count,
                        "readonly": client.readonly,
                    })
                })
                .collect::<Vec<_>>();
            Ok(vec![ToolResult::Result {
                data: json!({
                    "success": true,
                    "action": "list",
                    "count": listed.clients.len(),
                    "clients": clients,
                }),
                result_for_assistant: Some(result_for_assistant),
                image_attachments: None,
            }])
        }
        "delete" => {
            let session_id = required_session_id(params.session_id.as_deref(), "delete")?;
            let workspace_path = workspace_or_context(params.workspace_path.as_deref(), context)?;
            // R4 授权门：未授权（非 owner/creator/ancestor，或非幽灵 ACP 流会话）
            // 时拒绝删除，与 SessionControl delete 共享同一决策链。
            authorize_acp_session_mutation(
                context,
                &workspace_path,
                &session_id,
                "delete",
                SessionMutationAuthOptions::delete(),
            )
            .await?;
            // 删除持久化流会话记录并释放外部进程：两个效果都需要，否则只剩
            // release 会留下孤儿记录（已回收会话仍出现在列表里）。
            port.delete_session_record(session_id.clone(), Some(workspace_path))
                .await
                .map_err(port_error)?;
            Ok(vec![ToolResult::Result {
                data: json!({
                    "success": true,
                    "action": "delete",
                    "session_id": session_id,
                }),
                result_for_assistant: Some(format!(
                    "Deleted external ACP session '{}'.",
                    session_id
                )),
                image_attachments: None,
            }])
        }
        "cancel" => {
            let session_id = required_session_id(params.session_id.as_deref(), "cancel")?;
            let workspace_path = workspace_or_context(params.workspace_path.as_deref(), context)?;
            // R4 授权门：cancel 沿用 delete 的共享决策链；幽灵 ACP 流会话
            // （created_by 空是其设计形态）在 delete 语义下允许，cancel 同样允许
            // （流会话按设计无 created_by）。
            authorize_acp_session_mutation(
                context,
                &workspace_path,
                &session_id,
                "cancel",
                SessionMutationAuthOptions::delete(),
            )
            .await?;
            port.cancel_session(AcpClientCancelRequest {
                session_id: session_id.clone(),
            })
            .await
            .map_err(port_error)?;
            Ok(vec![ToolResult::Result {
                data: json!({
                    "success": true,
                    "action": "cancel",
                    "session_id": session_id,
                }),
                result_for_assistant: Some(format!(
                    "Cancelled the running turn of external ACP session '{}'.",
                    session_id
                )),
                image_attachments: None,
            }])
        }
        other => Err(BitFunError::tool(format!(
            "unknown acp_control action '{}'; expected one of create, list, delete, cancel",
            other
        ))),
    }
}

/// Execute one `acp_message` forward through the real channel.
pub(crate) async fn run_acp_message(
    port: &dyn AcpClientPort,
    input: &Value,
    context: &ToolUseContext,
) -> BitFunResult<Vec<ToolResult>> {
    let params: AcpMessageInput = serde_json::from_value(input.clone())
        .map_err(|error| BitFunError::tool(format!("Invalid input: {}", error)))?;
    let session_id = required_session_id(Some(&params.session_id), "message")?;
    let message = params
        .message
        .trim()
        .to_string();
    if message.is_empty() {
        return Err(BitFunError::tool("message is required".to_string()));
    }
    // d3-P2-6：缺省语义与 create/delete 统一——workspace_path 缺失时强制
    // 回退到当前会话工作区（workspace_or_context），不再传 None。此前传 None
    // 导致 send_message 跳过 session_storage_path → 不持久化 acpRemoteSessionId，
    // 断连后无法 Load/Resume，只能 New 重建（远程续接能力降级）。
    let workspace_path = workspace_or_context(params.workspace_path.as_deref(), context)?;
    let sent = port
        .send_message(AcpClientMessageRequest {
            session_id: session_id.clone(),
            message,
            workspace_path: Some(workspace_path),
            timeout_seconds: params.timeout_seconds,
        })
        .await
        .map_err(port_error)?;
    // 方向 C（并列返回面）：result_for_assistant 只内嵌极简通知句（对齐
    // task/execution.rs acp_send_input_notice 语义），不内嵌 sent.response 全文；
    // 全文留在 data JSON 的 response 字段，父会话按需取 data / SessionHistory。
    let result_for_assistant = if sent.response.trim().is_empty() {
        format!("External ACP session '{}' returned an empty response.", session_id)
    } else {
        format!(
            "External ACP session '{}' responded; use SessionHistory to view the full reply.",
            session_id
        )
    };
    Ok(vec![ToolResult::Result {
        data: json!({
            "success": true,
            "session_id": sent.session_id,
            "response": sent.response,
        }),
        result_for_assistant: Some(result_for_assistant),
        image_attachments: None,
    }])
}

/// Execute one `acp_history` transcript read.
pub(crate) async fn run_acp_history(
    port: &dyn AcpClientPort,
    input: &Value,
    context: &ToolUseContext,
) -> BitFunResult<Vec<ToolResult>> {
    let params: AcpHistoryInput = serde_json::from_value(input.clone())
        .map_err(|error| BitFunError::tool(format!("Invalid input: {}", error)))?;
    let session_id = required_session_id(Some(&params.session_id), "history")?;
    // d3-P2-6：缺省语义与 create/delete 统一（同 acp_message）——强制回退
    // 到当前会话工作区，保证远程续接（Load/Resume）能力不降级。
    let workspace_path = workspace_or_context(params.workspace_path.as_deref(), context)?;
    let read = port
        .read_history(AcpClientHistoryRequest {
            session_id: session_id.clone(),
            workspace_path: Some(workspace_path),
        })
        .await
        .map_err(port_error)?;
    let result_for_assistant = format!(
        "Session '{}' has {} transcript entr{}.",
        session_id,
        read.entries.len(),
        if read.entries.len() == 1 { "y" } else { "ies" }
    );
    Ok(vec![ToolResult::Result {
        data: json!({
            "success": true,
            "session_id": read.session_id,
            "count": read.entries.len(),
            "truncated": read.truncated,
            "entries": read.entries,
        }),
        result_for_assistant: Some(result_for_assistant),
        image_attachments: None,
    }])
}

/// `acp_control` tool - create, list, delete, or cancel real external ACP sessions.
pub struct AcpControlTool;

impl Default for AcpControlTool {
    fn default() -> Self {
        Self::new()
    }
}

impl AcpControlTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for AcpControlTool {
    fn name(&self) -> &str {
        "acp_control"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Manage real external ACP agent sessions (true bridge: every action drives the external ACP client process, never a local model).

Actions:
- "create": Start an external ACP client process for a client_id (for example "codex" or "claude-code") bound to a persisted session in the given workspace. Requires client_id and workspace_path.
- "list": List registered ACP clients with their runtime status and session counts.
- "delete": Delete an external ACP session: release the external process bound to a session_id created by this tool or acp_control create, and remove its persisted record so it stops appearing in listings.
- "cancel": Cancel the currently running dialog turn of the external ACP session.

Related tools:
- Use acp_message to send a message to an external ACP session (synchronous real-channel response).
- Use acp_history to read the persisted transcript of an ACP session.

Arguments:
- "action": Required. One of "create", "list", "delete", "cancel".
- "client_id": Required for create. Registered ACP client id.
- "workspace_path": Optional absolute workspace path; defaults to the current workspace when omitted. Used by create and delete.
- "session_name": Optional display name; only used by create.
- "session_id": Required for delete and cancel."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Create, list, delete, and cancel real external ACP agent sessions.".to_string()
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
                    "enum": ["create", "list", "delete", "cancel"],
                    "description": "The ACP session action to perform."
                },
                "client_id": {
                    "type": "string",
                    "description": "Required for create. Registered ACP client id."
                },
                "workspace_path": {
                    "type": "string",
                    "description": "Optional absolute workspace path for create and delete; defaults to the current workspace when omitted."
                },
                "session_name": {
                    "type": "string",
                    "description": "Optional display name when creating a session."
                },
                "session_id": {
                    "type": "string",
                    "description": "Required for delete and cancel."
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let parsed: AcpControlInput = match serde_json::from_value(input.clone()) {
            Ok(value) => value,
            Err(error) => {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Invalid input: {}", error)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };
        let mut message = None;
        let mut result = true;
        match parsed.action.as_str() {
            "create" => {
                if parsed
                    .client_id
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default()
                    .is_empty()
                {
                    result = false;
                    message = Some("client_id is required for create".to_string());
                }
            }
            "delete" | "cancel" => {
                if parsed
                    .session_id
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default()
                    .is_empty()
                {
                    result = false;
                    message = Some(format!(
                        "session_id is required for {}",
                        parsed.action
                    ));
                }
            }
            "list" => {}
            other => {
                result = false;
                message = Some(format!(
                    "unknown acp_control action '{}'; expected one of create, list, delete, cancel",
                    other
                ));
            }
        }
        ValidationResult {
            result,
            message,
            error_code: if result { None } else { Some(400) },
            meta: None,
        }
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let action = input
            .get("action")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        match action {
            "create" => {
                let client_id = input
                    .get("client_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown");
                format!("Start external ACP session for client '{}'", client_id)
            }
            "delete" => {
                let session_id = input
                    .get("session_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown");
                format!("Delete external ACP session '{}'", session_id)
            }
            "cancel" => {
                let session_id = input
                    .get("session_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown");
                format!("Cancel external ACP session '{}'", session_id)
            }
            _ => "List external ACP clients".to_string(),
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let port = resolve_acp_client_port()?;
        run_acp_control(port.as_ref(), input, context).await
    }
}

/// `acp_message` tool - forward one message through the real ACP channel.
pub struct AcpMessageTool;

impl Default for AcpMessageTool {
    fn default() -> Self {
        Self::new()
    }
}

impl AcpMessageTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for AcpMessageTool {
    fn name(&self) -> &str {
        "acp_message"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Send a message to an existing external ACP agent session and synchronously return the external agent's response.

This is the true bridge path: the message is forwarded to the real external ACP client process (for example Codex or Claude Code) and the response text comes back from that process, not from a local model.

Related tools:
- Use acp_control create to start an external ACP session, then acp_message to talk to it.
- Use acp_history to read the persisted transcript.

Arguments:
- "session_id": Required. The ACP session id returned by acp_control create.
- "message": Required. The prompt to forward to the external agent.
- "workspace_path": Optional absolute workspace path; defaults to the current workspace when omitted.
- "timeout_seconds": Optional timeout for the external agent turn; omitted means the host default."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Send a message to a real external ACP agent session and return its response.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "The ACP session id returned by acp_control create."
                },
                "message": {
                    "type": "string",
                    "description": "The prompt to forward to the external agent."
                },
                "workspace_path": {
                    "type": "string",
                    "description": "Optional absolute workspace path; defaults to the current workspace."
                },
                "timeout_seconds": {
                    "type": "integer",
                    "description": "Optional timeout for the external agent turn."
                }
            },
            "required": ["session_id", "message"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let parsed: AcpMessageInput = match serde_json::from_value(input.clone()) {
            Ok(value) => value,
            Err(error) => {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Invalid input: {}", error)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };
        let mut result = true;
        let mut message = None;
        if parsed.session_id.trim().is_empty() {
            result = false;
            message = Some("session_id is required".to_string());
        } else if parsed.message.trim().is_empty() {
            result = false;
            message = Some("message is required".to_string());
        }
        ValidationResult {
            result,
            message,
            error_code: if result { None } else { Some(400) },
            meta: None,
        }
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let session_id = input
            .get("session_id")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        format!("Send message to external ACP session '{}'", session_id)
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let port = resolve_acp_client_port()?;
        run_acp_message(port.as_ref(), input, context).await
    }
}

/// `acp_history` tool - read the persisted transcript of an ACP session.
pub struct AcpHistoryTool;

impl Default for AcpHistoryTool {
    fn default() -> Self {
        Self::new()
    }
}

impl AcpHistoryTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for AcpHistoryTool {
    fn name(&self) -> &str {
        "acp_history"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Read the persisted transcript of an external ACP agent session.

Returns the same turn history the external ACP process replays on restore, so the transcript reflects the real external conversation.

Related tools:
- Use acp_control create to start an external ACP session.
- Use acp_message to continue the conversation.

Arguments:
- "session_id": Required. The ACP session id returned by acp_control create.
- "workspace_path": Optional absolute workspace path; defaults to the current workspace when omitted."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Read the persisted transcript of an external ACP agent session.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "The ACP session id returned by acp_control create."
                },
                "workspace_path": {
                    "type": "string",
                    "description": "Optional absolute workspace path; defaults to the current workspace."
                }
            },
            "required": ["session_id"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let parsed: AcpHistoryInput = match serde_json::from_value(input.clone()) {
            Ok(value) => value,
            Err(error) => {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Invalid input: {}", error)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };
        let result = !parsed.session_id.trim().is_empty();
        ValidationResult {
            result,
            message: if result {
                None
            } else {
                Some("session_id is required".to_string())
            },
            error_code: if result { None } else { Some(400) },
            meta: None,
        }
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let session_id = input
            .get("session_id")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        format!("Read transcript of external ACP session '{}'", session_id)
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let port = resolve_acp_client_port()?;
        run_acp_history(port.as_ref(), input, context).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_runtime_ports::{
        AcpClientBitfunMessageRequest, AcpClientCreateResult, AcpClientHistoryEntry,
        AcpClientHistoryResult, AcpClientListResult, AcpClientMessageResult,
        AcpClientReleaseRequest, AcpClientStreamChunk, AcpClientStreamChunkSink, AcpClientSummary,
        PortResult, RuntimeServiceCapability, RuntimeServicePort,
    };
    use std::sync::Mutex;

    #[derive(Debug, Default)]
    struct FakeAcpClientPort {
        created: Mutex<Vec<AcpClientCreateRequest>>,
        listed: Mutex<usize>,
        released: Mutex<Vec<String>>,
        deleted: Mutex<Vec<String>>,
        cancelled: Mutex<Vec<String>>,
        messages: Mutex<Vec<AcpClientMessageRequest>>,
        bitfun_messages: Mutex<Vec<AcpClientBitfunMessageRequest>>,
        histories: Mutex<Vec<AcpClientHistoryRequest>>,
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
            *self.listed.lock().unwrap() += 1;
            Ok(AcpClientListResult {
                clients: vec![AcpClientSummary {
                    client_id: "codex".to_string(),
                    name: "Codex".to_string(),
                    status: "running".to_string(),
                    session_count: 1,
                    readonly: false,
                }],
            })
        }

        async fn release_session(&self, request: AcpClientReleaseRequest) -> PortResult<()> {
            self.released.lock().unwrap().push(request.session_id);
            Ok(())
        }

        async fn cancel_session(&self, request: AcpClientCancelRequest) -> PortResult<()> {
            self.cancelled.lock().unwrap().push(request.session_id);
            Ok(())
        }

        async fn send_message(
            &self,
            request: AcpClientMessageRequest,
        ) -> PortResult<AcpClientMessageResult> {
            self.messages.lock().unwrap().push(request.clone());
            Ok(AcpClientMessageResult {
                session_id: request.session_id,
                response: "external response".to_string(),
            })
        }

        async fn send_message_stream(
            &self,
            request: AcpClientMessageRequest,
            chunk_sink: AcpClientStreamChunkSink,
        ) -> PortResult<AcpClientMessageResult> {
            self.messages.lock().unwrap().push(request.clone());
            let _ = chunk_sink.send(AcpClientStreamChunk::Text {
                text: "external response".to_string(),
            });
            let _ = chunk_sink.send(AcpClientStreamChunk::Completed);
            Ok(AcpClientMessageResult {
                session_id: request.session_id,
                response: "external response".to_string(),
            })
        }

        async fn send_message_to_bitfun_session(
            &self,
            request: AcpClientBitfunMessageRequest,
        ) -> PortResult<AcpClientMessageResult> {
            self.bitfun_messages.lock().unwrap().push(request.clone());
            Ok(AcpClientMessageResult {
                session_id: request.bitfun_session_id,
                response: "external response".to_string(),
            })
        }

        async fn send_message_to_bitfun_session_stream(
            &self,
            request: AcpClientBitfunMessageRequest,
            chunk_sink: AcpClientStreamChunkSink,
        ) -> PortResult<AcpClientMessageResult> {
            self.bitfun_messages.lock().unwrap().push(request.clone());
            let _ = chunk_sink.send(AcpClientStreamChunk::Text {
                text: "external response".to_string(),
            });
            let _ = chunk_sink.send(AcpClientStreamChunk::Completed);
            Ok(AcpClientMessageResult {
                session_id: request.bitfun_session_id,
                response: "external response".to_string(),
            })
        }

        async fn delete_session_record(
            &self,
            session_id: String,
            _workspace_path: Option<String>,
        ) -> PortResult<()> {
            // 与真实桌面实现一致：delete_session_record 内部会 release + 删除记录
            self.deleted.lock().unwrap().push(session_id);
            Ok(())
        }

        async fn read_history(
            &self,
            request: AcpClientHistoryRequest,
        ) -> PortResult<AcpClientHistoryResult> {
            self.histories.lock().unwrap().push(request.clone());
            Ok(AcpClientHistoryResult {
                session_id: request.session_id,
                entries: vec![AcpClientHistoryEntry {
                    role: "user".to_string(),
                    content: "hello".to_string(),
                    timestamp_ms: Some(1_700_000_000_000),
                }],
                truncated: false,
            })
        }
    }

    fn context() -> ToolUseContext {
        use std::collections::HashMap;
        use std::path::PathBuf;
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: Some(crate::agentic::WorkspaceBinding::new(
                None,
                PathBuf::from("/repo/project"),
            )),
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: Default::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    #[tokio::test]
    async fn acp_control_create_forwards_client_and_workspace() {
        let port = FakeAcpClientPort::default();
        let results = run_acp_control(
            &port,
            &json!({
                "action": "create",
                "client_id": "codex",
                "workspace_path": "/repo/project",
                "session_name": "my acp",
            }),
            &context(),
        )
        .await
        .expect("create should succeed");

        let created = port.created.lock().unwrap();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].client_id, "codex");
        assert_eq!(created[0].workspace_path, "/repo/project");
        assert_eq!(created[0].session_name.as_deref(), Some("my acp"));

        let data = results[0].content();
        assert_eq!(data["success"], true);
        assert_eq!(data["action"], "create");
        assert_eq!(data["session"]["session_id"], "acp_codex_session-1");
        assert_eq!(data["session"]["agent_type"], "acp:codex");
    }

    #[tokio::test]
    async fn acp_control_create_falls_back_to_context_workspace() {
        let port = FakeAcpClientPort::default();
        run_acp_control(
            &port,
            &json!({ "action": "create", "client_id": "codex" }),
            &context(),
        )
        .await
        .expect("create should fall back to the context workspace");

        let created = port.created.lock().unwrap();
        assert_eq!(created[0].workspace_path, "/repo/project");
    }

    #[tokio::test]
    async fn acp_control_create_requires_client_id() {
        let port = FakeAcpClientPort::default();
        let error = run_acp_control(&port, &json!({ "action": "create" }), &context())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("client_id is required"));
        assert!(port.created.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn acp_control_list_returns_client_summaries() {
        let port = FakeAcpClientPort::default();
        let results = run_acp_control(&port, &json!({ "action": "list" }), &context())
            .await
            .expect("list should succeed");

        assert_eq!(*port.listed.lock().unwrap(), 1);
        let data = results[0].content();
        assert_eq!(data["count"], 1);
        assert_eq!(data["clients"][0]["client_id"], "codex");
        assert_eq!(data["clients"][0]["status"], "running");
    }

    #[tokio::test]
    async fn acp_control_delete_without_caller_session_is_rejected() {
        // R4 授权门：无 caller session → 拒绝 delete（不触碰 ACP port）。
        let port = FakeAcpClientPort::default();
        let error = run_acp_control(
            &port,
            &json!({
                "action": "delete",
                "session_id": "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b",
                "workspace_path": "/repo/project",
            }),
            &context(),
        )
        .await
        .expect_err("delete without a caller session must be rejected");
        assert!(
            error.to_string().contains("without a caller session"),
            "{error}"
        );
        assert!(port.deleted.lock().unwrap().is_empty());
        assert!(port.released.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn acp_control_cancel_without_caller_session_is_rejected() {
        // R4 授权门：无 caller session → 拒绝 cancel（不触碰 ACP port）。
        let port = FakeAcpClientPort::default();
        let error = run_acp_control(
            &port,
            &json!({
                "action": "cancel",
                "session_id": "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b",
                "workspace_path": "/repo/project",
            }),
            &context(),
        )
        .await
        .expect_err("cancel without a caller session must be rejected");
        assert!(
            error.to_string().contains("without a caller session"),
            "{error}"
        );
        assert!(port.cancelled.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn acp_control_delete_without_global_coordinator_is_rejected() {
        // R4 授权门：有 caller session 但无全局 coordinator → 拒绝 delete
        // （保守安全：无法完成授权时绝不触碰外部 port）。
        let port = FakeAcpClientPort::default();
        let mut ctx = context();
        ctx.session_id = Some("caller-1".to_string());
        let error = run_acp_control(
            &port,
            &json!({
                "action": "delete",
                "session_id": "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b",
                "workspace_path": "/repo/project",
            }),
            &ctx,
        )
        .await
        .expect_err("delete without a global coordinator must be rejected");
        assert!(
            error.to_string().contains("coordinator not initialized"),
            "{error}"
        );
        assert!(port.deleted.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn acp_control_delete_requires_session_id() {
        let port = FakeAcpClientPort::default();
        let error = run_acp_control(&port, &json!({ "action": "delete" }), &context())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("session_id is required"));
        assert!(port.released.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn acp_control_cancel_requires_session_id() {
        let port = FakeAcpClientPort::default();
        let error = run_acp_control(&port, &json!({ "action": "cancel" }), &context())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("session_id is required"));
        assert!(port.cancelled.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn acp_control_cancel_without_global_coordinator_is_rejected() {
        // R4 授权门：有 caller session 但无全局 coordinator → 拒绝 cancel。
        let port = FakeAcpClientPort::default();
        let mut ctx = context();
        ctx.session_id = Some("caller-1".to_string());
        let error = run_acp_control(
            &port,
            &json!({
                "action": "cancel",
                "session_id": "acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b",
                "workspace_path": "/repo/project",
            }),
            &ctx,
        )
        .await
        .expect_err("cancel without a global coordinator must be rejected");
        assert!(
            error.to_string().contains("coordinator not initialized"),
            "{error}"
        );
        assert!(port.cancelled.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn acp_control_unknown_action_rejected() {
        let port = FakeAcpClientPort::default();
        let error = run_acp_control(&port, &json!({ "action": "explode" }), &context())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("unknown acp_control action"));
    }

    #[tokio::test]
    async fn acp_message_forwards_through_real_channel() {
        let port = FakeAcpClientPort::default();
        let results = run_acp_message(
            &port,
            &json!({
                "session_id": "acp_codex_s1",
                "message": "hello external agent",
                "timeout_seconds": 30,
            }),
            &context(),
        )
        .await
        .expect("message should succeed");

        let messages = port.messages.lock().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].session_id, "acp_codex_s1");
        assert_eq!(messages[0].message, "hello external agent");
        assert_eq!(messages[0].timeout_seconds, Some(30));
        assert_eq!(messages[0].workspace_path.as_deref(), Some("/repo/project"));

        let data = results[0].content();
        assert_eq!(data["response"], "external response");
        let ToolResult::Result {
            result_for_assistant,
            ..
        } = &results[0]
        else {
            panic!("expected a result payload");
        };
        let assistant_text = result_for_assistant.as_ref().unwrap();
        // 方向 C：result_for_assistant 为极简通知句，不含全量 response 全文
        //（全文留在 data["response"]）；断言收到极简通知而非全文。
        assert!(assistant_text.contains("responded"));
        assert!(assistant_text.contains("SessionHistory"));
        assert!(!assistant_text.contains("external response"));
    }

    #[tokio::test]
    async fn acp_message_requires_message() {
        let port = FakeAcpClientPort::default();
        let error = run_acp_message(
            &port,
            &json!({ "session_id": "acp_codex_s1", "message": "   " }),
            &context(),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("message is required"));
    }

    #[tokio::test]
    async fn acp_history_returns_persisted_entries() {
        let port = FakeAcpClientPort::default();
        let results = run_acp_history(
            &port,
            &json!({ "session_id": "acp_codex_s1" }),
            &context(),
        )
        .await
        .expect("history should succeed");

        let histories = port.histories.lock().unwrap();
        assert_eq!(histories.len(), 1);
        assert_eq!(histories[0].session_id, "acp_codex_s1");

        let data = results[0].content();
        assert_eq!(data["count"], 1);
        assert_eq!(data["entries"][0]["role"], "user");
        assert_eq!(data["entries"][0]["content"], "hello");
        assert_eq!(data["truncated"], false);
    }

    #[tokio::test]
    async fn acp_history_requires_session_id() {
        let port = FakeAcpClientPort::default();
        let error = run_acp_history(&port, &json!({}), &context())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("session_id"));
    }

    #[tokio::test]
    async fn acp_control_validation_rejects_unknown_action() {
        let tool = AcpControlTool::new();
        let result = tool.validate_input(&json!({ "action": "boom" }), None).await;
        assert!(!result.result);
        assert!(result
            .message
            .unwrap()
            .contains("unknown acp_control action"));
    }

    #[tokio::test]
    async fn acp_control_validation_requires_client_id_for_create() {
        let tool = AcpControlTool::new();
        let result = tool.validate_input(&json!({ "action": "create" }), None).await;
        assert!(!result.result);
        assert!(result.message.unwrap().contains("client_id is required"));
    }

    #[tokio::test]
    async fn acp_message_validation_requires_session_and_message() {
        let tool = AcpMessageTool::new();
        let result = tool
            .validate_input(&json!({ "session_id": "", "message": "" }), None)
            .await;
        assert!(!result.result);

        let ok = tool
            .validate_input(
                &json!({ "session_id": "s1", "message": "hi" }),
                None,
            )
            .await;
        assert!(ok.result);
    }

    #[tokio::test]
    async fn acp_history_validation_requires_session_id() {
        let tool = AcpHistoryTool::new();
        let result = tool.validate_input(&json!({}), None).await;
        assert!(!result.result);

        let ok = tool.validate_input(&json!({ "session_id": "s1" }), None).await;
        assert!(ok.result);
    }

    #[test]
    fn acp_tool_names_match_registered_contract() {
        assert_eq!(AcpControlTool::new().name(), "acp_control");
        assert_eq!(AcpMessageTool::new().name(), "acp_message");
        assert_eq!(AcpHistoryTool::new().name(), "acp_history");
    }
}
