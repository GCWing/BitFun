use crate::agentic::tools::file_permissions::file_permission_intents;
use crate::agentic::tools::file_read_state_runtime::{
    get_review_read_coverage, local_file_modification_time_ms, local_file_revision,
    record_file_read_state, record_review_read_receipt, review_read_receipts_enabled,
};
use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::workspace_paths::is_bitfun_tool_uri;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::timing::elapsed_ms_u64;
use async_trait::async_trait;
use log::{debug, warn};
use serde_json::{json, Value};
use std::convert::TryFrom;
use std::path::Path;
use std::time::{Duration, Instant};
use tool_runtime::fs::document::{
    convert_document_to_markdown, is_supported_document_path, DocumentConversionError,
    MAX_DOCUMENT_INPUT_BYTES, MAX_DOCUMENT_MARKDOWN_BYTES,
};
use tool_runtime::fs::read_file::{
    build_read_file_presentation, build_remote_read_command, build_remote_tail_read_command,
    parse_remote_read_output, parse_remote_tail_read_output, read_file, read_file_bytes_bounded,
    read_file_tail, read_text, read_text_tail, ReadFileResult,
};

pub struct FileReadTool {
    default_max_lines_to_read: usize,
    max_line_chars: usize,
    max_total_chars: usize,
}

/// Default cap on characters returned by a single Read call (excluding wrapper text).
pub const DEFAULT_READ_MAX_TOTAL_CHARS: usize = 64_000;
// anydoc is synchronous, so this bounds the caller's wait rather than terminating the parser.
// The worker retains the global conversion permit until it actually exits, keeping failures closed.
const DOCUMENT_CONVERSION_TIMEOUT: Duration = Duration::from_secs(30);

struct DocumentReadMetadata {
    source_format: &'static str,
    source_size_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadRenderMode {
    Auto,
    Source,
    Markdown,
}

impl Default for FileReadTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FileReadTool {
    pub fn new() -> Self {
        Self {
            default_max_lines_to_read: 2000,
            max_line_chars: 2000,
            max_total_chars: DEFAULT_READ_MAX_TOTAL_CHARS,
        }
    }

    pub fn with_config(
        default_max_lines_to_read: usize,
        max_line_chars: usize,
        max_total_chars: usize,
    ) -> Self {
        Self {
            default_max_lines_to_read,
            max_line_chars,
            max_total_chars,
        }
    }

    fn already_served_result(
        logical_path: &str,
        coverage: crate::agentic::session::ReviewReadCoverage,
    ) -> ToolResult {
        ToolResult::Result {
            data: json!({
                "file_path": logical_path,
                "status": "already_served",
                "start_line": coverage.start_line,
                "end_line": coverage.end_line,
                "total_lines": coverage.total_lines,
            }),
            result_for_assistant: Some(format!(
                "{} lines {}-{} were already returned earlier in this review and the file revision is unchanged. Reuse the prior Read output; request only an unread range if more context is needed.",
                logical_path, coverage.start_line, coverage.end_line
            )),
            image_attachments: None,
        }
    }

    fn read_window_start_line(input: &Value) -> Result<usize, String> {
        Self::optional_line_number(input, "offset")?.map_or(Ok(1), |offset| Ok(offset.max(1)))
    }

    fn read_tail_mode(input: &Value) -> Result<bool, String> {
        let tail = match input.get("tail") {
            Some(value) => value
                .as_bool()
                .ok_or_else(|| "tail must be a boolean".to_string())?,
            None => false,
        };

        if tail && input.get("offset").is_some() {
            return Err("Do not provide offset when tail is true".to_string());
        }

        Ok(tail)
    }

    fn read_render_mode(input: &Value) -> Result<ReadRenderMode, String> {
        match input.get("render") {
            None => Ok(ReadRenderMode::Auto),
            Some(Value::String(value)) if value == "auto" => Ok(ReadRenderMode::Auto),
            Some(Value::String(value)) if value == "source" => Ok(ReadRenderMode::Source),
            Some(Value::String(value)) if value == "markdown" => Ok(ReadRenderMode::Markdown),
            Some(_) => Err("render must be one of: auto, source, markdown".to_string()),
        }
    }

    fn path_has_csv_extension(path: &str) -> bool {
        Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("csv"))
    }

    fn optional_line_number(input: &Value, key: &str) -> Result<Option<usize>, String> {
        match input.get(key) {
            Some(value) => Self::line_number_from_value(value)
                .map(Some)
                .map_err(|message| format!("{} {}", key, message)),
            None => Ok(None),
        }
    }

    fn line_number_from_value(value: &Value) -> Result<usize, &'static str> {
        if let Some(number) = value.as_u64() {
            return usize::try_from(number).map_err(|_| "is too large");
        }

        if let Some(number) = value.as_i64() {
            if number < 0 {
                return Err("must be a non-negative integer");
            }
            return usize::try_from(number as u64).map_err(|_| "is too large");
        }

        if let Some(number) = value.as_f64() {
            if !number.is_finite() || number < 0.0 || number.fract() != 0.0 {
                return Err("must be a non-negative integer");
            }
            if number > usize::MAX as f64 {
                return Err("is too large");
            }
            return Ok(number as usize);
        }

        Err("must be a non-negative integer")
    }

    async fn read_remote_window(
        &self,
        resolved_path: &str,
        start_line: usize,
        limit: usize,
        context: &ToolUseContext,
    ) -> BitFunResult<tool_runtime::fs::read_file::ReadFileResult> {
        let ws_shell = context.ws_shell().ok_or_else(|| {
            BitFunError::tool("Remote workspace shell is unavailable".to_string())
        })?;

        let command = build_remote_read_command(
            resolved_path,
            start_line,
            limit,
            self.max_line_chars,
            self.max_total_chars,
        )
        .map_err(BitFunError::tool)?;

        let remote_read_started_at = Instant::now();
        debug!(
            "Remote file read started: path={}, start_line={}, limit={}, timeout_ms={:?}, session_id={:?}, dialog_turn_id={:?}",
            resolved_path,
            start_line,
            limit,
            Option::<u64>::None,
            context.session_id,
            context.dialog_turn_id
        );
        let (stdout, stderr, status) = ws_shell
            .exec(&command, None)
            .await
            .map_err(|e| {
                warn!(
                    "Remote file read failed: path={}, start_line={}, limit={}, duration_ms={}, error={}",
                    resolved_path,
                    start_line,
                    limit,
                    elapsed_ms_u64(remote_read_started_at),
                    e
                );
                BitFunError::tool(format!("Failed to read file: {}", e))
            })?;
        debug!(
            "Remote file read command completed: path={}, start_line={}, limit={}, status={}, stdout_len={}, stderr_len={}, duration_ms={}",
            resolved_path,
            start_line,
            limit,
            status,
            stdout.len(),
            stderr.len(),
            elapsed_ms_u64(remote_read_started_at)
        );

        let result = parse_remote_read_output(&stdout, &stderr, status, resolved_path, start_line)
            .map_err(BitFunError::tool)?;

        debug!(
            "Remote file read parsed successfully: path={}, start_line={}, end_line={}, total_lines={}, hit_total_char_limit={}, duration_ms={}",
            resolved_path,
            result.start_line,
            result.end_line,
            result.total_lines,
            result.hit_total_char_limit,
            elapsed_ms_u64(remote_read_started_at)
        );

        Ok(result)
    }

    async fn read_remote_tail_window(
        &self,
        resolved_path: &str,
        limit: usize,
        context: &ToolUseContext,
    ) -> BitFunResult<tool_runtime::fs::read_file::ReadFileResult> {
        let ws_shell = context.ws_shell().ok_or_else(|| {
            BitFunError::tool("Remote workspace shell is unavailable".to_string())
        })?;

        let command = build_remote_tail_read_command(
            resolved_path,
            limit,
            self.max_line_chars,
            self.max_total_chars,
        )
        .map_err(BitFunError::tool)?;

        let remote_read_started_at = Instant::now();
        debug!(
            "Remote file tail read started: path={}, limit={}, timeout_ms={:?}, session_id={:?}, dialog_turn_id={:?}",
            resolved_path,
            limit,
            Option::<u64>::None,
            context.session_id,
            context.dialog_turn_id
        );
        let (stdout, stderr, status) = ws_shell.exec(&command, None).await.map_err(|e| {
            warn!(
                "Remote file tail read failed: path={}, limit={}, duration_ms={}, error={}",
                resolved_path,
                limit,
                elapsed_ms_u64(remote_read_started_at),
                e
            );
            BitFunError::tool(format!("Failed to read file: {}", e))
        })?;
        debug!(
            "Remote file tail read command completed: path={}, limit={}, status={}, stdout_len={}, stderr_len={}, duration_ms={}",
            resolved_path,
            limit,
            status,
            stdout.len(),
            stderr.len(),
            elapsed_ms_u64(remote_read_started_at)
        );

        let result = parse_remote_tail_read_output(&stdout, &stderr, status, resolved_path, limit)
            .map_err(BitFunError::tool)?;

        debug!(
            "Remote file tail read parsed successfully: path={}, start_line={}, end_line={}, total_lines={}, hit_total_char_limit={}, duration_ms={}",
            resolved_path,
            result.start_line,
            result.end_line,
            result.total_lines,
            result.hit_total_char_limit,
            elapsed_ms_u64(remote_read_started_at)
        );

        Ok(result)
    }

    async fn read_document_window(
        &self,
        resolved_path: &str,
        logical_path: &str,
        start_line: usize,
        limit: usize,
        tail: bool,
        uses_remote_workspace_backend: bool,
        context: &ToolUseContext,
    ) -> BitFunResult<(ReadFileResult, DocumentReadMetadata)> {
        let bytes = if uses_remote_workspace_backend {
            let ws_fs = context.ws_fs().ok_or_else(|| {
                BitFunError::tool("Remote workspace file system is unavailable".to_string())
            })?;
            ws_fs
                .read_file_bounded(resolved_path, MAX_DOCUMENT_INPUT_BYTES)
                .await
                .map_err(|error| {
                    BitFunError::tool(format!(
                        "Failed to read document {}: {}",
                        logical_path, error
                    ))
                })?
        } else {
            read_file_bytes_bounded(resolved_path, MAX_DOCUMENT_INPUT_BYTES)
                .map_err(BitFunError::tool)?
        }
        .ok_or_else(|| {
            BitFunError::tool(format!(
                "Document {} is larger than the {} MiB Read limit",
                logical_path,
                MAX_DOCUMENT_INPUT_BYTES / (1024 * 1024)
            ))
        })?;

        let source_size_bytes = bytes.len();
        let conversion_started_at = Instant::now();
        debug!(
            "Document conversion started: path={}, source_size_bytes={}, session_id={:?}, dialog_turn_id={:?}",
            logical_path,
            source_size_bytes,
            context.session_id,
            context.dialog_turn_id
        );
        let conversion = tokio::time::timeout(
            DOCUMENT_CONVERSION_TIMEOUT,
            convert_document_to_markdown(bytes, resolved_path.to_string()),
        )
        .await
        .map_err(|_| {
            warn!(
                "Document conversion timed out: path={}, source_size_bytes={}, timeout_ms={}, duration_ms={}",
                logical_path,
                source_size_bytes,
                DOCUMENT_CONVERSION_TIMEOUT.as_millis(),
                elapsed_ms_u64(conversion_started_at)
            );
            BitFunError::tool(format!(
                "Document conversion did not finish within {} seconds: {}",
                DOCUMENT_CONVERSION_TIMEOUT.as_secs(),
                logical_path
            ))
        })?;
        let converted = conversion.map_err(|error| {
                warn!(
                    "Document conversion failed: path={}, source_size_bytes={}, duration_ms={}, error_code={}, error={}",
                    logical_path,
                    source_size_bytes,
                    elapsed_ms_u64(conversion_started_at),
                    error.code(),
                    error
                );
                Self::document_conversion_error(logical_path, resolved_path, error)
            })?;
        debug!(
            "Document conversion completed: path={}, source_format={}, source_size_bytes={}, markdown_size_bytes={}, duration_ms={}",
            logical_path,
            converted.source_format,
            source_size_bytes,
            converted.markdown.len(),
            elapsed_ms_u64(conversion_started_at)
        );

        let read_result = if tail {
            read_text_tail(
                &converted.markdown,
                limit,
                self.max_line_chars,
                self.max_total_chars,
            )
        } else {
            read_text(
                &converted.markdown,
                start_line,
                limit,
                self.max_line_chars,
                self.max_total_chars,
            )
        }
        .map_err(BitFunError::tool)?;

        Ok((
            read_result,
            DocumentReadMetadata {
                source_format: converted.source_format,
                source_size_bytes,
            },
        ))
    }

    fn document_conversion_error(
        logical_path: &str,
        resolved_path: &str,
        error: DocumentConversionError,
    ) -> BitFunError {
        let ocr_hint = (error.code() == "unsupported"
            && Path::new(resolved_path)
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf")))
        .then_some(
            " Text PDFs are supported, but scanned or image-only PDFs require an OCR workflow.",
        )
        .unwrap_or_default();
        BitFunError::tool(format!(
            "Failed to convert document {} to Markdown ({}): {}.{}",
            logical_path,
            error.code(),
            error,
            ocr_hint
        ))
    }
}

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "Read"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(format!(
            r#"Reads a file from the current workspace filesystem. Office documents, OpenDocument files, RTF, EPUB, and PDFs are converted locally to GitHub-Flavored Markdown before reading. If the User provides a path to a file assume that path is valid. It is okay to read a file that does not exist; an error will be returned.

Usage:
- The file_path parameter must be workspace-relative, an absolute path inside the current workspace, or an exact `bitfun://...` URI returned by another tool.
- Do not read host roots or placeholder paths such as `/workspace`.
- Supported document extensions are .doc, .docx, .docm, .ppt, .pps, .pot, .pptx, .pptm, .ppsx, .ppsm, .xls, .xlsx, .xlsm, .xlsb, .odt, .ods, .odp, .rtf, .epub, .csv, and .pdf. Document input is capped at {} MiB and extracted Markdown at {} MiB. Conversion is offline and never fetches linked resources.
- render defaults to auto. auto converts supported documents but preserves CSV as exact source text for editing compatibility. Use render=markdown to turn CSV into a Markdown table or to content-detect a document with a missing/wrong extension. Use render=source to bypass conversion for a textual document such as CSV or RTF.
- For converted documents, offset, limit, tail, line numbers, and total_lines refer to the extracted Markdown, not source pages or rows. The Markdown is a read-only representation; do not use it as exact source text for Edit. Embedded objects are represented by text, and scanned/image-only PDF pages require OCR.
- By default, it reads up to {} lines starting from the beginning of the file. When you plan to Edit a file, prefer this default full read so you see the exact bytes you will need to match.
- You can optionally specify an offset and limit. offset is a 1-based line number. Use a range only when you already know the target lines; the range must include every line you will copy into Edit `old_string`.
- You can set tail=true with limit to read the last N lines. This is useful for command output and logs. Do not combine tail=true with offset.
- Any lines longer than {} characters will be truncated.
- Total output is capped at {} characters. If that limit is hit, continue with offset/limit, until the target lines are fully visible, then Edit using only text from those Read results.
- Results are returned using cat -n format, with line numbers starting at 1.
- This tool can only read files, not directories.
- You can call multiple tools in a single response. It is always better to speculatively read multiple potentially useful files in parallel.
- Avoid tiny repeated slices (e.g. 30-100 line chunks). If you need more context, read a larger window that covers the whole block you will edit.
- Do not use `limit` with a small value (e.g. < 50) to probe file type or structure. Source files typically begin with copyright headers — a probe read returns no useful code.
"#,
            MAX_DOCUMENT_INPUT_BYTES / (1024 * 1024),
            MAX_DOCUMENT_MARKDOWN_BYTES / (1024 * 1024),
            self.default_max_lines_to_read,
            self.max_line_chars,
            self.max_total_chars
        ))
    }

    fn short_description(&self) -> String {
        "Read text files and extract documents.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The file to read. Use a workspace-relative path, an absolute path inside the current workspace, or an exact bitfun:// URI returned by another tool."
                },
                "render": {
                    "type": "string",
                    "enum": ["auto", "source", "markdown"],
                    "description": "How to represent the file. auto converts supported documents but preserves CSV source text; source bypasses conversion; markdown forces local anydoc conversion and enables content detection. Defaults to auto."
                },
                "offset": {
                    "type": "number",
                    "description": "The 1-based line number to start reading from. offset=0 is accepted as offset=1. Only provide if the file is too large to read at once."
                },
                "tail": {
                    "type": "boolean",
                    "description": "Read the last N lines of the file, where N is limit. Do not provide offset when tail is true."
                },
                "limit": {
                    "type": "number",
                    "description": "The number of lines to read. Only provide if the file is too large to read at once."
                }
            },
            "required": ["file_path"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
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
        file_permission_intents("read", [file_path], context)
    }

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let file_path = match input.get("file_path").and_then(|v| v.as_str()) {
            Some(p) if !p.is_empty() => p,
            Some(_) => {
                return ValidationResult {
                    result: false,
                    message: Some("file_path cannot be empty".to_string()),
                    error_code: Some(400),
                    meta: None,
                }
            }
            None => {
                return ValidationResult {
                    result: false,
                    message: Some("file_path is required".to_string()),
                    error_code: Some(400),
                    meta: None,
                }
            }
        };

        if let Err(message) = Self::read_tail_mode(input)
            .and_then(|_| Self::read_window_start_line(input))
            .and_then(|_| Self::read_render_mode(input))
        {
            return ValidationResult {
                result: false,
                message: Some(message),
                error_code: Some(400),
                meta: None,
            };
        }

        let resolved = match context.map(|ctx| ctx.resolve_tool_path(file_path)) {
            Some(Ok(path)) => path,
            Some(Err(err)) => {
                return ValidationResult {
                    result: false,
                    message: Some(err.to_string()),
                    error_code: Some(400),
                    meta: None,
                }
            }
            None => {
                if is_bitfun_tool_uri(file_path) {
                    return ValidationResult {
                        result: false,
                        message: Some(
                            "Tool context is required to resolve BitFun URIs".to_string(),
                        ),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                let path = Path::new(file_path);
                if !path.is_absolute() {
                    return ValidationResult {
                        result: false,
                        message: Some("file_path must be absolute".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                if !path.exists() {
                    return ValidationResult {
                        result: false,
                        message: Some(format!("File does not exist: {}", file_path)),
                        error_code: Some(404),
                        meta: None,
                    };
                }

                if !path.is_file() {
                    return ValidationResult {
                        result: false,
                        message: Some(format!("Path is not a file: {}", file_path)),
                        error_code: Some(400),
                        meta: None,
                    };
                }

                return ValidationResult::default();
            }
        };

        if !resolved.uses_remote_workspace_backend() {
            let path = Path::new(&resolved.resolved_path);
            if !path.exists() {
                return ValidationResult {
                    result: false,
                    message: Some(format!("File does not exist: {}", resolved.logical_path)),
                    error_code: Some(404),
                    meta: None,
                };
            }
            if !path.is_file() {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Path is not a file: {}", resolved.logical_path)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        ValidationResult::default()
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        if let Some(file_path) = input.get("file_path").and_then(|v| v.as_str()) {
            if options.verbose {
                format!("Reading file: {}", file_path)
            } else {
                format!("Read {}", file_path)
            }
        } else {
            "Reading file".to_string()
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

        let tail = Self::read_tail_mode(input).map_err(BitFunError::tool)?;
        let render_mode = Self::read_render_mode(input).map_err(BitFunError::tool)?;
        let start_line = Self::read_window_start_line(input).map_err(BitFunError::tool)?;

        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.default_max_lines_to_read as u64) as usize;

        let resolved = context.resolve_tool_path(file_path)?;
        crate::agentic::deep_review::scope::ensure_focused_review_resolved_path_allowed(
            context,
            &resolved.resolved_path,
        )?;
        let supported_document_path = is_supported_document_path(&resolved.logical_path)
            || is_supported_document_path(&resolved.resolved_path);
        let csv_path = Self::path_has_csv_extension(&resolved.logical_path)
            || Self::path_has_csv_extension(&resolved.resolved_path);
        let reads_document_representation = match render_mode {
            ReadRenderMode::Auto => supported_document_path && !csv_path,
            ReadRenderMode::Source => false,
            ReadRenderMode::Markdown => true,
        };
        let revision_before_read = if reads_document_representation
            || resolved.uses_remote_workspace_backend()
            || tail
            || !review_read_receipts_enabled(context)
        {
            None
        } else {
            local_file_revision(Path::new(&resolved.resolved_path))
        };
        if let Some(coverage) = revision_before_read.and_then(|revision| {
            get_review_read_coverage(context, &resolved, revision, start_line, limit)
        }) {
            return Ok(vec![Self::already_served_result(
                &resolved.logical_path,
                coverage,
            )]);
        }

        let (read_file_result, document_metadata) = if reads_document_representation {
            let (result, metadata) = self
                .read_document_window(
                    &resolved.resolved_path,
                    &resolved.logical_path,
                    start_line,
                    limit,
                    tail,
                    resolved.uses_remote_workspace_backend(),
                    context,
                )
                .await?;
            (result, Some(metadata))
        } else if resolved.uses_remote_workspace_backend() {
            if tail {
                (
                    self.read_remote_tail_window(&resolved.resolved_path, limit, context)
                        .await?,
                    None,
                )
            } else {
                (
                    self.read_remote_window(&resolved.resolved_path, start_line, limit, context)
                        .await?,
                    None,
                )
            }
        } else if tail {
            (
                read_file_tail(
                    &resolved.resolved_path,
                    limit,
                    self.max_line_chars,
                    self.max_total_chars,
                )
                .map_err(BitFunError::tool)?,
                None,
            )
        } else {
            (
                read_file(
                    &resolved.resolved_path,
                    start_line,
                    limit,
                    self.max_line_chars,
                    self.max_total_chars,
                )
                .map_err(BitFunError::tool)?,
                None,
            )
        };

        if document_metadata.is_none() {
            let timestamp_ms = if resolved.uses_remote_workspace_backend() {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_millis() as u64)
                    .unwrap_or(0)
            } else {
                local_file_modification_time_ms(Path::new(&resolved.resolved_path))
            };
            record_file_read_state(context, &resolved, &read_file_result, timestamp_ms);
        }
        if let (Some(revision_before), Some(revision_after)) = (
            revision_before_read,
            (!resolved.uses_remote_workspace_backend() && !tail)
                .then(|| local_file_revision(Path::new(&resolved.resolved_path)))
                .flatten(),
        ) {
            if revision_before == revision_after {
                record_review_read_receipt(context, &resolved, revision_after, &read_file_result);
            }
        }

        let presentation = build_read_file_presentation(&resolved.logical_path, &read_file_result);
        let mut result_for_assistant = presentation.result_for_assistant;
        if let Some(metadata) = document_metadata.as_ref() {
            let extraction_note = if metadata.source_format == "pdf" {
                " OCR is not performed, so scanned or image-only pages may be omitted."
            } else {
                " Embedded images and objects are represented by their available text."
            };
            result_for_assistant = format!(
                "Converted {} from {} to GitHub-Flavored Markdown with anydoc. offset and limit refer to converted Markdown lines.{}\n\n{}",
                resolved.logical_path,
                metadata.source_format.to_ascii_uppercase(),
                extraction_note,
                result_for_assistant
            );
        }

        let mut data = json!({
            "file_path": resolved.logical_path,
            "content": read_file_result.content,
            "total_lines": read_file_result.total_lines,
            "lines_read": presentation.lines_read,
            "offset": read_file_result.start_line,
            "tail": tail,
            "start_line": read_file_result.start_line,
            "size": read_file_result.content.len(),
            "hit_total_char_limit": read_file_result.hit_total_char_limit
        });
        if let Some(metadata) = document_metadata {
            data["representation"] = json!("extracted_markdown");
            data["source_format"] = json!(metadata.source_format);
            data["source_size_bytes"] = json!(metadata.source_size_bytes);
            data["conversion_engine"] = json!("anydoc");
            data["extraction_warnings"] = if metadata.source_format == "pdf" {
                json!(["OCR is not performed; scanned or image-only pages may be omitted."])
            } else {
                json!(["Embedded images and objects are represented by their available text."])
            };
        }

        let result = ToolResult::Result {
            data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        };

        Ok(vec![result])
    }
}

#[cfg(test)]
mod tests {
    use super::{FileReadTool, ReadRenderMode, MAX_DOCUMENT_INPUT_BYTES};
    use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::agentic::WorkspaceBinding;
    use async_trait::async_trait;
    use bitfun_runtime_ports::{
        ToolRuntimeHandles, WorkspaceCommandOptions, WorkspaceCommandResult, WorkspaceDirEntry,
        WorkspaceFileSystem, WorkspaceServices, WorkspaceShell,
    };
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn local_context(root: PathBuf) -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: Some("Agent".to_string()),
            session_id: None,
            dialog_turn_id: Some("turn-1".to_string()),
            workspace: Some(WorkspaceBinding::new(
                Some("read-document-workspace".to_string()),
                root,
            )),
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            runtime_handles: ToolRuntimeHandles::default(),
        }
    }

    struct FakeRemoteFs {
        bytes: Vec<u8>,
        bounded_limit: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl WorkspaceFileSystem for FakeRemoteFs {
        async fn read_file(&self, _path: &str) -> anyhow::Result<Vec<u8>> {
            Ok(self.bytes.clone())
        }

        async fn read_file_bounded(
            &self,
            _path: &str,
            max_bytes: usize,
        ) -> anyhow::Result<Option<Vec<u8>>> {
            self.bounded_limit.store(max_bytes, Ordering::Relaxed);
            Ok((self.bytes.len() <= max_bytes).then(|| self.bytes.clone()))
        }

        async fn read_file_text(&self, _path: &str) -> anyhow::Result<String> {
            Ok(String::from_utf8_lossy(&self.bytes).to_string())
        }

        async fn write_file(&self, _path: &str, _contents: &[u8]) -> anyhow::Result<()> {
            Ok(())
        }

        async fn exists(&self, _path: &str) -> anyhow::Result<bool> {
            Ok(true)
        }

        async fn is_file(&self, _path: &str) -> anyhow::Result<bool> {
            Ok(true)
        }

        async fn is_dir(&self, _path: &str) -> anyhow::Result<bool> {
            Ok(false)
        }

        async fn read_dir(&self, _path: &str) -> anyhow::Result<Vec<WorkspaceDirEntry>> {
            Ok(Vec::new())
        }
    }

    struct PanicRemoteShell;

    #[async_trait]
    impl WorkspaceShell for PanicRemoteShell {
        async fn exec_with_options(
            &self,
            _command: &str,
            _options: WorkspaceCommandOptions,
        ) -> anyhow::Result<WorkspaceCommandResult> {
            panic!("document reads must not require a remote shell or remote anydoc install")
        }
    }

    fn remote_context(bytes: Vec<u8>, bounded_limit: Arc<AtomicUsize>) -> ToolUseContext {
        let root = "/remote/workspace";
        let session_identity =
            crate::service::remote_ssh::workspace_state::workspace_session_identity(
                root,
                Some("conn-1"),
                Some("remote-host"),
            )
            .expect("remote workspace identity");
        let mut context = local_context(PathBuf::from(root));
        context.workspace = Some(WorkspaceBinding::new_remote(
            Some("read-document-remote".to_string()),
            PathBuf::from(root),
            "conn-1".to_string(),
            "remote-host".to_string(),
            session_identity,
        ));
        context.runtime_handles = ToolRuntimeHandles::new(
            Some(WorkspaceServices {
                fs: Arc::new(FakeRemoteFs {
                    bytes,
                    bounded_limit,
                }),
                shell: Arc::new(PanicRemoteShell),
            }),
            None,
        );
        context
    }

    #[test]
    fn read_tool_schema_prefers_offset() {
        let schema = FileReadTool::new().input_schema();
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties");

        assert!(properties.contains_key("offset"));
        assert!(properties.contains_key("tail"));
        assert_eq!(
            properties["render"]["enum"],
            json!(["auto", "source", "markdown"])
        );
    }

    #[test]
    fn read_window_start_line_prefers_offset_and_normalizes_zero() {
        assert_eq!(
            FileReadTool::read_window_start_line(&json!({ "offset": 0 })).expect("offset"),
            1
        );
        assert_eq!(
            FileReadTool::read_window_start_line(&json!({ "offset": 42 })).expect("offset"),
            42
        );
        assert_eq!(
            FileReadTool::read_window_start_line(&json!({})).expect("default offset"),
            1
        );
    }

    #[test]
    fn read_tail_mode_rejects_offset() {
        let error = FileReadTool::read_tail_mode(&json!({
            "tail": true,
            "offset": 3
        }))
        .expect_err("tail and offset should not coexist");

        assert_eq!(error, "Do not provide offset when tail is true");
    }

    #[test]
    fn read_render_mode_defaults_to_auto_and_rejects_unknown_values() {
        assert_eq!(
            FileReadTool::read_render_mode(&json!({})).expect("default render"),
            ReadRenderMode::Auto
        );
        assert_eq!(
            FileReadTool::read_render_mode(&json!({ "render": "source" })).expect("source render"),
            ReadRenderMode::Source
        );
        assert!(FileReadTool::read_render_mode(&json!({ "render": "html" })).is_err());
        assert!(FileReadTool::read_render_mode(&json!({ "render": 1 })).is_err());
    }

    #[tokio::test]
    async fn read_converts_rtf_to_a_markdown_representation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = br"{\rtf1\ansi Hello from the document}";
        fs::write(dir.path().join("notes.rtf"), source).expect("write RTF");

        let results = FileReadTool::new()
            .call_impl(
                &json!({ "file_path": "notes.rtf" }),
                &local_context(dir.path().to_path_buf()),
            )
            .await
            .expect("document read should succeed");

        let ToolResult::Result {
            data,
            result_for_assistant,
            ..
        } = &results[0]
        else {
            panic!("expected result");
        };
        assert_eq!(data["representation"], "extracted_markdown");
        assert_eq!(data["source_format"], "rtf");
        assert_eq!(data["conversion_engine"], "anydoc");
        assert_eq!(data["source_size_bytes"], source.len());
        assert!(data["content"]
            .as_str()
            .is_some_and(|content| content.contains("Hello from the document")));
        assert!(result_for_assistant
            .as_deref()
            .is_some_and(|result| result.contains("from RTF to GitHub-Flavored Markdown")));
    }

    #[tokio::test]
    async fn csv_auto_preserves_source_while_markdown_render_extracts_a_table() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(
            dir.path().join("table.csv"),
            "name,value\nalpha,1\nbeta,2\n",
        )
        .expect("write CSV");
        let context = local_context(dir.path().to_path_buf());
        let tool = FileReadTool::new();

        let auto = tool
            .call_impl(&json!({ "file_path": "table.csv" }), &context)
            .await
            .expect("source read should succeed");
        let markdown = tool
            .call_impl(
                &json!({ "file_path": "table.csv", "render": "markdown" }),
                &context,
            )
            .await
            .expect("Markdown read should succeed");

        let ToolResult::Result {
            data: auto_data, ..
        } = &auto[0]
        else {
            panic!("expected source result");
        };
        let ToolResult::Result {
            data: markdown_data,
            ..
        } = &markdown[0]
        else {
            panic!("expected Markdown result");
        };
        assert!(auto_data.get("representation").is_none());
        assert!(auto_data["content"]
            .as_str()
            .is_some_and(|content| content.contains("name,value")));
        assert_eq!(markdown_data["representation"], "extracted_markdown");
        assert_eq!(markdown_data["source_format"], "csv");
        assert!(markdown_data["content"]
            .as_str()
            .is_some_and(|content| content.contains("| name | value |")));
    }

    #[tokio::test]
    async fn remote_document_uses_bounded_file_transfer_and_host_side_conversion() {
        let bounded_limit = Arc::new(AtomicUsize::new(0));
        let context = remote_context(
            br"{\rtf1\ansi Hello from remote RTF}".to_vec(),
            Arc::clone(&bounded_limit),
        );

        let results = FileReadTool::new()
            .call_impl(&json!({ "file_path": "notes.rtf" }), &context)
            .await
            .expect("remote document read should succeed");

        let ToolResult::Result { data, .. } = &results[0] else {
            panic!("expected result");
        };
        assert_eq!(
            bounded_limit.load(Ordering::Relaxed),
            MAX_DOCUMENT_INPUT_BYTES
        );
        assert_eq!(data["representation"], "extracted_markdown");
        assert!(data["content"]
            .as_str()
            .is_some_and(|content| content.contains("Hello from remote RTF")));
    }
}
