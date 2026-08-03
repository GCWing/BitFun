//! save_ui_screenshot tool — save screenshots from a UI verification run.
//!
//! Direct port of deveco-code `SaveUiScreenshotTool`. Proxies to the
//! `saveScreenshot` tool on the `ui-verification-mcp` MCP server.

use super::ui_verification_mcp::call_ui_verification_mcp;
use crate::agentic::tools::framework::{Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;

pub struct SaveUiScreenshotTool;

impl Default for SaveUiScreenshotTool {
    fn default() -> Self { Self::new() }
}

impl SaveUiScreenshotTool {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Tool for SaveUiScreenshotTool {
    fn name(&self) -> &str { "save_ui_screenshot" }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Save screenshots from a UI verification run.

Saves every step's screenshot for the given verification `id` to the specified directory. Returns a list of saved screenshot file names.

Parameters:
- id (required, string): verification task ID returned by `verify_ui`.
- dirname (required, string): absolute path to the screenshot save directory.

Example:
- {"id": "abc123", "dirname": "/tmp/screenshots"}"#.to_string())
    }

    fn short_description(&self) -> String { "Save screenshots from a UI verification run.".to_string() }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Verification task ID returned by verify_ui." },
                "dirname": { "type": "string", "description": "Absolute path to the screenshot save directory." }
            },
            "required": ["id", "dirname"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool { false }
    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool { false }

    async fn validate_input(&self, input: &Value, _ctx: Option<&ToolUseContext>) -> ValidationResult {
        let id = input.get("id").and_then(|v| v.as_str());
        if id.is_none() || id.map(|s| s.trim().is_empty()).unwrap_or(true) {
            return ValidationResult { result: false, message: Some("id is required".to_string()), error_code: Some(400), meta: None };
        }
        let dirname = input.get("dirname").and_then(|v| v.as_str());
        if dirname.is_none() || dirname.map(|s| s.trim().is_empty()).unwrap_or(true) {
            return ValidationResult { result: false, message: Some("dirname is required".to_string()), error_code: Some(400), meta: None };
        }
        // Path-traversal guard: dirname must be absolute and not contain ..
        let dir = dirname.unwrap();
        if !Path::new(dir).is_absolute() {
            return ValidationResult { result: false, message: Some("dirname must be an absolute path".to_string()), error_code: Some(400), meta: None };
        }
        if dir.contains("..") {
            return ValidationResult { result: false, message: Some("dirname must not contain '..'".to_string()), error_code: Some(400), meta: None };
        }
        ValidationResult { result: true, message: None, error_code: None, meta: None }
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        let id = input.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if options.verbose { format!("HarmonyOS save screenshots: {}", id) } else { format!("Save screenshots: {}", id) }
    }

    async fn call_impl(&self, input: &Value, _ctx: &ToolUseContext) -> BitFunResult<Vec<ToolResult>> {
        let id = input.get("id").and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("id is required".to_string()))?;
        let dirname = input.get("dirname").and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("dirname is required".to_string()))?;

        let payload = json!({ "id": id, "dirname": dirname });
        let result = call_ui_verification_mcp("saveScreenshot", payload).await?;
        Ok(vec![ToolResult::Result {
            data: json!({ "tool": "save_ui_screenshot", "id": id, "dirname": dirname, "success": true }),
            result_for_assistant: Some(result),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::SaveUiScreenshotTool;
    use crate::agentic::tools::framework::Tool;
    use serde_json::json;

    #[test]
    fn tool_name_matches() { assert_eq!(SaveUiScreenshotTool::new().name(), "save_ui_screenshot"); }
    #[test]
    fn is_not_readonly() { assert!(!SaveUiScreenshotTool::new().is_readonly()); }

    #[tokio::test]
    async fn rejects_missing_id() {
        let r = SaveUiScreenshotTool::new().validate_input(&json!({"dirname": "/tmp"}), None).await;
        assert!(!r.result);
        assert_eq!(r.error_code, Some(400));
    }

    #[tokio::test]
    async fn rejects_missing_dirname() {
        let r = SaveUiScreenshotTool::new().validate_input(&json!({"id": "abc"}), None).await;
        assert!(!r.result);
        assert_eq!(r.error_code, Some(400));
    }

    #[tokio::test]
    async fn rejects_relative_dirname() {
        let r = SaveUiScreenshotTool::new().validate_input(&json!({"id": "abc", "dirname": "relative/path"}), None).await;
        assert!(!r.result);
        assert_eq!(r.error_code, Some(400));
    }

    #[tokio::test]
    async fn rejects_path_traversal() {
        let r = SaveUiScreenshotTool::new().validate_input(&json!({"id": "abc", "dirname": "/tmp/../etc"}), None).await;
        assert!(!r.result);
        assert_eq!(r.error_code, Some(400));
    }

    #[tokio::test]
    async fn accepts_valid_input() {
        let dir = std::env::temp_dir().join("screenshots");
        let dir = dir.to_string_lossy().to_string();
        let r = SaveUiScreenshotTool::new().validate_input(&json!({"id": "abc", "dirname": dir}), None).await;
        assert!(r.result);
    }
}
