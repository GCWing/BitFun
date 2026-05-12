use crate::agentic::agents::{Agent, RequestContextPolicy};
use async_trait::async_trait;

pub struct ReviewFixerAgent {
    default_tools: Vec<String>,
}

impl Default for ReviewFixerAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl ReviewFixerAgent {
    pub fn new() -> Self {
        Self {
            default_tools: vec![
                "Read".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "LS".to_string(),
                "GetFileDiff".to_string(),
                "Edit".to_string(),
                "Write".to_string(),
                "Bash".to_string(),
                "TodoWrite".to_string(),
                "Git".to_string(),
            ],
        }
    }
}

#[async_trait]
impl Agent for ReviewFixerAgent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "ReviewFixer"
    }

    fn name(&self) -> &str {
        "Review Fixer"
    }

    fn description(&self) -> &str {
        r#"Bounded implementation subagent for deep-review remediation. Use it only after validated review findings exist and you want a minimal safe fix plus a concise verification summary before the next incremental review pass."#
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "review_fixer_agent"
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn request_context_policy(&self) -> RequestContextPolicy {
        RequestContextPolicy::instructions_only()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{Agent, ReviewFixerAgent};
    use crate::agentic::agents::RequestContextPolicy;

    #[test]
    fn review_fixer_agent_has_edit_and_verify_tools() {
        let agent = ReviewFixerAgent::new();
        let tools = agent.default_tools();

        assert_eq!(
            agent.request_context_policy(),
            RequestContextPolicy::instructions_only()
        );
        assert!(tools.contains(&"Edit".to_string()));
        assert!(tools.contains(&"Write".to_string()));
        assert!(tools.contains(&"Bash".to_string()));
        assert!(!agent.is_readonly());
    }
}
