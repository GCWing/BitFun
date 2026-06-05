//! Post-call hooks for generic tool execution.
//!
//! The tool framework stays generic and calls this module after successful
//! tool execution. Domain-specific hooks must keep their own gating inside the
//! owning domain module.

use crate::agentic::deep_review::tool_measurement;
use crate::agentic::tools::tool_context_runtime::ToolUseContext;
use bitfun_agent_runtime::post_call_hooks::{
    run_successful_tool_post_call_hooks, SuccessfulToolPostCallHookExecutor,
};
use serde_json::Value;

struct CorePostCallHookExecutor;

impl SuccessfulToolPostCallHookExecutor<ToolUseContext> for CorePostCallHookExecutor {
    fn record_deep_review_shared_context_tool_use(
        &mut self,
        tool_name: &str,
        input: &Value,
        context: &ToolUseContext,
    ) {
        tool_measurement::maybe_record_shared_context_tool_use(tool_name, input, context);
    }
}

pub(crate) fn record_successful_tool_call(
    tool_name: &str,
    input: &Value,
    context: &ToolUseContext,
) {
    let mut executor = CorePostCallHookExecutor;
    run_successful_tool_post_call_hooks(tool_name, input, context, &mut executor);
}
