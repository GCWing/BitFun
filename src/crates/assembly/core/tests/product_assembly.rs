use bitfun_core::product_assembly;
use bitfun_core::product_runtime::CoreRuntimeServicesProvider;
use bitfun_product_capabilities::{
    product_assembly_plan_for_profile, DeliveryProfile, ProductServiceCapabilityStatus,
};
use bitfun_runtime_ports::RuntimeServiceCapability;
use bitfun_runtime_services::test_support::FakeRuntimeServicesProvider;
use bitfun_runtime_services::{RuntimeServicesBuilder, RuntimeServicesRegistry};

#[test]
fn core_runtime_services_provider_registers_existing_adapters_and_capability_markers() {
    let registry = RuntimeServicesRegistry::new()
        .with_provider(FakeRuntimeServicesProvider::with_all_required())
        .with_provider(CoreRuntimeServicesProvider::new());

    let services = registry
        .build(RuntimeServicesBuilder::new())
        .expect("core product assembly provider should register concrete adapters");

    assert_eq!(
        services.session_store.capability(),
        RuntimeServiceCapability::SessionStore
    );
    assert!(services.has_capability(RuntimeServiceCapability::Terminal));
    assert!(services.has_capability(RuntimeServiceCapability::Network));
    assert!(services.has_capability(RuntimeServiceCapability::Git));
    assert!(services.has_capability(RuntimeServiceCapability::McpCatalog));
    assert!(services.has_capability(RuntimeServiceCapability::RemoteWorkspace));
    assert!(services.has_capability(RuntimeServiceCapability::RemoteProjection));
}

#[test]
fn product_assembly_facade_preserves_legacy_provider_import_path() {
    let registry = RuntimeServicesRegistry::new()
        .with_provider(FakeRuntimeServicesProvider::with_all_required())
        .with_provider(product_assembly::CoreRuntimeServicesProvider::new());

    let services = registry
        .build(RuntimeServicesBuilder::new())
        .expect("legacy product assembly facade should preserve provider behavior");

    assert!(services.has_capability(RuntimeServiceCapability::SessionStore));
    assert!(services.has_capability(RuntimeServiceCapability::Terminal));
}

#[test]
fn core_provider_closes_current_product_full_service_capability_requirements() {
    let registry = RuntimeServicesRegistry::new()
        .with_provider(FakeRuntimeServicesProvider::with_all_required())
        .with_provider(CoreRuntimeServicesProvider::new());
    let services = registry
        .build(RuntimeServicesBuilder::new())
        .expect("core product assembly provider should build with required services");
    let plan = product_assembly_plan_for_profile(DeliveryProfile::ProductFull);

    let unavailable = plan
        .service_availability_report(|capability| services.has_capability(capability))
        .into_iter()
        .filter(|entry| entry.status() == ProductServiceCapabilityStatus::Unavailable)
        .collect::<Vec<_>>();

    assert!(
        unavailable.is_empty(),
        "product-full service requirements must be explicitly satisfied: {unavailable:?}"
    );
}
