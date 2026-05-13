use crate::agentic::tools::ToolPathOperation;
use crate::agentic::tools::framework::{
    Tool, ToolPathResolution, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::path::Path;
use tokio::fs;

pub struct FileWriteTool;

impl Default for FileWriteTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FileWriteTool {
    pub fn new() -> Self {
        Self
    }

    pub(crate) async fn existing_file_error(
        context: &ToolUseContext,
        resolved: &ToolPathResolution,
    ) -> Option<String> {
        let file_already_exists = Self::file_exists(context, resolved).await;

        file_already_exists.then(|| {
            format!(
                "File {} already exists. The Write tool is reserved for creating NEW files. \
                 To modify the file, use the Edit tool. \
                 To fully rewrite the file, first call the Delete tool on this path, then call Write again.",
                resolved.logical_path
            )
        })
    }

    async fn file_exists(context: &ToolUseContext, resolved: &ToolPathResolution) -> bool {
        if resolved.uses_remote_workspace_backend() {
            if let Some(ws_fs) = context.ws_fs() {
                ws_fs.exists(&resolved.resolved_path).await.unwrap_or(false)
            } else {
                false
            }
        } else {
            Path::new(&resolved.resolved_path).exists()
        }
    }

    async fn existing_file_matches_content(
        context: &ToolUseContext,
        resolved: &ToolPathResolution,
        content: &str,
    ) -> Option<bool> {
        let existing = if resolved.uses_remote_workspace_backend() {
            context
                .ws_fs()?
                .read_file(&resolved.resolved_path)
                .await
                .ok()?
        } else {
            fs::read(&resolved.resolved_path).await.ok()?
        };

        Some(existing == content.as_bytes())
    }

    fn write_success_result(
        logical_path: &str,
        bytes_written: usize,
        status: &str,
        assistant_message: String,
    ) -> ToolResult {
        ToolResult::Result {
            data: json!({
                "file_path": logical_path,
                "bytes_written": bytes_written,
                "success": true,
                "status": status,
                "message": assistant_message,
            }),
            result_for_assistant: Some(assistant_message),
            image_attachments: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FileWriteTool;
    use crate::agentic::WorkspaceBinding;
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn local_context(root: PathBuf) -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: Some(WorkspaceBinding::new(None, root)),
            unlocked_collapsed_tools: Vec::new(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: None,
        }
    }

    #[tokio::test]
    async fn validate_input_rejects_existing_file_before_content_generation() {
        let root = std::env::temp_dir().join(format!("bitfun-write-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp workspace");
        let existing_file = root.join("existing.md");
        std::fs::write(&existing_file, "already here").expect("create existing file");

        let tool = FileWriteTool::new();
        let validation = tool
            .validate_input(
                &json!({ "file_path": "existing.md" }),
                Some(&local_context(root.clone())),
            )
            .await;

        let _ = std::fs::remove_dir_all(&root);

        assert!(!validation.result);
        let message = validation.message.unwrap_or_default();
        assert!(message.contains("already exists"));
        assert!(message.contains("Edit tool"));
    }

    #[tokio::test]
    async fn call_impl_treats_identical_existing_content_as_success() {
        let root = std::env::temp_dir().join(format!("bitfun-write-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp workspace");
        std::fs::write(root.join("existing.md"), "same content").expect("create existing file");

        let tool = FileWriteTool::new();
        let results = tool
            .call(
                &json!({ "file_path": "existing.md", "content": "same content" }),
                &local_context(root.clone()),
            )
            .await
            .expect("identical retry should be idempotent");

        let _ = std::fs::remove_dir_all(&root);

        let ToolResult::Result {
            data,
            result_for_assistant,
            ..
        } = &results[0]
        else {
            panic!("expected result");
        };
        assert_eq!(data["success"], true);
        assert_eq!(data["bytes_written"], 0);
        assert_eq!(data["status"], "already_exists_same_content");
        assert!(
            result_for_assistant
                .as_deref()
                .unwrap_or_default()
                .contains("do not call Write for this path again")
        );
    }

    #[tokio::test]
    async fn call_impl_rejects_different_existing_content() {
        let root = std::env::temp_dir().join(format!("bitfun-write-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp workspace");
        std::fs::write(root.join("existing.md"), "old content").expect("create existing file");

        let tool = FileWriteTool::new();
        let error = tool
            .call(
                &json!({ "file_path": "existing.md", "content": "new content" }),
                &local_context(root.clone()),
            )
            .await
            .expect_err("different content must not overwrite existing files");

        let _ = std::fs::remove_dir_all(&root);

        assert!(error.to_string().contains("already exists"));
        assert!(error.to_string().contains("Edit tool"));
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
- This tool is for creating NEW files only. Calling Write on a path that already exists will be REJECTED with an error.
- To MODIFY an existing file, use the Edit tool — it is the correct choice in almost every case.
- To FULLY REWRITE an existing file (e.g. regenerate a generated file, replace a template), first call the Delete tool on that path, then call Write to create the new version. Do not try to "overwrite" via Write directly.
- After Write succeeds for a path, do not call Write for that path again in later rounds. Use Edit for any additional changes.
- The file_path parameter must be workspace-relative, an absolute path inside the current workspace, or an exact `bitfun://runtime/...` URI returned by another tool.
- ALWAYS prefer editing existing files in the codebase. NEVER write new files unless explicitly required.
- NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the User.
- Only use emojis if the user explicitly requests it. Avoid writing emojis to files unless asked.
- Do NOT include the file content in the tool call arguments. Only provide file_path. The system will prompt you separately to output the file content as plain text."#.to_string())
    }

    fn short_description(&self) -> String {
        "Write a new file or fully replace an existing file.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The file to write. Use a workspace-relative path, an absolute path inside the current workspace, or an exact bitfun://runtime URI returned by another tool."
                }
            },
            "required": ["file_path"],
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

            // If content is absent, RoundExecutor would otherwise launch a
            // second model request to generate the full file. Reject existing
            // targets here so we do not spend tokens producing content that
            // Write must reject anyway. If a model already supplied content
            // despite the public schema, defer to call_impl so identical
            // retries can be treated as idempotent success.
            if input.get("content").is_none() {
                if let Some(error) = Self::existing_file_error(ctx, &resolved).await {
                    return ValidationResult {
                        result: false,
                        message: Some(error),
                        error_code: Some(400),
                        meta: None,
                    };
                }
            }
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
        context
            .record_light_checkpoint(
                "Write",
                &resolved.logical_path,
                vec![resolved.logical_path.clone()],
            )
            .await;

        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("content is required".to_string()))?;

        if let Some(error) = Self::existing_file_error(context, &resolved).await {
            if Self::existing_file_matches_content(context, &resolved, content).await == Some(true)
            {
                let result = Self::write_success_result(
                    &resolved.logical_path,
                    0,
                    "already_exists_same_content",
                    format!(
                        "Write skipped because {} already exists with identical content. Treat this file as successfully created and do not call Write for this path again. Use Edit for any further changes.",
                        resolved.logical_path
                    ),
                );
                return Ok(vec![result]);
            }

            return Err(BitFunError::tool(error));
        }

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

        let result = Self::write_success_result(
            &resolved.logical_path,
            content.len(),
            "created",
            format!(
                "Successfully created {} ({} bytes). The file now exists; do not call Write for this path again. Use Edit for any further changes.",
                resolved.logical_path,
                content.len()
            ),
        );

        Ok(vec![result])
    }
}
