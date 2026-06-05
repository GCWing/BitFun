use crate::agentic::tools::file_read_state_runtime::{
    local_file_modification_time_ms, record_file_read_state,
};
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::workspace_paths::is_bitfun_runtime_uri;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::timing::elapsed_ms_u64;
use async_trait::async_trait;
use log::{debug, warn};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Instant;
use tool_runtime::fs::read_file::{build_remote_read_command, parse_remote_read_output, read_file};

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
- The file_path parameter must be workspace-relative, an absolute path inside the current workspace, or an exact `bitfun://runtime/...` URI returned by another tool.
- Do not read host roots or placeholder paths such as `/workspace`.
- By default, it reads up to {} lines starting from the beginning of the file. When you plan to Edit a file, prefer this default full read so you see the exact bytes you will need to match.
- You can optionally specify a start_line and limit. Use a range only when you already know the target lines; the range must include every line you will copy into Edit `old_string`.
- Any lines longer than {} characters will be truncated.
- Total output is capped at {} characters. If that limit is hit, continue with start_line/limit until the target lines are fully visible, then Edit using only text from those Read results.
- Results are returned using cat -n format, with line numbers starting at 1.
- This tool can only read files, not directories. To read a directory, use an ls command via the Bash tool.
- You can call multiple tools in a single response. It is always better to speculatively read multiple potentially useful files in parallel.
- Avoid tiny repeated slices (e.g. 30-100 line chunks). If you need more context, read a larger window that covers the whole block you will edit.
"#,
            self.default_max_lines_to_read, self.max_line_chars, self.max_total_chars
        ))
    }

    fn short_description(&self) -> String {
        "Read file contents from the current workspace.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The file to read. Use a workspace-relative path, an absolute path inside the current workspace, or an exact bitfun://runtime URI returned by another tool."
                },
                "start_line": {
                    "type": "number",
                    "description": "The line number to start reading from. Only provide if the file is too large to read at once"
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

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
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
                if is_bitfun_runtime_uri(file_path) {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "Tool context is required to resolve bitfun runtime URIs".to_string(),
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

        let start_line = input
            .get("start_line")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;

        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.default_max_lines_to_read as u64) as usize;

        let resolved = context.resolve_tool_path(file_path)?;

        let read_file_result = if resolved.uses_remote_workspace_backend() {
            self.read_remote_window(&resolved.resolved_path, start_line, limit, context)
                .await?
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

        let mut result_for_assistant = format!(
            "Read lines {}-{} from {} ({} total lines)\n<file_content>\n{}\n</file_content>",
            read_file_result.start_line,
            read_file_result.end_line,
            resolved.logical_path,
            read_file_result.total_lines,
            read_file_result.content
        );

        let has_more = read_file_result.end_line < read_file_result.total_lines;
        if has_more {
            let next_start = read_file_result.end_line + 1;
            if read_file_result.hit_total_char_limit {
                result_for_assistant.push_str(
                    &format!("\n\n[Output truncated after reaching the Read tool size limit. Use start_line={} and limit to continue reading.]", next_start));
            } else {
                result_for_assistant.push_str(
                    &format!("\n\n[Showing lines {}-{} of {} total. Use start_line={} and limit to continue reading.]",
                        read_file_result.start_line, read_file_result.end_line, read_file_result.total_lines, next_start));
            }
        }

        let lines_read = if read_file_result.total_lines == 0
            || read_file_result.end_line < read_file_result.start_line
        {
            0
        } else {
            read_file_result.end_line - read_file_result.start_line + 1
        };

        let result = ToolResult::Result {
            data: json!({
                "file_path": resolved.logical_path,
                "content": read_file_result.content,
                "total_lines": read_file_result.total_lines,
                "lines_read": lines_read,
                "start_line": read_file_result.start_line,
                "size": read_file_result.content.len(),
                "hit_total_char_limit": read_file_result.hit_total_char_limit
            }),
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        };

        Ok(vec![result])
    }
}
