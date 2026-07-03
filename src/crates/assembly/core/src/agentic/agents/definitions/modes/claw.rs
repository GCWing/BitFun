//! Claw Mode

use crate::agentic::agents::{Agent, UserContextPolicy};
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
                "view_image".to_string(),
                "analyze_image".to_string(),
                "Write".to_string(),
                "Edit".to_string(),
                "Delete".to_string(),
                "ExecCommand".to_string(),
                "WriteStdin".to_string(),
                "ExecControl".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "WebSearch".to_string(),
                "WebFetch".to_string(),
                "Skill".to_string(),
                "Git".to_string(),
                "SessionControl".to_string(),
                "SessionMessage".to_string(),
                "SessionHistory".to_string(),
                "Cron".to_string(),
                // Browser, terminal, and routing metadata live under ControlHub.
                // Local desktop/system control is delegated to the ComputerUse
                // agent/tool instead of being surfaced as a ControlHub domain.
                "ControlHub".to_string(),
                "InitMiniApp".to_string(),
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

    fn user_context_policy(&self) -> UserContextPolicy {
        UserContextPolicy::empty()
            .with_workspace_context()
            .with_workspace_instructions()
            .with_workspace_memory_files()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::ClawMode;
    use crate::agentic::agents::Agent;

    #[test]
    fn claw_mode_includes_init_miniapp_in_default_tools() {
        let tools = ClawMode::new().default_tools();
        assert!(tools.contains(&"InitMiniApp".to_string()));
    }
}
