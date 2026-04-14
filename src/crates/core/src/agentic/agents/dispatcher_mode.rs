//! Dispatcher Mode - Agentic OS scheduling center

use super::Agent;
use async_trait::async_trait;

pub struct DispatcherMode {
    default_tools: Vec<String>,
}

impl Default for DispatcherMode {
    fn default() -> Self {
        Self::new()
    }
}

impl DispatcherMode {
    pub fn new() -> Self {
        Self {
            default_tools: vec![
                // Core dispatch tool
                "AgentDispatch".to_string(),
                // Communicate with existing sessions
                "SessionMessage".to_string(),
                "SessionHistory".to_string(),
                // Information gathering - read-only file access
                "Read".to_string(),
                "Glob".to_string(),
                "Grep".to_string(),
                // Command execution for environment inspection
                "Bash".to_string(),
                // Web research
                "WebSearch".to_string(),
                "WebFetch".to_string(),
                // Structured thinking and task tracking
                "TodoWrite".to_string(),
                // Clarification
                "AskUserQuestion".to_string(),
            ],
        }
    }
}

#[async_trait]
impl Agent for DispatcherMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "Dispatcher"
    }

    fn name(&self) -> &str {
        "Dispatcher"
    }

    fn description(&self) -> &str {
        "Agentic OS scheduling center - understands user intent and creates the right agents to execute tasks"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "dispatcher_mode"
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}
