use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::workspace_paths::resolve_workspace_tool_path;
use crate::service::ai_rules::get_global_ai_rules_service;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use log::debug;
use serde_json::{json, Value};
use std::path::Path;
use tool_runtime::fs::read_file::read_file;

pub struct FileReadTool {
    default_max_lines_to_read: usize,
    max_line_chars: usize,
    max_total_chars: usize,
}

impl Default for FileReadTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FileReadTool {
    pub fn new() -> Self {
        Self {
            default_max_lines_to_read: 250,
            max_line_chars: 300,
            max_total_chars: 32_000,
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
        const TOTAL_LINES_MARKER: &str = "__BITFUN_TOTAL_LINES__=";
        const HIT_TOTAL_CHAR_LIMIT_MARKER: &str = "__BITFUN_HIT_TOTAL_CHAR_LIMIT__=";

        let end_line = start_line
            .checked_add(limit.saturating_sub(1))
            .ok_or_else(|| BitFunError::tool("Requested line range is too large".to_string()))?;

        let ws_shell = context
            .ws_shell()
            .ok_or_else(|| BitFunError::tool("Remote workspace shell is unavailable".to_string()))?;

        let escaped_path = shell_escape(resolved_path);
        let command = format!(
            "if [ ! -f {path} ]; then exit 3; fi; awk -v start={start} -v end={end} -v max={max} -v budget={budget} 'BEGIN {{ total = 0; used = 0; hit = 0; }} {{ total = NR; if (!hit && NR >= start && NR <= end) {{ line = $0; if (length(line) > max) {{ line = substr(line, 1, max) \" [truncated]\"; }} rendered = sprintf(\"%6d\\t%s\", NR, line); extra = (used > 0 ? 1 : 0); next_used = used + extra + length(rendered); if (next_used > budget) {{ hit = 1; next; }} print rendered; used = next_used; }} }} END {{ printf(\"{marker}%d\\n\", total) > \"/dev/stderr\"; printf(\"{hit_marker}%d\\n\", hit) > \"/dev/stderr\"; }}' {path}",
            path = escaped_path,
            start = start_line,
            end = end_line,
            max = self.max_line_chars,
            budget = self.max_total_chars,
            marker = TOTAL_LINES_MARKER,
            hit_marker = HIT_TOTAL_CHAR_LIMIT_MARKER,
        );

        let (stdout, stderr, status) = ws_shell
            .exec(&command, None)
            .await
            .map_err(|e| BitFunError::tool(format!("Failed to read file: {}", e)))?;

        let mut total_lines = None;
        let mut hit_total_char_limit = false;
        let mut stderr_messages = Vec::new();
        for line in stderr.lines() {
            if let Some(rest) = line.strip_prefix(TOTAL_LINES_MARKER) {
                total_lines = rest.trim().parse::<usize>().ok();
            } else if let Some(rest) = line.strip_prefix(HIT_TOTAL_CHAR_LIMIT_MARKER) {
                hit_total_char_limit = rest.trim() == "1";
            } else if !line.trim().is_empty() {
                stderr_messages.push(line.to_string());
            }
        }

        if status != 0 {
            let message = if status == 3 {
                format!("File not found or not a regular file: {}", resolved_path)
            } else if !stderr_messages.is_empty() {
                stderr_messages.join("\n")
            } else {
                format!("Failed to read file: remote command exited with status {}", status)
            };
            return Err(BitFunError::tool(message));
        }

        let total_lines = total_lines.ok_or_else(|| {
            BitFunError::tool("Failed to read file: remote line count was unavailable".to_string())
        })?;

        if total_lines == 0 {
            return Ok(tool_runtime::fs::read_file::ReadFileResult {
                start_line: 0,
                end_line: 0,
                total_lines: 0,
                content: String::new(),
                hit_total_char_limit,
            });
        }

        if start_line > total_lines {
            return Err(BitFunError::tool(format!(
                "`start_line` {} is larger than the number of lines in the file: {}",
                start_line, total_lines
            )));
        }

        let content = stdout.trim_end_matches('\n').to_string();
        let lines_read = if content.is_empty() {
            0
        } else {
            content.lines().count()
        };
        let end_line = if lines_read == 0 {
            start_line
        } else {
            (start_line + lines_read).saturating_sub(1)
        };

        Ok(tool_runtime::fs::read_file::ReadFileResult {
            start_line,
            end_line,
            total_lines,
            content,
            hit_total_char_limit,
        })
    }
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "Read"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(format!(
            r#"Reads a file from the local filesystem. You can access any file directly by using this tool.
Assume this tool is able to read all files on the machine. If the User provides a path to a file assume that path is valid. It is okay to read a file that does not exist; an error will be returned.

Usage:
- The file_path parameter must be an absolute path, not a relative path.
- By default, it reads up to {} lines starting from the beginning of the file. 
- You can optionally specify a start_line and limit. For large files, prefer reading targeted ranges instead of starting over from the beginning every time.
- Any lines longer than {} characters will be truncated.
- Total output is capped at {} characters. If that limit is hit, narrow the range with start_line and limit.
- Results are returned using cat -n format, with line numbers starting at 1
- This tool can only read files, not directories. To read a directory, use an ls command via the Bash tool.
- You can call multiple tools in a single response. It is always better to speculatively read multiple potentially useful files in parallel.
"#,
            self.default_max_lines_to_read, self.max_line_chars, self.max_total_chars
        ))
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to read"
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

        let root_owned = context.and_then(|ctx| {
            ctx.workspace
                .as_ref()
                .map(|w| w.root_path_string())
        });
        let resolved_path = match resolve_workspace_tool_path(
            file_path,
            root_owned.as_deref(),
            context.map(|c| c.is_remote()).unwrap_or(false),
        ) {
            Ok(path) => path,
            Err(err) => {
                return ValidationResult {
                    result: false,
                    message: Some(err.to_string()),
                    error_code: Some(400),
                    meta: None,
                }
            }
        };

        // For remote workspaces, skip local filesystem checks — the actual
        // read goes through WorkspaceFileSystem in call_impl.
        let is_remote = context.map(|c| c.is_remote()).unwrap_or(false);
        if !is_remote {
            let path = Path::new(&resolved_path);
            if !path.exists() {
                return ValidationResult {
                    result: false,
                    message: Some(format!("File does not exist: {}", resolved_path)),
                    error_code: Some(404),
                    meta: None,
                };
            }
            if !path.is_file() {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Path is not a file: {}", resolved_path)),
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

        let resolved_path = context.resolve_workspace_tool_path(file_path)?;

        // Use the workspace file system from context — works for both local and remote.
        let read_file_result = if context.is_remote() {
            self.read_remote_window(&resolved_path, start_line, limit, context)
                .await?
        } else {
            read_file(
                &resolved_path,
                start_line,
                limit,
                self.max_line_chars,
                self.max_total_chars,
            )
                .map_err(BitFunError::tool)?
        };

        let file_rules = match get_global_ai_rules_service().await {
            Ok(rules_service) => {
                rules_service
                    .get_rules_for_file_with_workspace(&resolved_path, context.workspace_root())
                    .await
            }
            Err(e) => {
                debug!("Failed to get AIRulesService: {}", e);
                crate::service::ai_rules::FileRulesResult {
                    matched_count: 0,
                    formatted_content: None,
                }
            }
        };

        let mut result_for_assistant = format!(
            "Read lines {}-{} from {} ({} total lines)\n<file_content>\n{}\n</file_content>",
            read_file_result.start_line,
            read_file_result.end_line,
            resolved_path,
            read_file_result.total_lines,
            read_file_result.content
        );

        if let Some(rules_content) = &file_rules.formatted_content {
            result_for_assistant.push_str("\n\n");
            result_for_assistant.push_str(rules_content);
        }

        if read_file_result.hit_total_char_limit {
            result_for_assistant.push_str("\n\n[Output truncated after reaching the Read tool size limit. Request a narrower range with start_line and limit.]");
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
                "file_path": resolved_path,
                "content": read_file_result.content,
                "total_lines": read_file_result.total_lines,
                "lines_read": lines_read,
                "start_line": read_file_result.start_line,
                "size": read_file_result.content.len(),
                "hit_total_char_limit": read_file_result.hit_total_char_limit,
                "matched_rules_count": file_rules.matched_count
            }),
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        };

        Ok(vec![result])
    }
}
