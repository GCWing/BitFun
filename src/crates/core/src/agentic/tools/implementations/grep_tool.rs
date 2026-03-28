use super::util::resolve_path_with_workspace;
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use tool_runtime::search::grep_search::{grep_search, GrepOptions, OutputMode, ProgressCallback};

pub struct GrepTool;

impl GrepTool {
    pub fn new() -> Self {
        Self
    }

    async fn call_remote(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let ws_shell = context
            .ws_shell()
            .ok_or_else(|| BitFunError::tool("Workspace shell not available".to_string()))?;

        let pattern = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("pattern is required".to_string()))?;

        let search_path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let resolved_path = resolve_path_with_workspace(
            search_path,
            context.current_working_directory(),
            context.workspace_root(),
        )?;

        let case_insensitive = input.get("-i").and_then(|v| v.as_bool()).unwrap_or(false);
        let head_limit = input
            .get("head_limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(200);
        let glob_pattern = input.get("glob").and_then(|v| v.as_str());
        let file_type = input.get("type").and_then(|v| v.as_str());

        let escaped_path = resolved_path.replace('\'', "'\\''");
        let escaped_pattern = pattern.replace('\'', "'\\''");

        let mut cmd = "rg --no-heading --line-number".to_string();
        if case_insensitive {
            cmd.push_str(" -i");
        }
        if let Some(glob) = glob_pattern {
            cmd.push_str(&format!(" --glob '{}'", glob.replace('\'', "'\\''")));
        }
        if let Some(ft) = file_type {
            cmd.push_str(&format!(" --type {}", ft));
        }
        cmd.push_str(&format!(
            " '{}' '{}' 2>/dev/null | head -n {}",
            escaped_pattern, escaped_path, head_limit
        ));

        let full_cmd = format!(
            "if command -v rg >/dev/null 2>&1; then {}; else grep -rn{} '{}' '{}' 2>/dev/null | head -n {}; fi",
            cmd,
            if case_insensitive { "i" } else { "" },
            escaped_pattern,
            escaped_path,
            head_limit,
        );

        let (stdout, _stderr, _exit_code) = ws_shell
            .exec(&full_cmd, Some(30_000))
            .await
            .map_err(|e| BitFunError::tool(format!("Remote grep failed: {}", e)))?;

        let lines: Vec<&str> = stdout.lines().collect();
        let total_matches = lines.len();
        let result_text = if lines.is_empty() {
            format!("No matches found for pattern '{}'", pattern)
        } else {
            stdout.clone()
        };

        Ok(vec![ToolResult::Result {
            data: json!({
                "pattern": pattern,
                "path": resolved_path,
                "output_mode": "content",
                "total_matches": total_matches,
                "result": result_text,
            }),
            result_for_assistant: Some(result_text),
            image_attachments: None,
        }])
    }

    fn build_grep_options(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<GrepOptions> {
        let pattern = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("pattern is required".to_string()))?;

        let search_path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let resolved_path = resolve_path_with_workspace(
            search_path,
            context.current_working_directory(),
            context.workspace_root(),
        )?;

        let case_insensitive = input.get("-i").and_then(|v| v.as_bool()).unwrap_or(false);
        let multiline = input
            .get("multiline")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let output_mode_str = input
            .get("output_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("files_with_matches");
        let output_mode = OutputMode::from_str(output_mode_str);
        let show_line_numbers = input.get("-n").and_then(|v| v.as_bool()).unwrap_or(false);
        let context_c = input.get("-C").and_then(|v| v.as_u64()).map(|v| v as usize);
        let before_context = input.get("-B").and_then(|v| v.as_u64()).map(|v| v as usize);
        let after_context = input.get("-A").and_then(|v| v.as_u64()).map(|v| v as usize);
        let head_limit = input
            .get("head_limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let glob_pattern = input
            .get("glob")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let file_type = input
            .get("type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut options = GrepOptions::new(pattern, resolved_path)
            .case_insensitive(case_insensitive)
            .multiline(multiline)
            .output_mode(output_mode)
            .show_line_numbers(show_line_numbers);

        if let Some(c) = context_c {
            options = options.context(c);
        }
        if let Some(b) = before_context {
            options = options.before_context(b);
        }
        if let Some(a) = after_context {
            options = options.after_context(a);
        }
        if let Some(h) = head_limit {
            options = options.head_limit(h);
        }
        if let Some(g) = glob_pattern {
            options = options.glob(g);
        }
        if let Some(t) = file_type {
            options = options.file_type(t);
        }

        Ok(options)
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "Grep"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"A powerful search tool built on ripgrep

Usage:
- ALWAYS use Grep for search tasks. NEVER invoke `grep` or `rg` as a Bash command. The Grep tool has been optimized for correct permissions and access.
- Supports full regex syntax (e.g., "log.*Error", "function\s+\w+")
- Filter files with glob parameter (e.g., "*.js", "**/*.tsx") or type parameter (e.g., "js", "py", "rust")
- Output modes: "content" shows matching lines, "files_with_matches" shows only file paths (default), "count" shows match counts
- Use Task tool for open-ended searches requiring multiple rounds
- Pattern syntax: Uses ripgrep (not grep) - literal braces need escaping (use `interface\{\}` to find `interface{}` in Go code)
- Multiline matching: By default patterns match within single lines only. For cross-line patterns like `struct \{[\s\S]*?field`, use `multiline: true`"#.to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The regular expression pattern to search for in file contents"
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search in (rg PATH). Defaults to current working directory."
                },
                "glob": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g. \"*.js\", \"*.{ts,tsx}\") - maps to rg --glob"
                },
                "output_mode": {
                    "type": "string",
                    "enum": ["content", "files_with_matches", "count"],
                    "description": "Output mode: \"content\" shows matching lines (supports -A/-B/-C context, -n line numbers, head_limit), \"files_with_matches\" shows file paths (supports head_limit), \"count\" shows match counts (supports head_limit). Defaults to \"files_with_matches\"."
                },
                "-B": { "type": "number", "description": "Number of lines to show before each match (rg -B). Requires output_mode: \"content\", ignored otherwise." },
                "-A": { "type": "number", "description": "Number of lines to show after each match (rg -A). Requires output_mode: \"content\", ignored otherwise." },
                "-C": { "type": "number", "description": "Number of lines to show before and after each match (rg -C). Requires output_mode: \"content\", ignored otherwise." },
                "-n": { "type": "boolean", "description": "Show line numbers in output (rg -n). Requires output_mode: \"content\", ignored otherwise." },
                "-i": { "type": "boolean", "description": "Case insensitive search (rg -i)" },
                "type": { "type": "string", "description": "File type to search (rg --type). Common types: js, py, rust, go, java, etc." },
                "head_limit": { "type": "number", "description": "Limit output to first N lines/entries." },
                "multiline": { "type": "boolean", "description": "Enable multiline mode where . matches newlines and patterns can span lines (rg -U --multiline-dotall). Default: false." }
            },
            "required": ["pattern"],
            "additionalProperties": false,
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }
    
    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    fn render_tool_use_message(
        &self,
        input: &Value,
        _options: &crate::agentic::tools::framework::ToolRenderOptions,
    ) -> String {
        let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
        let search_path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let file_type = input.get("type").and_then(|v| v.as_str());
        let glob_pattern = input.get("glob").and_then(|v| v.as_str());
        let output_mode = input
            .get("output_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("files_with_matches");

        let scope = if search_path == "." {
            "Current workspace".to_string()
        } else {
            search_path.to_string()
        };
        let scope_with_filter = if let Some(ft) = file_type {
            format!("{} (*.{})", scope, ft)
        } else if let Some(gp) = glob_pattern {
            format!("{} ({})", scope, gp)
        } else {
            scope
        };
        let mode_desc = match output_mode {
            "content" => "Show matching content",
            "count" => "Count matches",
            _ => "List matching files",
        };

        format!(
            "Search \"{}\" | {} | {}",
            pattern, scope_with_filter, mode_desc
        )
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        // Remote workspace: use shell-based grep/rg
        if context.is_remote() {
            return self.call_remote(input, context).await;
        }

        let grep_options = self.build_grep_options(input, context)?;
        let pattern = grep_options.pattern.clone();
        let path = grep_options.path.clone();
        let output_mode = grep_options.output_mode.to_string();

        let event_system = crate::infrastructure::events::event_system::get_global_event_system();
        let tool_use_id = context
            .tool_call_id
            .clone()
            .unwrap_or_else(|| format!("grep_{}", uuid::Uuid::new_v4()));
        let tool_name = self.name().to_string();

        let tool_use_id_clone = tool_use_id.clone();
        let tool_name_clone = tool_name.clone();
        let event_system_clone = event_system.clone();
        let progress_callback: ProgressCallback = Arc::new(
            move |files_processed, file_count, total_matches| {
                let progress_message = format!(
                    "Scanned {} files | Found {} matching files ({} matches)",
                    files_processed, file_count, total_matches
                );

                let event = crate::infrastructure::events::event_system::BackendEvent::ToolExecutionProgress(
                    crate::util::types::event::ToolExecutionProgressInfo {
                        tool_use_id: tool_use_id_clone.clone(),
                        tool_name: tool_name_clone.clone(),
                        progress_message,
                        percentage: None,
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    }
                );

                let event_system = event_system_clone.clone();
                tokio::spawn(async move {
                    let _ = event_system.emit(event).await;
                });
            },
        );

        let search_result = tokio::task::spawn_blocking(move || {
            grep_search(grep_options, Some(progress_callback), Some(500))
        })
        .await;

        let (file_count, total_matches, result_text) = match search_result {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => return Err(BitFunError::tool(e)),
            Err(e) => return Err(BitFunError::tool(format!("grep search failed: {}", e))),
        };

        Ok(vec![ToolResult::Result {
            data: json!({
                "pattern": pattern,
                "path": path,
                "output_mode": output_mode,
                "file_count": file_count,
                "total_matches": total_matches,
                "result": result_text,
            }),
            result_for_assistant: Some(result_text),
            image_attachments: None,
        }])
    }
}
