//! Intent Coding Mode

use crate::agentic::agents::{
    get_embedded_prompt, shared_coding_mode_tools, Agent, PromptBuilder, PromptBuilderContext,
    RequestContextPolicy,
};
use crate::util::errors::*;
use async_trait::async_trait;

const INTENT_CODING_MODE_PROMPT_TEMPLATE: &str = "intent_coding_mode";

// Embedded rules loaded from prompts/intent_coding_rules/
const EMBEDDED_RULES: &[(&str, &str)] = &[
    ("accepted-checks", include_str!("../../prompts/intent_coding_rules/accepted-checks.md")),
    ("architecture", include_str!("../../prompts/intent_coding_rules/architecture.md")),
    ("coding-style", include_str!("../../prompts/intent_coding_rules/coding-style.md")),
    ("error-classification", include_str!("../../prompts/intent_coding_rules/error-classification.md")),
    ("provenance-chain", include_str!("../../prompts/intent_coding_rules/provenance-chain.md")),
    ("risk-classification", include_str!("../../prompts/intent_coding_rules/risk-classification.md")),
    ("security", include_str!("../../prompts/intent_coding_rules/security.md")),
    ("workflow-check", include_str!("../../prompts/intent_coding_rules/workflow-check.md")),
];

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

    async fn build_prompt(&self, context: &PromptBuilderContext) -> BitFunResult<String> {
        let prompt_components = PromptBuilder::new(context.clone());
        let system_prompt_template = get_embedded_prompt(INTENT_CODING_MODE_PROMPT_TEMPLATE)
            .ok_or_else(|| {
                BitFunError::Agent(format!(
                    "{} not found in embedded files",
                    INTENT_CODING_MODE_PROMPT_TEMPLATE
                ))
            })?;

        let mut prompt = prompt_components
            .build_prompt_from_template(system_prompt_template)
            .await?;

        // Inject embedded Intent Coding rules as a context section.
        if !prompt.is_empty() {
            prompt.push_str("\n\n");
        }
        prompt.push_str("## Intent Coding rules\n\n");
        prompt.push_str(
            "The following rules are built into the IntentCoding mode. Follow them for every task.\n\n",
        );
        for (name, content) in EMBEDDED_RULES {
            prompt.push_str(&format!(
                "<document name=\"intent_coding_rules/{}.md\">\n{}\n</document>\n\n",
                name,
                content.trim()
            ));
        }

        Ok(prompt)
    }

    fn is_readonly(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::IntentCodingMode;
    use super::EMBEDDED_RULES;
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
        assert!(prompt.contains("acceptance coverage result"));
        assert!(prompt.contains("pnpm run agent:check"));
        assert!(prompt.contains("Evidence Package"));
    }

    #[test]
    fn intent_coding_embeds_required_rules() {
        let rules: Vec<&str> = EMBEDDED_RULES.iter().map(|(name, _)| *name).collect();
        assert!(!rules.is_empty());
        for name in [
            "risk-classification",
            "accepted-checks",
            "error-classification",
            "provenance-chain",
            "workflow-check",
            "security",
            "architecture",
            "coding-style",
        ] {
            assert!(rules.contains(&name), "missing rule: {name}");
        }
        for (_name, content) in EMBEDDED_RULES {
            assert!(!content.is_empty(), "rule content must not be empty");
        }
    }
}
