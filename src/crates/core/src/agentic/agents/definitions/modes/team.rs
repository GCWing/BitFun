//! Team Mode — Virtual engineering team powered by gstack skills
//!
//! Orchestrates a full software development sprint through specialized roles:
//! Think → Plan → Build → Review → Test → Ship

use crate::agentic::agents::Agent;
use async_trait::async_trait;

pub struct TeamMode {
    default_tools: Vec<String>,
}

impl Default for TeamMode {
    fn default() -> Self {
        Self::new()
    }
}

impl TeamMode {
    pub fn new() -> Self {
        Self {
            default_tools: vec![
                "Skill".to_string(),
                "Task".to_string(),
                "Read".to_string(),
                "Write".to_string(),
                "Edit".to_string(),
                "Delete".to_string(),
                "Bash".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "WebSearch".to_string(),
                "WebFetch".to_string(),
                "TodoWrite".to_string(),
                "AskUserQuestion".to_string(),
                "Git".to_string(),
                "TerminalControl".to_string(),
                "ControlHub".to_string(),
                "GetFileDiff".to_string(),
            ],
        }
    }
}

#[async_trait]
impl Agent for TeamMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "Team"
    }

    fn name(&self) -> &str {
        "Team"
    }

    fn description(&self) -> &str {
        "Virtual engineering team: CEO, Eng Manager, Designer, Code Reviewer, QA Lead, Security Officer, Release Engineer — orchestrated through a full sprint workflow"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "team_mode"
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
    use super::{Agent, TeamMode};

    #[test]
    fn team_mode_basics() {
        let agent = TeamMode::new();
        assert_eq!(agent.id(), "Team");
        assert_eq!(agent.prompt_template_name(None), "team_mode");
        assert!(!agent.is_readonly());
        assert!(agent.default_tools().contains(&"Skill".to_string()));
    }
}
