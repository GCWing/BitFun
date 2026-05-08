//! LS tool implementation
//!
//! Provides functionality similar to Unix ls command for listing files and subdirectories in a directory

use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::workspace_paths::is_bitfun_runtime_uri;
use crate::service::filesystem::{format_directory_listing, list_directory_entries};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use chrono::{DateTime, Local};
use serde_json::{json, Value};
use std::path::Path;
use std::time::SystemTime;

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// LS tool - list directory tree
pub struct LSTool {
    /// Default maximum number of entries to return
    default_limit: usize,
}

impl Default for LSTool {
    fn default() -> Self {
        Self::new()
    }
}

impl LSTool {
    pub fn new() -> Self {
        Self { default_limit: 200 }
    }
}

/// Format system time as readable string
fn format_time(time: SystemTime) -> String {
    let datetime: DateTime<Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

#[async_trait]
impl Tool for LSTool {
    fn name(&self) -> &str {
        "LS"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Recursively lists files and directories in a given path.

Usage:
- The path parameter must be relative to the current workspace, an absolute path inside the current workspace, or an exact `bitfun://runtime/...` URI returned by another tool
- Do not list host roots such as `/`, `/Users`, `/home`, or placeholder paths such as `/workspace`
- Hidden files (files starting with '.') are automatically excluded
- Results are sorted by modification time (newest first)"#
            .to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory to list. Use a workspace-relative path, an absolute path inside the current workspace, or an exact bitfun://runtime URI returned by another tool."
                },
                "limit": {
                    "type": "number",
                    "description": "The maximum number of entries to return. Defaults to 100."
                },
            },
            "required": ["path"],
            "additionalProperties": false
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

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
            if path.is_empty() {
                return ValidationResult {
                    result: false,
                    message: Some("path cannot be empty".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }

            let resolved = match context.map(|ctx| ctx.resolve_tool_path(path)) {
                Some(Ok(value)) => value,
                Some(Err(err)) => {
                    return ValidationResult {
                        result: false,
                        message: Some(err.to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                }
                None => {
                    if is_bitfun_runtime_uri(path) {
                        return ValidationResult {
                            result: false,
                            message: Some(
                                "Tool context is required to resolve bitfun runtime URIs"
                                    .to_string(),
                            ),
                            error_code: Some(400),
                            meta: None,
                        };
                    }

                    let local_path = Path::new(path);
                    if !local_path.is_absolute() {
                        return ValidationResult {
                            result: false,
                            message: Some(format!("path must be an absolute path, got: {}", path)),
                            error_code: Some(400),
                            meta: None,
                        };
                    }

                    if !local_path.exists() {
                        return ValidationResult {
                            result: false,
                            message: Some(format!("Directory does not exist: {}", path)),
                            error_code: Some(404),
                            meta: None,
                        };
                    }

                    if !local_path.is_dir() {
                        return ValidationResult {
                            result: false,
                            message: Some(format!("Path is not a directory: {}", path)),
                            error_code: Some(400),
                            meta: None,
                        };
                    }

                    return ValidationResult {
                        result: true,
                        message: None,
                        error_code: None,
                        meta: None,
                    };
                }
            };

            if !resolved.uses_remote_workspace_backend() {
                let local_path = Path::new(&resolved.resolved_path);
                if !local_path.exists() {
                    return ValidationResult {
                        result: false,
                        message: Some(format!(
                            "Directory does not exist: {}",
                            resolved.logical_path
                        )),
                        error_code: Some(404),
                        meta: None,
                    };
                }

                if !local_path.is_dir() {
                    return ValidationResult {
                        result: false,
                        message: Some(format!(
                            "Path is not a directory: {}",
                            resolved.logical_path
                        )),
                        error_code: Some(400),
                        meta: None,
                    };
                }
            }
        } else {
            return ValidationResult {
                result: false,
                message: Some("path is required".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
            if options.verbose {
                format!("Listing directory: {}", path)
            } else {
                format!("List {}", path)
            }
        } else {
            "Listing directory".to_string()
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("path is required".to_string()))?;

        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(self.default_limit);

        let resolved = context.resolve_tool_path(path)?;

        // Remote workspace path: execute ls via SSH shell
        if resolved.uses_remote_workspace_backend() {
            let ws_shell = context.ws_shell().ok_or_else(|| {
                BitFunError::tool("Workspace shell not available for remote LS".to_string())
            })?;

            let ls_cmd = format!(
                "find {} -maxdepth 1 -not -name '.*' -not -path {} | head -n {} | sort",
                shell_escape(&resolved.resolved_path),
                shell_escape(&resolved.resolved_path),
                limit + 1
            );

            let (stdout, _stderr, _exit_code) =
                ws_shell.exec(&ls_cmd, Some(15_000)).await.map_err(|e| {
                    BitFunError::tool(format!("Failed to list remote directory: {}", e))
                })?;

            let mut file_lines = Vec::new();
            let mut dir_lines = Vec::new();

            for line in stdout.lines().filter(|l| !l.is_empty()) {
                let name = Path::new(line)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| line.to_string());
                let is_dir = line.ends_with('/');
                if is_dir || name.is_empty() {
                    dir_lines.push((name, line.to_string()));
                } else {
                    file_lines.push((name, line.to_string()));
                }
            }

            // Use a simpler stat-based listing for the text output
            let stat_cmd = format!(
                "ls -la --time-style=long-iso {} 2>/dev/null || ls -la {}",
                shell_escape(&resolved.resolved_path),
                shell_escape(&resolved.resolved_path)
            );
            let (ls_output, _, _) = ws_shell.exec(&stat_cmd, Some(15_000)).await.map_err(|e| {
                BitFunError::tool(format!("Failed to list remote directory: {}", e))
            })?;

            let result_text = format!(
                "Directory listing: {}\n\n{}",
                resolved.logical_path,
                ls_output.trim()
            );

            let entries_json: Vec<Value> = stdout
                .lines()
                .filter(|l| !l.is_empty())
                .map(|line| {
                    let name = Path::new(line)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| line.to_string());
                    json!({
                        "name": name,
                        "path": line,
                        "is_dir": line.ends_with('/'),
                    })
                })
                .collect();

            let total_entries = entries_json.len();
            let result = ToolResult::Result {
                data: json!({
                    "path": resolved.logical_path,
                    "entries": entries_json,
                    "total": total_entries,
                    "limit": limit,
                    "is_remote": true
                }),
                result_for_assistant: Some(result_text),
                image_attachments: None,
            };
            return Ok(vec![result]);
        }

        // Local: original implementation
        let entries =
            list_directory_entries(&resolved.resolved_path, limit).map_err(BitFunError::tool)?;

        let entries_json = entries
            .iter()
            .filter(|entry| entry.depth == 1)
            .map(|entry| {
                let entry_path = resolved
                    .logical_child_path(&entry.path)
                    .unwrap_or_else(|| entry.path.to_string_lossy().to_string());
                json!({
                    "name": entry.path.file_name().unwrap_or_default().to_string_lossy(),
                    "path": entry_path,
                    "is_dir": entry.is_dir,
                    "modified_time": format_time(entry.modified_time)
                })
            })
            .collect::<Vec<Value>>();
        let total_entries = entries.len();

        let mut result_text = format_directory_listing(&entries, &resolved.resolved_path);
        if resolved.logical_path != resolved.resolved_path {
            let physical_header = resolved.resolved_path.replace('\\', "/");
            let logical_header = resolved.logical_path.replace('\\', "/");
            result_text = result_text.replacen(&physical_header, &logical_header, 1);
        }
        if total_entries == 0 {
            result_text.push_str("\n(no entries found)");
        } else if total_entries >= limit {
            result_text.push_str(&format!("\n(showing up to {} entries)", limit));
        }

        let result = ToolResult::Result {
            data: json!({
                "path": resolved.logical_path,
                "entries": entries_json,
                "total": total_entries,
                "limit": limit
            }),
            result_for_assistant: Some(result_text),
            image_attachments: None,
        };

        Ok(vec![result])
    }
}
