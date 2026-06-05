use bitfun_harness::{
    build_descriptor_harness_registry, DescriptorHarnessProvider, HarnessCapability, HarnessError,
    HarnessInput, HarnessProvider, HarnessProviderDescriptor, HarnessRegistryBuildError,
    HarnessRegistryBuilder, HarnessStepKind, HarnessWorkflow,
};

#[tokio::test]
async fn registry_registers_multiple_workflow_providers_and_builds_legacy_plan() {
    let registry = HarnessRegistryBuilder::new()
        .install_provider(DescriptorHarnessProvider::legacy_facade(
            "core.deep_review",
            HarnessWorkflow::DeepReview,
            &[HarnessCapability::Plan, HarnessCapability::ReviewGate],
            "bitfun-core::agentic::deep_review",
        ))
        .install_provider(DescriptorHarnessProvider::legacy_facade(
            "core.miniapp",
            HarnessWorkflow::MiniApp,
            &[HarnessCapability::Plan, HarnessCapability::Artifact],
            "bitfun-core::miniapp",
        ))
        .build()
        .expect("two different workflow providers should register");

    assert_eq!(
        registry.provider_ids(),
        vec!["core.deep_review", "core.miniapp"]
    );
    assert_eq!(
        registry.workflows(),
        vec![HarnessWorkflow::DeepReview, HarnessWorkflow::MiniApp]
    );

    let provider = registry
        .provider_for_workflow(HarnessWorkflow::DeepReview)
        .expect("deep review workflow should resolve");
    let plan = provider
        .plan(
            Default::default(),
            HarnessInput::new(HarnessWorkflow::DeepReview, "review current branch"),
        )
        .await
        .expect("legacy facade provider should produce a route plan");

    assert_eq!(plan.provider_id().as_str(), "core.deep_review");
    assert_eq!(plan.workflow(), HarnessWorkflow::DeepReview);
    assert_eq!(plan.steps().len(), 1);
    assert_eq!(plan.steps()[0].kind(), HarnessStepKind::LegacyFacade);
    assert_eq!(
        plan.steps()[0].target(),
        "bitfun-core::agentic::deep_review"
    );

    let err = provider
        .execute(Default::default(), plan)
        .await
        .expect_err("execution must stay on the legacy path in PR4");
    assert!(matches!(err, HarnessError::UnsupportedExecution { .. }));
}

#[test]
fn registry_rejects_duplicate_provider_ids() {
    let err = HarnessRegistryBuilder::new()
        .install_provider(DescriptorHarnessProvider::legacy_facade(
            "core.deep_review",
            HarnessWorkflow::DeepReview,
            &[HarnessCapability::Plan],
            "bitfun-core::agentic::deep_review",
        ))
        .install_provider(DescriptorHarnessProvider::legacy_facade(
            "core.deep_review",
            HarnessWorkflow::DeepResearch,
            &[HarnessCapability::Plan],
            "bitfun-core::agentic::agents::definitions::modes::deep_research",
        ))
        .build()
        .expect_err("duplicate provider ids must be rejected");

    assert!(matches!(
        err,
        HarnessRegistryBuildError::DuplicateProviderId { .. }
    ));
}

#[test]
fn descriptor_registry_builder_installs_legacy_facade_descriptors() {
    let registry = build_descriptor_harness_registry([
        HarnessProviderDescriptor::legacy_facade(
            "core.deep_review",
            HarnessWorkflow::DeepReview,
            &[HarnessCapability::Plan, HarnessCapability::ReviewGate],
            "bitfun-core::agentic::deep_review",
        ),
        HarnessProviderDescriptor::legacy_facade(
            "core.deep_research",
            HarnessWorkflow::DeepResearch,
            &[HarnessCapability::Plan],
            "bitfun-core::agentic::agents::definitions::modes::deep_research",
        ),
    ])
    .expect("descriptor registry should build");

    assert_eq!(
        registry.provider_ids(),
        vec!["core.deep_review", "core.deep_research"]
    );
    assert_eq!(
        registry.workflows(),
        vec![HarnessWorkflow::DeepReview, HarnessWorkflow::DeepResearch]
    );
}

#[test]
fn legacy_facade_provider_never_exposes_execute_capability() {
    let provider = DescriptorHarnessProvider::legacy_facade(
        "core.deep_review",
        HarnessWorkflow::DeepReview,
        &[HarnessCapability::Plan, HarnessCapability::Execute],
        "bitfun-core::agentic::deep_review",
    );

    assert_eq!(provider.capabilities(), &[HarnessCapability::Plan]);
}

#[tokio::test]
async fn descriptor_provider_rejects_wrong_workflow_input() {
    let provider = DescriptorHarnessProvider::legacy_facade(
        "core.miniapp",
        HarnessWorkflow::MiniApp,
        &[HarnessCapability::Plan],
        "bitfun-core::miniapp",
    );

    let err = provider
        .plan(
            Default::default(),
            HarnessInput::new(HarnessWorkflow::DeepReview, "wrong workflow"),
        )
        .await
        .expect_err("provider should not plan a different workflow");

    assert!(matches!(err, HarnessError::UnsupportedWorkflow { .. }));
}
