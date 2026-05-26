//! Plan Mode

use crate::agentic::agents::{Agent, UserContextPolicy};
use async_trait::async_trait;

const PLAN_MODE_PROMPT_TEMPLATE: &str = "plan_mode";

pub struct PlanMode {
    default_tools: Vec<String>,
}

impl Default for PlanMode {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanMode {
    pub fn new() -> Self {
        Self {
            default_tools: vec![
                "Task".to_string(),
                "LS".to_string(),
                "Read".to_string(),
                "Write".to_string(),
                "Edit".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "AskUserQuestion".to_string(),
                "CreatePlan".to_string(),
                "ControlHub".to_string(),
            ],
        }
    }
}

#[async_trait]
impl Agent for PlanMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "Plan"
    }

    fn name(&self) -> &str {
        "Plan"
    }

    fn description(&self) -> &str {
        "Clarify request and create an implementation plan before executing the task"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        PLAN_MODE_PROMPT_TEMPLATE
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn user_context_policy(&self) -> UserContextPolicy {
        UserContextPolicy::empty()
            .with_workspace_context()
            .with_workspace_instructions()
            .with_workspace_memory_files()
            .with_project_layout()
    }

    fn is_readonly(&self) -> bool {
        // only modify plan file, not modify project code
        true
    }
}
