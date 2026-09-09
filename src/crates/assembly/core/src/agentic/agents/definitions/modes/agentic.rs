//! Agentic Mode
//!
//! Uses the shared coding prompt, tools, exposure policy, and user context.

use crate::agentic::agents::{
    shared_coding_mode_tool_exposure_overrides, shared_coding_mode_tools,
    shared_coding_mode_user_context_policy, Agent, AgentToolPolicyOverrides, UserContextPolicy,
    SHARED_CODING_MODE_PROMPT_TEMPLATE,
};
use async_trait::async_trait;

pub struct AgenticMode {
    default_tools: Vec<String>,
    tool_exposure_overrides: AgentToolPolicyOverrides,
}

impl Default for AgenticMode {
    fn default() -> Self {
        Self::new()
    }
}

impl AgenticMode {
    pub fn new() -> Self {
        Self {
            default_tools: shared_coding_mode_tools(),
            tool_exposure_overrides: shared_coding_mode_tool_exposure_overrides(),
        }
    }
}

#[async_trait]
impl Agent for AgenticMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "agentic"
    }

    fn name(&self) -> &str {
        "Agentic"
    }

    fn description(&self) -> &str {
        "Full-featured AI assistant with access to all tools for comprehensive software development tasks"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        SHARED_CODING_MODE_PROMPT_TEMPLATE
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn tool_exposure_overrides(&self) -> &AgentToolPolicyOverrides {
        &self.tool_exposure_overrides
    }

    fn user_context_policy(&self) -> UserContextPolicy {
        shared_coding_mode_user_context_policy()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}
