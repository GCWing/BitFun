use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use log::debug;
use serde_json::{json, Value};
use std::path::Path;
use tokio::fs;

/// File deletion tool - provides safe file/directory deletion functionality
///
/// This tool automatically integrates with the snapshot system, all deletion operations are recorded and support rollback
pub struct DeleteFileTool;

impl DeleteFileTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for DeleteFileTool {
    fn name(&self) -> &str {
        "Delete"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Deletes a file or directory from the filesystem. This operation is tracked by the snapshot system and can be rolled back if needed.

Usage guidelines:
1. **File Deletion**:
   - Provide the path to the file you want to delete (relative or absolute)
   - The file must exist and be accessible
   - Example: Delete a single file like `old_file.txt` or `/path/to/file.txt`

2. **Directory Deletion**:
   - For empty directories, just provide the path
   - For non-empty directories, you MUST set `recursive: true`
   - Be careful with recursive deletion as it will remove all contents

3. **Path Requirements**:
   - You can use either relative paths (e.g., "temp/data.txt") or absolute paths (e.g., "/workspace/temp/data.txt")
   - Relative paths will be automatically resolved relative to the workspace directory
   - The path must exist in the filesystem

4. **Safety Features**:
    - All deletions are tracked by the snapshot system
    - Users can review and roll back deletions if needed
    - The tool requires user confirmation for execution

5. **Best Practices**:
   - Before deleting, consider using the Read or LS tools to verify the target
   - For directories, use LS to check contents before recursive deletion
   - Prefer this tool over bash `rm` commands for better tracking and safety

Example usage:
```json
{
  "path": "/workspace/old_file.txt"
}
```

Example for directory:
```json
{
  "path": "/workspace/temp_folder",
  "recursive": true
}
```

Important notes:
 - NEVER use bash `rm` commands when this tool is available
 - This tool provides better safety through the snapshot system
 - All deletions can be rolled back through the snapshot interface
 - The tool will fail gracefully if permissions are insufficient"#.to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The absolute path to the file or directory to delete"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "If true, recursively delete directories and their contents. Required when deleting non-empty directories. Default: false"
                }
            },
            "required": ["path"]
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        // Validate path parameter
        let path_str = match input.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return ValidationResult {
                    result: false,
                    message: Some("path parameter is required".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        if path_str.is_empty() {
            return ValidationResult {
                result: false,
                message: Some("path cannot be empty".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        let path = Path::new(path_str);

        // Validate if path is absolute
        if !path.is_absolute() {
            return ValidationResult {
                result: false,
                message: Some("path must be an absolute path".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        // Validate if path exists
        if !path.exists() {
            return ValidationResult {
                result: false,
                message: Some(format!("Path does not exist: {}", path_str)),
                error_code: Some(404),
                meta: None,
            };
        }

        // If directory, check if recursive deletion is needed
        if path.is_dir() {
            let recursive = input
                .get("recursive")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            // Check if directory is empty
            let is_empty = match fs::read_dir(path).await {
                Ok(mut entries) => entries.next_entry().await.ok().flatten().is_none(),
                Err(_) => false,
            };

            if !is_empty && !recursive {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Directory is not empty: {}. Set recursive=true to delete non-empty directories", path_str)),
                    error_code: Some(400),
                    meta: Some(json!({
                        "is_directory": true,
                        "is_empty": false,
                        "requires_recursive": true
                    })),
                };
            }
        }

        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
            let recursive = input
                .get("recursive")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if recursive {
                format!("Deleting directory and contents: {}", path)
            } else {
                format!("Deleting: {}", path)
            }
        } else {
            "Deleting file or directory".to_string()
        }
    }

    fn render_result_for_assistant(&self, output: &Value) -> String {
        if let Some(path) = output.get("path").and_then(|v| v.as_str()) {
            let is_directory = output
                .get("is_directory")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let type_name = if is_directory { "directory" } else { "file" };

            format!("Successfully deleted {} at: {}", type_name, path)
        } else {
            "Deletion completed".to_string()
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let path_str = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("path is required".to_string()))?;

        let recursive = input
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let path = Path::new(path_str);
        let is_directory = path.is_dir();

        debug!(
            "DeleteFile tool deleting {}: {}",
            if is_directory { "directory" } else { "file" },
            path_str
        );

        // Execute deletion operation
        if is_directory {
            if recursive {
                fs::remove_dir_all(path)
                    .await
                    .map_err(|e| BitFunError::tool(format!("Failed to delete directory: {}", e)))?;
            } else {
                fs::remove_dir(path)
                    .await
                    .map_err(|e| BitFunError::tool(format!("Failed to delete directory: {}", e)))?;
            }
        } else {
            fs::remove_file(path)
                .await
                .map_err(|e| BitFunError::tool(format!("Failed to delete file: {}", e)))?;
        }

        // Build result
        let result_data = json!({
            "success": true,
            "path": path_str,
            "is_directory": is_directory,
            "recursive": recursive
        });

        let result_text = self.render_result_for_assistant(&result_data);

        Ok(vec![ToolResult::Result {
            data: result_data,
            result_for_assistant: Some(result_text),
        }])
    }
}
