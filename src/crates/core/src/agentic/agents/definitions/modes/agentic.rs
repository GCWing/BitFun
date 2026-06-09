//! Agentic Mode

use crate::agentic::agents::{
    shared_coding_mode_tools, shared_coding_mode_user_context_policy, Agent, UserContextPolicy,
    SHARED_CODING_MODE_PROMPT_TEMPLATE,
};
use async_trait::async_trait;
pub struct AgenticMode {
    default_tools: Vec<String>,
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

    fn user_context_policy(&self) -> UserContextPolicy {
        shared_coding_mode_user_context_policy()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}
