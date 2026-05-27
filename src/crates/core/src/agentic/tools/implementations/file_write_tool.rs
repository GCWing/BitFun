use crate::agentic::core::ToolCall;
use crate::agentic::tools::file_read_state_runtime::{
    assert_file_not_unexpectedly_modified, file_mutation_timestamp_ms, get_stored_file_read_state,
    local_file_modification_time_ms, read_current_file_content, read_state_tracking_enabled,
    update_file_read_state_after_mutation, validate_existing_file_read_before_write,
    FILE_UNEXPECTEDLY_MODIFIED_ERROR,
};
use crate::agentic::tools::file_tool_guidance::{
    file_tool_guidance_message, is_file_tool_guidance_message,
};
use crate::agentic::execution::write_content_sanitizer::{
    contains_tool_invocation_artifacts, strip_tool_invocation_artifacts,
};
use crate::agentic::tools::framework::{
    Tool, ToolPathResolution, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::ToolPathOperation;
use crate::service::config::types::WriteToolMode;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_ai_adapters::tool_call_accumulator::strip_write_inline_content_fields;
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

    pub(crate) fn write_guidance_message(message: impl Into<String>) -> String {
        file_tool_guidance_message(message)
    }

    pub(crate) fn is_write_guidance_message(message: &str) -> bool {
        is_file_tool_guidance_message(message)
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
            return Err(BitFunError::tool(Self::write_guidance_message(error)));
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
            return Some(Self::write_guidance_message(message));
        }

        Self::existing_file_write_freshness_error(context, resolved)
            .await
            .map(Self::write_guidance_message)
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
        status: &str,
        assistant_message: String,
        content: &str,
    ) -> ToolResult {
        let result_for_assistant = format!(
            "{assistant_message}\n\nWritten content:\n<bitfun_contents>\n{content}</bitfun_contents>"
        );

        ToolResult::Result {
            data: json!({
                "file_path": logical_path,
                "bytes_written": bytes_written,
                "success": true,
                "status": status,
                "message": assistant_message,
                "content": content,
            }),
            result_for_assistant: Some(result_for_assistant),
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

    fn model_input_schema(context: Option<&ToolUseContext>) -> Value {
        match Self::write_tool_mode(context) {
            WriteToolMode::InlineContent => Self::schema_with_content(),
            WriteToolMode::PlaintextFollowup => Self::schema_without_content(),
        }
    }

    /// PlaintextFollowup exposes only `file_path` to the model. Strip any inline
    /// content the model hallucinates so the follow-up content generation path
    /// remains the single source of file body text.
    pub(crate) fn strip_plaintext_followup_inline_content(arguments: &mut Value) {
        strip_write_inline_content_fields(arguments);
    }

    pub(crate) fn strip_plaintext_followup_inline_content_from_tool_calls(
        tool_calls: &mut [ToolCall],
    ) {
        for tool_call in tool_calls.iter_mut() {
            if tool_call.tool_name == "Write" {
                Self::strip_plaintext_followup_inline_content(&mut tool_call.arguments);
            }
        }
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
- This tool writes the COMPLETE file content and will overwrite the existing file if one already exists at the provided path.
- For partial changes to an existing file, use the Edit tool instead. Edit performs targeted string replacements; Write replaces the entire file.
- If this is an existing file, you MUST use the Read tool first to read the file's contents before calling Write.
- The file_path parameter must be workspace-relative, an absolute path inside the current workspace, or an exact `bitfun://runtime/...` URI returned by another tool.
- ALWAYS prefer editing existing files in the codebase. NEVER write new files unless explicitly required.
- Keep writes focused. For existing files, prefer Read + targeted Edit calls. Use Write only when you need to replace the entire file or create a new one.
- NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the User.
- Only use emojis if the user explicitly requests it. Avoid writing emojis to files unless asked.
- IMPORTANT — two-step protocol: This tool's schema accepts ONLY `file_path`. Do NOT include a `content` field; any inline content will be discarded. After your tool call, the system automatically issues a separate follow-up request that asks you to output the complete file body inside `<bitfun_contents>` tags, and that text is then written to disk.
- If the system returns an error saying the content-generation step produced no content, retry the same Write tool call with the same `file_path`. Still do not provide `content` inline."#
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
        let content = strip_tool_invocation_artifacts(content);
        if content.is_empty() {
            return Err(BitFunError::tool(Self::write_guidance_message(
                "Write content is empty after removing tool-invocation syntax. \
                 Provide the raw file body in the `content` field.",
            )));
        }
        if contains_tool_invocation_artifacts(&content) {
            return Err(BitFunError::tool(Self::write_guidance_message(
                "Write content still contains tool-invocation syntax after sanitization. \
                 Provide raw file content only.",
            )));
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
        } else {
            if let Some(parent) = Path::new(&resolved.resolved_path).parent() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|e| BitFunError::tool(format!("Failed to create directory: {}", e)))?;
            }
            fs::write(&resolved.resolved_path, &content)
                .await
                .map_err(|e| {
                    BitFunError::tool(format!(
                        "Failed to write file {}: {}",
                        resolved.logical_path, e
                    ))
                })?;
        }

        let timestamp_ms = file_mutation_timestamp_ms(context, &resolved).await;
        update_file_read_state_after_mutation(context, &resolved, &content, timestamp_ms);

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
        // PlaintextFollowup injects `content` in `generate_write_tool_contents` before
        // pipeline execution. Inline model content is stripped earlier (stream +
        // round_executor); do not strip again here or system-generated content is lost.
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
            .ok_or_else(|| {
                BitFunError::tool(Self::write_guidance_message(
                    "The system's Write content-generation follow-up step did not produce \
                     any file content. The Write tool uses a two-step protocol: the model \
                     supplies only `file_path`, and the system generates the file body in a \
                     separate follow-up request. Please retry the Write tool call with the \
                     same `file_path`. Do NOT include a `content` argument — the schema \
                     does not accept it; the system will generate the content again.",
                ))
            })?;

        let file_already_exists = Self::file_exists(context, &resolved).await;
        if file_already_exists
            && Self::existing_file_matches_content(context, &resolved, content).await == Some(true)
        {
            let result = Self::write_success_result(
                &resolved.logical_path,
                0,
                "already_exists_same_content",
                format!(
                    "Write skipped because {} already exists with identical content.",
                    resolved.logical_path
                ),
                content,
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

        let timestamp_ms = file_mutation_timestamp_ms(context, &resolved).await;
        update_file_read_state_after_mutation(context, &resolved, content, timestamp_ms);

        let result = Self::write_success_result(
            &resolved.logical_path,
            content.len(),
            status,
            assistant_message,
            content,
        );

        Ok(vec![result])
    }
}

#[cfg(test)]
mod tests {
    use super::{FileWriteTool, WRITE_TOOL_MODE_CONTEXT_KEY};
    use crate::agentic::core::ToolCall;
    use crate::agentic::tools::file_tool_guidance::FILE_TOOL_GUIDANCE_PREFIX;
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

    #[test]
    fn write_guidance_prefix_helpers_round_trip() {
        let message = FileWriteTool::write_guidance_message("Use Read first.");
        assert!(FileWriteTool::is_write_guidance_message(&message));
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
        assert_eq!(data["status"], "already_exists_same_content");
        assert_eq!(data["content"], "same content");
        assert!(result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .contains("identical content"));
        assert!(result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .contains("<bitfun_contents>\nsame content</bitfun_contents>"));
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
            .expect("plaintext followup should overwrite existing files");

        let written = std::fs::read_to_string(root.join("existing.md")).expect("read file");
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(written, "new content");

        let ToolResult::Result { data, .. } = &results[0] else {
            panic!("expected result");
        };
        assert_eq!(data["status"], "overwritten");
        assert_eq!(data["content"], "new content");
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
    async fn input_schema_for_model_without_context_omits_content() {
        let tool = FileWriteTool::new();

        let schema = tool.input_schema_for_model().await;

        assert_eq!(schema["required"], serde_json::json!(["file_path"]));
        assert!(schema["properties"].get("content").is_none());
    }

    #[test]
    fn strip_plaintext_followup_inline_content_removes_inline_body_fields() {
        let mut arguments = json!({
            "file_path": "notes.md",
            "content": "inline body",
            "contents": "legacy body"
        });

        FileWriteTool::strip_plaintext_followup_inline_content(&mut arguments);

        assert_eq!(arguments, json!({ "file_path": "notes.md" }));
    }

    #[test]
    fn strip_plaintext_followup_inline_content_from_tool_calls_keeps_non_write_calls() {
        let mut tool_calls = vec![
            ToolCall {
                tool_id: "write-1".to_string(),
                tool_name: "Write".to_string(),
                arguments: json!({
                    "file_path": "notes.md",
                    "content": "inline body"
                }),
                raw_arguments: None,
                is_error: false,
                recovered_from_truncation: false,
            },
            ToolCall {
                tool_id: "read-1".to_string(),
                tool_name: "Read".to_string(),
                arguments: json!({ "file_path": "notes.md" }),
                raw_arguments: None,
                is_error: false,
                recovered_from_truncation: false,
            },
        ];

        FileWriteTool::strip_plaintext_followup_inline_content_from_tool_calls(&mut tool_calls);

        assert_eq!(tool_calls[0].arguments, json!({ "file_path": "notes.md" }));
        assert_eq!(tool_calls[1].arguments, json!({ "file_path": "notes.md" }));
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
    async fn inline_mode_rejects_tool_invocation_content() {
        let tool = FileWriteTool::new();
        let mut custom_data = HashMap::new();
        custom_data.insert(
            WRITE_TOOL_MODE_CONTEXT_KEY.to_string(),
            serde_json::Value::String("inline_content".to_string()),
        );
        let context = context_with_custom_data(custom_data);

        let validation = tool
            .validate_input(
                &json!({
                    "file_path": "notes.md",
                    "content": "<tool_calls><invoke name=\"Write\"></invoke></tool_calls>"
                }),
                Some(&context),
            )
            .await;

        assert!(!validation.result);
        assert!(validation
            .message
            .as_deref()
            .is_some_and(|message| message.contains("tool-invocation syntax")));
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

    #[tokio::test]
    async fn plaintext_followup_missing_content_returns_two_step_guidance() {
        let root = std::env::temp_dir().join(format!("bitfun-write-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp workspace");

        let tool = FileWriteTool::new();
        let error = tool
            .call(
                &json!({ "file_path": "generated.txt" }),
                &local_context(root.clone()),
            )
            .await
            .expect_err("missing content in plaintext-followup mode must error");

        let _ = std::fs::remove_dir_all(&root);

        let message = error.to_string();
        assert!(
            !message.contains("content is required"),
            "should not surface the contradictory 'content is required' message: {message}"
        );
        assert!(
            message.contains("two-step protocol")
                || message.contains("follow-up")
                || message.contains("did not produce"),
            "should explain the two-step Write protocol: {message}"
        );
        assert!(
            message.contains(FILE_TOOL_GUIDANCE_PREFIX),
            "should be wrapped in file-tool guidance prefix: {message}"
        );
    }

    #[tokio::test]
    async fn plaintext_followup_executes_system_injected_content() {
        let root = std::env::temp_dir().join(format!("bitfun-write-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp workspace");

        let tool = FileWriteTool::new();
        let body = "system generated body";
        let results = tool
            .call(
                &json!({ "file_path": "generated.txt", "content": body }),
                &local_context(root.clone()),
            )
            .await
            .expect("plaintext followup should write system-injected content");

        let written =
            std::fs::read_to_string(root.join("generated.txt")).expect("read generated file");
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(written, body);
        let ToolResult::Result {
            data,
            result_for_assistant,
            ..
        } = &results[0]
        else {
            panic!("expected result");
        };
        assert_eq!(data["content"], body);
        assert!(result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .contains("<bitfun_contents>\nsystem generated body</bitfun_contents>"));
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
        "Write or overwrite a file.".to_string()
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

    async fn input_schema_for_model(&self) -> Value {
        Self::model_input_schema(None)
    }

    async fn input_schema_for_model_with_context(&self, context: Option<&ToolUseContext>) -> Value {
        Self::model_input_schema(context)
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

        if matches!(mode, WriteToolMode::InlineContent) {
            if let Some(content) = input.get("content").and_then(|v| v.as_str()) {
                if contains_tool_invocation_artifacts(content) {
                    return ValidationResult {
                        result: false,
                        message: Some(Self::write_guidance_message(
                            "Write content looks like tool-invocation syntax instead of raw file content. \
                             Output the file body directly in the `content` field without nested tool calls.",
                        )),
                        error_code: Some(400),
                        meta: Some(json!({ "failure_kind": "guidance" })),
                    };
                }
            }
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
            if let Some(message) = Self::preflight_write_error(ctx, file_path).await {
                let is_guidance = Self::is_write_guidance_message(&message);
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
        match Self::write_tool_mode(Some(context)) {
            WriteToolMode::InlineContent => self.call_inline_content_impl(input, context).await,
            WriteToolMode::PlaintextFollowup => {
                self.call_plaintext_followup_impl(input, context).await
            }
        }
    }
}
