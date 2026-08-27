//! Creative Mode
//!
//! Owns product-creation capabilities that intentionally do not appear in the
//! default tool manifests of general coding, office, or assistant modes.

use crate::agentic::agents::{
    get_embedded_prompt, shared_coding_mode_tool_exposure_overrides, shared_coding_mode_tools,
    shared_coding_mode_user_context_policy, Agent, AgentToolPolicyOverrides, UserContextPolicy,
    SHARED_CODING_MODE_PROMPT_TEMPLATE,
};
use async_trait::async_trait;

const CREATIVE_MODE_FIRST_ENTRY_REMINDER_TEMPLATE: &str = "creative_mode_first_entry_reminder";

pub struct CreativeMode {
    default_tools: Vec<String>,
    tool_exposure_overrides: AgentToolPolicyOverrides,
}

impl Default for CreativeMode {
    fn default() -> Self {
        Self::new()
    }
}

impl CreativeMode {
    pub fn new() -> Self {
        let mut default_tools = shared_coding_mode_tools();
        default_tools.extend(
            [
                "InitMiniApp",
                "FinalizeMiniApp",
                "PublishMiniApp",
                "FrontendWorkbench",
            ]
            .into_iter()
            .map(str::to_string),
        );
        Self {
            default_tools,
            tool_exposure_overrides: shared_coding_mode_tool_exposure_overrides(),
        }
    }
}

#[async_trait]
impl Agent for CreativeMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "Creative"
    }

    fn name(&self) -> &str {
        "Creative"
    }

    fn description(&self) -> &str {
        "Creation mode for building MiniApps and safely customizing the running BitFun frontend"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        SHARED_CODING_MODE_PROMPT_TEMPLATE
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn tool_exposure_overrides(&self) -> &AgentToolPolicyOverrides {
        &self.tool_exposure_overrides
    }

    fn user_context_policy(&self) -> UserContextPolicy {
        shared_coding_mode_user_context_policy()
    }

    async fn get_system_reminder(
        &self,
        previous_agent_type: Option<&str>,
        _workspace: Option<&crate::agentic::WorkspaceBinding>,
    ) -> crate::util::errors::BitFunResult<String> {
        if previous_agent_type == Some(self.id()) {
            return Ok(String::new());
        }
        get_embedded_prompt(CREATIVE_MODE_FIRST_ENTRY_REMINDER_TEMPLATE)
            .map(str::to_string)
            .ok_or_else(|| {
                crate::util::errors::BitFunError::Agent(format!(
                    "{} not found in embedded files",
                    CREATIVE_MODE_FIRST_ENTRY_REMINDER_TEMPLATE
                ))
            })
    }

    fn is_readonly(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::CreativeMode;
    use crate::agentic::agents::Agent;

    #[test]
    fn creative_mode_owns_product_creation_tools() {
        let tools = CreativeMode::new().default_tools();
        for tool in [
            "InitMiniApp",
            "FinalizeMiniApp",
            "PublishMiniApp",
            "FrontendWorkbench",
        ] {
            assert!(tools.contains(&tool.to_string()), "missing {tool}");
        }
    }

    #[tokio::test]
    async fn creative_reminder_is_only_injected_on_entry() {
        let mode = CreativeMode::new();
        assert!(mode
            .get_system_reminder(None, None)
            .await
            .expect("reminder")
            .contains("FrontendWorkbench"));
        assert!(mode
            .get_system_reminder(Some("Creative"), None)
            .await
            .expect("ongoing reminder")
            .is_empty());
    }
}
