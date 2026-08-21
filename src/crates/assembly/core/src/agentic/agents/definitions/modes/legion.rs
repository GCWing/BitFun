//! Workflow Mode — multi-agent workflow orchestration
//!
//! Fractal deployment topology: the commander only orchestrates (task
//! decomposition, agent session creation, message dispatch, quality gate
//! enforcement) and never executes. Every workflow member is a full agent
//! session that communicates via SessionMessage.

use crate::agentic::agents::{subagent_default_tools, Agent, UserContextPolicy};
use async_trait::async_trait;

/// Workflow 独有工具（不在 subagent_default_tools 共享集内）：LegionControl
/// （工作流模板一键部署）。
const LEGION_EXCLUSIVE_TOOLS: &[&str] = &["LegionControl"];

pub struct LegionMode {
    default_tools: Vec<String>,
}

impl Default for LegionMode {
    fn default() -> Self {
        Self::new()
    }
}

impl LegionMode {
    pub fn new() -> Self {
        // 共享子代理工具箱（含 SessionControl 裂变核心 + SessionMessage/
        // SessionHistory/goal 族），再追加 Legion 独有工具 LegionControl。
        let mut default_tools = subagent_default_tools();
        for tool in LEGION_EXCLUSIVE_TOOLS {
            if !default_tools.contains(&tool.to_string()) {
                default_tools.push(tool.to_string());
            }
        }
        Self { default_tools }
    }
}

#[async_trait]
impl Agent for LegionMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "Legion"
    }

    fn name(&self) -> &str {
        "Workflow"
    }

    fn description(&self) -> &str {
        "Multi-agent workflow commander: orchestrate agent sessions through a fractal deployment topology — decompose tasks, create sessions, dispatch via SessionMessage, enforce quality gates"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "legion_mode"
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
    use super::LegionMode;
    use crate::agentic::agents::Agent;

    #[test]
    fn legion_mode_basics() {
        let agent = LegionMode::new();
        assert_eq!(agent.id(), "Legion");
        assert_eq!(agent.prompt_template_name(None), "legion_mode");
        assert!(!agent.is_readonly());
        assert!(agent
            .default_tools()
            .contains(&"SessionControl".to_string()));
        assert!(agent
            .default_tools()
            .contains(&"SessionMessage".to_string()));
        assert!(agent.default_tools().contains(&"LegionControl".to_string()));
    }

    #[test]
    fn legion_mode_includes_all_subagent_shared_tools() {
        let tools = LegionMode::new().default_tools();
        let shared = crate::agentic::agents::subagent_default_tools();
        for tool in &shared {
            assert!(
                tools.contains(tool),
                "Legion default tools must include shared tool {}",
                tool
            );
        }
    }
}
