use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::workspace_paths::is_bitfun_runtime_uri;
use crate::agentic::tools::ToolPathOperation;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use log::debug;
use serde_json::{json, Value};
use std::path::Path;
use tokio::fs;

/// File deletion tool - provides safe file/directory deletion functionality
///
/// This tool records a lightweight checkpoint before deletion. Rollback is not automatic.
pub struct DeleteFileTool;

impl Default for DeleteFileTool {
    fn default() -> Self {
        Self::new()
    }
}

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
        Ok(r#"Deletes a file or directory from the filesystem. This operation records a lightweight checkpoint before deletion, but rollback is not automatic.

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
   - You can use either relative paths (e.g., "temp/data.txt"), absolute paths (e.g., "/workspace/temp/data.txt"), or exact `bitfun://runtime/...` URIs returned by another tool
   - Relative paths will be automatically resolved relative to the workspace directory
   - The path must exist in the filesystem

4. **Safety Features**:
    - Deletions record a lightweight checkpoint when session context is available
    - The checkpoint captures Git branch/dirty-state metadata when cheap
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
 - This tool provides better safety through checkpoint metadata
 - Rollback is not automatic; use the recorded checkpoint metadata to guide recovery
 - The tool will fail gracefully if permissions are insufficient"#.to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The absolute path to the file or directory to delete, or an exact bitfun://runtime URI returned by another tool"
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
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
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

        let resolved = match context.map(|ctx| ctx.resolve_tool_path(path_str)) {
            Some(Ok(value)) => value,
            Some(Err(err)) => {
                return ValidationResult {
                    result: false,
                    message: Some(err.to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
            None => {
                if is_bitfun_runtime_uri(path_str) {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "Tool context is required to resolve bitfun runtime URIs".to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                let local_path = Path::new(path_str);
                if !local_path.is_absolute() {
                    return ValidationResult {
                        result: false,
                        message: Some("path must be an absolute path".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                if !local_path.exists() {
                    return ValidationResult {
                        result: false,
                        message: Some(format!("Path does not exist: {}", path_str)),
                        error_code: Some(404),
                        meta: None,
                    };
                }

                return ValidationResult {
                    result: true,
                    message: None,
                    error_code: None,
                    meta: None,
                };
            }
        };

        if let Some(ctx) = context {
            if let Err(err) = ctx.enforce_path_operation(ToolPathOperation::Delete, &resolved) {
                return ValidationResult {
                    result: false,
                    message: Some(err.to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        if !resolved.uses_remote_workspace_backend() {
            let local_path = Path::new(&resolved.resolved_path);
            if !local_path.exists() {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Path does not exist: {}", resolved.logical_path)),
                    error_code: Some(404),
                    meta: None,
                };
            }

            if local_path.is_dir() {
                let recursive = input
                    .get("recursive")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let is_empty = match fs::read_dir(local_path).await {
                    Ok(mut entries) => entries.next_entry().await.ok().flatten().is_none(),
                    Err(_) => false,
                };

                if !is_empty && !recursive {
                    return ValidationResult {
                            result: false,
                            message: Some(format!("Directory is not empty: {}. Set recursive=true to delete non-empty directories", resolved.logical_path)),
                            error_code: Some(400),
                            meta: Some(json!({
                                "is_directory": true,
                            "is_empty": false,
                            "requires_recursive": true
                        })),
                    };
                }
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
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let path_str = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("path is required".to_string()))?;

        let recursive = input
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let resolved = context.resolve_tool_path(path_str)?;
        context.enforce_path_operation(ToolPathOperation::Delete, &resolved)?;
        context
            .record_light_checkpoint(
                "Delete",
                &resolved.logical_path,
                vec![resolved.logical_path.clone()],
            )
            .await;

        // Remote workspace path: delete via shell command
        if resolved.uses_remote_workspace_backend() {
            let ws_shell = context.ws_shell().ok_or_else(|| {
                BitFunError::tool("Workspace shell not available for remote Delete".to_string())
            })?;

            let rm_cmd = if recursive {
                format!("rm -rf '{}'", resolved.resolved_path.replace('\'', "'\\''"))
            } else {
                format!("rm -f '{}'", resolved.resolved_path.replace('\'', "'\\''"))
            };

            let (_stdout, stderr, exit_code) = ws_shell
                .exec(&rm_cmd, Some(15_000))
                .await
                .map_err(|e| BitFunError::tool(format!("Failed to delete on remote: {}", e)))?;

            if exit_code != 0 && !stderr.is_empty() {
                return Err(BitFunError::tool(format!(
                    "Remote delete failed: {}",
                    stderr
                )));
            }

            let result_data = json!({
                "success": true,
                "path": resolved.logical_path,
                "is_directory": recursive,
                "recursive": recursive,
                "is_remote": true
            });
            let result_text = self.render_result_for_assistant(&result_data);
            return Ok(vec![ToolResult::Result {
                data: result_data,
                result_for_assistant: Some(result_text),
                image_attachments: None,
            }]);
        }

        let path = Path::new(&resolved.resolved_path);
        let is_directory = path.is_dir();

        debug!(
            "DeleteFile tool deleting {}: {}",
            if is_directory { "directory" } else { "file" },
            resolved.logical_path
        );

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

        let result_data = json!({
            "success": true,
            "path": resolved.logical_path,
            "is_directory": is_directory,
            "recursive": recursive
        });

        let result_text = self.render_result_for_assistant(&result_data);

        Ok(vec![ToolResult::Result {
            data: result_data,
            result_for_assistant: Some(result_text),
            image_attachments: None,
        }])
    }
}
