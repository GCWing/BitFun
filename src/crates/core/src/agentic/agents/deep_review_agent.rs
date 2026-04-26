use super::Agent;
use async_trait::async_trait;

pub struct DeepReviewAgent {
    default_tools: Vec<String>,
}

impl Default for DeepReviewAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl DeepReviewAgent {
    pub fn new() -> Self {
        Self {
            default_tools: vec![
                "Task".to_string(),
                "Read".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "LS".to_string(),
                "GetFileDiff".to_string(),
                "Git".to_string(),
                "submit_code_review".to_string(),
                "AskUserQuestion".to_string(),
                "Edit".to_string(),
                "Write".to_string(),
                "Bash".to_string(),
                "TodoWrite".to_string(),
            ],
        }
    }
}

#[async_trait]
impl Agent for DeepReviewAgent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "DeepReview"
    }

    fn name(&self) -> &str {
        "DeepReview"
    }

    fn description(&self) -> &str {
        r#"Local deep-review orchestrator that builds a parallel Code Review Team for substantial changes. It dispatches independent specialist reviewers for business logic, performance, and security, can perform user-approved remediation plus incremental re-review, and then runs a quality-inspector pass before producing a consolidated report."#
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "deep_review_agent"
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
    use super::{Agent, DeepReviewAgent};

    #[test]
    fn deep_review_agent_has_team_orchestration_tools() {
        let agent = DeepReviewAgent::new();
        let tools = agent.default_tools();

        assert!(tools.contains(&"Task".to_string()));
        assert!(tools.contains(&"submit_code_review".to_string()));
        assert!(tools.contains(&"AskUserQuestion".to_string()));
        assert!(tools.contains(&"Edit".to_string()));
        assert!(tools.contains(&"Write".to_string()));
        assert!(tools.contains(&"Bash".to_string()));
        assert!(!agent.is_readonly());
    }
}
