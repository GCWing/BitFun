use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::infrastructure::events::event_system::{get_global_event_system, BackendEvent};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, RwLock};

/// SelfControl tool — lets the BitFun agent operate its own GUI.
///
/// The tool validates the required `action` field, then forwards the entire
/// camelCase payload to the frontend via the backend event system.  The
/// frontend executes the action and submits the result back through the
/// `submit_self_control_response` Tauri command.
pub struct SelfControlTool;

impl Default for SelfControlTool {
    fn default() -> Self {
        Self::new()
    }
}

impl SelfControlTool {
    pub fn new() -> Self {
        Self
    }
}

/// Minimal deserialization used only for input validation.
/// The full payload is forwarded as-is (Value) to the frontend.
#[derive(Debug, Clone, Deserialize)]
pub struct SelfControlInput {
    action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfControlResponse {
    pub request_id: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

struct PendingSelfControlRequest {
    sender: oneshot::Sender<SelfControlResponse>,
}

static PENDING_REQUESTS: std::sync::OnceLock<
    Arc<RwLock<HashMap<String, PendingSelfControlRequest>>>,
> = std::sync::OnceLock::new();

fn get_pending_requests() -> Arc<RwLock<HashMap<String, PendingSelfControlRequest>>> {
    PENDING_REQUESTS
        .get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
        .clone()
}

pub async fn submit_self_control_response(response: SelfControlResponse) -> BitFunResult<()> {
    let pending_requests = get_pending_requests();
    let pending = {
        let mut requests = pending_requests.write().await;
        requests.remove(&response.request_id)
    };

    let Some(pending) = pending else {
        return Err(BitFunError::NotFound(format!(
            "Self-control request not found: {}",
            response.request_id
        )));
    };

    let _ = pending.sender.send(response);
    Ok(())
}

#[async_trait]
impl Tool for SelfControlTool {
    fn name(&self) -> &str {
        "SelfControl"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Operate the BitFun application's own GUI.

Use this tool when the user asks you to change settings, open scenes/tabs,
click UI elements, set models, or perform any action inside the BitFun app itself.

Available actions (use EXACTLY one of these for the "action" field):
- "execute_task": Run a high-level task. Requires "task" field.
  Valid tasks: "set_primary_model", "set_fast_model", "enable_model", "disable_model", "toggle_model", "open_model_settings", "return_to_session", "delete_model", "open_live_app_gallery", "open_live_app" (legacy: "open_miniapp_gallery", "open_miniapp").
  Example: { "action": "execute_task", "task": "open_model_settings" }
  Example: { "action": "execute_task", "task": "enable_model", "params": { "modelQuery": "kimi" } }
  Example: { "action": "execute_task", "task": "disable_model", "params": { "modelQuery": "doubao-lite" } }
  Example: { "action": "execute_task", "task": "delete_model", "params": { "modelQuery": "OpenRouter" } }
  Example: { "action": "execute_task", "task": "set_primary_model", "params": { "modelQuery": "kimi" } }
  Example: { "action": "execute_task", "task": "open_live_app", "params": { "liveAppId": "<id from list_live_apps>" } }
  CRITICAL: "open_model_settings" is a TASK, not an action. Do NOT use { "action": "open_model_settings" }.
- "get_page_state": Returns the current page state including active scene, interactive elements, semantic hints, and quick-action targets. Each element carries a `selector` field that can be pasted directly into action="click". Optional "scope" (one of "auto" | "main" | "document" | <CSS-selector>); default "auto" prefers <main> and transparently widens when needed. Optional "includeOffscreen" (boolean | "auto", default "auto" — automatically true for the settings scene so the model list / MCP list rows below the viewport fold are visible).
- "click": Clicks an element by CSS selector. Requires non-empty "selector". For text-based targeting use click_by_text instead.
- "click_by_text": Clicks an element containing the given text. Requires "text" (also accepted as "query" / "label" / "searchText"). Optional "tag", optional "scope" (one of "auto" | "main" | "document" | <CSS-selector for a container>; default "auto" — searches inside <main> first, then widens to whole document on miss). Use "main" to avoid matching the global nav/sidebar/scene-tab bar. On AMBIGUOUS errors the response includes ready-to-use per-candidate selectors.
- "input": Sets the value of an input element. Requires "selector" and "value" (also accepted as "text" / "inputValue" / "content"). Missing "value" returns INVALID_PARAMS — no silent success.
- "scroll": Scrolls the page or an element. Optional "selector", requires "direction" ∈ {up, down, top, bottom}. As a convenience you may pass numeric "deltaY" instead (positive ⇒ down, negative ⇒ up); a missing/invalid direction returns INVALID_PARAMS — no silent success.
- "open_scene": Opens a scene by ID. Requires "sceneId" (e.g., "settings", "session", "welcome", "apps", or "live-app:<id>" for a specific Live App).
- "open_settings_tab": Opens the settings scene and switches to a tab. Requires "tabId".
- "open_live_app": Opens a specific installed Live App. Requires "liveAppId" (aliases: miniAppId; use ControlHub domain="app" action="list_live_apps" to discover).
- "open_miniapp": Legacy alias for open_live_app (same fields).
- "set_config": Sets a config value by key. Requires "key" and the value (accepted as "configValue", or any of the aliases "value" / "config_value" / "data" / "payload" — they all map to the same field, BUT only for this action; "value"/"text" are NOT cross-aliased on other actions).
- "get_config": Gets a config value by key. Requires "key".
- "list_models": Lists all configured models with their display names, providers, and IDs. Optional "includeDisabled" (boolean).
- "set_default_model": Directly sets the default model by config search. Falls back to UI if not found. Requires "modelQuery". Optional "slot" ("primary" or "fast").
- "select_option": Opens a custom Select dropdown and clicks an option by text. Requires "selector" and "optionText".
- "wait": Pauses for a given duration. Requires "durationMs" (e.g., 500).
- "press_key": Simulates a key press. Requires "key" (e.g., "Enter", "Escape").
- "read_text": Reads the visible text of an element. Requires "selector".
- "delete_model": Deletes a model from ai.models by matching name, model_name, or provider. Requires "modelQuery".

Guidelines:
1. For well-known requests (e.g., "set Kimi as the main model"), ALWAYS prefer "execute_task" with "set_primary_model".
2. To enable / disable / toggle a configured model, prefer execute_task with "enable_model" / "disable_model" / "toggle_model" — this is a pure config write and works from any scene without losing chat context. Only fall back to UI clicks when the user explicitly asks for the visual flow.
3. When a page changes, use "wait" with ~300-500ms before the next action to let UI settle.
4. For unknown UI tasks, use "get_page_state" first, read the "semanticHints" field, then decide.
5. After completing the user's request, return to the session scene with "return_to_session" task or open_scene "session".
6. For "what Live Apps are installed / show the gallery", call ControlHub domain="app" action="list_live_apps" first (it is pure-Rust, no UI round-trip), THEN open_live_app_gallery or open_live_app.

Failure modes you should expect (and not retry blindly):
- INVALID_PARAMS: required field missing or wrong type. Read the error hints — they list the canonical field name AND its accepted aliases for this exact action.
- NOT_FOUND: selector / model / Live App does not exist. Call get_page_state (or list_models / list_live_apps) before retrying with a guess.
- AMBIGUOUS: multiple candidates matched. The hint contains per-candidate concrete selectors — pick one and re-issue with action="click"."#
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "execute_task",
                        "get_page_state",
                        "click",
                        "click_by_text",
                        "input",
                        "scroll",
                        "open_scene",
                        "open_settings_tab",
                        "open_live_app",
                        "open_miniapp",
                        "set_config",
                        "get_config",
                        "list_models",
                        "set_default_model",
                        "select_option",
                        "wait",
                        "press_key",
                        "read_text",
                        "delete_model"
                    ],
                    "description": "The self-control action to perform. MUST be one of the enum values. For open_model_settings or delete_model, use execute_task with the task field, NOT the action field."
                },
                "task": {
                    "type": "string",
                    "enum": ["set_primary_model", "set_fast_model", "enable_model", "disable_model", "toggle_model", "open_model_settings", "return_to_session", "delete_model", "open_live_app_gallery", "open_live_app", "open_miniapp_gallery", "open_miniapp"],
                    "description": "Task name when using execute_task. enable_model / disable_model / toggle_model are pure config writes (no UI navigation)."
                },
                "liveAppId": {
                    "type": "string",
                    "description": "Live App id (use ControlHub domain=\"app\" action=\"list_live_apps\" to discover) for open_live_app action / task."
                },
                "miniAppId": {
                    "type": "string",
                    "description": "Legacy alias for liveAppId when opening a Live App."
                },
                "params": {
                    "type": "object",
                    "description": "Task parameters when using execute_task (e.g., { \"modelQuery\": \"kimi\" })."
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector for click, input, select_option, or read_text actions."
                },
                "text": {
                    "type": "string",
                    "description": "Dual-purpose, scoped per action: (1) the search text for click_by_text (canonical field); (2) accepted alias for the `value` field on the `input` action (also: inputValue, input_value, content). NOT cross-aliased to other actions — the frontend resolves aliases per action so the same surface name can mean different things."
                },
                "value": {
                    "description": "Dual-purpose, scoped per action: (1) the string value for the `input` action (canonical; aliases: text / inputValue / input_value / content); (2) accepted alias for `configValue` on the `set_config` action (also: config_value / data / payload). Pass any JSON value for set_config; pass a string for input."
                },
                "deltaY": {
                    "type": "number",
                    "description": "Optional convenience input for the scroll action. When `direction` is missing, deltaY>0 derives `down` and deltaY<0 derives `up`. Magnitude is otherwise ignored — each scroll action moves a fixed step."
                },
                "includeOffscreen": {
                    "description": "Optional for get_page_state. boolean | \"auto\" (default \"auto\" — automatically true on the settings scene). When true, elements outside the current viewport are also enumerated (necessary for the model list / MCP list which usually live below the fold)."
                },
                "tag": {
                    "type": "string",
                    "description": "Optional HTML tag to restrict click_by_text (e.g. \"button\")."
                },
                "scope": {
                    "type": "string",
                    "description": "Optional search scope for click_by_text / get_page_state. One of \"auto\" (default — try <main>, fall back to whole document), \"main\" (strictly inside <main data-testid=\"app-main-content\">), \"document\" (whole document, legacy behavior), or a CSS selector for a custom container element."
                },
                "direction": {
                    "type": "string",
                    "enum": ["up", "down", "top", "bottom"],
                    "description": "Scroll direction."
                },
                "sceneId": {
                    "type": "string",
                    "description": "Scene ID for open_scene (e.g., settings, session, welcome)."
                },
                "tabId": {
                    "type": "string",
                    "description": "Settings tab ID for open_settings_tab (e.g., models, basics, session-config)."
                },
                "key": {
                    "type": "string",
                    "description": "Config key for get_config / set_config."
                },
                "configValue": {
                    "description": "Config value for set_config (canonical). Alias-equivalent to `value` / `config_value` / `data` / `payload`, but ONLY for the set_config action — `value` on the `input` action means something different (the input text)."
                },
                "modelQuery": {
                    "type": "string",
                    "description": "Model name or ID to search for when using set_default_model, delete_model, or the enable_model / disable_model / toggle_model tasks (e.g., \"doubao pro\", \"gpt-4o\", \"kimi\"). Fuzzy-matched against display name, model_name, provider, base_url, id."
                },
                "slot": {
                    "type": "string",
                    "enum": ["primary", "fast"],
                    "description": "Which default model slot to set (primary or fast). Defaults to primary."
                },
                "optionText": {
                    "type": "string",
                    "description": "Text of the dropdown option to select. Used with select_option."
                },
                "durationMs": {
                    "type": "integer",
                    "description": "Duration in milliseconds to wait when using wait action."
                },
                "includeDisabled": {
                    "type": "boolean",
                    "description": "Whether to include disabled models when using list_models. Defaults to false."
                }
            },
            "required": ["action"]
        })
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        format!("Using SelfControl: {}", action)
    }

    fn render_result_for_assistant(&self, output: &Value) -> String {
        let base = match output.get("result").and_then(|v| v.as_str()) {
            Some(result) => result.to_string(),
            None => output.to_string(),
        };
        format!(
            "{}\n\n(Reminder: return to the session scene when done.)",
            base
        )
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        if input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .is_empty()
        {
            return ValidationResult {
                result: false,
                message: Some("Missing required field: action".to_string()),
                error_code: None,
                meta: None,
            };
        }
        ValidationResult::default()
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        // Validate action field — full payload is forwarded as-is to the frontend
        let validated: SelfControlInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::tool(format!("Invalid SelfControl input: {}", e)))?;
        let action_name = validated.action;

        let request_id = format!("selfcontrol_{}", uuid::Uuid::new_v4());
        let (tx, rx) = oneshot::channel();

        {
            let pending_requests = get_pending_requests();
            let mut pending = pending_requests.write().await;
            pending.insert(request_id.clone(), PendingSelfControlRequest { sender: tx });
        }

        // Forward the entire input payload directly — no field re-mapping needed.
        // The LLM fills fields using the camelCase names from input_schema, so the
        // frontend receives them in the correct format without any normalization.
        let event_payload = json!({
            "requestId": request_id,
            "actionType": action_name,
            "action": input,
        });

        let event_system = get_global_event_system();
        if let Err(e) = event_system
            .emit(BackendEvent::Custom {
                event_name: "selfcontrol://request".to_string(),
                payload: event_payload,
            })
            .await
        {
            // Emit failed — clean up the pending entry and return immediately
            // rather than blocking until timeout.
            let pending_requests = get_pending_requests();
            pending_requests.write().await.remove(&request_id);
            return Err(BitFunError::tool(format!(
                "Failed to emit self-control request: {}",
                e
            )));
        }

        let wait_timeout = Duration::from_secs(30);
        let decision = tokio::time::timeout(wait_timeout, rx).await;

        {
            let pending_requests = get_pending_requests();
            let mut pending = pending_requests.write().await;
            pending.remove(&request_id);
        }

        match decision {
            Ok(Ok(response)) => {
                if response.success {
                    let result_text = response.result.unwrap_or_else(|| "Done".to_string());
                    Ok(vec![ToolResult::ok(
                        json!({ "success": true, "result": result_text }),
                        Some(result_text),
                    )])
                } else {
                    let error_text = response
                        .error
                        .unwrap_or_else(|| "Unknown error".to_string());
                    Err(BitFunError::tool(format!(
                        "Self-control action failed: {}",
                        error_text
                    )))
                }
            }
            Ok(Err(_)) => Err(BitFunError::tool(
                "Self-control channel closed before response".to_string(),
            )),
            Err(_) => Err(BitFunError::tool(
                "Timed out waiting for self-control response".to_string(),
            )),
        }
    }
}
