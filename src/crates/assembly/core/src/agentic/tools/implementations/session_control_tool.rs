//! SessionControl manages persisted workspace-scoped sessions.
//!
//! The `cancel` action only cancels the target session's current running dialog turn.
//! It does not permanently stop the session itself, and it does not clear queued
//! messages that may still run later through the scheduler.

use super::util::normalize_path;
use crate::agentic::coordination::{get_global_coordinator, get_global_scheduler};
use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::service_agent_runtime::CoreServiceAgentRuntime;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_agent_runtime::sdk::AgentRuntime;
use bitfun_agent_runtime::session_control::{
    render_session_control_tool_use_message, resolve_session_control_cancel_route,
    session_control_agent_type_or_default, session_control_cancel_result_message,
    session_control_cancel_status, session_control_created_result_message,
    session_control_creator_marker, session_control_deleted_result_message,
    session_control_session_name_or_default, validate_session_control_input, validate_session_id,
    SessionControlAction, SessionControlCancelRoute, SessionControlInput,
    SessionControlValidationContext, SessionControlValidationResult,
};
use bitfun_runtime_ports::{
    AgentSessionCreateRequest, AgentSessionDeleteRequest, AgentSessionListRequest,
    AgentSessionSummary, AgentSessionWorkspaceBinding, AgentSessionWorkspaceRequest,
    AgentSubmissionSource, AgentTurnCancellationRequest,
};
use bitfun_services_core::session::tree::SessionTreeManager;
use log::warn;
use serde_json::{json, Value};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// SessionControl tool - create, cancel, delete, or list persisted sessions
/// list：列出 SessionControl 创建的持久 session。
/// list_tasks：列出 Task spawn 的子对话 session。
pub struct SessionControlTool;

const CANCEL_WAIT_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
struct SessionControlWorkspaceTarget {
    display_workspace: String,
    remote_connection_id: Option<String>,
    remote_ssh_host: Option<String>,
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

    fn escape_markdown_table_cell(value: &str) -> String {
        value
            .replace('\\', "\\\\")
            .replace('|', "\\|")
            .replace('\n', "<br>")
    }

    fn format_system_time(time: SystemTime) -> String {
        let datetime: chrono::DateTime<chrono::Local> = time.into();
        datetime.format("%Y-%m-%dT%H:%M:%S").to_string()
    }

    fn creator_session_marker(&self, context: &ToolUseContext) -> BitFunResult<String> {
        let creator_session_id = context.session_id.as_ref().ok_or_else(|| {
            BitFunError::tool("create requires a creator session in tool context".to_string())
        })?;
        Ok(session_control_creator_marker(creator_session_id))
    }

    async fn resolve_effective_workspace(
        &self,
        action: SessionControlAction,
        session_id: Option<&str>,
        context: &ToolUseContext,
        runtime: &AgentRuntime,
    ) -> BitFunResult<SessionControlWorkspaceTarget> {
        match action {
            SessionControlAction::Cancel | SessionControlAction::Delete => {
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

    fn workspace_target_from_context(
        workspace: &crate::agentic::WorkspaceBinding,
    ) -> SessionControlWorkspaceTarget {
        SessionControlWorkspaceTarget {
            display_workspace: normalize_path(&workspace.root_path_string()),
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
        SessionControlWorkspaceTarget {
            display_workspace: binding.workspace_path,
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

    fn system_time_from_epoch_ms(epoch_ms: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(epoch_ms)
    }

    fn build_list_result_for_assistant(
        &self,
        workspace: &str,
        sessions: &[AgentSessionSummary],
        current_session_id: Option<&str>,
        tree: Option<&SessionTreeManager>,
    ) -> String {
        if sessions.is_empty() {
            return format!("No sessions found in workspace '{}'.", workspace);
        }

        // --- Build tree JSON from flat list ---
        let tree_json = self.build_session_tree_json(sessions, tree);

        // --- Flat markdown table (fallback) ---
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

        // Tree structure header
        lines.push("## Session Tree (JSON)".to_string());
        lines.push("```json".to_string());
        lines.push(tree_json);
        lines.push("```".to_string());
        lines.push(String::new());

        // Flat table
        lines.push("## Flat List".to_string());
        lines.push(
            "| session_id | session_name | agent_type | created_at | last_active_at | parent |".to_string(),
        );
        lines.push("| --- | --- | --- | --- | --- | --- |".to_string());
        for session in sessions {
            lines.push(format!(
                "| {} | {} | {} | {} | {} | {} |",
                Self::escape_markdown_table_cell(&session.session_id),
                Self::escape_markdown_table_cell(&session.session_name),
                Self::escape_markdown_table_cell(&session.agent_type),
                Self::format_system_time(Self::system_time_from_epoch_ms(session.created_at_ms)),
                Self::format_system_time(Self::system_time_from_epoch_ms(
                    session.last_active_at_ms
                )),
                session.parent_session_id.as_deref().unwrap_or("-"),
            ));
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

    async fn get_available_agent_type_ids(&self, context: Option<&ToolUseContext>) -> Vec<String> {
        use crate::agentic::agents::{get_agent_registry, SubagentListScope, SubagentQueryContext};
        let registry = get_agent_registry();
        let workspace_root = context.and_then(|ctx| ctx.workspace_root());
        registry.load_custom_agents(workspace_root).await;
        let agents = registry
            .get_subagents_for_query(&SubagentQueryContext {
                parent_agent_type: context.and_then(|ctx| ctx.agent_type.as_deref()),
                workspace_root,
                list_scope: SubagentListScope::TaskVisible,
                include_disabled: false,
                external_sources_supported: context.is_none_or(|ctx| !ctx.is_remote()),
            })
            .await;
        let mut ids: Vec<String> = agents.into_iter().map(|a| a.id).collect();
        for builtin in &["agentic", "Plan", "Cowork"] {
            let b = builtin.to_string();
            if !ids.contains(&b) {
                ids.push(b);
            }
        }
        ids.sort();
        ids.dedup();
        ids
    }
}

/// Build a JSON tree structure from the flat session list.
/// Sessions are grouped by `parent_session_id` into a forest of root nodes.
pub(crate) fn build_session_tree_json_impl(
    sessions: &[AgentSessionSummary],
    tree: Option<&SessionTreeManager>,
) -> String {
    use std::collections::HashMap;

    // children_by_parent: parent_session_id -> list of children
    let mut children_by_parent: HashMap<&str, Vec<&AgentSessionSummary>> = HashMap::new();
    let mut roots: Vec<&AgentSessionSummary> = Vec::new();

    let known_ids: std::collections::HashSet<&str> =
        sessions.iter().map(|s| s.session_id.as_str()).collect();

    for session in sessions {
        if let Some(ref pid) = session.parent_session_id {
            if known_ids.contains(pid.as_str()) {
                children_by_parent
                    .entry(pid.as_str())
                    .or_default()
                    .push(session);
            } else {
                // Parent not in this list — treat as root
                roots.push(session);
            }
        } else {
            roots.push(session);
        }
    }

    /// Maximum recursion depth for tree serialization to prevent stack overflow.
    const TREE_SERIALIZE_MAX_DEPTH: usize = 256;

    fn serialize_node(
        session: &AgentSessionSummary,
        children_by_parent: &HashMap<&str, Vec<&AgentSessionSummary>>,
        tree: Option<&SessionTreeManager>,
        recursion_depth: usize,
    ) -> serde_json::Value {
        let children: Vec<serde_json::Value> = if recursion_depth >= TREE_SERIALIZE_MAX_DEPTH {
            Vec::new()
        } else {
            children_by_parent
                .get(session.session_id.as_str())
                .map(|list| {
                    let mut sorted = list.to_vec();
                    sorted.sort_by(|a, b| a.created_at_ms.cmp(&b.created_at_ms));
                    sorted
                        .iter()
                        .map(|s| serialize_node(s, children_by_parent, tree, recursion_depth + 1))
                        .collect()
                })
                .unwrap_or_default()
        };

        let depth = tree
            .and_then(|t| t.get_depth(&session.session_id))
            .unwrap_or(0);

        let status = session.status.clone().unwrap_or_else(|| "active".to_string());

        let mut map = serde_json::Map::new();
        map.insert("sessionId".to_string(), json!(session.session_id));
        map.insert("sessionName".to_string(), json!(session.session_name));
        map.insert("agentType".to_string(), json!(session.agent_type));
        map.insert("depth".to_string(), json!(depth));
        map.insert("status".to_string(), json!(status));
        map.insert("children".to_string(), json!(children));
        serde_json::Value::Object(map)
    }

    // Sort roots by created_at_ms descending (newest first)
    let mut sorted_roots = roots;
    sorted_roots.sort_by(|a, b| b.created_at_ms.cmp(&a.created_at_ms));

    let forest: Vec<serde_json::Value> = sorted_roots
        .iter()
        .map(|s| serialize_node(s, &children_by_parent, tree, 0))
        .collect();

    serde_json::to_string_pretty(&forest).unwrap_or_else(|_| "[]".to_string())
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
- "create": Create a new session. You may optionally provide session_name and agent_type.
- "cancel": Cancel the target session's currently running dialog turn. This does not delete the session or clear any queued messages that may still run later.
- "delete": Delete an existing session by session_id.
- "list": List all sessions. Sessions are displayed in a tree structure showing parent-child relationships (created via Task tool).

Related tools:
- Use Task (spawn) to launch subagents that appear as children in the session tree.
- Use SessionMessage to send messages to existing sessions.
- Use SessionHistory to export a session transcript.

Arguments:
- "workspace": Absolute workspace path. Required for create and list. Ignored for cancel and delete.
- "session_name": Only used by create. Defaults to "New Session".
- "agent_type": Only used by create. Defaults to "agentic". Allowed values are dynamically resolved from the available agent registry (common values include "agentic", "Plan", "Cowork", and any custom/external subagent types).
- "session_id": Required for cancel and delete."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Create, list, cancel, and delete persisted agent sessions.".to_string()
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
                    "enum": ["create", "cancel", "delete", "list"],
                    "description": "The session action to perform."
                },
                "workspace": {
                    "type": "string",
                    "description": "Required absolute workspace path for create and list. Ignored for cancel and delete."
                },
                "session_id": {
                    "type": "string",
                    "description": "Required for cancel and delete."
                },
                "session_name": {
                    "type": "string",
                    "description": "Optional display name when creating a session."
                },
                "agent_type": {
                    "type": "string",
                    "description": "Optional agent type when creating a session (defaults to \"agentic\"). Valid values are dynamically resolved from the available agent registry."
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    /// Dynamically resolves allowed agent_type values from the agent registry.
    async fn input_schema_for_model_with_context(&self, context: Option<&ToolUseContext>) -> Value {
        let agent_type_ids = self.get_available_agent_type_ids(context).await;
        let agent_type_enum: Vec<&str> = agent_type_ids.iter().map(|s| s.as_str()).collect();
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "cancel", "delete", "list"],
                    "description": "The session action to perform."
                },
                "workspace": {
                    "type": "string",
                    "description": "Required absolute workspace path for create and list. Ignored for cancel and delete."
                },
                "session_id": {
                    "type": "string",
                    "description": "Required for cancel and delete."
                },
                "session_name": {
                    "type": "string",
                    "description": "Optional display name when creating a session."
                },
                "agent_type": {
                    "type": "string",
                    "enum": agent_type_enum,
                    "description": "Optional agent type when creating a session. Defaults to \"agentic\"."
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
                        context,
                        &runtime,
                    )
                    .await?;
                let session_name =
                    session_control_session_name_or_default(params.session_name.as_deref());
                let agent_type = session_control_agent_type_or_default(params.agent_type.as_ref());
                let created_by = self.creator_session_marker(context)?;
                let mut metadata = serde_json::Map::new();
                metadata.insert("createdBy".to_string(), json!(created_by));
                let session = runtime
                    .create_session(AgentSessionCreateRequest {
                        session_name,
                        agent_type,
                        workspace_path: Some(workspace.display_workspace.clone()),
                        workspace_id: None,
                        remote_connection_id: workspace.remote_connection_id.clone(),
                        remote_ssh_host: workspace.remote_ssh_host.clone(),
                        model_id: None,
                        metadata,
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?;
                let created_session_id = session.session_id.clone();
                let created_session_name = session.session_name.clone();
                let created_agent_type = session.agent_type.clone();

                // --- R-001/R-002: 写入 SessionRelationship，depth 从父继承 ---
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
                                &std::path::PathBuf::from(&workspace.display_workspace),
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
                    if let Err(e) = coordinator
                        .session_manager
                        .persist_session_lineage(&created_session_id, relationship)
                        .await
                    {
                        log::error!(
                            "SessionControl create: failed to persist session lineage for {}: {:?}",
                            created_session_id,
                            e
                        );
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
                }
                let result_for_assistant = session_control_created_result_message(
                    &created_session_id,
                    &workspace.display_workspace,
                    &created_agent_type,
                );

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "create",
                        "workspace": workspace.display_workspace.clone(),
                        "session": {
                            "session_id": created_session_id,
                            "session_name": created_session_name,
                            "agent_type": created_agent_type,
                        }
                    }),
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

                // R-011: Skip list-based pre-check so subagent (Task) sessions can be cancelled.
                // The runtime's cancel_turn handles session-existence internally.

                // Authorization: verify the calling session is an ancestor of the target session.
                // First try the in-memory tree (fast path). If the tree is not yet populated
                // (walk_ancestors returns empty), fall back to a persisted metadata chain query
                // so that an empty tree cannot be exploited to bypass authorization.
                let current_session_id = context.session_id.as_ref().ok_or_else(|| {
                    BitFunError::tool(
                        "cannot cancel a session without a caller session in tool context"
                            .to_string(),
                    )
                })?;
                {
                    let tree = coordinator.session_tree();
                    let tree_ancestors = tree.walk_ancestors(session_id);
                    let ancestors: Vec<String> = if !tree_ancestors.is_empty() {
                        // Fast path: tree is populated.
                        tree_ancestors
                    } else {
                        // Fallback: tree is empty, walk persisted metadata chain.
                        // 已知优化点：可改为批量查询，避免每个祖先 session 都串行 await。
                        let session_manager = coordinator.get_session_manager();
                        let mut metadata_ancestors = Vec::new();
                        let mut current = session_id.to_string();
                        loop {
                            let metadata = session_manager
                                .load_session_metadata(
                                    &std::path::PathBuf::from(&workspace.display_workspace),
                                    &current,
                                )
                                .await
                                .ok()
                                .flatten();
                            match metadata
                                .and_then(|m| m.relationship.and_then(|r| r.parent_session_id))
                            {
                                Some(parent_id) => {
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
                            "session '{current_session_id}' is not authorized to cancel session '{session_id}': not a parent/ancestor"
                        )));
                    }
                }

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

                // coordinator.delete_session() handles session-existence internally;
                // skipping the list-based pre-check so subagent (Task) sessions are supported.

                // Authorization: verify the calling session is an ancestor of the target session.
                // First try the in-memory tree (fast path). If the tree is not yet populated
                // (walk_ancestors returns empty), fall back to a persisted metadata chain query
                // so that an empty tree cannot be exploited to bypass authorization.
                let current_session_id = context.session_id.as_ref().ok_or_else(|| {
                    BitFunError::tool(
                        "cannot delete a session without a caller session in tool context"
                            .to_string(),
                    )
                })?;
                {
                    let tree = coordinator.session_tree();
                    let tree_ancestors = tree.walk_ancestors(session_id);
                    let ancestors: Vec<String> = if !tree_ancestors.is_empty() {
                        // Fast path: tree is populated.
                        tree_ancestors
                    } else {
                        // Fallback: tree is empty, walk persisted metadata chain.
                        // 已知优化点：可改为批量查询，避免每个祖先 session 都串行 await。
                        let session_manager = coordinator.get_session_manager();
                        let mut metadata_ancestors = Vec::new();
                        let mut current = session_id.to_string();
                        loop {
                            let metadata = session_manager
                                .load_session_metadata(
                                    &std::path::PathBuf::from(&workspace.display_workspace),
                                    &current,
                                )
                                .await
                                .ok()
                                .flatten();
                            match metadata
                                .and_then(|m| m.relationship.and_then(|r| r.parent_session_id))
                            {
                                Some(parent_id) => {
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
                            "session '{current_session_id}' is not authorized to delete session '{session_id}': not a parent/ancestor"
                        )));
                    }
                }

                let scheduler = get_global_scheduler().ok_or_else(|| {
                    BitFunError::tool("scheduler not initialized for session deletion".to_string())
                })?;
                let deletion_runtime = CoreServiceAgentRuntime::agent_runtime_with_scheduler_ports(
                    coordinator.clone(),
                    scheduler,
                )
                .map_err(BitFunError::tool)?;

                // R-012: Cascade delete children before deleting parent.
                // Prefer the in-memory tree for descendant discovery; fall back to
                // full metadata scan only when the tree is not populated.
                let mut cascade_failures: Vec<String> = Vec::new();
                {
                    let tree = coordinator.session_tree();
                    let mut cascade_ids = tree.get_descendants(session_id);

                    if cascade_ids.is_empty() {
                        // Tree fallback: load all metadata and build parent→children map.
                        let all_metadata = coordinator
                            .get_session_manager()
                            .persistence_manager()
                            .list_session_metadata_including_internal(
                                &std::path::PathBuf::from(&workspace.display_workspace),
                            )
                            .await
                            .unwrap_or_default();

                        let mut children_map: std::collections::HashMap<String, Vec<String>> =
                            std::collections::HashMap::new();
                        for m in &all_metadata {
                            if let Some(ref parent_id) = m
                                .relationship
                                .as_ref()
                                .and_then(|r| r.parent_session_id.as_ref())
                            {
                                children_map
                                    .entry(parent_id.to_string())
                                    .or_default()
                                    .push(m.session_id.clone());
                            }
                        }

                        // DFS to collect all descendants in pre-order, then reverse for post-order.
                        let mut stack: Vec<String> = vec![session_id.to_string()];
                        let mut order: Vec<String> = Vec::new();
                        while let Some(id) = stack.pop() {
                            order.push(id.clone());
                            if let Some(children) = children_map.get(&id) {
                                for child in children.iter().rev() {
                                    stack.push(child.clone());
                                }
                            }
                        }

                        // Post-order: children before parent, skip the target session itself.
                        for id in order.into_iter().rev() {
                            if id != session_id {
                                cascade_ids.push(id);
                            }
                        }
                    }

                    for child_id in &cascade_ids {
                        if let Err(error) = deletion_runtime
                            .delete_session(AgentSessionDeleteRequest {
                                workspace_path: workspace.display_workspace.clone(),
                                session_id: child_id.clone(),
                                remote_connection_id: workspace.remote_connection_id.clone(),
                                remote_ssh_host: workspace.remote_ssh_host.clone(),
                            })
                            .await
                        {
                            warn!(
                                "Failed to cascade-delete child session {}: {}",
                                child_id,
                                CoreServiceAgentRuntime::runtime_error_message(error)
                            );
                            // Do NOT remove_subtree for failed deletions — keep tree
                            // consistent with storage.
                            cascade_failures.push(child_id.clone());
                        }
                    }
                }

                deletion_runtime
                    .delete_session(AgentSessionDeleteRequest {
                        workspace_path: workspace.display_workspace.clone(),
                        session_id: session_id.to_string(),
                        remote_connection_id: workspace.remote_connection_id.clone(),
                        remote_ssh_host: workspace.remote_ssh_host.clone(),
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?;

                // R-012: Remove subtree from in-memory tree after successful deletion
                if cascade_failures.is_empty() {
                    coordinator
                        .session_tree()
                        .remove_subtree(session_id);
                } else {
                    log::warn!(
                        "SessionControl delete: {} child deletions failed, tree not cleaned for {}",
                        cascade_failures.len(),
                        session_id
                    );
                }

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "delete",
                        "workspace": workspace.display_workspace.clone(),
                        "session_id": session_id,
                    }),
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
                        context,
                        &runtime,
                    )
                    .await?;
                let sessions = runtime
                    .list_sessions(AgentSessionListRequest {
                        workspace_path: workspace.display_workspace.clone(),
                        remote_connection_id: workspace.remote_connection_id.clone(),
                        remote_ssh_host: workspace.remote_ssh_host.clone(),
                    })
                    .await
                    .map_err(|error| {
                        BitFunError::tool(CoreServiceAgentRuntime::runtime_error_message(error))
                    })?;
                let current_session_id =
                    self.current_workspace_session(context, &workspace.display_workspace);
                let result_for_assistant = self.build_list_result_for_assistant(
                    &workspace.display_workspace,
                    &sessions,
                    current_session_id,
                    Some(coordinator.session_tree().as_ref()),
                );

                let tree_json = self.build_session_tree_json(
                    &sessions,
                    Some(coordinator.session_tree().as_ref()),
                );
                let tree_value: Value =
                    serde_json::from_str(&tree_json).unwrap_or(Value::Null);

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "list",
                        "workspace": workspace.display_workspace.clone(),
                        "current_session_id": current_session_id,
                        "count": sessions.len(),
                        "sessions": sessions,
                        "tree": tree_value,
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
    use serde_json::json;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
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
}
