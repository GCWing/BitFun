use crate::agentic::agents::{Agent, UserContextPolicy};
use async_trait::async_trait;

/// A focused Agent with a stable prompt and tool manifest.
pub struct MinimalMode {
    default_tools: Vec<String>,
}

impl Default for MinimalMode {
    fn default() -> Self {
        Self::new()
    }
}

impl MinimalMode {
    pub fn new() -> Self {
        Self {
            default_tools: [
                "Read",
                "Edit",
                "Write",
                "ExecCommand",
                "WriteStdin",
                "ExecControl",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        }
    }
}

#[async_trait]
impl Agent for MinimalMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "minimal"
    }

    fn name(&self) -> &str {
        "Minimal"
    }

    fn description(&self) -> &str {
        "Minimal coding mode with a stable, focused tool set."
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "minimal-harness-v1"
    }

    fn user_context_policy(&self) -> UserContextPolicy {
        UserContextPolicy::empty()
            .with_workspace_context()
            .with_workspace_instructions()
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn include_dynamic_mcp_tools(&self) -> bool {
        false
    }

    fn include_implicit_thread_goal_tools(&self) -> bool {
        false
    }

    fn is_readonly(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_manifest_is_stable_and_focused() {
        let mode = MinimalMode::new();

        assert_eq!(
            mode.default_tools(),
            vec![
                "Read",
                "Edit",
                "Write",
                "ExecCommand",
                "WriteStdin",
                "ExecControl",
            ]
        );
        assert!(!mode.include_dynamic_mcp_tools());
        assert!(!mode.include_implicit_thread_goal_tools());
        assert!(!mode.is_readonly());
    }
}
