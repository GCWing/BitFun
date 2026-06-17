use super::types::AgentCategory;
use super::visibility::SubagentVisibilityPolicy;
use crate::agentic::agents::{
    Agent, AgenticMode, ArchitectureReviewerAgent, BusinessLogicReviewerAgent, ClawMode,
    CodeReviewAgent, ComputerUseMode, CoworkMode, DebugMode, DeepResearchMode, DeepReviewAgent,
    ExploreAgent, FileFinderAgent, FrontendReviewerAgent, GeneralPurposeAgent, GenerateDocAgent,
    MultitaskMode, PerformanceReviewerAgent, PlanMode, ResearchSpecialistAgent, ReviewFixerAgent,
    ReviewJudgeAgent, SecurityReviewerAgent, TeamMode,
};
use bitfun_agent_runtime::agents as runtime_agents;
use std::sync::Arc;

#[derive(Clone)]
pub struct BuiltinAgentSpec {
    pub factory: fn() -> Arc<dyn Agent>,
    pub category: AgentCategory,
    pub visibility_policy: SubagentVisibilityPolicy,
}

pub fn builtin_agent_specs() -> Vec<BuiltinAgentSpec> {
    runtime_agents::builtin_agent_definition_specs()
        .into_iter()
        .map(|spec| BuiltinAgentSpec {
            factory: builtin_agent_factory(spec.id),
            category: map_builtin_agent_category(spec.category),
            visibility_policy: spec.visibility_policy,
        })
        .collect()
}

fn map_builtin_agent_category(category: runtime_agents::BuiltinAgentCategory) -> AgentCategory {
    match category {
        runtime_agents::BuiltinAgentCategory::Mode => AgentCategory::Mode,
        runtime_agents::BuiltinAgentCategory::SubAgent => AgentCategory::SubAgent,
        runtime_agents::BuiltinAgentCategory::Hidden => AgentCategory::Hidden,
    }
}

fn builtin_agent_factory(id: &str) -> fn() -> Arc<dyn Agent> {
    match id {
        "agentic" => || Arc::new(AgenticMode::new()),
        "Cowork" => || Arc::new(CoworkMode::new()),
        "debug" => || Arc::new(DebugMode::new()),
        "Multitask" => || Arc::new(MultitaskMode::new()),
        "Plan" => || Arc::new(PlanMode::new()),
        "Claw" => || Arc::new(ClawMode::new()),
        "DeepResearch" => || Arc::new(DeepResearchMode::new()),
        "Team" => || Arc::new(TeamMode::new()),
        "ComputerUse" => || Arc::new(ComputerUseMode::new()),
        "Explore" => || Arc::new(ExploreAgent::new()),
        "GeneralPurpose" => || Arc::new(GeneralPurposeAgent::new()),
        "ResearchSpecialist" => || Arc::new(ResearchSpecialistAgent::new()),
        "FileFinder" => || Arc::new(FileFinderAgent::new()),
        "ReviewBusinessLogic" => || Arc::new(BusinessLogicReviewerAgent::new()),
        "ReviewPerformance" => || Arc::new(PerformanceReviewerAgent::new()),
        "ReviewSecurity" => || Arc::new(SecurityReviewerAgent::new()),
        "ReviewArchitecture" => || Arc::new(ArchitectureReviewerAgent::new()),
        "ReviewFrontend" => || Arc::new(FrontendReviewerAgent::new()),
        "ReviewJudge" => || Arc::new(ReviewJudgeAgent::new()),
        "ReviewFixer" => || Arc::new(ReviewFixerAgent::new()),
        "CodeReview" => || Arc::new(CodeReviewAgent::new()),
        "DeepReview" => || Arc::new(DeepReviewAgent::new()),
        "GenerateDoc" => || Arc::new(GenerateDocAgent::new()),
        _ => panic!("missing legacy Agent factory for builtin agent {id}"),
    }
}
