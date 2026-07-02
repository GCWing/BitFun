use super::completion::{exec_command_local_completion, exec_command_remote_completion};
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::service::remote_ssh::{
    get_global_remote_exec_process_manager, RemoteExecControlAction, RemoteExecControlOrigin,
    RemoteExecControlRequest, RemoteExecError,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use terminal_core::{
    get_global_exec_process_manager, LocalExecControlAction, LocalExecControlOrigin,
    LocalExecControlRequest, TerminalError,
};
use tool_runtime::exec_command::{
    exec_command_control_tool_input_from_input, exec_command_control_tool_input_validation_message,
    exec_control_result_value, exec_control_session_not_found_result,
    render_exec_control_response_for_assistant, ExecCommandControlAction, ExecCommandControlOrigin,
    ExecCommandControlRequest, ExecCommandControlResponse, ExecCommandResultFields,
};

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

#[derive(Debug, thiserror::Error)]
pub enum ExecCommandControlError {
    #[error("session not found: {0}")]
    SessionNotFound(i32),

    #[error(transparent)]
    Tool(#[from] BitFunError),
}

pub async fn control_exec_command_session(
    request: ExecCommandControlRequest,
) -> Result<ExecCommandControlResponse, ExecCommandControlError> {
    if request.remote {
        let response = get_global_remote_exec_process_manager()
            .control_session(RemoteExecControlRequest {
                session_id: request.session_id,
                action: ExecControlTool::remote_action(request.action),
                origin: ExecControlTool::remote_origin(request.origin),
                yield_time_ms: request.yield_time_ms,
                max_output_chars: None,
            })
            .await
            .map_err(|error| match error {
                RemoteExecError::SessionNotFound(session_id) => {
                    ExecCommandControlError::SessionNotFound(session_id)
                }
                error => ExecCommandControlError::Tool(BitFunError::tool(format!(
                    "ExecControl failed: {error}"
                ))),
            })?;

        return Ok(ExecCommandControlResponse {
            chunk_id: response.chunk_id,
            wall_time_seconds: response.wall_time_seconds,
            output: response.output,
            session_id: response.session_id,
            exit_code: response.exit_code,
            original_output_chars: response.original_output_chars,
            action: request.action,
            remote: true,
            completion: response.completion.map(exec_command_remote_completion),
        });
    }

    let response = get_global_exec_process_manager()
        .control_session(LocalExecControlRequest {
            session_id: request.session_id,
            action: ExecControlTool::local_action(request.action),
            origin: ExecControlTool::local_origin(request.origin),
            yield_time_ms: request.yield_time_ms,
            max_output_chars: None,
        })
        .await
        .map_err(|error| match error {
            TerminalError::SessionNotFound(_) => {
                ExecCommandControlError::SessionNotFound(request.session_id)
            }
            error => ExecCommandControlError::Tool(BitFunError::tool(format!(
                "ExecControl failed: {error}"
            ))),
        })?;

    Ok(ExecCommandControlResponse {
        chunk_id: response.chunk_id,
        wall_time_seconds: response.wall_time_seconds,
        output: response.output,
        session_id: response.session_id,
        exit_code: response.exit_code,
        original_output_chars: response.original_output_chars,
        action: request.action,
        remote: false,
        completion: response.completion.map(exec_command_local_completion),
    })
}

impl Default for ExecControlTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecControlTool {
    pub fn new() -> Self {
        Self
    }

    fn response_for_assistant(data: &Value, action: ExecCommandControlAction) -> String {
        render_exec_control_response_for_assistant(data, action)
    }

    fn session_not_found_result(
        session_id: i32,
        action: ExecCommandControlAction,
        remote: bool,
    ) -> Vec<ToolResult> {
        let result = exec_control_session_not_found_result(session_id, action, remote);

        vec![ToolResult::Result {
            data: result.data,
            result_for_assistant: Some(result.assistant_message),
            image_attachments: None,
        }]
    }

    fn local_action(action: ExecCommandControlAction) -> LocalExecControlAction {
        match action {
            ExecCommandControlAction::Interrupt => LocalExecControlAction::Interrupt,
            ExecCommandControlAction::Kill => LocalExecControlAction::Kill,
        }
    }

    fn local_origin(origin: ExecCommandControlOrigin) -> LocalExecControlOrigin {
        match origin {
            ExecCommandControlOrigin::ModelTool => LocalExecControlOrigin::ModelTool,
            ExecCommandControlOrigin::OutOfBand => LocalExecControlOrigin::OutOfBand,
        }
    }

    fn remote_action(action: ExecCommandControlAction) -> RemoteExecControlAction {
        match action {
            ExecCommandControlAction::Interrupt => RemoteExecControlAction::Interrupt,
            ExecCommandControlAction::Kill => RemoteExecControlAction::Kill,
        }
    }

    fn remote_origin(origin: ExecCommandControlOrigin) -> RemoteExecControlOrigin {
        match origin {
            ExecCommandControlOrigin::ModelTool => RemoteExecControlOrigin::ModelTool,
            ExecCommandControlOrigin::OutOfBand => RemoteExecControlOrigin::OutOfBand,
        }
    }

    async fn call_remote_pipe(&self, input: &Value) -> BitFunResult<Vec<ToolResult>> {
        if let Some(message) = exec_command_control_tool_input_validation_message(input) {
            return Err(BitFunError::tool(message.to_string()));
        }
        let parsed_input = exec_command_control_tool_input_from_input(input)
            .expect("validated ExecControl input should parse");
        let session_id = parsed_input.session_id;
        let action = parsed_input.action;
        let response = match control_exec_command_session(ExecCommandControlRequest {
            session_id,
            action,
            origin: ExecCommandControlOrigin::ModelTool,
            remote: true,
            yield_time_ms: parsed_input.yield_time_ms,
        })
        .await
        {
            Ok(response) => response,
            Err(ExecCommandControlError::SessionNotFound(session_id)) => {
                return Ok(Self::session_not_found_result(session_id, action, true));
            }
            Err(ExecCommandControlError::Tool(error)) => return Err(error),
        };

        let data = exec_control_result_value(
            ExecCommandResultFields {
                chunk_id: response.chunk_id,
                wall_time_seconds: response.wall_time_seconds,
                output: response.output,
                session_id: response.session_id,
                exit_code: response.exit_code,
                original_output_chars: response.original_output_chars,
                completion: None,
                remote: response.remote,
            },
            action,
        );
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
After the action, yield_time_ms waits for output or exit status.
Output is only what was produced during this tool call's wait window."#
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
                    "description": "session_id returned by ExecCommand."
                },
                "action": {
                    "type": "string",
                    "enum": ["interrupt", "kill"],
                    "description": "Use interrupt to stop gracefully; use kill to force termination."
                },
                "yield_time_ms": {
                    "type": "number",
                    "description": "How long to wait for output after the control action before yielding. Defaults to 10000 ms."
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

    fn manages_own_execution_timeout(&self) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        if let Some(message) = exec_command_control_tool_input_validation_message(input) {
            return ValidationResult {
                result: false,
                message: Some(message.to_string()),
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

        if let Some(message) = exec_command_control_tool_input_validation_message(input) {
            return Err(BitFunError::tool(message.to_string()));
        }
        let parsed_input = exec_command_control_tool_input_from_input(input)
            .expect("validated ExecControl input should parse");
        let session_id = parsed_input.session_id;
        let action = parsed_input.action;
        let response = match control_exec_command_session(ExecCommandControlRequest {
            session_id,
            action,
            origin: ExecCommandControlOrigin::ModelTool,
            remote: false,
            yield_time_ms: parsed_input.yield_time_ms,
        })
        .await
        {
            Ok(response) => response,
            Err(ExecCommandControlError::SessionNotFound(session_id)) => {
                return Ok(Self::session_not_found_result(session_id, action, false));
            }
            Err(ExecCommandControlError::Tool(error)) => return Err(error),
        };

        let data = exec_control_result_value(
            ExecCommandResultFields {
                chunk_id: response.chunk_id,
                wall_time_seconds: response.wall_time_seconds,
                output: response.output,
                session_id: response.session_id,
                exit_code: response.exit_code,
                original_output_chars: response.original_output_chars,
                completion: None,
                remote: false,
            },
            action,
        );
        let result_for_assistant = Self::response_for_assistant(&data, action);

        Ok(vec![ToolResult::Result {
            data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::{
        control_exec_command_session, ExecCommandControlAction, ExecCommandControlError,
        ExecCommandControlOrigin, ExecCommandControlRequest, ExecControlTool,
    };
    use crate::agentic::tools::framework::ToolResult;

    #[test]
    fn session_not_found_result_uses_plain_assistant_message() {
        let results = ExecControlTool::session_not_found_result(
            456,
            ExecCommandControlAction::Interrupt,
            false,
        );
        let ToolResult::Result {
            data,
            result_for_assistant,
            ..
        } = &results[0]
        else {
            panic!("expected result");
        };

        assert_eq!(
            data.get("status").and_then(|value| value.as_str()),
            Some("session_not_found")
        );
        assert_eq!(
            data.get("requested_session_id")
                .and_then(|value| value.as_i64()),
            Some(456)
        );
        let assistant = result_for_assistant.as_deref().expect("assistant text");
        assert!(assistant.contains("No interrupt was sent"));
        assert!(assistant.contains("ExecCommand session 456 was not found"));
        assert!(!assistant.contains("<wall_time>"));
        assert!(!assistant.contains("<output>"));
    }

    #[tokio::test]
    async fn control_exec_command_session_returns_structured_session_not_found() {
        let error = control_exec_command_session(ExecCommandControlRequest {
            session_id: 987_654,
            action: ExecCommandControlAction::Kill,
            origin: ExecCommandControlOrigin::ModelTool,
            remote: false,
            yield_time_ms: Some(0),
        })
        .await
        .expect_err("missing session should be structured");

        assert!(matches!(
            error,
            ExecCommandControlError::SessionNotFound(987_654)
        ));
    }
}
