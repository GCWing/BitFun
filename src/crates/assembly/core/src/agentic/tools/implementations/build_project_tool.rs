//! build_project tool — build a HarmonyOS project via devecocli.
//!
//! Direct port of deveco-code `build_project`. Shells out to
//! `devecocli build [clean] [--build-mode X] [--product X] [--modules X]`.

use super::devecocli_run::{run_devecocli, DevecocliOptions};
use crate::agentic::tools::framework::{Tool, ToolRenderOptions, ToolResult, ToolUseContext};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct BuildProjectTool;

impl Default for BuildProjectTool {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildProjectTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for BuildProjectTool {
    fn name(&self) -> &str {
        "build_project"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Build a HarmonyOS project.

Drives the hvigor build pipeline with optional clean, build-mode, product, module, and log capture. Use this when the user asks to build, compile, package, or clean a HarmonyOS/OpenHarmony project that contains build-profile.json5 or oh-package.json5.

Parameters:
- clean (optional, boolean): run clean before build (like mvn clean).
- build_mode (optional, string): build mode name from build-profile.json5 buildModeSet; defaults to debug.
- product (optional, string): product name when building the entire APP.
- module (optional, string): module and target, e.g. "entry@default".
- log_path (optional, string): save build stdout/stderr to this path.

Example:
- Build: {"build_mode": "debug"}
- Clean build: {"clean": true}"#
            .to_string())
    }

    fn short_description(&self) -> String {
        "Build a HarmonyOS project via devecocli/hvigor.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "clean": { "type": "boolean", "description": "If true, run clean before build (like cargo clean or mvn clean)." },
                "build_mode": { "type": "string", "description": "Build mode name from build-profile.json5 buildModeSet (default: debug)." },
                "product": { "type": "string", "description": "Product name when building the entire APP." },
                "module": { "type": "string", "description": "Module and target, e.g. entry@default." },
                "log_path": { "type": "string", "description": "If set, save build stdout/stderr to this path." }
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
        let build_mode = input.get("build_mode").and_then(|v| v.as_str()).unwrap_or("debug");
        let clean = input.get("clean").and_then(|v| v.as_bool()).unwrap_or(false);
        let label = if clean { "Clean build" } else { "Build" };
        if options.verbose {
            format!("HarmonyOS {} (mode={})", label, build_mode)
        } else {
            format!("HarmonyOS {}: mode={}", label, build_mode)
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let clean = input.get("clean").and_then(|v| v.as_bool()).unwrap_or(false);
        let build_mode = input.get("build_mode").and_then(|v| v.as_str());
        let product = input.get("product").and_then(|v| v.as_str());
        let module = input.get("module").and_then(|v| v.as_str());
        let log_path = input.get("log_path").and_then(|v| v.as_str()).map(String::from);

        let mut argv: Vec<&str> = Vec::new();
        if clean {
            argv.extend_from_slice(&["build", "clean"]);
        } else {
            argv.push("build");
        }
        let bm = build_mode.map(|s| s.to_string());
        let prod = product.map(|s| s.to_string());
        let mod_ = module.map(|s| s.to_string());
        if let Some(bm) = &bm {
            argv.push("--build-mode");
            argv.push(bm.as_str());
        }
        if let Some(p) = &prod {
            argv.push("--product");
            argv.push(p.as_str());
        }
        if let Some(m) = &mod_ {
            argv.push("--modules");
            argv.push(m.as_str());
        }

        let out = run_devecocli(&argv, context, DevecocliOptions { log_path, ..Default::default() }).await?;
        let combined = [out.stdout.as_str(), out.stderr.as_str()]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join("\n");

        if out.exit_code != 0 {
            return Err(BitFunError::tool(format!(
                "build_project failed (exit {}):\n{}",
                out.exit_code, combined
            )));
        }

        Ok(vec![ToolResult::Result {
            data: json!({
                "tool": "build_project",
                "exitCode": out.exit_code,
                "cwd": out.cwd,
                "clean": clean,
                "build_mode": bm,
                "product": prod,
                "module": mod_,
            }),
            result_for_assistant: Some(if combined.is_empty() {
                "Build completed successfully.".to_string()
            } else {
                combined
            }),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::BuildProjectTool;
    use crate::agentic::tools::framework::Tool;
    use serde_json::json;

    #[test]
    fn build_project_schema_declares_optional_parameters() {
        let schema = BuildProjectTool::new().input_schema();
        let props = schema.get("properties").and_then(|v| v.as_object()).expect("properties");
        for key in ["clean", "build_mode", "product", "module", "log_path"] {
            assert!(props.contains_key(key), "missing {key}");
        }
    }

    #[test]
    fn build_project_is_not_readonly() {
        assert!(!BuildProjectTool::new().is_readonly());
    }

    #[test]
    fn tool_name_matches() {
        assert_eq!(BuildProjectTool::new().name(), "build_project");
    }
}
