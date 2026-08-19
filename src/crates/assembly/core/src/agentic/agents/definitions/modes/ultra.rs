use crate::agentic::agents::{Agent, UserContextPolicy};
use async_trait::async_trait;

/// The user-facing entry point for the Swarm planner.
pub struct UltraMode {
    default_tools: Vec<String>,
}

impl Default for UltraMode {
    fn default() -> Self {
        Self::new()
    }
}

impl UltraMode {
    pub fn new() -> Self {
        Self {
            default_tools: [
                "AgentSpawn",
                "AgentSendInput",
                "AgentInterrupt",
                "AgentWait",
                "Read",
                "Grep",
                "Glob",
                "LS",
                "AskUserQuestion",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        }
    }
}

#[async_trait]
impl Agent for UltraMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn id(&self) -> &str {
        "Ultra"
    }
    fn name(&self) -> &str {
        "Ultra"
    }
    fn description(&self) -> &str {
        "Swarm planning mode for decomposing complex work into coordinated worker and review tasks. It may issue many model requests concurrently, increasing API cost and provider rate-limit pressure."
    }
    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "ultra_mode"
    }
    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }
    fn user_context_policy(&self) -> UserContextPolicy {
        UserContextPolicy::empty()
            .with_workspace_context()
            .with_workspace_instructions()
            .with_project_layout()
    }
    fn is_readonly(&self) -> bool {
        false
    }
}
