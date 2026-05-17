//! Agentic Mode

use crate::agentic::agents::{
    shared_coding_mode_tools, Agent, SHARED_CODING_MODE_PROMPT_TEMPLATE,
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

    fn is_readonly(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{shared_coding_mode_tools, Agent, AgenticMode, SHARED_CODING_MODE_PROMPT_TEMPLATE};

    #[test]
    fn always_uses_default_prompt_template() {
        let agent = AgenticMode::new();
        assert_eq!(
            agent.prompt_template_name(Some("gpt-5.1")),
            SHARED_CODING_MODE_PROMPT_TEMPLATE
        );
        assert_eq!(
            agent.prompt_template_name(Some("GPT-5-CODEX")),
            SHARED_CODING_MODE_PROMPT_TEMPLATE
        );
        assert_eq!(
            agent.prompt_template_name(Some("claude-sonnet-4")),
            SHARED_CODING_MODE_PROMPT_TEMPLATE
        );
        assert_eq!(
            agent.prompt_template_name(None),
            SHARED_CODING_MODE_PROMPT_TEMPLATE
        );
    }

    #[test]
    fn shared_coding_tools_include_plan_and_debug_helpers() {
        let tools = shared_coding_mode_tools();
        assert!(tools.contains(&"CreatePlan".to_string()));
        assert!(tools.contains(&"Log".to_string()));
    }
}
