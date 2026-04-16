//! GenerativeUI tool — renders LLM-generated HTML/SVG widgets.

use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::util::errors::BitFunResult;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct GenerativeUITool;

impl GenerativeUITool {
    pub fn new() -> Self {
        Self
    }

    fn architecture_widget_reminder() -> &'static str {
        "Architecture/codebase widget reminder: if the widget is a repo map, README architecture view, or module diagram, clickable nodes must carry verified file metadata on the clickable element itself. Use `data-file-path` for a REAL existing file and `data-line` for the exact definition line when the node represents code. Do not attach file metadata to abstract grouping nodes, package containers, or directories. If a node is conceptual or cannot be verified, leave it non-clickable."
    }
}

impl Default for GenerativeUITool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GenerativeUITool {
    fn name(&self) -> &str {
        "GenerativeUI"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Use GenerativeUI to render visual HTML or SVG content.

Use this when the user asks for visual or interactive output such as:
- charts, dashboards, tables
- explainers with sliders or controls
- diagrams, mockups, or small simulations
- SVG illustrations

Input rules:
1. Put the widget code in `widget_code`.
2. For HTML, provide a raw fragment only. Do NOT include Markdown fences, <!DOCTYPE>, <html>, <head>, or <body>.
3. For SVG, provide raw SVG starting with <svg>.
4. Put CSS first, then HTML, then scripts last so the preview can stream progressively.
5. Keep the first useful content visible early. Avoid giant style blocks.
6. Prefer self-contained widgets. CDN scripts are allowed when needed, but keep them minimal.
7. If the user only needs text, do not use this tool.
8. Prefer compact, scroll-light layouts. Avoid large CSS resets, fixed overlays, oversized app chrome, and nested scrolling.
9. Keep the widget focused. Prefer one clear visual or one small interactive tool.
10. If the widget needs follow-up reasoning, use `sendPrompt('...')` from inside the widget.
11. Do not invent custom desktop bridge APIs such as `window.app.call(...)` for file opening inside widgets.
12. Do not use `parent.postMessage(...)` or custom `onclick` protocols for file opening when `data-file-path` can be attached directly to the clickable element.
13. CRITICAL for codebase maps, repo overviews, and architecture diagrams: NEVER guess or invent paths. Every clickable `data-file-path` MUST point to a REAL file that exists in the workspace.
14. For clickable file navigation, add `data-file-path` on the clickable element itself, and add `data-line` for the exact definition or anchor line whenever the node represents code.
15. `data-file-path` may be workspace-relative such as `src/crates/core/src/lib.rs`, or absolute when already verified, but it MUST resolve to a file, not a directory.
16. Do NOT attach `data-file-path` to abstract grouping nodes such as "Core", "Frontend", "Agent System", or module containers unless that node intentionally opens one specific real file.
17. For codebase architecture diagrams, prefer one clickable node per concrete file. If a node represents a broader concept, package, or directory, leave it non-clickable instead of pointing it at a folder.
18. Workflow for architecture widgets: first verify candidate files with Glob or LS, then use Read with line numbers when needed, and only then emit clickable nodes with verified file paths and lines.
19. If you cannot verify the exact file path and line number, do not make that node clickable. Better to have fewer accurate links than many broken ones.
20. If the user asks for click-to-open files, do not build a details-only interaction with `data-key` and `onclick="showDetail(...)"` unless the clickable node also carries its own `data-file-path`.
21. Do not put one `data-file-path` on a large wrapper that contains multiple visual nodes. The actual clickable node must own the path metadata.
22. Make clickable nodes look clickable with visible grouping, spacing, and hover feedback instead of producing a static poster.
23. For charts, give charts a fixed-height wrapper and keep legends or summary numbers outside the canvas when possible.
24. For mockups, use compact spacing and clear hierarchy. Avoid building full app chrome unless the chrome itself is the point.
25. For lightweight generative art, prefer SVG and keep the output deterministic and performant."#.to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "description": format!(
                "Render a compact HTML/SVG widget. {}",
                Self::architecture_widget_reminder()
            ),
            "required": ["title", "widget_code"],
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Short widget title, for example 'compound interest simulator' or 'latency dashboard'."
                },
                "widget_code": {
                    "type": "string",
                    "description": format!(
                        "Raw HTML fragment or raw SVG. No Markdown code fences. For HTML: no <!DOCTYPE>, <html>, <head>, or <body>. {} If the user asked for file navigation, do not finish this field until each clickable node has verified file metadata or is intentionally non-clickable.",
                        Self::architecture_widget_reminder()
                    )
                },
                "width": {
                    "type": "integer",
                    "minimum": 240,
                    "maximum": 1600,
                    "description": "Preferred width in pixels for enlarged panel view. Optional."
                },
                "height": {
                    "type": "integer",
                    "minimum": 160,
                    "maximum": 1600,
                    "description": "Preferred height in pixels for enlarged panel view. Optional."
                },
                "modules": {
                    "type": "array",
                    "description": "Optional guidance tags such as interactive, chart, mockup, art, diagram, architecture, or repo-map. If this includes architecture/repo-map/diagram, apply the architecture widget reminder strictly.",
                    "items": {
                        "type": "string"
                    }
                }
            }
        })
    }

    async fn input_schema_for_model_with_context(
        &self,
        _context: Option<&ToolUseContext>,
    ) -> Value {
        let mut schema = self.input_schema();
        if let Some(obj) = schema.as_object_mut() {
            obj.insert(
                "x-bitfun-reminder".to_string(),
                Value::String(Self::architecture_widget_reminder().to_string()),
            );
        }
        schema
    }

    fn user_facing_name(&self) -> String {
        "Generative UI".to_string()
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let title = match input.get("title").and_then(|v| v.as_str()) {
            Some(value) if !value.trim().is_empty() => value.trim(),
            _ => {
                return ValidationResult {
                    result: false,
                    message: Some("Missing or empty title".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        let widget_code = match input.get("widget_code").and_then(|v| v.as_str()) {
            Some(value) if !value.trim().is_empty() => value.trim(),
            _ => {
                return ValidationResult {
                    result: false,
                    message: Some("Missing or empty widget_code".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        if title.len() > 120 {
            return ValidationResult {
                result: false,
                message: Some("title is too long; keep it under 120 characters".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        if widget_code.starts_with("```") {
            return ValidationResult {
                result: false,
                message: Some(
                    "widget_code must be raw HTML or SVG, not Markdown code fences".to_string(),
                ),
                error_code: Some(400),
                meta: None,
            };
        }

        ValidationResult::default()
    }

    fn render_result_for_assistant(&self, output: &Value) -> String {
        let title = output
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("widget");

        format!("Rendered widget preview '{}'.", title)
    }

    fn render_tool_use_message(
        &self,
        input: &Value,
        _options: &crate::agentic::tools::framework::ToolRenderOptions,
    ) -> String {
        let title = input
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("widget");
        format!("Rendering widget: {}", title)
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let title = input
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Widget");
        let widget_code = input
            .get("widget_code")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let width = input.get("width").and_then(|v| v.as_i64()).unwrap_or(960);
        let height = input.get("height").and_then(|v| v.as_i64()).unwrap_or(640);
        let modules = input
            .get("modules")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let is_svg = widget_code.trim_start().starts_with("<svg");

        let widget_id = context
            .tool_call_id
            .clone()
            .unwrap_or_else(|| format!("widget_{}", chrono::Utc::now().timestamp_millis()));

        Ok(vec![ToolResult::Result {
            data: json!({
                "success": true,
                "widget_id": widget_id,
                "title": title,
                "widget_code": widget_code,
                "width": width,
                "height": height,
                "is_svg": is_svg,
                "modules": modules,
            }),
            result_for_assistant: Some(format!(
                "Rendered widget '{}' inline in the FlowChat tool card.",
                title
            )),
            image_attachments: None,
        }])
    }
}
