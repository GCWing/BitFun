//! GetToolSpec tool implementation

use crate::agentic::agents::get_agent_registry;
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::resolve_visible_tools;
use crate::agentic::tools::registry::get_global_tool_registry;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use log::debug;
use serde_json::{json, Value};
use std::sync::Arc;

pub struct GetToolSpecTool;

impl GetToolSpecTool {
    pub fn new() -> Self {
        Self
    }

    fn escape_xml_text(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }

    fn render_collapsed_tools_description(&self, collapsed_tools_list: String) -> String {
        format!(
            r#"Read usage instructions for additional tools.

You have access to an additional tools listed below.

<collapsed_tools>
{}
</collapsed_tools>

Before using one of these tools, first call GetToolSpec with its exact tool name to read its full description and input schema.

After reading the returned definition, call the real tool directly using its own name.

Do not call GetToolSpec again for a tool whose definition is already loaded in the current conversation.

Example:
- Suppose the catalog includes a tool named `GetWeather` and you need to use it.
- First call `GetToolSpec` with `{{"tool_name":"GetWeather"}}`
- Then read the returned schema and call `GetWeather` itself with the appropriate arguments
"#,
            collapsed_tools_list
        )
    }

    async fn get_contextual_collapsed_tools(
        &self,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<Arc<dyn Tool>>> {
        let agent_type = context.agent_type.as_deref().ok_or_else(|| {
            BitFunError::Validation("GetToolSpec requires agent type context".to_string())
        })?;
        let workspace_root = context.workspace_root();
        let agent_registry = get_agent_registry();
        let policy = agent_registry
            .get_agent_tool_policy(agent_type, workspace_root)
            .await;
        let visible_tools =
            resolve_visible_tools(&policy.allowed_tools, &policy.exposure_overrides, context).await;
        Ok(visible_tools.collapsed_tools)
    }

    async fn build_collapsed_tools_description(&self, context: Option<&ToolUseContext>) -> String {
        let mut entries = Vec::new();

        if let Some(context) = context {
            if let Ok(collapsed_tools) = self.get_contextual_collapsed_tools(context).await {
                for tool in collapsed_tools {
                    entries.push(format!("- {}", tool.name()));
                }
            }
        } else {
            let registry = get_global_tool_registry();
            let collapsed_tools = {
                let registry = registry.read().await;
                registry
                    .get_all_tools()
                    .into_iter()
                    .filter(|tool| {
                        tool.default_exposure()
                            == crate::agentic::tools::framework::ToolExposure::Collapsed
                    })
                    .map(|tool| (tool.name().to_string(), tool.short_description()))
                    .collect::<Vec<_>>()
            };

            for (tool_name, short_description) in collapsed_tools {
                entries.push(format!("- {}: {}", tool_name, short_description));
            }
        }

        let collapsed_tools_list = if entries.is_empty() {
            "No additional tools are available.".to_string()
        } else {
            entries.join("\n")
        };

        self.render_collapsed_tools_description(collapsed_tools_list)
    }

    async fn build_tool_detail(
        &self,
        tool_name: &str,
        context: Option<&ToolUseContext>,
    ) -> BitFunResult<Value> {
        let context = context.ok_or_else(|| {
            BitFunError::Validation("GetToolSpec requires execution context".to_string())
        })?;
        let collapsed_tools = self.get_contextual_collapsed_tools(context).await?;
        let tool = collapsed_tools
            .into_iter()
            .find(|tool| tool.name() == tool_name)
            .ok_or_else(|| {
                BitFunError::Validation(format!(
                    "Tool '{}' is not an available collapsed tool in the current context",
                    tool_name
                ))
            })?;

        if tool.name() == self.name() {
            return Err(BitFunError::Validation(format!(
                "Tool '{}' cannot inspect itself",
                tool_name
            )));
        }

        let description = tool
            .description_with_context(Some(context))
            .await
            .unwrap_or_else(|_| format!("Tool: {}", tool.name()));
        let input_schema = tool.input_schema_for_model_with_context(Some(context)).await;

        Ok(json!({
            "tool_name": tool_name,
            "description": description,
            "input_schema": input_schema
        }))
    }
}

impl Default for GetToolSpecTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GetToolSpecTool {
    fn name(&self) -> &str {
        "GetToolSpec"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(self.build_collapsed_tools_description(None).await)
    }

    fn short_description(&self) -> String {
        "Discover collapsed tools and read their detailed definitions.".to_string()
    }

    async fn description_with_context(
        &self,
        context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        Ok(self.build_collapsed_tools_description(context).await)
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["tool_name"],
            "properties": {
                "tool_name": {
                    "type": "string",
                    "description": "The exact tool name to read details for."
                }
            }
        })
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

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let tool_name = input
            .get("tool_name")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        format!("Reading tool spec for '{}'.", tool_name)
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let Some(tool_name) = input.get("tool_name").and_then(|v| v.as_str()) else {
            return ValidationResult {
                result: false,
                message: Some("tool_name is required and cannot be empty".to_string()),
                error_code: Some(400),
                meta: None,
            };
        };

        if tool_name.is_empty() {
            return ValidationResult {
                result: false,
                message: Some("tool_name is required and cannot be empty".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        ValidationResult::default()
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let tool_name = input
            .get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("tool_name is required".to_string()))?;

        if context
            .unlocked_collapsed_tools
            .iter()
            .any(|loaded| loaded == tool_name)
        {
            return Ok(vec![ToolResult::Result {
                data: json!({
                    "tool_name": tool_name,
                    "already_loaded": true
                }),
                result_for_assistant: Some(format!(
                    "Tool '{}' is already loaded in the current conversation. Do not call GetToolSpec again for it. Use '{}' directly.",
                    tool_name, tool_name
                )),
                image_attachments: None,
            }]);
        }

        debug!("GetToolSpec reading tool: {}", tool_name);
        let detail = self.build_tool_detail(tool_name, Some(context)).await?;
        let description = detail
            .get("description")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let input_schema = detail
            .get("input_schema")
            .map(|value| value.to_string())
            .unwrap_or_else(|| "{}".to_string());
        let assistant_detail = format!(
            "<description>\n{}\n</description>\n<input_schema>\n{}\n</input_schema>",
            Self::escape_xml_text(description),
            Self::escape_xml_text(&input_schema)
        );

        Ok(vec![ToolResult::Result {
            data: detail,
            result_for_assistant: Some(assistant_detail),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::GetToolSpecTool;
    use crate::agentic::tools::framework::{
        Tool, ToolExposure, ToolResult, ToolUseContext, ValidationResult,
    };
    use crate::agentic::tools::registry::get_global_tool_registry;
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::util::errors::BitFunResult;
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::sync::Arc;

    struct CatalogDescriptionTestTool {
        name: String,
    }

    #[async_trait]
    impl Tool for CatalogDescriptionTestTool {
        fn name(&self) -> &str {
            &self.name
        }

        async fn description(&self) -> BitFunResult<String> {
            Ok("Verbose description first line.\nSecond line.".to_string())
        }

        fn short_description(&self) -> String {
            "Concise catalog entry.".to_string()
        }

        fn default_exposure(&self) -> ToolExposure {
            ToolExposure::Collapsed
        }

        fn input_schema(&self) -> Value {
            json!({ "type": "object" })
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
            _input: &Value,
            _context: &ToolUseContext,
        ) -> BitFunResult<Vec<ToolResult>> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn get_tool_spec_uses_explicit_short_description() {
        let tool_name = format!("CatalogDescriptionTestTool_{}", uuid::Uuid::new_v4());
        let registry = get_global_tool_registry();
        {
            let mut registry = registry.write().await;
            registry.register_tool(Arc::new(CatalogDescriptionTestTool {
                name: tool_name.clone(),
            }));
        }

        let description = GetToolSpecTool::new()
            .build_collapsed_tools_description(None)
            .await;

        assert!(description.contains(&format!("- {}: Concise catalog entry.", tool_name)));
        assert!(!description.contains(&format!(
            "- {}: Verbose description first line.",
            tool_name
        )));
    }

    #[tokio::test]
    async fn reloading_already_unlocked_tool_returns_assistant_hint() {
        let tool = GetToolSpecTool::new();
        let context = ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            unlocked_collapsed_tools: vec!["WebFetch".to_string()],
            custom_data: HashMap::new(),
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: None,
        };

        let results = tool
            .call_impl(&json!({ "tool_name": "WebFetch" }), &context)
            .await;

        let results = results.expect("duplicate load should return a normal result");
        let ToolResult::Result {
            data,
            result_for_assistant,
            ..
        } = &results[0]
        else {
            panic!("expected regular tool result");
        };

        assert_eq!(data["tool_name"], "WebFetch");
        assert_eq!(data["already_loaded"], true);
        assert!(result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .contains("already loaded in the current conversation"));
    }
}
