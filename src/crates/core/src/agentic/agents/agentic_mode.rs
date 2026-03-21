//! Agentic Mode

use super::Agent;
use async_trait::async_trait;
pub struct AgenticMode {
    default_tools: Vec<String>,
}

impl AgenticMode {
    pub fn new() -> Self {
        Self {
            default_tools: vec![
                "Task".to_string(),
                "Read".to_string(),
                "Write".to_string(),
                "Edit".to_string(),
                "Delete".to_string(),
                "Bash".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "WebSearch".to_string(),
                "TodoWrite".to_string(),
                "IdeControl".to_string(),
                "MermaidInteractive".to_string(),
                "ReadLints".to_string(),
                "Skill".to_string(),
                "AskUserQuestion".to_string(),
                "Git".to_string(),
                "TerminalControl".to_string(),
            ],
        }
    }
}

#[async_trait]
impl Agent for AgenticMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "agentic"
    }

    fn name(&self) -> &str {
        "Agentic"
    }

    fn description(&self) -> &str {
        "Full-featured AI assistant with access to all tools for comprehensive software development tasks"
    }

    fn prompt_template_name(&self, model_name: Option<&str>) -> &str {
        let Some(model_name) = model_name else {
            return "agentic_mode";
        };
        let model_name = model_name.trim().to_ascii_lowercase();
        if model_name.contains("gpt-5") {
            "agentic_mode_gpt5"
        } else {
            "agentic_mode"
        }
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
    use super::{Agent, AgenticMode};

    #[test]
    fn selects_gpt5_prompt_template() {
        let agent = AgenticMode::new();
        assert_eq!(
            agent.prompt_template_name(Some("gpt-5.1")),
            "agentic_mode_gpt5"
        );
        assert_eq!(
            agent.prompt_template_name(Some("GPT-5-CODEX")),
            "agentic_mode_gpt5"
        );
    }

    #[test]
    fn keeps_default_template_for_non_gpt5_models() {
        let agent = AgenticMode::new();
        assert_eq!(
            agent.prompt_template_name(Some("claude-sonnet-4")),
            "agentic_mode"
        );
        assert_eq!(agent.prompt_template_name(None), "agentic_mode");
    }
}
