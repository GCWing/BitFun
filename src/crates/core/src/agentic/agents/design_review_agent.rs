use super::{Agent, RequestContextPolicy};
use async_trait::async_trait;

pub struct DesignReviewAgent {
    default_tools: Vec<String>,
}

impl Default for DesignReviewAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl DesignReviewAgent {
    pub fn new() -> Self {
        Self {
            default_tools: vec![
                "Read".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "LS".to_string(),
            ],
        }
    }
}

#[async_trait]
impl Agent for DesignReviewAgent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "DesignReview"
    }

    fn name(&self) -> &str {
        "Design Review"
    }

    fn description(&self) -> &str {
        r#"Read-only design artifact reviewer for HTML/CSS/JS deliverables. Use after a meaningful design iteration to catch syntax issues, broken references, token misuse, fragile layout structure, likely overflow or alignment problems, and other high-probability defects before handoff. Prefer grounded, file-backed findings over broad aesthetic opinions."#
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "design_review"
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
    use super::{Agent, DesignReviewAgent};

    #[test]
    fn has_expected_default_tools() {
        let agent = DesignReviewAgent::new();
        assert_eq!(
            agent.default_tools(),
            vec![
                "Read".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "LS".to_string(),
            ]
        );
    }

    #[test]
    fn always_uses_default_prompt_template() {
        let agent = DesignReviewAgent::new();
        assert_eq!(agent.prompt_template_name(Some("gpt-5.1")), "design_review");
        assert_eq!(agent.prompt_template_name(None), "design_review");
    }

    #[test]
    fn is_readonly_for_safe_parallel_review() {
        let agent = DesignReviewAgent::new();
        assert!(agent.is_readonly());
    }
}
