//! CheckCppFiles tool — runs static C/C++ syntax check on native source files.
//!
//! Bridges to the ArkTS frontend via JS_THREADSAFE_FUNCTION so the actual
//! devecocli MCP check call happens on the JS side (which owns the MCP
//! connection lifecycle).

use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::JS_THREADSAFE_FUNCTION;
use async_trait::async_trait;
use serde_json::{json, Value};

const CALLBACK_KEY: &str = "call_check_cpp_files";

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
Requires devecocli installed; uses the shared deveco-mcp connection warmed on workspace open or switch_cwd.

Provide absolute or workspace-relative paths to source files."#
            .to_string())
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
        let files: Vec<String> = input
            .get("files")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        if files.is_empty() {
            return Err(BitFunError::tool("files must not be empty".to_string()));
        }

        let payload = json!({ "files": files }).to_string();
        let result = call_js_callback(CALLBACK_KEY, payload).await?;

        Ok(vec![ToolResult::Result {
            data: json!({
                "files": files,
                "success": true,
            }),
            result_for_assistant: Some(result),
            image_attachments: None,
        }])
    }
}

/// Shared JS callback bridge: looks up the threadsafe function by key,
/// calls it with the JSON payload, and awaits the promise result.
async fn call_js_callback(key: &str, payload: String) -> BitFunResult<String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get(key).cloned()
    };

    let Some(function) = function else {
        return Err(BitFunError::tool(format!(
            "{} has not been registered. Ensure the ArkTS frontend registers the callback.",
            key
        )));
    };

    let res = function.call_async(Ok(payload)).await;
    match res {
        Ok(promise) => match promise.await {
            Ok(json) => Ok(json),
            Err(err) => Err(BitFunError::tool(format!("{} failed: {}", key, err))),
        },
        Err(err) => Err(BitFunError::tool(format!("{} callback error: {}", key, err))),
    }
}
