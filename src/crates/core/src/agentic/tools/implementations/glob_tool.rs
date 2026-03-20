use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use globset::GlobBuilder;
use ignore::WalkBuilder;
use log::warn;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn glob_with_ignore(
    search_path: &str,
    pattern: &str,
    ignore: bool,
    ignore_hidden: bool,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let path = std::path::Path::new(search_path);
    if !path.exists() {
        return Err(format!("Search path '{}' does not exist", search_path).into());
    }
    if !path.is_dir() {
        return Err(format!("Search path '{}' is not a directory", search_path).into());
    }

    let search_path_abs = dunce::canonicalize(Path::new(search_path))?;
    let search_path_str = search_path_abs.to_string_lossy();

    let absolute_pattern = format!("{}/{}", search_path_str, pattern);

    let glob = GlobBuilder::new(&absolute_pattern)
        .literal_separator(true)
        .build()?
        .compile_matcher();

    let walker = WalkBuilder::new(&search_path_abs)
        .git_ignore(ignore)
        .hidden(ignore_hidden)
        .build();

    let mut results = Vec::new();

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warn!("Glob walker entry error (skipped): {}", err);
                continue;
            }
        };
        let path = entry.path().to_path_buf();

        if glob.is_match(&path) {
            let simplified_path = dunce::simplified(&path);
            results.push(simplified_path.to_string_lossy().to_string());
        }
    }

    Ok(results)
}

fn limit_paths(paths: &[String], limit: usize) -> Vec<String> {
    let mut depth_and_paths = paths
        .iter()
        .map(|path| {
            let normalized_path = path.replace('\\', "/");
            let n = normalized_path.split('/').count();
            (n, normalized_path)
        })
        .collect::<Vec<_>>();
    depth_and_paths.sort_by_key(|(depth, _)| *depth);
    let mut result = depth_and_paths
        .into_iter()
        .take(limit)
        .map(|(_, path)| path)
        .collect::<Vec<_>>();
    result.sort();
    result
}

fn call_glob(search_path: &str, pattern: &str, limit: usize) -> Result<Vec<String>, String> {
    let is_whitelisted = pattern.starts_with(".bitfun")
        || pattern.contains("/.bitfun")
        || pattern.contains("\\.bitfun");

    let apply_gitignore = !is_whitelisted;
    let ignore_hidden_files = !is_whitelisted;

    let all_paths = glob_with_ignore(search_path, pattern, apply_gitignore, ignore_hidden_files)
        .map_err(|e| e.to_string())?;
    let limited_paths = limit_paths(&all_paths, limit);
    Ok(limited_paths)
}

fn build_remote_find_command(search_dir: &str, pattern: &str, limit: usize) -> String {
    let name_pattern = if pattern.contains("**/") {
        pattern.replacen("**/", "", 1)
    } else {
        pattern.to_string()
    };

    let escaped_dir = search_dir.replace('\'', "'\\''");
    let escaped_pattern = name_pattern.replace('\'', "'\\''");

    format!(
        "find '{}' -maxdepth 10 -name '{}' -not -path '*/.git/*' -not -path '*/node_modules/*' 2>/dev/null | head -n {}",
        escaped_dir, escaped_pattern, limit
    )
}

pub struct GlobTool;

impl GlobTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "Glob"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Fast file pattern matching tool support Standard Unix-style glob syntax
- Supports glob patterns like "**/*.js" or "src/**/*.ts"
- Returns matching file paths
- Use this tool when you need to find files by name patterns
- You can call multiple tools in a single response. It is always better to speculatively perform multiple searches in parallel if they are potentially useful.
<example>
- List files and directories in path: path = "/path/to/search", pattern = "*"
- Search all markdown files in path recursively: path = "/path/to/search", pattern = "**/*.md"
</example>
"#.to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The glob pattern to match files against (relative to `path`)"
                },
                "path": {
                    "type": "string",
                    "description": "The directory to search in. If not specified, the current working directory will be used. IMPORTANT: Omit this field to use the default directory. DO NOT enter \"undefined\" or \"null\" - simply omit it for the default behavior. Must be a valid absolute path if provided."
                },
                "limit": {
                    "type": "number",
                    "description": "The maximum number of entries to return. Defaults to 100."
                }
            },
            "required": ["pattern"]
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

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let pattern = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("pattern is required".to_string()))?;

        let resolved_path = match input.get("path").and_then(|v| v.as_str()) {
            Some(user_path) if Path::new(user_path).is_absolute() => PathBuf::from(user_path),
            Some(user_path) => {
                let workspace_root = context.workspace_root().ok_or_else(|| {
                    BitFunError::tool(format!(
                        "workspace_path is required to resolve relative search path: {}",
                        user_path
                    ))
                })?;
                workspace_root.join(user_path)
            }
            None => context.workspace_root().map(PathBuf::from).ok_or_else(|| {
                BitFunError::tool(
                    "workspace_path is required when Glob path is omitted".to_string(),
                )
            })?,
        };

        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(100);

        // Remote workspace: use `find` via the workspace shell
        if context.is_remote() {
            let ws_shell = context.ws_shell().ok_or_else(|| {
                BitFunError::tool("Workspace shell not available".to_string())
            })?;

            let search_dir = resolved_path.display().to_string();
            let find_cmd = build_remote_find_command(&search_dir, pattern, limit);

            let (stdout, _stderr, _exit_code) = ws_shell
                .exec(&find_cmd, Some(30_000))
                .await
                .map_err(|e| BitFunError::tool(format!("Failed to glob on remote: {}", e)))?;

            let matches: Vec<String> = stdout
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.to_string())
                .collect();
            let limited = limit_paths(&matches, limit);
            let result_text = if limited.is_empty() {
                format!("No files found matching pattern '{}'", pattern)
            } else {
                limited.join("\n")
            };

            return Ok(vec![ToolResult::Result {
                data: json!({
                    "pattern": pattern,
                    "path": search_dir,
                    "matches": limited,
                    "match_count": limited.len()
                }),
                result_for_assistant: Some(result_text),
            }]);
        }

        let matches = call_glob(&resolved_path.display().to_string(), pattern, limit)
            .map_err(|e| BitFunError::tool(e))?;

        let result_text = if matches.is_empty() {
            format!("No files found matching pattern '{}'", pattern)
        } else {
            matches.join("\n")
        };

        let result = ToolResult::Result {
            data: json!({
                "pattern": pattern,
                "path": resolved_path.display().to_string(),
                "matches": matches,
                "match_count": matches.len()
            }),
            result_for_assistant: Some(result_text),
        };

        Ok(vec![result])
    }
}
