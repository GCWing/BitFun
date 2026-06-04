use crate::agentic::execution::write_content_sanitizer::{
    contains_tool_invocation_artifacts, strip_tool_invocation_artifacts,
};
use crate::agentic::tools::file_read_state_runtime::{
    assert_file_not_unexpectedly_modified, file_mutation_timestamp_ms, get_stored_file_read_state,
    local_file_modification_time_ms, read_current_file_content, read_state_tracking_enabled,
    update_file_read_state_after_mutation, validate_existing_file_read_before_write,
    FILE_UNEXPECTEDLY_MODIFIED_ERROR,
};
use crate::agentic::tools::file_tool_guidance::{
    file_tool_guidance_message, is_file_tool_guidance_message,
};
use crate::agentic::tools::framework::{
    Tool, ToolPathResolution, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::ToolPathOperation;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;
use tokio::fs;
use tool_runtime::fs::{write_local_file, WriteLocalFileRequest};

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

    fn guidance_failure(message: String) -> ValidationResult {
        ValidationResult {
            result: false,
            message: Some(file_tool_guidance_message(message)),
            error_code: Some(400),
            meta: Some(json!({ "failure_kind": "guidance" })),
        }
    }

    fn format_write_freshness_guidance(logical_path: &str, error: String) -> String {
        if error == FILE_UNEXPECTEDLY_MODIFIED_ERROR || error.contains("unexpectedly modified") {
            format!(
                "The file {} changed since it was last read. Use Read again, then retry Write.",
                logical_path
            )
        } else if error.contains("modified since read") {
            format!(
                "The file {} changed after it was last read. Use Read again, then retry Write.",
                logical_path
            )
        } else {
            error
        }
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

    async fn existing_file_write_freshness_error(
        context: &ToolUseContext,
        resolved: &ToolPathResolution,
    ) -> Option<String> {
        if !Self::file_exists(context, resolved).await {
            return None;
        }
        if !read_state_tracking_enabled(context) {
            return None;
        }

        let current_content = match read_current_file_content(context, resolved).await {
            Ok(content) => content,
            Err(error) => return Some(error.to_string()),
        };
        let read_state = get_stored_file_read_state(context, resolved);
        let current_mtime_ms = if resolved.uses_remote_workspace_backend() {
            None
        } else {
            Some(local_file_modification_time_ms(Path::new(
                &resolved.resolved_path,
            )))
        };

        assert_file_not_unexpectedly_modified(
            read_state.as_ref(),
            &current_content,
            current_mtime_ms,
        )
        .err()
        .map(|error| Self::format_write_freshness_guidance(&resolved.logical_path, error))
    }

    async fn assert_atomic_write_freshness_if_exists(
        context: &ToolUseContext,
        resolved: &ToolPathResolution,
    ) -> BitFunResult<()> {
        if let Some(error) = Self::existing_file_write_freshness_error(context, resolved).await {
            return Err(BitFunError::tool(file_tool_guidance_message(error)));
        }

        Ok(())
    }

    async fn write_guardrail_preflight_error(
        context: &ToolUseContext,
        resolved: &ToolPathResolution,
    ) -> Option<String> {
        if !Self::file_exists(context, resolved).await {
            return None;
        }

        if let Some(message) = validate_existing_file_read_before_write(context, resolved).await {
            return Some(file_tool_guidance_message(message));
        }

        Self::existing_file_write_freshness_error(context, resolved)
            .await
            .map(file_tool_guidance_message)
    }

    pub(crate) async fn preflight_write_error(
        context: &ToolUseContext,
        file_path: &str,
    ) -> Option<String> {
        let resolved = match context.resolve_tool_path(file_path) {
            Ok(resolved) => resolved,
            Err(err) => return Some(err.to_string()),
        };

        if let Err(err) = context.enforce_path_operation(ToolPathOperation::Write, &resolved) {
            return Some(err.to_string());
        }

        Self::write_guardrail_preflight_error(context, &resolved).await
    }

    fn write_success_result(
        logical_path: &str,
        bytes_written: usize,
        lines_written: usize,
        status: &str,
        assistant_message: String,
    ) -> ToolResult {
        ToolResult::Result {
            data: json!({
                "file_path": logical_path,
                "bytes_written": bytes_written,
                "lines_written": lines_written,
                "success": true,
                "status": status,
                "message": assistant_message,
            }),
            result_for_assistant: Some(assistant_message),
            image_attachments: None,
        }
    }

    fn input_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to write (must be absolute, not relative)"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["file_path", "content"],
            "additionalProperties": false
        })
    }

    fn description() -> String {
        r#"Writes a file to the local filesystem.

Usage:
- This tool will overwrite the existing file if there is one at the provided path.
- Always emit `file_path` before `content` in the tool input JSON so the path is available while content streams.
- If this is an existing file, you MUST use the Read tool first to read the file's contents. This tool will fail if you did not read the file first.
- Prefer the Edit tool for modifying existing files — it only sends the diff. Only use this tool to create new files or for complete rewrites.
- NEVER create documentation files (*.md) or README files unless explicitly requested by the User.
- Only use emojis if the user explicitly requests it. Avoid writing emojis to files unless asked."#
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::FileWriteTool;
    use crate::agentic::tools::file_tool_guidance::{
        file_tool_guidance_message, is_file_tool_guidance_message, FILE_TOOL_GUIDANCE_PREFIX,
    };
    use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::agentic::WorkspaceBinding;
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
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    #[test]
    fn guidance_prefix_helpers_round_trip() {
        let message = file_tool_guidance_message("Use Read first.");
        assert!(is_file_tool_guidance_message(&message));
        assert_eq!(
            message.strip_prefix(FILE_TOOL_GUIDANCE_PREFIX).unwrap(),
            "Use Read first."
        );
    }

    #[tokio::test]
    async fn preflight_write_error_allows_new_file_target() {
        let root = std::env::temp_dir().join(format!("bitfun-write-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp workspace");

        let error =
            FileWriteTool::preflight_write_error(&local_context(root.clone()), "new.txt").await;

        let _ = std::fs::remove_dir_all(&root);

        assert!(error.is_none());
    }

    #[tokio::test]
    async fn preflight_write_error_allows_existing_file_without_read_state_tracking() {
        let root = std::env::temp_dir().join(format!("bitfun-write-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp workspace");
        std::fs::write(root.join("existing.md"), "already here").expect("create existing file");

        let error =
            FileWriteTool::preflight_write_error(&local_context(root.clone()), "existing.md").await;

        let _ = std::fs::remove_dir_all(&root);

        assert!(error.is_none());
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
        assert_eq!(data["lines_written"], 0);
        assert_eq!(data["status"], "already_exists_same_content");
        assert!(result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .contains("identical content"));
    }

    #[tokio::test]
    async fn call_impl_overwrites_different_existing_content() {
        let root = std::env::temp_dir().join(format!("bitfun-write-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp workspace");
        std::fs::write(root.join("existing.md"), "old content").expect("create existing file");

        let tool = FileWriteTool::new();
        let results = tool
            .call(
                &json!({ "file_path": "existing.md", "content": "new content" }),
                &local_context(root.clone()),
            )
            .await
            .expect("write should overwrite existing files");

        let written = std::fs::read_to_string(root.join("existing.md")).expect("read file");
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(written, "new content");

        let ToolResult::Result { data, .. } = &results[0] else {
            panic!("expected result");
        };
        assert_eq!(data["status"], "overwritten");
        assert_eq!(data["bytes_written"], "new content".len());
        assert_eq!(data["lines_written"], 1);
    }

    #[tokio::test]
    async fn schema_requires_file_path_and_content() {
        let tool = FileWriteTool::new();

        let schema = tool.input_schema_for_model().await;

        assert_eq!(
            schema["required"],
            serde_json::json!(["file_path", "content"])
        );
        assert!(schema["properties"].get("content").is_some());
    }

    #[tokio::test]
    async fn validate_input_rejects_tool_invocation_content() {
        let tool = FileWriteTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "file_path": "notes.md",
                    "content": "<tool_calls><invoke name=\"Write\"></invoke></tool_calls>"
                }),
                None,
            )
            .await;

        assert!(!validation.result);
        assert!(validation
            .message
            .as_deref()
            .is_some_and(|message| message.contains("tool-invocation syntax")));
    }

    #[tokio::test]
    async fn validate_input_requires_content() {
        let tool = FileWriteTool::new();

        let validation = tool
            .validate_input(&json!({ "file_path": "new.txt" }), None)
            .await;

        assert!(!validation.result);
        assert_eq!(validation.message.as_deref(), Some("content is required"));
    }
}

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "Write"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(FileWriteTool::description())
    }

    fn short_description(&self) -> String {
        "Write or overwrite a file.".to_string()
    }

    async fn description_with_context(
        &self,
        _context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        Ok(FileWriteTool::description())
    }

    fn input_schema(&self) -> Value {
        FileWriteTool::input_schema()
    }

    async fn input_schema_for_model(&self) -> Value {
        FileWriteTool::input_schema()
    }

    async fn input_schema_for_model_with_context(
        &self,
        _context: Option<&ToolUseContext>,
    ) -> Value {
        FileWriteTool::input_schema()
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

        if let Some(content) = input.get("content").and_then(|v| v.as_str()) {
            if contains_tool_invocation_artifacts(content) {
                return Self::guidance_failure(
                    "Write content looks like tool-invocation syntax instead of raw file content. \
                     Output the file body directly in the `content` field without nested tool calls."
                        .to_string(),
                );
            }
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
            if let Some(message) = Self::preflight_write_error(ctx, file_path).await {
                let is_guidance = is_file_tool_guidance_message(&message);
                return ValidationResult {
                    result: false,
                    message: Some(message),
                    error_code: Some(400),
                    meta: is_guidance.then(|| json!({ "failure_kind": "guidance" })),
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
        let content = strip_tool_invocation_artifacts(content);
        if content.is_empty() {
            return Err(BitFunError::tool(file_tool_guidance_message(
                "Write content is empty after removing tool-invocation syntax. \
                 Provide the raw file body in the `content` field.",
            )));
        }
        if contains_tool_invocation_artifacts(&content) {
            return Err(BitFunError::tool(file_tool_guidance_message(
                "Write content still contains tool-invocation syntax after sanitization. \
                 Provide raw file content only.",
            )));
        }

        let file_already_exists = Self::file_exists(context, &resolved).await;
        if file_already_exists
            && Self::existing_file_matches_content(context, &resolved, &content).await == Some(true)
        {
            let result = Self::write_success_result(
                &resolved.logical_path,
                0,
                0,
                "already_exists_same_content",
                format!(
                    "Write skipped because {} already exists with identical content.",
                    resolved.logical_path
                ),
            );
            return Ok(vec![result]);
        }

        Self::assert_atomic_write_freshness_if_exists(context, &resolved).await?;

        if resolved.uses_remote_workspace_backend() {
            let ws_fs = context.ws_fs().ok_or_else(|| {
                BitFunError::tool("Remote workspace file system is unavailable".to_string())
            })?;
            ws_fs
                .write_file(&resolved.resolved_path, content.as_bytes())
                .await
                .map_err(|e| BitFunError::tool(format!("Failed to write file: {}", e)))?;
            let timestamp_ms = file_mutation_timestamp_ms(context, &resolved).await;
            update_file_read_state_after_mutation(context, &resolved, &content, timestamp_ms);

            let (status, assistant_message) = if file_already_exists {
                (
                    "overwritten",
                    format!(
                        "Successfully overwrote {} ({} bytes).",
                        resolved.logical_path,
                        content.len()
                    ),
                )
            } else {
                (
                    "created",
                    format!(
                        "Successfully created {} ({} bytes).",
                        resolved.logical_path,
                        content.len()
                    ),
                )
            };

            let result = Self::write_success_result(
                &resolved.logical_path,
                content.len(),
                if content.is_empty() {
                    0
                } else {
                    content.lines().count().max(1)
                },
                status,
                assistant_message,
            );
            return Ok(vec![result]);
        }

        let write_request = WriteLocalFileRequest {
            logical_path: resolved.logical_path.clone(),
            resolved_path: Path::new(&resolved.resolved_path).to_path_buf(),
            content: content.clone(),
        };
        let outcome = tokio::task::spawn_blocking(move || write_local_file(write_request))
            .await
            .map_err(|error| BitFunError::tool(format!("Write task failed: {}", error)))?
            .map_err(BitFunError::tool)?;

        let timestamp_ms = file_mutation_timestamp_ms(context, &resolved).await;
        update_file_read_state_after_mutation(context, &resolved, &content, timestamp_ms);

        let result = Self::write_success_result(
            &resolved.logical_path,
            outcome.bytes_written,
            outcome.lines_written,
            outcome.status.as_str(),
            outcome.assistant_message,
        );

        Ok(vec![result])
    }
}
