use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::service::search::{
    get_global_workspace_search_service, remote_workspace_search_service_for_path,
    workspace_search_feature_enabled, workspace_search_runtime_available, GlobSearchRequest,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use log::{info, warn};
use serde_json::{json, Value};
use std::path::PathBuf;
use tool_runtime::search::glob_search::{
    build_remote_find_command, build_remote_rg_command, collect_remote_glob_matches,
    execute_local_glob, normalize_path, LocalGlobRequest,
};

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
- The path parameter may be workspace-relative, an absolute path inside the current workspace, or an exact `bitfun://runtime/...` URI returned by another tool
- Omit path to search the current workspace. Do not use host roots or placeholder paths such as `/workspace`.
- You can call multiple tools in a single response. It is always better to speculatively perform multiple searches in parallel if they are potentially useful.
<example>
- List files in current workspace: pattern = "*"
- Search all markdown files in src recursively: path = "src", pattern = "**/*.md"
</example>
"#.to_string())
    }

    fn short_description(&self) -> String {
        "Find files by glob pattern.".to_string()
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
                    "description": "The directory to search in. Omit this field to search the current workspace. If provided, use a workspace-relative path, an absolute path inside the current workspace, or an exact bitfun://runtime URI. Do not enter \"undefined\", \"null\", host roots, or placeholder paths such as /workspace."
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

        if resolved.uses_remote_workspace_backend() {
            if workspace_search_feature_enabled().await {
                let remote_workspace_glob_result = async {
                    let workspace_root = context
                        .workspace
                        .as_ref()
                        .map(|workspace| PathBuf::from(workspace.root_path_string()))
                        .ok_or_else(|| {
                            BitFunError::tool(
                                "workspace_path is required when Glob path is omitted".to_string(),
                            )
                        })?;
                    let resolved_path = PathBuf::from(&resolved.resolved_path);
                    let repo_root = workspace_root.to_string_lossy().to_string();
                    let preferred_connection_id = context
                        .workspace
                        .as_ref()
                        .and_then(|workspace| workspace.connection_id())
                        .map(str::to_string);
                    let search_service = remote_workspace_search_service_for_path(
                        &repo_root,
                        preferred_connection_id,
                    )
                    .await
                    .map_err(BitFunError::tool)?;
                    let glob_result = search_service
                        .glob(GlobSearchRequest {
                            repo_root: workspace_root.clone(),
                            search_path: (resolved_path != workspace_root).then_some(resolved_path),
                            pattern: pattern.to_string(),
                            limit,
                        })
                        .await
                        .map_err(BitFunError::tool)?;

                    let match_count = glob_result.paths.len();
                    let result_text = if glob_result.paths.is_empty() {
                        format!("No files found matching pattern '{}'", pattern)
                    } else {
                        glob_result.paths.join("\n")
                    };

                    Ok::<Vec<ToolResult>, BitFunError>(vec![ToolResult::Result {
                        data: json!({
                            "pattern": pattern,
                            "path": resolved.logical_path,
                            "matches": glob_result.paths,
                            "match_count": match_count,
                            "repo_phase": glob_result.repo_status.phase,
                            "rebuild_recommended": glob_result.repo_status.rebuild_recommended
                        }),
                        result_for_assistant: Some(result_text),
                        image_attachments: None,
                    }])
                }
                .await;

                match remote_workspace_glob_result {
                    Ok(results) => return Ok(results),
                    Err(error) => {
                        warn!(
                            "Glob tool remote workspace-search failed; falling back to shell glob: {}",
                            error
                        );
                    }
                }
            }

            // Remote workspace fallback: prefer `rg --files --glob`, but fall back to `find`.
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

            let limited = collect_remote_glob_matches(&search_dir, &stdout, limit)
                .into_iter()
                .map(|path| {
                    resolved
                        .logical_child_path(&path)
                        .unwrap_or_else(|| normalize_path(&path))
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

        if workspace_search_runtime_available().await {
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
        }
        let resolved_str_for_rg = resolved_str.clone();
        let pattern_for_rg = pattern.to_string();
        let glob_result = tokio::task::spawn_blocking(move || {
            execute_local_glob(LocalGlobRequest {
                search_path: PathBuf::from(resolved_str_for_rg),
                pattern: pattern_for_rg,
                limit,
            })
        })
        .await
        .map_err(|err| BitFunError::tool(format!("Glob tool task failed: {}", err)))?
        .map_err(BitFunError::tool)?;

        let matches = glob_result
            .matches
            .into_iter()
            .map(|path| {
                resolved
                    .logical_child_path(&path)
                    .unwrap_or_else(|| normalize_path(&path))
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
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tool_runtime::search::glob_search::{
        derive_walk_root, execute_local_glob, extract_glob_base_directory, normalize_path,
        LocalGlobRequest,
    };

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
        let root = make_temp_dir("limit");
        fs::create_dir_all(root.join("src/deep")).unwrap();
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(root.join("Cargo.toml"), "").unwrap();
        fs::write(root.join("src/lib.rs"), "").unwrap();
        fs::write(root.join("src/deep/mod.rs"), "").unwrap();
        fs::write(root.join("tests/mod.rs"), "").unwrap();

        let matches = execute_local_glob(LocalGlobRequest {
            search_path: root.clone(),
            pattern: "**/*.rs".to_string(),
            limit: 2,
        })
        .unwrap()
        .matches
        .into_iter()
        .map(|path| normalize_path(&path))
        .collect::<Vec<_>>();

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
        let root = make_temp_dir("files-only");
        fs::create_dir_all(root.join("src/nested")).unwrap();
        fs::write(root.join("src/nested/lib.rs"), "").unwrap();

        let matches = execute_local_glob(LocalGlobRequest {
            search_path: root.clone(),
            pattern: "*".to_string(),
            limit: 10,
        })
        .unwrap()
        .matches
        .into_iter()
        .map(|path| normalize_path(&path))
        .collect::<Vec<_>>();

        assert!(matches.iter().all(|path| !path.ends_with("/src")));
        assert!(!matches.is_empty());

        let _ = fs::remove_dir_all(root);
    }
}
