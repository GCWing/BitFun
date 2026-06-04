use super::rendering::render_exec_response_for_assistant;
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::service::remote_ssh::{get_global_remote_exec_process_manager, RemoteWriteStdinRequest};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use terminal_core::{get_global_exec_process_manager, LocalWriteStdinRequest};

const DEFAULT_MAX_OUTPUT_CHARS: u64 = 10_000;

pub struct WriteStdinTool;

impl Default for WriteStdinTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WriteStdinTool {
    pub fn new() -> Self {
        Self
    }

    pub(crate) fn session_id_from_input(input: &Value) -> Option<i32> {
        input.get("session_id").and_then(|value| {
            value
                .as_i64()
                .and_then(|id| i32::try_from(id).ok())
                .or_else(|| value.as_u64().and_then(|id| i32::try_from(id).ok()))
        })
    }

    fn response_for_assistant(data: &Value) -> String {
        let mut status_lines = Vec::new();
        if let Some(exit_code) = data.get("exit_code").and_then(Value::as_i64) {
            status_lines.push(format!("Process exited with code {exit_code}."));
        } else if let Some(session_id) = data.get("session_id").and_then(Value::as_i64) {
            status_lines.push(format!(
                "Process is still running. session_id: {session_id}"
            ));
        }
        render_exec_response_for_assistant(data, status_lines, 4)
    }

    async fn call_remote_pipe(&self, input: &Value) -> BitFunResult<Vec<ToolResult>> {
        let session_id = Self::session_id_from_input(input).ok_or_else(|| {
            BitFunError::tool("session_id is required for WriteStdin".to_string())
        })?;
        let chars = input
            .get("chars")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let append_enter = input
            .get("append_enter")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let yield_time_ms = input.get("yield_time_ms").and_then(Value::as_u64);
        let max_output_chars = input
            .get("max_output_chars")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_MAX_OUTPUT_CHARS)
            .try_into()
            .unwrap_or(usize::MAX);

        let response = get_global_remote_exec_process_manager()
            .write_stdin(RemoteWriteStdinRequest {
                session_id,
                chars,
                append_enter,
                yield_time_ms,
                max_output_chars: Some(max_output_chars),
            })
            .await
            .map_err(|error| BitFunError::tool(format!("WriteStdin failed: {error}")))?;

        let data = json!({
            "chunk_id": response.chunk_id,
            "wall_time_seconds": response.wall_time_seconds,
            "output": response.output,
            "session_id": response.session_id,
            "exit_code": response.exit_code,
            "original_output_chars": response.original_output_chars,
            "remote": true,
        });
        let result_for_assistant = Self::response_for_assistant(&data);

        Ok(vec![ToolResult::Result {
            data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}

#[async_trait]
impl Tool for WriteStdinTool {
    fn name(&self) -> &str {
        "WriteStdin"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Writes stdin to, or polls, a running ExecCommand session.

Pass the session_id returned by ExecCommand. Leave chars empty or omit it to poll for new output.
chars is sent only to sessions started with tty=true. For tty=false sessions, this tool only polls.
Use append_enter=true to submit a line after chars. Use this for line-oriented interactive prompts instead of trying to encode \\r or \\n manually.
Output is only what was produced during this tool call's wait window."#
            .to_string())
    }

    fn short_description(&self) -> String {
        "Write to or poll a running ExecCommand session.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "number",
                    "description": "session_id returned by ExecCommand while a process is still running."
                },
                "chars": {
                    "type": "string",
                    "description": "Characters to write to stdin. Empty or omitted means poll for new output."
                },
                "append_enter": {
                    "type": "boolean",
                    "description": "When true, append an Enter key after chars."
                },
                "yield_time_ms": {
                    "type": "number",
                    "description": "How long to wait for output before yielding."
                },
                "max_output_chars": {
                    "type": "number",
                    "description": "Maximum output characters to return. Defaults to 10000; excess output keeps head and tail."
                }
            },
            "required": ["session_id"],
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
        if Self::session_id_from_input(input).is_none() {
            return ValidationResult {
                result: false,
                message: Some("session_id is required for WriteStdin".to_string()),
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
            BitFunError::tool("session_id is required for WriteStdin".to_string())
        })?;
        let chars = input
            .get("chars")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let append_enter = input
            .get("append_enter")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let yield_time_ms = input.get("yield_time_ms").and_then(Value::as_u64);
        let max_output_chars = input
            .get("max_output_chars")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_MAX_OUTPUT_CHARS)
            .try_into()
            .unwrap_or(usize::MAX);

        let response = get_global_exec_process_manager()
            .write_stdin(LocalWriteStdinRequest {
                session_id,
                chars,
                append_enter,
                yield_time_ms,
                max_output_chars: Some(max_output_chars),
            })
            .await
            .map_err(|error| BitFunError::tool(format!("WriteStdin failed: {error}")))?;

        let data = json!({
            "chunk_id": response.chunk_id,
            "wall_time_seconds": response.wall_time_seconds,
            "output": response.output,
            "session_id": response.session_id,
            "exit_code": response.exit_code,
            "original_output_chars": response.original_output_chars,
        });
        let result_for_assistant = Self::response_for_assistant(&data);

        Ok(vec![ToolResult::Result {
            data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}
