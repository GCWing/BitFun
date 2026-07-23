use crate::agentic::tools::file_permissions::file_permission_intents;
use crate::agentic::tools::file_read_state_runtime::{
    assert_file_not_unexpectedly_modified, file_mutation_timestamp_ms, get_stored_file_read_state,
    local_file_modification_time_ms, read_current_file_content, read_state_tracking_enabled,
    update_file_read_state_after_mutation, validate_edit_against_read_state,
    validate_edit_has_prior_read, FILE_UNEXPECTEDLY_MODIFIED_ERROR,
};
use crate::agentic::tools::file_tool_guidance::file_tool_guidance_message;
use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolPathResolution, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::ToolPathOperation;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;
use tool_runtime::fs::edit_file::{
    apply_edit_to_content, edit_local_file_with_content, edit_success_message,
    is_edit_content_guardrail_error, EditLocalFileWithContentRequest,
};

pub struct FileEditTool;

const EDIT_TOOL_PROMPT: &str = r#"Performs exact string replacements in files.

Usage:
- You must use your `Read` tool at least once in the conversation before editing. This tool will error if you attempt an edit without reading the file.
- The `file_path` parameter must be a workspace-relative path, an absolute path inside the current workspace, or an exact `bitfun://...` URI returned by another tool.
- When editing text from Read tool output, ensure you preserve the exact indentation (tabs/spaces) as it appears AFTER the line number prefix. The line number prefix format is: spaces + line number + tab. Everything after that is the actual file content to match. Never include any part of the line number prefix in the old_string or new_string.
- Copy `old_string` verbatim from your latest Read of this file. Do not reformat HTML/CSS/JS, do not normalize indentation, and do not reconstruct the block from memory.
- Use the smallest `old_string` that is clearly unique — usually 2-4 adjacent lines with stable surrounding context is sufficient.
- If Read output was truncated or used start_line/limit, re-read until the full target block is visible before editing.
- ALWAYS prefer editing existing files in the codebase. NEVER write new files unless explicitly required.
- Only use emojis if the user explicitly requests it. Avoid adding emojis to files unless asked.
- The edit will FAIL if `old_string` is not unique in the file. Either provide a larger string with more surrounding context to make it unique or use `replace_all` to change every instance of `old_string`.
- Use `replace_all` for replacing and renaming strings across the file. This parameter is useful if you want to rename a variable for instance.
- If an edit fails because the text was not found, call Read again on the target lines and retry with a freshly copied `old_string`."#;

impl Default for FileEditTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FileEditTool {
    pub fn new() -> Self {
        Self
    }

    fn guidance_failure(message: String) -> ValidationResult {
        ValidationResult {
            result: false,
            message: Some(file_tool_guidance_message(message)),
            error_code: Some(400),
            meta: Some(json!({ "failure_kind": "guidance" })),
        }
    }

    fn format_edit_freshness_guidance(logical_path: &str, error: String) -> String {
        if error == FILE_UNEXPECTEDLY_MODIFIED_ERROR || error.contains("unexpectedly modified") {
            format!(
                "The file {} changed since it was last read. Use Read again, then retry Edit.",
                logical_path
            )
        } else {
            error
        }
    }

    async fn edit_read_state_guardrail_error(
        context: &ToolUseContext,
        resolved: &ToolPathResolution,
    ) -> Option<String> {
        if let Some(message) = validate_edit_has_prior_read(context, resolved) {
            return Some(message);
        }

        validate_edit_against_read_state(context, resolved).await
    }

    fn assert_atomic_edit_freshness(
        context: &ToolUseContext,
        resolved: &ToolPathResolution,
        content: &str,
    ) -> BitFunResult<()> {
        if !read_state_tracking_enabled(context) {
            return Ok(());
        }

        let read_state = get_stored_file_read_state(context, resolved);
        let current_mtime_ms = if resolved.uses_remote_workspace_backend() {
            None
        } else {
            Some(local_file_modification_time_ms(Path::new(
                &resolved.resolved_path,
            )))
        };

        if let Some(error) =
            assert_file_not_unexpectedly_modified(read_state.as_ref(), content, current_mtime_ms)
                .err()
        {
            return Err(BitFunError::tool(file_tool_guidance_message(
                Self::format_edit_freshness_guidance(&resolved.logical_path, error),
            )));
        }

        Ok(())
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
                    "description": "Exact text to replace, copied verbatim from your latest Read of this file (content after the line-number tab only). Preserve indentation; do not reformat."
                },
                "new_string": {
                    "type": "string",
                    "description": "Replacement text with the same indentation style as old_string (must be different from old_string)"
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

    fn permission_intents(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<PermissionIntent>> {
        let file_path = input
            .get("file_path")
            .and_then(Value::as_str)
            .ok_or_else(|| BitFunError::validation("file_path is required".to_string()))?;
        file_permission_intents("edit", [file_path], context)
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

        let force = input
            .get("force")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if let Some(rejection) = crate::agentic::execution::edit_constraint_guard::check_edit(
            context, "Edit", "edit", file_path, force,
        ) {
            return rejection;
        }

        let old_string = input
            .get("old_string")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let new_string = input
            .get("new_string")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let replace_all = input
            .get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

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

            if let Some(message) = Self::edit_read_state_guardrail_error(ctx, &resolved).await {
                return Self::guidance_failure(message);
            }

            let file_content = match read_current_file_content(ctx, &resolved).await {
                Ok(content) => content,
                Err(error) => {
                    return ValidationResult {
                        result: false,
                        message: Some(format!(
                            "Failed to read file {}: {}",
                            resolved.logical_path, error
                        )),
                        error_code: Some(400),
                        meta: None,
                    };
                }
            };

            if let Err(error) =
                apply_edit_to_content(&file_content, old_string, new_string, replace_all)
            {
                if is_edit_content_guardrail_error(&error) {
                    return Self::guidance_failure(error);
                }

                return ValidationResult {
                    result: false,
                    message: Some(error),
                    error_code: Some(400),
                    meta: None,
                };
            }
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
            Self::assert_atomic_edit_freshness(context, &resolved, &content)?;
            let edit_result = apply_edit_to_content(&content, old_string, new_string, replace_all)
                .map_err(|error| {
                    if is_edit_content_guardrail_error(&error) {
                        BitFunError::tool(file_tool_guidance_message(error))
                    } else {
                        BitFunError::tool(error)
                    }
                })?;

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
            crate::agentic::execution::edit_constraint_guard::record_mutation_applied(
                context,
                "Edit",
                "edit",
                &resolved.logical_path,
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
                result_for_assistant: Some(edit_success_message(&resolved.logical_path)),
                image_attachments: None,
            };
            return Ok(vec![result]);
        }

        // Local: core keeps freshness/checkpoint, tool-runtime owns edit application and write-back.
        let content = std::fs::read_to_string(&resolved.resolved_path).map_err(|e| {
            BitFunError::tool(format!(
                "Failed to read file {}: {}",
                resolved.logical_path, e
            ))
        })?;
        Self::assert_atomic_edit_freshness(context, &resolved, &content)?;
        let edit_result = edit_local_file_with_content(EditLocalFileWithContentRequest {
            logical_path: resolved.logical_path.clone(),
            resolved_path: Path::new(&resolved.resolved_path).to_path_buf(),
            current_content: content,
            old_string: old_string.to_string(),
            new_string: new_string.to_string(),
            replace_all,
        })
        .map_err(|error| {
            if is_edit_content_guardrail_error(&error) {
                BitFunError::tool(file_tool_guidance_message(error))
            } else {
                BitFunError::tool(error)
            }
        })?;

        let timestamp_ms = file_mutation_timestamp_ms(context, &resolved).await;
        update_file_read_state_after_mutation(
            context,
            &resolved,
            &edit_result.new_content,
            timestamp_ms,
        );
        crate::agentic::execution::edit_constraint_guard::record_mutation_applied(
            context,
            "Edit",
            "edit",
            &resolved.logical_path,
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
            result_for_assistant: Some(edit_success_message(&resolved.logical_path)),
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
        let description = FileEditTool::new()
            .description()
            .await
            .expect("description");

        assert_eq!(description, EDIT_TOOL_PROMPT);
        assert!(description.contains("You must use your `Read` tool"));
        assert!(description.contains("spaces + line number + tab"));
        assert!(description.contains("verbatim from your latest Read"));
        assert!(description.contains("NEVER write new files unless explicitly required"));
        assert!(!description.contains("auto-strip"));
    }

    #[test]
    fn edit_tool_schema_describes_exact_copy_from_read() {
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
        assert!(properties
            .get("old_string")
            .and_then(|value| value.get("description"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("latest Read"));
        assert!(properties
            .get("new_string")
            .and_then(|value| value.get("description"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("indentation"));
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
        assert!(properties.get("force").is_none());
    }

    #[test]
    fn edit_tool_short_description_matches_claude_summary() {
        assert_eq!(
            FileEditTool::new().short_description(),
            "A tool for editing files"
        );
    }
}
