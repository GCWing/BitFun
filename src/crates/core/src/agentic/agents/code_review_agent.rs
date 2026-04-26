//! Code Review Agent - Agentic code review with context gathering capabilities
//!
//! This agent can use Read/Grep/Glob/LS tools to gather context before
//! submitting a code review, reducing false positives from missing context.

use super::Agent;
use async_trait::async_trait;

pub struct CodeReviewAgent {
    default_tools: Vec<String>,
}

impl CodeReviewAgent {
    pub fn new() -> Self {
        Self {
            default_tools: vec![
                // Context gathering tools (read-only)
                "Read".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "LS".to_string(),
                "GetFileDiff".to_string(),
                // Code review submission tool
                "submit_code_review".to_string(),
                // User interaction tool
                "AskUserQuestion".to_string(),
                // Remediation tools, only after explicit user approval
                "Edit".to_string(),
                "Write".to_string(),
                "Bash".to_string(),
                "TodoWrite".to_string(),
                // Git operations tool
                "Git".to_string(),
            ],
        }
    }
}

impl Default for CodeReviewAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for CodeReviewAgent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "CodeReview"
    }

    fn name(&self) -> &str {
        "CodeReview"
    }

    fn description(&self) -> &str {
        r#"Agentic code review agent that can gather context before reviewing. Use this for thorough code reviews that require understanding of the broader codebase. The agent will use Read/Grep/Glob tools to understand function definitions, type structures, and related code before reporting issues."#
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "code_review"
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn is_readonly(&self) -> bool {
        false // Code review agent can remediate only after explicit user approval
    }
}

#[cfg(test)]
mod tests {
    use super::{Agent, CodeReviewAgent};

    #[test]
    fn code_review_agent_has_review_and_user_approved_remediation_tools() {
        let agent = CodeReviewAgent::new();
        let tools = agent.default_tools();

        assert!(tools.contains(&"Read".to_string()));
        assert!(tools.contains(&"Grep".to_string()));
        assert!(tools.contains(&"GetFileDiff".to_string()));
        assert!(tools.contains(&"submit_code_review".to_string()));
        assert!(tools.contains(&"AskUserQuestion".to_string()));
        assert!(tools.contains(&"Edit".to_string()));
        assert!(tools.contains(&"Write".to_string()));
        assert!(tools.contains(&"Bash".to_string()));
        assert!(tools.contains(&"TodoWrite".to_string()));
        assert!(!agent.is_readonly());
    }
}
