//! get_ui_verification_log tool — retrieve device logs for a UI verification run.
//!
//! Direct port of deveco-code `GetUiVerificationLogTool`. Proxies to the `getLog`
//! tool on the `ui-verification-mcp` MCP server.

use super::ui_verification_mcp::call_ui_verification_mcp;
use crate::agentic::tools::framework::{Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct GetUiVerificationLogTool;

impl Default for GetUiVerificationLogTool {
    fn default() -> Self { Self::new() }
}

impl GetUiVerificationLogTool {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Tool for GetUiVerificationLogTool {
    fn name(&self) -> &str { "get_ui_verification_log" }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Retrieve device logs for a UI verification run.

Use after `verify_ui` to get the device logs collected during the verification. Provide the `id` returned by `verify_ui`.

Parameters:
- id (required, string): verification task ID returned by `verify_ui`.
- maxLogSize (optional, integer): total log character limit (default 5000; pass -1 for unlimited).
- searchKeywords (optional, string): log search keywords; pass empty string for full logs.

Example:
- {"id": "abc123"}
- {"id": "abc123", "maxLogSize": -1, "searchKeywords": "Error"}"#.to_string())
    }

    fn short_description(&self) -> String { "Retrieve device logs for a UI verification run.".to_string() }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Verification task ID returned by verify_ui." },
                "maxLogSize": { "type": "integer", "description": "Total log character limit (default 5000; -1 for unlimited)." },
                "searchKeywords": { "type": "string", "description": "Log search keywords; pass empty string for full logs." }
            },
            "required": ["id"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool { true }
    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool { true }

    async fn validate_input(&self, input: &Value, _ctx: Option<&ToolUseContext>) -> ValidationResult {
        let id = input.get("id").and_then(|v| v.as_str());
        if id.is_none() || id.map(|s| s.trim().is_empty()).unwrap_or(true) {
            return ValidationResult { result: false, message: Some("id is required".to_string()), error_code: Some(400), meta: None };
        }
        ValidationResult { result: true, message: None, error_code: None, meta: None }
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        let id = input.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if options.verbose { format!("HarmonyOS UI log: {}", id) } else { format!("UI log: {}", id) }
    }

    async fn call_impl(&self, input: &Value, _ctx: &ToolUseContext) -> BitFunResult<Vec<ToolResult>> {
        let id = input.get("id").and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("id is required".to_string()))?;
        let mut payload = json!({ "id": id });
        if let Some(size) = input.get("maxLogSize").and_then(|v| v.as_i64()) {
            payload["maxLogSize"] = json!(size);
        }
        if let Some(kw) = input.get("searchKeywords").and_then(|v| v.as_str()) {
            payload["searchKeywords"] = json!(kw);
        }

        let result = call_ui_verification_mcp("getLog", payload).await?;
        Ok(vec![ToolResult::Result {
            data: json!({ "tool": "get_ui_verification_log", "id": id, "success": true }),
            result_for_assistant: Some(result),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::GetUiVerificationLogTool;
    use crate::agentic::tools::framework::Tool;
    use serde_json::json;

    #[test]
    fn tool_name_matches() { assert_eq!(GetUiVerificationLogTool::new().name(), "get_ui_verification_log"); }
    #[test]
    fn is_readonly() { assert!(GetUiVerificationLogTool::new().is_readonly()); }

    #[tokio::test]
    async fn rejects_missing_id() {
        let r = GetUiVerificationLogTool::new().validate_input(&json!({}), None).await;
        assert!(!r.result);
        assert_eq!(r.error_code, Some(400));
    }

    #[tokio::test]
    async fn accepts_valid_id() {
        let r = GetUiVerificationLogTool::new().validate_input(&json!({"id": "abc123"}), None).await;
        assert!(r.result);
    }
}
