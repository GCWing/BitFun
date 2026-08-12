//! Claw Mode

use crate::agentic::agents::{
    shared_coding_mode_tool_exposure_overrides, subagent_default_tools, Agent,
    AgentToolPolicyOverrides, UserContextPolicy,
};
use async_trait::async_trait;

/// Claw 独有工具（不在 subagent_default_tools 共享集内）：WorkspaceScan
/// （跨工作区扫描）、AgentWait（后台任务等待）、Cron（定时任务）。
const CLAW_EXCLUSIVE_TOOLS: &[&str] = &["WorkspaceScan", "AgentWait", "Cron"];

pub struct ClawMode {
    default_tools: Vec<String>,
    tool_exposure_overrides: AgentToolPolicyOverrides,
}

impl Default for ClawMode {
    fn default() -> Self {
        Self::new()
    }
}

impl ClawMode {
    pub fn new() -> Self {
        // 全套工具箱：subagent_default_tools()（agentic 全工具 + 会话核心）单源
        // 同步，再追加 Claw 独有工具（WorkspaceScan/AgentWait/Cron 不在共享集）。
        // Claw 助理会话默认即全量工具（含 TodoWrite/goal 族/Plan 族/
        // GenerativeUI/AskUserQuestion/ReviewPlatform/canvas 族 + 独有集）。
        let mut default_tools = subagent_default_tools();
        for tool in CLAW_EXCLUSIVE_TOOLS {
            if !default_tools.contains(&tool.to_string()) {
                default_tools.push(tool.to_string());
            }
        }
        Self {
            default_tools,
            tool_exposure_overrides: shared_coding_mode_tool_exposure_overrides(),
        }
    }
}

#[async_trait]
impl Agent for ClawMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "Claw"
    }

    fn name(&self) -> &str {
        "Claw"
    }

    fn description(&self) -> &str {
        "Personal assistant for daily tasks"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "claw_mode"
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn tool_exposure_overrides(&self) -> &AgentToolPolicyOverrides {
        // 继承共享编码模式的曝光覆盖：WebSearch/WebFetch/CreatePlan 提 Direct，
        // 省 GetToolSpec 解锁往返（与 agentic/Plan 等模式一致）。
        &self.tool_exposure_overrides
    }

    fn user_context_policy(&self) -> UserContextPolicy {
        UserContextPolicy::empty()
            .with_workspace_context()
            .with_workspace_instructions()
            .with_memory_summary()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::ClawMode;
    use crate::agentic::agents::{Agent, PromptBuilderContext};
    use bitfun_agent_runtime::prompt::UserContextSection;

    #[test]
    fn claw_mode_includes_miniapp_lifecycle_tools_in_defaults() {
        let tools = ClawMode::new().default_tools();
        assert!(tools.contains(&"InitMiniApp".to_string()));
        assert!(tools.contains(&"FinalizeMiniApp".to_string()));
        assert!(tools.contains(&"ListModels".to_string()));
    }

    #[test]
    fn claw_mode_defaults_to_full_toolkit_aligned_with_subagents() {
        // 全套工具箱（E2）：Claw 默认工具 = subagent_default_tools() 单源
        // + Claw 独有工具（WorkspaceScan/AgentWait/Cron）——含之前缺失的
        // TodoWrite/goal 族/Plan 族/GenerativeUI/AskUserQuestion/
        // ReviewPlatform/canvas 族，且保留 Claw 独有集。
        let tools = ClawMode::new().default_tools();
        let shared = crate::agentic::agents::subagent_default_tools();
        for tool in &shared {
            assert!(
                tools.contains(tool),
                "Claw default tools must include shared tool {}",
                tool
            );
        }
        for tool in [
            "TodoWrite",
            "get_goal",
            "create_goal",
            "update_goal",
            "GenerativeUI",
            "AskUserQuestion",
            "CreatePlan",
            "PlanList",
            "PlanRead",
            "PlanUpdate",
            "ReviewPlatform",
            "CreateCanvas",
            "ReadCanvas",
            "UpdateCanvas",
            "PatchCanvas",
        ] {
            assert!(
                tools.contains(&tool.to_string()),
                "Claw default tools must include {}",
                tool
            );
        }
        // Claw 独有集保留。
        for tool in ["WorkspaceScan", "AgentWait", "Cron"] {
            assert!(
                tools.contains(&tool.to_string()),
                "Claw default tools must include exclusive {}",
                tool
            );
        }
    }

    #[test]
    fn claw_mode_user_context_policy_includes_memory_summary() {
        assert!(ClawMode::new()
            .user_context_policy()
            .includes(UserContextSection::MemorySummary));
    }

    #[tokio::test]
    async fn claw_prompt_conditions_optional_control_and_session_tools() {
        let prompt = ClawMode::new()
            .get_system_prompt(Some(&PromptBuilderContext::new("/workspace", None, None)))
            .await
            .expect("Claw prompt");

        assert!(prompt.contains("only when it appears in your current tool list"));
        assert!(prompt.contains("only when both tools appear in your current tool list"));
    }
}
