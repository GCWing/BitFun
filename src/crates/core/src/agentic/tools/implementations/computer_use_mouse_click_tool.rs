//! Mouse button click and wheel at the current pointer (Computer use).

use crate::agentic::tools::computer_use_capability::computer_use_desktop_available;
use crate::agentic::tools::implementations::computer_use_tool::computer_use_execute_mouse_click_tool;
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::service::config::global::GlobalConfigManager;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct ComputerUseMouseClickTool;

impl ComputerUseMouseClickTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ComputerUseMouseClickTool {
    fn name(&self) -> &str {
        "ComputerUseMouseClick"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            "Click or scroll the **mouse wheel** at the **current** pointer (does not move the pointer). **`action`: `click`** — optional **`button`** (`left` | `right` | `middle`, default left), optional **`num_clicks`** (1 = single click default, 2 = double click, 3 = triple click); host enforces a fresh **fine** screenshot basis before click (same as former `ComputerUse` `click`). **`action`: `wheel`** — **`delta_x`** / **`delta_y`** (non-zero) for horizontal/vertical wheel ticks at the cursor (same as former `ComputerUse` `scroll`). Position the pointer first with **`ComputerUseMousePrecise`** / **`ComputerUseMouseStep`** / **`ComputerUse`** `pointer_move_rel`, then **`screenshot`** before click when the host requires it."
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["click", "wheel"],
                    "description": "`click` — press a mouse button at the current pointer. `wheel` — scroll wheel at the current pointer (use delta_x/delta_y; host-dependent units)."
                },
                "button": {
                    "type": "string",
                    "enum": ["left", "right", "middle"],
                    "description": "For `action` **click** only (default left). Ignored for `wheel`."
                },
                "num_clicks": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 3,
                    "description": "For `action` **click** only: number of clicks (1 = single click, 2 = double click for opening files / selecting words, 3 = triple click for selecting lines). Default 1."
                },
                "delta_x": {
                    "type": "integer",
                    "description": "For `action` **wheel** only: horizontal wheel delta (non-zero with delta_y or alone). Ignored for `click`."
                },
                "delta_y": {
                    "type": "integer",
                    "description": "For `action` **wheel** only: vertical wheel delta (non-zero with delta_x or alone). Ignored for `click`."
                }
            },
            "required": ["action"],
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
                "ComputerUseMouseClick cannot run while the session workspace is remote (SSH)."
                    .to_string(),
            ));
        }
        let host = context.computer_use_host.as_ref().ok_or_else(|| {
            BitFunError::tool("Computer use is only available in the BitFun desktop app.".to_string())
        })?;

        computer_use_execute_mouse_click_tool(host.as_ref(), input).await
    }
}
