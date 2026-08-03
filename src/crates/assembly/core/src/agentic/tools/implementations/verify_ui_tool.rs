//! verify_ui tool — run UI verification on a HarmonyOS device.
//!
//! Direct port of deveco-code `VerifyUiTool`. Proxies to the `verifyUI` tool
//! on the `ui-verification-mcp` MCP server. The agent provides a natural-language
//! test plan; the MCP server runs the app, takes screenshots, performs UI
//! operations, and verifies expected behavior.

use super::ui_verification_mcp::call_ui_verification_mcp;
use crate::agentic::tools::framework::{Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct VerifyUiTool;

impl Default for VerifyUiTool {
    fn default() -> Self { Self::new() }
}

impl VerifyUiTool {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Tool for VerifyUiTool {
    fn name(&self) -> &str { "verify_ui" }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Run UI verification on a HarmonyOS device via the ui-verification-mcp server.

Provide a natural-language test plan. The tool runs the app on a connected device, captures screenshots, performs UI operations (tap, swipe, long-press, key), and verifies whether each step succeeds.

Returns a description of the verification steps (successPart / failPart) and an `id` that can be used with `get_ui_verification_log` and `save_ui_screenshot`.

Parameters:
- testPlan (required, string): natural-language test case plan with steps and expected results.
- bundleName (optional, string): app bundle name; auto-resolved from the project if omitted.
- device (optional, string): device name (substring match) or serial (e.g. "127.0.0.1:5555"). Required when multiple devices are connected.
- freshStart (optional, boolean): restart the app before testing (default: false).

Example:
- {"testPlan": "1. Open the app 2. Tap the login button 3. Expect a login form"}
- {"testPlan": "...", "bundleName": "com.example.myapp", "device": "emulator-5555", "freshStart": true}"#.to_string())
    }

    fn short_description(&self) -> String { "Run UI verification on a HarmonyOS device.".to_string() }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "testPlan": { "type": "string", "description": "Natural-language test case plan with steps and expected results." },
                "bundleName": { "type": "string", "description": "App bundle name; auto-resolved from the project if omitted." },
                "device": { "type": "string", "description": "Device name (substring match) or serial. Required when multiple devices are connected." },
                "freshStart": { "type": "boolean", "description": "Restart the app before testing (default: false)." }
            },
            "required": ["testPlan"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool { false }
    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool { false }

    async fn validate_input(&self, input: &Value, _ctx: Option<&ToolUseContext>) -> ValidationResult {
        let test_plan = input.get("testPlan").and_then(|v| v.as_str());
        if test_plan.is_none() || test_plan.map(|s| s.trim().is_empty()).unwrap_or(true) {
            return ValidationResult { result: false, message: Some("testPlan is required and must not be empty".to_string()), error_code: Some(400), meta: None };
        }
        ValidationResult { result: true, message: None, error_code: None, meta: None }
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        let plan = input.get("testPlan").and_then(|v| v.as_str()).unwrap_or("");
        let preview: String = plan.chars().take(80).collect();
        if options.verbose { format!("HarmonyOS UI verify: {}", preview) } else { format!("Verify UI: {}", preview) }
    }

    async fn call_impl(&self, input: &Value, _ctx: &ToolUseContext) -> BitFunResult<Vec<ToolResult>> {
        let test_plan = input.get("testPlan").and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("testPlan is required".to_string()))?;
        let bundle_name = input.get("bundleName").and_then(|v| v.as_str());
        let device = input.get("device").and_then(|v| v.as_str());
        let fresh_start = input.get("freshStart").and_then(|v| v.as_bool()).unwrap_or(false);

        let mut payload = json!({ "testPlan": test_plan, "freshStart": fresh_start });
        if let Some(bn) = bundle_name { payload["bundleName"] = json!(bn); }
        if let Some(d) = device { payload["device"] = json!(d); }

        let result = call_ui_verification_mcp("verifyUI", payload).await?;
        Ok(vec![ToolResult::Result {
            data: json!({ "tool": "verify_ui", "success": true }),
            result_for_assistant: Some(result),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::VerifyUiTool;
    use crate::agentic::tools::framework::Tool;
    use serde_json::json;

    #[test]
    fn tool_name_matches() { assert_eq!(VerifyUiTool::new().name(), "verify_ui"); }
    #[test]
    fn is_not_readonly() { assert!(!VerifyUiTool::new().is_readonly()); }

    #[tokio::test]
    async fn rejects_empty_test_plan() {
        let tool = VerifyUiTool::new();
        let r = tool.validate_input(&json!({"testPlan": "  "}), None).await;
        assert!(!r.result);
        assert_eq!(r.error_code, Some(400));
    }

    #[tokio::test]
    async fn rejects_missing_test_plan() {
        let tool = VerifyUiTool::new();
        let r = tool.validate_input(&json!({}), None).await;
        assert!(!r.result);
        assert_eq!(r.error_code, Some(400));
    }

    #[tokio::test]
    async fn accepts_valid_plan() {
        let tool = VerifyUiTool::new();
        let r = tool.validate_input(&json!({"testPlan": "1. Open app 2. Check login"}), None).await;
        assert!(r.result);
    }
}
