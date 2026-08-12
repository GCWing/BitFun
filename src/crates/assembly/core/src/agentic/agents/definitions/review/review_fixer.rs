use crate::agentic::agents::{subagent_default_tools, Agent, AgentToolPolicyOverrides, UserContextPolicy};
use crate::agentic::tools::framework::ToolExposure;
use async_trait::async_trait;

pub struct ReviewFixerAgent {
    default_tools: Vec<String>,
    tool_exposure_overrides: AgentToolPolicyOverrides,
}

impl Default for ReviewFixerAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl ReviewFixerAgent {
    pub fn new() -> Self {
        let mut tool_exposure_overrides = AgentToolPolicyOverrides::default();
        tool_exposure_overrides.insert("GetFileDiff".to_string(), ToolExposure::Direct);
        tool_exposure_overrides.insert("Git".to_string(), ToolExposure::Direct);
        // 执行者工具模板改 agentic 全工具：ReviewFixer 也是执行修复的
        // 角色，工具不足一用就卡，改用 subagent_default_tools() 全工具清单
        // （TodoWrite/Plan 系列/Session 系列/Web 系列等），再补上专属的
        // GetFileDiff（不在 shared_coding_mode_tools 内）。
        let mut default_tools = subagent_default_tools();
        if !default_tools.contains(&"GetFileDiff".to_string()) {
            default_tools.push("GetFileDiff".to_string());
        }
        // 审查类智能体统一配齐 submit_code_review（severity 结构化提交）。
        if !default_tools.contains(&"submit_code_review".to_string()) {
            default_tools.push("submit_code_review".to_string());
        }
        Self {
            default_tools,
            tool_exposure_overrides,
        }
    }
}

#[async_trait]
impl Agent for ReviewFixerAgent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "ReviewFixer"
    }

    fn name(&self) -> &str {
        "Review Fixer"
    }

    fn description(&self) -> &str {
        r#"Bounded implementation subagent for deep-review remediation. Use it only after validated review findings exist and you want a minimal safe fix plus a concise verification summary before the next incremental review pass."#
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "review_fixer_agent"
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn user_context_policy(&self) -> UserContextPolicy {
        UserContextPolicy::empty()
            .with_workspace_context()
            .with_workspace_instructions()
    }

    fn tool_exposure_overrides(&self) -> &AgentToolPolicyOverrides {
        &self.tool_exposure_overrides
    }

    fn is_readonly(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{Agent, ReviewFixerAgent};
    use crate::agentic::agents::UserContextPolicy;

    #[test]
    fn review_fixer_agent_has_edit_and_verify_tools() {
        let agent = ReviewFixerAgent::new();
        let tools = agent.default_tools();

        assert_eq!(
            agent.user_context_policy(),
            UserContextPolicy::empty()
                .with_workspace_context()
                .with_workspace_instructions()
        );
        assert!(tools.contains(&"Edit".to_string()));
        assert!(tools.contains(&"Write".to_string()));
        assert!(tools.contains(&"ExecCommand".to_string()));
        assert!(tools.contains(&"WriteStdin".to_string()));
        assert!(tools.contains(&"ExecControl".to_string()));
        // 执行修复角色也用 agentic 全工具底子（TodoWrite/GetFileDiff）。
        assert!(tools.contains(&"TodoWrite".to_string()));
        assert!(tools.contains(&"GetFileDiff".to_string()));
        // 审查类智能体统一配齐 submit_code_review（severity 结构化提交）。
        assert!(tools.contains(&"submit_code_review".to_string()));
        // 审查工具全家桶：ReviewPlatform（subagent_default_tools 已含）。
        assert!(tools.contains(&"ReviewPlatform".to_string()));
        assert!(!agent.is_readonly());
    }
}
