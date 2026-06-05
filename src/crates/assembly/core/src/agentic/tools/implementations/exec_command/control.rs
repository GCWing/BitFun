use super::rendering::render_exec_response_for_assistant;
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::service::remote_ssh::{
    get_global_remote_exec_process_manager, RemoteExecControlAction, RemoteExecControlRequest,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use terminal_core::{
    get_global_exec_process_manager, LocalExecControlAction, LocalExecControlRequest,
};

const DEFAULT_MAX_OUTPUT_CHARS: u64 = 10_000;

// ExecControl termination semantics by execution surface:
//
// Local workspace:
// - tty=true: interrupt writes Ctrl+C to the PTY; kill uses the PTY child killer.
// - tty=false, Windows: interrupt and kill both terminate the process tree via
//   taskkill /T /F, with child.kill() as fallback. This is intentionally
//   forceful because Windows pipe-mode Ctrl+C delivery is not reliable.
// - tty=false, Unix: the pipe child starts in its own session/process group.
//   interrupt sends SIGINT to that group, waits a short grace window, then
//   sends SIGKILL to clean up descendants; kill sends SIGKILL to the group.
//
// Remote SSH workspace:
// - tty=true: interrupt writes Ctrl+C to the remote PTY; kill sends an SSH
//   SIGKILL request and closes the channel after a short drain.
// - tty=false, Unix/POSIX SSH host: ExecCommand wraps the user command in a
//   remote process-group owner. interrupt asks the wrapper to send SIGINT,
//   wait its grace window, then SIGKILL the remote process group. kill sends a
//   catchable TERM to the wrapper, which immediately SIGKILLs the remote group.
// - Remote Windows SSH hosts are not part of the current ExecCommand contract;
//   remote workspaces assume POSIX paths and shells.
pub struct ExecControlTool;

impl Default for ExecControlTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecControlTool {
    pub fn new() -> Self {
        Self
    }

    fn session_id_from_input(input: &Value) -> Option<i32> {
        input.get("session_id").and_then(|value| {
            value
                .as_i64()
                .and_then(|id| i32::try_from(id).ok())
                .or_else(|| value.as_u64().and_then(|id| i32::try_from(id).ok()))
        })
    }

    fn action_from_input(input: &Value) -> Option<LocalExecControlAction> {
        match input.get("action").and_then(Value::as_str)?.trim() {
            "interrupt" => Some(LocalExecControlAction::Interrupt),
            "kill" => Some(LocalExecControlAction::Kill),
            _ => None,
        }
    }

    fn response_for_assistant(data: &Value, action: LocalExecControlAction) -> String {
        let mut status_lines = Vec::new();
        match action {
            LocalExecControlAction::Interrupt => {
                status_lines.push("Sent interrupt to process.".to_string())
            }
            LocalExecControlAction::Kill => status_lines.push("Sent kill to process.".to_string()),
        }
        if let Some(exit_code) = data.get("exit_code").and_then(Value::as_i64) {
            status_lines.push(format!("Process exited with code {exit_code}."));
        } else if let Some(session_id) = data.get("session_id").and_then(Value::as_i64) {
            status_lines.push(format!(
                "Process is still running. session_id: {session_id}"
            ));
        }
        render_exec_response_for_assistant(data, status_lines, 4)
    }

    fn remote_action(action: LocalExecControlAction) -> RemoteExecControlAction {
        match action {
            LocalExecControlAction::Interrupt => RemoteExecControlAction::Interrupt,
            LocalExecControlAction::Kill => RemoteExecControlAction::Kill,
        }
    }

    async fn call_remote_pipe(&self, input: &Value) -> BitFunResult<Vec<ToolResult>> {
        let session_id = Self::session_id_from_input(input).ok_or_else(|| {
            BitFunError::tool("session_id is required for ExecControl".to_string())
        })?;
        let action = Self::action_from_input(input).ok_or_else(|| {
            BitFunError::tool("action must be either 'interrupt' or 'kill'".to_string())
        })?;
        let yield_time_ms = input.get("yield_time_ms").and_then(Value::as_u64);
        let max_output_chars = input
            .get("max_output_chars")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_MAX_OUTPUT_CHARS)
            .try_into()
            .unwrap_or(usize::MAX);

        let response = get_global_remote_exec_process_manager()
            .control_session(RemoteExecControlRequest {
                session_id,
                action: Self::remote_action(action),
                yield_time_ms,
                max_output_chars: Some(max_output_chars),
            })
            .await
            .map_err(|error| BitFunError::tool(format!("ExecControl failed: {error}")))?;

        let action_name = match action {
            LocalExecControlAction::Interrupt => "interrupt",
            LocalExecControlAction::Kill => "kill",
        };
        let data = json!({
            "chunk_id": response.chunk_id,
            "wall_time_seconds": response.wall_time_seconds,
            "output": response.output,
            "session_id": response.session_id,
            "exit_code": response.exit_code,
            "original_output_chars": response.original_output_chars,
            "action": action_name,
            "remote": true,
        });
        let result_for_assistant = Self::response_for_assistant(&data, action);

        Ok(vec![ToolResult::Result {
            data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}

#[async_trait]
impl Tool for ExecControlTool {
    fn name(&self) -> &str {
        "ExecControl"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Interrupts or kills a running ExecCommand session.

Pass the session_id returned by ExecCommand.
Use action="interrupt" when a command should stop gracefully, like pressing Ctrl+C. Use action="kill" when the process must be terminated.
After the action, yield_time_ms waits for output or exit status. Output is only what was produced during this tool call's wait window."#
            .to_string())
    }

    fn short_description(&self) -> String {
        "Interrupt or kill a running ExecCommand session.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "number",
                    "description": "session_id returned by ExecCommand while a process is still running."
                },
                "action": {
                    "type": "string",
                    "enum": ["interrupt", "kill"],
                    "description": "Use interrupt to stop gracefully; use kill to force termination."
                },
                "yield_time_ms": {
                    "type": "number",
                    "description": "How long to wait for output after the control action before yielding."
                },
                "max_output_chars": {
                    "type": "number",
                    "description": "Maximum output characters to return. Defaults to 10000; excess output keeps head and tail."
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
        false
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        true
    }

    fn manages_own_execution_timeout(&self) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        if Self::session_id_from_input(input).is_none() {
            return ValidationResult {
                result: false,
                message: Some("session_id is required for ExecControl".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }
        if Self::action_from_input(input).is_none() {
            return ValidationResult {
                result: false,
                message: Some("action must be either 'interrupt' or 'kill'".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }
        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        if context.is_remote() {
            return self.call_remote_pipe(input).await;
        }

        let session_id = Self::session_id_from_input(input).ok_or_else(|| {
            BitFunError::tool("session_id is required for ExecControl".to_string())
        })?;
        let action = Self::action_from_input(input).ok_or_else(|| {
            BitFunError::tool("action must be either 'interrupt' or 'kill'".to_string())
        })?;
        let yield_time_ms = input.get("yield_time_ms").and_then(Value::as_u64);
        let max_output_chars = input
            .get("max_output_chars")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_MAX_OUTPUT_CHARS)
            .try_into()
            .unwrap_or(usize::MAX);

        let response = get_global_exec_process_manager()
            .control_session(LocalExecControlRequest {
                session_id,
                action,
                yield_time_ms,
                max_output_chars: Some(max_output_chars),
            })
            .await
            .map_err(|error| BitFunError::tool(format!("ExecControl failed: {error}")))?;

        let action_name = match action {
            LocalExecControlAction::Interrupt => "interrupt",
            LocalExecControlAction::Kill => "kill",
        };
        let data = json!({
            "chunk_id": response.chunk_id,
            "wall_time_seconds": response.wall_time_seconds,
            "output": response.output,
            "session_id": response.session_id,
            "exit_code": response.exit_code,
            "original_output_chars": response.original_output_chars,
            "action": action_name,
        });
        let result_for_assistant = Self::response_for_assistant(&data, action);

        Ok(vec![ToolResult::Result {
            data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}
