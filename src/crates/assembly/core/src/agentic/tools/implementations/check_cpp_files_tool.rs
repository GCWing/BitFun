//! CheckCppFiles tool — static C/C++ syntax check on native source files.
//!
//! Calls the "check" MCP tool on the `deveco-mcp` server (configured in app
//! settings) via BitFun's existing `MCPServerManager` infrastructure.

use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::service::mcp::get_global_mcp_service;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_services_integrations::mcp::protocol::MCPToolResultContent;
use serde_json::{json, Value};
use std::path::Path;

const MCP_SERVER_ID: &str = "deveco-mcp";
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
Requires the `deveco-mcp` MCP server configured in app settings with `devecocli serve mcp`.

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
        _context: &ToolUseContext,
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

        let result = call_mcp_check(&cpp_files).await?;

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

/// Get the `deveco-mcp` connection from the global MCP service and call the
/// "check" tool. The server must be configured in app settings.
async fn call_mcp_check(files: &[String]) -> BitFunResult<String> {
    let mcp_service = get_global_mcp_service().ok_or_else(|| {
        BitFunError::tool("MCP service is not initialized".to_string())
    })?;

    let connection = mcp_service
        .server_manager()
        .get_connection(MCP_SERVER_ID)
        .await
        .ok_or_else(|| {
            BitFunError::tool(format!(
                "MCP server '{}' is not connected. Configure it in app settings with command `devecocli` and args `[\"serve\", \"mcp\"]`.",
                MCP_SERVER_ID
            ))
        })?;

    let result = connection
        .call_tool("check", Some(json!({ "files": files })))
        .await
        .map_err(|e| BitFunError::tool(format!("MCP check call failed: {}", e)))?;

    if result.is_error {
        return Err(BitFunError::tool(extract_text(&result)));
    }

    Ok(extract_text(&result))
}

/// Extract concatenated text content from an MCP tool result.
fn extract_text(result: &bitfun_services_integrations::mcp::protocol::MCPToolResult) -> String {
    if let Some(content) = &result.content {
        let texts: Vec<String> = content
            .iter()
            .filter_map(|c| match c {
                MCPToolResultContent::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect();
        if !texts.is_empty() {
            return texts.join("\n");
        }
    }
    serde_json::to_string_pretty(result).unwrap_or_default()
}
