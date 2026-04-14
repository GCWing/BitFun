//! Claw Mode

use super::{Agent, RequestContextPolicy};
use async_trait::async_trait;
pub struct ClawMode {
    default_tools: Vec<String>,
}

impl Default for ClawMode {
    fn default() -> Self {
        Self::new()
    }
}

impl ClawMode {
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
                "Skill".to_string(),
                "TerminalControl".to_string(),
                "SessionControl".to_string(),
                "SessionMessage".to_string(),
                "SessionHistory".to_string(),
                "Cron".to_string(),
                // All desktop automation consolidated into ComputerUse
                // (click_element, click, mouse_move, scroll, drag, screenshot, locate, etc.)
                "ComputerUse".to_string(),
            ],
        }
    }
}

#[async_trait]
impl Agent for ClawMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "Claw"
    }

    fn name(&self) -> &str {
        "Claw"
    }

    fn description(&self) -> &str {
        "Personal assistant for daily tasks"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "claw_mode"
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn request_context_policy(&self) -> RequestContextPolicy {
        RequestContextPolicy::full_without_layout()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}
