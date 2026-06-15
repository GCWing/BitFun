use bitfun_harness::{HarnessCapability, HarnessWorkflow};
use bitfun_product_capabilities::{
    default_product_assembly_plan, default_product_capability_assembly,
    default_product_capability_registry, default_product_harness_registry,
    product_assembly_plan_for_profile, DeliveryProfile, ProductCapabilityBuildError,
    ProductCapabilityId, ProductCapabilityPack, ProductCapabilityRegistry,
    ProductServiceCapabilityRequirement, ProductServiceCapabilityStatus,
};
use bitfun_runtime_ports::RuntimeServiceCapability;

#[test]
fn default_capability_registry_preserves_product_tool_provider_order() {
    let assembly = default_product_capability_assembly();
    let provider_ids = assembly
        .tool_provider_group_plan()
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
fn product_assembly_plan_makes_delivery_profile_explicit_without_reducing_capabilities() {
    let expected_capabilities = vec!["code-agent", "deep-review", "deep-research", "miniapp"];
    let expected_tool_groups = vec![
        "core.basic",
        "core.agent",
        "core.session",
        "core.integration",
    ];

    for profile in DeliveryProfile::all_current_product_profiles()
        .iter()
        .copied()
    {
        let plan = product_assembly_plan_for_profile(profile);

        assert_eq!(plan.profile(), profile);
        assert_eq!(
            plan.capability_set()
                .ids()
                .iter()
                .map(|capability_id| capability_id.id())
                .collect::<Vec<_>>(),
            expected_capabilities,
            "{profile} must preserve the current product-full capability set until explicit trimming is proven"
        );
        assert_eq!(
            plan.capability_assembly()
                .tool_provider_group_plan()
                .iter()
                .map(|group| group.provider_id())
                .collect::<Vec<_>>(),
            expected_tool_groups,
            "{profile} must preserve current tool provider groups"
        );
    }
}

#[test]
fn product_assembly_plan_reports_service_availability_by_capability() {
    let plan = default_product_assembly_plan();

    let unavailable = plan
        .service_availability_report(|capability| {
            !matches!(
                capability,
                RuntimeServiceCapability::Git | RuntimeServiceCapability::Network
            )
        })
        .into_iter()
        .filter(|entry| entry.status() == ProductServiceCapabilityStatus::Unavailable)
        .collect::<Vec<_>>();

    assert_eq!(unavailable.len(), 2);
    assert_eq!(
        unavailable[0].requirement(),
        ProductServiceCapabilityRequirement::new(
            ProductCapabilityId::DeepReview,
            RuntimeServiceCapability::Git,
        )
    );
    assert_eq!(
        unavailable[1].requirement(),
        ProductServiceCapabilityRequirement::new(
            ProductCapabilityId::DeepResearch,
            RuntimeServiceCapability::Network,
        )
    );
}

#[test]
fn default_capability_assembly_keeps_service_tool_and_harness_facts_together() {
    let assembly = default_product_capability_assembly();

    let capability_ids = assembly
        .capability_ids()
        .iter()
        .map(|capability_id| capability_id.id())
        .collect::<Vec<_>>();
    assert_eq!(
        capability_ids,
        vec!["code-agent", "deep-review", "deep-research", "miniapp"]
    );

    let service_capabilities = assembly.required_service_capabilities();
    assert_eq!(
        service_capabilities,
        vec![
            RuntimeServiceCapability::FileSystem,
            RuntimeServiceCapability::Workspace,
            RuntimeServiceCapability::SessionStore,
            RuntimeServiceCapability::Permission,
            RuntimeServiceCapability::Events,
            RuntimeServiceCapability::Clock,
            RuntimeServiceCapability::Terminal,
            RuntimeServiceCapability::Git,
            RuntimeServiceCapability::Network,
        ]
    );

    let tool_provider_ids = assembly
        .tool_provider_group_plan()
        .iter()
        .map(|group| group.provider_id())
        .collect::<Vec<_>>();
    assert_eq!(
        tool_provider_ids,
        vec![
            "core.basic",
            "core.agent",
            "core.session",
            "core.integration"
        ]
    );

    let harness_provider_ids = assembly
        .harness_provider_descriptors()
        .iter()
        .map(|descriptor| descriptor.provider_id())
        .collect::<Vec<_>>();
    assert_eq!(
        harness_provider_ids,
        vec!["core.deep_review", "core.deep_research", "core.miniapp"]
    );
}

#[test]
fn capability_assembly_reports_missing_services_without_concrete_runtime_dependency() {
    let assembly = default_product_capability_assembly();

    let missing = assembly.missing_service_requirements(|capability| {
        !matches!(
            capability,
            RuntimeServiceCapability::Git | RuntimeServiceCapability::Network
        )
    });

    assert_eq!(
        missing,
        vec![
            ProductServiceCapabilityRequirement::new(
                ProductCapabilityId::DeepReview,
                RuntimeServiceCapability::Git,
            ),
            ProductServiceCapabilityRequirement::new(
                ProductCapabilityId::DeepResearch,
                RuntimeServiceCapability::Network,
            ),
        ]
    );

    assert!(
        assembly
            .missing_service_requirements(|_capability| true)
            .is_empty(),
        "fully assembled product runtime must report no service capability gaps"
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

    let registry = ProductCapabilityRegistry::new(BROKEN_PACKS);
    let harness_registry = registry
        .build_harness_registry()
        .expect("harness registry should not depend on tool provider group validity");
    assert!(harness_registry.provider_ids().is_empty());

    let error = registry
        .try_tool_provider_group_plan()
        .expect_err("unknown provider groups must not be silently dropped");

    assert_eq!(
        error,
        ProductCapabilityBuildError::UnknownToolProviderGroup {
            provider_id: "core.missing"
        }
    );
}
