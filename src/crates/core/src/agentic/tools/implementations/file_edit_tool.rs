use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::agentic::tools::ToolPathOperation;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use tool_runtime::fs::edit_file::{apply_edit_to_content, edit_file};

pub struct FileEditTool;

const LARGE_EDIT_SOFT_LINE_LIMIT: usize = 200;
const LARGE_EDIT_SOFT_BYTE_LIMIT: usize = 20 * 1024;

impl Default for FileEditTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FileEditTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &str {
        "Edit"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Performs exact string replacements in files.

Usage:
- You must use your `Read` tool at least once in the conversation before editing. This tool will error if you attempt an edit without reading the file.
- The file_path parameter must be workspace-relative, an absolute path inside the current workspace, or an exact `bitfun://runtime/...` URI returned by another tool.
- When editing text from Read tool output, ensure you preserve the exact indentation (tabs/spaces) as it appears AFTER the line number prefix. The line number prefix format is: spaces + line number + tab. Everything after that tab is the actual file content to match. Never include any part of the line number prefix in the old_string or new_string.
- ALWAYS prefer editing existing files in the codebase. NEVER write new files unless explicitly required.
- Only use emojis if the user explicitly requests it. Avoid adding emojis to files unless asked.
- The edit will FAIL if `old_string` is not unique in the file. Either provide a larger string with more surrounding context to make it unique or use `replace_all` to change every instance of `old_string`.
- Keep edits focused. The 200-line / 20KB guideline is a soft reliability threshold, not a hard cap. If a large change is required, split it into several focused Edit calls by section, function, or component instead of truncating or doing one huge replacement.
- Use `replace_all` for replacing and renaming strings across the file. This parameter is useful if you want to rename a variable for instance."#
        .to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The file to modify. Use a workspace-relative path, an absolute path inside the current workspace, or an exact bitfun://runtime URI returned by another tool."
                },
                "old_string": {
                    "type": "string",
                    "default": "",
                    "description": "The text to replace (must be unique within the file, and must match the file contents exactly, including all whitespace and indentation). Include enough surrounding context to avoid broad replacements, but avoid huge multi-hundred-line old_string payloads."
                },
                "new_string": {
                    "type": "string",
                    "description": "The text to replace it with (must be different from old_string). Keep edits targeted. The 200-line / 20KB guideline is a soft reliability threshold; for larger changes, split the work into several focused Edit calls by section, function, or component."
                },
                "replace_all": {
                    "type": "boolean",
                    "default": false,
                    "description": "Replace all occurences of old_string (default false)"
                }
            },
            "required": ["file_path", "old_string", "new_string"],
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

        if input.get("old_string").is_none() {
            return ValidationResult {
                result: false,
                message: Some("old_string is required".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        if input.get("new_string").is_none() {
            return ValidationResult {
                result: false,
                message: Some("new_string is required".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

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

            if let Err(err) = ctx.enforce_path_operation(ToolPathOperation::Edit, &resolved) {
                return ValidationResult {
                    result: false,
                    message: Some(err.to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        let old_string = input
            .get("old_string")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let new_string = input
            .get("new_string")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let largest_lines = old_string.lines().count().max(new_string.lines().count());
        let largest_bytes = old_string.len().max(new_string.len());
        if largest_lines > LARGE_EDIT_SOFT_LINE_LIMIT || largest_bytes > LARGE_EDIT_SOFT_BYTE_LIMIT
        {
            return ValidationResult {
                result: true,
                message: Some(format!(
                    "Large Edit payload: largest side is {} lines, {} bytes. This is allowed when necessary, but prefer a staged approach: split the change into several focused Edit calls by section, function, or component instead of one huge replacement.",
                    largest_lines, largest_bytes
                )),
                error_code: None,
                meta: Some(json!({
                    "large_edit": true,
                    "largest_line_count": largest_lines,
                    "largest_byte_count": largest_bytes,
                    "soft_line_limit": LARGE_EDIT_SOFT_LINE_LIMIT,
                    "soft_byte_limit": LARGE_EDIT_SOFT_BYTE_LIMIT
                })),
            };
        }

        ValidationResult::default()
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

        let new_string = input
            .get("new_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("new_string is required".to_string()))?;

        let old_string = input
            .get("old_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("old_string is required".to_string()))?;

        let replace_all = input
            .get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let resolved = context.resolve_tool_path(file_path)?;
        context.enforce_path_operation(ToolPathOperation::Edit, &resolved)?;

        // For remote workspace paths, use the abstract FS to read → edit in memory → write back.
        if resolved.uses_remote_workspace_backend() {
            let ws_fs = context.ws_fs().ok_or_else(|| {
                BitFunError::tool("Remote workspace file system is unavailable".to_string())
            })?;
            let content = ws_fs
                .read_file_text(&resolved.resolved_path)
                .await
                .map_err(|e| BitFunError::tool(format!("Failed to read file: {}", e)))?;
            let edit_result = apply_edit_to_content(&content, old_string, new_string, replace_all)
                .map_err(BitFunError::tool)?;

            ws_fs
                .write_file(&resolved.resolved_path, edit_result.new_content.as_bytes())
                .await
                .map_err(|e| BitFunError::tool(format!("Failed to write file: {}", e)))?;

            let result = ToolResult::Result {
                data: json!({
                    "file_path": resolved.logical_path,
                    "old_string": old_string,
                    "new_string": new_string,
                    "success": true,
                    "match_count": edit_result.match_count,
                    "start_line": edit_result.edit_result.start_line,
                    "old_end_line": edit_result.edit_result.old_end_line,
                    "new_end_line": edit_result.edit_result.new_end_line,
                }),
                result_for_assistant: Some(format!(
                    "Successfully edited {}",
                    resolved.logical_path
                )),
                image_attachments: None,
            };
            return Ok(vec![result]);
        }

        // Local: direct local edit via tool-runtime
        let edit_result = edit_file(&resolved.resolved_path, old_string, new_string, replace_all)?;

        let result = ToolResult::Result {
            data: json!({
                "file_path": resolved.logical_path,
                "old_string": old_string,
                "new_string": new_string,
                "success": true,
                "start_line": edit_result.start_line,
                "old_end_line": edit_result.old_end_line,
                "new_end_line": edit_result.new_end_line,
            }),
            result_for_assistant: Some(format!("Successfully edited {}", resolved.logical_path)),
            image_attachments: None,
        };

        Ok(vec![result])
    }
}
