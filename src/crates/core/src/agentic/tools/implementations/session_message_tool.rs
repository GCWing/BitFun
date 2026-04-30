use super::util::normalize_path;
use crate::agentic::coordination::{
    get_global_coordinator, get_global_scheduler, AgentSessionReplyRoute, DialogSubmissionPolicy,
    DialogTriggerSource,
};
use crate::agentic::core::PromptEnvelope;
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
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

    fn format_forwarded_message(&self, message: &str) -> String {
        let mut envelope = PromptEnvelope::new();
        envelope.push_system_reminder(
            "This request was sent by another agent, not human user. Do not use interactive tools for this request. In particular, do not call AskUserQuestion."
                .to_string(),
        );
        envelope.push_user_query(message.to_string());
        envelope.render()
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

    fn from_str(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case("agentic") {
            Some(Self::Agentic)
        } else if value.eq_ignore_ascii_case("plan") {
            Some(Self::Plan)
        } else if value.eq_ignore_ascii_case("cowork") {
            Some(Self::Cowork)
        } else {
            None
        }
    }

    fn is_coding_mode(&self) -> bool {
        matches!(self, Self::Agentic | Self::Plan)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SessionMessageInput {
    workspace: String,
    session_id: String,
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
            
You must provide the target workspace as an absolute path, and you can optionally set agent_type to choose how the target session handles the request:
- "agentic": Coding-focused agent for implementation, debugging, and code changes.
- "Plan": Planning agent for clarifying requirements and producing an implementation plan before coding.
- "Cowork": Collaborative agent for office-style work such as research, documentation, presentations, etc.

When overriding an existing session's agent_type, only switching between "agentic" and "Plan" is allowed. It will not switch coding sessions to or from "Cowork"."#
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "workspace": {
                    "type": "string",
                    "description": "Required absolute target workspace path."
                },
                "session_id": {
                    "type": "string",
                    "description": "Target session ID."
                },
                "message": {
                    "type": "string",
                    "description": "Message to send to the target session."
                },
                "agent_type": {
                    "type": "string",
                    "enum": ["agentic", "Plan", "Cowork"],
                    "description": "Optional target agent type. Defaults to the target session's current agent type."
                }
            },
            "required": ["workspace", "session_id", "message"],
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

        if let Err(message) = Self::validate_session_id(&parsed.session_id) {
            return ValidationResult {
                result: false,
                message: Some(message),
                error_code: Some(400),
                meta: None,
            };
        }

        if parsed.message.trim().is_empty() {
            return ValidationResult {
                result: false,
                message: Some("message cannot be empty".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        if parsed.workspace.trim().is_empty() {
            return ValidationResult {
                result: false,
                message: Some("workspace is required and cannot be empty".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        let Some(context) = context else {
            if !Path::new(parsed.workspace.trim()).is_absolute()
                && !posix_style_path_is_absolute(parsed.workspace.trim())
            {
                return ValidationResult {
                    result: false,
                    message: Some("workspace must be an absolute path".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
            return ValidationResult::default();
        };

        let ws_ok = if context.is_remote() {
            posix_style_path_is_absolute(parsed.workspace.trim())
        } else {
            Path::new(parsed.workspace.trim()).is_absolute()
        };
        if !ws_ok {
            return ValidationResult {
                result: false,
                message: Some("workspace must be an absolute path".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

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

        if source_session_id == parsed.session_id {
            return ValidationResult {
                result: false,
                message: Some(
                    "SessionMessage cannot send a message to the same session".to_string(),
                ),
                error_code: Some(400),
                meta: None,
            };
        }

        ValidationResult::default()
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let workspace = input
            .get("workspace")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown workspace");
        let session_id = input
            .get("session_id")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");

        format!("Send message to session {} in {}", session_id, workspace)
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: SessionMessageInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::tool(format!("Invalid input: {}", e)))?;
        let workspace = self.resolve_workspace(&params.workspace, context)?;
        let workspace_path = Path::new(&workspace);
        let source_session_id = self.sender_session_id(context)?.to_string();
        let target_session_id = params.session_id.clone();

        if source_session_id == target_session_id {
            return Err(BitFunError::tool(
                "SessionMessage cannot send a message to the same session".to_string(),
            ));
        }

        let source_workspace = self.sender_workspace(context)?;

        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;
        let scheduler = get_global_scheduler()
            .ok_or_else(|| BitFunError::tool("scheduler not initialized".to_string()))?;

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
        let target_agent_type = if let Some(requested_agent_type) = params.agent_type.as_ref() {
            let current_agent_type = if persisted_agent_type.is_empty() {
                SessionMessageAgentType::Agentic
            } else {
                SessionMessageAgentType::from_str(persisted_agent_type).ok_or_else(|| {
                    BitFunError::tool(format!(
                        "SessionMessage agent_type override is only supported for sessions using 'agentic', 'Plan', or 'Cowork'. Current agent type is '{}'.",
                        persisted_agent_type
                    ))
                })?
            };

            if requested_agent_type.as_str() != current_agent_type.as_str()
                && !(requested_agent_type.is_coding_mode() && current_agent_type.is_coding_mode())
            {
                return Err(BitFunError::tool(format!(
                    "SessionMessage only allows agent_type override between 'agentic' and 'Plan'. Cannot switch session '{}' from '{}' to '{}'.",
                    target_session_id,
                    current_agent_type.as_str(),
                    requested_agent_type.as_str()
                )));
            }

            requested_agent_type.as_str().to_string()
        } else if persisted_agent_type.is_empty() {
            "agentic".to_string()
        } else {
            persisted_agent_type.to_string()
        };

        let forwarded_message = self.format_forwarded_message(&params.message);

        scheduler
            .submit(
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
            }),
            result_for_assistant: Some(format!(
                "Message accepted for session '{}' in workspace '{}' using agent type '{}'.",
                target_session_id, workspace, target_agent_type
            )),
            image_attachments: None,
        }])
    }
}
