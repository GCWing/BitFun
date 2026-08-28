use bitfun_product_capabilities::{
    product_assembly_plan_for_profile, DeliveryProfile, ProductCapabilityId,
};

const HEADLESS_PROFILES: &[DeliveryProfile] = &[
    DeliveryProfile::Cli,
    DeliveryProfile::Acp,
    DeliveryProfile::Sdk,
];

#[test]
fn headless_agent_hosts_select_only_the_code_agent_product_capability() {
    for profile in HEADLESS_PROFILES {
        let plan = product_assembly_plan_for_profile(*profile);

        assert_eq!(
            plan.capability_set().ids(),
            &[ProductCapabilityId::CodeAgent],
            "{profile} must not assemble desktop product workflows"
        );
    }
}

#[test]
fn code_agent_tools_are_selected_from_atomic_provider_groups() {
    let plan = product_assembly_plan_for_profile(DeliveryProfile::Sdk);
    let provider_ids = plan
        .tool_plan()
        .tool_provider_group_plan()
        .iter()
        .map(|provider| provider.provider_id())
        .collect::<Vec<_>>();

    assert_eq!(
        provider_ids,
        vec![
            "core.basic",
            "core.agent",
            "core.session",
            "core.git",
            "core.web",
            "core.mcp",
            "core.computer-use",
        ]
    );

    let tool_names = plan
        .tool_plan()
        .tool_provider_group_plan()
        .iter()
        .flat_map(|provider| provider.tool_names())
        .copied()
        .collect::<Vec<_>>();
    for explore_tool in [
        "AgentSpawn",
        "AgentSendInput",
        "AgentInterrupt",
        "AgentList",
        "AgentDelete",
        "PortForward",
        "BitFunControl",
    ] {
        assert!(
            tool_names.contains(&explore_tool),
            "{} must preserve explore tool {explore_tool}",
            DeliveryProfile::Sdk,
        );
    }
    for product_tool in [
        "LaunchReviewAgent",
        "submit_code_review",
        "GenerativeUI",
        "InitMiniApp",
        "FinalizeMiniApp",
        "PublishMiniApp",
        "CreateCanvas",
        "ReadCanvas",
    ] {
        assert!(
            !tool_names.contains(&product_tool),
            "{} must not expose {product_tool}",
            DeliveryProfile::Sdk,
        );
    }
}

#[test]
fn headless_agent_hosts_keep_explore_code_agents_without_product_workflow_agents() {
    for profile in HEADLESS_PROFILES {
        let plan = product_assembly_plan_for_profile(*profile);
        let agent_ids = plan.agent_ids();

        for product_agent in [
            "DeepResearch",
            "ResearchSpecialist",
            "DeepReview",
            "CodeReview",
            "ReviewWorker",
            "ReviewJudge",
            "ReviewFixer",
            "debug",
            "FileFinder",
        ] {
            assert!(
                !agent_ids.contains(&product_agent),
                "{profile} must not register {product_agent}"
            );
        }

        for code_agent in [
            "minimal",
            "agentic",
            "Explore",
            "GeneralPurpose",
            "Ultra",
            "SwarmPlanner",
            "SwarmWorker",
            "SwarmReviewer",
        ] {
            assert!(
                agent_ids.contains(&code_agent),
                "{profile} must keep the explore code agent {code_agent}"
            );
        }
    }
}
