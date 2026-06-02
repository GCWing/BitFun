//! Product Tool Runtime owned GetToolSpec concrete adapter.

use super::{
    product_get_tool_spec_runtime, resolve_product_get_tool_spec_results,
    ProductGetToolSpecRuntime, ProductToolCatalogProvider,
};
use crate::agentic::tools::framework::{Tool, ToolRenderOptions, ToolResult, ValidationResult};
use crate::agentic::tools::tool_context_runtime::ToolUseContext;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_agent_tools::{
    build_get_tool_spec_collapsed_tool_entry, GetToolSpecCollapsedToolSummary,
    GetToolSpecExecutionError, GET_TOOL_SPEC_TOOL_NAME,
};
use serde_json::Value;

const GET_TOOL_SPEC_DESCRIPTION: &str = r#"Read usage instructions for additional tools.

Some tools are collapsed: their names may appear in the tool list, but you must not call them directly until you have loaded their definition with GetToolSpec.

When the current collapsed tool listing includes a <collapsed_tools> section, use the exact tool names from that section. Before using one of those tools, first call GetToolSpec with its exact tool name to read its full description and input schema. After reading the returned definition, call the real tool directly using its own name.

Do not call GetToolSpec again for a tool whose definition is already loaded in the current conversation."#;

pub struct GetToolSpecTool;

impl GetToolSpecTool {
    pub fn new() -> Self {
        Self
    }

    pub(crate) fn build_collapsed_tools_context_section(
        collapsed_tools: &[GetToolSpecCollapsedToolSummary],
    ) -> Option<String> {
        if collapsed_tools.is_empty() {
            return None;
        }

        let collapsed_tools_list = collapsed_tools
            .iter()
            .map(|tool| {
                build_get_tool_spec_collapsed_tool_entry(&tool.name, &tool.short_description)
            })
            .collect::<Vec<_>>()
            .join("\n");

        Some(format!(
            "<collapsed_tools>\n{}\n</collapsed_tools>",
            collapsed_tools_list
        ))
    }
}

impl Default for GetToolSpecTool {
    fn default() -> Self {
        Self::new()
    }
}

fn with_runtime<Result>(operation: impl FnOnce(ProductGetToolSpecRuntime<'_>) -> Result) -> Result {
    let provider = ProductToolCatalogProvider;
    operation(product_get_tool_spec_runtime(&provider))
}

#[async_trait]
impl Tool for GetToolSpecTool {
    fn name(&self) -> &str {
        GET_TOOL_SPEC_TOOL_NAME
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(GET_TOOL_SPEC_DESCRIPTION.to_string())
    }

    fn short_description(&self) -> String {
        with_runtime(|runtime| runtime.short_description())
    }

    async fn description_with_context(
        &self,
        _context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        Ok(GET_TOOL_SPEC_DESCRIPTION.to_string())
    }

    fn input_schema(&self) -> Value {
        with_runtime(|runtime| runtime.input_schema())
    }

    fn is_readonly(&self) -> bool {
        with_runtime(|runtime| runtime.is_readonly())
    }

    fn is_concurrency_safe(&self, input: Option<&Value>) -> bool {
        with_runtime(|runtime| runtime.is_concurrency_safe(input))
    }

    fn needs_permissions(&self, input: Option<&Value>) -> bool {
        with_runtime(|runtime| runtime.needs_permissions(input))
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        with_runtime(|runtime| runtime.render_tool_use_message(input))
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        with_runtime(|runtime| runtime.validate_input(input))
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        resolve_product_get_tool_spec_results(input, context, self.name())
            .await
            .map_err(map_get_tool_spec_execution_error)
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
    use crate::agentic::tools::framework::{Tool, ToolResult};
    use crate::agentic::tools::tool_context_runtime::ToolUseContext;
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn collapsed_tools_context_uses_explicit_short_description() {
        let tool_name = format!("CatalogDescriptionTestTool_{}", uuid::Uuid::new_v4());
        let description = GetToolSpecTool::build_collapsed_tools_context_section(&[
            bitfun_agent_tools::GetToolSpecCollapsedToolSummary {
                name: tool_name.clone(),
                short_description: "Concise catalog entry.".to_string(),
            },
        ])
        .expect("collapsed tools section");

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
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
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
