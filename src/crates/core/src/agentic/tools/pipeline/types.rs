//! Tool pipeline type definitions

use crate::agentic::core::{ToolCall, ToolExecutionState};
use crate::agentic::events::SubagentParentInfo as EventSubagentParentInfo;
use std::collections::HashMap;
use std::time::SystemTime;

/// Tool execution options
#[derive(Debug, Clone)]
pub struct ToolExecutionOptions {
    pub allow_parallel: bool,
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
    pub agent_type: String,
    pub context_vars: HashMap<String, String>,
    pub subagent_parent_info: Option<SubagentParentInfo>,
    /// Allowed tools list (whitelist)
    /// If empty, allow all registered tools
    /// If not empty, only allow tools in the list to be executed
    pub allowed_tools: Vec<String>,
}

/// Tool execution task
#[derive(Debug, Clone)]
pub struct ToolTask {
    pub tool_call: ToolCall,
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
        Self {
            tool_call,
            context,
            options,
            state: ToolExecutionState::Queued { position: 0 },
            created_at: SystemTime::now(),
            started_at: None,
            completed_at: None,
        }
    }
}

/// Tool execution result wrapper
#[derive(Debug, Clone)]
pub struct ToolExecutionResult {
    pub tool_id: String,
    pub tool_name: String,
    pub result: crate::agentic::core::ToolResult,
    pub execution_time_ms: u64,
}
