//! Design Mode
//!
//! A design-focused mode that creates design artifacts and prototypes on behalf of the user.

use super::Agent;
use async_trait::async_trait;

pub struct DesignMode {
    default_tools: Vec<String>,
}

impl Default for DesignMode {
    fn default() -> Self {
        Self::new()
    }
}

impl DesignMode {
    pub fn new() -> Self {
        Self {
            default_tools: vec![
                // Clarification + planning helpers
                "AskUserQuestion".to_string(),
                "TodoWrite".to_string(),
                "Task".to_string(),
                "Skill".to_string(),
                // Discovery + editing
                "LS".to_string(),
                "Read".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "Write".to_string(),
                "Edit".to_string(),
                "Delete".to_string(),
                // Utilities
                "GetFileDiff".to_string(),
                "Bash".to_string(),
                "TerminalControl".to_string(),
                "WebSearch".to_string(),
                "ComputerUse".to_string(),
            ],
        }
    }
}

#[async_trait]
impl Agent for DesignMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "Design"
    }

    fn name(&self) -> &str {
        "Design"
    }

    fn description(&self) -> &str {
        "Design mode: create HTML-based design artifacts, prototypes, and visual deliverables"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "design_mode"
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}
