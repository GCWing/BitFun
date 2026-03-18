use super::util::normalize_path;
use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::core::SessionConfig;
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use std::time::SystemTime;

/// SessionControl tool - create, delete, or list persisted sessions
pub struct SessionControlTool;

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
            normalize_path(&current_workspace.to_string_lossy().to_string());

        if normalized_current_workspace == workspace {
            Some(current_session_id)
        } else {
            None
        }
    }

    fn validate_session_id(session_id: &str) -> Result<(), String> {
        if session_id.is_empty() {
            return Err("session_id cannot be empty".to_string());
        }
        if session_id == "." || session_id == ".." {
            return Err("session_id cannot be '.' or '..'".to_string());
        }
        if session_id.contains('/') || session_id.contains('\\') {
            return Err("session_id cannot contain path separators".to_string());
        }
        if !session_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            return Err(
                "session_id can only contain ASCII letters, numbers, '-' and '_'".to_string(),
            );
        }
        Ok(())
    }

    fn resolve_workspace(&self, workspace: &str) -> BitFunResult<String> {
        let workspace = workspace.trim();
        if workspace.is_empty() {
            return Err(BitFunError::tool(
                "workspace is required and cannot be empty".to_string(),
            ));
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

    fn default_session_name() -> String {
        "New Session".to_string()
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
        Ok(format!("session-{}", creator_session_id))
    }

    fn build_list_result_for_assistant(
        &self,
        workspace: &str,
        sessions: &[crate::agentic::core::SessionSummary],
        current_session_id: Option<&str>,
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
        lines.push(
            "| session_id | session_name | agent_type | created_at | last_active_at |".to_string(),
        );
        lines.push("| --- | --- | --- | --- | --- |".to_string());
        for session in sessions {
            lines.push(format!(
                "| {} | {} | {} | {} | {} |",
                Self::escape_markdown_table_cell(&session.session_id),
                Self::escape_markdown_table_cell(&session.session_name),
                Self::escape_markdown_table_cell(&session.agent_type),
                Self::format_system_time(session.created_at),
                Self::format_system_time(session.last_activity_at),
            ));
        }
        lines.join("\n")
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SessionControlAction {
    Create,
    Delete,
    List,
}

#[derive(Debug, Clone, Deserialize)]
enum SessionControlAgentType {
    #[serde(rename = "agentic", alias = "Agentic", alias = "AGENTIC")]
    Agentic,
    #[serde(rename = "Plan", alias = "plan", alias = "PLAN")]
    Plan,
    #[serde(rename = "Cowork", alias = "cowork", alias = "COWORK")]
    Cowork,
}

impl SessionControlAgentType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Agentic => "agentic",
            Self::Plan => "Plan",
            Self::Cowork => "Cowork",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SessionControlInput {
    action: SessionControlAction,
    workspace: String,
    session_id: Option<String>,
    session_name: Option<String>,
    agent_type: Option<SessionControlAgentType>,
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
- "delete": Delete an existing session by session_id.
- "list": List all sessions.

Required inputs:
- "workspace": Absolute workspace path for the target session scope.

Optional inputs:
- "session_name": Only used by create. Defaults to "New Session".
- "agent_type": Only used by create. Defaults to "agentic".
  - "agentic": Coding-focused agent for implementation, debugging, and code changes.
  - "Plan": Planning agent for clarifying requirements and producing an implementation plan before coding.
  - "Cowork": Collaborative agent for office-style work such as research, documentation, presentations, etc."#
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "delete", "list"],
                    "description": "The session action to perform."
                },
                "workspace": {
                    "type": "string",
                    "description": "Required absolute workspace path."
                },
                "session_id": {
                    "type": "string",
                    "description": "Required for delete."
                },
                "session_name": {
                    "type": "string",
                    "description": "Optional display name when creating a session."
                },
                "agent_type": {
                    "type": "string",
                    "enum": ["agentic", "Plan", "Cowork"],
                    "description": "Optional agent type when creating a session. Defaults to agentic."
                }
            },
            "required": ["action", "workspace"],
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

        if parsed.workspace.trim().is_empty() {
            return ValidationResult {
                result: false,
                message: Some("workspace is required and cannot be empty".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        if !Path::new(parsed.workspace.trim()).is_absolute() {
            return ValidationResult {
                result: false,
                message: Some("workspace must be an absolute path".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        match parsed.action {
            SessionControlAction::Create => {
                if parsed.session_id.is_some() {
                    return ValidationResult {
                        result: false,
                        message: Some("session_id is not allowed for create".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                }
                if context
                    .and_then(|value| value.session_id.as_ref())
                    .is_none()
                {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "create requires a creator session in tool context".to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }
            }
            SessionControlAction::Delete => {
                if parsed.agent_type.is_some() {
                    return ValidationResult {
                        result: false,
                        message: Some("agent_type is only allowed for create".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                }
                let Some(session_id) = parsed.session_id.as_deref() else {
                    return ValidationResult {
                        result: false,
                        message: Some("session_id is required for delete".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                };
                if let Err(message) = Self::validate_session_id(session_id) {
                    return ValidationResult {
                        result: false,
                        message: Some(message),
                        error_code: Some(400),
                        meta: None,
                    };
                }
                if let Some(tool_context) = context {
                    if let Ok(workspace) = self.resolve_workspace(&parsed.workspace) {
                        if self.current_workspace_session(tool_context, &workspace)
                            == Some(session_id)
                        {
                            return ValidationResult {
                                result: false,
                                message: Some(
                                    "cannot delete the current session from SessionControl"
                                        .to_string(),
                                ),
                                error_code: Some(400),
                                meta: None,
                            };
                        }
                    }
                }
            }
            SessionControlAction::List => {
                if parsed.agent_type.is_some() {
                    return ValidationResult {
                        result: false,
                        message: Some("agent_type is only allowed for create".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                }
            }
        }

        ValidationResult::default()
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let action = input
            .get("action")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        let workspace = input
            .get("workspace")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown workspace");
        let session_id = input
            .get("session_id")
            .and_then(|value| value.as_str())
            .unwrap_or("auto");

        match action {
            "create" => format!("Create session in {}", workspace),
            "delete" => format!("Delete session {} in {}", session_id, workspace),
            "list" => format!("List sessions in {}", workspace),
            _ => format!("Manage sessions in {}", workspace),
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: SessionControlInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::tool(format!("Invalid input: {}", e)))?;
        let workspace = self.resolve_workspace(&params.workspace)?;
        let workspace_path = Path::new(&workspace);
        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;

        match params.action {
            SessionControlAction::Create => {
                let session_name = params
                    .session_name
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(Self::default_session_name);
                let agent_type = params
                    .agent_type
                    .as_ref()
                    .map(|agent_type| agent_type.as_str().to_string())
                    .unwrap_or_else(|| "agentic".to_string());
                let created_by = self.creator_session_marker(context)?;

                let session = coordinator
                    .create_session_with_workspace_and_creator(
                        None,
                        session_name,
                        agent_type,
                        SessionConfig {
                            workspace_path: Some(workspace.clone()),
                            ..Default::default()
                        },
                        workspace.clone(),
                        Some(created_by.clone()),
                    )
                    .await?;
                let created_session_id = session.session_id.clone();
                let created_session_name = session.session_name.clone();
                let created_agent_type = session.agent_type.clone();
                let result_for_assistant = format!(
                    "Created session '{}' in workspace '{}' using agent type '{}'.",
                    created_session_id, workspace, created_agent_type
                );

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "create",
                        "workspace": workspace.clone(),
                        "session": {
                            "session_id": created_session_id,
                            "session_name": created_session_name,
                            "agent_type": created_agent_type,
                        }
                    }),
                    result_for_assistant: Some(result_for_assistant),
                }])
            }
            SessionControlAction::Delete => {
                let session_id = params.session_id.as_deref().ok_or_else(|| {
                    BitFunError::tool("session_id is required for delete".to_string())
                })?;
                Self::validate_session_id(session_id).map_err(BitFunError::tool)?;
                if self.current_workspace_session(context, &workspace) == Some(session_id) {
                    return Err(BitFunError::tool(
                        "cannot delete the current session from SessionControl".to_string(),
                    ));
                }

                let existing_sessions = coordinator.list_sessions(workspace_path).await?;
                if !existing_sessions
                    .iter()
                    .any(|session| session.session_id == session_id)
                {
                    return Err(BitFunError::NotFound(format!(
                        "Session not found in workspace: {}",
                        session_id
                    )));
                }

                coordinator
                    .delete_session(workspace_path, session_id)
                    .await?;

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "delete",
                        "workspace": workspace.clone(),
                        "session_id": session_id,
                    }),
                    result_for_assistant: Some(format!(
                        "Deleted session '{}' from workspace '{}'.",
                        session_id, workspace
                    )),
                }])
            }
            SessionControlAction::List => {
                let sessions = coordinator.list_sessions(workspace_path).await?;
                let current_session_id = self.current_workspace_session(context, &workspace);
                let result_for_assistant =
                    self.build_list_result_for_assistant(&workspace, &sessions, current_session_id);

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "list",
                        "workspace": workspace.clone(),
                        "current_session_id": current_session_id,
                        "count": sessions.len(),
                        "sessions": sessions,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                }])
            }
        }
    }
}
