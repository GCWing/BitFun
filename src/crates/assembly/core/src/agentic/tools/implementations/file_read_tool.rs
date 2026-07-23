use crate::agentic::tools::file_permissions::file_permission_intents;
use crate::agentic::tools::file_read_state_runtime::{
    local_file_modification_time_ms, record_file_read_state,
};
use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::workspace_paths::is_bitfun_tool_uri;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::timing::elapsed_ms_u64;
use async_trait::async_trait;
use log::{debug, warn};
use serde_json::{json, Value};
use std::convert::TryFrom;
use std::path::Path;
use std::time::Instant;
use tool_runtime::fs::read_file::{
    build_read_file_presentation, build_remote_read_command, build_remote_tail_read_command,
    parse_remote_read_output, parse_remote_tail_read_output, read_file, read_file_tail,
};

pub struct FileReadTool {
    default_max_lines_to_read: usize,
    max_line_chars: usize,
    max_total_chars: usize,
}

/// Default cap on characters returned by a single Read call (excluding wrapper text).
pub const DEFAULT_READ_MAX_TOTAL_CHARS: usize = 64_000;

impl Default for FileReadTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FileReadTool {
    pub fn new() -> Self {
        Self {
            default_max_lines_to_read: 2000,
            max_line_chars: 2000,
            max_total_chars: DEFAULT_READ_MAX_TOTAL_CHARS,
        }
    }

    pub fn with_config(
        default_max_lines_to_read: usize,
        max_line_chars: usize,
        max_total_chars: usize,
    ) -> Self {
        Self {
            default_max_lines_to_read,
            max_line_chars,
            max_total_chars,
        }
    }

    fn read_window_start_line(input: &Value) -> Result<usize, String> {
        Self::optional_line_number(input, "offset")?.map_or(Ok(1), |offset| Ok(offset.max(1)))
    }

    fn read_tail_mode(input: &Value) -> Result<bool, String> {
        let tail = match input.get("tail") {
            Some(value) => value
                .as_bool()
                .ok_or_else(|| "tail must be a boolean".to_string())?,
            None => false,
        };

        if tail && input.get("offset").is_some() {
            return Err("Do not provide offset when tail is true".to_string());
        }

        Ok(tail)
    }

    fn optional_line_number(input: &Value, key: &str) -> Result<Option<usize>, String> {
        match input.get(key) {
            Some(value) => Self::line_number_from_value(value)
                .map(Some)
                .map_err(|message| format!("{} {}", key, message)),
            None => Ok(None),
        }
    }

    fn line_number_from_value(value: &Value) -> Result<usize, &'static str> {
        if let Some(number) = value.as_u64() {
            return usize::try_from(number).map_err(|_| "is too large");
        }

        if let Some(number) = value.as_i64() {
            if number < 0 {
                return Err("must be a non-negative integer");
            }
            return usize::try_from(number as u64).map_err(|_| "is too large");
        }

        if let Some(number) = value.as_f64() {
            if !number.is_finite() || number < 0.0 || number.fract() != 0.0 {
                return Err("must be a non-negative integer");
            }
            if number > usize::MAX as f64 {
                return Err("is too large");
            }
            return Ok(number as usize);
        }

        Err("must be a non-negative integer")
    }

    async fn read_remote_window(
        &self,
        resolved_path: &str,
        start_line: usize,
        limit: usize,
        context: &ToolUseContext,
    ) -> BitFunResult<tool_runtime::fs::read_file::ReadFileResult> {
        let ws_shell = context.ws_shell().ok_or_else(|| {
            BitFunError::tool("Remote workspace shell is unavailable".to_string())
        })?;

        let command = build_remote_read_command(
            resolved_path,
            start_line,
            limit,
            self.max_line_chars,
            self.max_total_chars,
        )
        .map_err(BitFunError::tool)?;

        let remote_read_started_at = Instant::now();
        debug!(
            "Remote file read started: path={}, start_line={}, limit={}, timeout_ms={:?}, session_id={:?}, dialog_turn_id={:?}",
            resolved_path,
            start_line,
            limit,
            Option::<u64>::None,
            context.session_id,
            context.dialog_turn_id
        );
        let (stdout, stderr, status) = ws_shell
            .exec(&command, None)
            .await
            .map_err(|e| {
                warn!(
                    "Remote file read failed: path={}, start_line={}, limit={}, duration_ms={}, error={}",
                    resolved_path,
                    start_line,
                    limit,
                    elapsed_ms_u64(remote_read_started_at),
                    e
                );
                BitFunError::tool(format!("Failed to read file: {}", e))
            })?;
        debug!(
            "Remote file read command completed: path={}, start_line={}, limit={}, status={}, stdout_len={}, stderr_len={}, duration_ms={}",
            resolved_path,
            start_line,
            limit,
            status,
            stdout.len(),
            stderr.len(),
            elapsed_ms_u64(remote_read_started_at)
        );

        let result = parse_remote_read_output(&stdout, &stderr, status, resolved_path, start_line)
            .map_err(BitFunError::tool)?;

        debug!(
            "Remote file read parsed successfully: path={}, start_line={}, end_line={}, total_lines={}, hit_total_char_limit={}, duration_ms={}",
            resolved_path,
            result.start_line,
            result.end_line,
            result.total_lines,
            result.hit_total_char_limit,
            elapsed_ms_u64(remote_read_started_at)
        );

        Ok(result)
    }

    async fn read_remote_tail_window(
        &self,
        resolved_path: &str,
        limit: usize,
        context: &ToolUseContext,
    ) -> BitFunResult<tool_runtime::fs::read_file::ReadFileResult> {
        let ws_shell = context.ws_shell().ok_or_else(|| {
            BitFunError::tool("Remote workspace shell is unavailable".to_string())
        })?;

        let command = build_remote_tail_read_command(
            resolved_path,
            limit,
            self.max_line_chars,
            self.max_total_chars,
        )
        .map_err(BitFunError::tool)?;

        let remote_read_started_at = Instant::now();
        debug!(
            "Remote file tail read started: path={}, limit={}, timeout_ms={:?}, session_id={:?}, dialog_turn_id={:?}",
            resolved_path,
            limit,
            Option::<u64>::None,
            context.session_id,
            context.dialog_turn_id
        );
        let (stdout, stderr, status) = ws_shell.exec(&command, None).await.map_err(|e| {
            warn!(
                "Remote file tail read failed: path={}, limit={}, duration_ms={}, error={}",
                resolved_path,
                limit,
                elapsed_ms_u64(remote_read_started_at),
                e
            );
            BitFunError::tool(format!("Failed to read file: {}", e))
        })?;
        debug!(
            "Remote file tail read command completed: path={}, limit={}, status={}, stdout_len={}, stderr_len={}, duration_ms={}",
            resolved_path,
            limit,
            status,
            stdout.len(),
            stderr.len(),
            elapsed_ms_u64(remote_read_started_at)
        );

        let result = parse_remote_tail_read_output(&stdout, &stderr, status, resolved_path, limit)
            .map_err(BitFunError::tool)?;

        debug!(
            "Remote file tail read parsed successfully: path={}, start_line={}, end_line={}, total_lines={}, hit_total_char_limit={}, duration_ms={}",
            resolved_path,
            result.start_line,
            result.end_line,
            result.total_lines,
            result.hit_total_char_limit,
            elapsed_ms_u64(remote_read_started_at)
        );

        Ok(result)
    }
}

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "Read"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(format!(
            r#"Reads a file from the current workspace filesystem. If the User provides a path to a file assume that path is valid. It is okay to read a file that does not exist; an error will be returned.

Usage:
- The file_path parameter must be workspace-relative, an absolute path inside the current workspace, or an exact `bitfun://...` URI returned by another tool.
- Do not read host roots or placeholder paths such as `/workspace`.
- By default, it reads up to {} lines starting from the beginning of the file. When you plan to Edit a file, prefer this default full read so you see the exact bytes you will need to match.
- You can optionally specify an offset and limit. offset is a 1-based line number. Use a range only when you already know the target lines; the range must include every line you will copy into Edit `old_string`.
- You can set tail=true with limit to read the last N lines. This is useful for command output and logs. Do not combine tail=true with offset.
- Any lines longer than {} characters will be truncated.
- Total output is capped at {} characters. If that limit is hit, continue with offset/limit, until the target lines are fully visible, then Edit using only text from those Read results.
- Results are returned using cat -n format, with line numbers starting at 1.
- This tool can only read files, not directories.
- You can call multiple tools in a single response. It is always better to speculatively read multiple potentially useful files in parallel.
- Avoid tiny repeated slices (e.g. 30-100 line chunks). If you need more context, read a larger window that covers the whole block you will edit.
- Do not use `limit` with a small value (e.g. < 50) to probe file type or structure. Source files typically begin with copyright headers — a probe read returns no useful code.
"#,
            self.default_max_lines_to_read, self.max_line_chars, self.max_total_chars
        ))
    }

    fn short_description(&self) -> String {
        "Read file contents.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The file to read. Use a workspace-relative path, an absolute path inside the current workspace, or an exact bitfun:// URI returned by another tool."
                },
                "offset": {
                    "type": "number",
                    "description": "The 1-based line number to start reading from. offset=0 is accepted as offset=1. Only provide if the file is too large to read at once."
                },
                "tail": {
                    "type": "boolean",
                    "description": "Read the last N lines of the file, where N is limit. Do not provide offset when tail is true."
                },
                "limit": {
                    "type": "number",
                    "description": "The number of lines to read. Only provide if the file is too large to read at once."
                }
            },
            "required": ["file_path"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    fn permission_intents(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<PermissionIntent>> {
        let file_path = input
            .get("file_path")
            .and_then(Value::as_str)
            .ok_or_else(|| BitFunError::validation("file_path is required".to_string()))?;
        file_permission_intents("read", [file_path], context)
    }

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let file_path = match input.get("file_path").and_then(|v| v.as_str()) {
            Some(p) if !p.is_empty() => p,
            Some(_) => {
                return ValidationResult {
                    result: false,
                    message: Some("file_path cannot be empty".to_string()),
                    error_code: Some(400),
                    meta: None,
                }
            }
            None => {
                return ValidationResult {
                    result: false,
                    message: Some("file_path is required".to_string()),
                    error_code: Some(400),
                    meta: None,
                }
            }
        };

        if let Err(message) =
            Self::read_tail_mode(input).and_then(|_| Self::read_window_start_line(input))
        {
            return ValidationResult {
                result: false,
                message: Some(message),
                error_code: Some(400),
                meta: None,
            };
        }

        let resolved = match context.map(|ctx| ctx.resolve_tool_path(file_path)) {
            Some(Ok(path)) => path,
            Some(Err(err)) => {
                return ValidationResult {
                    result: false,
                    message: Some(err.to_string()),
                    error_code: Some(400),
                    meta: None,
                }
            }
            None => {
                if is_bitfun_tool_uri(file_path) {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "Tool context is required to resolve BitFun URIs".to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                let path = Path::new(file_path);
                if !path.is_absolute() {
                    return ValidationResult {
                        result: false,
                        message: Some("file_path must be absolute".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                if !path.exists() {
                    return ValidationResult {
                        result: false,
                        message: Some(format!("File does not exist: {}", file_path)),
                        error_code: Some(404),
                        meta: None,
                    };
                }

                if !path.is_file() {
                    return ValidationResult {
                        result: false,
                        message: Some(format!("Path is not a file: {}", file_path)),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                return ValidationResult::default();
            }
        };

        if !resolved.uses_remote_workspace_backend() {
            let path = Path::new(&resolved.resolved_path);
            if !path.exists() {
                return ValidationResult {
                    result: false,
                    message: Some(format!("File does not exist: {}", resolved.logical_path)),
                    error_code: Some(404),
                    meta: None,
                };
            }
            if !path.is_file() {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Path is not a file: {}", resolved.logical_path)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        ValidationResult::default()
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        if let Some(file_path) = input.get("file_path").and_then(|v| v.as_str()) {
            if options.verbose {
                format!("Reading file: {}", file_path)
            } else {
                format!("Read {}", file_path)
            }
        } else {
            "Reading file".to_string()
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let file_path = input
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("file_path is required".to_string()))?;

        let tail = Self::read_tail_mode(input).map_err(BitFunError::tool)?;
        let start_line = Self::read_window_start_line(input).map_err(BitFunError::tool)?;

        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.default_max_lines_to_read as u64) as usize;

        let resolved = context.resolve_tool_path(file_path)?;

        let read_file_result = if resolved.uses_remote_workspace_backend() {
            if tail {
                self.read_remote_tail_window(&resolved.resolved_path, limit, context)
                    .await?
            } else {
                self.read_remote_window(&resolved.resolved_path, start_line, limit, context)
                    .await?
            }
        } else if tail {
            read_file_tail(
                &resolved.resolved_path,
                limit,
                self.max_line_chars,
                self.max_total_chars,
            )
            .map_err(BitFunError::tool)?
        } else {
            read_file(
                &resolved.resolved_path,
                start_line,
                limit,
                self.max_line_chars,
                self.max_total_chars,
            )
            .map_err(BitFunError::tool)?
        };

        let timestamp_ms = if resolved.uses_remote_workspace_backend() {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or(0)
        } else {
            local_file_modification_time_ms(Path::new(&resolved.resolved_path))
        };
        record_file_read_state(context, &resolved, &read_file_result, timestamp_ms);

        let presentation = build_read_file_presentation(&resolved.logical_path, &read_file_result);

        let result = ToolResult::Result {
            data: json!({
                "file_path": resolved.logical_path,
                "content": read_file_result.content,
                "total_lines": read_file_result.total_lines,
                "lines_read": presentation.lines_read,
                "offset": read_file_result.start_line,
                "tail": tail,
                "start_line": read_file_result.start_line,
                "size": read_file_result.content.len(),
                "hit_total_char_limit": read_file_result.hit_total_char_limit
            }),
            result_for_assistant: Some(presentation.result_for_assistant),
            image_attachments: None,
        };

        Ok(vec![result])
    }
}

#[cfg(test)]
mod tests {
    use super::FileReadTool;
    use crate::agentic::tools::framework::Tool;
    use serde_json::{json, Value};

    #[test]
    fn read_tool_schema_prefers_offset() {
        let schema = FileReadTool::new().input_schema();
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");

        assert!(properties.contains_key("offset"));
        assert!(properties.contains_key("tail"));
    }

    #[test]
    fn read_window_start_line_prefers_offset_and_normalizes_zero() {
        assert_eq!(
            FileReadTool::read_window_start_line(&json!({ "offset": 0 })).expect("offset"),
            1
        );
        assert_eq!(
            FileReadTool::read_window_start_line(&json!({ "offset": 42 })).expect("offset"),
            42
        );
        assert_eq!(
            FileReadTool::read_window_start_line(&json!({})).expect("default offset"),
            1
        );
    }

    #[test]
    fn read_tail_mode_rejects_offset() {
        let error = FileReadTool::read_tail_mode(&json!({
            "tail": true,
            "offset": 3
        }))
        .expect_err("tail and offset should not coexist");

        assert_eq!(error, "Do not provide offset when tail is true");
    }
}
