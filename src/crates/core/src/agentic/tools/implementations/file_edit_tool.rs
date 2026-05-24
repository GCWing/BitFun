use crate::agentic::tools::file_read_state_runtime::{
    file_mutation_timestamp_ms, update_file_read_state_after_mutation,
    validate_edit_against_read_state, validate_edit_has_prior_read,
};
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::agentic::tools::ToolPathOperation;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use tool_runtime::fs::edit_file::apply_edit_to_content;

pub struct FileEditTool;

const LARGE_EDIT_SOFT_LINE_LIMIT: usize = 200;
const LARGE_EDIT_SOFT_BYTE_LIMIT: usize = 20 * 1024;
const EDIT_TOOL_PROMPT: &str = r#"Performs exact string replacements in files.

Usage:
- You must use your `Read` tool at least once in the conversation before editing. This tool will error if you attempt an edit without reading the file.
- The `file_path` parameter must be a workspace-relative path, an absolute path inside the current workspace, or an exact `bitfun://runtime/...` URI returned by another tool.
- When editing text from Read tool output, ensure you preserve the exact indentation (tabs/spaces) as it appears AFTER the line number prefix. The line number prefix format is: spaces + line number + tab. Everything after that is the actual file content to match. Never include any part of the line number prefix in the old_string or new_string.
- ALWAYS prefer editing existing files in the codebase. NEVER write new files unless explicitly required.
- Only use emojis if the user explicitly requests it. Avoid adding emojis to files unless asked.
- The edit will FAIL if `old_string` is not unique in the file. Either provide a larger string with more surrounding context to make it unique or use `replace_all` to change every instance of `old_string`.
- Use `replace_all` for replacing and renaming strings across the file. This parameter is useful if you want to rename a variable for instance."#;

const EDIT_RETRY_GUIDANCE: &str = "Common causes: stale Read output after another edit, copied Read line-number prefixes, changed whitespace, truncated Read lines, or an old_string that is too broad. Recovery: read the current target area again, copy the exact current text after the tab on each Read line, and retry with a uniquely matching old_string. If several edits target the same file, apply them sequentially from fresh content or replace one stable enclosing block. If the text appears more than once, include more surrounding context or set replace_all only when every occurrence should change.";

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
        Ok(EDIT_TOOL_PROMPT.to_string())
    }

    fn short_description(&self) -> String {
        "A tool for editing files".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path to the file to modify"
                },
                "old_string": {
                    "type": "string",
                    "description": "The text to replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The text to replace it with (must be different from old_string)"
                },
                "replace_all": {
                    "type": "boolean",
                    "default": false,
                    "description": "Replace all occurrences of old_string (default false)"
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

            if let Some(message) = validate_edit_has_prior_read(ctx, &resolved) {
                return ValidationResult {
                    result: false,
                    message: Some(message),
                    error_code: Some(400),
                    meta: None,
                };
            }

            if let Some(message) = validate_edit_against_read_state(ctx, &resolved).await {
                return ValidationResult {
                    result: false,
                    message: Some(message),
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
        if old_string.is_empty() {
            return ValidationResult {
                result: false,
                message: Some("old_string cannot be empty".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }
        if old_string == new_string {
            return ValidationResult {
                result: false,
                message: Some("new_string must be different from old_string".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

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

            let timestamp_ms = file_mutation_timestamp_ms(context, &resolved).await;
            update_file_read_state_after_mutation(
                context,
                &resolved,
                &edit_result.new_content,
                timestamp_ms,
            );

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

        // Local: read → edit in memory → write back so failures can include current file context.
        let content = std::fs::read_to_string(&resolved.resolved_path).map_err(|e| {
            BitFunError::tool(format!(
                "Failed to read file {}: {}",
                resolved.logical_path, e
            ))
        })?;
        let edit_result = apply_edit_to_content(&content, old_string, new_string, replace_all)
            .map_err(|e| BitFunError::tool(Self::enhance_edit_error(file_path, e)))?;

        std::fs::write(&resolved.resolved_path, edit_result.new_content.as_bytes()).map_err(
            |e| {
                BitFunError::tool(format!(
                    "Failed to write file {}: {}",
                    resolved.logical_path, e
                ))
            },
        )?;

        let timestamp_ms = file_mutation_timestamp_ms(context, &resolved).await;
        update_file_read_state_after_mutation(
            context,
            &resolved,
            &edit_result.new_content,
            timestamp_ms,
        );

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
            result_for_assistant: Some(format!("Successfully edited {}", resolved.logical_path)),
            image_attachments: None,
        };

        Ok(vec![result])
    }
}

#[cfg(test)]
mod tests {
    use super::{FileEditTool, EDIT_TOOL_PROMPT};
    use crate::agentic::tools::framework::Tool;
    use serde_json::Value;

    #[tokio::test]
    async fn edit_tool_prompt_matches_claude_style() {
        let description = FileEditTool::new().description().await.expect("description");

        assert_eq!(description, EDIT_TOOL_PROMPT);
        assert!(description.contains("You must use your `Read` tool"));
        assert!(description.contains("spaces + line number + tab"));
        assert!(description.contains("NEVER write new files unless explicitly required"));
        assert!(!description.contains("Large Edit payload"));
        assert!(!description.contains("auto-strip"));
    }

    #[test]
    fn edit_tool_schema_uses_minimal_parameter_descriptions() {
        let schema = FileEditTool::new().input_schema();
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");

        assert_eq!(
            properties
                .get("file_path")
                .and_then(|value| value.get("description"))
                .and_then(Value::as_str),
            Some("The path to the file to modify")
        );
        assert_eq!(
            properties
                .get("old_string")
                .and_then(|value| value.get("description"))
                .and_then(Value::as_str),
            Some("The text to replace")
        );
        assert_eq!(
            properties
                .get("new_string")
                .and_then(|value| value.get("description"))
                .and_then(Value::as_str),
            Some("The text to replace it with (must be different from old_string)")
        );
        assert_eq!(
            properties
                .get("replace_all")
                .and_then(|value| value.get("description"))
                .and_then(Value::as_str),
            Some("Replace all occurrences of old_string (default false)")
        );
        assert!(properties
            .get("old_string")
            .and_then(|value| value.get("minLength"))
            .is_none());
    }

    #[test]
    fn edit_tool_short_description_matches_claude_summary() {
        assert_eq!(
            FileEditTool::new().short_description(),
            "A tool for editing files"
        );
    }

    #[test]
    fn edit_not_found_error_includes_retry_guidance() {
        let message = FileEditTool::enhance_edit_error(
            "src/lib.rs",
            "old_string not found in file.\n[nearby content around line 2]\nfn main() {}".to_string(),
        );

        assert!(message.contains("Edit failed for src/lib.rs"));
        assert!(message.contains("Common causes"));
        assert!(message.contains("stale Read output"));
        assert!(message.contains("read the current target area again"));
        assert!(message.contains("[nearby content around line 2]"));
    }

    #[test]
    fn edit_multiple_match_error_includes_unique_context_guidance() {
        let message = FileEditTool::enhance_edit_error(
            "src/lib.rs",
            "`old_string` appears 2 times in file\nMatched contexts:\n[match 1 starts at line 4]"
                .to_string(),
        );

        assert!(message.contains("old_string"));
        assert!(message.contains("[match 1 starts at line 4]"));
        assert!(message.contains("include more surrounding context"));
        assert!(message.contains("replace_all only when every occurrence should change"));
    }
}
