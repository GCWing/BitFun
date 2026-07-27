//! SwitchCwd tool — switches the session project context directory for
//! HarmonyOS project actions.
//!
//! Bridges to the ArkTS frontend via JS_THREADSAFE_FUNCTION so the actual
//! session context update and MCP restart happen on the JS side.

use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::JS_THREADSAFE_FUNCTION;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;

const CALLBACK_KEY: &str = "call_switch_cwd";

/// SwitchCwd tool — switch the session project context directory.
pub struct SwitchCwdTool;

impl Default for SwitchCwdTool {
    fn default() -> Self {
        Self::new()
    }
}

impl SwitchCwdTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for SwitchCwdTool {
    fn name(&self) -> &str {
        "switch_cwd"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Switch the session project context directory for Harmony project actions.

Use this tool before running `devecocli build` / `devecocli run --skip-build`.
These tools only work correctly when the context directory is a valid project path.
If the current context directory is already the ArkTS project root, do not switch again.
When the `deveco-create-project` skill creates a full project under the current path,
you must switch to that generated project directory before running build or run commands.
Accepts absolute paths and relative paths (resolved from the current workspace directory).

After a successful switch, syntax-check MCP (`deveco-mcp`) is warmed in the background; large projects may take tens of seconds.
Subsequent `check_arkts_files`, `check_cpp_files`, and `devecocli` build/run commands use the new path.

For project-creation requests, you MUST first load the `deveco-create-project` skill instead of using this tool to jump to an existing directory."#.to_string())
    }

    fn short_description(&self) -> String {
        "Switch the session project context directory for Harmony project actions.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "project_path": {
                    "type": "string",
                    "description": "Target project directory path. Relative path is resolved from the current workspace directory."
                }
            },
            "required": ["project_path"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let project_path = match input.get("project_path").and_then(|v| v.as_str()) {
            Some(path) => path,
            None => {
                return ValidationResult {
                    result: false,
                    message: Some("project_path is required".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        if project_path.trim().is_empty() {
            return ValidationResult {
                result: false,
                message: Some("project_path must not be empty".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        let path = Path::new(project_path);
        if !path.exists() {
            return ValidationResult {
                result: false,
                message: Some(format!("Project path does not exist: {}", project_path)),
                error_code: Some(404),
                meta: None,
            };
        }

        if !path.is_dir() {
            return ValidationResult {
                result: false,
                message: Some(format!("Project path is not a directory: {}", project_path)),
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
        let path = input
            .get("project_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if options.verbose {
            format!("Switch project context to: {}", path)
        } else {
            format!("Switch to {}", path)
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let project_path = input
            .get("project_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("project_path is required".to_string()))?;

        let payload = json!({ "project_path": project_path }).to_string();
        let result = call_js_callback(CALLBACK_KEY, payload).await?;

        Ok(vec![ToolResult::Result {
            data: json!({
                "project_path": project_path,
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
