use super::types::AgentCategory;
use super::visibility::SubagentVisibilityPolicy;
use crate::agentic::agents::{
    Agent, AgenticMode, ArchitectureReviewerAgent, BusinessLogicReviewerAgent, ClawMode,
    CodeReviewAgent, ComputerUseMode, CoworkMode, DebugMode, DeepResearchMode,
    DeepReviewAgent, ExploreAgent, FileFinderAgent, FrontendReviewerAgent, GenerateDocAgent,
    InitAgent, PerformanceReviewerAgent, PlanMode, ResearchSpecialistAgent, ReviewFixerAgent,
    ReviewJudgeAgent, SecurityReviewerAgent, TeamMode,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct BuiltinAgentSpec {
    pub factory: fn() -> Arc<dyn Agent>,
    pub category: AgentCategory,
    pub visibility_policy: SubagentVisibilityPolicy,
}

pub fn builtin_agent_specs() -> Vec<BuiltinAgentSpec> {
    vec![
        BuiltinAgentSpec {
            factory: || Arc::new(AgenticMode::new()),
            category: AgentCategory::Mode,
            visibility_policy: SubagentVisibilityPolicy::default(),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(CoworkMode::new()),
            category: AgentCategory::Mode,
            visibility_policy: SubagentVisibilityPolicy::default(),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(DebugMode::new()),
            category: AgentCategory::Mode,
            visibility_policy: SubagentVisibilityPolicy::default(),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(PlanMode::new()),
            category: AgentCategory::Mode,
            visibility_policy: SubagentVisibilityPolicy::default(),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(ClawMode::new()),
            category: AgentCategory::Mode,
            visibility_policy: SubagentVisibilityPolicy::default(),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(DeepResearchMode::new()),
            category: AgentCategory::Mode,
            visibility_policy: SubagentVisibilityPolicy::default(),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(TeamMode::new()),
            category: AgentCategory::Mode,
            visibility_policy: SubagentVisibilityPolicy::default(),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(ComputerUseMode::new()),
            category: AgentCategory::SubAgent,
            visibility_policy: SubagentVisibilityPolicy::restricted(["Claw", "Team"]),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(ExploreAgent::new()),
            category: AgentCategory::SubAgent,
            visibility_policy: SubagentVisibilityPolicy::public(),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(ResearchSpecialistAgent::new()),
            category: AgentCategory::SubAgent,
            visibility_policy: SubagentVisibilityPolicy::restricted(["DeepResearch"]),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(FileFinderAgent::new()),
            category: AgentCategory::SubAgent,
            visibility_policy: SubagentVisibilityPolicy::public(),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(BusinessLogicReviewerAgent::new()),
            category: AgentCategory::SubAgent,
            visibility_policy: SubagentVisibilityPolicy::restricted(["DeepReview"]),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(PerformanceReviewerAgent::new()),
            category: AgentCategory::SubAgent,
            visibility_policy: SubagentVisibilityPolicy::restricted(["DeepReview"]),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(SecurityReviewerAgent::new()),
            category: AgentCategory::SubAgent,
            visibility_policy: SubagentVisibilityPolicy::restricted(["DeepReview"]),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(ArchitectureReviewerAgent::new()),
            category: AgentCategory::SubAgent,
            visibility_policy: SubagentVisibilityPolicy::restricted(["DeepReview"]),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(FrontendReviewerAgent::new()),
            category: AgentCategory::SubAgent,
            visibility_policy: SubagentVisibilityPolicy::restricted(["DeepReview"]),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(ReviewJudgeAgent::new()),
            category: AgentCategory::SubAgent,
            visibility_policy: SubagentVisibilityPolicy::restricted(["DeepReview"]),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(ReviewFixerAgent::new()),
            category: AgentCategory::SubAgent,
            visibility_policy: SubagentVisibilityPolicy::restricted(["DeepReview"]),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(CodeReviewAgent::new()),
            category: AgentCategory::Hidden,
            visibility_policy: SubagentVisibilityPolicy::default(),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(DeepReviewAgent::new()),
            category: AgentCategory::Hidden,
            visibility_policy: SubagentVisibilityPolicy::default(),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(GenerateDocAgent::new()),
            category: AgentCategory::Hidden,
            visibility_policy: SubagentVisibilityPolicy::default(),
        },
        BuiltinAgentSpec {
            factory: || Arc::new(InitAgent::new()),
            category: AgentCategory::Hidden,
            visibility_policy: SubagentVisibilityPolicy::default(),
        },
    ]
}
