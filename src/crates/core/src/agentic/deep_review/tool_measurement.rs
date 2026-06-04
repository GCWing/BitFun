//! Deep Review shared-context measurement hook for successful tool calls.
//!
//! The hook is intentionally narrow: only successful reviewer `Read` and
//! `GetFileDiff` calls are measured, and BitFun runtime URIs are ignored. It
//! records normalized metadata for diagnostics, not file contents.

use crate::agentic::deep_review_policy::record_deep_review_shared_context_tool_use;
use crate::agentic::tools::framework::ToolUseContext;
use bitfun_agent_runtime::post_call_hooks::{
    resolve_deep_review_shared_context_tool_use, DeepReviewSharedContextToolUseFacts,
};
use serde_json::Value;

pub(crate) fn maybe_record_shared_context_tool_use(
    tool_name: &str,
    input: &Value,
    context: &ToolUseContext,
) {
    let Some(record) =
        resolve_deep_review_shared_context_tool_use(DeepReviewSharedContextToolUseFacts {
            tool_name,
            input,
            custom_data: &context.custom_data,
            workspace_root: context.workspace_root(),
            is_remote: context.is_remote(),
            agent_type: context.agent_type.as_deref(),
        })
    else {
        return;
    };

    record_deep_review_shared_context_tool_use(
        &record.parent_turn_id,
        &record.subagent_type,
        &record.tool_name,
        &record.measured_path,
    );
}
