//! Post-call hooks for generic tool execution.
//!
//! The tool framework stays generic and calls this module after successful
//! tool execution. Domain-specific hooks must keep their own gating inside the
//! owning domain module.

use crate::agentic::deep_review::tool_measurement;
use crate::agentic::tools::framework::ToolUseContext;
use serde_json::Value;

pub(crate) fn record_successful_tool_call(
    tool_name: &str,
    input: &Value,
    context: &ToolUseContext,
) {
    tool_measurement::maybe_record_shared_context_tool_use(tool_name, input, context);
}
