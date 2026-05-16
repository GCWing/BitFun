use crate::agentic::agents::Agent;
use async_trait::async_trait;

pub struct GenerateDocAgent {
    default_tools: Vec<String>,
}

impl Default for GenerateDocAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl GenerateDocAgent {
    pub fn new() -> Self {
        Self {
            default_tools: vec![
                "LS".to_string(),
                "Read".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
            ],
        }
    }
}

#[async_trait]
impl Agent for GenerateDocAgent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "GenerateDoc"
    }

    fn name(&self) -> &str {
        "GenerateDoc"
    }

    fn description(&self) -> &str {
        "Agent for generating documentation such as AGENTS.md, CLAUDE.md, README.md, etc."
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "generate_doc_agent"
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}
