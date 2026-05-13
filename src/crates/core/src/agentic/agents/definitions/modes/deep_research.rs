use crate::agentic::agents::{Agent, AgentToolPolicyOverrides};
use crate::agentic::tools::framework::ToolExposure;
use async_trait::async_trait;

pub struct DeepResearchMode {
    default_tools: Vec<String>,
    tool_exposure_overrides: AgentToolPolicyOverrides,
}

impl Default for DeepResearchMode {
    fn default() -> Self {
        Self::new()
    }
}

impl DeepResearchMode {
    pub fn new() -> Self {
        let mut tool_exposure_overrides = AgentToolPolicyOverrides::default();
        tool_exposure_overrides.insert("WebSearch".to_string(), ToolExposure::Expanded);
        tool_exposure_overrides.insert("WebFetch".to_string(), ToolExposure::Expanded);
        Self {
            default_tools: vec![
                "Task".to_string(),
                "WebSearch".to_string(),
                "WebFetch".to_string(),
                "Read".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "LS".to_string(),
                "Write".to_string(),
                "Edit".to_string(),
                "Bash".to_string(),
                "TerminalControl".to_string(),
                "ControlHub".to_string(),
                "TodoWrite".to_string(),
                "AskUserQuestion".to_string(),
            ],
            tool_exposure_overrides,
        }
    }
}

#[async_trait]
impl Agent for DeepResearchMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "DeepResearch"
    }

    fn name(&self) -> &str {
        "Deep Research"
    }

    fn description(&self) -> &str {
        r#"Produces a comprehensive deep-research report on any subject using parallel sub-agent orchestration. Dispatches multiple research agents concurrently to investigate different chapters and competitors simultaneously, then synthesizes findings into a cohesive report. Uses the Longitudinal + Cross-sectional Analysis method covering full historical evolution, competitive landscape, and integrated synthesis. Best for open-ended research questions about products, companies, technologies, or individuals where depth, speed, and narrative quality matter."#
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "deep_research_agent"
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn tool_exposure_overrides(&self) -> &AgentToolPolicyOverrides {
        &self.tool_exposure_overrides
    }

    fn is_readonly(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{Agent, DeepResearchMode};

    #[test]
    fn has_expected_default_tools() {
        let agent = DeepResearchMode::new();
        let tools = agent.default_tools();
        assert!(
            tools.contains(&"Task".to_string()),
            "Task tool required for parallel sub-agent orchestration"
        );
        assert!(tools.contains(&"WebSearch".to_string()));
        assert!(tools.contains(&"WebFetch".to_string()));
        assert!(tools.contains(&"Write".to_string()));
        assert!(
            tools.contains(&"Edit".to_string()),
            "Edit required so the agent can continue a Write that was truncated by max_tokens"
        );
        assert!(tools.contains(&"Bash".to_string()));
        assert!(tools.contains(&"TerminalControl".to_string()));
        assert!(tools.contains(&"ControlHub".to_string()));
        assert!(
            tools.contains(&"AskUserQuestion".to_string()),
            "AskUserQuestion required for Pro mode Phase 0/5 user confirmation"
        );
    }

    #[test]
    fn always_uses_default_prompt_template() {
        let agent = DeepResearchMode::new();
        assert_eq!(
            agent.prompt_template_name(Some("gpt-5.1")),
            "deep_research_agent"
        );
        assert_eq!(agent.prompt_template_name(None), "deep_research_agent");
    }
}
