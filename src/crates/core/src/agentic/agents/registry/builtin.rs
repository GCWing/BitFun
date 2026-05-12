use super::types::AgentEntry;
use super::visibility::SubagentVisibilityPolicy;
use super::AgentRegistry;
use crate::agentic::agents::{
    Agent, AgentCategory, CustomSubagentConfig, SubAgentSource,
};
use crate::agentic::agents::registry::catalog::builtin_agent_specs;
use log::error;
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) fn default_model_id_for_builtin_agent(agent_type: &str) -> &'static str {
    match agent_type {
        "agentic" | "Cowork" | "ComputerUse" | "Plan" | "debug" | "Claw" | "DeepResearch"
        | "Team" => "auto",
        "DeepReview"
        | "ReviewBusinessLogic"
        | "ReviewPerformance"
        | "ReviewSecurity"
        | "ReviewArchitecture"
        | "ReviewFrontend"
        | "ReviewJudge"
        | "ReviewFixer" => "fast",
        "Explore" | "FileFinder" | "CodeReview" | "GenerateDoc" | "Init" => "primary",
        _ => "fast",
    }
}

impl AgentRegistry {
    pub(crate) fn build_builtin_agents() -> HashMap<String, AgentEntry> {
        let mut agents = HashMap::new();

        let register = |agents: &mut HashMap<String, AgentEntry>,
                        agent: Arc<dyn Agent>,
                        category: AgentCategory,
                        subagent_source: Option<SubAgentSource>,
                        visibility_policy: SubagentVisibilityPolicy| {
            let id = agent.id().to_string();
            if agents.contains_key(&id) {
                error!("Agent {} already registered, skip registration", id);
                return;
            }
            agents.insert(
                id,
                AgentEntry {
                    category,
                    subagent_source,
                    agent,
                    visibility_policy,
                    custom_config: None,
                },
            );
        };

        for spec in builtin_agent_specs() {
            let source = if spec.category == AgentCategory::SubAgent {
                Some(SubAgentSource::Builtin)
            } else {
                None
            };
            register(
                &mut agents,
                (spec.factory)(),
                spec.category,
                source,
                spec.visibility_policy,
            );
        }

        agents
    }

    /// Create a new agent registry with built-in agents
    pub fn new() -> Self {
        Self {
            agents: std::sync::RwLock::new(Self::build_builtin_agents()),
            project_subagents: std::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Register a new agent. For custom SubAgent, pass Some(custom_config); for builtin/Mode/Hidden pass None.
    pub fn register_agent(
        &self,
        agent: Arc<dyn Agent>,
        category: AgentCategory,
        subagent_source: Option<SubAgentSource>,
        custom_config: Option<CustomSubagentConfig>,
    ) {
        let id = agent.id().to_string();
        let visibility_policy = SubagentVisibilityPolicy::public();
        let mut map = self.write_agents();
        if map.contains_key(&id) {
            error!("Agent {} already registered, skip registration", id);
            return;
        }
        map.insert(
            id,
            AgentEntry {
                category,
                subagent_source,
                agent,
                visibility_policy,
                custom_config,
            },
        );
    }
}
