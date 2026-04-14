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
/// The tool sends events to the frontend via the backend event system,
/// waits for the frontend to execute the action, and returns the result.
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelfControlActionType {
    ExecuteTask,
    GetPageState,
    Click,
    ClickByText,
    Input,
    Scroll,
    OpenScene,
    OpenSettingsTab,
    SetConfig,
    GetConfig,
    ListModels,
    SetDefaultModel,
    SelectOption,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SelfControlInput {
    action: SelfControlActionType,
    #[serde(default)]
    selector: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    tag: Option<String>,
    #[serde(default)]
    direction: Option<String>,
    #[serde(default)]
    scene_id: Option<String>,
    #[serde(default)]
    tab_id: Option<String>,
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    config_value: Option<Value>,
    #[serde(default)]
    model_query: Option<String>,
    #[serde(default)]
    slot: Option<String>,
    #[serde(default)]
    option_text: Option<String>,
    #[serde(default)]
    task: Option<String>,
    #[serde(default)]
    params: Option<Value>,
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

static PENDING_REQUESTS: std::sync::OnceLock<Arc<RwLock<HashMap<String, PendingSelfControlRequest>>>> =
    std::sync::OnceLock::new();

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

Actions:
- "execute_task": Run a high-level task that is internally planned and executed. Preferred for common workflows. Requires "task".
  Available tasks: "set_primary_model" (params: { modelQuery }), "set_fast_model" (params: { modelQuery }), "open_model_settings", "return_to_session".
- "get_page_state": Returns the current page state including active scene, interactive elements, semantic hints, and quick-action targets.
- "click": Clicks an element by CSS selector. Requires "selector".
- "click_by_text": Clicks an element containing the given text. Requires "text". Optional "tag".
- "input": Sets the value of an input element. Requires "selector" and "value".
- "scroll": Scrolls the page or an element. Optional "selector", requires "direction" (up, down, top, bottom).
- "open_scene": Opens a scene by ID. Requires "scene_id" (e.g., "settings", "session", "welcome").
- "open_settings_tab": Opens the settings scene and switches to a tab. Requires "tab_id".
- "set_config": Sets a config value by key. Requires "key" and "config_value".
- "get_config": Gets a config value by key. Requires "key".
- "list_models": Lists all enabled models with their display names, providers, and IDs.
- "set_default_model": Directly sets the default model by config search. Falls back to UI if not found. Requires "model_query".
- "select_option": Opens a custom Select dropdown and clicks an option by text. Requires "selector" and "option_text".

Guidelines:
1. For well-known requests (e.g., "set Kimi as the main model"), ALWAYS prefer "execute_task" with "set_primary_model".
2. For model requests, use "list_models" only when the user explicitly asks to see available models.
3. For unknown UI tasks, use "get_page_state" first, read the "semanticHints" field, then decide.
4. After completing the user's request, return to the session scene with "return_to_session" task or open_scene "session"."#
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
                        "set_config",
                        "get_config",
                        "list_models",
                        "set_default_model",
                        "select_option"
                    ],
                    "description": "The self-control action to perform. Prefer execute_task for common workflows."
                },
                "task": {
                    "type": "string",
                    "enum": ["set_primary_model", "set_fast_model", "open_model_settings", "return_to_session"],
                    "description": "Task name when using execute_task."
                },
                "params": {
                    "type": "object",
                    "description": "Task parameters when using execute_task (e.g., { modelQuery: \"kimi\" })."
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector for click, input, or select_option actions."
                },
                "text": {
                    "type": "string",
                    "description": "Text content to match for click_by_text."
                },
                "value": {
                    "type": "string",
                    "description": "Value to set for input actions."
                },
                "tag": {
                    "type": "string",
                    "description": "Optional HTML tag to restrict click_by_text."
                },
                "direction": {
                    "type": "string",
                    "enum": ["up", "down", "top", "bottom"],
                    "description": "Scroll direction."
                },
                "scene_id": {
                    "type": "string",
                    "description": "Scene ID for open_scene (e.g., settings, session, welcome)."
                },
                "tab_id": {
                    "type": "string",
                    "description": "Settings tab ID for open_settings_tab (e.g., models, basics, session-config)."
                },
                "key": {
                    "type": "string",
                    "description": "Config key for get_config / set_config."
                },
                "config_value": {
                    "description": "Config value for set_config."
                },
                "model_query": {
                    "type": "string",
                    "description": "Model name or ID to search for when using set_default_model (e.g., \"doubao pro\", \"gpt-4o\")."
                },
                "slot": {
                    "type": "string",
                    "enum": ["primary", "fast"],
                    "description": "Which default model slot to set (primary or fast). Defaults to primary."
                },
                "option_text": {
                    "type": "string",
                    "description": "Text of the dropdown option to select. Used with select_option."
                }
            },
            "required": ["action"]
        })
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("unknown");
        format!("Using SelfControl: {}", action)
    }

    fn render_result_for_assistant(&self, output: &Value) -> String {
        let base = match output.get("result").and_then(|v| v.as_str()) {
            Some(result) => result.to_string(),
            None => output.to_string(),
        };
        format!("{}\n\n(Reminder: return to the session scene when done.)", base)
    }

    async fn validate_input(
        &self,
        _input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        ValidationResult::default()
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: SelfControlInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::tool(format!("Invalid SelfControl input: {}", e)))?;

        let request_id = format!("selfcontrol_{}", uuid::Uuid::new_v4());
        let (tx, rx) = oneshot::channel();

        {
            let pending_requests = get_pending_requests();
            let mut pending = pending_requests.write().await;
            pending.insert(
                request_id.clone(),
                PendingSelfControlRequest { sender: tx },
            );
        }

        let mut action_payload = json!({
            "type": match params.action {
                SelfControlActionType::ExecuteTask => "execute_task",
                SelfControlActionType::GetPageState => "get_page_state",
                SelfControlActionType::Click => "click",
                SelfControlActionType::ClickByText => "click_by_text",
                SelfControlActionType::Input => "input",
                SelfControlActionType::Scroll => "scroll",
                SelfControlActionType::OpenScene => "open_scene",
                SelfControlActionType::OpenSettingsTab => "open_settings_tab",
                SelfControlActionType::SetConfig => "set_config",
                SelfControlActionType::GetConfig => "get_config",
                SelfControlActionType::ListModels => "list_models",
                SelfControlActionType::SetDefaultModel => "set_default_model",
                SelfControlActionType::SelectOption => "select_option",
            },
            "selector": params.selector,
            "text": params.text,
            "value": params.value,
            "tag": params.tag,
            "direction": params.direction,
            "scene_id": params.scene_id,
            "tab_id": params.tab_id,
            "key": params.key,
            "config_value": params.config_value,
            "model_query": params.model_query,
            "slot": params.slot,
            "option_text": params.option_text,
        });

        if let Some(task) = &params.task {
            action_payload["task"] = json!(task);
        }
        if let Some(params_val) = &params.params {
            action_payload["params"] = params_val.clone();
        }

        let event_payload = json!({
            "requestId": request_id,
            "action": action_payload,
        });

        let event_system = get_global_event_system();
        if let Err(e) = event_system
            .emit(BackendEvent::Custom {
                event_name: "selfcontrol://request".to_string(),
                payload: event_payload,
            })
            .await
        {
            log::warn!("Failed to emit self-control request event: {}", e);
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
                    let error_text = response.error.unwrap_or_else(|| "Unknown error".to_string());
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
