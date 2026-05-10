//! Mode system for BitFun
//!
//! Provides flexible mode selection with different system prompts and tool sets

mod custom_subagents;
mod prompt_builder;
mod readonly_subagent;
mod registry;
// Modes
mod agentic_mode;
mod claw_mode;
mod computer_use_mode;
mod cowork_mode;
mod debug_mode;
mod plan_mode;
mod team_mode;
// Built-in subagents
mod deep_research_agent;
mod explore_agent;
mod file_finder_agent;
mod research_specialist_agent;
mod review_fixer_agent;
mod review_specialist_agents;
// Hidden agents
mod code_review_agent;
mod deep_review_agent;
mod generate_doc_agent;
mod init_agent;

use crate::util::errors::{BitFunError, BitFunResult};
pub use agentic_mode::AgenticMode;
use async_trait::async_trait;
pub use claw_mode::ClawMode;
pub use code_review_agent::CodeReviewAgent;
pub use computer_use_mode::ComputerUseMode;
pub use cowork_mode::CoworkMode;
pub use custom_subagents::{CustomSubagent, CustomSubagentKind};
pub use debug_mode::DebugMode;
pub use deep_research_agent::DeepResearchAgent;
pub use deep_review_agent::DeepReviewAgent;
pub use explore_agent::ExploreAgent;
pub use file_finder_agent::FileFinderAgent;
pub use generate_doc_agent::GenerateDocAgent;
pub use init_agent::InitAgent;
pub use plan_mode::PlanMode;
pub use prompt_builder::{
    PromptBuilder, PromptBuilderContext, RemoteExecutionHints, RequestContextPolicy,
    RequestContextSection,
};
pub use readonly_subagent::ReadonlySubagent;
pub use registry::{
    get_agent_registry, AgentCategory, AgentInfo, AgentRegistry, CustomSubagentConfig,
    CustomSubagentDetail, SubAgentSource,
};
pub use research_specialist_agent::ResearchSpecialistAgent;
pub use review_fixer_agent::ReviewFixerAgent;
pub use review_specialist_agents::{
    ArchitectureReviewerAgent, BusinessLogicReviewerAgent, FrontendReviewerAgent,
    PerformanceReviewerAgent, ReviewJudgeAgent, SecurityReviewerAgent,
};
use std::any::Any;
pub use team_mode::TeamMode;

// Include embedded prompts generated at compile time
include!(concat!(env!("OUT_DIR"), "/embedded_agents_prompt.rs"));

/// Agent trait defining the interface for all agents
#[async_trait]
pub trait Agent: Send + Sync + 'static {
    /// downcast to specific type
    fn as_any(&self) -> &dyn Any;

    /// Unique identifier for the agent
    fn id(&self) -> &str;

    /// Human-readable name
    fn name(&self) -> &str;

    /// Description of what the agent does
    fn description(&self) -> &str;

    /// Prompt template name for the agent.
    fn prompt_template_name(&self, model_name: Option<&str>) -> &str;

    fn system_reminder_template_name(&self) -> Option<&str> {
        None // by default, no system reminder
    }

    fn request_context_policy(&self) -> RequestContextPolicy {
        RequestContextPolicy::default()
    }

    /// Build the system prompt for this agent
    async fn build_prompt(&self, context: &PromptBuilderContext) -> BitFunResult<String> {
        let prompt_components = PromptBuilder::new(context.clone());
        let template_name = self.prompt_template_name(context.model_name.as_deref());
        let system_prompt_template = get_embedded_prompt(template_name).ok_or_else(|| {
            BitFunError::Agent(format!("{} not found in embedded files", template_name))
        })?;

        let prompt = prompt_components
            .build_prompt_from_template(system_prompt_template)
            .await?;

        Ok(prompt)
    }

    /// Get the system prompt for this agent
    async fn get_system_prompt(
        &self,
        context: Option<&PromptBuilderContext>,
    ) -> BitFunResult<String> {
        if let Some(context) = context {
            self.build_prompt(context).await
        } else {
            Err(BitFunError::Agent(
                "Prompt build context is required".to_string(),
            ))
        }
    }

    /// Get the system reminder for this agent, only used for modes
    /// system_reminder will be appended to the user_query
    /// This is not necessary for all modes
    /// index is not used for now (Cursor first time enter plan mode and keep plan mode will use different reminder)
    async fn get_system_reminder(&self, _index: usize) -> BitFunResult<String> {
        if let Some(system_reminder_template_name) = self.system_reminder_template_name() {
            let system_reminder =
                get_embedded_prompt(system_reminder_template_name).ok_or_else(|| {
                    BitFunError::Agent(format!(
                        "{} not found in embedded files",
                        system_reminder_template_name
                    ))
                })?;
            Ok(system_reminder.to_string())
        } else {
            Ok("".to_string())
        }
    }

    /// Get the list of default tools for this agent
    fn default_tools(&self) -> Vec<String>;

    /// Whether this agent is read-only (prevents file modifications)
    fn is_readonly(&self) -> bool {
        false
    }
}
