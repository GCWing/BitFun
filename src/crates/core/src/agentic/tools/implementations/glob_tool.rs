use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::service::search::{get_global_workspace_search_service, GlobSearchRequest};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use globset::{GlobBuilder, GlobMatcher};
use ignore::WalkBuilder;
use log::{info, warn};
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

fn extract_glob_base_directory(pattern: &str) -> (String, String) {
    let glob_start = pattern.find(['*', '?', '[', '{']);

    match glob_start {
        Some(index) => {
            let static_prefix = &pattern[..index];
            let last_separator = static_prefix
                .char_indices()
                .rev()
                .find(|(_, ch)| *ch == '/' || *ch == '\\')
                .map(|(idx, _)| idx);

            if let Some(separator_index) = last_separator {
                (
                    static_prefix[..separator_index].to_string(),
                    pattern[separator_index + 1..].to_string(),
                )
            } else {
                (String::new(), pattern.to_string())
            }
        }
        None => {
            let trimmed = pattern.trim_end_matches(['/', '\\']);
            let literal_path = Path::new(trimmed);
            let base_dir = literal_path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty() && *parent != Path::new("."))
                .map(|parent| parent.to_string_lossy().to_string())
                .unwrap_or_default();
            let file_name = literal_path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| trimmed.to_string());

            let relative_pattern = if pattern.ends_with('/') || pattern.ends_with('\\') {
                format!("{}/", file_name)
            } else {
                file_name
            };

            (base_dir, relative_pattern)
        }
    }
}

fn normalize_path(path: &Path) -> String {
    dunce::simplified(path).to_string_lossy().replace('\\', "/")
}

fn shell_escape(value: &str) -> String {
    value.replace('\'', "'\\''")
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct GlobCandidate {
    depth: usize,
    path: String,
}

impl Ord for GlobCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.depth
            .cmp(&other.depth)
            .then_with(|| self.path.cmp(&other.path))
    }
}

impl PartialOrd for GlobCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn is_safe_relative_subpath(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn derive_walk_root(search_path_abs: &Path, pattern: &str) -> (PathBuf, String) {
    let (base_dir, relative_pattern) = extract_glob_base_directory(pattern);
    let base_path = Path::new(&base_dir);

    if base_dir.is_empty() || !is_safe_relative_subpath(base_path) {
        return (search_path_abs.to_path_buf(), pattern.to_string());
    }

    let walk_root = search_path_abs.join(base_path);
    if walk_root.starts_with(search_path_abs) {
        (walk_root, relative_pattern)
    } else {
        (search_path_abs.to_path_buf(), pattern.to_string())
    }
}

fn resolve_glob_config(pattern: &str) -> (bool, bool) {
    let is_whitelisted = pattern.starts_with(".bitfun")
        || pattern.contains("/.bitfun")
        || pattern.contains("\\.bitfun");

    let apply_gitignore = !is_whitelisted;
    let ignore_hidden_files = !is_whitelisted;
    (apply_gitignore, ignore_hidden_files)
}

fn build_rg_args(
    relative_pattern: &str,
    apply_gitignore: bool,
    ignore_hidden_files: bool,
) -> Vec<String> {
    let mut args = vec![
        "--files".to_string(),
        "--glob".to_string(),
        relative_pattern.to_string(),
        "--sort".to_string(),
        "path".to_string(),
    ];

    if !apply_gitignore {
        args.push("--no-ignore".to_string());
    }

    if !ignore_hidden_files {
        args.push("--hidden".to_string());
    }

    args
}

fn build_fallback_matcher(relative_pattern: &str) -> Result<GlobMatcher, String> {
    GlobBuilder::new(relative_pattern)
        .literal_separator(true)
        .build()
        .map_err(|err| err.to_string())
        .map(|glob| glob.compile_matcher())
}

fn match_relative_path(matcher: &GlobMatcher, relative_path: &str, is_dir: bool) -> bool {
    if is_dir {
        matcher.is_match(relative_path) || matcher.is_match(format!("{}/", relative_path))
    } else {
        matcher.is_match(relative_path)
    }
}

fn collect_with_walk_fallback(
    walk_root: &Path,
    relative_pattern: &str,
    apply_gitignore: bool,
    ignore_hidden_files: bool,
    limit: usize,
) -> Result<Vec<String>, String> {
    let matcher = build_fallback_matcher(relative_pattern)?;
    let walker = WalkBuilder::new(walk_root)
        .ignore(apply_gitignore)
        .git_ignore(apply_gitignore)
        .git_global(apply_gitignore)
        .git_exclude(apply_gitignore)
        .hidden(ignore_hidden_files)
        .build();

    let mut best_matches = BinaryHeap::with_capacity(limit.saturating_add(1));
    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warn!("Glob walker fallback entry error (skipped): {}", err);
                continue;
            }
        };

        let path = entry.path().to_path_buf();
        let relative_path = match path.strip_prefix(walk_root) {
            Ok(relative) => relative,
            Err(_) => continue,
        };
        let relative_path = normalize_path(relative_path);

        if match_relative_path(
            &matcher,
            &relative_path,
            entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false),
        ) {
            let normalized_path = normalize_path(&path);
            let candidate = GlobCandidate {
                depth: normalized_path.split('/').count(),
                path: normalized_path,
            };

            if best_matches.len() < limit {
                best_matches.push(candidate);
            } else if let Some(worst_match) = best_matches.peek() {
                if candidate < *worst_match {
                    best_matches.pop();
                    best_matches.push(candidate);
                }
            }
        }
    }

    let mut results = best_matches
        .into_sorted_vec()
        .into_iter()
        .map(|candidate| candidate.path)
        .collect::<Vec<_>>();
    results.sort();
    Ok(results)
}

fn call_rg(search_path: &str, pattern: &str, limit: usize) -> Result<Vec<String>, String> {
    let path = std::path::Path::new(search_path);
    if !path.exists() {
        return Err(format!("Search path '{}' does not exist", search_path));
    }
    if !path.is_dir() {
        return Err(format!("Search path '{}' is not a directory", search_path));
    }

    let search_path_abs =
        dunce::canonicalize(Path::new(search_path)).map_err(|err| err.to_string())?;
    let (walk_root, relative_pattern) = derive_walk_root(&search_path_abs, pattern);
    let (apply_gitignore, ignore_hidden_files) = resolve_glob_config(pattern);

    if !walk_root.exists() || !walk_root.is_dir() || limit == 0 {
        return Ok(Vec::new());
    }

    let args = build_rg_args(&relative_pattern, apply_gitignore, ignore_hidden_files);
    let output = Command::new("rg")
        .current_dir(&walk_root)
        .args(&args)
        .arg(".")
        .output()
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                "ripgrep (rg) is required for Glob tool execution but was not found".to_string()
            } else {
                format!("Failed to execute rg for Glob tool: {}", err)
            }
        });

    let output = match output {
        Ok(output) => {
            info!(
                "Glob backend selected: backend=rg, search_root={}, pattern={}",
                walk_root.display(),
                relative_pattern
            );
            output
        }
        Err(err) if err.contains("ripgrep (rg) is required") => {
            info!(
                "Glob backend selected: backend=fallback_walk, reason=rg_not_found, search_root={}, pattern={}",
                walk_root.display(),
                relative_pattern
            );
            return collect_with_walk_fallback(
                &walk_root,
                &relative_pattern,
                apply_gitignore,
                ignore_hidden_files,
                limit,
            );
        }
        Err(err) => return Err(err),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let message = if stderr.is_empty() {
            format!("rg --files failed with status {}", output.status)
        } else {
            format!("rg --files failed: {}", stderr)
        };
        if stderr.contains("No such file or directory") || stderr.contains("not found") {
            info!(
                "Glob backend selected: backend=fallback_walk, reason=rg_execution_failed, search_root={}, pattern={}",
                walk_root.display(),
                relative_pattern
            );
            return collect_with_walk_fallback(
                &walk_root,
                &relative_pattern,
                apply_gitignore,
                ignore_hidden_files,
                limit,
            );
        }
        return Err(message);
    }

    let all_paths = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let relative_path = line.strip_prefix("./").unwrap_or(line);
            let full_path = walk_root.join(relative_path);
            normalize_path(&full_path)
        })
        .collect::<Vec<_>>();
    Ok(limit_paths(&all_paths, limit))
}

fn limit_paths(paths: &[String], limit: usize) -> Vec<String> {
    let mut depth_and_paths = paths
        .iter()
        .map(|path| {
            let normalized_path = path.replace('\\', "/");
            let depth = normalized_path.split('/').count();
            (depth, normalized_path)
        })
        .collect::<Vec<_>>();
    depth_and_paths.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

    let mut result = depth_and_paths
        .into_iter()
        .take(limit)
        .map(|(_, path)| path)
        .collect::<Vec<_>>();
    result.sort();
    result
}

fn build_remote_rg_command(search_dir: &str, pattern: &str) -> String {
    let search_dir_path = Path::new(search_dir);
    let (remote_walk_root, remote_pattern) = derive_walk_root(search_dir_path, pattern);
    let (apply_gitignore, ignore_hidden_files) = resolve_glob_config(pattern);

    let mut parts = vec![
        "cd".to_string(),
        format!(
            "'{}'",
            shell_escape(remote_walk_root.to_string_lossy().as_ref())
        ),
        "&&".to_string(),
        "rg".to_string(),
        "--files".to_string(),
        "--glob".to_string(),
        format!("'{}'", shell_escape(&remote_pattern)),
        "--sort".to_string(),
        "path".to_string(),
    ];

    if !apply_gitignore {
        parts.push("--no-ignore".to_string());
    }

    if !ignore_hidden_files {
        parts.push("--hidden".to_string());
    }

    parts.push(".".to_string());
    parts.push("2>/dev/null".to_string());
    parts.join(" ")
}

fn build_remote_find_command(search_dir: &str, pattern: &str, limit: usize) -> String {
    let search_dir_path = Path::new(search_dir);
    let (remote_walk_root, remote_pattern) = derive_walk_root(search_dir_path, pattern);

    let name_pattern = if remote_pattern.contains("**/") {
        remote_pattern.replacen("**/", "", 1)
    } else if remote_pattern.contains('/') || remote_pattern.contains('\\') {
        "*".to_string()
    } else {
        remote_pattern
    };

    let escaped_dir = remote_walk_root.to_string_lossy().replace('\'', "'\\''");
    let escaped_pattern = name_pattern.replace('\'', "'\\''");

    format!(
        "find '{}' -maxdepth 10 -name '{}' -not -path '*/.git/*' -not -path '*/node_modules/*' 2>/dev/null | head -n {}",
        escaped_dir, escaped_pattern, limit
    )
}

pub struct GlobTool;

impl Default for GlobTool {
    fn default() -> Self {
        Self::new()
    }
}

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
- The path parameter may be an absolute path or an exact `bitfun://runtime/...` URI returned by another tool
- You can call multiple tools in a single response. It is always better to speculatively perform multiple searches in parallel if they are potentially useful.
<example>
- List files in path: path = "/path/to/search", pattern = "*"
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
                    "description": "The directory to search in. If not specified, the current working directory will be used. IMPORTANT: Omit this field to use the default directory. DO NOT enter \"undefined\" or \"null\" - simply omit it for the default behavior. Must be a valid absolute path or exact bitfun://runtime URI if provided."
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

        let resolved = match input.get("path").and_then(|v| v.as_str()) {
            Some(user_path) => context.resolve_tool_path(user_path)?,
            None => {
                let root = context
                    .workspace
                    .as_ref()
                    .map(|w| w.root_path_string())
                    .ok_or_else(|| {
                        BitFunError::tool(
                            "workspace_path is required when Glob path is omitted".to_string(),
                        )
                    })?;
                crate::agentic::tools::framework::ToolPathResolution {
                    requested_path: root.clone(),
                    logical_path: root.clone(),
                    resolved_path: root,
                    backend: if context.is_remote() {
                        crate::agentic::tools::framework::ToolPathBackend::RemoteWorkspace
                    } else {
                        crate::agentic::tools::framework::ToolPathBackend::Local
                    },
                    runtime_scope: None,
                    runtime_root: None,
                }
            }
        };
        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(100);

        // Remote workspace: prefer `rg --files --glob`, but fall back to `find`
        if resolved.uses_remote_workspace_backend() {
            let ws_shell = context
                .ws_shell()
                .ok_or_else(|| BitFunError::tool("Workspace shell not available".to_string()))?;

            let search_dir = resolved.resolved_path.clone();
            let (_stdout, _stderr, exit_code) = ws_shell
                .exec("command -v rg >/dev/null 2>&1", Some(5_000))
                .await
                .map_err(|e| BitFunError::tool(format!("Failed to detect rg on remote: {}", e)))?;

            let remote_cmd = if exit_code == 0 {
                info!(
                    "Glob backend selected: backend=remote_rg, search_path={}, pattern={}",
                    search_dir, pattern
                );
                build_remote_rg_command(&search_dir, pattern)
            } else {
                info!(
                    "Glob backend selected: backend=remote_find, reason=rg_not_found, search_path={}, pattern={}",
                    search_dir, pattern
                );
                build_remote_find_command(&search_dir, pattern, limit)
            };

            let (stdout, _stderr, _exit_code) = ws_shell
                .exec(&remote_cmd, Some(30_000))
                .await
                .map_err(|e| {
                    BitFunError::tool(format!("Failed to glob on remote with rg: {}", e))
                })?;

            let matches: Vec<String> = stdout
                .lines()
                .filter(|l| !l.is_empty())
                .map(|line| {
                    let relative_path = line.strip_prefix("./").unwrap_or(line);
                    normalize_path(&Path::new(&search_dir).join(relative_path))
                })
                .collect();
            let limited = limit_paths(&matches, limit)
                .into_iter()
                .map(|path| {
                    resolved
                        .logical_child_path(Path::new(&path))
                        .unwrap_or(path)
                })
                .collect::<Vec<_>>();
            let result_text = if limited.is_empty() {
                format!("No files found matching pattern '{}'", pattern)
            } else {
                limited.join("\n")
            };

            return Ok(vec![ToolResult::Result {
                data: json!({
                    "pattern": pattern,
                    "path": resolved.logical_path,
                    "matches": limited,
                    "match_count": limited.len()
                }),
                result_for_assistant: Some(result_text),
                image_attachments: None,
            }]);
        }

        let resolved_str = resolved.resolved_path.clone();

        if let Some(search_service) = get_global_workspace_search_service() {
            let workspace_root = context
                .workspace
                .as_ref()
                .map(|workspace| PathBuf::from(workspace.root_path_string()))
                .ok_or_else(|| {
                    BitFunError::tool(
                        "workspace_path is required when Glob path is omitted".to_string(),
                    )
                })?;
            let resolved_path = PathBuf::from(&resolved_str);
            let glob_result = search_service
                .glob(GlobSearchRequest {
                    repo_root: workspace_root.clone(),
                    search_path: (resolved_path != workspace_root).then_some(resolved_path),
                    pattern: pattern.to_string(),
                    limit,
                })
                .await?;

            let result_text = if glob_result.paths.is_empty() {
                format!("No files found matching pattern '{}'", pattern)
            } else {
                glob_result.paths.join("\n")
            };

            return Ok(vec![ToolResult::Result {
                data: json!({
                    "pattern": pattern,
                    "path": resolved_str,
                    "matches": glob_result.paths,
                    "match_count": glob_result.paths.len(),
                    "repo_phase": glob_result.repo_status.phase,
                    "rebuild_recommended": glob_result.repo_status.rebuild_recommended
                }),
                result_for_assistant: Some(result_text),
                image_attachments: None,
            }]);
        }
        let resolved_str_for_rg = resolved_str.clone();
        let pattern_for_rg = pattern.to_string();
        let matches = tokio::task::spawn_blocking(move || {
            call_rg(&resolved_str_for_rg, &pattern_for_rg, limit)
        })
        .await
        .map_err(|err| BitFunError::tool(format!("Glob tool task failed: {}", err)))?
        .map_err(BitFunError::tool)?;

        let matches = matches
            .into_iter()
            .map(|path| {
                resolved
                    .logical_child_path(Path::new(&path))
                    .unwrap_or(path)
            })
            .collect::<Vec<_>>();

        let result_text = if matches.is_empty() {
            format!("No files found matching pattern '{}'", pattern)
        } else {
            matches.join("\n")
        };

        let result = ToolResult::Result {
            data: json!({
                "pattern": pattern,
                "path": resolved.logical_path,
                "matches": matches,
                "match_count": matches.len()
            }),
            result_for_assistant: Some(result_text),
            image_attachments: None,
        };

        Ok(vec![result])
    }
}

#[cfg(test)]
mod tests {
    use super::{call_rg, derive_walk_root, extract_glob_base_directory};
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("bitfun-glob-tool-{name}-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn extracts_static_glob_prefix() {
        assert_eq!(
            extract_glob_base_directory("src/**/*.rs"),
            ("src".to_string(), "**/*.rs".to_string())
        );
        assert_eq!(
            extract_glob_base_directory("*.rs"),
            (String::new(), "*.rs".to_string())
        );
        assert_eq!(
            extract_glob_base_directory("src/lib.rs"),
            ("src".to_string(), "lib.rs".to_string())
        );
    }

    #[test]
    fn does_not_expand_walk_root_outside_search_path() {
        let root = std::env::temp_dir().join("bitfun-glob-root");
        let (walk_root, relative_pattern) = derive_walk_root(&root, "../*.rs");

        assert_eq!(walk_root, root);
        assert_eq!(relative_pattern, "../*.rs".to_string());
    }

    #[test]
    fn keeps_shallowest_matches_from_rg_results() {
        if Command::new("rg").arg("--version").output().is_err() {
            return;
        }

        let root = make_temp_dir("limit");
        fs::create_dir_all(root.join("src/deep")).unwrap();
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(root.join("Cargo.toml"), "").unwrap();
        fs::write(root.join("src/lib.rs"), "").unwrap();
        fs::write(root.join("src/deep/mod.rs"), "").unwrap();
        fs::write(root.join("tests/mod.rs"), "").unwrap();

        let matches = call_rg(root.to_string_lossy().as_ref(), "**/*.rs", 2).unwrap();

        assert_eq!(matches.len(), 2);
        assert!(matches.iter().any(|path| path.ends_with("/src/lib.rs")));
        assert!(matches.iter().any(|path| path.ends_with("/tests/mod.rs")));
        assert!(!matches
            .iter()
            .any(|path| path.ends_with("/src/deep/mod.rs")));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn wildcard_search_now_returns_files_only() {
        if Command::new("rg").arg("--version").output().is_err() {
            return;
        }

        let root = make_temp_dir("files-only");
        fs::create_dir_all(root.join("src/nested")).unwrap();
        fs::write(root.join("src/nested/lib.rs"), "").unwrap();

        let matches = call_rg(root.to_string_lossy().as_ref(), "*", 10).unwrap();

        assert!(matches.iter().all(|path| !path.ends_with("/src")));
        assert!(!matches.is_empty());

        let _ = fs::remove_dir_all(root);
    }
}
