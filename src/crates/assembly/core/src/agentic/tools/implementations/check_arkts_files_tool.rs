//! CheckArktsFiles tool — static ArkTS syntax check on .ets files.
//!
//! Spawns `devecocli serve mcp` via the shared `deveco_mcp` module and calls
//! the MCP "check" tool. No JS callback bridge required.

use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::implementations::deveco_mcp;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;

/// CheckArktsFiles tool — static ArkTS syntax check on .ets files.
pub struct CheckArktsFilesTool;

impl Default for CheckArktsFilesTool {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckArktsFilesTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for CheckArktsFilesTool {
    fn name(&self) -> &str {
        "check_arkts_files"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Run static ArkTS syntax check (ArkTS-Check) on .ets files via devecocli MCP.

Use before a full build for fast feedback on syntax and type errors.
Requires devecocli installed; uses the shared deveco-mcp connection warmed on workspace open or switch_cwd.

Provide absolute or workspace-relative paths to .ets files."#
            .to_string())
    }

    fn short_description(&self) -> String {
        "Run static ArkTS syntax check on .ets files.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "ETS file paths to check, e.g. [\"src/main/ets/pages/Index.ets\"]."
                }
            },
            "required": ["files"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        match input.get("files").and_then(|v| v.as_array()) {
            Some(files) if !files.is_empty() => ValidationResult {
                result: true,
                message: None,
                error_code: None,
                meta: None,
            },
            _ => ValidationResult {
                result: false,
                message: Some("files must be a non-empty array of .ets file paths".to_string()),
                error_code: Some(400),
                meta: None,
            },
        }
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let count = input
            .get("files")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        format!("ArkTS check on {} file(s)", count)
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let all_files: Vec<String> = input
            .get("files")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // Filter to .ets files only
        let ets_files: Vec<String> = all_files
            .iter()
            .filter(|f| {
                Path::new(f)
                    .extension()
                    .map(|ext| ext.eq_ignore_ascii_case("ets"))
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        if ets_files.is_empty() {
            return Err(BitFunError::tool(
                "No .ets files provided. All files were filtered out.".to_string(),
            ));
        }

        // Resolve project path from workspace context
        let project_path = resolve_project_path(context);

        let result = deveco_mcp::run_deveco_check(&ets_files, &project_path)
            .await
            .map_err(BitFunError::tool)?;

        Ok(vec![ToolResult::Result {
            data: json!({
                "files": ets_files,
                "project_path": project_path,
                "success": true,
            }),
            result_for_assistant: Some(result),
            image_attachments: None,
        }])
    }
}

/// Resolve the current project path from the tool context's workspace root,
/// falling back to the process working directory.
fn resolve_project_path(context: &ToolUseContext) -> String {
    if let Some(root) = context.workspace_root() {
        return root.to_string_lossy().to_string();
    }
    std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string())
}
