//! GetToolSpec tool implementation

use crate::agentic::tools::catalog_provider::{
    build_product_get_tool_spec_catalog_description, resolve_product_get_tool_spec_execution_result,
};
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_agent_tools::{
    get_tool_spec_input_schema, get_tool_spec_is_concurrency_safe, get_tool_spec_is_readonly,
    get_tool_spec_needs_permissions, get_tool_spec_short_description,
    render_get_tool_spec_tool_use_message, validate_get_tool_spec_input, GetToolSpecExecutionError,
    GET_TOOL_SPEC_TOOL_NAME,
};
use serde_json::Value;

pub struct GetToolSpecTool;

impl GetToolSpecTool {
    pub fn new() -> Self {
        Self
    }

    async fn build_collapsed_tools_description(&self, context: Option<&ToolUseContext>) -> String {
        build_product_get_tool_spec_catalog_description(context).await
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
        GET_TOOL_SPEC_TOOL_NAME
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(self.build_collapsed_tools_description(None).await)
    }

    fn short_description(&self) -> String {
        get_tool_spec_short_description()
    }

    async fn description_with_context(
        &self,
        context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        Ok(self.build_collapsed_tools_description(context).await)
    }

    fn input_schema(&self) -> Value {
        get_tool_spec_input_schema()
    }

    fn is_readonly(&self) -> bool {
        get_tool_spec_is_readonly()
    }

    fn is_concurrency_safe(&self, input: Option<&Value>) -> bool {
        get_tool_spec_is_concurrency_safe(input)
    }

    fn needs_permissions(&self, input: Option<&Value>) -> bool {
        get_tool_spec_needs_permissions(input)
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        render_get_tool_spec_tool_use_message(input)
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        validate_get_tool_spec_input(input)
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let result = resolve_product_get_tool_spec_execution_result(input, context, self.name())
            .await
            .map_err(map_get_tool_spec_execution_error)?;
        Ok(vec![result])
    }
}

fn map_get_tool_spec_execution_error(error: GetToolSpecExecutionError) -> BitFunError {
    match error {
        GetToolSpecExecutionError::MissingToolName => BitFunError::tool(error.to_string()),
        GetToolSpecExecutionError::Detail(message) => BitFunError::Validation(message),
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
        assert!(!description.contains(&format!("- {}: Verbose description first line.", tool_name)));
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
