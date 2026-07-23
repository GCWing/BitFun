use bitfun_core::product_assembly::{
    DeliveryProfile, ProductAssembler, ProductAssemblyError, ProductAssemblyInput,
    ProductRuntimeParts,
};
use bitfun_runtime_services::RuntimeServices;

pub(crate) fn assemble_cli_runtime_parts(
    services: RuntimeServices,
) -> Result<ProductRuntimeParts, ProductAssemblyError> {
    assemble_runtime_parts(DeliveryProfile::Cli, services)
}

pub(crate) fn assemble_acp_runtime_parts(
    services: RuntimeServices,
) -> Result<ProductRuntimeParts, ProductAssemblyError> {
    assemble_runtime_parts(DeliveryProfile::Acp, services)
}

fn assemble_runtime_parts(
    profile: DeliveryProfile,
    services: RuntimeServices,
) -> Result<ProductRuntimeParts, ProductAssemblyError> {
    ProductAssembler::new().assemble(ProductAssemblyInput::new(profile, services))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{assemble_acp_runtime_parts, assemble_cli_runtime_parts};
    use crate::runtime::services::{CliClock, CliRuntimeEventSink, CliRuntimeServicesProvider};
    use bitfun_core::product_assembly::ProductServiceCapabilityStatus;
    use bitfun_core::product_assembly::{product_assembly_plan_for_profile, DeliveryProfile};
    use bitfun_runtime_ports::{
        PluginRuntimeAvailability, PluginRuntimeUnavailableReason, RuntimeServiceCapability,
    };

    #[test]
    fn cli_product_plan_declares_required_services_without_constructing_runtime_parts() {
        let plan = product_assembly_plan_for_profile(DeliveryProfile::Cli);

        assert_eq!(plan.profile(), DeliveryProfile::Cli);
        assert!(!plan.capability_assembly().service_requirements().is_empty());
        for capability in [
            RuntimeServiceCapability::FileSystem,
            RuntimeServiceCapability::Workspace,
            RuntimeServiceCapability::SessionStore,
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

    #[test]
    fn cli_product_assembly_consumes_production_runtime_services() {
        let workspace = tempfile::tempdir().expect("workspace");
        let services = CliRuntimeServicesProvider::new(
            workspace.path(),
            Arc::new(CliRuntimeEventSink::new(8)),
            Arc::new(CliClock),
        )
        .expect("provider")
        .build()
        .expect("runtime services");

        let parts = assemble_cli_runtime_parts(services).expect("CLI product runtime parts");

        assert_eq!(parts.plan().profile(), DeliveryProfile::Cli);
        assert!(parts.missing_service_requirements().is_empty());
        assert!(parts
            .service_availability()
            .iter()
            .all(|entry| entry.status() == ProductServiceCapabilityStatus::Available));
        assert!(matches!(
            parts.plugin_runtime().availability(),
            PluginRuntimeAvailability::Disabled {
                reason: PluginRuntimeUnavailableReason::NotBuilt
            }
        ));
        assert!(!parts.harness_registry().provider_ids().is_empty());
    }

    #[test]
    fn acp_product_assembly_uses_acp_profile_and_production_services() {
        let workspace = tempfile::tempdir().expect("workspace");
        let services = CliRuntimeServicesProvider::new(
            workspace.path(),
            Arc::new(CliRuntimeEventSink::new(8)),
            Arc::new(CliClock),
        )
        .expect("provider")
        .build()
        .expect("runtime services");

        let parts = assemble_acp_runtime_parts(services).expect("ACP product runtime parts");

        assert_eq!(parts.plan().profile(), DeliveryProfile::Acp);
        assert!(parts.missing_service_requirements().is_empty());
        assert!(parts
            .service_availability()
            .iter()
            .all(|entry| entry.status() == ProductServiceCapabilityStatus::Available));
        assert!(matches!(
            parts.plugin_runtime().availability(),
            PluginRuntimeAvailability::Disabled {
                reason: PluginRuntimeUnavailableReason::UnsupportedProfile
            }
        ));
        assert!(!parts.harness_registry().provider_ids().is_empty());
    }
}
