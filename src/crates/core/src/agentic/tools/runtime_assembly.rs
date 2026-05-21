//! Core-owned product tool runtime assembly.
//!
//! This module is the single core-side owner for assembling the product tool
//! registry while concrete tools, `ToolUseContext`, runtime manifest assembly,
//! and snapshot decoration remain core-owned.

use crate::agentic::tools::registry::{ProductToolDecoratorRef, ToolRef, ToolRegistry};
use crate::agentic::tools::static_providers::builtin_static_tool_providers;
#[cfg(test)]
use bitfun_agent_tools::StaticToolProvider;
use bitfun_agent_tools::{SnapshotToolDecorator, SnapshotToolWrapper, ToolRuntimeAssembly};
use std::sync::Arc;

#[derive(Clone)]
pub(in crate::agentic::tools) struct ProductToolRuntimeAssembly {
    tool_decorator: ProductToolDecoratorRef,
}

impl Default for ProductToolRuntimeAssembly {
    fn default() -> Self {
        Self::new()
    }
}

impl ProductToolRuntimeAssembly {
    pub(in crate::agentic::tools) fn new() -> Self {
        Self::with_tool_decorator(Arc::new(SnapshotToolDecorator::new(Arc::new(
            ProductSnapshotToolWrapper,
        ))))
    }

    pub(in crate::agentic::tools) fn with_tool_decorator(
        tool_decorator: ProductToolDecoratorRef,
    ) -> Self {
        Self { tool_decorator }
    }

    #[cfg(test)]
    pub(in crate::agentic::tools) fn provider_group_ids(&self) -> Vec<&'static str> {
        builtin_static_tool_providers()
            .iter()
            .map(|provider| provider.provider_id())
            .collect()
    }

    pub(in crate::agentic::tools) fn create_registry(&self) -> ToolRegistry {
        let providers = builtin_static_tool_providers();
        let inner = ToolRuntimeAssembly::with_tool_decorator(self.tool_decorator.clone())
            .create_registry_from_static_providers(&providers);
        ToolRegistry::from_inner(inner)
    }
}

#[derive(Debug, Clone)]
struct ProductSnapshotToolWrapper;

impl SnapshotToolWrapper<dyn crate::agentic::tools::framework::Tool>
    for ProductSnapshotToolWrapper
{
    fn wrap_for_snapshot_tracking(&self, tool: ToolRef) -> ToolRef {
        crate::service::snapshot::wrap_tool_for_snapshot_tracking(tool)
    }
}
