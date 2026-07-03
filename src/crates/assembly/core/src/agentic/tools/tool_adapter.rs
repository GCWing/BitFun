//! Core product tool adapter for provider-neutral tool contracts.
//!
//! Keep these adapters in core until `ToolUseContext` and concrete tools have a
//! reviewed owner migration. Generic contracts live in `bitfun-agent-tools`;
//! this module only projects core-owned `Tool` behavior into those contracts.

use crate::agentic::tools::framework::{DynamicToolInfo, Tool, ToolExposure};
use crate::agentic::tools::tool_context_runtime::ToolUseContext;
use bitfun_agent_tools::{ContextualToolManifestItem, ToolRegistryItem};
use serde_json::Value;

#[async_trait::async_trait]
impl ToolRegistryItem for dyn Tool {
    fn name(&self) -> &str {
        Tool::name(self)
    }

    async fn description(&self) -> Result<String, String> {
        Tool::description(self)
            .await
            .map_err(|error| error.to_string())
    }

    fn input_schema(&self) -> Value {
        Tool::input_schema(self)
    }

    fn short_description(&self) -> String {
        Tool::short_description(self)
    }

    fn default_exposure(&self) -> ToolExposure {
        Tool::default_exposure(self)
    }

    fn is_readonly(&self) -> bool {
        Tool::is_readonly(self)
    }

    fn is_concurrency_safe(&self, input: Option<&Value>) -> bool {
        Tool::is_concurrency_safe(self, input)
    }

    fn needs_permissions(&self, input: Option<&Value>) -> bool {
        Tool::needs_permissions(self, input)
    }

    fn manages_own_execution_timeout(&self) -> bool {
        Tool::manages_own_execution_timeout(self)
    }

    async fn is_enabled(&self) -> bool {
        Tool::is_enabled(self).await
    }

    async fn input_schema_for_model(&self) -> Value {
        Tool::input_schema_for_model(self).await
    }

    fn dynamic_provider_id(&self) -> Option<&str> {
        Tool::dynamic_provider_id(self)
    }

    fn dynamic_tool_info(&self) -> Option<DynamicToolInfo> {
        Tool::dynamic_tool_info(self)
    }
}

#[async_trait::async_trait]
impl ContextualToolManifestItem<ToolUseContext> for dyn Tool {
    async fn is_available_in_context(&self, context: &ToolUseContext) -> bool {
        Tool::is_available_in_context(self, Some(context)).await
    }

    async fn description_with_context(&self, context: &ToolUseContext) -> Result<String, String> {
        Tool::description_with_context(self, Some(context))
            .await
            .map_err(|error| error.to_string())
    }

    async fn input_schema_for_model_with_context(
        &self,
        context: &ToolUseContext,
    ) -> serde_json::Value {
        Tool::input_schema_for_model_with_context(self, Some(context)).await
    }
}
