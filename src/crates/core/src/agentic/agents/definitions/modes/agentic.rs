//! Agentic Mode

use crate::agentic::agents::{
    get_embedded_prompt, shared_coding_mode_tools, shared_coding_mode_user_context_policy, Agent,
    UserContextPolicy, SHARED_CODING_MODE_PROMPT_TEMPLATE,
};
use async_trait::async_trait;

const AGENTIC_MODE_FIRST_ENTRY_REMINDER_TEMPLATE: &str = "agentic_mode_first_entry_reminder";

pub struct AgenticMode {
    default_tools: Vec<String>,
}

impl Default for AgenticMode {
    fn default() -> Self {
        Self::new()
    }
}

impl AgenticMode {
    pub fn new() -> Self {
        Self {
            default_tools: shared_coding_mode_tools(),
        }
    }

    fn load_reminder_template(
        &self,
        template_name: &str,
    ) -> crate::util::errors::BitFunResult<String> {
        get_embedded_prompt(template_name)
            .map(str::to_string)
            .ok_or_else(|| {
                crate::util::errors::BitFunError::Agent(format!(
                    "{} not found in embedded files",
                    template_name
                ))
            })
    }
}

#[async_trait]
impl Agent for AgenticMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "agentic"
    }

    fn name(&self) -> &str {
        "Agentic"
    }

    fn description(&self) -> &str {
        "Full-featured AI assistant with access to all tools for comprehensive software development tasks"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        SHARED_CODING_MODE_PROMPT_TEMPLATE
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
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
            Ok(String::new())
        } else {
            self.load_reminder_template(AGENTIC_MODE_FIRST_ENTRY_REMINDER_TEMPLATE)
        }
    }

    fn is_readonly(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::AgenticMode;
    use crate::agentic::agents::Agent;

    #[tokio::test]
    async fn returns_first_entry_reminder_only_when_entering_agentic_mode() {
        let mode = AgenticMode::new();

        assert_eq!(
            mode.get_system_reminder(None, None).await.unwrap(),
            "You have entered agentic mode."
        );
        assert!(mode
            .get_system_reminder(Some("agentic"), None)
            .await
            .unwrap()
            .is_empty());
    }
}
