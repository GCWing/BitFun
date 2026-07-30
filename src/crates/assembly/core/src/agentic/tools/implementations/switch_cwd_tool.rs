//! SwitchCwd tool — switch the session project context directory for
//! HarmonyOS project actions.
//!
//! Pure Rust implementation: validates the path, checks for HarmonyOS project
//! markers (AppScope/app.json5, build-profile.json5, oh-package.json5/json),
//! and returns whether the directory is a valid HarmonyOS application root.

use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

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
        Ok(r#"Switch the session project directory for HarmonyOS tools (build_project, start_app, hdc_log, check_arkts_files, check_cpp_files).

Only use this tool when the HarmonyOS project directory is DIFFERENT from the current workspace root. For example, when the `deveco-create-project` skill creates a project in a subdirectory like `./MyApp`, call this tool with `project_path` pointing to that subdirectory.

Do NOT call this tool if:
- The workspace root already contains `build-profile.json5` / `AppScope/app.json5` — the tools already use the workspace root as the project directory by default.
- You are not sure whether a switch is needed — check first with `Glob` for `build-profile.json5` in the workspace root.

Parameter:
- project_path (required, string): absolute or relative path to the HarmonyOS project root directory. Relative paths are resolved from the current workspace directory.

After a successful switch, all HarmonyOS tools use the new path until the session ends."#.to_string())
    }

    fn short_description(&self) -> String {
        "Switch project directory — only when project is NOT the workspace root.".to_string()
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

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
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

        // Resolve relative paths against workspace root or cwd
        let resolved = resolve_target_path(project_path, context);

        if !resolved.exists() {
            return ValidationResult {
                result: false,
                message: Some(format!("Project path does not exist: {}", project_path)),
                error_code: Some(404),
                meta: None,
            };
        }

        if !resolved.is_dir() {
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
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let project_path = input
            .get("project_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("project_path is required".to_string()))?;

        let resolved = resolve_target_path(project_path, Some(context));
        let resolved_str = resolved.to_string_lossy().to_string();

        // Check if the directory is a HarmonyOS application root
        let is_harmony = is_harmony_application_root(&resolved);

        let message = if is_harmony {
            format!("Session directory updated to {}.", resolved_str)
        } else {
            format!(
                "Session directory updated to {}.\n\
                 It's not a HarmonyOS application project root.\n\
                 It's a directory without AppScope/app.json5, or build-profile.json5 with oh-package.json5 (or oh-package.json).\n\
                 You can create a new HarmonyOS project.",
                resolved_str
            )
        };

        Ok(vec![ToolResult::Result {
            data: json!({
                "project_path": resolved_str,
                "is_harmony_project": is_harmony,
                "success": true,
            }),
            result_for_assistant: Some(message),
            image_attachments: None,
        }])
    }
}

/// Resolve a project path: if absolute, use as-is; if relative, resolve
/// against the workspace root or process cwd.
fn resolve_target_path(project_path: &str, context: Option<&ToolUseContext>) -> PathBuf {
    let trimmed = project_path.trim();
    let path = Path::new(trimmed);

    if path.is_absolute() {
        return path.to_path_buf();
    }

    // Try resolving against workspace root
    if let Some(ctx) = context {
        if let Some(root) = ctx.workspace_root() {
            return root.join(path);
        }
    }

    // Fall back to process cwd
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

/// Check if a directory is a HarmonyOS application root.
///
/// A directory is a HarmonyOS application root if:
/// - It has `AppScope/app.json5`, OR
/// - It has `build-profile.json5` AND (`oh-package.json5` OR `oh-package.json`)
fn is_harmony_application_root(dir: &Path) -> bool {
    let is_file = |p: PathBuf| p.exists() && p.is_file();

    // AppScope/app.json5 — strong signal
    if is_file(dir.join("AppScope").join("app.json5")) {
        return true;
    }

    // build-profile.json5 is required for the fallback check
    if !is_file(dir.join("build-profile.json5")) {
        return false;
    }

    // oh-package.json5 or oh-package.json — project root with OHPM metadata
    if is_file(dir.join("oh-package.json5")) {
        return true;
    }
    if is_file(dir.join("oh-package.json")) {
        return true;
    }

    false
}
