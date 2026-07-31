//! CheckCppFiles tool — static C/C++ syntax check on native source files.
//!
//! Calls the "check" MCP tool on the `deveco-mcp` server via the shared
//! `deveco_mcp_check` helper, which auto-provisions and (re)starts the
//! `devecocli serve mcp` server on demand when it is not connected.

use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;

const CPP_EXTENSIONS: &[&str] = &["c", "cc", "cpp", "cxx", "h", "hh", "hpp", "hxx"];

/// CheckCppFiles tool — static C/C++ syntax check on native source files.
pub struct CheckCppFilesTool;

impl Default for CheckCppFilesTool {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckCppFilesTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for CheckCppFilesTool {
    fn name(&self) -> &str {
        "check_cpp_files"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Run static C/C++ syntax check on native source files via devecocli MCP.

Use for native modules (.c, .cc, .cpp, .cxx, .h, .hh, .hpp, .hxx) before a full native build.
The `deveco-mcp` MCP server (`devecocli serve mcp`) is auto-provisioned and started on demand when not already connected, so no manual MCP configuration is required.

Provide absolute or workspace-relative paths to source files."#.to_string())
    }

    fn short_description(&self) -> String {
        "Run static C/C++ syntax check on native source files.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "C/C++ source or header file paths to check."
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
                message: Some(
                    "files must be a non-empty array of C/C++ file paths".to_string(),
                ),
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
        format!("C/C++ check on {} file(s)", count)
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

        let cpp_files: Vec<String> = all_files
            .iter()
            .filter(|f| {
                Path::new(f)
                    .extension()
                    .map(|ext| {
                        let ext_lower = ext.to_string_lossy().to_lowercase();
                        CPP_EXTENSIONS.contains(&ext_lower.as_str())
                    })
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        if cpp_files.is_empty() {
            return Err(BitFunError::tool(
                "No C/C++ files provided. All files were filtered out.".to_string(),
            ));
        }

        let result = super::deveco_mcp_check::call_deveco_mcp_check(&cpp_files, context).await?;

        Ok(vec![ToolResult::Result {
            data: json!({
                "files": cpp_files,
                "success": true,
            }),
            result_for_assistant: Some(result),
            image_attachments: None,
        }])
    }
}
