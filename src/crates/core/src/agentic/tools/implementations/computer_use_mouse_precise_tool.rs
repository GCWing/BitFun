//! Absolute pointer positioning for Computer use.

use crate::agentic::tools::computer_use_capability::computer_use_desktop_available;
use crate::agentic::tools::implementations::computer_use_tool::computer_use_execute_mouse_precise;
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::service::config::global::GlobalConfigManager;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct ComputerUseMousePreciseTool;

impl ComputerUseMousePreciseTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ComputerUseMousePreciseTool {
    fn name(&self) -> &str {
        "ComputerUseMousePrecise"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            "Move the mouse pointer to **absolute global** coordinates only: set **`use_screen_coordinates`: true** (macOS: **points**). **Do not** use `coordinate_mode` image/normalized — that path is disabled (vision-derived positions are unreliable). Use numbers from **`move_to_text`**, **`locate`**, AX tools, or **`pointer_global`** in tool JSON. Same as `ComputerUse` **`mouse_move`**. For **small** cardinal nudges, prefer **ComputerUseMouseStep**.".to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "x": {
                    "type": "integer",
                    "description": "Target x in **global display** units — requires **use_screen_coordinates**: true (e.g. from move_to_text global_center_x, locate, pointer_global.x)."
                },
                "y": { "type": "integer", "description": "Target y; same as x (global display units)." },
                "coordinate_mode": {
                    "type": "string",
                    "enum": ["image", "normalized"],
                    "description": "Ignored — image/normalized positioning is disabled; always use **use_screen_coordinates**: true."
                },
                "use_screen_coordinates": {
                    "type": "boolean",
                    "description": "**Must be true.** x/y are global display coordinates (macOS: **points**)."
                }
            },
            "required": ["x", "y", "use_screen_coordinates"],
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

    async fn call_impl(&self, input: &Value, context: &ToolUseContext) -> BitFunResult<Vec<ToolResult>> {
        if context.is_remote() {
            return Err(BitFunError::tool(
                "ComputerUseMousePrecise cannot run while the session workspace is remote (SSH)."
                    .to_string(),
            ));
        }
        let host = context.computer_use_host.as_ref().ok_or_else(|| {
            BitFunError::tool("Computer use is only available in the BitFun desktop app.".to_string())
        })?;

        computer_use_execute_mouse_precise(host.as_ref(), input).await
    }
}
