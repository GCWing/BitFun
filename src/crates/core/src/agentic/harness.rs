use bitfun_harness::{
    DescriptorHarnessProvider, HarnessCapability, HarnessRegistry, HarnessRegistryBuildError,
    HarnessRegistryBuilder, HarnessWorkflow,
};

pub const CORE_DEEP_REVIEW_HARNESS_PROVIDER_ID: &str = "core.deep_review";
pub const CORE_DEEP_RESEARCH_HARNESS_PROVIDER_ID: &str = "core.deep_research";
pub const CORE_MINIAPP_HARNESS_PROVIDER_ID: &str = "core.miniapp";

pub fn product_harness_registry() -> Result<HarnessRegistry, HarnessRegistryBuildError> {
    HarnessRegistryBuilder::new()
        .install_provider(DescriptorHarnessProvider::legacy_facade(
            CORE_DEEP_REVIEW_HARNESS_PROVIDER_ID,
            HarnessWorkflow::DeepReview,
            &[
                HarnessCapability::Plan,
                HarnessCapability::ReviewGate,
                HarnessCapability::PostProcessor,
            ],
            "bitfun-core::agentic::deep_review",
        ))
        .install_provider(DescriptorHarnessProvider::legacy_facade(
            CORE_DEEP_RESEARCH_HARNESS_PROVIDER_ID,
            HarnessWorkflow::DeepResearch,
            &[HarnessCapability::Plan, HarnessCapability::PostProcessor],
            "bitfun-core::agentic::agents::definitions::modes::deep_research",
        ))
        .install_provider(DescriptorHarnessProvider::legacy_facade(
            CORE_MINIAPP_HARNESS_PROVIDER_ID,
            HarnessWorkflow::MiniApp,
            &[HarnessCapability::Plan, HarnessCapability::Artifact],
            "bitfun-core::miniapp",
        ))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_harness::{HarnessInput, HarnessStepKind};

    #[test]
    fn product_harness_registry_registers_existing_workflow_facades() {
        let registry = product_harness_registry().expect("product harness registry should build");

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

    #[tokio::test]
    async fn product_harness_provider_plans_route_to_legacy_facade_without_execution() {
        let registry = product_harness_registry().expect("product harness registry should build");
        let provider = registry
            .provider_for_workflow(HarnessWorkflow::DeepResearch)
            .expect("DeepResearch should be registered");

        let plan = provider
            .plan(
                Default::default(),
                HarnessInput::new(HarnessWorkflow::DeepResearch, "research current question"),
            )
            .await
            .expect("DeepResearch harness should produce a legacy route plan");

        assert_eq!(plan.steps().len(), 1);
        assert_eq!(plan.steps()[0].kind(), HarnessStepKind::LegacyFacade);
        assert_eq!(
            plan.steps()[0].target(),
            "bitfun-core::agentic::agents::definitions::modes::deep_research"
        );

        assert!(
            provider.execute(Default::default(), plan).await.is_err(),
            "PR4 must not move concrete workflow execution out of legacy paths"
        );
    }
}
