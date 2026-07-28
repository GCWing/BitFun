//! hdc_log tool — collect HarmonyOS device logs or list connected devices.
//!
//! Direct port of deveco-code `hdc_log`. Shells out to
//! `devecocli log --tail N --keyword K --device D` (collect) or
//! `devecocli device list` (list_devices). Read-only and concurrency-safe.

use super::devecocli_run::{run_devecocli, DevecocliOptions};
use super::harmony_device::format_connected_device_list;
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

const HDC_LOG_TIMEOUT: Duration = Duration::from_secs(120);

pub struct HdcLogTool;

impl Default for HdcLogTool {
    fn default() -> Self {
        Self::new()
    }
}

impl HdcLogTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for HdcLogTool {
    fn name(&self) -> &str {
        "hdc_log"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Collect HarmonyOS device logs or list connected devices.

Use action "list_devices" to enumerate connected devices and emulators; use action "collect" to tail filtered hilog output. This tool is read-only.

Parameters:
- action (required, string): "collect" or "list_devices".
- device_id (optional, string): target device id; omit to auto-select.
- log_prefix (optional, string): keyword filter (maps to hilog --keyword). Empty string disables filtering.
- lines (optional, integer): number of log lines to collect, 1-5000 (default: 2000).

Example:
- List devices: {"action": "list_devices"}
- Collect logs: {"action": "collect", "log_prefix": "[VCODER_DEBUG]", "lines": 500}"#
            .to_string())
    }

    fn short_description(&self) -> String {
        "Collect HarmonyOS device logs or list devices via devecocli/hdc.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["collect", "list_devices"], "description": "Action to perform." },
                "device_id": { "type": "string", "description": "Optional device ID." },
                "log_prefix": { "type": "string", "description": "Log keyword filter. Empty string disables filtering." },
                "lines": { "type": "integer", "minimum": 1, "maximum": 5000, "description": "Number of log lines to collect (default: 2000)." }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let action = match input.get("action").and_then(|v| v.as_str()) {
            Some(a) => a,
            None => {
                return ValidationResult {
                    result: false,
                    message: Some("action is required".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };
        if action != "collect" && action != "list_devices" {
            return ValidationResult {
                result: false,
                message: Some(format!("action must be \"collect\" or \"list_devices\", got: {}", action)),
                error_code: Some(400),
                meta: None,
            };
        }
        if let Some(lines) = input.get("lines").and_then(|v| v.as_i64()) {
            if !(1..=5000).contains(&lines) {
                return ValidationResult {
                    result: false,
                    message: Some(format!("lines must be between 1 and 5000, got: {}", lines)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }
        ValidationResult { result: true, message: None, error_code: None, meta: None }
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("collect");
        if options.verbose {
            format!("HarmonyOS hdc_log action: {}", action)
        } else {
            format!("hdc_log: {}", action)
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("collect");

        if action == "list_devices" {
            let out = run_devecocli(&["device", "list"], context, DevecocliOptions::default()).await?;
            let combined = [out.stdout.as_str(), out.stderr.as_str()]
                .iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join("\n");
            if out.exit_code != 0 {
                return Err(BitFunError::tool(format!(
                    "hdc_log list_devices failed (exit {}):\n{}", out.exit_code, combined
                )));
            }
            let (formatted, device_count) = format_connected_device_list(&combined);
            return Ok(vec![ToolResult::Result {
                data: json!({ "tool": "hdc_log", "action": "list_devices", "exitCode": out.exit_code, "deviceCount": device_count }),
                result_for_assistant: Some(formatted),
                image_attachments: None,
            }]);
        }

        let lines = input.get("lines").and_then(|v| v.as_u64()).unwrap_or(2000);
        let log_prefix = input.get("log_prefix").and_then(|v| v.as_str()).unwrap_or("[VCODER_DEBUG]");
        let device_id = input.get("device_id").and_then(|v| v.as_str());

        let lines_str = lines.to_string();
        let mut argv: Vec<&str> = vec!["log", "--tail", lines_str.as_str()];
        if !log_prefix.is_empty() {
            argv.push("--keyword");
            argv.push(log_prefix);
        }
        let did = device_id.map(|s| s.to_string());
        if let Some(did) = &did {
            argv.push("--device");
            argv.push(did.as_str());
        }

        let out = run_devecocli(&argv, context, DevecocliOptions { timeout: HDC_LOG_TIMEOUT, ..Default::default() }).await?;
        let combined = [out.stdout.as_str(), out.stderr.as_str()]
            .iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join("\n");
        if out.exit_code != 0 {
            return Err(BitFunError::tool(format!(
                "hdc_log collect failed (exit {}):\n{}", out.exit_code, combined
            )));
        }
        let line_count = combined.lines().filter(|l| !l.is_empty()).count();
        Ok(vec![ToolResult::Result {
            data: json!({
                "tool": "hdc_log", "action": "collect", "exitCode": out.exit_code,
                "lineCount": line_count, "lines": lines, "logPrefix": log_prefix, "deviceId": did,
            }),
            result_for_assistant: Some(if combined.is_empty() { "No logs collected.".to_string() } else { combined }),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::HdcLogTool;
    use crate::agentic::tools::framework::{Tool, ToolUseContext, ValidationResult};
    use serde_json::json;
    use std::collections::HashMap;

    fn test_context() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    #[tokio::test]
    async fn hdc_log_rejects_missing_action() {
        let r = HdcLogTool::new().validate_input(&json!({}), Some(&test_context())).await;
        assert!(!r.result);
        assert_eq!(r.error_code, Some(400));
    }

    #[tokio::test]
    async fn hdc_log_rejects_unknown_action() {
        let r = HdcLogTool::new().validate_input(&json!({"action": "clear"}), Some(&test_context())).await;
        assert!(!r.result);
        assert_eq!(r.error_code, Some(400));
    }

    #[tokio::test]
    async fn hdc_log_rejects_out_of_range_lines() {
        let r = HdcLogTool::new().validate_input(&json!({"action": "collect", "lines": 0}), Some(&test_context())).await;
        assert!(!r.result);
        let r = HdcLogTool::new().validate_input(&json!({"action": "collect", "lines": 5001}), Some(&test_context())).await;
        assert!(!r.result);
    }

    #[tokio::test]
    async fn hdc_log_accepts_valid_collect() {
        let r = HdcLogTool::new().validate_input(&json!({"action": "collect", "lines": 200, "log_prefix": "[VCODER_DEBUG]"}), Some(&test_context())).await;
        assert!(r.result);
    }

    #[tokio::test]
    async fn hdc_log_accepts_list_devices() {
        let r = HdcLogTool::new().validate_input(&json!({"action": "list_devices"}), Some(&test_context())).await;
        assert!(r.result);
    }

    #[test]
    fn hdc_log_is_readonly() {
        assert!(HdcLogTool::new().is_readonly());
    }

    #[test]
    fn tool_name_matches() {
        assert_eq!(HdcLogTool::new().name(), "hdc_log");
    }
}
