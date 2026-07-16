use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::service::search::{
    get_global_workspace_search_service, remote_workspace_search_service_for_path,
    workspace_search_feature_enabled, workspace_search_runtime_available, GlobSearchRequest,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use log::{info, warn};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tool_runtime::search::glob_search::{
    build_remote_find_command, build_remote_rg_command, collect_remote_glob_result,
    derive_walk_root, execute_local_glob, extract_glob_base_directory, normalize_path,
    LocalGlobRequest,
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

const GLOB_RESULT_LIMIT: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
struct EffectiveGlobSearch {
    search_path: String,
    pattern: String,
}

/// Converts an absolute glob pattern into a search root plus a glob relative to
/// that root. `rg --glob` matches paths relative to its search root, so passing
/// an absolute pattern through unchanged can never match a workspace walk.
fn resolve_effective_glob_search(
    search_path: &str,
    pattern: &str,
    is_remote_workspace: bool,
) -> EffectiveGlobSearch {
    let (base_dir, relative_pattern) = extract_glob_base_directory(pattern);
    let is_absolute_base = if is_remote_workspace {
        base_dir.starts_with('/')
    } else {
        Path::new(&base_dir).is_absolute()
    };

    if is_absolute_base {
        EffectiveGlobSearch {
            search_path: base_dir,
            pattern: relative_pattern,
        }
    } else {
        EffectiveGlobSearch {
            search_path: search_path.to_string(),
            pattern: pattern.to_string(),
        }
    }
}

/// Selects whether the local workspace-search backend can serve this path.
/// Flashgrep sessions are scoped to one canonical workspace root; paths outside
/// that root skip this backend and continue through the caller's fallback chain.
fn workspace_search_supports_search_path(workspace_root: &Path, search_path: &Path) -> bool {
    let Ok(workspace_root) = dunce::canonicalize(workspace_root) else {
        return false;
    };
    let Ok(search_path) = dunce::canonicalize(search_path) else {
        return false;
    };

    search_path.starts_with(workspace_root)
}

fn render_glob_result_text(
    pattern: &str,
    matches: &[String],
    total_matches: Option<usize>,
    truncated: bool,
    matches_relative_to: Option<&str>,
) -> String {
    let relative_note = matches_relative_to
        .map(|base| format!(" relative to {base}"))
        .unwrap_or_default();

    if matches.is_empty() {
        return format!("No files found matching pattern '{pattern}'{relative_note}");
    }

    let result_text = matches.join("\n");
    if !truncated {
        return format!(
            "Found {} matches{relative_note}\n<matches>\n{result_text}\n</matches>",
            matches.len()
        );
    }

    let count_text = match total_matches {
        Some(total) => format!(
            "Showing {} of {} matches{relative_note}",
            matches.len(),
            total
        ),
        None => format!("Showing {} matches{relative_note}", matches.len()),
    };

    format!(
        "{count_text} (This list is truncated and not complete. Narrow the pattern or search a more specific path to see the rest.)\n<matches>\n{result_text}\n</matches>"
    )
}

fn display_path(path: &Path) -> String {
    normalize_path(path)
}

fn resolve_effective_glob_scope(search_path: &Path, pattern: &str) -> (PathBuf, String) {
    derive_walk_root(search_path, pattern)
}

fn relative_base_note(original_search_path: &Path, walk_root: &Path) -> Option<String> {
    (walk_root != original_search_path).then(|| display_path(walk_root))
}

fn remote_shell_result_relative_base(
    original_search_path: &str,
    remote_walk_root: &Path,
) -> Option<String> {
    relative_base_note(Path::new(original_search_path), remote_walk_root)
}

fn relative_json_field(base_note: Option<&str>) -> Value {
    base_note.map_or(Value::Null, |base| json!(base))
}

fn result_relative_base_note(
    matches_relative_to: &str,
    original_search_path: &Path,
) -> Option<String> {
    let original = display_path(original_search_path);
    (matches_relative_to != original).then(|| matches_relative_to.to_string())
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "Glob"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Fast file pattern matching tool
- Supports glob patterns like "**/*.js" or "src/**/*.ts"
- Returns matching file paths
- Use this tool when you need to find files by name patterns
- The path parameter may be workspace-relative, an absolute path inside the current workspace, or an exact `bitfun://...` URI returned by another tool
- An absolute pattern is searched from its static parent directory (for example, `C:/logs/*.log` searches `C:/logs` with `*.log`)
- Omit path to search the current workspace. Do not use placeholder paths such as `/workspace`.
- Returns up to 100 matching paths. Narrow the pattern or search a more specific path if the result is truncated.
- You can call multiple tools in a single response. It is always better to speculatively perform multiple searches in parallel if they are potentially useful.
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
                    "description": "The directory to search in. Omit this field to search the current workspace. Do not enter \"undefined\", \"null\", host roots, or placeholder paths such as /workspace."
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
        let limit = GLOB_RESULT_LIMIT;
        let mut effective_glob = resolve_effective_glob_search(
            &resolved.resolved_path,
            pattern,
            resolved.uses_remote_workspace_backend(),
        );

        if resolved.uses_remote_workspace_backend() {
            effective_glob.search_path =
                context.resolve_workspace_tool_path(&effective_glob.search_path)?;
        }

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
                    let resolved_path = PathBuf::from(&effective_glob.search_path);
                    let (_walk_root, effective_pattern) =
                        resolve_effective_glob_scope(&resolved_path, &effective_glob.pattern);
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
                            pattern: effective_glob.pattern.clone(),
                            limit,
                        })
                        .await
                        .map_err(BitFunError::tool)?;

                    let match_count = glob_result.paths.len();
                    let total_matches = glob_result.total_matches;
                    let truncated = glob_result.truncated;
                    let result_relative_base = result_relative_base_note(
                        &glob_result.matches_relative_to,
                        &PathBuf::from(&resolved.resolved_path),
                    );
                    let result_text = render_glob_result_text(
                        pattern,
                        &glob_result.paths,
                        total_matches,
                        truncated,
                        result_relative_base.as_deref(),
                    );

                    Ok::<Vec<ToolResult>, BitFunError>(vec![ToolResult::Result {
                        data: json!({
                            "pattern": pattern,
                            "path": resolved.logical_path,
                            "effective_pattern": effective_pattern,
                            "matches_relative_to": relative_json_field(result_relative_base.as_deref()),
                            "matches": glob_result.paths,
                            "match_count": match_count,
                            "total_matches": total_matches,
                            "truncated": truncated,
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

            let search_dir = effective_glob.search_path.clone();
            let search_dir_path = PathBuf::from(&search_dir);
            let (remote_walk_root, _remote_pattern) =
                resolve_effective_glob_scope(&search_dir_path, &effective_glob.pattern);
            let relative_base =
                remote_shell_result_relative_base(&resolved.resolved_path, &remote_walk_root);
            let (_stdout, _stderr, exit_code) = ws_shell
                .exec("command -v rg >/dev/null 2>&1", Some(5_000))
                .await
                .map_err(|e| BitFunError::tool(format!("Failed to detect rg on remote: {}", e)))?;

            let (remote_cmd, exact_total) = if exit_code == 0 {
                info!(
                    "Glob backend selected: backend=remote_rg, search_path={}, pattern={}",
                    search_dir, pattern
                );
                (
                    build_remote_rg_command(&search_dir, &effective_glob.pattern),
                    true,
                )
            } else {
                info!(
                    "Glob backend selected: backend=remote_find, reason=rg_not_found, search_path={}, pattern={}",
                    search_dir, pattern
                );
                (
                    build_remote_find_command(&search_dir, &effective_glob.pattern, limit),
                    false,
                )
            };

            let (stdout, _stderr, _exit_code) = ws_shell
                .exec(&remote_cmd, Some(30_000))
                .await
                .map_err(|e| {
                    BitFunError::tool(format!("Failed to glob on remote with rg: {}", e))
                })?;

            let remote_walk_root_str = remote_walk_root.to_string_lossy().to_string();
            let glob_result =
                collect_remote_glob_result(&remote_walk_root_str, &stdout, limit, exact_total);
            let total_matches = glob_result.total_matches;
            let truncated = glob_result.truncated;
            let matches = glob_result
                .matches
                .into_iter()
                .map(|path| normalize_path(&path))
                .collect::<Vec<_>>();
            let match_count = matches.len();
            let result_text = render_glob_result_text(
                pattern,
                &matches,
                total_matches,
                truncated,
                relative_base.as_deref(),
            );

            return Ok(vec![ToolResult::Result {
                data: json!({
                    "pattern": pattern,
                    "path": resolved.logical_path,
                    "matches_relative_to": relative_json_field(relative_base.as_deref()),
                    "matches": matches,
                    "match_count": match_count,
                    "total_matches": total_matches,
                    "truncated": truncated
                }),
                result_for_assistant: Some(result_text),
                image_attachments: None,
            }]);
        }

        let resolved_str = resolved.resolved_path.clone();
        let effective_search_path = PathBuf::from(&effective_glob.search_path);
        let workspace_root = context
            .workspace
            .as_ref()
            .map(|workspace| PathBuf::from(workspace.root_path_string()));

        if let Some(workspace_root) = workspace_root.filter(|workspace_root| {
            workspace_search_supports_search_path(workspace_root, &effective_search_path)
        }) {
            if workspace_search_runtime_available().await {
                if let Some(search_service) = get_global_workspace_search_service() {
                    let resolved_path = effective_search_path.clone();
                    let (_walk_root, effective_pattern) =
                        resolve_effective_glob_scope(&resolved_path, &effective_glob.pattern);
                    let workspace_glob_result = search_service
                        .glob(GlobSearchRequest {
                            repo_root: workspace_root.clone(),
                            search_path: (resolved_path != workspace_root).then_some(resolved_path),
                            pattern: effective_glob.pattern.clone(),
                            limit,
                        })
                        .await;

                    match workspace_glob_result {
                        Ok(glob_result) => {
                            let match_count = glob_result.paths.len();
                            let total_matches = glob_result.total_matches;
                            let truncated = glob_result.truncated;
                            let result_relative_base = result_relative_base_note(
                                &glob_result.matches_relative_to,
                                &PathBuf::from(&resolved_str),
                            );
                            let result_text = render_glob_result_text(
                                pattern,
                                &glob_result.paths,
                                total_matches,
                                truncated,
                                result_relative_base.as_deref(),
                            );

                            return Ok(vec![ToolResult::Result {
                                data: json!({
                                    "pattern": pattern,
                                    "path": resolved_str,
                                    "effective_pattern": effective_pattern,
                                    "matches_relative_to": relative_json_field(result_relative_base.as_deref()),
                                    "matches": glob_result.paths,
                                    "match_count": match_count,
                                    "total_matches": total_matches,
                                    "truncated": truncated,
                                    "repo_phase": glob_result.repo_status.phase,
                                    "rebuild_recommended": glob_result.repo_status.rebuild_recommended
                                }),
                                result_for_assistant: Some(result_text),
                                image_attachments: None,
                            }]);
                        }
                        Err(error) => {
                            warn!(
                                "Glob tool workspace-search failed; falling back to local rg: {}",
                                error
                            );
                        }
                    }
                }
            }
        }

        let resolved_str_for_rg = effective_glob.search_path.clone();
        let pattern_for_rg = effective_glob.pattern.clone();
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
            .map(|path| normalize_path(&path))
            .collect::<Vec<_>>();

        let total_matches = glob_result.total_matches;
        let truncated = glob_result.truncated;
        let match_count = matches.len();
        let original_search_path = PathBuf::from(&resolved_str);
        let relative_base = relative_base_note(&original_search_path, &glob_result.walk_root);
        let result_text = render_glob_result_text(
            pattern,
            &matches,
            total_matches,
            truncated,
            relative_base.as_deref(),
        );

        let result = ToolResult::Result {
            data: json!({
                "pattern": pattern,
                "path": resolved.logical_path,
                "matches_relative_to": relative_json_field(relative_base.as_deref()),
                "matches": matches,
                "match_count": match_count,
                "total_matches": total_matches,
                "truncated": truncated
            }),
            result_for_assistant: Some(result_text),
            image_attachments: None,
        };

        Ok(vec![result])
    }
}

#[cfg(test)]
mod tests {
    use super::{
        remote_shell_result_relative_base, render_glob_result_text, resolve_effective_glob_search,
        workspace_search_supports_search_path, GlobTool,
    };
    use crate::agentic::tools::framework::{Tool, ToolUseContext};
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::agentic::WorkspaceBinding;
    use serde_json::json;
    use std::collections::HashMap;
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

    fn remote_context(root: &str) -> ToolUseContext {
        let session_identity =
            crate::service::remote_ssh::workspace_state::workspace_session_identity(
                root,
                Some("conn-1"),
                Some("ssh.dev"),
            )
            .expect("remote identity");
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: Some(WorkspaceBinding::new_remote(
                None,
                PathBuf::from(root),
                "conn-1".to_string(),
                "Dev SSH".to_string(),
                session_identity,
            )),
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    #[test]
    fn input_schema_does_not_expose_model_controlled_limit() {
        let schema = GlobTool::new().input_schema();
        assert!(schema["properties"].get("pattern").is_some());
        assert!(schema["properties"].get("path").is_some());
        assert!(schema["properties"].get("limit").is_none());
    }

    #[test]
    fn renders_truncation_note_from_backend_metadata_only() {
        let matches = (0..100)
            .map(|idx| format!("file{idx}.txt"))
            .collect::<Vec<_>>();

        let exact_limit_complete =
            render_glob_result_text("*.txt", &matches, Some(100), false, None);
        assert!(!exact_limit_complete.contains("[truncated:"));
        assert!(exact_limit_complete.starts_with("Found 100 matches\n<matches>\n"));

        let exact_truncated = render_glob_result_text("*.txt", &matches, Some(101), true, None);
        assert!(exact_truncated.starts_with("Showing 100 of 101 matches (This list is truncated"));
        assert!(exact_truncated.contains("not complete"));
        assert!(exact_truncated.contains("\n<matches>\nfile0.txt"));
        assert!(exact_truncated.ends_with("</matches>"));

        let unknown_total = render_glob_result_text("*.txt", &matches, None, true, None);
        assert!(unknown_total.starts_with("Showing 100 matches (This list is truncated"));

        let relative_to =
            render_glob_result_text("*.txt", &matches[..1], Some(1), false, Some("/repo/src"));
        assert!(relative_to.starts_with("Found 1 matches relative to /repo/src\n<matches>\n"));
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
    fn absolute_pattern_uses_its_static_parent_as_the_search_path() {
        let root = make_temp_dir("absolute-pattern");
        let transcript_dir = root.join("terminal-transcripts");
        fs::create_dir_all(&transcript_dir).unwrap();

        let pattern = format!("{}/*.log", transcript_dir.display());
        let effective = resolve_effective_glob_search("E:/workspace", &pattern, false);

        assert_eq!(PathBuf::from(effective.search_path), transcript_dir);
        assert_eq!(effective.pattern, "*.log");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn absolute_pattern_searches_its_external_parent_with_local_rg() {
        let workspace_root = make_temp_dir("absolute-pattern-workspace");
        let transcript_dir = make_temp_dir("absolute-pattern-transcripts");
        fs::write(transcript_dir.join("session.log"), "transcript").unwrap();

        let pattern = format!("{}/*.log", transcript_dir.display());
        let effective =
            resolve_effective_glob_search(&workspace_root.to_string_lossy(), &pattern, false);
        let result = execute_local_glob(LocalGlobRequest {
            search_path: PathBuf::from(effective.search_path),
            pattern: effective.pattern,
            limit: 100,
        })
        .unwrap();

        assert_eq!(
            result
                .matches
                .into_iter()
                .map(|path| normalize_path(&path))
                .collect::<Vec<_>>(),
            vec!["session.log"]
        );

        let _ = fs::remove_dir_all(workspace_root);
        let _ = fs::remove_dir_all(transcript_dir);
    }

    #[test]
    fn remote_absolute_pattern_uses_a_posix_search_path() {
        let effective = resolve_effective_glob_search("/workspace", "/var/log/*.log", true);

        assert_eq!(effective.search_path, "/var/log");
        assert_eq!(effective.pattern, "*.log");
    }

    #[tokio::test]
    async fn remote_absolute_pattern_outside_workspace_is_rejected() {
        let error = GlobTool::new()
            .call_impl(
                &json!({ "pattern": "/etc/*.conf" }),
                &remote_context("/workspace"),
            )
            .await
            .expect_err("external remote pattern should be rejected");

        assert!(error
            .to_string()
            .contains("resolves outside current workspace"));
    }

    #[test]
    fn remote_absolute_pattern_reports_its_effective_result_base() {
        let effective = resolve_effective_glob_search("/workspace", "/workspace/src/*.rs", true);
        let base =
            remote_shell_result_relative_base("/workspace", &PathBuf::from(effective.search_path));

        assert_eq!(base.as_deref(), Some("/workspace/src"));
    }

    #[test]
    fn workspace_search_is_limited_to_the_workspace_root() {
        let workspace_root = make_temp_dir("workspace-search-root");
        let workspace_child = workspace_root.join("src");
        fs::create_dir_all(&workspace_child).unwrap();
        let external_root = make_temp_dir("workspace-search-external");

        assert!(workspace_search_supports_search_path(
            &workspace_root,
            &workspace_child
        ));
        assert!(!workspace_search_supports_search_path(
            &workspace_root,
            &external_root
        ));

        let _ = fs::remove_dir_all(workspace_root);
        let _ = fs::remove_dir_all(external_root);
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
        assert!(matches.iter().any(|path| path == "src/lib.rs"));
        assert!(matches.iter().any(|path| path == "tests/mod.rs"));
        assert!(!matches.iter().any(|path| path == "src/deep/mod.rs"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn static_glob_prefix_results_are_relative_to_walk_root() {
        let root = make_temp_dir("relative-walk-root");
        fs::create_dir_all(root.join("src/deep")).unwrap();
        fs::write(root.join("src/lib.rs"), "").unwrap();
        fs::write(root.join("src/deep/mod.rs"), "").unwrap();

        let result = execute_local_glob(LocalGlobRequest {
            search_path: root.clone(),
            pattern: "src/*.rs".to_string(),
            limit: 10,
        })
        .unwrap();
        let matches = result
            .matches
            .into_iter()
            .map(|path| normalize_path(&path))
            .collect::<Vec<_>>();
        let expected_walk_root = fs::canonicalize(&root).unwrap().join("src");

        assert_eq!(
            normalize_path(&result.walk_root),
            normalize_path(&expected_walk_root)
        );
        assert!(matches.iter().any(|path| path == "lib.rs"));
        assert!(matches.iter().any(|path| path == "deep/mod.rs"));
        assert!(matches.iter().all(|path| !path.starts_with("src/")));

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
