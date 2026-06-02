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

use crate::agentic::tools::framework::Tool;
use crate::agentic::tools::registry::{ProductToolDecoratorRef, ToolRegistry};
#[cfg(test)]
use bitfun_agent_tools::StaticToolProvider;
use bitfun_agent_tools::{SnapshotToolDecorator, StaticToolProviderGroup, ToolRuntimeAssembly};
use bitfun_tool_packs::product_tool_provider_group_plan;
use materialization::ProductToolMaterializer;
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

    #[cfg(test)]
    pub(in crate::agentic::tools) fn provider_group_ids(&self) -> Vec<&'static str> {
        builtin_static_tool_providers()
            .iter()
            .map(|provider| provider.provider_id())
            .collect()
    }

    #[cfg(test)]
    pub(in crate::agentic::tools) fn provider_tool_names(&self) -> Vec<String> {
        builtin_static_tool_providers()
            .into_iter()
            .flat_map(|provider| provider.tools())
            .map(|tool| tool.name().to_string())
            .collect()
    }

    pub(crate) fn create_registry(&self) -> ToolRegistry {
        let providers = builtin_static_tool_providers();
        let inner = ToolRuntimeAssembly::with_tool_decorator(self.tool_decorator.clone())
            .create_registry_from_static_providers(&providers);
        ToolRegistry::from_inner(inner)
    }
}

fn builtin_static_tool_providers() -> Vec<StaticToolProviderGroup<dyn Tool>> {
    ProductToolMaterializer.materialize_provider_groups(product_tool_provider_group_plan())
}

#[cfg(test)]
mod tests {
    use super::{materialization::ProductToolMaterializer, ProductToolRuntime};
    use crate::agentic::tools::registry::create_tool_registry;
    use bitfun_agent_tools::StaticToolProvider;
    use bitfun_tool_packs::product_tool_provider_group_plan;

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
    fn product_tool_materializer_preserves_provider_plan_order() {
        let materializer = ProductToolMaterializer::default();
        let providers =
            materializer.materialize_provider_groups(product_tool_provider_group_plan());
        let provider_ids = providers
            .iter()
            .map(|provider| provider.provider_id())
            .collect::<Vec<_>>();
        let planned_ids = product_tool_provider_group_plan()
            .iter()
            .map(|group| group.provider_id())
            .collect::<Vec<_>>();

        assert_eq!(provider_ids, planned_ids);

        let materialized_names = providers
            .into_iter()
            .flat_map(|provider| provider.tools())
            .map(|tool| tool.name().to_string())
            .collect::<Vec<_>>();
        let registry_names = create_tool_registry().get_tool_names();

        assert_eq!(materialized_names, registry_names);
    }
}
