use crate::agentic::tools::framework::{
    Tool, ToolPathResolution, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::ToolPathOperation;
use crate::service::config::types::WriteToolMode;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;
use tokio::fs;

pub struct FileWriteTool;

const LARGE_WRITE_SOFT_LINE_LIMIT: usize = 200;
const LARGE_WRITE_SOFT_BYTE_LIMIT: usize = 20 * 1024;
pub(crate) const WRITE_TOOL_MODE_CONTEXT_KEY: &str = "write_tool_mode";

impl Default for FileWriteTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FileWriteTool {
    pub fn new() -> Self {
        Self
    }

    pub(crate) fn write_tool_mode(context: Option<&ToolUseContext>) -> WriteToolMode {
        if Self::is_acp_context(context) {
            return WriteToolMode::InlineContent;
        }

        WriteToolMode::from_context_var(
            context
                .and_then(|ctx| ctx.custom_data.get(WRITE_TOOL_MODE_CONTEXT_KEY))
                .and_then(|value| value.as_str()),
        )
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

    fn is_acp_context(context: Option<&ToolUseContext>) -> bool {
        context
            .and_then(|ctx| ctx.custom_data.get("acp_transport"))
            .is_some_and(|value| value == "true" || value == &json!(true))
    }

    fn schema_with_content() -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The file to write. Use a workspace-relative path, an absolute path inside the current workspace, or an exact bitfun://runtime URI returned by another tool."
                },
                "content": {
                    "type": "string",
                    "description": "The complete file content to write."
                }
            },
            "required": ["file_path", "content"],
            "additionalProperties": false
        })
    }

    fn schema_without_content() -> Value {
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

    fn inline_description() -> String {
        r#"Writes a file to the local filesystem.

Usage:
- This tool will overwrite the existing file if there is one at the provided path.
- If this is an existing file, you MUST use the Read tool first to read the file's contents. This tool will fail if you did not read the file first.
- The file_path parameter must be workspace-relative, an absolute path inside the current workspace, or an exact `bitfun://runtime/...` URI returned by another tool.
- ALWAYS prefer editing existing files in the codebase. NEVER write new files unless explicitly required.
- Keep writes focused. The 200-line / 20KB guideline is a soft reliability threshold, not a hard cap. If a task genuinely needs more content, preserve correctness and use a staged plan instead of truncating.
- For existing files, prefer Read + targeted Edit calls. For large new files or rewrites, write the stable scaffold first, then fill or revise sections with focused Edit calls. Do not replace an entire existing file just to change a few sections.
- NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the User.
- Only use emojis if the user explicitly requests it. Avoid writing emojis to files unless asked.
- Include the complete file content in the `content` argument."#
            .to_string()
    }

    fn plaintext_followup_description() -> String {
        r#"Writes a file to the local filesystem.

Usage:
- This tool is for creating NEW files only. Calling Write on a path that already exists will be REJECTED with an error.
- To MODIFY an existing file, use the Edit tool — it is the correct choice in almost every case.
- To FULLY REWRITE an existing file (e.g. regenerate a generated file, replace a template), first call the Delete tool on that path, then call Write to create the new version. Do not try to "overwrite" via Write directly.
- After Write succeeds for a path, do not call Write for that path again in later rounds. Use Edit for any additional changes.
- The file_path parameter must be workspace-relative, an absolute path inside the current workspace, or an exact `bitfun://runtime/...` URI returned by another tool.
- ALWAYS prefer editing existing files in the codebase. NEVER write new files unless explicitly required.
- NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the User.
- Only use emojis if the user explicitly requests it. Avoid writing emojis to files unless asked.
- Do NOT include the file content in the tool call arguments. Only provide file_path. The system will prompt you separately to output the file content as plain text."#
            .to_string()
    }

    async fn call_inline_content_impl(
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

    async fn call_plaintext_followup_impl(
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

#[cfg(test)]
mod tests {
    use super::{FileWriteTool, WRITE_TOOL_MODE_CONTEXT_KEY};
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
            cancellation_token: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: None,
        }
    }

    fn local_context_with_custom_data(
        root: PathBuf,
        custom_data: HashMap<String, serde_json::Value>,
    ) -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: Some(WorkspaceBinding::new(None, root)),
            unlocked_collapsed_tools: Vec::new(),
            custom_data,
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: None,
        }
    }

    fn context_with_custom_data(custom_data: HashMap<String, serde_json::Value>) -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            unlocked_collapsed_tools: Vec::new(),
            custom_data,
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
        assert!(result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .contains("do not call Write for this path again"));
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

    #[tokio::test]
    async fn acp_schema_requires_inline_content() {
        let tool = FileWriteTool::new();
        let mut custom_data = HashMap::new();
        custom_data.insert(
            "acp_transport".to_string(),
            serde_json::Value::String("true".to_string()),
        );
        let context = context_with_custom_data(custom_data);

        let schema = tool
            .input_schema_for_model_with_context(Some(&context))
            .await;

        assert_eq!(
            schema["required"],
            serde_json::json!(["file_path", "content"])
        );
        assert!(schema["properties"].get("content").is_some());
    }

    #[tokio::test]
    async fn default_schema_keeps_two_stage_write_contract() {
        let tool = FileWriteTool::new();
        let context = context_with_custom_data(HashMap::new());

        let schema = tool
            .input_schema_for_model_with_context(Some(&context))
            .await;

        assert_eq!(schema["required"], serde_json::json!(["file_path"]));
        assert!(schema["properties"].get("content").is_none());
    }

    #[tokio::test]
    async fn inline_mode_schema_requires_content() {
        let tool = FileWriteTool::new();
        let mut custom_data = HashMap::new();
        custom_data.insert(
            WRITE_TOOL_MODE_CONTEXT_KEY.to_string(),
            serde_json::Value::String("inline_content".to_string()),
        );
        let context = context_with_custom_data(custom_data);

        let schema = tool
            .input_schema_for_model_with_context(Some(&context))
            .await;

        assert_eq!(
            schema["required"],
            serde_json::json!(["file_path", "content"])
        );
    }

    #[tokio::test]
    async fn inline_mode_requires_content_during_validation() {
        let tool = FileWriteTool::new();
        let mut custom_data = HashMap::new();
        custom_data.insert(
            WRITE_TOOL_MODE_CONTEXT_KEY.to_string(),
            serde_json::Value::String("inline_content".to_string()),
        );
        let context = context_with_custom_data(custom_data);

        let validation = tool
            .validate_input(&json!({ "file_path": "new.txt" }), Some(&context))
            .await;

        assert!(!validation.result);
        assert_eq!(validation.message.as_deref(), Some("content is required"));
    }

    #[tokio::test]
    async fn inline_mode_overwrites_existing_file() {
        let root = std::env::temp_dir().join(format!("bitfun-write-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp workspace");
        std::fs::write(root.join("existing.md"), "old content").expect("create existing file");

        let mut custom_data = HashMap::new();
        custom_data.insert(
            WRITE_TOOL_MODE_CONTEXT_KEY.to_string(),
            serde_json::Value::String("inline_content".to_string()),
        );

        let tool = FileWriteTool::new();
        tool.call(
            &json!({ "file_path": "existing.md", "content": "new content" }),
            &local_context_with_custom_data(root.clone(), custom_data),
        )
        .await
        .expect("inline mode should overwrite existing files");

        let written = std::fs::read_to_string(root.join("existing.md")).expect("read file");
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(written, "new content");
    }
}

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "Write"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(Self::plaintext_followup_description())
    }

    fn short_description(&self) -> String {
        "Write a new file.".to_string()
    }

    async fn description_with_context(
        &self,
        context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        match Self::write_tool_mode(context) {
            WriteToolMode::InlineContent => Ok(Self::inline_description()),
            WriteToolMode::PlaintextFollowup => Ok(Self::plaintext_followup_description()),
        }
    }

    fn input_schema(&self) -> Value {
        Self::schema_without_content()
    }

    async fn input_schema_for_model_with_context(&self, context: Option<&ToolUseContext>) -> Value {
        match Self::write_tool_mode(context) {
            WriteToolMode::InlineContent => Self::schema_with_content(),
            WriteToolMode::PlaintextFollowup => self.input_schema(),
        }
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

        let mode = Self::write_tool_mode(context);
        if matches!(mode, WriteToolMode::InlineContent) && input.get("content").is_none() {
            return ValidationResult {
                result: false,
                message: Some("content is required".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        let large_write_warning = if matches!(mode, WriteToolMode::InlineContent) {
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
                })
        } else {
            None
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

            if matches!(mode, WriteToolMode::PlaintextFollowup) {
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
        match Self::write_tool_mode(Some(context)) {
            WriteToolMode::InlineContent => self.call_inline_content_impl(input, context).await,
            WriteToolMode::PlaintextFollowup => {
                self.call_plaintext_followup_impl(input, context).await
            }
        }
    }
}
