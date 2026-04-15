//! VisualizeReadMe tool
//!
//! Provides a lightweight guidance checkpoint before the model generates
//! architecture and README-style widgets with GenerativeUI.

use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::util::errors::BitFunResult;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct VisualizeReadMeTool;

impl VisualizeReadMeTool {
    pub fn new() -> Self {
        Self
    }

    fn guidance_summary() -> String {
        [
            "Widget design guidance loaded.",
            "- Design for the inline FlowChat tool card, not a side panel.",
            "- For clickable architecture nodes, attach `data-file-path` and optional `data-line`.",
            "- Prefer one clickable node per concrete file or module that should open in the editor.",
            "- Avoid details-only `onclick` interactions unless the clickable node also carries file metadata.",
            "- Use compact hierarchy, clear click affordances, and avoid nested scrolling.",
        ]
        .join("\n")
    }
}

impl Default for VisualizeReadMeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for VisualizeReadMeTool {
    fn name(&self) -> &str {
        "VisualizeReadMe"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Use VisualizeReadMe as a guidance step before calling GenerativeUI for README visualizations, architecture maps, repo overviews, and other codebase widgets.

This tool does not render UI by itself. It exists to remind the model about the visual and interaction constraints that matter for BitFun widgets.

Use it when:
- the user asks for a repo architecture diagram, module map, or README-style explainer
- the widget should support click-to-open file navigation
- the output should be designed for the FlowChat inline tool card

Guidance:
1. After this tool, call GenerativeUI to render the actual widget.
2. Design for the inline FlowChat card first. Do not assume a separate right-side panel.
3. For clickable file navigation, put `data-file-path` on the clickable element and optionally `data-line`, `data-column`, and `data-line-end`.
4. `data-file-path` can be workspace-relative such as `src/crates/core/src/lib.rs` or absolute when already verified.
5. For architecture diagrams, prefer one clickable node per concrete file or module that should open in the editor.
6. Avoid widgets that only update a detail panel through `onclick` or `data-key` but never expose `data-file-path` on the clickable node.
7. Keep the widget compact, readable, and scroll-light. Avoid oversized app chrome, giant CSS resets, and nested scrolling.
8. Make clickable nodes look clickable with spacing, hover state, and visible grouping instead of producing a static poster.
9. Verify file paths before using them. Do not invent paths for navigation nodes."#
            .to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "modules": {
                    "type": "array",
                    "description": "Optional guidance tags such as interactive, diagram, architecture, or widget.",
                    "items": {
                        "type": "string"
                    }
                }
            }
        })
    }

    fn user_facing_name(&self) -> String {
        "Visualize README".to_string()
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
        if let Some(modules) = input.get("modules") {
            let Some(items) = modules.as_array() else {
                return ValidationResult {
                    result: false,
                    message: Some("modules must be an array of strings".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            };

            if items.iter().any(|item| {
                item.as_str()
                    .map(|value| value.trim().is_empty())
                    .unwrap_or(true)
            }) {
                return ValidationResult {
                    result: false,
                    message: Some("modules entries must be non-empty strings".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        ValidationResult::default()
    }

    fn render_result_for_assistant(&self, _output: &Value) -> String {
        Self::guidance_summary()
    }

    fn render_tool_use_message(
        &self,
        _input: &Value,
        _options: &crate::agentic::tools::framework::ToolRenderOptions,
    ) -> String {
        "Loading widget design guidance".to_string()
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let modules = input
            .get("modules")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();

        let data = json!({
            "success": true,
            "tool": "VisualizeReadMe",
            "modules": modules,
            "guidance_loaded": true,
        });

        Ok(vec![ToolResult::ok(data, Some(Self::guidance_summary()))])
    }
}
