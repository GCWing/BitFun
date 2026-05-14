use crate::agentic::agents::definitions::custom::{CustomSubagent, CustomSubagentKind};
use crate::agentic::deep_review_policy::{
    REVIEWER_ARCHITECTURE_AGENT_TYPE, REVIEWER_BUSINESS_LOGIC_AGENT_TYPE,
    REVIEWER_FRONTEND_AGENT_TYPE, REVIEWER_PERFORMANCE_AGENT_TYPE, REVIEWER_SECURITY_AGENT_TYPE,
    REVIEW_JUDGE_AGENT_TYPE,
};
use crate::agentic::agents::{Agent, AgentToolPolicyOverrides};
use crate::agentic::agents::registry::visibility::{
    SubagentVisibilityPolicy, SubagentVisibilitySummary,
};
use crate::service::config::types::AgentSubagentOverrideState;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentListScope {
    TaskVisible,
    RegistryManagement,
}

#[derive(Debug, Clone)]
pub struct SubagentQueryContext<'a> {
    pub parent_agent_type: Option<&'a str>,
    pub workspace_root: Option<&'a Path>,
    pub list_scope: SubagentListScope,
    pub include_disabled: bool,
}

/// subagent source (builtin / project / user), used for frontend display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubAgentSource {
    Builtin,
    Project,
    User,
}

impl SubAgentSource {
    pub fn from_custom_kind(kind: CustomSubagentKind) -> Self {
        match kind {
            CustomSubagentKind::Project => SubAgentSource::Project,
            CustomSubagentKind::User => SubAgentSource::User,
        }
    }
}

/// mutable configuration for custom subagent (model will change, path/kind can be obtained by downcast)
#[derive(Clone, Debug)]
pub struct CustomSubagentConfig {
    /// used model ID
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct AgentToolPolicy {
    pub allowed_tools: Vec<String>,
    pub exposure_overrides: AgentToolPolicyOverrides,
}

/// Agent category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentCategory {
    /// mode agent (displayed in frontend mode selector)
    Mode,
    /// subagent (displayed in frontend subagent list, discovered by TaskTool)
    SubAgent,
    /// hidden agent (not displayed in frontend, not discovered by TaskTool, used internally)
    Hidden,
}

/// one agent record in registry
#[derive(Clone)]
pub(crate) struct AgentEntry {
    pub(crate) category: AgentCategory,
    /// only when category == SubAgent has value
    pub(crate) subagent_source: Option<SubAgentSource>,
    pub(crate) agent: Arc<dyn Agent>,
    pub(crate) visibility_policy: SubagentVisibilityPolicy,
    /// custom subagent configuration (model), only user/project subagent has value
    pub(crate) custom_config: Option<CustomSubagentConfig>,
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
    #[serde(default)]
    pub default_enabled: bool,
    #[serde(default = "default_true")]
    pub effective_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_state: Option<AgentSubagentOverrideState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_reason: Option<SubagentStateReason>,
    /// subagent source, only subagent has value, used for frontend display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_source: Option<SubAgentSource>,
    pub path: Option<String>,
    /// model configuration, only custom subagent has value (read from file)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<SubagentVisibilitySummary>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubagentStateReason {
    BuiltinDefaultVisible,
    BuiltinDefaultHidden,
    CustomDefaultEnabled,
    EnabledByProjectOverride,
    DisabledByProjectOverride,
    EnabledByUserOverride,
    DisabledByUserOverride,
}

fn default_true() -> bool {
    true
}

pub fn subagent_key_for(
    source: Option<SubAgentSource>,
    agent: &dyn Agent,
) -> Option<String> {
    let source = source?;
    let slot = match source {
        SubAgentSource::Builtin => "builtin",
        SubAgentSource::Project => {
            let custom = agent.as_any().downcast_ref::<CustomSubagent>()?;
            match custom.kind {
                CustomSubagentKind::Project => "bitfun",
                CustomSubagentKind::User => "bitfun",
            }
        }
        SubAgentSource::User => {
            let custom = agent.as_any().downcast_ref::<CustomSubagent>()?;
            match custom.kind {
                CustomSubagentKind::Project => "bitfun",
                CustomSubagentKind::User => "bitfun",
            }
        }
    };
    let prefix = match source {
        SubAgentSource::Builtin => "builtin",
        SubAgentSource::Project => "project",
        SubAgentSource::User => "user",
    };
    Some(format!("{prefix}::{slot}::{}", agent.id()))
}

impl AgentInfo {
    pub(crate) fn from_agent_entry(entry: &AgentEntry) -> Self {
        let agent = entry.agent.as_ref();
        let default_tools = agent.default_tools();

        // get model from custom_config; path by downcast
        let model = entry.custom_config.as_ref().map(|config| config.model.clone());

        // get path by downcast to CustomSubagent (only custom subagent has path)
        let path = agent
            .as_any()
            .downcast_ref::<CustomSubagent>()
            .map(|c| c.path.clone());

        AgentInfo {
            key: subagent_key_for(entry.subagent_source, agent).unwrap_or_else(|| agent.id().to_string()),
            id: agent.id().to_string(),
            name: agent.name().to_string(),
            description: agent.description().to_string(),
            is_readonly: agent.is_readonly(),
            is_review: is_review_agent_entry(entry),
            tool_count: default_tools.len(),
            default_tools,
            default_enabled: true,
            effective_enabled: true,
            override_state: None,
            state_reason: None,
            subagent_source: entry.subagent_source,
            path,
            model,
            visibility: (entry.category == AgentCategory::SubAgent)
                .then(|| entry.visibility_policy.summary()),
        }
    }
}

pub(crate) fn is_review_agent_entry(entry: &AgentEntry) -> bool {
    let agent = entry.agent.as_ref();
    if let Some(custom) = agent.as_any().downcast_ref::<CustomSubagent>() {
        return custom.review;
    }

    matches!(
        agent.id(),
        REVIEWER_BUSINESS_LOGIC_AGENT_TYPE
            | REVIEWER_PERFORMANCE_AGENT_TYPE
            | REVIEWER_SECURITY_AGENT_TYPE
            | REVIEWER_ARCHITECTURE_AGENT_TYPE
            | REVIEWER_FRONTEND_AGENT_TYPE
            | REVIEW_JUDGE_AGENT_TYPE
    )
}
