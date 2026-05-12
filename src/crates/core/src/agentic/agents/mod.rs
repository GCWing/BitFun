//! Mode system for BitFun
//!
//! Provides flexible mode selection with different system prompts and tool sets

mod definitions;
mod prompt_builder;
mod registry;

use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
pub use definitions::custom::{CustomSubagent, CustomSubagentKind};
pub use definitions::hidden::{
    CodeReviewAgent, DeepReviewAgent, GenerateDocAgent, InitAgent,
};
pub use definitions::modes::{
    AgenticMode, ClawMode, CoworkMode, DebugMode, DeepResearchMode, PlanMode, TeamMode,
};
pub use definitions::review::{
    ArchitectureReviewerAgent, BusinessLogicReviewerAgent, FrontendReviewerAgent,
    PerformanceReviewerAgent, ReviewFixerAgent, ReviewJudgeAgent, SecurityReviewerAgent,
};
pub use definitions::subagents::{
    ComputerUseMode, ExploreAgent, FileFinderAgent, ResearchSpecialistAgent,
};
pub use definitions::shared::ReadonlySubagent;
pub use prompt_builder::{
    PromptBuilder, PromptBuilderContext, RemoteExecutionHints, RequestContextPolicy,
    RequestContextSection,
};
pub use registry::{get_agent_registry, AgentRegistry, CustomSubagentDetail};
pub use registry::catalog::{
    builtin_agent_specs, BuiltinAgentSpec,
};
pub use registry::types::{
    AgentCategory, AgentInfo, CustomSubagentConfig, SubAgentSource, SubagentListScope,
    SubagentQueryContext,
};
pub use registry::visibility::{
    BuiltinSubagentExposure, SubagentVisibilityPolicy, SubagentVisibilitySummary,
};
use std::any::Any;

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
