//! Cowork Mode
//!
//! A collaborative mode that prioritizes early clarification and lightweight progress tracking.

use super::Agent;
use async_trait::async_trait;

pub struct CoworkMode {
    default_tools: Vec<String>,
}

impl CoworkMode {
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
                "ReadLints".to_string(),
                "Git".to_string(),
                "Bash".to_string(),
                "WebFetch".to_string(),
                "WebSearch".to_string(),
            ],
        }
    }
}

#[async_trait]
impl Agent for CoworkMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "Cowork"
    }

    fn name(&self) -> &str {
        "Cowork"
    }

    fn description(&self) -> &str {
        "Collaborative mode: clarify first, track progress lightly, verify outcomes"
    }

    fn prompt_template_name(&self) -> &str {
        "cowork_mode"
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}
