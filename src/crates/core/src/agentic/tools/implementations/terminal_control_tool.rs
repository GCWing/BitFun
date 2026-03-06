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

The session_id is returned inside <session_id>...</session_id> tags in BashTool results."#
            .to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "The ID of the terminal session to control."
                },
                "action": {
                    "type": "string",
                    "enum": ["kill", "interrupt"],
                    "description": "The action to perform: 'kill' closes the session permanently; 'interrupt' cancels the running process."
                }
            },
            "required": ["session_id", "action"],
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

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        if input.get("session_id").and_then(|v| v.as_str()).is_none() {
            return ValidationResult {
                result: false,
                message: Some("session_id is required".to_string()),
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
        let session_id = input
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        match action {
            "kill" => format!("Kill terminal session: {}", session_id),
            "interrupt" => format!("Interrupt terminal session: {}", session_id),
            _ => format!("Control terminal session: {}", session_id),
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let session_id = input
            .get("session_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("session_id is required".to_string()))?;

        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("action is required".to_string()))?;

        let terminal_api = TerminalApi::from_singleton()
            .map_err(|e| BitFunError::tool(format!("Terminal not initialized: {}", e)))?;

        match action {
            "interrupt" => {
                debug!("TerminalControl: sending SIGINT to session {}", session_id);

                terminal_api
                    .signal(SignalRequest {
                        session_id: session_id.to_string(),
                        signal: "SIGINT".to_string(),
                    })
                    .await
                    .map_err(|e| {
                        BitFunError::tool(format!("Failed to interrupt terminal session: {}", e))
                    })?;

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "session_id": session_id,
                        "action": "interrupt",
                    }),
                    result_for_assistant: Some(format!(
                        "Sent interrupt (SIGINT) to terminal session '{}'.",
                        session_id
                    )),
                }])
            }

            "kill" => {
                // Determine if this is a primary (persistent) session by checking the binding.
                // For primary sessions, owner_id == terminal_session_id, so binding.get(session_id)
                // returns Some(session_id) when the session is primary.
                let binding = terminal_api.session_manager().binding();
                let is_primary = binding
                    .get(session_id)
                    .map(|bound_id| bound_id == session_id)
                    .unwrap_or(false);

                debug!(
                    "TerminalControl: killing session {}, is_primary={}",
                    session_id, is_primary
                );

                if is_primary {
                    binding.remove(session_id).await.map_err(|e| {
                        BitFunError::tool(format!("Failed to close terminal session: {}", e))
                    })?;
                } else {
                    terminal_api
                        .close_session(CloseSessionRequest {
                            session_id: session_id.to_string(),
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
                        session_id
                    )
                } else {
                    format!(
                        "Background terminal session '{}' has been killed.",
                        session_id
                    )
                };

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "session_id": session_id,
                        "action": "kill",
                    }),
                    result_for_assistant: Some(result_for_assistant),
                }])
            }

            _ => Err(BitFunError::tool(format!(
                "Unknown action: '{}'. Must be 'kill' or 'interrupt'.",
                action
            ))),
        }
    }
}
