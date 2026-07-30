//! start_app tool — run a HarmonyOS app on a device or emulator.
//!
//! Direct port of deveco-code `start_app`. Shells out to
//! `devecocli run --skip-build --device D --module entry@default --ability EntryAbility`.
//! When `hvd` is omitted, lists available targets; when it matches a stopped
//! emulator, auto-starts it first.

use super::devecocli_run::{run_devecocli, DevecocliOptions};
use super::harmony_device::{resolve_start_app_device, DeviceResolution};
use crate::agentic::tools::framework::{Tool, ToolRenderOptions, ToolResult, ToolUseContext};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct StartAppTool;

impl Default for StartAppTool {
    fn default() -> Self {
        Self::new()
    }
}

impl StartAppTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for StartAppTool {
    fn name(&self) -> &str {
        "start_app"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Run a HarmonyOS app on a device or emulator.

When `hvd` is omitted the tool lists connected devices and installed emulators; when `hvd` matches a stopped emulator it is auto-started first. Use this after build_project to deploy and launch the app.

Parameters:
- hvd (optional, string): target device name or ID. Omit to list available targets.
- module (optional, string): module name, e.g. "entry" (default: entry).
- target (optional, string): build target, e.g. "default" (default: default).
- ability (optional, string): ability to launch, e.g. "EntryAbility" (default: EntryAbility).

Example:
- List devices: {}
- Start on device: {"hvd": "emulator-5555"}"#
            .to_string())
    }

    fn short_description(&self) -> String {
        "Run a HarmonyOS app on a device or emulator.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "hvd": { "type": "string", "description": "Target device name or ID. Omit to list available devices." },
                "module": { "type": "string", "description": "Module name, e.g. entry." },
                "target": { "type": "string", "description": "Build target, e.g. default." },
                "ability": { "type": "string", "description": "Ability to launch, e.g. EntryAbility." }
            },
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        let device = input.get("hvd").and_then(|v| v.as_str()).unwrap_or("(list)");
        if options.verbose {
            format!("HarmonyOS start app on device: {}", device)
        } else {
            format!("Start app: {}", device)
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let hvd = input.get("hvd").and_then(|v| v.as_str());
        let module = input.get("module").and_then(|v| v.as_str()).unwrap_or("entry");
        let target = input.get("target").and_then(|v| v.as_str()).unwrap_or("default");
        let ability = input.get("ability").and_then(|v| v.as_str()).unwrap_or("EntryAbility");

        let resolved = resolve_start_app_device(hvd, context).await?;
        match resolved {
            DeviceResolution::List { output, device_count } => {
                Ok(vec![ToolResult::Result {
                    data: json!({
                        "tool": "start_app", "action": "list",
                        "deviceCount": device_count,
                    }),
                    result_for_assistant: Some(output),
                    image_attachments: None,
                }])
            }
            DeviceResolution::Ready { device } => {
                let module_target = format!("{}@{}", module, target);
                let argv = vec!["run", "--skip-build", "--device", device.as_str(), "--module", module_target.as_str(), "--ability", ability];
                let out = run_devecocli(&argv, context, DevecocliOptions::default()).await?;
                let combined = [out.stdout.as_str(), out.stderr.as_str()]
                    .iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join("\n");
                if out.exit_code != 0 {
                    return Err(BitFunError::tool(format!(
                        "start_app failed (exit {}):\n{}", out.exit_code, combined
                    )));
                }
                Ok(vec![ToolResult::Result {
                    data: json!({
                        "tool": "start_app", "action": "run", "exitCode": out.exit_code,
                        "cwd": out.cwd, "device": device,
                        "module": module, "target": target, "ability": ability,
                    }),
                    result_for_assistant: Some(if combined.is_empty() {
                        "App started successfully.".to_string()
                    } else {
                        combined
                    }),
                    image_attachments: None,
                }])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StartAppTool;
    use crate::agentic::tools::framework::Tool;
    use serde_json::json;

    #[test]
    fn start_app_schema_has_optional_device_params() {
        let schema = StartAppTool::new().input_schema();
        let props = schema.get("properties").and_then(|v| v.as_object()).expect("properties");
        for key in ["hvd", "module", "target", "ability"] {
            assert!(props.contains_key(key), "missing {key}");
        }
    }

    #[test]
    fn start_app_is_not_readonly() {
        assert!(!StartAppTool::new().is_readonly());
    }

    #[test]
    fn tool_name_matches() {
        assert_eq!(StartAppTool::new().name(), "start_app");
    }
}
