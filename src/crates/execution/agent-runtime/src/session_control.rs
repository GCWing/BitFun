//! Portable SessionControl tool decisions.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionControlAction {
    Create,
    Cancel,
    Delete,
    List,
}

impl SessionControlAction {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Cancel => "cancel",
            Self::Delete => "delete",
            Self::List => "list",
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub enum SessionControlAgentType {
    #[serde(rename = "agentic", alias = "Agentic", alias = "AGENTIC")]
    Agentic,
    #[serde(rename = "Plan", alias = "plan", alias = "PLAN")]
    Plan,
    #[serde(rename = "Cowork", alias = "cowork", alias = "COWORK")]
    Cowork,
}

impl SessionControlAgentType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Agentic => "agentic",
            Self::Plan => "Plan",
            Self::Cowork => "Cowork",
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SessionControlInput {
    pub action: SessionControlAction,
    pub workspace: Option<String>,
    pub session_id: Option<String>,
    pub session_name: Option<String>,
    pub agent_type: Option<SessionControlAgentType>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SessionControlValidationContext<'a> {
    pub current_session_id: Option<&'a str>,
    pub has_workspace_root: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionControlValidationResult {
    pub result: bool,
    pub message: Option<String>,
    pub error_code: Option<i32>,
    pub meta: Option<Value>,
}

impl Default for SessionControlValidationResult {
    fn default() -> Self {
        Self {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionControlCancelRoute {
    RequesterViaScheduler { requester_session_id: String },
    CoordinatorDirect,
}

pub fn resolve_session_control_cancel_route(
    requester_session_id: Option<&str>,
    scheduler_available: bool,
) -> SessionControlCancelRoute {
    match (requester_session_id, scheduler_available) {
        (Some(requester_session_id), true) => SessionControlCancelRoute::RequesterViaScheduler {
            requester_session_id: requester_session_id.to_string(),
        },
        _ => SessionControlCancelRoute::CoordinatorDirect,
    }
}

fn invalid(message: impl Into<String>) -> SessionControlValidationResult {
    SessionControlValidationResult {
        result: false,
        message: Some(message.into()),
        error_code: Some(400),
        meta: None,
    }
}

pub fn validate_session_id(session_id: &str) -> Result<(), String> {
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
        return Err("session_id can only contain ASCII letters, numbers, '-' and '_'".to_string());
    }
    Ok(())
}

pub fn default_session_name() -> &'static str {
    "New Session"
}

pub fn session_control_session_name_or_default(session_name: Option<&str>) -> String {
    session_name
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(default_session_name())
        .to_string()
}

pub fn session_control_agent_type_or_default(
    agent_type: Option<&SessionControlAgentType>,
) -> String {
    agent_type
        .map(|agent_type| agent_type.as_str().to_string())
        .unwrap_or_else(|| "agentic".to_string())
}

pub fn session_control_creator_marker(creator_session_id: &str) -> String {
    format!("session-{creator_session_id}")
}

fn validate_workspace_shape(workspace: &str) -> SessionControlValidationResult {
    if workspace.trim().is_empty() {
        return invalid("workspace is required and cannot be empty");
    }

    if !Path::new(workspace.trim()).is_absolute() {
        return invalid("workspace must be an absolute path");
    }

    SessionControlValidationResult::default()
}

fn validate_mutating_action_target(
    action: &SessionControlAction,
    input: &SessionControlInput,
    context: SessionControlValidationContext<'_>,
) -> SessionControlValidationResult {
    if input.agent_type.is_some() {
        return invalid("agent_type is only allowed for create");
    }
    if input.session_name.is_some() {
        return invalid("session_name is only allowed for create");
    }

    let Some(session_id) = input.session_id.as_deref() else {
        return invalid(format!("session_id is required for {}", action.as_str()));
    };
    if let Err(message) = validate_session_id(session_id) {
        return invalid(message);
    }

    if context.current_session_id == Some(session_id) && context.has_workspace_root {
        return invalid(format!(
            "cannot {} the current session from SessionControl",
            action.as_str()
        ));
    }

    SessionControlValidationResult::default()
}

pub fn validate_session_control_input(
    input: &SessionControlInput,
    context: SessionControlValidationContext<'_>,
) -> SessionControlValidationResult {
    if let Some(workspace) = input.workspace.as_deref() {
        let should_validate_workspace = matches!(
            input.action,
            SessionControlAction::Create | SessionControlAction::List
        );
        if !should_validate_workspace {
            return validate_mutating_action_target(&input.action, input, context);
        }

        let workspace_validation = validate_workspace_shape(workspace);
        if !workspace_validation.result {
            return workspace_validation;
        }
    }

    match input.action {
        SessionControlAction::Create => {
            if input.workspace.is_none() {
                return invalid("workspace is required for create");
            }
            if input.session_id.is_some() {
                return invalid("session_id is not allowed for create");
            }
            if context.current_session_id.is_none() {
                return invalid("create requires a creator session in tool context");
            }
        }
        SessionControlAction::Cancel | SessionControlAction::Delete => {
            return validate_mutating_action_target(&input.action, input, context);
        }
        SessionControlAction::List => {
            if input.workspace.is_none() {
                return invalid("workspace is required for list");
            }
            if input.agent_type.is_some() {
                return invalid("agent_type is only allowed for create");
            }
            if input.session_name.is_some() {
                return invalid("session_name is only allowed for create");
            }
            if input.session_id.is_some() {
                return invalid("session_id is not allowed for list");
            }
        }
    }

    SessionControlValidationResult::default()
}

pub fn render_session_control_tool_use_message(input: &Value) -> String {
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
        "create" => format!("Create session in {workspace}"),
        "cancel" => format!("Cancel active turn for session {session_id}"),
        "delete" => format!("Delete session {session_id}"),
        "list" => format!("List sessions in {workspace}"),
        _ => format!("Manage sessions in {workspace}"),
    }
}

pub fn session_control_created_result_message(
    session_id: &str,
    workspace: &str,
    agent_type: &str,
) -> String {
    format!("Created session '{session_id}' in workspace '{workspace}' using agent type '{agent_type}'.")
}

pub fn session_control_cancel_status(cancelled_turn_id: Option<&str>) -> &'static str {
    if cancelled_turn_id.is_some() {
        "cancel_requested"
    } else {
        "no_active_turn"
    }
}

pub fn session_control_cancel_result_message(
    session_id: &str,
    workspace: &str,
    cancelled_turn_id: Option<&str>,
) -> String {
    if let Some(turn_id) = cancelled_turn_id {
        format!(
            "Cancellation requested for the active turn '{turn_id}' in session '{session_id}' within workspace '{workspace}'. The session remains available for future work, and queued messages are not cleared."
        )
    } else {
        format!(
            "Session '{session_id}' in workspace '{workspace}' has no active turn to cancel. The session remains available for future work."
        )
    }
}

pub fn session_control_deleted_result_message(session_id: &str, workspace: &str) -> String {
    format!("Deleted session '{session_id}' from workspace '{workspace}'.")
}
