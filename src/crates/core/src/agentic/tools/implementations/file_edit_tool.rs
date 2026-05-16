use crate::agentic::tools::ToolPathOperation;
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{Value, json};
use tool_runtime::fs::edit_file::{apply_edit_to_content, edit_file};

pub struct FileEditTool;

const LARGE_EDIT_SOFT_LINE_LIMIT: usize = 200;
const LARGE_EDIT_SOFT_BYTE_LIMIT: usize = 20 * 1024;
const EDIT_RETRY_GUIDANCE: &str = "Common causes: stale Read output after another edit, copied line-number prefixes, changed whitespace, or an old_string that is too broad. Recovery: read the current target area again, copy the exact current text after any line-number prefix, and retry with a uniquely matching old_string. If several edits target the same file, apply them sequentially from fresh content or replace one stable enclosing block. If the text appears more than once, include more surrounding context or set replace_all only when every occurrence should change.";

impl Default for FileEditTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FileEditTool {
    pub fn new() -> Self {
        Self
    }

    fn enhance_edit_error(file_path: &str, error: String) -> String {
        if error.contains("old_string not found in file") || error.contains("`old_string` appears")
        {
            format!(
                "Edit failed for {}: {}\n{}",
                file_path, error, EDIT_RETRY_GUIDANCE
            )
        } else {
            error
        }
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
- Use the Read tool before editing so `old_string` is based on current file content.
- Treat Read output as stale after any successful edit to the same file. For multiple edits in one file, either apply them sequentially from fresh content or replace a stable enclosing block once.
- The file_path parameter must be workspace-relative, an absolute path inside the current workspace, or an exact `bitfun://runtime/...` URI returned by another tool.
- Build `old_string` from current file contents rather than from memory, an intended final version, or a guessed retry.
- When editing text from Read output, copy only the text after the line-number prefix and preserve indentation exactly.
- Prefer editing existing files in the codebase; create new files only when the task genuinely calls for a new artifact.
- Avoid adding emojis to files unless the user asks.
- The edit requires `old_string` to be unique unless `replace_all` is true. Add surrounding context from the same stable block when a snippet may appear more than once, or use `replace_all` when every occurrence should change.
- If an edit fails because `old_string` was not found or matched multiple places, read the current target area again before retrying. Do not retry by slightly modifying the failed `old_string` from memory.
- Keep edits focused. Large replacements are allowed when necessary, but staged section/function/component edits are usually more reliable than one huge replacement.
- Use `replace_all` for intentional file-wide replacements, such as renaming a variable."#
        .to_string())
    }

    fn short_description(&self) -> String {
        "Apply exact string replacements to an existing file.".to_string()
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
                    "description": "The exact current text to replace. It must match the current file contents exactly, including whitespace and indentation, and must be unique unless replace_all is true. Copy it from a fresh Read result, excluding the line-number prefix. If this file was edited earlier in the turn, read the target area again before building old_string. Include stable surrounding context when a short snippet may appear multiple times."
                },
                "new_string": {
                    "type": "string",
                    "description": "The replacement text. It must be different from old_string. Keep edits targeted. Large replacements are allowed when necessary; focused edits by section, function, or component are usually more reliable."
                },
                "replace_all": {
                    "type": "boolean",
                    "default": false,
                    "description": "Replace all occurrences of old_string (default false). Use only when every occurrence should change."
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
                    "Large Edit payload: largest side is {} lines, {} bytes. This is allowed when necessary, but a staged approach is usually more reliable: edit one stable section, function, or component at a time, and refresh file context before additional edits to the same file.",
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
        context
            .record_light_checkpoint(
                "Edit",
                &resolved.logical_path,
                vec![resolved.logical_path.clone()],
            )
            .await;

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
                .map_err(|e| BitFunError::tool(Self::enhance_edit_error(file_path, e)))?;

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
        let edit_result = edit_file(&resolved.resolved_path, old_string, new_string, replace_all)
            .map_err(|e| BitFunError::tool(Self::enhance_edit_error(file_path, e)))?;

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

#[cfg(test)]
mod tests {
    use super::FileEditTool;

    #[test]
    fn edit_not_found_error_includes_retry_guidance() {
        let message = FileEditTool::enhance_edit_error(
            "src/lib.rs",
            "old_string not found in file.".to_string(),
        );

        assert!(message.contains("Edit failed for src/lib.rs"));
        assert!(message.contains("Common causes"));
        assert!(message.contains("stale Read output"));
        assert!(message.contains("read the current target area again"));
    }

    #[test]
    fn edit_multiple_match_error_includes_unique_context_guidance() {
        let message = FileEditTool::enhance_edit_error(
            "src/lib.rs",
            "`old_string` appears 2 times in file".to_string(),
        );

        assert!(message.contains("old_string"));
        assert!(message.contains("include more surrounding context"));
        assert!(message.contains("replace_all only when every occurrence should change"));
    }
}
