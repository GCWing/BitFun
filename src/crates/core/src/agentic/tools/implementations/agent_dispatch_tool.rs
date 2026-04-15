use crate::agentic::coordination::{
    get_global_coordinator, DialogTriggerSource, DialogSubmissionPolicy, get_global_scheduler,
    AgentSessionReplyRoute,
};
use crate::agentic::core::SessionConfig;
use crate::agentic::SessionSummary;
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::service::workspace::get_global_workspace_service;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use log::warn;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

/// AgentDispatch tool — creates and manages Standard agent sessions.
///
/// Unlike the existing SessionControl tool, AgentDispatch:
/// - Supports all Mode agent types (including Claw, debug, Dispatcher)
/// - Accepts an optional task_briefing sent as the first message to the new session
/// - Provides a `list` action that combines recent workspaces with their sessions
/// - Provides a `status` action scoped to sessions created by this Dispatcher
pub struct AgentDispatchTool;

impl Default for AgentDispatchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentDispatchTool {
    pub fn new() -> Self {
        Self
    }

    async fn normalize_workspace(workspace: &str, context: &ToolUseContext) -> BitFunResult<String> {
        let workspace = workspace.trim();
        if workspace.is_empty() {
            return Err(BitFunError::tool("workspace cannot be empty".to_string()));
        }

        // "global" is a special sentinel — resolve to the default assistant workspace
        if workspace == "global" {
            return Ok(Self::get_global_workspace_path().await);
        }

        let path = Path::new(workspace);
        if !path.is_absolute() {
            return Err(BitFunError::tool(
                "workspace must be an absolute path or the keyword 'global'".to_string(),
            ));
        }

        // For remote workspaces path existence is checked on the remote host
        if !context.is_remote() {
            if !path.exists() {
                return Err(BitFunError::tool(format!(
                    "workspace does not exist: {}",
                    workspace
                )));
            }
            if !path.is_dir() {
                return Err(BitFunError::tool(format!(
                    "workspace is not a directory: {}",
                    workspace
                )));
            }
        }

        Ok(workspace.to_string())
    }

    /// Returns the path for the "global" workspace (default assistant workspace).
    /// Falls back to the user's home directory if no assistant workspace is found.
    async fn get_global_workspace_path() -> String {
        if let Some(ws_service) = get_global_workspace_service() {
            let assistants = ws_service.get_assistant_workspaces().await;
            // Prefer the default workspace (no assistantId), then first available
            let global = assistants
                .iter()
                .find(|w| w.assistant_id.is_none())
                .or_else(|| assistants.first());
            if let Some(ws) = global {
                return ws.root_path.to_string_lossy().into_owned();
            }
        }
        dirs::home_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/".to_string())
    }

    fn valid_agent_types() -> &'static [&'static str] {
        &["agentic", "Plan", "Cowork", "debug", "Claw"]
    }

    fn creator_marker(context: &ToolUseContext) -> BitFunResult<String> {
        let session_id = context.session_id.as_ref().ok_or_else(|| {
            BitFunError::tool("AgentDispatch requires a session context".to_string())
        })?;
        Ok(format!("session-{}", session_id))
    }

}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum AgentDispatchAction {
    Create,
    List,
    Status,
}

#[derive(Debug, Deserialize)]
struct AgentDispatchInput {
    action: AgentDispatchAction,
    /// Target workspace: absolute path or "global"
    workspace: Option<String>,
    /// Agent type for create action
    agent_type: Option<String>,
    /// Display name for the new session
    session_name: Option<String>,
    /// Initial task briefing sent as first message to the new session
    task_briefing: Option<String>,
}

#[async_trait]
impl Tool for AgentDispatchTool {
    fn name(&self) -> &str {
        "AgentDispatch"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Create and manage agent sessions as the Dispatcher.

Actions:
- "create": Create a new Standard agent session in a workspace. The agent will be visible in the session list and the user can switch to it directly.
- "list": List recent workspaces and their existing sessions, so you can find matching workspace paths.
- "status": Show all sessions that were created by this Dispatcher session.

Parameters for "create":
- workspace: Absolute path to the project directory, or "global" for non-project tasks.
- agent_type: One of "agentic" (coding), "Plan" (planning), "Cowork" (collaboration), "debug" (debugging), "Claw" (desktop automation).
- session_name: Short descriptive name for the session (e.g. "Fix login bug").
- task_briefing: Full instructions sent as the first message to the new agent. Include all relevant context since the agent cannot see the Dispatcher conversation.

Parameters for "list":
  No additional parameters required.

Parameters for "status":
  No additional parameters required."#
            .to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "list", "status"],
                    "description": "create: create a new agent session; list: discover workspaces and sessions; status: view Dispatcher-created sessions"
                },
                "workspace": {
                    "type": "string",
                    "description": "Absolute path to the workspace directory, or 'global' for non-project tasks. Required for create."
                },
                "agent_type": {
                    "type": "string",
                    "enum": ["agentic", "Plan", "Cowork", "debug", "Claw"],
                    "description": "Type of agent to create. Required for create."
                },
                "session_name": {
                    "type": "string",
                    "description": "Short display name for the session. Optional for create."
                },
                "task_briefing": {
                    "type": "string",
                    "description": "Full task description sent as the agent's first message. Optional for create."
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let parsed: AgentDispatchInput = match serde_json::from_value(input.clone()) {
            Ok(v) => v,
            Err(e) => {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Invalid input: {}", e)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        if let AgentDispatchAction::Create = parsed.action {
            if parsed.workspace.as_deref().unwrap_or("").is_empty() {
                return ValidationResult {
                    result: false,
                    message: Some("workspace is required for create".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
            if let Some(ref at) = parsed.agent_type {
                if !Self::valid_agent_types().contains(&at.as_str()) {
                    return ValidationResult {
                        result: false,
                        message: Some(format!(
                            "agent_type must be one of: {}",
                            Self::valid_agent_types().join(", ")
                        )),
                        error_code: Some(400),
                        meta: None,
                    };
                }
            } else {
                return ValidationResult {
                    result: false,
                    message: Some("agent_type is required for create".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        ValidationResult::default()
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("?");
        match action {
            "create" => {
                let agent = input.get("agent_type").and_then(|v| v.as_str()).unwrap_or("agent");
                let name = input.get("session_name").and_then(|v| v.as_str()).unwrap_or("New Session");
                format!("Create {} session: {}", agent, name)
            }
            "list" => "List workspaces and sessions".to_string(),
            "status" => "Check agent session status".to_string(),
            _ => format!("Agent dispatch: {}", action),
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: AgentDispatchInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::tool(format!("Invalid input: {}", e)))?;

        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;

        match params.action {
            AgentDispatchAction::Create => {
                let workspace_raw = params.workspace.as_deref().unwrap_or("");
                let workspace = Self::normalize_workspace(workspace_raw, context).await?;
                let agent_type = params
                    .agent_type
                    .clone()
                    .unwrap_or_else(|| "agentic".to_string());
                let session_name = params
                    .session_name
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| format!("{} session", agent_type));
                let created_by = Self::creator_marker(context)?;

                let session = coordinator
                    .create_session_with_workspace_and_creator(
                        None,
                        session_name.clone(),
                        agent_type.clone(),
                        SessionConfig {
                            workspace_path: Some(workspace.clone()),
                            ..Default::default()
                        },
                        workspace.clone(),
                        Some(created_by),
                    )
                    .await?;

                let session_id = session.session_id.clone();

                // Send task_briefing as first message if provided.
                // Include a reply_route so that when the sub-agent's turn completes, the
                // scheduler automatically forwards the result back to this Dispatcher session
                // as a user message, enabling the Dispatcher to report task completion.
                if let Some(briefing) = params.task_briefing.filter(|b| !b.trim().is_empty()) {
                    if let Some(scheduler) = get_global_scheduler() {
                        let dispatcher_session_id = context
                            .session_id
                            .clone()
                            .unwrap_or_default();
                        let dispatcher_workspace = if let Some(root) = context.workspace_root() {
                            root.to_string_lossy().into_owned()
                        } else {
                            Self::get_global_workspace_path().await
                        };
                        let reply_route = if dispatcher_session_id.is_empty() {
                            None
                        } else {
                            Some(AgentSessionReplyRoute {
                                source_session_id: dispatcher_session_id,
                                source_workspace_path: dispatcher_workspace,
                            })
                        };

                        let submit_result = scheduler
                            .submit(
                                session_id.clone(),
                                briefing,
                                None,
                                None,
                                agent_type.clone(),
                                Some(workspace.clone()),
                                DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
                                reply_route,
                                None,
                            )
                            .await;

                        if let Err(e) = submit_result {
                            warn!(
                                "AgentDispatch: task_briefing submission failed for session {}: {}",
                                session_id, e
                            );
                        }
                    }
                }

                let result_for_assistant = format!(
                    "Created {} session '{}' (id: {}) in workspace '{}'.",
                    agent_type, session_name, session_id, workspace
                );

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "action": "create",
                        "success": true,
                        "session_id": session_id,
                        "session_name": session.session_name,
                        "agent_type": session.agent_type,
                        "workspace": workspace,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }

            AgentDispatchAction::List => {
                let mut workspace_entries: Vec<Value> = Vec::new();

                if let Some(ws_service) = get_global_workspace_service() {
                    // --- Global (assistant) workspaces first ---
                    let assistant_workspaces = ws_service.get_assistant_workspaces().await;
                    for ws_info in &assistant_workspaces {
                        let workspace_path = ws_info.root_path.to_string_lossy().into_owned();
                        let path = Path::new(&workspace_path);
                        let sessions: Vec<SessionSummary> = if path.exists() {
                            coordinator.list_sessions(path).await.unwrap_or_default()
                        } else {
                            Vec::new()
                        };
                        workspace_entries.push(json!({
                            "name": ws_info.name,
                            "path": workspace_path,
                            "kind": "global",
                            "session_count": sessions.len(),
                            "sessions": sessions.iter().map(|s| json!({
                                "session_id": s.session_id,
                                "session_name": s.session_name,
                                "agent_type": s.agent_type,
                                "created_at": s.created_at,
                                "last_activity_at": s.last_activity_at,
                            })).collect::<Vec<_>>(),
                        }));
                    }

                    // --- Recent project workspaces ---
                    let recent = ws_service.get_recent_workspaces().await;
                    for workspace_info in recent {
                        let workspace_path = workspace_info.root_path.to_string_lossy().into_owned();
                        let path = Path::new(&workspace_path);

                        let sessions: Vec<SessionSummary> = if path.exists() {
                            coordinator.list_sessions(path).await.unwrap_or_default()
                        } else {
                            Vec::new()
                        };

                        workspace_entries.push(json!({
                            "name": workspace_info.name,
                            "path": workspace_path,
                            "kind": "project",
                            "last_accessed": workspace_info.last_accessed.to_rfc3339(),
                            "session_count": sessions.len(),
                            "sessions": sessions.iter().map(|s| json!({
                                "session_id": s.session_id,
                                "session_name": s.session_name,
                                "agent_type": s.agent_type,
                                "created_at": s.created_at,
                                "last_activity_at": s.last_activity_at,
                            })).collect::<Vec<_>>(),
                        }));
                    }
                }

                let total_sessions: usize = workspace_entries
                    .iter()
                    .filter_map(|e| e["session_count"].as_u64())
                    .map(|n| n as usize)
                    .sum();

                let mut text_lines = vec![format!(
                    "Found {} workspace(s) with {} total session(s):",
                    workspace_entries.len(),
                    total_sessions
                )];
                for entry in &workspace_entries {
                    let name = entry["name"].as_str().unwrap_or("?");
                    let path = entry["path"].as_str().unwrap_or("?");
                    let kind = entry["kind"].as_str().unwrap_or("project");
                    let count = entry["session_count"].as_u64().unwrap_or(0);
                    text_lines.push(format!(
                        "  - [{}] {} ({}): {} session(s)",
                        kind, name, path, count
                    ));
                }

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "action": "list",
                        "workspace_count": workspace_entries.len(),
                        "workspaces": workspace_entries,
                    }),
                    result_for_assistant: Some(text_lines.join("\n")),
                    image_attachments: None,
                }])
            }

            AgentDispatchAction::Status => {
                let creator_marker = Self::creator_marker(context)?;
                let workspace_path = context.workspace_root();

                let all_sessions: Vec<SessionSummary> = if let Some(path) = workspace_path {
                    coordinator.list_sessions(path).await.unwrap_or_default()
                } else {
                    Vec::new()
                };

                // Also include sessions from recent workspaces
                let mut dispatcher_sessions: Vec<Value> = Vec::new();

                if let Some(ws_service) = get_global_workspace_service() {
                    // Check global (assistant) workspaces
                    let assistant_workspaces = ws_service.get_assistant_workspaces().await;
                    for workspace_info in &assistant_workspaces {
                        let path = workspace_info.root_path.as_path();
                        if !path.exists() {
                            continue;
                        }
                        let sessions = coordinator.list_sessions(path).await.unwrap_or_default();
                        for s in sessions {
                            if s.created_by.as_deref() == Some(&creator_marker) {
                                dispatcher_sessions.push(json!({
                                    "session_id": s.session_id,
                                    "session_name": s.session_name,
                                    "agent_type": s.agent_type,
                                    "workspace": workspace_info.root_path.to_string_lossy(),
                                    "workspace_kind": "global",
                                    "created_at": s.created_at,
                                    "last_activity_at": s.last_activity_at,
                                }));
                            }
                        }
                    }
                    // Check recent project workspaces
                    let recent = ws_service.get_recent_workspaces().await;
                    for workspace_info in recent {
                        let path = workspace_info.root_path.as_path();
                        if !path.exists() {
                            continue;
                        }
                        let sessions = coordinator.list_sessions(path).await.unwrap_or_default();
                        for s in sessions {
                            if s.created_by.as_deref() == Some(&creator_marker) {
                                dispatcher_sessions.push(json!({
                                    "session_id": s.session_id,
                                    "session_name": s.session_name,
                                    "agent_type": s.agent_type,
                                    "workspace": workspace_info.root_path.to_string_lossy(),
                                    "workspace_kind": "project",
                                    "created_at": s.created_at,
                                    "last_activity_at": s.last_activity_at,
                                }));
                            }
                        }
                    }
                }

                // Also check current workspace sessions
                for s in &all_sessions {
                    let already_included = dispatcher_sessions
                        .iter()
                        .any(|e| e["session_id"].as_str() == Some(&s.session_id));
                    if !already_included && s.created_by.as_deref() == Some(&creator_marker) {
                        let ws = workspace_path
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        dispatcher_sessions.push(json!({
                            "session_id": s.session_id,
                            "session_name": s.session_name,
                            "agent_type": s.agent_type,
                            "workspace": ws,
                            "created_at": s.created_at,
                            "last_activity_at": s.last_activity_at,
                        }));
                    }
                }

                let sessions_table = {
                    let lines = if dispatcher_sessions.is_empty() {
                        vec!["No sessions created by this Dispatcher yet.".to_string()]
                    } else {
                        let mut l = vec![
                            "| session_id | session_name | agent_type | workspace |".to_string(),
                            "| --- | --- | --- | --- |".to_string(),
                        ];
                        for e in &dispatcher_sessions {
                            l.push(format!(
                                "| {} | {} | {} | {} |",
                                e["session_id"].as_str().unwrap_or(""),
                                e["session_name"].as_str().unwrap_or(""),
                                e["agent_type"].as_str().unwrap_or(""),
                                e["workspace"].as_str().unwrap_or(""),
                            ));
                        }
                        l
                    };
                    lines.join("\n")
                };

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "action": "status",
                        "dispatcher_session_count": dispatcher_sessions.len(),
                        "sessions": dispatcher_sessions,
                    }),
                    result_for_assistant: Some(format!(
                        "Dispatcher has created {} session(s):\n{}",
                        dispatcher_sessions.len(),
                        sessions_table
                    )),
                    image_attachments: None,
                }])
            }
        }
    }
}
