use crate::agentic::agents::definitions::custom::{CustomMode, CustomSubagent, CustomSubagentKind};
use crate::agentic::agents::registry::visibility::{
    SubagentVisibilityPolicy, SubagentVisibilitySummary,
};
use crate::agentic::agents::{
    mode_config_profile_label, mode_config_profile_member_mode_ids, resolve_mode_config_profile_id,
    Agent, AgentToolPolicyOverrides,
};
use crate::agentic::deep_review_policy::{
    REVIEWER_ARCHITECTURE_AGENT_TYPE, REVIEWER_BUSINESS_LOGIC_AGENT_TYPE,
    REVIEWER_FRONTEND_AGENT_TYPE, REVIEWER_GENERAL_AGENT_TYPE, REVIEWER_PERFORMANCE_AGENT_TYPE,
    REVIEWER_SECURITY_AGENT_TYPE, REVIEW_JUDGE_AGENT_TYPE,
};
pub(super) use bitfun_agent_runtime::agents::SubagentOverrideState;
pub use bitfun_agent_runtime::agents::{
    BuiltinAgentCategory as AgentCategory, SubAgentSource, SubagentListScope, SubagentQueryContext,
    SubagentStateReason,
};
use bitfun_agent_runtime::custom_agent::CustomAgentLevel;
use bitfun_agent_runtime::prompt_cache::prompt_cache_scope_key;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Mutable configuration for file-backed custom agents.
#[derive(Clone, Debug)]
pub struct CustomAgentConfig {
    /// used model ID
    pub model: String,
    /// Whether the custom agent Markdown explicitly overrides the model.
    pub model_is_explicit: bool,
}

pub type CustomSubagentConfig = CustomAgentConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentSource {
    Builtin,
    Project,
    User,
    External,
}

#[derive(Debug, Clone)]
pub struct AgentToolPolicy {
    pub allowed_tools: Vec<String>,
    pub exposure_overrides: AgentToolPolicyOverrides,
}

/// one agent record in registry
#[derive(Clone)]
pub(crate) struct AgentEntry {
    pub(crate) category: AgentCategory,
    pub(crate) source: AgentSource,
    /// only when category == SubAgent has value
    pub(crate) subagent_source: Option<SubAgentSource>,
    pub(crate) agent: Arc<dyn Agent>,
    pub(crate) visibility_policy: SubagentVisibilityPolicy,
    /// file-backed custom agent configuration (model)
    pub(crate) custom_config: Option<CustomAgentConfig>,
}

/// Information about a agent for frontend display
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    pub key: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_readonly: bool,
    pub is_review: bool,
    pub tool_count: usize,
    pub default_tools: Vec<String>,
    /// Combined prompt-cache compatibility key for frontend mode-switch guards.
    ///
    /// Modes that share this key can reuse the same session-level prompt cache
    /// for the next accepted submission.
    pub prompt_cache_scope_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_profile_label: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub config_profile_member_mode_ids: Vec<String>,
    #[serde(default)]
    pub default_enabled: bool,
    #[serde(default = "default_true")]
    pub effective_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_state: Option<SubagentOverrideState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_reason: Option<SubagentStateReason>,
    pub source: AgentSource,
    /// subagent source, only subagent has value, used for frontend display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_source: Option<SubAgentSource>,
    pub path: Option<String>,
    /// model configuration, only custom subagent has value (read from file)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Whether `model` is an explicit custom Subagent override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_is_explicit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<SubagentVisibilitySummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_provider_label: Option<String>,
    #[serde(default = "default_true")]
    pub supports_follow_up: bool,
}

fn default_true() -> bool {
    true
}

pub fn subagent_source_from_custom_kind(kind: CustomSubagentKind) -> SubAgentSource {
    match kind {
        CustomSubagentKind::Project => SubAgentSource::Project,
        CustomSubagentKind::User => SubAgentSource::User,
    }
}

pub(super) fn agent_source_from_custom_level(level: CustomAgentLevel) -> AgentSource {
    match level {
        CustomAgentLevel::Project => AgentSource::Project,
        CustomAgentLevel::User => AgentSource::User,
    }
}

pub(super) fn subagent_key_for(
    source: Option<SubAgentSource>,
    agent: &dyn Agent,
) -> Option<String> {
    let source = source?;
    let slot = match source {
        SubAgentSource::Builtin => "builtin",
        SubAgentSource::Project => {
            let _custom = agent.as_any().downcast_ref::<CustomSubagent>()?;
            "bitfun"
        }
        SubAgentSource::User => {
            let _custom = agent.as_any().downcast_ref::<CustomSubagent>()?;
            "bitfun"
        }
        SubAgentSource::External => "external",
    };
    let prefix = match source {
        SubAgentSource::Builtin => "builtin",
        SubAgentSource::Project => "project",
        SubAgentSource::User => "user",
        SubAgentSource::External => "external",
    };
    Some(format!("{prefix}::{slot}::{}", agent.id()))
}

impl AgentInfo {
    pub(crate) fn from_agent_entry(entry: &AgentEntry) -> Self {
        let agent = entry.agent.as_ref();
        let default_tools = agent.default_tools();
        let config_profile_id = (entry.category == AgentCategory::Mode)
            .then(|| resolve_mode_config_profile_id(agent.id()).into_owned());
        let config_profile_label = config_profile_id
            .as_deref()
            .and_then(mode_config_profile_label)
            .map(str::to_string);
        let config_profile_member_mode_ids = if let Some(profile_id) = config_profile_id.as_deref()
        {
            let members = mode_config_profile_member_mode_ids(profile_id);
            if members.is_empty() {
                vec![agent.id().to_string()]
            } else {
                members
                    .iter()
                    .map(|mode_id| (*mode_id).to_string())
                    .collect()
            }
        } else {
            Vec::new()
        };

        // get model from custom_config; path by downcast
        let model = entry
            .custom_config
            .as_ref()
            .map(|config| config.model.clone());
        let model_is_explicit = entry
            .custom_config
            .as_ref()
            .map(|config| config.model_is_explicit);

        // get path by downcast to CustomSubagent (only custom subagent has path)
        let path = custom_agent_path(agent);

        AgentInfo {
            key: subagent_key_for(entry.subagent_source, agent)
                .unwrap_or_else(|| agent.id().to_string()),
            id: agent.id().to_string(),
            name: agent.name().to_string(),
            description: agent.description().to_string(),
            is_readonly: agent.is_readonly(),
            is_review: is_review_agent_entry(entry),
            tool_count: default_tools.len(),
            default_tools,
            prompt_cache_scope_key: prompt_cache_scope_key(
                &agent.system_prompt_cache_identity(None),
                &agent.user_context_cache_identity(),
            ),
            config_profile_id,
            config_profile_label,
            config_profile_member_mode_ids,
            default_enabled: true,
            effective_enabled: true,
            override_state: None,
            state_reason: None,
            source: entry.source,
            subagent_source: entry.subagent_source,
            path,
            model,
            model_is_explicit,
            visibility: (entry.category == AgentCategory::SubAgent)
                .then(|| entry.visibility_policy.summary()),
            external_provider_label: None,
            supports_follow_up: true,
        }
    }
}

pub(crate) fn is_review_agent_entry(entry: &AgentEntry) -> bool {
    let agent = entry.agent.as_ref();
    if let Some(custom) = agent.as_any().downcast_ref::<CustomSubagent>() {
        return custom.data.review;
    }

    matches!(
        agent.id(),
        REVIEWER_BUSINESS_LOGIC_AGENT_TYPE
            | REVIEWER_PERFORMANCE_AGENT_TYPE
            | REVIEWER_SECURITY_AGENT_TYPE
            | REVIEWER_ARCHITECTURE_AGENT_TYPE
            | REVIEWER_FRONTEND_AGENT_TYPE
            | REVIEWER_GENERAL_AGENT_TYPE
            | REVIEW_JUDGE_AGENT_TYPE
            | "CodeReview"
    )
}

pub(crate) fn custom_agent_path(agent: &dyn Agent) -> Option<String> {
    if let Some(custom) = agent.as_any().downcast_ref::<CustomSubagent>() {
        return Some(custom.data.path.clone());
    }
    agent
        .as_any()
        .downcast_ref::<CustomMode>()
        .map(|custom| custom.data.path.clone())
}
