//! GenerativeUI tool — renders LLM-generated HTML/SVG widgets inline in FlowChat.

use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::util::errors::BitFunResult;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct GenerativeUITool;

impl GenerativeUITool {
    pub fn new() -> Self {
        Self
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
        Ok(r#"Use GenerativeUI to render visual HTML or SVG content inline inside the FlowChat tool card.

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
8. Design for the inline FlowChat card first. Avoid layouts that assume a separate side panel.
9. Avoid large CSS resets, fixed overlays, and nested scrolling.
10. Keep the widget focused. Prefer one clear visual or one small interactive tool.
11. If the widget needs follow-up reasoning, use `sendPrompt('...')` from inside the widget.
12. Do not invent custom desktop bridge APIs such as `window.app.call(...)` for file opening inside widgets.
13. Do not use `parent.postMessage(...)` or custom `onclick` protocols for file opening when `data-file-path` can be attached directly to the clickable element.
14. For clickable file navigation, add attributes like `data-file-path="src/main.rs"` and optional `data-line="42"` on the clickable element.
15. For codebase maps or architecture diagrams, clickable nodes MUST use `data-file-path`.
16. For codebase architecture diagrams, prefer one clickable node per file or module that should open in the editor.
17. If the user asks for click-to-open files, do not build a details-only interaction with `data-key` and `onclick="showDetail(...)"` unless the clickable node also carries `data-file-path`.
18. For charts, give charts a fixed-height wrapper and keep legends or summary numbers outside the canvas when possible.
19. For mockups, use compact spacing and clear hierarchy. Avoid building full app chrome unless the chrome itself is the point.
20. For lightweight generative art, prefer SVG and keep the output deterministic and performant.

The rendered widget appears directly inside the FlowChat tool card and should be designed to fit that inline context."#.to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["title", "widget_code"],
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Short widget title, for example 'compound interest simulator' or 'latency dashboard'."
                },
                "widget_code": {
                    "type": "string",
                    "description": "Raw HTML fragment or raw SVG. No Markdown code fences. For HTML: no <!DOCTYPE>, <html>, <head>, or <body>."
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
                    "description": "Optional guidance tags such as interactive, chart, mockup, art, diagram.",
                    "items": {
                        "type": "string"
                    }
                }
            }
        })
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
