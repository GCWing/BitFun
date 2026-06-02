use bitfun_harness::{HarnessCapability, HarnessWorkflow};
use bitfun_product_capabilities::{
    default_product_capability_registry, default_product_harness_registry,
    default_product_tool_provider_group_plan, ProductCapabilityBuildError, ProductCapabilityId,
    ProductCapabilityPack, ProductCapabilityRegistry,
};
use bitfun_runtime_ports::RuntimeServiceCapability;

#[test]
fn default_capability_registry_preserves_product_tool_provider_order() {
    let provider_ids = default_product_tool_provider_group_plan()
        .iter()
        .map(|group| group.provider_id())
        .collect::<Vec<_>>();

    assert_eq!(
        provider_ids,
        vec![
            "core.basic",
            "core.agent",
            "core.session",
            "core.integration",
        ]
    );
}

#[test]
fn default_capability_registry_preserves_legacy_harness_routes() {
    let registry = default_product_harness_registry().expect("harness registry should build");

    assert_eq!(
        registry.provider_ids(),
        vec!["core.deep_review", "core.deep_research", "core.miniapp"]
    );
    assert_eq!(
        registry.workflows(),
        vec![
            HarnessWorkflow::DeepReview,
            HarnessWorkflow::DeepResearch,
            HarnessWorkflow::MiniApp,
        ]
    );
}

#[test]
fn capability_packs_describe_service_tool_and_harness_requirements() {
    let registry = default_product_capability_registry();

    let capability_ids = registry
        .capability_ids()
        .into_iter()
        .map(ProductCapabilityId::id)
        .collect::<Vec<_>>();
    assert_eq!(
        capability_ids,
        vec!["code-agent", "deep-review", "deep-research", "miniapp"]
    );

    let service_capabilities = registry.required_service_capabilities();
    assert!(service_capabilities.contains(&RuntimeServiceCapability::FileSystem));
    assert!(service_capabilities.contains(&RuntimeServiceCapability::Workspace));
    assert!(service_capabilities.contains(&RuntimeServiceCapability::Permission));
    assert!(service_capabilities.contains(&RuntimeServiceCapability::Events));

    let harness_capabilities = registry
        .harness_provider_descriptors()
        .into_iter()
        .map(|descriptor| {
            (
                descriptor.provider_id(),
                descriptor.workflow(),
                descriptor.capabilities().to_vec(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        harness_capabilities,
        vec![
            (
                "core.deep_review",
                HarnessWorkflow::DeepReview,
                vec![
                    HarnessCapability::Plan,
                    HarnessCapability::ReviewGate,
                    HarnessCapability::PostProcessor,
                ],
            ),
            (
                "core.deep_research",
                HarnessWorkflow::DeepResearch,
                vec![HarnessCapability::Plan, HarnessCapability::PostProcessor],
            ),
            (
                "core.miniapp",
                HarnessWorkflow::MiniApp,
                vec![HarnessCapability::Plan, HarnessCapability::Artifact],
            ),
        ]
    );
}

#[test]
fn capability_registry_rejects_unknown_tool_provider_groups() {
    static BROKEN_TOOL_GROUPS: &[&str] = &["core.missing"];
    static BROKEN_PACKS: &[ProductCapabilityPack] = &[ProductCapabilityPack::new(
        ProductCapabilityId::CodeAgent,
        &[],
        BROKEN_TOOL_GROUPS,
        &[],
    )];

    let error = ProductCapabilityRegistry::new(BROKEN_PACKS)
        .try_tool_provider_group_plan()
        .expect_err("unknown provider groups must not be silently dropped");

    assert_eq!(
        error,
        ProductCapabilityBuildError::UnknownToolProviderGroup {
            provider_id: "core.missing"
        }
    );
}
