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
        let default_tools = [
            "Read",
            "Edit",
            "Write",
            "ExecCommand",
            "WriteStdin",
            "ExecControl",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
        Self { default_tools }
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

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn user_context_policy(&self) -> UserContextPolicy {
        UserContextPolicy::empty()
            .with_workspace_context()
            .with_workspace_instructions()
    }

    fn include_dynamic_mcp_tools(&self) -> bool {
        false
    }

    fn is_readonly(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::MinimalMode;
    use crate::agentic::agents::Agent;

    #[test]
    fn minimal_manifest_is_stable_and_has_no_listing_tools() {
        let mode = MinimalMode::new();
        assert_eq!(
            mode.default_tools(),
            [
                "Read",
                "Edit",
                "Write",
                "ExecCommand",
                "WriteStdin",
                "ExecControl"
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
        );
        assert!(!mode.include_dynamic_mcp_tools());
    }
}
