use super::Agent;
use async_trait::async_trait;

pub struct DeepResearchAgent {
    default_tools: Vec<String>,
}

impl Default for DeepResearchAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl DeepResearchAgent {
    pub fn new() -> Self {
        Self {
            default_tools: vec![
                // Sub-agent orchestration — parallel research via Task
                "Task".to_string(),
                // Web research (used in planning phase & direct lookups)
                "WebSearch".to_string(),
                "WebFetch".to_string(),
                // Codebase / file exploration
                "Read".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "LS".to_string(),
                // File output (save report)
                "Write".to_string(),
                // Terminal — run commands to gather data (e.g. git log, curl, jq)
                "Bash".to_string(),
                "TerminalControl".to_string(),
                // Task tracking
                "TodoWrite".to_string(),
            ],
        }
    }
}

#[async_trait]
impl Agent for DeepResearchAgent {
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

    fn is_readonly(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{Agent, DeepResearchAgent};

    #[test]
    fn has_expected_default_tools() {
        let agent = DeepResearchAgent::new();
        let tools = agent.default_tools();
        assert!(
            tools.contains(&"Task".to_string()),
            "Task tool required for parallel sub-agent orchestration"
        );
        assert!(tools.contains(&"WebSearch".to_string()));
        assert!(tools.contains(&"WebFetch".to_string()));
        assert!(tools.contains(&"Write".to_string()));
        assert!(tools.contains(&"Bash".to_string()));
        assert!(tools.contains(&"TerminalControl".to_string()));
    }

    #[test]
    fn always_uses_default_prompt_template() {
        let agent = DeepResearchAgent::new();
        assert_eq!(
            agent.prompt_template_name(Some("gpt-5.1")),
            "deep_research_agent"
        );
        assert_eq!(agent.prompt_template_name(None), "deep_research_agent");
    }
}
