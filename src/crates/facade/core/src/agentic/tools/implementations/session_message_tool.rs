use super::util::normalize_path;
use crate::agentic::coordination::{
    get_global_coordinator, get_global_scheduler, AgentSessionReplyRoute, DialogSubmissionPolicy,
    DialogTriggerSource,
};
use crate::agentic::core::{InternalReminderKind, Message, SessionConfig};
use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::workspace_paths::posix_style_path_is_absolute;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

/// SessionMessage tool - send a message to another session via the dialog scheduler
pub struct SessionMessageTool;

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

    fn format_forwarded_message(&self, message: &str) -> (String, Vec<Message>) {
        (
            message.to_string(),
            vec![Message::internal_reminder(
                InternalReminderKind::SessionMessageRequest,
                "This request was sent by another agent, not human user. Do not use interactive tools for this request. In particular, do not call AskUserQuestion."
                    .to_string(),
            )],
        )
    }
}

#[derive(Debug, Clone, Deserialize)]
enum SessionMessageAgentType {
    #[serde(rename = "agentic", alias = "Agentic", alias = "AGENTIC")]
    Agentic,
    #[serde(rename = "Plan", alias = "plan", alias = "PLAN")]
    Plan,
    #[serde(rename = "Cowork", alias = "cowork", alias = "COWORK")]
    Cowork,
}

impl SessionMessageAgentType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Agentic => "agentic",
            Self::Plan => "Plan",
            Self::Cowork => "Cowork",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SessionMessageInput {
    workspace: Option<String>,
    session_id: Option<String>,
    session_name: Option<String>,
    message: String,
    agent_type: Option<SessionMessageAgentType>,
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

Allowed agent types when creating a session:
- "agentic": Coding-focused agent for implementation, debugging, and code changes.
- "Plan": Planning agent for clarifying requirements and producing an implementation plan before coding.
- "Cowork": Collaborative agent for office-style work such as research, documentation, presentations, etc.
"#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Send a message to another agent session and receive the result asynchronously.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Collapsed
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
                    "enum": ["agentic", "Plan", "Cowork"],
                    "description": "Required when session_id is omitted. Not allowed when sending to an existing session."
                }
            },
            "required": ["message"],
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

        if parsed.message.trim().is_empty() {
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

                if let Some(workspace) = parsed.workspace.as_deref() {
                    let workspace_validation = self.validate_workspace_shape(workspace, context);
                    if !workspace_validation.result {
                        return workspace_validation;
                    }
                }
            }
            None => {
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
        let source_session_id = self.sender_session_id(context)?.to_string();
        let source_workspace = self.sender_workspace(context)?;

        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;
        let scheduler = get_global_scheduler()
            .ok_or_else(|| BitFunError::tool("scheduler not initialized".to_string()))?;

        let (target_session_id, target_agent_type, created_session_id, workspace) =
            if let Some(target_session_id) = params.session_id.clone() {
                if source_session_id == target_session_id {
                    return Err(BitFunError::tool(
                        "SessionMessage cannot send a message to the same session".to_string(),
                    ));
                }

                let workspace = if let Some(workspace) = params.workspace.as_deref() {
                    self.resolve_workspace(workspace, context)?
                } else {
                    coordinator
                        .resolve_session_workspace_path(&target_session_id)
                        .await
                        .map(|path| path.to_string_lossy().to_string())
                        .ok_or_else(|| {
                            BitFunError::NotFound(format!(
                                "Workspace for session '{}' could not be resolved",
                                target_session_id
                            ))
                        })?
                };
                let workspace_path = Path::new(&workspace);
                let existing_sessions = coordinator.list_sessions(workspace_path).await?;
                let target_session = existing_sessions
                    .iter()
                    .find(|session| session.session_id == target_session_id.as_str())
                    .ok_or_else(|| {
                        BitFunError::NotFound(format!(
                            "Session '{}' not found in workspace '{}'",
                            target_session_id, workspace
                        ))
                    })?;

                let persisted_agent_type = target_session.agent_type.trim();
                let target_agent_type = if persisted_agent_type.is_empty() {
                    "agentic".to_string()
                } else {
                    persisted_agent_type.to_string()
                };

                (target_session_id, target_agent_type, None, workspace)
            } else {
                let workspace = self.resolve_workspace(
                    params.workspace.as_deref().ok_or_else(|| {
                        BitFunError::tool(
                            "workspace is required when session_id is omitted".to_string(),
                        )
                    })?,
                    context,
                )?;
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
                let created_by = self.creator_session_marker(context)?;
                let session = coordinator
                    .create_session_with_workspace_and_creator(
                        None,
                        session_name,
                        agent_type.clone(),
                        SessionConfig {
                            workspace_path: Some(workspace.clone()),
                            ..Default::default()
                        },
                        workspace.clone(),
                        Some(created_by),
                    )
                    .await?;

                (
                    session.session_id.clone(),
                    session.agent_type.clone(),
                    Some(session.session_id),
                    workspace,
                )
            };

        let (forwarded_message, prepended_messages) =
            self.format_forwarded_message(&params.message);

        scheduler
            .submit_with_prepended_messages(
                target_session_id.clone(),
                forwarded_message,
                Some(params.message.clone()),
                None,
                target_agent_type.clone(),
                Some(workspace.clone()),
                DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
                Some(AgentSessionReplyRoute {
                    source_session_id,
                    source_workspace_path: source_workspace,
                }),
                None,
                prepended_messages,
                None,
            )
            .await
            .map_err(BitFunError::tool)?;

        Ok(vec![ToolResult::Result {
            data: json!({
                "success": true,
                "target_workspace": workspace.clone(),
                "target_session_id": target_session_id.clone(),
                "target_agent_type": target_agent_type.clone(),
                "created_session_id": created_session_id.clone(),
            }),
            result_for_assistant: Some(if let Some(created_session_id) = created_session_id {
                format!(
                    "Created session '{}' and accepted the message in workspace '{}' using agent type '{}'.",
                    created_session_id, workspace, target_agent_type
                )
            } else {
                format!(
                    "Message accepted for session '{}' in workspace '{}' using agent type '{}'.",
                    target_session_id, workspace, target_agent_type
                )
            }),
            image_attachments: None,
        }])
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
            unlocked_collapsed_tools: Vec::new(),
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
                    "agent_type": "Plan",
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
                    "agent_type": "agentic",
                }),
                Some(&session_context("source_1")),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
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
}
