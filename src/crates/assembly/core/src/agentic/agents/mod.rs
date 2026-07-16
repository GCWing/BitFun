//! Mode system for BitFun
//!
//! Provides flexible mode selection with different system prompts and tool sets

mod definitions;
mod prompt_builder;
mod registry;

use crate::agentic::session::{SystemPromptCacheIdentity, UserContextCacheIdentity};
use crate::agentic::tools::framework::ToolExposure;
use crate::agentic::WorkspaceBinding;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
pub use bitfun_agent_runtime::agents::{
    mode_config_profile_label, mode_config_profile_member_mode_ids, mode_presentation_rank,
    resolve_mode_config_profile_id, shared_coding_mode_user_context_policy,
    SHARED_CODING_MODE_CONFIG_PROFILE_ID, SHARED_CODING_MODE_CONFIG_PROFILE_LABEL,
    SHARED_CODING_MODE_IDS, SHARED_CODING_MODE_PROMPT_TEMPLATE,
};
pub use bitfun_agent_runtime::custom_agent::{
    custom_agent_model_or_default, custom_agent_review_writable_tools, default_custom_agent_tools,
    default_custom_agent_user_context_policy, CustomAgentKind, CustomAgentLevel,
};
pub use definitions::custom::{CustomMode, CustomSubagent, CustomSubagentKind};
pub use definitions::hidden::{CodeReviewAgent, DeepReviewAgent, GenerateDocAgent};
pub use definitions::modes::{
    AgenticMode, ClawMode, CoworkMode, DebugMode, DeepResearchMode, MultitaskMode, PlanMode,
    TeamMode,
};
pub use definitions::review::{
    ArchitectureReviewerAgent, BusinessLogicReviewerAgent, FrontendReviewerAgent,
    GeneralReviewerAgent, PerformanceReviewerAgent, ReviewFixerAgent, ReviewJudgeAgent,
    SecurityReviewerAgent,
};
pub use definitions::shared::ReadonlySubagent;
pub use definitions::subagents::{
    ComputerUseMode, ExploreAgent, FileFinderAgent, GeneralPurposeAgent, ResearchSpecialistAgent,
};
use indexmap::IndexMap;
pub use prompt_builder::{
    build_prompt_context_for_workspace, PrependedPromptReminders, PromptBuilder,
    PromptBuilderContext, RemoteExecutionHints, RuntimeContextNeeds, ToolListingSections,
    UserContextPolicy, UserContextSection,
};
pub use registry::catalog::{builtin_agent_specs, BuiltinAgentSpec};
pub use registry::types::{
    subagent_source_from_custom_kind, AgentCategory, AgentInfo, AgentSource, AgentToolPolicy,
    CustomSubagentConfig, SubAgentSource, SubagentListScope, SubagentQueryContext,
    SubagentStateReason,
};
pub use registry::visibility::{
    BuiltinSubagentExposure, SubagentVisibilityPolicy, SubagentVisibilitySummary,
};
pub use registry::{get_agent_registry, AgentRegistry, CustomAgentDetail, CustomSubagentDetail};
use std::any::Any;

// Include embedded prompts generated at compile time
include!(concat!(env!("OUT_DIR"), "/embedded_agents_prompt.rs"));

pub type AgentToolPolicyOverrides = IndexMap<String, ToolExposure>;

static EMPTY_AGENT_TOOL_POLICY_OVERRIDES: std::sync::LazyLock<AgentToolPolicyOverrides> =
    std::sync::LazyLock::new(AgentToolPolicyOverrides::default);

pub fn shared_coding_mode_tool_exposure_overrides() -> AgentToolPolicyOverrides {
    // Web research is a baseline capability of the shared coding modes; keep
    // WebSearch/WebFetch expanded so models do not need a GetToolSpec
    // unlock round-trip when switching between those modes.
    let mut overrides = AgentToolPolicyOverrides::default();
    overrides.insert("WebSearch".to_string(), ToolExposure::Direct);
    overrides.insert("WebFetch".to_string(), ToolExposure::Direct);
    overrides
}

fn append_provider_group_tools(tools: &mut Vec<String>, provider_id: &'static str) {
    #[cfg(feature = "tool-packs")]
    {
        let provider_groups =
            bitfun_tool_packs::try_product_tool_provider_group_plan_for_ids(&[provider_id])
                .expect("shared coding mode provider group must exist");
        for group in provider_groups {
            tools.extend(
                group
                    .tool_names()
                    .iter()
                    .map(|tool_name| tool_name.to_string()),
            );
        }
    }

    #[cfg(all(feature = "canvas-runtime", not(feature = "tool-packs")))]
    if provider_id == "core.canvas" {
        tools.extend(
            ["CreateCanvas", "ReadCanvas", "UpdateCanvas", "PatchCanvas"]
                .into_iter()
                .map(str::to_string),
        );
    }
}

pub fn shared_coding_mode_tools() -> Vec<String> {
    let mut tools = vec![
        "Task".to_string(),
        "Read".to_string(),
        "view_image".to_string(),
        "analyze_image".to_string(),
        "Write".to_string(),
        "Edit".to_string(),
        "Delete".to_string(),
        "ExecCommand".to_string(),
        "WriteStdin".to_string(),
        "ExecControl".to_string(),
        "Grep".to_string(),
        "Glob".to_string(),
        "WebSearch".to_string(),
        "WebFetch".to_string(),
        "TodoWrite".to_string(),
        "get_goal".to_string(),
        "create_goal".to_string(),
        "update_goal".to_string(),
        "GenerativeUI".to_string(),
        "Skill".to_string(),
        "AskUserQuestion".to_string(),
        "CreatePlan".to_string(),
        "Git".to_string(),
        "ReviewPlatform".to_string(),
        "ControlHub".to_string(),
        "InitMiniApp".to_string(),
    ];
    append_provider_group_tools(&mut tools, "core.canvas");
    tools
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

    /// Get the system reminder for this agent, only used for modes.
    /// The returned reminder may be prepended immediately before the user's
    /// actual message in runtime context.
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
    use super::{
        shared_coding_mode_tool_exposure_overrides, shared_coding_mode_tools,
        shared_coding_mode_user_context_policy, Agent, AgenticMode, DebugMode, MultitaskMode,
        PlanMode,
    };

    #[test]
    fn shared_template_modes_share_system_prompt_cache_identity() {
        let agentic = AgenticMode::new();
        let multitask = MultitaskMode::new();
        let plan = PlanMode::new();
        let debug = DebugMode::new();

        assert_eq!(
            agentic.system_prompt_cache_identity(None),
            multitask.system_prompt_cache_identity(None)
        );
        assert_eq!(
            agentic.system_prompt_cache_identity(None),
            plan.system_prompt_cache_identity(None)
        );
        assert_eq!(
            agentic.system_prompt_cache_identity(None),
            debug.system_prompt_cache_identity(None)
        );
        assert_eq!(
            agentic.user_context_cache_identity(),
            multitask.user_context_cache_identity()
        );
        assert_eq!(
            agentic.user_context_cache_identity(),
            plan.user_context_cache_identity()
        );
        assert_eq!(
            agentic.user_context_cache_identity(),
            debug.user_context_cache_identity()
        );
    }

    #[test]
    fn shared_coding_mode_tools_include_plan_and_debug_specific_tools() {
        let tools = shared_coding_mode_tools();

        assert!(tools.contains(&"CreatePlan".to_string()));
        assert!(tools.contains(&"get_goal".to_string()));
        assert!(tools.contains(&"update_goal".to_string()));
    }

    #[test]
    fn shared_coding_mode_tools_include_review_platform() {
        let tools = shared_coding_mode_tools();

        assert!(tools.contains(&"ReviewPlatform".to_string()));
    }

    #[test]
    fn shared_coding_mode_tools_include_canvas_provider_tools() {
        let tools = shared_coding_mode_tools();

        assert!(tools.contains(&"CreateCanvas".to_string()));
        assert!(tools.contains(&"ReadCanvas".to_string()));
        assert!(tools.contains(&"UpdateCanvas".to_string()));
        assert!(tools.contains(&"PatchCanvas".to_string()));
    }

    #[test]
    fn shared_coding_modes_share_default_tools() {
        let shared_tools = shared_coding_mode_tools();

        assert_eq!(AgenticMode::new().default_tools(), shared_tools);
        assert_eq!(MultitaskMode::new().default_tools(), shared_tools);
        assert_eq!(PlanMode::new().default_tools(), shared_tools);
        assert_eq!(DebugMode::new().default_tools(), shared_tools);
    }

    #[test]
    fn shared_coding_mode_user_context_policy_matches_all_shared_modes() {
        let shared_policy = shared_coding_mode_user_context_policy();

        assert_eq!(AgenticMode::new().user_context_policy(), shared_policy);
        assert_eq!(MultitaskMode::new().user_context_policy(), shared_policy);
        assert_eq!(PlanMode::new().user_context_policy(), shared_policy);
        assert_eq!(DebugMode::new().user_context_policy(), shared_policy);
    }

    #[test]
    fn shared_coding_mode_tool_exposure_overrides_match_all_shared_modes() {
        let shared_overrides = shared_coding_mode_tool_exposure_overrides();
        let agentic = AgenticMode::new();
        let multitask = MultitaskMode::new();
        let plan = PlanMode::new();
        let debug = DebugMode::new();

        assert_eq!(agentic.tool_exposure_overrides(), &shared_overrides);
        assert_eq!(multitask.tool_exposure_overrides(), &shared_overrides);
        assert_eq!(plan.tool_exposure_overrides(), &shared_overrides);
        assert_eq!(debug.tool_exposure_overrides(), &shared_overrides);
    }
}
