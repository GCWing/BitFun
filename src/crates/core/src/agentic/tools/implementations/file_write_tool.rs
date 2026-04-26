use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::ToolPathOperation;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;
use tokio::fs;

pub struct FileWriteTool;

const LARGE_WRITE_SOFT_LINE_LIMIT: usize = 200;
const LARGE_WRITE_SOFT_BYTE_LIMIT: usize = 20 * 1024;

impl Default for FileWriteTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FileWriteTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "Write"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Writes a file to the local filesystem.

Usage:
- This tool will overwrite the existing file if there is one at the provided path.
- If this is an existing file, you MUST use the Read tool first to read the file's contents. This tool will fail if you did not read the file first.
- The file_path parameter must be either an absolute path or an exact `bitfun://runtime/...` URI returned by another tool.
- ALWAYS prefer editing existing files in the codebase. NEVER write new files unless explicitly required.
- Keep writes focused. The 200-line / 20KB guideline is a soft reliability threshold, not a hard cap. If a task genuinely needs more content, preserve correctness and use a staged plan instead of truncating.
- For existing files, prefer Read + targeted Edit calls. For large new files or rewrites, write the stable scaffold first, then fill or revise sections with focused Edit calls. Do not replace an entire existing file just to change a few sections.
- NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the User.
- Only use emojis if the user explicitly requests it. Avoid writing emojis to files unless asked."#.to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to write, or an exact bitfun://runtime URI returned by another tool"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file. 200 lines / 20KB is a soft reliability threshold, not a hard cap. For existing files prefer Read + focused Edit calls. For large new files, write a stable scaffold first, then add sections in follow-up focused edits unless a complete initial body is required."
                }
            },
            "required": ["file_path", "content"],
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
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let file_path = match input.get("file_path").and_then(|v| v.as_str()) {
            Some(path) if !path.is_empty() => path,
            _ => {
                return ValidationResult {
                    result: false,
                    message: Some("file_path is required and cannot be empty".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        if input.get("content").is_none() {
            return ValidationResult {
                result: false,
                message: Some("content is required".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        let large_write_warning =
            input
                .get("content")
                .and_then(|v| v.as_str())
                .and_then(|content| {
                    let line_count = content.lines().count();
                    let byte_count = content.len();
                    if line_count > LARGE_WRITE_SOFT_LINE_LIMIT
                        || byte_count > LARGE_WRITE_SOFT_BYTE_LIMIT
                    {
                        Some((line_count, byte_count))
                    } else {
                        None
                    }
                });

        if let Some(ctx) = context {
            let resolved = match ctx.resolve_tool_path(file_path) {
                Ok(resolved) => resolved,
                Err(err) => {
                    return ValidationResult {
                        result: false,
                        message: Some(err.to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                }
            };

            if let Err(err) = ctx.enforce_path_operation(ToolPathOperation::Write, &resolved) {
                return ValidationResult {
                    result: false,
                    message: Some(err.to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        if let Some((line_count, byte_count)) = large_write_warning {
            return ValidationResult {
                result: true,
                message: Some(format!(
                    "Large Write payload: {} lines, {} bytes. This is allowed when necessary, but prefer a staged approach: for existing files use Read + focused Edit calls; for large new files write a stable scaffold first, then add sections in follow-up edits unless a complete initial body is required.",
                    line_count, byte_count
                )),
                error_code: None,
                meta: Some(json!({
                    "large_write": true,
                    "line_count": line_count,
                    "byte_count": byte_count,
                    "soft_line_limit": LARGE_WRITE_SOFT_LINE_LIMIT,
                    "soft_byte_limit": LARGE_WRITE_SOFT_BYTE_LIMIT
                })),
            };
        }

        ValidationResult::default()
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        if let Some(file_path) = input.get("file_path").and_then(|v| v.as_str()) {
            if options.verbose {
                let content_len = input
                    .get("content")
                    .and_then(|v| v.as_str())
                    .map(|s| s.len())
                    .unwrap_or(0);
                format!("Writing {} characters to {}", content_len, file_path)
            } else {
                format!("Write {}", file_path)
            }
        } else {
            "Writing file".to_string()
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

        let resolved = context.resolve_tool_path(file_path)?;
        context.enforce_path_operation(ToolPathOperation::Write, &resolved)?;

        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("content is required".to_string()))?;

        if resolved.uses_remote_workspace_backend() {
            let ws_fs = context.ws_fs().ok_or_else(|| {
                BitFunError::tool("Remote workspace file system is unavailable".to_string())
            })?;
            ws_fs
                .write_file(&resolved.resolved_path, content.as_bytes())
                .await
                .map_err(|e| BitFunError::tool(format!("Failed to write file: {}", e)))?;
        } else {
            if let Some(parent) = Path::new(&resolved.resolved_path).parent() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|e| BitFunError::tool(format!("Failed to create directory: {}", e)))?;
            }
            fs::write(&resolved.resolved_path, content)
                .await
                .map_err(|e| {
                    BitFunError::tool(format!(
                        "Failed to write file {}: {}",
                        resolved.logical_path, e
                    ))
                })?;
        }

        let result = ToolResult::Result {
            data: json!({
                "file_path": resolved.logical_path,
                "bytes_written": content.len(),
                "success": true
            }),
            result_for_assistant: Some(format!("Successfully wrote to {}", resolved.logical_path)),
            image_attachments: None,
        };

        Ok(vec![result])
    }
}
