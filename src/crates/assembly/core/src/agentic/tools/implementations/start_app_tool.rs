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
        Ok(r#"Run a HarmonyOS app on a connected device via devecocli.

When `hvd` is omitted, lists connected devices. Use this after build_project to deploy and launch the app.

Parameters:
- hvd (optional, string): target device name or ID. Omit to list available targets.
- module (optional, string): module name, e.g. "entry" (default: entry).
- target (optional, string): build target, e.g. "default" (default: default).
- ability (optional, string): ability to launch, e.g. "EntryAbility" (default: EntryAbility).

If devecocli is unavailable, cannot enumerate devices (e.g. permission/PATH issues), or `devecocli run` fails, this tool automatically falls back to driving `hdc` directly: it lists targets via `hdc list targets`, picks the matching device (or the sole device when `hvd` is omitted), locates the built `*.hap`, reads `bundleName` from `AppScope/app.json5`, runs `hdc -t <device> install -r <hap>`, and launches with `hdc -t <device> shell aa start -a <ability> -b <bundleName>`. When both devecocli and hdc fail, the error lists manual steps to run via ExecCommand.

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

        // Primary path: devecocli device resolution + run.
        match resolve_start_app_device(hvd, context).await {
            Ok(DeviceResolution::Ready { device }) => {
                let module_target = format!("{}@{}", module, target);
                let argv = vec!["run", "--skip-build", "--device", device.as_str(), "--module", module_target.as_str(), "--ability", ability];
                match run_devecocli(&argv, context, DevecocliOptions::default()).await {
                    Ok(out) => {
                        let combined = [out.stdout.as_str(), out.stderr.as_str()]
                            .iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join("\n");
                        if out.exit_code != 0 {
                            // devecocli run failed — try hdc fallback before surfacing the error.
                            return try_hdc_fallback(hvd, module, target, ability, context)
                                .await
                                .or_else(|fb_err| Err(append_fallback_hint(
                                    format!("start_app failed (exit {}):\n{}", out.exit_code, combined),
                                    Some(&fb_err.to_string()),
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
                    Err(e) => {
                        // devecocli unavailable / spawn failure — try hdc fallback.
                        try_hdc_fallback(hvd, module, target, ability, context)
                            .await
                            .or_else(|fb_err| Err(append_fallback_hint(e.to_string(), Some(&fb_err.to_string()))))
                    }
                }
            }
            Ok(DeviceResolution::List { output, device_count }) => {
                // devecocli enumerated fine and (when devices exist) we just list.
                // Only fall back to hdc when devecocli saw zero devices — hdc may
                // still see a device devecocli's permission model hides.
                if device_count > 0 {
                    return Ok(vec![ToolResult::Result {
                        data: json!({
                            "tool": "start_app", "action": "list",
                            "deviceCount": device_count,
                        }),
                        result_for_assistant: Some(output),
                        image_attachments: None,
                    }]);
                }
                try_hdc_fallback(hvd, module, target, ability, context)
                    .await
                    .or_else(|fb_err| Err(append_fallback_hint(
                        format!("{} found 0 devices via devecocli", output),
                        Some(&fb_err.to_string()),
                    )))
            }
            Err(e) => {
                // devecocli device resolution failed outright — try hdc fallback.
                try_hdc_fallback(hvd, module, target, ability, context)
                    .await
                    .or_else(|fb_err| Err(append_fallback_hint(e.to_string(), Some(&fb_err.to_string()))))
            }
        }
    }
}

/// Attempt the hdc fallback path and wrap a single `ToolResult` into the
/// expected `Vec<ToolResult>`.
async fn try_hdc_fallback(
    hvd: Option<&str>,
    module: &str,
    target: &str,
    ability: &str,
    context: &ToolUseContext,
) -> BitFunResult<Vec<ToolResult>> {
    log::info!("start_app: devecocli path failed, attempting hdc fallback");
    let result = super::hdc_fallback::run_hdc_start_fallback(hvd, module, target, ability, context).await?;
    Ok(vec![result])
}

/// Build the final error for the assistant: the primary failure message plus,
/// when the hdc fallback also failed, its reason, and finally the manual
/// ExecCommand hint as a last resort.
fn append_fallback_hint(primary: String, fallback_reason: Option<&str>) -> BitFunError {
    let mut msg = primary;
    if let Some(r) = fallback_reason {
        if !r.is_empty() {
            msg.push_str("\n\nhdc fallback also failed:\n");
            msg.push_str(r);
        }
    }
    msg.push_str("\n\n");
    msg.push_str(&hdc_fallback_hint());
    BitFunError::tool(msg)
}

/// Last-resort manual instructions embedded in start_app error messages after
/// both the devecocli path and the automatic hdc fallback have failed, so the
/// model knows exactly how to drive hdc via ExecCommand without a skill.
fn hdc_fallback_hint() -> String {
    r#"Manual fallback — the automatic hdc fallback above already failed. Use ExecCommand to run hdc by hand:
 1. hdc list targets  — check if hdc can find devices
 2. find . -name "*.hap" -path "*/outputs/*"  — locate the built HAP
 3. Read AppScope/app.json5 for bundleName; entry/src/main/module.json5 for ability name
 4. hdc -t <target> install -r "<hap_path>"  — (re)install the HAP on a specific device
 5. hdc -t <target> shell aa start -a <ability> -b <bundleName>  — launch the app
If hdc finds no devices: ask user to check system settings → developer options → wireless debugging, then hdc tconn <ip:port>"#
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::StartAppTool;
    use crate::agentic::tools::framework::Tool;

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
