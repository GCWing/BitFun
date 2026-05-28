//! Mode system for BitFun
//!
//! Provides flexible mode selection with different system prompts and tool sets

mod definitions;
mod prompt_builder;
mod registry;
// Utility hooks used by specific agents (not themselves an agent definition):
// citation_renumber finalizes a DeepResearch report's cit_XXX references into
// consecutive `[N]` display IDs after the dialog turn completes.
pub(crate) mod citation_renumber;

use crate::agentic::session::{SystemPromptCacheIdentity, UserContextCacheIdentity};
use crate::agentic::tools::framework::ToolExposure;
use crate::agentic::WorkspaceBinding;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
pub use definitions::custom::{CustomSubagent, CustomSubagentKind};
pub use definitions::hidden::{CodeReviewAgent, DeepReviewAgent, GenerateDocAgent};
pub use definitions::modes::{
    AgenticMode, ClawMode, CoworkMode, DebugMode, DeepResearchMode, MultitaskMode, PlanMode,
    TeamMode,
};
pub use definitions::review::{
    ArchitectureReviewerAgent, BusinessLogicReviewerAgent, FrontendReviewerAgent,
    PerformanceReviewerAgent, ReviewFixerAgent, ReviewJudgeAgent, SecurityReviewerAgent,
};
pub use definitions::shared::ReadonlySubagent;
pub use definitions::subagents::{
    ComputerUseMode, ExploreAgent, FileFinderAgent, GeneralPurposeAgent, ResearchSpecialistAgent,
};
use indexmap::IndexMap;
pub use prompt_builder::{
    PrependedPromptReminders, PromptBuilder, PromptBuilderContext, RemoteExecutionHints,
    ToolListingSections, UserContextPolicy, UserContextSection,
};
pub use registry::catalog::{builtin_agent_specs, BuiltinAgentSpec};
pub use registry::types::{
    AgentCategory, AgentInfo, AgentToolPolicy, CustomSubagentConfig, SubAgentSource,
    SubagentListScope, SubagentQueryContext, SubagentStateReason,
};
pub use registry::visibility::{
    BuiltinSubagentExposure, SubagentVisibilityPolicy, SubagentVisibilitySummary,
};
pub use registry::{get_agent_registry, AgentRegistry, CustomSubagentDetail};
use std::any::Any;

// Include embedded prompts generated at compile time
include!(concat!(env!("OUT_DIR"), "/embedded_agents_prompt.rs"));

pub type AgentToolPolicyOverrides = IndexMap<String, ToolExposure>;

static EMPTY_AGENT_TOOL_POLICY_OVERRIDES: std::sync::LazyLock<AgentToolPolicyOverrides> =
    std::sync::LazyLock::new(AgentToolPolicyOverrides::default);

pub const SHARED_CODING_MODE_PROMPT_TEMPLATE: &str = "agentic_mode";

pub fn shared_coding_mode_tools() -> Vec<String> {
    vec![
        "Task".to_string(),
        "Read".to_string(),
        "Write".to_string(),
        "Edit".to_string(),
        "Delete".to_string(),
        "Bash".to_string(),
        "Grep".to_string(),
        "Glob".to_string(),
        "WebSearch".to_string(),
        "WebFetch".to_string(),
        "TodoWrite".to_string(),
        "GenerativeUI".to_string(),
        "Skill".to_string(),
        "AskUserQuestion".to_string(),
        "Git".to_string(),
        "TerminalControl".to_string(),
        "ControlHub".to_string(),
        "InitMiniApp".to_string(),
    ]
}

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

    fn system_prompt_cache_identity(&self, model_name: Option<&str>) -> SystemPromptCacheIdentity {
        let template_name = self.prompt_template_name(model_name).trim();
        let scope_key = if template_name.is_empty() {
            format!("agent:{}", self.id())
        } else {
            format!("template:{}", template_name)
        };

        SystemPromptCacheIdentity::new(scope_key)
    }

    fn user_context_cache_identity(&self) -> UserContextCacheIdentity {
        UserContextCacheIdentity::new(self.user_context_policy().cache_scope_key())
    }

    fn system_reminder_template_name(&self) -> Option<&str> {
        None // by default, no system reminder
    }

    fn user_context_policy(&self) -> UserContextPolicy;

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
    /// `previous_agent_type` can be used to distinguish first entry vs staying
    /// in the same mode across turns.
    async fn get_system_reminder(
        &self,
        _previous_agent_type: Option<&str>,
        _workspace: Option<&WorkspaceBinding>,
    ) -> BitFunResult<String> {
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

    /// Per-agent exposure overrides for allowed tools.
    ///
    /// Tools omitted here inherit their tool-defined default exposure.
    fn tool_exposure_overrides(&self) -> &AgentToolPolicyOverrides {
        &EMPTY_AGENT_TOOL_POLICY_OVERRIDES
    }

    /// Whether this agent is read-only (prevents file modifications)
    fn is_readonly(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{Agent, AgenticMode, MultitaskMode};

    #[test]
    fn shared_template_modes_share_system_prompt_cache_identity() {
        let agentic = AgenticMode::new();
        let multitask = MultitaskMode::new();

        assert_eq!(
            agentic.system_prompt_cache_identity(None),
            multitask.system_prompt_cache_identity(None)
        );
        assert_eq!(
            agentic.user_context_cache_identity(),
            multitask.user_context_cache_identity()
        );
    }
}
