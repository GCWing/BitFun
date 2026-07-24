//! Core-owned product tool runtime owner.
//!
//! This module is the single core-side owner for assembling product tool
//! registry adapters, catalog manifests, GetToolSpec lookup, and snapshot
//! decoration. Concrete tools and `ToolUseContext` stay in core so this owner
//! remains an equivalent structural boundary rather than a behavior migration.

mod call_deferred_tool;
mod catalog;
mod get_tool_spec_tool;
mod loaded_spec_state;
mod materialization;
mod snapshot;

use crate::agentic::tools::registry::{ProductToolDecoratorRef, ToolRegistry};
use bitfun_agent_tools::SnapshotToolDecorator;
use bitfun_product_capabilities::{
    product_assembly_plan_for_profile, DeliveryProfile, ProductAssemblyPlan,
};
use materialization::create_product_tool_registry_from_plan;
use snapshot::ProductSnapshotToolWrapper;
use std::sync::Arc;

pub use call_deferred_tool::CallDeferredTool;
pub(crate) use catalog::{
    product_get_tool_spec_runtime, resolve_product_get_tool_spec_results,
    resolve_product_readonly_enabled_tools, resolve_product_resolved_tool_manifest,
    resolve_product_resolved_visible_tools, ProductGetToolSpecRuntime, ProductToolCatalogProvider,
};
pub use catalog::{ResolvedToolManifest, ResolvedVisibleTools};
pub use get_tool_spec_tool::GetToolSpecTool;
pub(crate) use loaded_spec_state::collect_product_loaded_deferred_tool_specs;

#[derive(Clone)]
pub(crate) struct ProductToolRuntime {
    tool_decorator: ProductToolDecoratorRef,
    assembly_plan: ProductAssemblyPlan,
}

impl Default for ProductToolRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl ProductToolRuntime {
    pub(crate) fn new() -> Self {
        Self::for_profile(DeliveryProfile::ProductFull)
    }

    pub(crate) fn for_profile(profile: DeliveryProfile) -> Self {
        Self::with_tool_decorator_and_assembly_plan(
            Arc::new(SnapshotToolDecorator::new(Arc::new(
                ProductSnapshotToolWrapper,
            ))),
            product_assembly_plan_for_profile(profile),
        )
    }

    pub(crate) fn with_tool_decorator(tool_decorator: ProductToolDecoratorRef) -> Self {
        Self::with_tool_decorator_and_assembly_plan(
            tool_decorator,
            product_assembly_plan_for_profile(DeliveryProfile::ProductFull),
        )
    }

    pub(crate) fn with_tool_decorator_and_assembly_plan(
        tool_decorator: ProductToolDecoratorRef,
        assembly_plan: ProductAssemblyPlan,
    ) -> Self {
        Self {
            tool_decorator,
            assembly_plan,
        }
    }

    pub(crate) fn create_registry(&self) -> ToolRegistry {
        let inner = create_product_tool_registry_from_plan(
            self.assembly_plan
                .capability_assembly()
                .tool_provider_group_plan(),
            self.tool_decorator.clone(),
        );
        ToolRegistry::from_inner(inner)
    }
}

#[cfg(test)]
mod tests {
    use super::ProductToolRuntime;
    use crate::agentic::tools::registry::create_tool_registry;
    use bitfun_product_capabilities::{product_assembly_plan_for_profile, DeliveryProfile};

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
            owner_registry.get_deferred_tool_names(),
            compatibility_registry.get_deferred_tool_names(),
            "product tool runtime owner must preserve deferred-tool exposure"
        );
    }

    #[test]
    fn product_tool_runtime_registry_preserves_provider_plan_order() {
        let assembly = product_assembly_plan_for_profile(DeliveryProfile::ProductFull)
            .capability_assembly()
            .clone();
        let planned_names = assembly
            .tool_provider_group_plan()
            .iter()
            .flat_map(|group| group.tool_names())
            .map(|tool_name| tool_name.to_string())
            .collect::<Vec<_>>();

        assert_eq!(planned_names, create_tool_registry().get_tool_names());
    }

    #[test]
    fn product_tool_runtime_can_consume_explicit_product_assembly_plan() {
        let runtime = ProductToolRuntime::for_profile(DeliveryProfile::Cli);
        let owner_registry = runtime.create_registry();
        let compatibility_registry = create_tool_registry();

        assert_eq!(
            owner_registry.get_tool_names(),
            compatibility_registry.get_tool_names()
        );
        assert_eq!(
            owner_registry.get_deferred_tool_names(),
            compatibility_registry.get_deferred_tool_names()
        );
    }

    #[test]
    fn sdk_and_cli_profiles_current_tool_plan_ceilings_match_without_sharing_identity() {
        let sdk = ProductToolRuntime::for_profile(DeliveryProfile::Sdk).create_registry();
        let cli = ProductToolRuntime::for_profile(DeliveryProfile::Cli).create_registry();

        assert_eq!(sdk.get_tool_names(), cli.get_tool_names());
        assert_eq!(sdk.get_deferred_tool_names(), cli.get_deferred_tool_names());
    }

    #[test]
    fn product_tool_runtime_keeps_no_direct_core_profiles_empty() {
        for profile in [
            DeliveryProfile::Server,
            DeliveryProfile::Remote,
            DeliveryProfile::Web,
            DeliveryProfile::MobileWeb,
        ] {
            let runtime = ProductToolRuntime::for_profile(profile);
            let registry = runtime.create_registry();

            assert!(
                registry.get_tool_names().is_empty(),
                "{profile} must not expose product-full tools"
            );
            assert!(
                registry.get_deferred_tool_names().is_empty(),
                "{profile} must not expose deferred product-full tools"
            );
        }
    }
}
