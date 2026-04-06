use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use tool_runtime::fs::edit_file::{apply_edit_to_content, edit_file};

pub struct FileEditTool;

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
- When editing text from Read tool output, ensure you preserve the exact indentation (tabs/spaces) as it appears AFTER the line number prefix. The line number prefix format is: spaces + line number + tab. Everything after that tab is the actual file content to match. Never include any part of the line number prefix in the old_string or new_string.
- ALWAYS prefer editing existing files in the codebase. NEVER write new files unless explicitly required.
- Only use emojis if the user explicitly requests it. Avoid adding emojis to files unless asked.
- The edit will FAIL if `old_string` is not unique in the file. Either provide a larger string with more surrounding context to make it unique or use `replace_all` to change every instance of `old_string`.
- Use `replace_all` for replacing and renaming strings across the file. This parameter is useful if you want to rename a variable for instance."#
        .to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to modify"
                },
                "old_string": {
                    "type": "string",
                    "default": "",
                    "description": "The text to replace (must be unique within the file, and must match the file contents exactly, including all whitespace and indentation)"
                },
                "new_string": {
                    "type": "string",
                    "description": "The text to replace it with (must be different from old_string)"
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

        let resolved_path = context.resolve_workspace_tool_path(file_path)?;

        // For remote workspaces, use the abstract FS to read → edit in memory → write back.
        if context.is_remote() {
            let ws_fs = context.ws_fs().ok_or_else(|| {
                BitFunError::tool("Remote workspace file system is unavailable".to_string())
            })?;
            let content = ws_fs
                .read_file_text(&resolved_path)
                .await
                .map_err(|e| BitFunError::tool(format!("Failed to read file: {}", e)))?;
            let edit_result = apply_edit_to_content(&content, old_string, new_string, replace_all)
                .map_err(BitFunError::tool)?;

            ws_fs
                .write_file(&resolved_path, edit_result.new_content.as_bytes())
                .await
                .map_err(|e| BitFunError::tool(format!("Failed to write file: {}", e)))?;

            let result = ToolResult::Result {
                data: json!({
                    "file_path": resolved_path,
                    "old_string": old_string,
                    "new_string": new_string,
                    "success": true,
                    "match_count": edit_result.match_count,
                    "start_line": edit_result.edit_result.start_line,
                    "old_end_line": edit_result.edit_result.old_end_line,
                    "new_end_line": edit_result.edit_result.new_end_line,
                }),
                result_for_assistant: Some(format!("Successfully edited {}", resolved_path)),
                image_attachments: None,
            };
            return Ok(vec![result]);
        }

        // Local: direct local edit via tool-runtime
        let edit_result = edit_file(&resolved_path, old_string, new_string, replace_all)?;

        let result = ToolResult::Result {
            data: json!({
                "file_path": resolved_path,
                "old_string": old_string,
                "new_string": new_string,
                "success": true,
                "start_line": edit_result.start_line,
                "old_end_line": edit_result.old_end_line,
                "new_end_line": edit_result.new_end_line,
            }),
            result_for_assistant: Some(format!("Successfully edited {}", resolved_path)),
            image_attachments: None,
        };

        Ok(vec![result])
    }
}
