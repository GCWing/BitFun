use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::workspace_paths::resolve_workspace_tool_path;
use crate::service::snapshot::manager::get_snapshot_manager_for_workspace;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use log::debug;
use log::warn;
use serde_json::{json, Value};
use similar::ChangeTag;
use similar::TextDiff;
use std::fs;
use std::path::Path;

/// Get file diff tool
///
/// Priority order:
/// 1. Baseline snapshot diff (if exists)
/// 2. Return full file content
pub struct GetFileDiffTool;

impl Default for GetFileDiffTool {
    fn default() -> Self {
        Self::new()
    }
}

impl GetFileDiffTool {
    pub fn new() -> Self {
        Self
    }

    /// Generate unified diff format
    fn generate_unified_diff(&self, old: &str, new: &str) -> String {
        let diff = TextDiff::from_lines(old, new);
        diff.unified_diff().to_string()
    }

    /// Calculate diff statistics
    fn calculate_diff_stats(&self, old: &str, new: &str) -> (usize, usize) {
        let diff = TextDiff::from_lines(old, new);
        let mut additions = 0;
        let mut deletions = 0;

        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Delete => deletions += 1,
                ChangeTag::Insert => additions += 1,
                ChangeTag::Equal => {}
            }
        }

        (additions, deletions)
    }

    /// Try to get diff from baseline
    async fn try_baseline_diff(
        &self,
        file_path: &Path,
        workspace_root: Option<&Path>,
    ) -> Option<BitFunResult<Value>> {
        let snapshot_manager = workspace_root.and_then(get_snapshot_manager_for_workspace)?;

        // Get snapshot service
        let snapshot_service = snapshot_manager.get_snapshot_service();
        let snapshot_service = snapshot_service.read().await;

        // Get baseline snapshot ID
        let baseline_id = snapshot_service.get_baseline_snapshot_id(file_path).await;

        if let Some(id) = baseline_id {
            debug!("GetFileDiff tool found baseline snapshot: {}", id);

            // Read current file content
            let current_content = fs::read_to_string(file_path).ok()?;

            // Read baseline content
            let baseline_content = match snapshot_service.get_snapshot_content(&id).await {
                Ok(content) => content,
                Err(e) => {
                    warn!("GetFileDiff tool failed to read baseline content: {}", e);
                    return None;
                }
            };

            // Generate diff
            let diff_content = self.generate_unified_diff(&baseline_content, &current_content);

            // Calculate statistics
            let (additions, deletions) =
                self.calculate_diff_stats(&baseline_content, &current_content);

            return Some(Ok(json!({
                "file_path": file_path,
                "diff_type": "baseline",
                "diff_format": "unified",
                "diff_content": diff_content,
                "original_content": baseline_content,
                "modified_content": current_content,
                "stats": {
                    "additions": additions,
                    "deletions": deletions
                },
                "message": format!("Diff from baseline snapshot (ID: {})", id)
            })));
        }

        None
    }

    /// Return full file content
    fn return_full_content(&self, file_path: &Path) -> BitFunResult<Value> {
        let content = fs::read_to_string(file_path)
            .map_err(|e| BitFunError::tool(format!("Failed to read file: {}", e)))?;

        let total_lines = content.lines().count();

        Ok(json!({
            "file_path": file_path,
            "diff_type": "full",
            "diff_format": "unified",
            "diff_content": content.clone(),
            "original_content": "",
            "modified_content": content,
            "stats": {
                "additions": 0,
                "deletions": 0,
                "total_lines": total_lines
            },
            "message": "File full content (no baseline snapshot found)"
        }))
    }
}

#[async_trait]
impl Tool for GetFileDiffTool {
    fn name(&self) -> &str {
        "GetFileDiff"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Gets the diff for a file, showing changes from its baseline snapshot when available.

This tool compares the current file content against:
1. Baseline snapshot (if available) - the state before AI modifications
2. Full file content (if no baseline snapshot)

Usage:
- The file_path parameter must be an absolute path, not a relative path.
- The diff is returned in unified diff format, showing additions (+) and deletions (-).
- The response includes diff_type indicating the source: "baseline" or "full".
- The response includes stats for additions and deletions.
- This tool is read-only and safe to use for code review and analysis.
"#
            .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to get diff for"
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

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        if let Some(file_path) = input.get("file_path").and_then(|v| v.as_str()) {
            if file_path.is_empty() {
                return ValidationResult {
                    result: false,
                    message: Some("file_path cannot be empty".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }

            let is_remote = context.map(|c| c.is_remote()).unwrap_or(false);

            let root_owned =
                context.and_then(|c| c.workspace.as_ref().map(|w| w.root_path_string()));
            let resolved_path =
                match resolve_workspace_tool_path(file_path, root_owned.as_deref(), is_remote) {
                    Ok(p) => p,
                    Err(e) => {
                        return ValidationResult {
                            result: false,
                            message: Some(e.to_string()),
                            error_code: Some(400),
                            meta: None,
                        };
                    }
                };

            if !is_remote {
                let path = Path::new(&resolved_path);
                if !path.exists() {
                    return ValidationResult {
                        result: false,
                        message: Some(format!("File does not exist: {}", resolved_path)),
                        error_code: Some(404),
                        meta: None,
                    };
                }

                if !path.is_file() {
                    return ValidationResult {
                        result: false,
                        message: Some(format!("Path is not a file: {}", resolved_path)),
                        error_code: Some(400),
                        meta: None,
                    };
                }
            }
        } else {
            return ValidationResult {
                result: false,
                message: Some("file_path is required".to_string()),
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
        if let Some(file_path) = input.get("file_path").and_then(|v| v.as_str()) {
            if options.verbose {
                format!("Getting diff for file: {}", file_path)
            } else {
                format!("GetFileDiff {}", file_path)
            }
        } else {
            "Getting file diff".to_string()
        }
    }

    fn render_tool_result_message(&self, output: &Value) -> String {
        if let Some(diff_type) = output.get("diff_type").and_then(|v| v.as_str()) {
            if let Some(message) = output.get("message").and_then(|v| v.as_str()) {
                format!("{} ({})", message, diff_type)
            } else {
                diff_type.to_string()
            }
        } else {
            "File diff retrieved".to_string()
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

        let resolved_path = context.resolve_workspace_tool_path(file_path)?;

        debug!(
            "GetFileDiff tool starting diff retrieval for file: {:?}",
            resolved_path
        );

        if context.is_remote() {
            let ws_fs = context.ws_fs().ok_or_else(|| {
                BitFunError::tool("Workspace file system not available for remote diff".to_string())
            })?;
            let content = ws_fs
                .read_file_text(&resolved_path)
                .await
                .map_err(|e| BitFunError::tool(format!("Failed to read file: {}", e)))?;
            let total_lines = content.lines().count();
            let data = json!({
                "file_path": resolved_path,
                "diff_type": "full",
                "diff_format": "unified",
                "diff_content": content.clone(),
                "original_content": "",
                "modified_content": content,
                "stats": {
                    "additions": 0,
                    "deletions": 0,
                    "total_lines": total_lines
                },
                "message": "File full content on remote workspace (baseline diff not available locally)"
            });
            let result_for_assistant = self.render_tool_result_message(&data);
            return Ok(vec![ToolResult::Result {
                data,
                result_for_assistant: Some(result_for_assistant),
                image_attachments: None,
            }]);
        }

        let path = Path::new(&resolved_path);
        if let Some(result) = self.try_baseline_diff(path, context.workspace_root()).await {
            match result {
                Ok(data) => {
                    debug!("GetFileDiff tool using baseline diff");
                    let result_for_assistant = self.render_tool_result_message(&data);
                    return Ok(vec![ToolResult::Result {
                        data,
                        result_for_assistant: Some(result_for_assistant),
                        image_attachments: None,
                    }]);
                }
                Err(e) => {
                    warn!(
                        "GetFileDiff tool baseline diff failed: {}, returning full content",
                        e
                    );
                }
            }
        }

        debug!("GetFileDiff tool returning full file content");
        let data = self.return_full_content(path)?;
        let result_for_assistant = self.render_tool_result_message(&data);

        Ok(vec![ToolResult::Result {
            data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}
