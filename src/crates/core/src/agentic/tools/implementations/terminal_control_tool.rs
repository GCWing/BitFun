use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use log::debug;
use serde_json::{json, Value};
use terminal_core::{CloseSessionRequest, SignalRequest, TerminalApi};

/// TerminalControl tool - kill or interrupt a terminal session
pub struct TerminalControlTool;

impl Default for TerminalControlTool {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalControlTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for TerminalControlTool {
    fn name(&self) -> &str {
        "TerminalControl"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Control a terminal session by performing a kill or interrupt action.

Actions:
- "kill": Permanently close a terminal session. When to use:
  1. Clean up terminals that are no longer needed (e.g., after stopping a server or when a long-running task completes).
  2. Close the persistent shell used by BashTool - if BashTool output appears clearly abnormal (e.g., garbled output, stuck prompts, corrupted shell state), use this to forcefully close the persistent shell. The next BashTool invocation will automatically create a fresh shell session.
- "interrupt": Cancel the currently running process without closing the session.

The terminal_session_id is returned inside <terminal_session_id>...</terminal_session_id> tags in BashTool results."#
            .to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "terminal_session_id": {
                    "type": "string",
                    "description": "The ID of the terminal session to control."
                },
                "action": {
                    "type": "string",
                    "enum": ["kill", "interrupt"],
                    "description": "The action to perform: 'kill' closes the session permanently; 'interrupt' cancels the running process."
                }
            },
            "required": ["terminal_session_id", "action"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn is_available_in_context(&self, context: Option<&ToolUseContext>) -> bool {
        !context.map(|ctx| ctx.is_remote()).unwrap_or(false)
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        if input
            .get("terminal_session_id")
            .and_then(|v| v.as_str())
            .is_none()
        {
            return ValidationResult {
                result: false,
                message: Some("terminal_session_id is required".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }
        match input.get("action").and_then(|v| v.as_str()) {
            Some("kill") | Some("interrupt") => {}
            _ => {
                return ValidationResult {
                    result: false,
                    message: Some("action must be one of: \"kill\", \"interrupt\"".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }
        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let terminal_session_id = input
            .get("terminal_session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        match action {
            "kill" => format!("Kill terminal session: {}", terminal_session_id),
            "interrupt" => format!("Interrupt terminal session: {}", terminal_session_id),
            _ => format!("Control terminal session: {}", terminal_session_id),
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let terminal_session_id = input
            .get("terminal_session_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("terminal_session_id is required".to_string()))?;

        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("action is required".to_string()))?;

        let terminal_api = TerminalApi::from_singleton()
            .map_err(|e| BitFunError::tool(format!("Terminal not initialized: {}", e)))?;

        match action {
            "interrupt" => {
                debug!(
                    "TerminalControl: sending SIGINT to session {}",
                    terminal_session_id
                );

                terminal_api
                    .signal(SignalRequest {
                        session_id: terminal_session_id.to_string(),
                        signal: "SIGINT".to_string(),
                    })
                    .await
                    .map_err(|e| {
                        BitFunError::tool(format!("Failed to interrupt terminal session: {}", e))
                    })?;

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "terminal_session_id": terminal_session_id,
                        "action": "interrupt",
                    }),
                    result_for_assistant: Some(format!(
                        "Sent interrupt (SIGINT) to terminal session '{}'.",
                        terminal_session_id
                    )),
                    image_attachments: None,
                }])
            }

            "kill" => {
                // Determine if this is a primary (persistent) session by checking the binding.
                // For primary sessions, owner_id == terminal_session_id, so
                // binding.get(terminal_session_id) returns Some(terminal_session_id)
                // when the session is primary.
                let binding = terminal_api.session_manager().binding();
                let is_primary = binding
                    .get(terminal_session_id)
                    .map(|bound_id| bound_id == terminal_session_id)
                    .unwrap_or(false);

                debug!(
                    "TerminalControl: killing session {}, is_primary={}",
                    terminal_session_id, is_primary
                );

                if is_primary {
                    binding.remove(terminal_session_id).await.map_err(|e| {
                        BitFunError::tool(format!("Failed to close terminal session: {}", e))
                    })?;
                } else {
                    terminal_api
                        .close_session(CloseSessionRequest {
                            session_id: terminal_session_id.to_string(),
                            immediate: Some(true),
                        })
                        .await
                        .map_err(|e| {
                            BitFunError::tool(format!("Failed to close terminal session: {}", e))
                        })?;
                }

                let result_for_assistant = if is_primary {
                    format!(
                        "Terminal session '{}' has been killed. The next Bash tool call will automatically create a new persistent shell session.",
                        terminal_session_id
                    )
                } else {
                    format!(
                        "Background terminal session '{}' has been killed.",
                        terminal_session_id
                    )
                };

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "terminal_session_id": terminal_session_id,
                        "action": "kill",
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }

            _ => Err(BitFunError::tool(format!(
                "Unknown action: '{}'. Must be 'kill' or 'interrupt'.",
                action
            ))),
        }
    }
}
