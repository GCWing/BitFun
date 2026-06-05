pub use bitfun_product_capabilities::{
    default_product_harness_registry as product_harness_registry,
    product_harness_registry_for_profile, CORE_DEEP_RESEARCH_HARNESS_PROVIDER_ID,
    CORE_DEEP_REVIEW_HARNESS_PROVIDER_ID, CORE_MINIAPP_HARNESS_PROVIDER_ID,
};

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_harness::{HarnessInput, HarnessStepKind, HarnessWorkflow};
    use bitfun_product_capabilities::DeliveryProfile;

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

    #[test]
    fn product_harness_registry_can_be_built_from_explicit_delivery_profile() {
        let registry = product_harness_registry_for_profile(DeliveryProfile::Cli)
            .expect("profile-scoped product harness registry should build");

        assert_eq!(
            registry.provider_ids(),
            vec!["core.deep_review", "core.deep_research", "core.miniapp"]
        );
    }
}
