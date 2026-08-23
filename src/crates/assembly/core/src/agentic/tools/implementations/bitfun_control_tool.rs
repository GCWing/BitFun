//! BitFunControl — discover and control user-facing BitFun features and settings.

use crate::agentic::agents::get_agent_registry;
use crate::agentic::tools::bitfun_control_host::{
    bitfun_control_host_available, invoke_bitfun_control, BitFunControlHostRequest,
    ProductControlAction,
};
use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_product_domains::product_control::{
    capability as product_capability, discover as discover_product_capabilities,
    inspect_contract as inspect_product_control_contract, validate_open_target,
    validate_operation_argument_scopes, validate_operation_arguments, validate_option_value,
    ProductControlRisk,
};
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

    fn typed_action(action: &str) -> Option<ProductControlAction> {
        match action {
            "list" => Some(ProductControlAction::List),
            "search" => Some(ProductControlAction::Search),
            "get" => Some(ProductControlAction::Get),
            "open" => Some(ProductControlAction::Open),
            "execute" => Some(ProductControlAction::Execute),
            "configure" => Some(ProductControlAction::Configure),
            _ => None,
        }
    }

    fn agent_is_readonly(context: &ToolUseContext) -> bool {
        let Some(agent_type) = context.agent_type.as_deref() else {
            return false;
        };
        get_agent_registry()
            .get_agent(agent_type, context.workspace_root())
            .is_some_and(|agent| agent.is_readonly())
    }

    fn operation_is_readonly(input: &Value) -> bool {
        let (Some(capability_id), Some(operation_id)) = (
            input.get("capability_id").and_then(Value::as_str),
            input.get("operation_id").and_then(Value::as_str),
        ) else {
            return false;
        };
        product_capability(capability_id)
            .ok()
            .and_then(|capability| {
                capability
                    .operations
                    .iter()
                    .find(|operation| operation.id == operation_id)
            })
            .is_some_and(|operation| operation.risk == ProductControlRisk::Read)
    }

    fn validate_operation_scope(input: &Value, context: &ToolUseContext) -> Result<(), String> {
        if Self::action(input) != Some("execute") {
            return Ok(());
        }
        let capability_id = input
            .get("capability_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let operation_id = input
            .get("operation_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let capability = product_capability(capability_id)?;
        let operation = capability
            .operations
            .iter()
            .find(|operation| operation.id == operation_id)
            .ok_or_else(|| format!("Operation {operation_id} is not exposed by {capability_id}"))?;
        validate_operation_argument_scopes(operation, input.get("arguments"), context.is_remote())
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
            "Control BitFun features and settings through its internal API. Use a two-step flow: (1) call `list` or `search`, then `get` the relevant capability; (2) follow the returned item `control.kind`: `direct` uses `execute`/`configure`, `delegate` calls the named owning tool, and `open` opens the exact BitFun UI. The catalog is loaded only on demand and is not embedded here."
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
                "arguments": {
                    "type": "object",
                    "description": "Arguments for execute, following the operation input schema returned by get. Omit for operations with no arguments."
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
                    "description": "Maximum results per page. Defaults to 50 for list and 20 for search; use nextCursor to continue."
                }
            }
        })
    }

    async fn is_available_in_context(&self, _context: Option<&ToolUseContext>) -> bool {
        bitfun_product_domains::product_control::catalog().is_ok()
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
        if action == "execute" && Self::operation_is_readonly(input) {
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
        context: Option<&ToolUseContext>,
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
        if input
            .get("arguments")
            .is_some_and(|arguments| !arguments.is_object())
        {
            return invalid("arguments must be an object when provided.");
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
        if matches!(action, "get" | "open") {
            let capability_id = input
                .get("capability_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if action == "open" {
                if let Err(error) = validate_open_target(
                    capability_id,
                    input.get("item_id").and_then(Value::as_str),
                ) {
                    return invalid(&error);
                }
            } else if product_capability(capability_id).is_err() {
                return invalid("capability_id does not identify a known BitFun capability.");
            }
        }
        if action == "execute" {
            let capability_id = input
                .get("capability_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let operation_id = input
                .get("operation_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let Ok(capability) = product_capability(capability_id) else {
                return invalid("capability_id does not identify a known BitFun capability.");
            };
            let Some(operation) = capability
                .operations
                .iter()
                .find(|operation| operation.id == operation_id)
            else {
                return invalid("operation_id is not exposed by this BitFun capability.");
            };
            if let Err(error) =
                validate_operation_arguments(&operation.input_schema, input.get("arguments"))
            {
                return invalid(&error);
            }
        }
        if action == "configure" {
            let capability_id = input
                .get("capability_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let option_id = input
                .get("option_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let Ok(capability) = product_capability(capability_id) else {
                return invalid("capability_id does not identify a known BitFun capability.");
            };
            let Some(option) = capability
                .options
                .iter()
                .find(|option| option.id == option_id)
            else {
                return invalid("option_id is not exposed by this BitFun setting.");
            };
            if let Some(value) = input.get("value") {
                if let Err(error) = validate_option_value(&option.value_schema, value) {
                    return invalid(&error);
                }
            }
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
        if let Some(context) = context {
            if let Err(error) = Self::validate_operation_scope(input, context) {
                return invalid(&error);
            }
        }
        ValidationResult::default()
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let action = Self::action(input)
            .ok_or_else(|| BitFunError::tool("action is required".to_string()))?;
        let typed_action = Self::typed_action(action).ok_or_else(|| {
            BitFunError::tool(format!("Unsupported BitFunControl action: {action}"))
        })?;
        Self::validate_operation_scope(input, context).map_err(BitFunError::tool)?;
        let mutating_control = matches!(action, "open" | "configure")
            || (action == "execute" && !Self::operation_is_readonly(input));
        if mutating_control && Self::agent_is_readonly(context) {
            return Err(BitFunError::tool(
                "This read-only agent may discover BitFun features and settings but cannot control them"
                    .to_string(),
            ));
        }

        let request = BitFunControlHostRequest {
            action: typed_action,
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
            arguments: input.get("arguments").cloned(),
            value: input.get("value").cloned(),
            cursor: input
                .get("cursor")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok()),
            limit: input
                .get("limit")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok()),
        };

        let result = match typed_action {
            ProductControlAction::List | ProductControlAction::Search => {
                discover_product_capabilities(&request).map_err(BitFunError::tool)?
            }
            ProductControlAction::Get => {
                let capability_id = request
                    .capability_id
                    .as_deref()
                    .ok_or_else(|| BitFunError::tool("capability_id is required".to_string()))?;
                if bitfun_control_host_available() {
                    match invoke_bitfun_control(request.clone()).await {
                        Ok(result) => result,
                        Err(error) => {
                            let mut result = inspect_product_control_contract(capability_id)
                                .map_err(BitFunError::tool)?;
                            if let Some(object) = result.as_object_mut() {
                                object.insert(
                                    "controlAvailability".to_string(),
                                    json!({
                                        "status": "degraded",
                                        "contractAvailable": true,
                                        "readBack": false,
                                        "reason": error,
                                    }),
                                );
                            }
                            result
                        }
                    }
                } else {
                    let mut result = inspect_product_control_contract(capability_id)
                        .map_err(BitFunError::tool)?;
                    if let Some(object) = result.as_object_mut() {
                        object.insert(
                            "controlAvailability".to_string(),
                            json!({
                                "status": "unavailable",
                                "contractAvailable": true,
                                "readBack": false,
                                "reason": "This product surface has no BitFun control adapter",
                            }),
                        );
                    }
                    result
                }
            }
            ProductControlAction::Open
            | ProductControlAction::Execute
            | ProductControlAction::Configure => {
                if !bitfun_control_host_available() {
                    return Err(BitFunError::tool(
                        "This BitFun product surface can discover the capability but does not provide its control adapter"
                            .to_string(),
                    ));
                }
                invoke_bitfun_control(request)
                    .await
                    .map_err(BitFunError::tool)?
            }
        };

        let assistant = match action {
            "list" | "search" => {
                let count = result
                    .get("items")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or_default();
                format!("BitFun feature and setting discovery returned {count} item(s). If nextCursor is present, continue the same discovery action with that cursor. Call get for the relevant capability, then follow the returned item control route.")
            }
            "get" => "Loaded the BitFun feature or setting manual. Follow each item's control.kind: direct uses BitFunControl, delegate names the owning tool, and open routes to the exact UI.".to_string(),
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

    fn remote_context() -> ToolUseContext {
        let mut context = context();
        context.workspace = Some(crate::agentic::WorkspaceBinding::new_remote(
            None,
            std::path::PathBuf::from("/remote/workspace"),
            "connection-1".to_string(),
            "Remote".to_string(),
            crate::service::remote_ssh::workspace_state::WorkspaceSessionIdentity {
                hostname: "remote.example".to_string(),
                logical_workspace_path: "/remote/workspace".to_string(),
                remote_connection_id: Some("connection-1".to_string()),
            },
        ));
        context
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
        assert!(
            !tool
                .validate_input(
                    &json!({
                        "action": "execute",
                        "capability_id": "setting.application.pet",
                        "operation_id": "use-pet",
                        "arguments": {}
                    }),
                    None,
                )
                .await
                .result
        );
        assert!(
            !tool
                .validate_input(
                    &json!({
                        "action": "open",
                        "capability_id": "setting.application.input",
                        "item_id": "removed-setting-row"
                    }),
                    None,
                )
                .await
                .result
        );
        assert!(
            !tool
                .validate_input(
                    &json!({
                        "action": "configure",
                        "capability_id": "setting.application.pet",
                        "option_id": "display-mode",
                        "value": "floating"
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

        let read_operation = json!({
            "action": "execute",
            "capability_id": "setting.application.pet",
            "operation_id": "list-pets"
        });
        assert!(BitFunControlTool::operation_is_readonly(&read_operation));
        assert!(tool
            .permission_intents(&read_operation, &context())
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn headless_profiles_discover_contracts_and_degrade_control_explicitly() {
        assert!(!bitfun_control_host_available());
        let tool = BitFunControlTool::new();
        let results = tool
            .call_impl(
                &json!({
                    "action": "get",
                    "capability_id": "setting.application.pet"
                }),
                &context(),
            )
            .await
            .unwrap();
        let ToolResult::Result { data, .. } = &results[0] else {
            panic!("expected a structured product-control result");
        };
        assert_eq!(data["controlAvailability"]["status"], "unavailable");
        assert_eq!(data["controlAvailability"]["contractAvailable"], true);

        let error = tool
            .call_impl(
                &json!({
                    "action": "execute",
                    "capability_id": "setting.application.pet",
                    "operation_id": "list-pets"
                }),
                &context(),
            )
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("does not provide its control adapter"));
    }

    #[tokio::test]
    async fn remote_workspaces_reject_product_host_paths_but_allow_stable_ids() {
        let tool = BitFunControlTool::new();
        let path_request = json!({
            "action": "execute",
            "capability_id": "setting.application.pet",
            "operation_id": "use-pet",
            "arguments": { "path": "/remote/workspace/petdex" }
        });
        let remote = remote_context();
        let validation = tool.validate_input(&path_request, Some(&remote)).await;
        assert!(!validation.result);
        assert!(validation
            .message
            .as_deref()
            .is_some_and(|message| message.contains("product-host path")));

        let id_request = json!({
            "action": "execute",
            "capability_id": "setting.application.pet",
            "operation_id": "use-pet",
            "arguments": { "id": "bitfun" }
        });
        assert!(tool.validate_input(&id_request, Some(&remote)).await.result);
    }
}
