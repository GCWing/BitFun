use super::{Agent, RequestContextPolicy};
use async_trait::async_trait;
pub struct ExploreAgent {
    default_tools: Vec<String>,
}

impl Default for ExploreAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl ExploreAgent {
    pub fn new() -> Self {
        Self {
            default_tools: vec![
                "Grep".to_string(),
                "Glob".to_string(),
                "Read".to_string(),
                "LS".to_string(),
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
        r#"Read-only subagent for **wide** codebase exploration. Prefer search-first workflows: use Grep and Glob to narrow the space, then Read the small set of relevant files. Use LS only sparingly to confirm directory shape after search has narrowed the target. Do **not** use for narrow tasks: a known path, a single class/symbol lookup, one obvious Grep pattern, or reading a handful of files — the main agent should handle those directly. When calling, set thoroughness in the prompt: \"quick\", \"medium\", or \"very thorough\"."#
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "explore_agent"
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn request_context_policy(&self) -> RequestContextPolicy {
        RequestContextPolicy::instructions_only()
    }

    fn is_readonly(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{Agent, ExploreAgent};

    #[test]
    fn uses_search_first_default_tool_order() {
        let agent = ExploreAgent::new();
        assert_eq!(
            agent.default_tools(),
            vec![
                "Grep".to_string(),
                "Glob".to_string(),
                "Read".to_string(),
                "LS".to_string(),
            ]
        );
    }

    #[test]
    fn always_uses_default_prompt_template() {
        let agent = ExploreAgent::new();
        assert_eq!(agent.prompt_template_name(Some("gpt-5.1")), "explore_agent");
        assert_eq!(
            agent.prompt_template_name(Some("claude-sonnet-4")),
            "explore_agent"
        );
        assert_eq!(agent.prompt_template_name(None), "explore_agent");
    }
}
