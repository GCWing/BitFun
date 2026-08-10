use crate::agentic::agents::{subagent_default_tools, Agent, UserContextPolicy};
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
        // 执行者工具模板改 agentic 全工具：执行者工具太少一用就卡，
        // 改用 subagent_default_tools()（shared_coding_mode_tools + SessionControl）
        // 的 agentic 全工具清单——TodoWrite/Plan 系列/SessionMessage/SessionHistory/
        // Git 等全部纳入。
        Self {
            default_tools: subagent_default_tools(),
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
    use crate::agentic::agents::subagent_default_tools;

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
    fn general_purpose_agent_gets_agentic_full_tool_suite() {
        // 执行者改 agentic 全工具（subagent_default_tools）。
        // 必须包含 TodoWrite/Plan 系列/会话系列/Git 等，不再是最小贫瘠集合。
        let agent = GeneralPurposeAgent::new();
        let tools = agent.default_tools();
        for tool in [
            "TodoWrite",
            "CreatePlan",
            "PlanList",
            "PlanRead",
            "PlanUpdate",
            "SessionControl",
            "SessionMessage",
            "SessionHistory",
            "Git",
            "ListModels",
        ] {
            assert!(
                tools.contains(&tool.to_string()),
                "GeneralPurpose default tools must include agentic tool {tool}"
            );
        }
    }

    #[test]
    fn general_purpose_agent_matches_subagent_default_tools() {
        // 执行者模板 = subagent_default_tools() 全集（含 SessionControl），
        // 与「agentic 类型工具」清单保持一致。
        let agent = GeneralPurposeAgent::new();
        assert_eq!(agent.default_tools(), subagent_default_tools());
    }
}
