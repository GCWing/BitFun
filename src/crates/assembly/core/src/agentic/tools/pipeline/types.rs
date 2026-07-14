//! Tool pipeline type definitions

use crate::agentic::core::{ToolCall, ToolExecutionState};
use crate::agentic::events::SubagentParentInfo as EventSubagentParentInfo;
use crate::agentic::round_preempt::DialogRoundInjectionInterrupt;
use crate::agentic::tools::ToolRuntimeRestrictions;
use crate::agentic::workspace::WorkspaceServices;
use crate::agentic::WorkspaceBinding;
use bitfun_agent_tools::ResolvedToolInvocation;
use bitfun_runtime_ports::{DelegationPolicy, RemoteExecPort, TerminalPort};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
pub use tool_runtime::context::PrimaryModelFacts;
pub use tool_runtime::pipeline::SubagentBatchExecutionPolicy;

/// Tool execution options
#[derive(Debug, Clone)]
pub struct ToolExecutionOptions {
    pub allow_parallel: bool,
    pub subagent_batch_execution_policy: SubagentBatchExecutionPolicy,
    pub max_retries: usize,
    /// Tool execution timeout (seconds), None means infinite waiting
    pub timeout_secs: Option<u64>,
    pub confirm_before_run: bool,
    /// Tool confirmation timeout (seconds), None means infinite waiting
    pub confirmation_timeout_secs: Option<u64>,
}

impl Default for ToolExecutionOptions {
    fn default() -> Self {
        Self {
            allow_parallel: true,
            subagent_batch_execution_policy: SubagentBatchExecutionPolicy::default(),
            max_retries: 0,
            timeout_secs: None, // Default no timeout (infinite waiting)
            confirm_before_run: true,
            confirmation_timeout_secs: None, // Default no timeout (infinite waiting)
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubagentParentInfo {
    pub tool_call_id: String,
    pub session_id: String,
    pub dialog_turn_id: String,
}

impl From<SubagentParentInfo> for EventSubagentParentInfo {
    fn from(info: SubagentParentInfo) -> Self {
        Self {
            tool_call_id: info.tool_call_id,
            session_id: info.session_id,
            dialog_turn_id: info.dialog_turn_id,
        }
    }
}

/// Tool execution context
#[derive(Debug, Clone)]
pub struct ToolExecutionContext {
    pub session_id: String,
    pub dialog_turn_id: String,
    pub round_id: String,
    pub attempt_id: Option<String>,
    pub attempt_index: Option<u32>,
    pub agent_type: String,
    pub workspace: Option<WorkspaceBinding>,
    pub primary_model_facts: PrimaryModelFacts,
    pub context_vars: HashMap<String, String>,
    pub subagent_parent_info: Option<SubagentParentInfo>,
    pub(crate) delegation_policy: DelegationPolicy,
    pub deferred_tools: Vec<String>,
    pub loaded_deferred_tool_specs: Vec<bitfun_agent_tools::LoadedDeferredToolSpec>,
    pub catalog_generation: u64,
    /// Allowed tools list (whitelist)
    /// If empty, allow all registered tools
    /// If not empty, only allow tools in the list to be executed
    pub allowed_tools: Vec<String>,
    pub runtime_tool_restrictions: ToolRuntimeRestrictions,
    /// Optional cooperative interrupt used to stop remaining tool calls when a
    /// round injection is waiting for this turn.
    pub steering_interrupt: Option<DialogRoundInjectionInterrupt>,
    pub workspace_services: Option<WorkspaceServices>,
    pub terminal_port: Option<Arc<dyn TerminalPort>>,
    pub remote_exec_port: Option<Arc<dyn RemoteExecPort>>,
}

/// Tool execution task
#[derive(Debug, Clone)]
pub struct ToolTask {
    pub tool_call: ToolCall,
    pub invocation: ResolvedToolInvocation,
    pub invocation_resolution_error: Option<String>,
    pub context: ToolExecutionContext,
    pub options: ToolExecutionOptions,
    pub state: ToolExecutionState,
    pub created_at: SystemTime,
    pub started_at: Option<SystemTime>,
    pub completed_at: Option<SystemTime>,
}

impl ToolTask {
    pub fn new(
        tool_call: ToolCall,
        context: ToolExecutionContext,
        options: ToolExecutionOptions,
    ) -> Self {
        let invocation = ResolvedToolInvocation::direct(
            tool_call.tool_name.clone(),
            tool_call.arguments.clone(),
        );
        Self::new_resolved(tool_call, invocation, None, context, options)
    }

    pub fn new_resolved(
        tool_call: ToolCall,
        invocation: ResolvedToolInvocation,
        invocation_resolution_error: Option<String>,
        context: ToolExecutionContext,
        options: ToolExecutionOptions,
    ) -> Self {
        Self {
            tool_call,
            invocation,
            invocation_resolution_error,
            context,
            options,
            state: ToolExecutionState::Queued { position: 0 },
            created_at: SystemTime::now(),
            started_at: None,
            completed_at: None,
        }
    }

    pub fn effective_tool_name(&self) -> &str {
        &self.invocation.effective_tool_name
    }

    pub fn effective_arguments(&self) -> &serde_json::Value {
        &self.invocation.effective_arguments
    }

    pub fn update_effective_arguments(&mut self, arguments: serde_json::Value) {
        self.invocation
            .replace_effective_arguments(arguments.clone());
        self.tool_call.arguments = self.invocation.wire_arguments.clone();
    }
}

/// Tool execution result wrapper
#[derive(Debug, Clone)]
pub struct ToolExecutionResult {
    pub tool_id: String,
    /// Provider-facing tool name. For deferred calls this remains CallDeferredTool.
    pub tool_name: String,
    /// Runtime target used for validation, permissions, hooks, and execution.
    pub effective_tool_name: String,
    pub result: crate::agentic::core::ToolResult,
    pub execution_time_ms: u64,
}
