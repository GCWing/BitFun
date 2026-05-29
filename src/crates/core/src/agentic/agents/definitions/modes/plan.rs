//! Plan Mode

use crate::agentic::agents::{
    get_embedded_prompt, shared_coding_mode_tools, shared_coding_mode_user_context_policy, Agent,
    UserContextPolicy, SHARED_CODING_MODE_PROMPT_TEMPLATE,
};
use async_trait::async_trait;

pub struct PlanMode {
    default_tools: Vec<String>,
}

const PLAN_MODE_FIRST_ENTRY_REMINDER_TEMPLATE: &str = "plan_mode_first_entry_reminder";
const PLAN_MODE_ONGOING_REMINDER_TEMPLATE: &str = "plan_mode_ongoing_reminder";

impl Default for PlanMode {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanMode {
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
            self.load_reminder_template(PLAN_MODE_ONGOING_REMINDER_TEMPLATE)
        } else {
            self.load_reminder_template(PLAN_MODE_FIRST_ENTRY_REMINDER_TEMPLATE)
        }
    }

    fn is_readonly(&self) -> bool {
        // only modify plan file, not modify project code
        true
    }
}
