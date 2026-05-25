//! Intent Coding Mode

use crate::agentic::agents::{shared_coding_mode_tools, Agent, RequestContextPolicy};
use async_trait::async_trait;

const INTENT_CODING_MODE_PROMPT_TEMPLATE: &str = "intent_coding_mode";

pub struct IntentCodingMode {
    default_tools: Vec<String>,
}

impl Default for IntentCodingMode {
    fn default() -> Self {
        Self::new()
    }
}

impl IntentCodingMode {
    pub fn new() -> Self {
        let mut default_tools = shared_coding_mode_tools();
        default_tools.push("CreatePlan".to_string());
        Self { default_tools }
    }
}

#[async_trait]
impl Agent for IntentCodingMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "IntentCoding"
    }

    fn name(&self) -> &str {
        "Intent Coding"
    }

    fn description(&self) -> &str {
        "Intent-aligned coding mode that clarifies requirements, records acceptance checks, verifies changes, and delivers evidence"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        INTENT_CODING_MODE_PROMPT_TEMPLATE
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn request_context_policy(&self) -> RequestContextPolicy {
        RequestContextPolicy::empty()
            .with_workspace_context()
            .with_workspace_instructions()
            .with_workspace_memory_files()
            .with_project_layout()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::IntentCodingMode;
    use crate::agentic::agents::{get_embedded_prompt, Agent};

    #[test]
    fn intent_coding_mode_uses_dedicated_prompt_and_planning_tools() {
        let mode = IntentCodingMode::new();

        assert_eq!(mode.id(), "IntentCoding");
        assert_eq!(mode.prompt_template_name(None), "intent_coding_mode");

        let tools = mode.default_tools();
        assert!(tools.contains(&"AskUserQuestion".to_string()));
        assert!(tools.contains(&"TodoWrite".to_string()));
        assert!(tools.contains(&"CreatePlan".to_string()));
        assert!(tools.contains(&"Edit".to_string()));
    }

    #[test]
    fn intent_coding_prompt_embeds_acceptance_and_evidence_workflow() {
        let prompt = get_embedded_prompt("intent_coding_mode").expect("embedded prompt");

        assert!(prompt.contains("# Intent Coding workflow"));
        assert!(prompt.contains("Accepted Checks or Accepted Tests"));
        assert!(prompt.contains(".agent/rules/accepted-checks.md"));
        assert!(prompt.contains("acceptance coverage result"));
        assert!(prompt.contains("pnpm run agent:check"));
        assert!(prompt.contains("Evidence Package"));
    }
}
