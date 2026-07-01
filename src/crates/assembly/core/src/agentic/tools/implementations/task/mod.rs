use crate::agentic::agents::{
    get_agent_registry, AgentInfo, SubagentListScope, SubagentQueryContext,
};
use crate::agentic::coordination::{get_global_coordinator, SubagentExecutionRequest};
use crate::agentic::deep_review::task_adapter::{
    self as deep_review_task_adapter, DeepReviewLaunchBatchInfo,
    DeepReviewProviderQueueWaitOutcome, DeepReviewQueueWaitOutcome, DeepReviewQueueWaitSkipReason,
};
use crate::agentic::deep_review_policy::{
    deep_review_active_reviewer_count, deep_review_effective_parallel_instances,
    deep_review_has_judge_been_launched, deep_review_turn_elapsed_seconds,
    load_default_deep_review_policy, record_deep_review_effective_concurrency_success,
    record_deep_review_runtime_auto_retry, record_deep_review_runtime_auto_retry_suppressed,
    record_deep_review_runtime_manual_retry, record_deep_review_task_budget,
    DeepReviewActiveReviewerGuard, DeepReviewCapacityQueueReason, DeepReviewConcurrencyPolicy,
    DeepReviewExecutionPolicy, DeepReviewPolicyViolation, DeepReviewRunManifestGate,
    DeepReviewSubagentRole, DEEP_REVIEW_AGENT_TYPE,
};
use crate::agentic::events::DeepReviewQueueStatus;
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::pipeline::SubagentParentInfo;
use crate::service::config::global::GlobalConfigManager;
use crate::service::config::types::AIConfig;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::timing::elapsed_ms_u64;
use async_trait::async_trait;
use bitfun_runtime_ports::SubagentContextMode;
use input::{TaskAction, TaskInvocation};
use log::{debug, warn};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::time::Instant;

mod background;
mod deep_review;
mod execution;
mod input;
mod launch_review_agent;
mod schema;
mod validation;

pub use launch_review_agent::LaunchReviewAgentTool;

pub struct TaskTool;

const LARGE_TASK_PROMPT_SOFT_LINE_LIMIT: usize = 180;
const LARGE_TASK_PROMPT_SOFT_BYTE_LIMIT: usize = 16 * 1024;

impl Default for TaskTool {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskTool {
    pub fn new() -> Self {
        Self
    }

    fn format_agent_descriptions(agents: &[AgentInfo]) -> String {
        if agents.is_empty() {
            return String::new();
        }
        let mut out = String::from("<available_agents>\n");
        for agent in agents {
            out.push_str(&format!(
                "<agent type=\"{}\">\n<description>\n{}\n</description>\n<tools>{}</tools>\n</agent>\n",
                agent.id,
                agent.description,
                agent.default_tools.join(", ")
            ));
        }
        out.push_str("</available_agents>");
        out
    }

    pub(crate) async fn build_available_agents_context_section(
        context: Option<&ToolUseContext>,
    ) -> Option<String> {
        let agents = Self::get_enabled_agents(context).await;
        let agent_descriptions = Self::format_agent_descriptions(&agents);
        if agent_descriptions.trim().is_empty() {
            None
        } else {
            Some(agent_descriptions)
        }
    }

    async fn get_enabled_agents(context: Option<&ToolUseContext>) -> Vec<AgentInfo> {
        let registry = get_agent_registry();
        let workspace_root = context.and_then(|ctx| ctx.workspace_root());
        registry.load_custom_agents(workspace_root).await;
        registry
            .get_subagents_for_query(&SubagentQueryContext {
                parent_agent_type: context.and_then(|ctx| ctx.agent_type.as_deref()),
                workspace_root,
                list_scope: SubagentListScope::TaskVisible,
                include_disabled: false,
            })
            .await
    }

    async fn get_agents_types(&self, context: Option<&ToolUseContext>) -> Vec<String> {
        Self::get_enabled_agents(context)
            .await
            .into_iter()
            .map(|agent| agent.id)
            .collect()
    }
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        "Task"
    }

    fn manages_own_execution_timeout(&self) -> bool {
        true
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(self.render_description())
    }

    async fn is_available_in_context(&self, _context: Option<&ToolUseContext>) -> bool {
        // Keep Task prompt-visible even when no fresh subagents are currently
        // available. Hiding it based on transient subagent availability makes
        // the tool manifest drift across turns and causes provider prefix/KV
        // cache misses. Task also still supports `fork_context=true` in that
        // state, so removing it from the manifest would be behaviorally wrong.
        true
    }

    fn short_description(&self) -> String {
        "Delegate work to a subagent task and collect the result.".to_string()
    }

    async fn description_with_context(
        &self,
        _context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        Ok(self.render_description())
    }

    fn input_schema(&self) -> Value {
        Self::regular_input_schema()
    }

    async fn input_schema_for_model_with_context(
        &self,
        _context: Option<&ToolUseContext>,
    ) -> Value {
        Self::regular_input_schema()
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, input: Option<&Value>) -> bool {
        if input
            .and_then(|value| Self::context_mode_from_input(value).ok())
            .is_some_and(|mode| mode == SubagentContextMode::Fork)
        {
            return false;
        }
        let subagent_type = input
            .and_then(|v| v.get("subagent_type"))
            .and_then(|v| v.as_str());
        match subagent_type {
            Some(id) => get_agent_registry()
                .get_subagent_is_readonly(id)
                .unwrap_or(false),
            None => false,
        }
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let _ = context;
        Self::validate_invocation_input(input, false)
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        match TaskAction::parse(input).ok() {
            Some(TaskAction::Cancel) => input
                .get("session_id")
                .and_then(Value::as_str)
                .map(|session_id| format!("Cancelling background task: {}", session_id))
                .unwrap_or_else(|| "Cancelling background task".to_string()),
            Some(TaskAction::SendInput) => input
                .get("description")
                .and_then(Value::as_str)
                .map(|description| {
                    if options.verbose {
                        format!("Sending input to task: {}", description)
                    } else {
                        format!("Task input: {}", description)
                    }
                })
                .unwrap_or_else(|| "Sending input to task".to_string()),
            Some(TaskAction::Spawn) | None => {
                if let Some(description) = input.get("description").and_then(|v| v.as_str()) {
                    if options.verbose {
                        format!("Creating task: {}", description)
                    } else {
                        format!("Task: {}", description)
                    }
                } else {
                    "Creating task".to_string()
                }
            }
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        self.call_task_impl(input, context).await
    }
}

#[cfg(test)]
mod tests;
