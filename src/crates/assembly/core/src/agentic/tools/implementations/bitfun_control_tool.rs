//! BitFunControl — discover and control user-facing BitFun features and settings.

use crate::agentic::agents::get_agent_registry;
use crate::agentic::tools::bitfun_control_host::{
    bitfun_control_host_available, invoke_bitfun_control, BitFunControlHostRequest,
};
use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};

const ACTIONS: &[&str] = &["list", "search", "get", "open", "execute", "configure"];

pub struct BitFunControlTool;

impl BitFunControlTool {
    pub fn new() -> Self {
        Self
    }

    fn action(input: &Value) -> Option<&str> {
        input.get("action").and_then(Value::as_str).map(str::trim)
    }

    fn requires_capability_id(action: &str) -> bool {
        matches!(action, "get" | "open" | "execute" | "configure")
    }

    fn agent_is_readonly(context: &ToolUseContext) -> bool {
        let Some(agent_type) = context.agent_type.as_deref() else {
            return false;
        };
        get_agent_registry()
            .get_agent(agent_type, context.workspace_root())
            .is_some_and(|agent| agent.is_readonly())
    }
}

impl Default for BitFunControlTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BitFunControlTool {
    fn name(&self) -> &str {
        "BitFunControl"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            "Control BitFun features and settings through its internal API. Use a two-step flow: first call `list` or `search` (then `get` when needed) to discover a user-facing capability, then call `open`, `execute`, or `configure`. The catalog is loaded only on demand and is not embedded in this description."
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Discover and control BitFun features and settings.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ACTIONS,
                    "description": "Discovery: list/search/get. Control: open/execute/configure. Discover a capability before controlling it."
                },
                "query": {
                    "type": "string",
                    "description": "Chinese or English query for search."
                },
                "capability_id": {
                    "type": "string",
                    "description": "Stable capability ID returned by list/search/get."
                },
                "item_id": {
                    "type": "string",
                    "description": "Optional documented item ID returned by search/get; open uses it to navigate to an exact subview."
                },
                "operation_id": {
                    "type": "string",
                    "description": "User-level operation ID returned by get; required for execute."
                },
                "option_id": {
                    "type": "string",
                    "description": "User-level setting option ID returned by get; required for configure."
                },
                "value": {
                    "description": "New option value for configure, following the value schema returned by get."
                },
                "cursor": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Zero-based list/search cursor."
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 50,
                    "description": "Maximum discovery results. Defaults to all 50 slots for list and 20 for search; maximum 50."
                }
            }
        })
    }

    async fn is_available_in_context(&self, _context: Option<&ToolUseContext>) -> bool {
        bitfun_control_host_available()
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, input: Option<&Value>) -> bool {
        input
            .and_then(Self::action)
            .is_some_and(|action| matches!(action, "list" | "search" | "get"))
    }

    fn permission_intents(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<PermissionIntent>> {
        let action = Self::action(input).unwrap_or("<missing-action>");
        if matches!(action, "list" | "search" | "get") {
            return Ok(Vec::new());
        }
        let capability_id = input
            .get("capability_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("<missing-capability-id>");
        let target = match action {
            "execute" => input
                .get("operation_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| format!("{capability_id}:{value}"))
                .unwrap_or_else(|| capability_id.to_string()),
            "configure" => input
                .get("option_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| format!("{capability_id}:{value}"))
                .unwrap_or_else(|| capability_id.to_string()),
            "open" => input
                .get("item_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| format!("{capability_id}:{value}"))
                .unwrap_or_else(|| capability_id.to_string()),
            _ => capability_id.to_string(),
        };
        Ok(vec![PermissionIntent::new(
            "bitfun_control",
            vec![format!("{action}:{target}")],
        )])
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let invalid = |message: &str| ValidationResult {
            result: false,
            message: Some(message.to_string()),
            error_code: None,
            meta: None,
        };
        if !input.is_object() {
            return invalid("Input must be an object.");
        }
        let Some(action) = Self::action(input) else {
            return invalid("action is required.");
        };
        if !ACTIONS.contains(&action) {
            return invalid(
                "action must be one of list, search, get, open, execute, or configure.",
            );
        }
        if action == "search"
            && !input
                .get("query")
                .and_then(Value::as_str)
                .is_some_and(|query| !query.trim().is_empty())
        {
            return invalid("query is required for search.");
        }
        if Self::requires_capability_id(action)
            && !input
                .get("capability_id")
                .and_then(Value::as_str)
                .is_some_and(|id| !id.trim().is_empty())
        {
            return invalid("capability_id is required for get, open, execute, and configure.");
        }
        if input.get("item_id").is_some_and(|value| {
            !value
                .as_str()
                .is_some_and(|item_id| !item_id.trim().is_empty())
        }) {
            return invalid("item_id must be a non-empty string when provided.");
        }
        if action == "execute"
            && !input
                .get("operation_id")
                .and_then(Value::as_str)
                .is_some_and(|id| !id.trim().is_empty())
        {
            return invalid("operation_id is required for execute.");
        }
        if action == "configure"
            && !input
                .get("option_id")
                .and_then(Value::as_str)
                .is_some_and(|id| !id.trim().is_empty())
        {
            return invalid("option_id is required for configure.");
        }
        if action == "configure" && input.get("value").is_none() {
            return invalid("value is required for configure.");
        }
        if input
            .get("cursor")
            .is_some_and(|value| value.as_u64().is_none())
        {
            return invalid("cursor must be a non-negative integer when provided.");
        }
        if input.get("limit").is_some_and(|value| {
            value
                .as_u64()
                .is_none_or(|limit| !(1..=50).contains(&limit))
        }) {
            return invalid("limit must be an integer between 1 and 50.");
        }
        ValidationResult::default()
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        if !bitfun_control_host_available() {
            return Err(BitFunError::tool(
                "BitFunControl is unavailable on this product surface".to_string(),
            ));
        }
        let action = Self::action(input)
            .ok_or_else(|| BitFunError::tool("action is required".to_string()))?;
        if matches!(action, "open" | "execute" | "configure") && Self::agent_is_readonly(context) {
            return Err(BitFunError::tool(
                "This read-only agent may discover BitFun features and settings but cannot control them"
                    .to_string(),
            ));
        }

        let result = invoke_bitfun_control(BitFunControlHostRequest {
            action: action.to_string(),
            query: input
                .get("query")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            capability_id: input
                .get("capability_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            item_id: input
                .get("item_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            operation_id: input
                .get("operation_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            option_id: input
                .get("option_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            value: input.get("value").cloned(),
            cursor: input
                .get("cursor")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok()),
            limit: input
                .get("limit")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok()),
        })
        .await
        .map_err(BitFunError::tool)?;

        let assistant = match action {
            "list" | "search" => {
                let count = result
                    .get("items")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or_default();
                format!("BitFun feature and setting discovery returned {count} item(s). Use a returned capability ID with get, open, execute, or configure.")
            }
            "get" => "Loaded the BitFun feature or setting manual, including its user-level operations and configurable options.".to_string(),
            "open" => "Opened the BitFun feature or setting in the active product surface.".to_string(),
            "execute" => "Executed the selected user-level BitFun operation.".to_string(),
            "configure" => "Updated the selected BitFun setting option.".to_string(),
            _ => "BitFunControl completed.".to_string(),
        };
        Ok(vec![ToolResult::ok(result, Some(assistant))])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn context() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    #[tokio::test]
    async fn description_keeps_the_catalog_out_of_the_prompt() {
        let description = BitFunControlTool::new().description().await.unwrap();
        assert!(description.contains("two-step"));
        assert!(description.contains("list"));
        assert!(description.contains("search"));
        assert!(!description.contains("get_configs"));
        assert!(description.len() < 600);
    }

    #[tokio::test]
    async fn validates_the_discover_then_execute_contract() {
        let tool = BitFunControlTool::new();
        assert!(
            tool.validate_input(&json!({ "action": "list" }), None)
                .await
                .result
        );
        assert!(
            !tool
                .validate_input(&json!({ "action": "search" }), None)
                .await
                .result
        );
        assert!(
            !tool
                .validate_input(&json!({ "action": "execute" }), None)
                .await
                .result
        );
        assert!(
            tool.validate_input(
                &json!({
                    "action": "execute",
                    "capability_id": "feature.ai-assistant",
                    "operation_id": "new-session"
                }),
                None,
            )
            .await
            .result
        );
        assert!(
            tool.validate_input(
                &json!({
                    "action": "configure",
                    "capability_id": "setting.application.general",
                    "option_id": "auto-update",
                    "value": false
                }),
                None,
            )
            .await
            .result
        );
    }

    #[test]
    fn discovery_is_permission_free_but_execution_is_scoped() {
        let tool = BitFunControlTool::new();
        assert!(tool
            .permission_intents(&json!({ "action": "search", "query": "theme" }), &context())
            .unwrap()
            .is_empty());
        let intents = tool
            .permission_intents(
                &json!({
                    "action": "configure",
                    "capability_id": "setting.application.general",
                    "option_id": "auto-update",
                    "value": false
                }),
                &context(),
            )
            .unwrap();
        assert_eq!(intents[0].action, "bitfun_control");
        assert_eq!(
            intents[0].resources,
            vec!["configure:setting.application.general:auto-update"]
        );

        let open_intents = tool
            .permission_intents(
                &json!({
                    "action": "open",
                    "capability_id": "setting.application.input",
                    "item_id": "shortcut-browser"
                }),
                &context(),
            )
            .unwrap();
        assert_eq!(
            open_intents[0].resources,
            vec!["open:setting.application.input:shortcut-browser"]
        );
    }
}
