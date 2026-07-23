//! Multitask Mode
//!
//! Shared coding modes inherit the agentic baseline for
//! `SHARED_CODING_MODE_PROMPT_TEMPLATE`, `shared_coding_mode_tools()`,
//! `shared_coding_mode_tool_exposure_overrides()`, and
//! `shared_coding_mode_user_context_policy()`. Across the four coding modes,
//! only the reminder content differs.

use crate::agentic::agents::{
    get_embedded_prompt, shared_coding_mode_tool_exposure_overrides, shared_coding_mode_tools,
    shared_coding_mode_user_context_policy, Agent, AgentToolPolicyOverrides, UserContextPolicy,
    SHARED_CODING_MODE_PROMPT_TEMPLATE,
};
use async_trait::async_trait;

pub struct MultitaskMode {
    default_tools: Vec<String>,
    tool_exposure_overrides: AgentToolPolicyOverrides,
}

const MULTITASK_MODE_FIRST_ENTRY_REMINDER_TEMPLATE: &str = "multitask_mode_first_entry_reminder";
const MULTITASK_MODE_ONGOING_REMINDER_TEMPLATE: &str = "multitask_mode_ongoing_reminder";

impl Default for MultitaskMode {
    fn default() -> Self {
        Self::new()
    }
}

impl MultitaskMode {
    pub fn new() -> Self {
        Self {
            default_tools: shared_coding_mode_tools(),
            tool_exposure_overrides: shared_coding_mode_tool_exposure_overrides(),
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
impl Agent for MultitaskMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "Multitask"
    }

    fn name(&self) -> &str {
        "Multitask"
    }

    fn description(&self) -> &str {
        "Agentic coding mode optimized for orthogonal task decomposition and proactive parallel subagent execution"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        SHARED_CODING_MODE_PROMPT_TEMPLATE
    }

    fn user_context_policy(&self) -> UserContextPolicy {
        shared_coding_mode_user_context_policy()
    }

    fn tool_exposure_overrides(&self) -> &AgentToolPolicyOverrides {
        &self.tool_exposure_overrides
    }

    async fn get_system_reminder(
        &self,
        previous_agent_type: Option<&str>,
        _workspace: Option<&crate::agentic::WorkspaceBinding>,
    ) -> crate::util::errors::BitFunResult<String> {
        if previous_agent_type == Some(self.id()) {
            self.load_reminder_template(MULTITASK_MODE_ONGOING_REMINDER_TEMPLATE)
        } else {
            self.load_reminder_template(MULTITASK_MODE_FIRST_ENTRY_REMINDER_TEMPLATE)
        }
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}
