//! Cardinal pointer step (up/down/left/right) for Computer use.

use crate::agentic::tools::computer_use_capability::computer_use_desktop_available;
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::agentic::tools::implementations::computer_use_tool::computer_use_execute_mouse_step;
use crate::service::config::global::GlobalConfigManager;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct ComputerUseMouseStepTool;

impl Default for ComputerUseMouseStepTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputerUseMouseStepTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ComputerUseMouseStepTool {
    fn name(&self) -> &str {
        "ComputerUseMouseStep"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            "Move the pointer **one cardinal step** (up / down / left / right) by **`pixels`** (default 32, clamped 1..400) — same as **`ComputerUse`** **`pointer_move_rel`** on macOS scale. **Host blocks this immediately after a `screenshot`** until you reposition with **`move_to_text`**, **`mouse_move`** (`use_screen_coordinates`: true), or **`click_element`** (do not nudge from the JPEG). For diagonals, use **`ComputerUse`** **`pointer_move_rel`**.".to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Move the mouse pointer by a small directional step.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "direction": {
                    "type": "string",
                    "enum": ["up", "down", "left", "right"],
                    "description": "Cardinal direction for the step."
                },
                "pixels": {
                    "type": "integer",
                    "description": "Distance in screenshot/display pixels (default 32, clamped 1..400). Use smaller values (e.g. 8–24) for fine alignment."
                }
            },
            "required": ["direction"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn is_enabled(&self) -> bool {
        if !computer_use_desktop_available() {
            return false;
        }
        let Ok(service) = GlobalConfigManager::get_service().await else {
            return false;
        };
        let ai: crate::service::config::types::AIConfig =
            service.get_config(Some("ai")).await.unwrap_or_default();
        ai.computer_use_enabled
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        if context.is_remote() {
            return Err(BitFunError::tool(
                "ComputerUseMouseStep cannot run while the session workspace is remote (SSH)."
                    .to_string(),
            ));
        }
        let host = context.computer_use_host.as_ref().ok_or_else(|| {
            BitFunError::tool(
                "Computer use is only available in the BitFun desktop app.".to_string(),
            )
        })?;

        computer_use_execute_mouse_step(host.as_ref(), input).await
    }
}
