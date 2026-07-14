use bitfun_core::product_assembly::{
    product_assembly_plan_for_profile, DeliveryProfile, ProductAssemblyPlan,
};

pub(crate) fn cli_product_assembly_plan() -> ProductAssemblyPlan {
    product_assembly_plan_for_profile(DeliveryProfile::Cli)
}

#[cfg(test)]
mod tests {
    use super::cli_product_assembly_plan;
    use bitfun_core::product_assembly::DeliveryProfile;
    use bitfun_runtime_ports::{
        PluginRuntimeAvailability, PluginRuntimeUnavailableReason, RuntimeServiceCapability,
    };

    #[test]
    fn cli_product_plan_declares_required_services_without_constructing_runtime_parts() {
        let plan = cli_product_assembly_plan();

        assert_eq!(plan.profile(), DeliveryProfile::Cli);
        assert!(!plan.capability_assembly().service_requirements().is_empty());
        for capability in [
            RuntimeServiceCapability::FileSystem,
            RuntimeServiceCapability::Workspace,
            RuntimeServiceCapability::SessionStore,
            RuntimeServiceCapability::Permission,
            RuntimeServiceCapability::Events,
            RuntimeServiceCapability::Clock,
            RuntimeServiceCapability::Terminal,
            RuntimeServiceCapability::Git,
            RuntimeServiceCapability::Network,
        ] {
            assert!(
                plan.capability_assembly()
                    .service_requirements()
                    .iter()
                    .any(|entry| entry.service_capability() == capability),
                "CLI profile should require {capability}"
            );
        }
        assert_eq!(
            plan.extension_capabilities().plugin_runtime(),
            PluginRuntimeAvailability::Disabled {
                reason: PluginRuntimeUnavailableReason::NotBuilt,
            }
        );
    }
}
