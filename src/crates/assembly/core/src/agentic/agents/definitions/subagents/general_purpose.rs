use crate::agentic::agents::{Agent, UserContextPolicy};
use async_trait::async_trait;

pub struct GeneralPurposeAgent {
    default_tools: Vec<String>,
}

impl Default for GeneralPurposeAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl GeneralPurposeAgent {
    pub fn new() -> Self {
        Self {
            default_tools: vec![
                "Read".to_string(),
                "view_image".to_string(),
                "analyze_image".to_string(),
                "Glob".to_string(),
                "Grep".to_string(),
                "Write".to_string(),
                "Edit".to_string(),
                "Delete".to_string(),
                "ExecCommand".to_string(),
                "WriteStdin".to_string(),
                "ExecControl".to_string(),
                "WebSearch".to_string(),
                "WebFetch".to_string(),
                "Skill".to_string(),
                "Task".to_string(),
            ],
        }
    }
}

#[async_trait]
impl Agent for GeneralPurposeAgent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "GeneralPurpose"
    }

    fn name(&self) -> &str {
        "General Purpose"
    }

    fn description(&self) -> &str {
        r#"General-purpose implementation and research subagent for multi-step tasks that need focused codebase search, targeted file edits."#
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "general_purpose_agent"
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

#[cfg(test)]
mod tests {
    use super::{Agent, GeneralPurposeAgent};

    #[test]
    fn general_purpose_agent_includes_task_for_delegation() {
        // R-14: executor subagents (GeneralPurpose) must keep the Task tool so
        // chain fission keeps working beyond the first delegation level.
        let agent = GeneralPurposeAgent::new();
        assert!(
            agent.default_tools().contains(&"Task".to_string()),
            "GeneralPurpose (executor) default tools must include Task"
        );
    }

    #[test]
    fn general_purpose_agent_includes_skill_for_skills_workflow() {
        // F4: subagents had no Skill tool by default; GeneralPurpose (executor)
        // must keep Skill so delegated runs can load specialized skills.
        let agent = GeneralPurposeAgent::new();
        assert!(
            agent.default_tools().contains(&"Skill".to_string()),
            "GeneralPurpose (executor) default tools must include Skill"
        );
    }

    #[test]
    fn general_purpose_agent_keeps_core_working_tools() {
        let agent = GeneralPurposeAgent::new();
        let tools = agent.default_tools();
        for tool in [
            "Read",
            "view_image",
            "analyze_image",
            "Glob",
            "Grep",
            "Write",
            "Edit",
            "Delete",
            "ExecCommand",
            "WriteStdin",
            "ExecControl",
            "WebSearch",
            "WebFetch",
            "Skill",
        ] {
            assert!(
                tools.contains(&tool.to_string()),
                "GeneralPurpose default tools must keep {tool}"
            );
        }
    }

    #[test]
    fn general_purpose_agent_does_not_get_session_series() {
        // Executor delegation uses Task, not the Session toolset; keep the
        // tool set minimal and unchanged apart from Task.
        let agent = GeneralPurposeAgent::new();
        let tools = agent.default_tools();
        assert!(!tools.contains(&"SessionControl".to_string()));
        assert!(!tools.contains(&"SessionMessage".to_string()));
        assert!(!tools.contains(&"SessionHistory".to_string()));
    }
}
