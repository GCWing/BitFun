use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::agentic::tools::ToolPathOperation;
use crate::service::search::{
    get_global_workspace_search_service, remote_workspace_search_service_for_path,
    workspace_search_feature_enabled, workspace_search_runtime_available, ContentSearchOutputMode,
    ContentSearchRequest, WorkspaceSearchHit, WorkspaceSearchLine,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;
use tool_runtime::search::grep_search::{
    apply_offset_and_limit, build_remote_grep_command, count_remote_grep_matches, grep_search,
    relativize_result_text, render_remote_grep_result_text, GrepOptions, GrepSearchResult,
    OutputMode, ProgressCallback, RemoteGrepCommandRequest,
};

const DEFAULT_HEAD_LIMIT: usize = 250;

/// Prefixed to workspace-search output when the daemon's worktree view is behind.
///
/// No search path waits for the daemon to reconcile the worktree: on a large repository that wait is
/// seconds, and it would land on whichever query happened to come first. The staleness is stated
/// instead. That keeps the failure mode legible — a caller that just edited a file can reconcile the
/// difference itself, but only if it is told the view may predate the edit.
pub(crate) const WORKSPACE_PROBE_PENDING_NOTE: &str = "Note: the workspace index is still folding in recent worktree changes, so these results describe the repository as of a moment ago. Very recent edits may be missing; re-run the search if a match you expect is absent.";

/// Prepends [`WORKSPACE_PROBE_PENDING_NOTE`] to `body` when the daemon reported a pending probe.
pub(crate) fn annotate_workspace_probe_pending(
    body: String,
    workspace_probe_pending: bool,
) -> String {
    if !workspace_probe_pending {
        return body;
    }
    if body.is_empty() {
        return WORKSPACE_PROBE_PENDING_NOTE.to_string();
    }
    format!("{WORKSPACE_PROBE_PENDING_NOTE}\n\n{body}")
}

pub struct GrepTool;

impl Default for GrepTool {
    fn default() -> Self {
        Self::new()
    }
}

impl GrepTool {
    pub fn new() -> Self {
        Self
    }

    fn explicit_head_limit(input: &Value) -> Option<Option<usize>> {
        input
            .get("head_limit")
            .and_then(|v| v.as_u64())
            .map(|value| {
                if value == 0 {
                    None
                } else {
                    Some(value as usize)
                }
            })
    }

    fn resolve_head_limit(input: &Value) -> Option<usize> {
        Self::explicit_head_limit(input).unwrap_or(Some(DEFAULT_HEAD_LIMIT))
    }

    fn backend_max_results(
        input: &Value,
        offset: usize,
        _display_head_limit: Option<usize>,
    ) -> Option<usize> {
        Self::explicit_head_limit(input)
            .flatten()
            .map(|limit| limit.saturating_add(offset))
    }

    fn parse_glob_patterns(glob: Option<&str>) -> Vec<String> {
        let Some(glob) = glob else {
            return Vec::new();
        };

        let mut patterns = Vec::new();
        for raw_pattern in glob.split_whitespace() {
            if raw_pattern.contains('{') && raw_pattern.contains('}') {
                patterns.push(raw_pattern.to_string());
            } else {
                patterns.extend(
                    raw_pattern
                        .split(',')
                        .filter(|pattern| !pattern.is_empty())
                        .map(|pattern| pattern.to_string()),
                );
            }
        }
        patterns
    }

    fn resolve_offset(input: &Value) -> usize {
        input
            .get("offset")
            .and_then(|v| v.as_u64())
            .map(|value| value as usize)
            .unwrap_or(0)
    }

    fn display_base(context: &ToolUseContext) -> Option<String> {
        context
            .workspace
            .as_ref()
            .map(|workspace| workspace.root_path_string())
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
        let resolved = context.resolve_tool_path(search_path)?;
        let resolved_path = resolved.resolved_path.clone();

        let case_insensitive = input.get("-i").and_then(|v| v.as_bool()).unwrap_or(false);
        let head_limit = Self::resolve_head_limit(input);
        let offset = Self::resolve_offset(input);
        let output_mode = input
            .get("output_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("files_with_matches");
        let output_mode_enum =
            OutputMode::from_str(output_mode).map_err(|e| BitFunError::tool(e.to_string()))?;
        let show_line_numbers = input
            .get("-n")
            .and_then(|v| v.as_bool())
            .unwrap_or(output_mode == "content");
        let context_c = input
            .get("context")
            .or_else(|| input.get("-C"))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let before_context = input.get("-B").and_then(|v| v.as_u64()).map(|v| v as usize);
        let after_context = input.get("-A").and_then(|v| v.as_u64()).map(|v| v as usize);
        let glob_patterns = Self::parse_glob_patterns(input.get("glob").and_then(|v| v.as_str()));
        let file_type = input
            .get("type")
            .and_then(|v| v.as_str())
            .map(|value| value.to_string());

        let full_cmd = build_remote_grep_command(&RemoteGrepCommandRequest {
            pattern: pattern.to_string(),
            path: resolved_path,
            case_insensitive,
            output_mode: output_mode_enum,
            show_line_numbers,
            context: context_c,
            before_context,
            after_context,
            glob_patterns,
            file_type,
            head_limit,
            offset,
        });

        let (stdout, _stderr, _exit_code) = ws_shell
            .exec(&full_cmd, Some(30_000))
            .await
            .map_err(|e| BitFunError::tool(format!("Remote grep failed: {}", e)))?;

        let total_matches = count_remote_grep_matches(&stdout);
        let display_base = Self::display_base(context);
        let result_text = render_remote_grep_result_text(&stdout, pattern, display_base.as_deref());

        Ok(vec![ToolResult::Result {
            data: json!({
                "pattern": pattern,
                "path": resolved.logical_path,
                "output_mode": output_mode,
                "total_matches": total_matches,
                "applied_limit": head_limit,
                "applied_offset": if offset > 0 { Some(offset) } else { None::<usize> },
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
        let resolved = context.resolve_tool_path(search_path)?;
        let resolved_path = resolved.resolved_path.clone();

        let case_insensitive = input.get("-i").and_then(|v| v.as_bool()).unwrap_or(false);
        let multiline = input
            .get("multiline")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let output_mode_str = input
            .get("output_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("files_with_matches");
        let output_mode =
            OutputMode::from_str(output_mode_str).map_err(|e| BitFunError::tool(e.to_string()))?;
        let show_line_numbers = input
            .get("-n")
            .and_then(|v| v.as_bool())
            .unwrap_or(output_mode_str == "content");
        let context_c = input
            .get("context")
            .or_else(|| input.get("-C"))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let before_context = input.get("-B").and_then(|v| v.as_u64()).map(|v| v as usize);
        let after_context = input.get("-A").and_then(|v| v.as_u64()).map(|v| v as usize);
        let head_limit = Self::resolve_head_limit(input);
        let offset = Self::resolve_offset(input);
        let glob_patterns = Self::parse_glob_patterns(input.get("glob").and_then(|v| v.as_str()));
        let file_type = input
            .get("type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut options = GrepOptions::new(pattern, resolved_path)
            .case_insensitive(case_insensitive)
            .multiline(multiline)
            .output_mode(output_mode)
            .show_line_numbers(show_line_numbers);

        if resolved.is_runtime_artifact() {
            if let Some(runtime_root) = &resolved.runtime_root {
                options = options.display_base(runtime_root.to_string_lossy().to_string());
            }
        } else if let Some(display_base) = Self::display_base(context) {
            options = options.display_base(display_base);
        }

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
        if offset > 0 {
            options = options.offset(offset);
        }
        if !glob_patterns.is_empty() {
            options = options.globs(glob_patterns);
        }
        if let Some(t) = file_type {
            options = options.file_type(t);
        }

        Ok(options)
    }

    /// Whether the caller asked for surrounding context lines (`-A` / `-B` / `-C` / `context`).
    ///
    /// The flashgrep daemon protocol has no context-line concept, so these requests must be
    /// served by the ripgrep path instead of workspace search.
    fn context_lines_requested(input: &Value) -> bool {
        ["-A", "-B", "-C", "context"]
            .iter()
            .filter_map(|key| input.get(*key))
            .filter_map(|value| value.as_u64())
            .any(|lines| lines > 0)
    }

    fn build_workspace_search_request(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<(ContentSearchRequest, String, bool, usize, Option<usize>)> {
        let workspace_root = context
            .workspace
            .as_ref()
            .map(|workspace| PathBuf::from(workspace.root_path_string()))
            .ok_or_else(|| BitFunError::tool("Workspace is required for Grep".to_string()))?;

        let pattern = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("pattern is required".to_string()))?;
        let search_path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let resolved_path = context.resolve_workspace_tool_path(search_path)?;
        let resolved_path_buf = PathBuf::from(&resolved_path);
        let output_mode = input
            .get("output_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("files_with_matches")
            .to_string();
        let show_line_numbers = input
            .get("-n")
            .and_then(|v| v.as_bool())
            .unwrap_or(output_mode == "content");
        let offset = Self::resolve_offset(input);
        let head_limit = Self::resolve_head_limit(input);
        let max_results = Self::backend_max_results(input, offset, head_limit);
        let globs = Self::parse_glob_patterns(input.get("glob").and_then(|v| v.as_str()));
        let file_types = input
            .get("type")
            .and_then(|v| v.as_str())
            .map(|value| vec![value.to_string()])
            .unwrap_or_default();
        let output_mode_enum = match output_mode.as_str() {
            "content" => ContentSearchOutputMode::Content,
            "count" => ContentSearchOutputMode::Count,
            _ => ContentSearchOutputMode::FilesWithMatches,
        };
        let request = ContentSearchRequest {
            repo_root: workspace_root.clone(),
            search_path: (resolved_path_buf != workspace_root).then_some(resolved_path_buf),
            pattern: pattern.to_string(),
            output_mode: output_mode_enum,
            case_sensitive: !input.get("-i").and_then(|v| v.as_bool()).unwrap_or(false),
            use_regex: true,
            whole_word: false,
            multiline: input
                .get("multiline")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            max_results,
            globs,
            file_types,
            exclude_file_types: Vec::new(),
        };

        Ok((request, output_mode, show_line_numbers, offset, head_limit))
    }

    fn format_workspace_search_output(
        &self,
        output_mode: &str,
        show_line_numbers: bool,
        offset: usize,
        head_limit: Option<usize>,
        result: &crate::service::search::ContentSearchResult,
        display_base: Option<&str>,
    ) -> (String, usize, usize) {
        match output_mode {
            "content" => {
                let mut lines =
                    render_workspace_search_content_lines(&result.hits, show_line_numbers);
                if lines.is_empty() {
                    lines = render_workspace_search_result_lines(
                        &result.outcome.results,
                        show_line_numbers,
                    );
                }
                apply_offset_and_limit(&mut lines, offset, head_limit);
                let rendered = relativize_result_text(&lines.join("\n"), display_base);
                let file_count = if result.hits.is_empty() {
                    result
                        .outcome
                        .results
                        .iter()
                        .map(|item| item.path.as_str())
                        .collect::<HashSet<_>>()
                        .len()
                } else {
                    result
                        .hits
                        .iter()
                        .map(|hit| hit.path.as_str())
                        .collect::<HashSet<_>>()
                        .len()
                };
                (rendered, file_count, result.matched_occurrences)
            }
            "count" => {
                let mut lines = result
                    .file_counts
                    .iter()
                    .map(|count| format!("{}:{}", count.path, count.matched_lines))
                    .collect::<Vec<_>>();
                lines.sort();
                let mut lines = lines.into_iter().collect::<Vec<_>>();
                apply_offset_and_limit(&mut lines, offset, head_limit);
                let rendered = relativize_result_text(&lines.join("\n"), display_base);
                (rendered, result.file_counts.len(), result.matched_lines)
            }
            _ => {
                let mut files = result
                    .outcome
                    .results
                    .iter()
                    .map(|item| item.path.clone())
                    .collect::<Vec<_>>();
                files.sort();
                files.dedup();
                apply_offset_and_limit(&mut files, offset, head_limit);
                let rendered = relativize_result_text(&files.join("\n"), display_base);
                let total_matches = files.len();
                (rendered, total_matches, total_matches)
            }
        }
    }
}

/// Renders one line per content match.
///
/// Matches hydrated from disk carry their text. Transports that cannot read the
/// matched files (remote SSH) surface positions only, because the flashgrep
/// daemon never sends line text on the wire; those render as a bare
/// `path:line:` locator rather than being dropped, so the caller still learns
/// where the matches are. A match with neither text nor a line number carries no
/// usable information and is skipped.
fn render_workspace_search_result_lines(
    results: &[crate::infrastructure::FileSearchResult],
    show_line_numbers: bool,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::with_capacity(results.len());

    for result in results {
        let content = result
            .matched_content
            .as_deref()
            .map(str::trim_end)
            .filter(|content| !content.is_empty());

        let rendered = match (content, result.line_number) {
            (Some(content), Some(line)) if show_line_numbers => {
                format!("{}:{}:{}", result.path, line, content)
            }
            (Some(content), _) => format!("{}:{}", result.path, content),
            (None, Some(line)) if show_line_numbers => format!("{}:{}:", result.path, line),
            // Without line numbers a text-less match collapses to its path, so
            // avoid repeating the same path once per match in the same file.
            (None, Some(_)) => result.path.clone(),
            (None, None) => continue,
        };

        if lines.last().is_some_and(|last| last == &rendered) {
            continue;
        }
        lines.push(rendered);
    }

    lines
}

fn render_workspace_search_content_lines(
    hits: &[WorkspaceSearchHit],
    show_line_numbers: bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    for hit in hits {
        for line in &hit.lines {
            match line {
                WorkspaceSearchLine::Match { value } => {
                    let snippet = value.snippet.trim_end();
                    if show_line_numbers {
                        lines.push(format!("{}:{}:{}", hit.path, value.location.line, snippet));
                    } else {
                        lines.push(format!("{}:{}", hit.path, snippet));
                    }
                }
                WorkspaceSearchLine::Context { value } => {
                    let snippet = value.snippet.trim_end();
                    if show_line_numbers {
                        lines.push(format!("{}-{}:{}", hit.path, value.line_number, snippet));
                    } else {
                        lines.push(format!("{}-{}", hit.path, snippet));
                    }
                }
                WorkspaceSearchLine::ContextBreak => lines.push("--".to_string()),
            }
        }
    }
    lines
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "Grep"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"A powerful search tool built on ripgrep

Usage:
- Use Grep by default for codebase content search because it preserves workspace-aware permissions and consistent output. Shell out to `grep` or `rg` only when this tool cannot meet the requirement, and prefer explaining why when doing so.
- For simple literal names or symbols, start with the literal text before trying broad regexes.
- Narrow searches with `path`, `glob`, or `type` when you know the likely area or language, and use `head_limit` to keep exploratory searches readable.
- A common workflow is `output_mode: "files_with_matches"` to locate candidate files, followed by `output_mode: "content"` with `-n` and small context when exact lines are needed.
- Supports full regex syntax (e.g., "log.*Error", "function\s+\w+")
- Filter files with glob parameter (e.g., "*.js", "**/*.tsx") or type parameter (e.g., "js", "py", "rust")
- The path parameter may be workspace-relative, an absolute path inside the current workspace, or an exact `bitfun://...` URI returned by another tool
- Omit path to search the current workspace. Do not search host roots or placeholder paths such as `/workspace`.
- Output modes: "content" shows matching lines, "files_with_matches" shows only file paths (default), "count" shows match counts
- Use Task tool for open-ended searches requiring multiple rounds
- Pattern syntax: Uses ripgrep (not grep) - literal braces need escaping (use `interface\{\}` to find `interface{}` in Go code)
- Multiline matching: By default patterns match within single lines only. For cross-line patterns like `struct \{[\s\S]*?field`, use `multiline: true`"#.to_string())
    }

    fn short_description(&self) -> String {
        "Search file contents with ripgrep-powered pattern matching.".to_string()
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
                    "description": "File or directory to search in. Omit to search the current workspace. If provided, use a workspace-relative path, an absolute path inside the current workspace, or an exact bitfun:// URI."
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
                "context": { "type": "number", "description": "Alias for -C. Number of lines to show before and after each match." },
                "-n": { "type": "boolean", "description": "Show line numbers in output (rg -n). Requires output_mode: \"content\", ignored otherwise." },
                "-i": { "type": "boolean", "description": "Case insensitive search (rg -i)" },
                "type": { "type": "string", "description": "File type to search (rg --type). Common types: js, py, rust, go, java, etc." },
                "head_limit": { "type": "number", "description": "Limit output to first N lines/entries." },
                "offset": { "type": "number", "description": "Skip the first N lines/entries before applying head_limit." },
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
        let search_path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let resolved = context.resolve_tool_path(search_path)?;
        context.enforce_path_operation(ToolPathOperation::Read, &resolved)?;
        crate::agentic::deep_review::scope::ensure_focused_review_resolved_path_allowed(
            context,
            &resolved.resolved_path,
        )?;
        let focused_excluded_paths =
            crate::agentic::deep_review::scope::focused_review_excluded_changed_paths(context)?;

        // The flashgrep daemon has no context-line support, so `-A`/`-B`/`-C` must go
        // through the ripgrep path to produce surrounding lines.
        let context_lines_requested = Self::context_lines_requested(input);

        if resolved.uses_remote_workspace_backend() {
            if !context_lines_requested && workspace_search_feature_enabled().await {
                let remote_workspace_search_result = async {
                    let (request, output_mode, show_line_numbers, offset, head_limit) =
                        self.build_workspace_search_request(input, context)?;
                    let pattern = request.pattern.clone();
                    let path = request
                        .search_path
                        .as_ref()
                        .map(|path| path.to_string_lossy().to_string())
                        .unwrap_or_else(|| request.repo_root.to_string_lossy().to_string());
                    let repo_root = request.repo_root.to_string_lossy().to_string();
                    let preferred_connection_id = context
                        .workspace
                        .as_ref()
                        .and_then(|workspace| workspace.connection_id())
                        .map(str::to_string);
                    let search_service =
                        remote_workspace_search_service_for_path(&repo_root, preferred_connection_id)
                            .await
                            .map_err(BitFunError::tool)?;
                    let search_started_at = Instant::now();
                    let search_result = search_service
                        .search_content(request)
                        .await
                        .map_err(BitFunError::tool)?;
                    let display_base = Self::display_base(context);
                    let (result_text, file_count, total_matches) =
                        self.format_workspace_search_output(
                            &output_mode,
                            show_line_numbers,
                            offset,
                            head_limit,
                            &search_result,
                            display_base.as_deref(),
                        );
                    let workspace_search_elapsed_ms = search_started_at.elapsed().as_millis();

                    log::info!(
                        "Grep tool remote workspace-search result: pattern={}, path={}, output_mode={}, file_count={}, total_matches={}, backend={:?}, repo_phase={:?}, base_advance_in_progress={}, dirty_modified={}, dirty_deleted={}, dirty_new={}, candidate_docs={}, matched_lines={}, matched_occurrences={}, workspace_search_ms={}",
                        pattern,
                        path,
                        output_mode,
                        file_count,
                        total_matches,
                        search_result.backend,
                        search_result.repo_status.phase,
                        search_result.repo_status.base_advance_in_progress,
                        search_result.repo_status.dirty_files.modified,
                        search_result.repo_status.dirty_files.deleted,
                        search_result.repo_status.dirty_files.new,
                        search_result.candidate_docs,
                        search_result.matched_lines,
                        search_result.matched_occurrences,
                        workspace_search_elapsed_ms,
                    );

                    Ok::<Vec<ToolResult>, BitFunError>(vec![ToolResult::Result {
                        data: json!({
                            "pattern": pattern,
                            "path": path,
                            "output_mode": output_mode,
                            "file_count": file_count,
                            "total_matches": total_matches,
                            "backend": search_result.backend,
                            "repo_phase": search_result.repo_status.phase,
                            "base_advance_in_progress": search_result.repo_status.base_advance_in_progress,
                            "workspace_probe_pending": search_result.repo_status.workspace_probe_pending,
                            "applied_limit": head_limit,
                            "applied_offset": if offset > 0 { Some(offset) } else { None::<usize> },
                            "result": result_text,
                        }),
                        result_for_assistant: Some(annotate_workspace_probe_pending(
                            result_text,
                            search_result.repo_status.workspace_probe_pending,
                        )),
                        image_attachments: None,
                    }])
                }
                .await;

                match remote_workspace_search_result {
                    Ok(results) => return Ok(results),
                    Err(error) => {
                        log::warn!(
                            "Grep tool remote workspace-search failed; falling back to shell grep: {}",
                            error
                        );
                    }
                }
            }
            return self.call_remote(input, context).await;
        }

        if focused_excluded_paths.is_none()
            && !context_lines_requested
            && workspace_search_runtime_available().await
        {
            if let Some(search_service) = get_global_workspace_search_service() {
                let (request, output_mode, show_line_numbers, offset, head_limit) =
                    self.build_workspace_search_request(input, context)?;
                let pattern = request.pattern.clone();
                let path = request
                    .search_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().to_string())
                    .unwrap_or_else(|| request.repo_root.to_string_lossy().to_string());
                let search_started_at = Instant::now();
                match search_service.search_content(request).await {
                    Ok(search_result) => {
                        let display_base = Self::display_base(context);
                        let (result_text, file_count, total_matches) = self
                            .format_workspace_search_output(
                                &output_mode,
                                show_line_numbers,
                                offset,
                                head_limit,
                                &search_result,
                                display_base.as_deref(),
                            );
                        let workspace_search_elapsed_ms = search_started_at.elapsed().as_millis();

                        log::info!(
                            "Grep tool workspace-search result: pattern={}, path={}, output_mode={}, file_count={}, total_matches={}, backend={:?}, repo_phase={:?}, base_advance_in_progress={}, dirty_modified={}, dirty_deleted={}, dirty_new={}, candidate_docs={}, matched_lines={}, matched_occurrences={}, workspace_search_ms={}",
                            pattern,
                            path,
                            output_mode,
                            file_count,
                            total_matches,
                            search_result.backend,
                            search_result.repo_status.phase,
                            search_result.repo_status.base_advance_in_progress,
                            search_result.repo_status.dirty_files.modified,
                            search_result.repo_status.dirty_files.deleted,
                            search_result.repo_status.dirty_files.new,
                            search_result.candidate_docs,
                            search_result.matched_lines,
                            search_result.matched_occurrences,
                            workspace_search_elapsed_ms,
                        );

                        return Ok(vec![ToolResult::Result {
                            data: json!({
                                "pattern": pattern,
                                "path": path,
                                "output_mode": output_mode,
                                "file_count": file_count,
                                "total_matches": total_matches,
                                "backend": search_result.backend,
                                "repo_phase": search_result.repo_status.phase,
                                "base_advance_in_progress": search_result.repo_status.base_advance_in_progress,
                                "workspace_probe_pending": search_result.repo_status.workspace_probe_pending,
                                "applied_limit": head_limit,
                                "applied_offset": if offset > 0 { Some(offset) } else { None::<usize> },
                                "result": result_text,
                            }),
                            result_for_assistant: Some(annotate_workspace_probe_pending(
                                result_text,
                                search_result.repo_status.workspace_probe_pending,
                            )),
                            image_attachments: None,
                        }]);
                    }
                    Err(error) => {
                        log::warn!(
                            "Grep tool workspace-search failed; falling back to local rg: {}",
                            error
                        );
                    }
                }
            }
        }

        let mut grep_options = self.build_grep_options(input, context)?;
        if let Some(excluded_paths) = focused_excluded_paths {
            grep_options = grep_options
                .excluded_paths(
                    excluded_paths
                        .into_iter()
                        .map(|path| path.to_string_lossy().into_owned())
                        .collect(),
                )
                .reject_linked_files(true);
        }
        let pattern = grep_options.pattern.clone();
        let path = resolved.logical_path.clone();
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

        let GrepSearchResult {
            file_count,
            total_matches,
            result_text,
            applied_limit,
            applied_offset,
            // Always false here: this call site supplies no cancellation token, so the search has
            // no way to stop early.
            cancelled: _,
        } = match search_result {
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
                "applied_limit": applied_limit,
                "applied_offset": applied_offset,
                "result": result_text,
            }),
            result_for_assistant: Some(result_text),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::{
        annotate_workspace_probe_pending, render_workspace_search_content_lines,
        render_workspace_search_result_lines, GrepTool, DEFAULT_HEAD_LIMIT,
        WORKSPACE_PROBE_PENDING_NOTE,
    };
    use crate::agentic::tools::framework::{Tool, ToolUseContext};
    use crate::agentic::tools::{ToolPathPolicy, ToolRuntimeRestrictions};
    use crate::agentic::WorkspaceBinding;
    use crate::infrastructure::{FileSearchOutcome, FileSearchResult, SearchMatchType};
    use crate::service::search::{
        ContentSearchResult, WorkspaceSearchBackend, WorkspaceSearchHit, WorkspaceSearchLine,
        WorkspaceSearchMatch, WorkspaceSearchMatchLocation, WorkspaceSearchRepoPhase,
        WorkspaceSearchRepoStatus,
    };
    use bitfun_runtime_ports::ToolRuntimeHandles;
    use serde_json::json;
    use std::collections::HashMap;
    use tool_runtime::search::grep_search::relativize_result_text;

    #[tokio::test]
    async fn grep_tool_enforces_runtime_read_roots() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scope = "0123456789abcdef0123456789abcdef";
        let allowed_root = dir.path().join(".miniapp-context").join(scope);
        std::fs::create_dir_all(&allowed_root).expect("create context root");
        std::fs::write(allowed_root.join("stocks.ndjson"), "allowed market row")
            .expect("write allowed file");
        std::fs::write(dir.path().join("storage.json"), "blocked").expect("write blocked file");
        let context = ToolUseContext {
            tool_call_id: None,
            agent_type: Some("Agent".to_string()),
            session_id: None,
            dialog_turn_id: Some("turn-1".to_string()),
            workspace: Some(WorkspaceBinding::new(
                Some("grep-context-workspace".to_string()),
                dir.path().to_path_buf(),
            )),
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions {
                path_policy: ToolPathPolicy {
                    read_roots: vec![format!(".miniapp-context/{scope}")],
                    reject_symlinked_read_roots: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            runtime_handles: ToolRuntimeHandles::default(),
        };

        GrepTool::new()
            .call_impl(
                &json!({
                    "pattern": "allowed market row",
                    "path": format!(".miniapp-context/{scope}")
                }),
                &context,
            )
            .await
            .expect("Grep should search the exact context snapshot root");
        let error = GrepTool::new()
            .call_impl(
                &json!({ "pattern": "blocked", "path": "storage.json" }),
                &context,
            )
            .await
            .expect_err("Grep must not search app storage outside reserved context");
        assert!(error.to_string().contains("is not allowed for read"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn grep_tool_rejects_symlinked_context_snapshot_roots() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scope = "0123456789abcdef0123456789abcdef";
        let outside = dir.path().join("outside");
        let context_parent = dir.path().join(".miniapp-context");
        std::fs::create_dir_all(&outside).expect("create outside root");
        std::fs::create_dir_all(&context_parent).expect("create context parent");
        std::fs::write(outside.join("stocks.ndjson"), "escaped market row")
            .expect("write outside file");
        std::os::unix::fs::symlink(&outside, context_parent.join(scope))
            .expect("create context symlink");

        let context = ToolUseContext {
            tool_call_id: None,
            agent_type: Some("Agent".to_string()),
            session_id: None,
            dialog_turn_id: Some("turn-1".to_string()),
            workspace: Some(WorkspaceBinding::new(
                Some("grep-context-workspace".to_string()),
                dir.path().to_path_buf(),
            )),
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions {
                path_policy: ToolPathPolicy {
                    read_roots: vec![format!(".miniapp-context/{scope}")],
                    reject_symlinked_read_roots: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            runtime_handles: ToolRuntimeHandles::default(),
        };

        let error = GrepTool::new()
            .call_impl(
                &json!({
                    "pattern": "escaped market row",
                    "path": format!(".miniapp-context/{scope}")
                }),
                &context,
            )
            .await
            .expect_err("Grep must reject a symlinked context snapshot root");
        assert!(error.to_string().contains("contains a symlink"));
    }

    #[test]
    fn head_limit_defaults_and_zero_escape_hatch() {
        assert_eq!(
            GrepTool::resolve_head_limit(&json!({})),
            Some(DEFAULT_HEAD_LIMIT)
        );
        assert_eq!(
            GrepTool::resolve_head_limit(&json!({ "head_limit": 25 })),
            Some(25)
        );
        assert_eq!(
            GrepTool::resolve_head_limit(&json!({ "head_limit": 0 })),
            None
        );
    }

    #[test]
    fn context_lines_requested_detects_every_context_flag() {
        assert!(!GrepTool::context_lines_requested(&json!({})));
        assert!(!GrepTool::context_lines_requested(
            &json!({ "pattern": "foo", "-A": 0, "-B": 0, "-C": 0 })
        ));

        for key in ["-A", "-B", "-C", "context"] {
            assert!(
                GrepTool::context_lines_requested(&json!({ "pattern": "foo", key: 2 })),
                "expected {key} to route the request to ripgrep"
            );
        }
    }

    #[test]
    fn backend_max_results_only_uses_explicit_limit() {
        assert_eq!(
            GrepTool::backend_max_results(&json!({}), 0, Some(DEFAULT_HEAD_LIMIT)),
            None
        );
        assert_eq!(
            GrepTool::backend_max_results(&json!({ "head_limit": 25 }), 3, Some(25)),
            Some(28)
        );
        assert_eq!(
            GrepTool::backend_max_results(&json!({ "head_limit": 0 }), 7, None),
            None
        );
    }

    #[test]
    fn relativizes_prefixed_result_lines() {
        let text = "/repo/src/main.rs:12:fn main()\n/repo/src/lib.rs:3:pub fn lib()";
        let relativized = relativize_result_text(text, Some("/repo"));

        assert_eq!(
            relativized,
            "src/main.rs:12:fn main()\nsrc/lib.rs:3:pub fn lib()"
        );
    }

    #[test]
    fn renders_workspace_search_context_lines_in_rg_style() {
        let lines = render_workspace_search_content_lines(
            &[WorkspaceSearchHit {
                path: "/repo/src/main.rs".to_string(),
                matches: vec![WorkspaceSearchMatch {
                    location: WorkspaceSearchMatchLocation {
                        line: 12,
                        column: 5,
                    },
                    snippet: "panic!(\"x\")".to_string(),
                    matched_text: "panic".to_string(),
                }],
                lines: vec![
                    WorkspaceSearchLine::Context {
                        value: crate::service::search::WorkspaceSearchContextLine {
                            line_number: 10,
                            snippet: "let a = 1".to_string(),
                        },
                    },
                    WorkspaceSearchLine::Context {
                        value: crate::service::search::WorkspaceSearchContextLine {
                            line_number: 11,
                            snippet: "let b = 2".to_string(),
                        },
                    },
                    WorkspaceSearchLine::Match {
                        value: WorkspaceSearchMatch {
                            location: WorkspaceSearchMatchLocation {
                                line: 12,
                                column: 5,
                            },
                            snippet: "panic!(\"x\")".to_string(),
                            matched_text: "panic".to_string(),
                        },
                    },
                    WorkspaceSearchLine::Context {
                        value: crate::service::search::WorkspaceSearchContextLine {
                            line_number: 13,
                            snippet: "cleanup()".to_string(),
                        },
                    },
                    WorkspaceSearchLine::ContextBreak,
                    WorkspaceSearchLine::Context {
                        value: crate::service::search::WorkspaceSearchContextLine {
                            line_number: 20,
                            snippet: "return".to_string(),
                        },
                    },
                ],
            }],
            true,
        );

        assert_eq!(
            lines,
            vec![
                "/repo/src/main.rs-10:let a = 1",
                "/repo/src/main.rs-11:let b = 2",
                "/repo/src/main.rs:12:panic!(\"x\")",
                "/repo/src/main.rs-13:cleanup()",
                "--",
                "/repo/src/main.rs-20:return",
            ]
        );
    }

    #[test]
    fn content_workspace_output_uses_hits_for_context_lines() {
        let tool = GrepTool::new();
        let result = ContentSearchResult {
            outcome: FileSearchOutcome {
                results: Vec::new(),
                truncated: false,
            },
            file_counts: Vec::new(),
            hits: vec![WorkspaceSearchHit {
                path: "/repo/src/main.rs".to_string(),
                matches: vec![WorkspaceSearchMatch {
                    location: WorkspaceSearchMatchLocation {
                        line: 12,
                        column: 5,
                    },
                    snippet: "panic!(\"x\")".to_string(),
                    matched_text: "panic".to_string(),
                }],
                lines: vec![
                    WorkspaceSearchLine::Context {
                        value: crate::service::search::WorkspaceSearchContextLine {
                            line_number: 11,
                            snippet: "let b = 2".to_string(),
                        },
                    },
                    WorkspaceSearchLine::Match {
                        value: WorkspaceSearchMatch {
                            location: WorkspaceSearchMatchLocation {
                                line: 12,
                                column: 5,
                            },
                            snippet: "panic!(\"x\")".to_string(),
                            matched_text: "panic".to_string(),
                        },
                    },
                ],
            }],
            backend: WorkspaceSearchBackend::Indexed,
            repo_status: WorkspaceSearchRepoStatus {
                repo_id: "repo".to_string(),
                repo_path: "/repo".to_string(),
                storage_root: "/repo/.bitfun/search/flashgrep-index".to_string(),
                base_snapshot_root: "/repo/.bitfun/search/flashgrep-index/base-snapshot"
                    .to_string(),
                workspace_overlay_root: "/repo/.bitfun/search/flashgrep-index/workspace-overlay"
                    .to_string(),
                phase: WorkspaceSearchRepoPhase::Ready,
                snapshot_key: None,
                base_head_commit: None,
                workspace_head_commit: None,
                base_advance_in_progress: false,
                base_advance_target_head: None,
                base_delta_depth: 0,
                base_compaction_recommended: false,
                last_probe_unix_secs: None,
                last_rebuild_unix_secs: None,
                dirty_files: crate::service::search::WorkspaceSearchDirtyFiles {
                    modified: 0,
                    deleted: 0,
                    new: 0,
                },
                active_task_id: None,
                probe_healthy: true,
                workspace_probe_pending: false,
                last_error: None,
                last_maintenance_error: None,
                overlay: None,
            },
            candidate_docs: 1,
            matched_lines: 1,
            matched_occurrences: 1,
        };

        let (rendered, file_count, total_matches) =
            tool.format_workspace_search_output("content", true, 0, None, &result, Some("/repo"));

        assert_eq!(
            rendered,
            "src/main.rs-11:let b = 2\nsrc/main.rs:12:panic!(\"x\")"
        );
        assert_eq!(file_count, 1);
        assert_eq!(total_matches, 1);
    }

    #[test]
    fn content_workspace_output_falls_back_to_converted_line_results() {
        let tool = GrepTool::new();
        let result = ContentSearchResult {
            outcome: FileSearchOutcome {
                results: vec![
                    FileSearchResult {
                        path: "/repo/src/main.rs".to_string(),
                        name: "main.rs".to_string(),
                        is_directory: false,
                        match_type: SearchMatchType::Content,
                        line_number: Some(12),
                        matched_content: Some("panic!(\"x\")".to_string()),
                        preview_before: None,
                        preview_inside: Some("panic!(\"x\")".to_string()),
                        preview_after: None,
                    },
                    FileSearchResult {
                        path: "/repo/src/lib.rs".to_string(),
                        name: "lib.rs".to_string(),
                        is_directory: false,
                        match_type: SearchMatchType::Content,
                        line_number: Some(3),
                        matched_content: Some("pub fn lib() {}".to_string()),
                        preview_before: None,
                        preview_inside: Some("pub fn lib() {}".to_string()),
                        preview_after: None,
                    },
                ],
                truncated: false,
            },
            file_counts: Vec::new(),
            hits: Vec::new(),
            backend: WorkspaceSearchBackend::Indexed,
            repo_status: WorkspaceSearchRepoStatus {
                repo_id: "repo".to_string(),
                repo_path: "/repo".to_string(),
                storage_root: "/repo/.bitfun/search/flashgrep-index".to_string(),
                base_snapshot_root: "/repo/.bitfun/search/flashgrep-index/base-snapshot"
                    .to_string(),
                workspace_overlay_root: "/repo/.bitfun/search/flashgrep-index/workspace-overlay"
                    .to_string(),
                phase: WorkspaceSearchRepoPhase::Ready,
                snapshot_key: None,
                base_head_commit: None,
                workspace_head_commit: None,
                base_advance_in_progress: false,
                base_advance_target_head: None,
                base_delta_depth: 0,
                base_compaction_recommended: false,
                last_probe_unix_secs: None,
                last_rebuild_unix_secs: None,
                dirty_files: crate::service::search::WorkspaceSearchDirtyFiles {
                    modified: 0,
                    deleted: 0,
                    new: 0,
                },
                active_task_id: None,
                probe_healthy: true,
                workspace_probe_pending: false,
                last_error: None,
                last_maintenance_error: None,
                overlay: None,
            },
            candidate_docs: 2,
            matched_lines: 2,
            matched_occurrences: 2,
        };

        let (rendered, file_count, total_matches) =
            tool.format_workspace_search_output("content", true, 0, None, &result, Some("/repo"));

        assert_eq!(
            rendered,
            "src/main.rs:12:panic!(\"x\")\nsrc/lib.rs:3:pub fn lib() {}"
        );
        assert_eq!(file_count, 2);
        assert_eq!(total_matches, 2);
    }

    #[test]
    fn renders_workspace_search_result_lines_without_line_numbers() {
        let lines = render_workspace_search_result_lines(
            &[FileSearchResult {
                path: "/repo/src/main.rs".to_string(),
                name: "main.rs".to_string(),
                is_directory: false,
                match_type: SearchMatchType::Content,
                line_number: Some(12),
                matched_content: Some("panic!(\"x\")".to_string()),
                preview_before: None,
                preview_inside: Some("panic!(\"x\")".to_string()),
                preview_after: None,
            }],
            false,
        );

        assert_eq!(lines, vec!["/repo/src/main.rs:panic!(\"x\")"]);
    }

    #[test]
    fn renders_locators_for_matches_without_line_text() {
        let positions_only = |line: usize| FileSearchResult {
            path: "/repo/src/main.rs".to_string(),
            name: "main.rs".to_string(),
            is_directory: false,
            match_type: SearchMatchType::Content,
            line_number: Some(line),
            matched_content: None,
            preview_before: None,
            preview_inside: None,
            preview_after: None,
        };

        let lines =
            render_workspace_search_result_lines(&[positions_only(12), positions_only(73)], true);
        assert_eq!(
            lines,
            vec!["/repo/src/main.rs:12:", "/repo/src/main.rs:73:"]
        );

        // Without line numbers there is nothing left but the path, so repeated
        // matches in one file collapse to a single line.
        let lines =
            render_workspace_search_result_lines(&[positions_only(12), positions_only(73)], false);
        assert_eq!(lines, vec!["/repo/src/main.rs"]);
    }

    #[test]
    fn skips_matches_without_text_or_line_number() {
        let lines = render_workspace_search_result_lines(
            &[FileSearchResult {
                path: "/repo/src/main.rs".to_string(),
                name: "main.rs".to_string(),
                is_directory: false,
                match_type: SearchMatchType::Content,
                line_number: None,
                matched_content: None,
                preview_before: None,
                preview_inside: None,
                preview_after: None,
            }],
            true,
        );

        assert!(lines.is_empty());
    }

    #[test]
    fn stale_workspace_view_is_stated_in_the_output_the_model_reads() {
        // No search path waits for the daemon to reconcile, so the only thing that keeps a stale
        // result from silently misleading the caller is saying so in the text it reads.
        let annotated = annotate_workspace_probe_pending("src/lib.rs:1:hit".to_string(), true);
        assert!(annotated.starts_with(WORKSPACE_PROBE_PENDING_NOTE));
        assert!(annotated.ends_with("src/lib.rs:1:hit"));
    }

    #[test]
    fn a_current_workspace_view_adds_nothing_to_the_output() {
        // The pending case is the exception; the common case must stay byte-identical so the note
        // never becomes background noise the model learns to skip.
        let body = "src/lib.rs:1:hit".to_string();
        assert_eq!(annotate_workspace_probe_pending(body.clone(), false), body);
    }

    #[test]
    fn a_stale_empty_result_still_says_why_it_may_be_empty() {
        // "No matches" plus a stale index is exactly the case where the caller needs the note most.
        assert_eq!(
            annotate_workspace_probe_pending(String::new(), true),
            WORKSPACE_PROBE_PENDING_NOTE
        );
    }
}
