//! Core-owned product tool runtime owner.
//!
//! This module is the single core-side owner for assembling product tool
//! registry adapters, catalog manifests, GetToolSpec lookup, and snapshot
//! decoration. Concrete tools and `ToolUseContext` stay in core so this owner
//! remains an equivalent structural boundary rather than a behavior migration.

mod catalog;
mod get_tool_spec_tool;
mod materialization;
mod snapshot;
mod unlock_state;

use crate::agentic::tools::registry::{ProductToolDecoratorRef, ToolRegistry};
use bitfun_agent_tools::SnapshotToolDecorator;
use materialization::create_product_tool_registry_from_plan;
use snapshot::ProductSnapshotToolWrapper;
use std::sync::Arc;

pub(crate) use catalog::{
    product_get_tool_spec_runtime, resolve_product_get_tool_spec_results,
    resolve_product_readonly_enabled_tools, resolve_product_resolved_tool_manifest,
    resolve_product_resolved_visible_tools, ProductGetToolSpecRuntime, ProductToolCatalogProvider,
};
pub use catalog::{ResolvedToolManifest, ResolvedVisibleTools};
pub use get_tool_spec_tool::GetToolSpecTool;
pub(crate) use unlock_state::collect_product_unlocked_collapsed_tools;

#[derive(Clone)]
pub(crate) struct ProductToolRuntime {
    tool_decorator: ProductToolDecoratorRef,
}

impl Default for ProductToolRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl ProductToolRuntime {
    pub(crate) fn new() -> Self {
        Self::with_tool_decorator(Arc::new(SnapshotToolDecorator::new(Arc::new(
            ProductSnapshotToolWrapper,
        ))))
    }

    pub(crate) fn with_tool_decorator(tool_decorator: ProductToolDecoratorRef) -> Self {
        Self { tool_decorator }
    }

    pub(crate) fn create_registry(&self) -> ToolRegistry {
        let assembly = bitfun_product_capabilities::default_product_capability_assembly();
        let inner = create_product_tool_registry_from_plan(
            assembly.tool_provider_group_plan(),
            self.tool_decorator.clone(),
        );
        ToolRegistry::from_inner(inner)
    }
}

#[cfg(test)]
mod tests {
    use super::ProductToolRuntime;
    use crate::agentic::tools::registry::create_tool_registry;
    use bitfun_product_capabilities::default_product_capability_assembly;

    #[test]
    fn product_tool_runtime_owner_preserves_registry_contract() {
        let runtime = ProductToolRuntime::default();
        let owner_registry = runtime.create_registry();
        let compatibility_registry = create_tool_registry();

        assert_eq!(
            owner_registry.get_tool_names(),
            compatibility_registry.get_tool_names(),
            "product tool runtime owner must preserve legacy registry output"
        );
        assert_eq!(
            owner_registry.get_collapsed_tool_names(),
            compatibility_registry.get_collapsed_tool_names(),
            "product tool runtime owner must preserve collapsed-tool exposure"
        );
    }

    #[test]
    fn product_tool_runtime_registry_preserves_provider_plan_order() {
        let assembly = default_product_capability_assembly();
        let planned_names = assembly
            .tool_provider_group_plan()
            .iter()
            .flat_map(|group| group.tool_names())
            .map(|tool_name| tool_name.to_string())
            .collect::<Vec<_>>();

        assert_eq!(planned_names, create_tool_registry().get_tool_names());
    }
}
