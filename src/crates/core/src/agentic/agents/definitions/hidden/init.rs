use crate::agentic::agents::{Agent, RequestContextPolicy};
use async_trait::async_trait;

pub struct InitAgent {
    default_tools: Vec<String>,
}

impl Default for InitAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl InitAgent {
    pub fn new() -> Self {
        Self {
            default_tools: vec![
                "LS".to_string(),
                "Read".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "Write".to_string(),
                "Edit".to_string(),
                "Bash".to_string(),
                "ControlHub".to_string(),
            ],
        }
    }
}

#[async_trait]
impl Agent for InitAgent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "Init"
    }

    fn name(&self) -> &str {
        "Init"
    }

    fn description(&self) -> &str {
        "Agent for /init command"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "init_agent"
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

    fn is_readonly(&self) -> bool {
        false
    }
}
