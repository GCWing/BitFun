use super::Agent;
use async_trait::async_trait;
pub struct ExploreAgent {
    default_tools: Vec<String>,
}

impl ExploreAgent {
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
impl Agent for ExploreAgent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "Explore"
    }

    fn name(&self) -> &str {
        "Explore"
    }

    fn description(&self) -> &str {
        r#"Subagent for **wide** codebase exploration only. Use when the main agent would need many sequential search/read rounds across multiple areas, or the user asks for an architectural survey. Do **not** use for narrow tasks: a known path, a single class/symbol lookup, one obvious Grep pattern, or reading a handful of files — the main agent should use Grep, Glob, and Read for those. When calling, set thoroughness in the prompt: \"quick\", \"medium\", or \"very thorough\"."#
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "explore_agent"
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn is_readonly(&self) -> bool {
        true
    }
}
