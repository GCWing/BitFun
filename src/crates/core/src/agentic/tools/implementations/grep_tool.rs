use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::service::search::{
    get_global_workspace_search_service, ContentSearchOutputMode, ContentSearchRequest,
    WorkspaceSearchHit, WorkspaceSearchLine,
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
    grep_search, GrepOptions, GrepSearchResult, OutputMode, ProgressCallback,
};

const DEFAULT_HEAD_LIMIT: usize = 250;

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

    fn shell_escape(value: &str) -> String {
        value.replace('\'', "'\\''")
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

    fn relativize_result_text(result_text: &str, display_base: Option<&str>) -> String {
        let Some(base) = display_base else {
            return result_text.to_string();
        };

        let normalized_base = base.replace('\\', "/").trim_end_matches('/').to_string();
        if normalized_base.is_empty() {
            return result_text.to_string();
        }

        result_text
            .lines()
            .map(|line| {
                if let Some(rest) = line.strip_prefix(&(normalized_base.clone() + "/")) {
                    rest.to_string()
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
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
        let show_line_numbers = input
            .get("-n")
            .and_then(|v| v.as_bool())
            .unwrap_or(output_mode == "content");
        let context_c = input
            .get("context")
            .or_else(|| input.get("-C"))
            .and_then(|v| v.as_u64())
            .map(|v| v.to_string());
        let before_context = input
            .get("-B")
            .and_then(|v| v.as_u64())
            .map(|v| v.to_string());
        let after_context = input
            .get("-A")
            .and_then(|v| v.as_u64())
            .map(|v| v.to_string());
        let glob_patterns = Self::parse_glob_patterns(input.get("glob").and_then(|v| v.as_str()));
        let file_type = input.get("type").and_then(|v| v.as_str());

        let escaped_path = Self::shell_escape(&resolved_path);
        let escaped_pattern = Self::shell_escape(pattern);
        let offset_cmd = if offset > 0 {
            format!(" | tail -n +{}", offset + 1)
        } else {
            String::new()
        };
        let limit_cmd = head_limit
            .map(|limit| format!(" | head -n {}", limit))
            .unwrap_or_default();

        let mut cmd = "rg --no-heading --hidden --max-columns 500".to_string();
        if case_insensitive {
            cmd.push_str(" -i");
        }
        if output_mode == "files_with_matches" {
            cmd.push_str(" -l");
        } else if output_mode == "count" {
            cmd.push_str(" -c");
        } else if show_line_numbers {
            cmd.push_str(" --line-number");
        }
        if output_mode == "content" {
            if let Some(context) = context_c {
                cmd.push_str(&format!(" -C {}", context));
            } else {
                if let Some(before) = before_context {
                    cmd.push_str(&format!(" -B {}", before));
                }
                if let Some(after) = after_context {
                    cmd.push_str(&format!(" -A {}", after));
                }
            }
        }
        for glob_pattern in glob_patterns {
            cmd.push_str(&format!(" --glob '{}'", Self::shell_escape(&glob_pattern)));
        }
        if let Some(ft) = file_type {
            cmd.push_str(&format!(" --type '{}'", Self::shell_escape(ft)));
        }
        cmd.push_str(&format!(
            " -e '{}' '{}' 2>/dev/null{}{}",
            escaped_pattern, escaped_path, offset_cmd, limit_cmd
        ));

        let full_cmd = format!(
            "if command -v rg >/dev/null 2>&1; then {}; else grep -rn{} -e '{}' '{}' 2>/dev/null{}{}; fi",
            cmd,
            if case_insensitive { "i" } else { "" },
            escaped_pattern,
            escaped_path,
            offset_cmd,
            limit_cmd,
        );

        let (stdout, _stderr, _exit_code) = ws_shell
            .exec(&full_cmd, Some(30_000))
            .await
            .map_err(|e| BitFunError::tool(format!("Remote grep failed: {}", e)))?;

        let lines: Vec<&str> = stdout.lines().collect();
        let total_matches = lines.len();
        let display_base = Self::display_base(context);
        let result_text = if lines.is_empty() {
            format!("No matches found for pattern '{}'", pattern)
        } else {
            Self::relativize_result_text(&stdout, display_base.as_deref())
        };

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
        let before_context = input.get("-B").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let after_context = input.get("-A").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let shared_context = input
            .get("context")
            .or_else(|| input.get("-C"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
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
            before_context: if shared_context > 0 {
                shared_context
            } else {
                before_context
            },
            after_context: if shared_context > 0 {
                shared_context
            } else {
                after_context
            },
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
                apply_offset_and_limit(&mut lines, offset, head_limit);
                let rendered = Self::relativize_result_text(&lines.join("\n"), display_base);
                let file_count = result
                    .hits
                    .iter()
                    .map(|hit| hit.path.as_str())
                    .collect::<HashSet<_>>()
                    .len();
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
                let rendered = Self::relativize_result_text(&lines.join("\n"), display_base);
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
                let rendered = Self::relativize_result_text(&files.join("\n"), display_base);
                let total_matches = files.len();
                (rendered, total_matches, total_matches)
            }
        }
    }
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

fn apply_offset_and_limit(items: &mut Vec<String>, offset: usize, head_limit: Option<usize>) {
    if offset > 0 {
        if offset >= items.len() {
            items.clear();
        } else {
            *items = items[offset..].to_vec();
        }
    }

    if let Some(limit) = head_limit {
        if items.len() > limit {
            items.truncate(limit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{render_workspace_search_content_lines, GrepTool, DEFAULT_HEAD_LIMIT};
    use crate::infrastructure::FileSearchOutcome;
    use crate::service::search::{
        ContentSearchResult, WorkspaceSearchBackend, WorkspaceSearchHit, WorkspaceSearchLine,
        WorkspaceSearchMatch, WorkspaceSearchMatchLocation, WorkspaceSearchRepoPhase,
        WorkspaceSearchRepoStatus,
    };
    use serde_json::json;

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
        let relativized = GrepTool::relativize_result_text(text, Some("/repo"));

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
                base_snapshot_root: "/repo/.bitfun/search/flashgrep-index/base-snapshot".to_string(),
                workspace_overlay_root: "/repo/.bitfun/search/flashgrep-index/workspace-overlay"
                    .to_string(),
                phase: WorkspaceSearchRepoPhase::Ready,
                snapshot_key: None,
                last_probe_unix_secs: None,
                last_rebuild_unix_secs: None,
                dirty_files: crate::service::search::WorkspaceSearchDirtyFiles {
                    modified: 0,
                    deleted: 0,
                    new: 0,
                },
                rebuild_recommended: false,
                active_task_id: None,
                probe_healthy: true,
                last_error: None,
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
- The path parameter may be an absolute path or an exact `bitfun://runtime/...` URI returned by another tool
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
                    "description": "File or directory to search in (rg PATH). Defaults to current working directory. May be an absolute path or an exact bitfun://runtime URI."
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
        false
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
        let search_path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let resolved = context.resolve_tool_path(search_path)?;

        if resolved.uses_remote_workspace_backend() {
            return self.call_remote(input, context).await;
        }

        if let Some(search_service) = get_global_workspace_search_service() {
            let (request, output_mode, show_line_numbers, offset, head_limit) =
                self.build_workspace_search_request(input, context)?;
            let pattern = request.pattern.clone();
            let search_mode = request.output_mode.search_mode();
            let path = request
                .search_path
                .as_ref()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_else(|| request.repo_root.to_string_lossy().to_string());
            let search_started_at = Instant::now();
            let search_result = search_service.search_content(request).await?;
            let display_base = Self::display_base(context);
            let (result_text, file_count, total_matches) = self.format_workspace_search_output(
                &output_mode,
                show_line_numbers,
                offset,
                head_limit,
                &search_result,
                display_base.as_deref(),
            );
            let workspace_search_elapsed_ms = search_started_at.elapsed().as_millis();

            log::info!(
                "Grep tool workspace-search result: pattern={}, path={}, output_mode={}, search_mode={:?}, file_count={}, total_matches={}, backend={:?}, repo_phase={:?}, rebuild_recommended={}, dirty_modified={}, dirty_deleted={}, dirty_new={}, candidate_docs={}, matched_lines={}, matched_occurrences={}, workspace_search_ms={}",
                pattern,
                path,
                output_mode,
                search_mode,
                file_count,
                total_matches,
                search_result.backend,
                search_result.repo_status.phase,
                search_result.repo_status.rebuild_recommended,
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
                    "rebuild_recommended": search_result.repo_status.rebuild_recommended,
                    "applied_limit": head_limit,
                    "applied_offset": if offset > 0 { Some(offset) } else { None::<usize> },
                    "result": result_text,
                }),
                result_for_assistant: Some(result_text),
                image_attachments: None,
            }]);
        }

        let grep_options = self.build_grep_options(input, context)?;
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
