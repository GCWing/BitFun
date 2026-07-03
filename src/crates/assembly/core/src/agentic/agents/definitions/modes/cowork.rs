//! Cowork Mode
//!
//! A collaborative mode that prioritizes early clarification and lightweight progress tracking.

use crate::agentic::agents::{Agent, AgentToolPolicyOverrides, UserContextPolicy};
use crate::agentic::tools::framework::ToolExposure;
use async_trait::async_trait;

pub struct CoworkMode {
    default_tools: Vec<String>,
    tool_exposure_overrides: AgentToolPolicyOverrides,
}

impl Default for CoworkMode {
    fn default() -> Self {
        Self::new()
    }
}

impl CoworkMode {
    pub fn new() -> Self {
        // Cowork is the office/research mode; web research is baseline there,
        // so keep WebSearch/WebFetch expanded (same as DeepResearch).
        let mut tool_exposure_overrides = AgentToolPolicyOverrides::default();
        tool_exposure_overrides.insert("WebSearch".to_string(), ToolExposure::Expanded);
        tool_exposure_overrides.insert("WebFetch".to_string(), ToolExposure::Expanded);
        Self {
            tool_exposure_overrides,
            default_tools: vec![
                // Clarification + planning helpers
                "AskUserQuestion".to_string(),
                "TodoWrite".to_string(),
                "Task".to_string(),
                "Skill".to_string(),
                // Discovery + editing
                "LS".to_string(),
                "Read".to_string(),
                "view_image".to_string(),
                "analyze_image".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "Write".to_string(),
                "Edit".to_string(),
                "Delete".to_string(),
                // Utilities
                "GetFileDiff".to_string(),
                "Git".to_string(),
                "ExecCommand".to_string(),
                "WriteStdin".to_string(),
                "ExecControl".to_string(),
                "WebSearch".to_string(),
                "WebFetch".to_string(),
                "ControlHub".to_string(),
                "InitMiniApp".to_string(),
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
        "Office and collaboration mode for documents, research, drafting, and structured multi-step work"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "cowork_mode"
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn tool_exposure_overrides(&self) -> &AgentToolPolicyOverrides {
        &self.tool_exposure_overrides
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

#[cfg(test)]
mod tests {
    use super::CoworkMode;
    use crate::agentic::agents::Agent;

    #[test]
    fn cowork_mode_includes_init_miniapp_in_default_tools() {
        let tools = CoworkMode::new().default_tools();
        assert!(tools.contains(&"InitMiniApp".to_string()));
    }
}
