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
use tool_runtime::fs::{
    write_file_success_outcome, write_local_file, write_same_content_outcome,
    WriteLocalFileOutcome, WriteLocalFileRequest,
};

pub struct FileWriteTool;

#[derive(Debug, PartialEq, Eq)]
enum ParsedWritePayload<'a> {
    Target {
        file_path: &'a str,
        content: &'a str,
    },
    MissingPath {
        content: &'a str,
    },
}

impl<'a> ParsedWritePayload<'a> {
    fn content(&self) -> &'a str {
        match self {
            Self::Target { content, .. } | Self::MissingPath { content } => content,
        }
    }
}

const WRITE_PAYLOAD_PATH_PREFIX: &str = "+++ ";
const WRITE_FALLBACK_FILE_ATTEMPTS: usize = 16;
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

    fn parse_payload(input: &Value) -> Result<ParsedWritePayload<'_>, String> {
        let value = input
            .get("payload")
            .and_then(Value::as_str)
            .ok_or_else(|| "payload is required".to_string())?;
        let (first_line, content) = value.split_once('\n').unwrap_or((value, ""));
        let first_line = first_line.strip_suffix('\r').unwrap_or(first_line);
        let Some(file_path) = first_line.strip_prefix(WRITE_PAYLOAD_PATH_PREFIX) else {
            return Ok(ParsedWritePayload::MissingPath { content: value });
        };
        if file_path.trim().is_empty() {
            return Ok(ParsedWritePayload::MissingPath { content: value });
        }

        Ok(ParsedWritePayload::Target { file_path, content })
    }

    async fn unused_fallback_file_path(context: &ToolUseContext) -> BitFunResult<String> {
        for _ in 0..WRITE_FALLBACK_FILE_ATTEMPTS {
            let random_id = uuid::Uuid::new_v4().simple().to_string();
            let file_path = format!("write_{}.tmp", &random_id[..6]);
            let resolved = context.resolve_tool_path(&file_path)?;
            context.enforce_path_operation(ToolPathOperation::Write, &resolved)?;
            if !Self::file_exists(context, &resolved).await {
                return Ok(file_path);
            }
        }

        Err(BitFunError::tool(
            "Failed to allocate a unique fallback file for Write".to_string(),
        ))
    }

    fn ignored_top_level_parameter_names(input: &Value) -> Vec<String> {
        let mut parameter_names = input
            .as_object()
            .into_iter()
            .flat_map(|object| object.keys())
            .filter(|name| name.as_str() != "payload")
            .cloned()
            .collect::<Vec<_>>();
        parameter_names.sort();
        parameter_names
    }

    fn write_success_result(
        logical_path: &str,
        outcome: WriteLocalFileOutcome,
        missing_path_fallback: bool,
        ignored_parameter_names: &[String],
    ) -> ToolResult {
        let mut assistant_message = if missing_path_fallback {
            format!(
                "The Write payload did not start with the required '+++ {{file_path}}' marker. The entire payload was saved to {}. Rename this temporary file to the intended path before continuing; this way, you do not need to rewrite the entire content.",
                logical_path
            )
        } else {
            outcome.assistant_message
        };
        if !ignored_parameter_names.is_empty() {
            let formatted_names = ignored_parameter_names
                .iter()
                .map(|name| format!("`{}`", name))
                .collect::<Vec<_>>()
                .join(", ");
            assistant_message.push_str(&format!(
                " The Write tool accepts only the `payload` parameter; these additional parameters were ignored and should not be passed again: {}.",
                formatted_names
            ));
        }
        ToolResult::Result {
            data: json!({
                "file_path": logical_path,
                "bytes_written": outcome.bytes_written,
                "lines_written": outcome.lines_written,
                "success": true,
                "status": outcome.status.as_str(),
                "missing_path_fallback": missing_path_fallback,
                "rename_required": missing_path_fallback,
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
                "payload": {
                    "type": "string",
                    "description": "A path-first Write payload in the format `+++ {absolute_file_path_or_bitfun_uri}\n{file_content}`. Content lines do not need a leading `+`."
                }
            },
            "required": ["payload"],
            "additionalProperties": false
        })
    }

    fn description() -> String {
        r#"Create or overwrite a file.

Parameter: `payload` (a single string)
- Format: `+++ {file_path}\n{file_content}`
- This is a path-first Write payload format: the first line uses Git's `+++` marker to specify the target file, but content lines do NOT need a leading `+`. Do not include `---`, `@@`, or other Git diff headers.
- `{file_path}` must be an absolute path or an exact `bitfun://...` URI. Everything after the first newline is the complete content to write to that file.
- The `+++ ` marker is required. If it is missing or has no file path, the tool saves the entire `payload` unchanged to `write_{random id}.tmp` in the workspace root.
- Do NOT pass `path`, `file_path`, or `content`, etc. They are not valid parameters for this tool. Only `payload` is accepted.

Usage:
- This tool creates the file if it does not exist; otherwise, it overwrites the existing file.
- If this is an existing file, you MUST use the Read tool first to read the file's contents. This tool will fail if you did not read the file first.
- Prefer the Edit tool for modifying existing files — it only sends the diff. Only use this tool to create new files or for complete rewrites.
- NEVER create documentation files (*.md) or README files unless explicitly requested by the User.
- Only use emojis if the user explicitly requests it. Avoid writing emojis to files unless asked.

Examples:
<good-example>
`{"payload":"+++ /path/to/main.py\ndef main():\n\tprint(\"Hello world\")\n\nmain()"}`

This call creates or overwrites `/path/to/main.py` with the following content:
```
def main():
	print("Hello world")

main()
```
</good-example>

<bad-example>
`{"file_path":"/path/to/main.py","content":"print(\"Hello world\")"}`

This call is invalid because Write requires the single `payload` parameter. Do not pass `file_path` and `content` separately.
</bad-example>

<bad-example>
`{"payload":"+++ /path/to/main.py\nprint(\"Hello world\")","file_path":"/path/to/main.py"}`

This call includes an unnecessary `file_path` parameter. Write only uses `payload`; specify the target path in the first `+++ {file_path}` line and do not pass additional parameters.
</bad-example>
"#
            .to_string()
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
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let parsed = match Self::parse_payload(input) {
            Ok(parsed) => parsed,
            Err(message) => {
                return ValidationResult {
                    result: false,
                    message: Some(message),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        let content = parsed.content();
        let line_count = content.lines().count();
        let byte_count = content.len();
        let large_write_warning = if line_count > LARGE_WRITE_SOFT_LINE_LIMIT
            || byte_count > LARGE_WRITE_SOFT_BYTE_LIMIT
        {
            Some((line_count, byte_count))
        } else {
            None
        };

        if let Some(ctx) = context {
            let preflight_error = match &parsed {
                ParsedWritePayload::Target { file_path, .. } => {
                    Self::preflight_write_error(ctx, file_path).await
                }
                ParsedWritePayload::MissingPath { .. } => {
                    let fallback_path = "write_000000.tmp";
                    match ctx.resolve_tool_path(fallback_path) {
                        Ok(resolved) => ctx
                            .enforce_path_operation(ToolPathOperation::Write, &resolved)
                            .err()
                            .map(|error| error.to_string()),
                        Err(error) => Some(error.to_string()),
                    }
                }
            };
            if let Some(message) = preflight_error {
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
        match Self::parse_payload(input) {
            Ok(ParsedWritePayload::Target { file_path, content }) => {
                if options.verbose {
                    format!("Writing {} characters to {}", content.len(), file_path)
                } else {
                    format!("Write {}", file_path)
                }
            }
            Ok(ParsedWritePayload::MissingPath { content }) => {
                if options.verbose {
                    format!(
                        "Writing {} characters to a workspace temporary file",
                        content.len()
                    )
                } else {
                    "Write workspace temporary file".to_string()
                }
            }
            Err(_) => "Writing file".to_string(),
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let ignored_parameter_names = Self::ignored_top_level_parameter_names(input);
        let parsed = Self::parse_payload(input).map_err(BitFunError::tool)?;
        let (file_path, content, missing_path_fallback) = match parsed {
            ParsedWritePayload::Target { file_path, content } => {
                (file_path.to_string(), content.to_string(), false)
            }
            ParsedWritePayload::MissingPath { content } => (
                Self::unused_fallback_file_path(context).await?,
                content.to_string(),
                true,
            ),
        };

        let resolved = context.resolve_tool_path(&file_path)?;
        context.enforce_path_operation(ToolPathOperation::Write, &resolved)?;
        context
            .record_light_checkpoint(
                "Write",
                &resolved.logical_path,
                vec![resolved.logical_path.clone()],
            )
            .await;

        let file_already_exists = Self::file_exists(context, &resolved).await;
        if file_already_exists
            && Self::existing_file_matches_content(context, &resolved, &content).await == Some(true)
        {
            let result = Self::write_success_result(
                &resolved.logical_path,
                write_same_content_outcome(&resolved.logical_path),
                missing_path_fallback,
                &ignored_parameter_names,
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

            let result = Self::write_success_result(
                &resolved.logical_path,
                write_file_success_outcome(&resolved.logical_path, file_already_exists, &content),
                missing_path_fallback,
                &ignored_parameter_names,
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
            outcome,
            missing_path_fallback,
            &ignored_parameter_names,
        );

        Ok(vec![result])
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
    use std::path::{Path, PathBuf};

    fn local_context(root: PathBuf) -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: Some(WorkspaceBinding::new(None, root)),
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
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
                &json!({ "payload": "+++ existing.md\nsame content" }),
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
                &json!({ "payload": "+++ existing.md\nnew content" }),
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
    async fn call_impl_appends_warning_for_ignored_top_level_parameters() {
        let root = std::env::temp_dir().join(format!("bitfun-write-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp workspace");

        let tool = FileWriteTool::new();
        let input = json!({
            "payload": "+++ new.txt\nalpha",
            "content": "ignored content",
            "file_path": "ignored.txt"
        });
        let validation = tool.validate_input(&input, None).await;
        assert!(validation.result);

        let results = tool
            .call(&input, &local_context(root.clone()))
            .await
            .expect("extra parameters should be ignored");

        let _ = std::fs::remove_dir_all(&root);

        let ToolResult::Result {
            data,
            result_for_assistant,
            ..
        } = &results[0]
        else {
            panic!("expected result");
        };
        let result_path = data["file_path"].as_str().expect("result file path");
        let expected = format!(
            "Successfully created {} (1 lines, 5 bytes). The Write tool accepts only the `payload` parameter; these additional parameters were ignored and should not be passed again: `content`, `file_path`.",
            result_path
        );
        assert_eq!(result_for_assistant.as_deref(), Some(expected.as_str()));
        assert_eq!(data["message"], expected);
    }

    #[tokio::test]
    async fn call_impl_accepts_path_only_for_empty_file() {
        let root = std::env::temp_dir().join(format!("bitfun-write-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp workspace");

        let tool = FileWriteTool::new();
        let results = tool
            .call(
                &json!({ "payload": "+++ empty.txt" }),
                &local_context(root.clone()),
            )
            .await
            .expect("path-only input should create an empty file");

        let written = std::fs::read(root.join("empty.txt")).expect("read empty file");
        let _ = std::fs::remove_dir_all(&root);

        assert!(written.is_empty());
        let ToolResult::Result { data, .. } = &results[0] else {
            panic!("expected result");
        };
        assert_eq!(data["bytes_written"], 0);
        assert_eq!(data["lines_written"], 0);
    }

    #[test]
    fn description_includes_bad_examples_for_invalid_parameter_shapes() {
        let description = FileWriteTool::description();

        assert_eq!(description.matches("<bad-example>").count(), 2);
        assert!(description
            .contains(r#"{"file_path":"/path/to/main.py","content":"print(\"Hello world\")"}"#));
        assert!(description.contains(
            r#"{"payload":"+++ /path/to/main.py\nprint(\"Hello world\")","file_path":"/path/to/main.py"}"#
        ));
    }

    #[tokio::test]
    async fn schema_requires_single_payload_parameter() {
        let tool = FileWriteTool::new();

        let schema = tool.input_schema_for_model().await;

        assert_eq!(schema["required"], serde_json::json!(["payload"]));
        assert_eq!(
            schema["properties"].as_object().map(|value| value.len()),
            Some(1)
        );
        assert!(schema["properties"].get("payload").is_some());
        assert!(schema["properties"].get("mode").is_none());
    }

    #[tokio::test]
    async fn validate_input_requires_payload() {
        let tool = FileWriteTool::new();

        let validation = tool.validate_input(&json!({}), None).await;

        assert!(!validation.result);
        assert_eq!(validation.message.as_deref(), Some("payload is required"));
    }

    #[tokio::test]
    async fn validate_input_accepts_path_only_for_empty_file() {
        let tool = FileWriteTool::new();

        let validation = tool
            .validate_input(&json!({ "payload": "+++ C:/workspace/empty.txt" }), None)
            .await;

        assert!(validation.result);
        assert!(validation.message.is_none());
    }

    #[test]
    fn parse_payload_recognizes_marked_path_with_lf_or_crlf() {
        for value in [
            "+++ C:/workspace/main.rs\nfn main() {}",
            "+++ C:/workspace/main.rs\r\nfn main() {}",
        ] {
            let input = json!({ "payload": value });
            let parsed = FileWriteTool::parse_payload(&input).expect("valid payload");

            assert_eq!(
                parsed,
                super::ParsedWritePayload::Target {
                    file_path: "C:/workspace/main.rs",
                    content: "fn main() {}",
                }
            );
        }
    }

    #[test]
    fn parse_payload_treats_missing_or_empty_marker_path_as_fallback_content() {
        for value in ["content", "\ncontent", "+++", "+++ \ncontent"] {
            let input = json!({ "payload": value });
            let parsed = FileWriteTool::parse_payload(&input).expect("fallback payload");

            assert_eq!(
                parsed,
                super::ParsedWritePayload::MissingPath { content: value }
            );
        }
    }

    #[tokio::test]
    async fn call_impl_preserves_malformed_payload_in_workspace_temp_file() {
        let root = std::env::temp_dir().join(format!("bitfun-write-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp workspace");
        let original_payload = "def main():\n    print(\"Hello world\")";

        let results = FileWriteTool::new()
            .call(
                &json!({ "payload": original_payload }),
                &local_context(root.clone()),
            )
            .await
            .expect("malformed payload should be preserved");

        let entries = std::fs::read_dir(&root)
            .expect("read workspace root")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect workspace entries");
        assert_eq!(entries.len(), 1);
        let file_name = entries[0].file_name().to_string_lossy().into_owned();
        assert!(file_name.starts_with("write_"));
        assert!(file_name.ends_with(".tmp"));
        assert_eq!(file_name.len(), "write_000000.tmp".len());
        assert!(file_name[6..12]
            .chars()
            .all(|character| character.is_ascii_hexdigit()));
        assert_eq!(
            std::fs::read_to_string(entries[0].path()).expect("read fallback file"),
            original_payload
        );

        let ToolResult::Result {
            data,
            result_for_assistant,
            ..
        } = &results[0]
        else {
            panic!("expected result");
        };
        let result_path = data["file_path"].as_str().expect("result file path");
        assert_eq!(
            Path::new(result_path)
                .file_name()
                .and_then(|value| value.to_str()),
            Some(file_name.as_str())
        );
        assert_eq!(data["missing_path_fallback"], true);
        assert_eq!(data["rename_required"], true);
        assert!(result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .contains("Rename this temporary file"));

        let _ = std::fs::remove_dir_all(&root);
    }
}
