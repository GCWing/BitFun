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
                "view_image".to_string(),
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

    fn prompt_template_name(&self) -> &str {
        "agentic_mode"
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}
