//! Multitask Mode

use crate::agentic::agents::{
    shared_coding_mode_tools, Agent, SHARED_CODING_MODE_PROMPT_TEMPLATE,
};
use async_trait::async_trait;

pub struct MultitaskMode {
    default_tools: Vec<String>,
}

impl Default for MultitaskMode {
    fn default() -> Self {
        Self::new()
    }
}

impl MultitaskMode {
    pub fn new() -> Self {
        Self {
            default_tools: shared_coding_mode_tools(),
        }
    }

    fn build_first_entry_reminder(&self) -> String {
        r#"You are now in Multitask mode.

Treat the task as a parallel work orchestration problem whenever it is beneficial. First decompose the work into orthogonal subtasks or an explicit DAG with clear dependency edges. Then use subagents proactively to execute independent branches in parallel.

Prefer:
- independent subtasks with minimal overlap
- clear ownership and deliverables per subagent
- parallel execution for non-blocking branches
- background execution for independent subagents whenever possible; prefer the Task tool's `run_in_background` mode instead of blocking on each branch
- local execution only for the immediate critical path

Do not force parallelism when the task is tiny or tightly coupled, but default to decomposition-first thinking in this mode."#
            .to_string()
    }

    fn build_ongoing_reminder(&self) -> String {
        r#"You are still in Multitask mode.

Continue working with a parallel-first mindset. Prefer orthogonal decomposition, preserve clear dependency edges, and keep using subagents proactively for independent branches whenever parallel execution is beneficial."#
            .to_string()
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

    async fn get_system_reminder(
        &self,
        previous_agent_type: Option<&str>,
        _workspace: Option<&crate::agentic::WorkspaceBinding>,
    ) -> crate::util::errors::BitFunResult<String> {
        if previous_agent_type == Some(self.id()) {
            Ok(self.build_ongoing_reminder())
        } else {
            Ok(self.build_first_entry_reminder())
        }
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}
