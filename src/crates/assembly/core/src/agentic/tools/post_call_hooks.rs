//! Post-call hooks for generic tool execution.
//!
//! The tool framework stays generic and calls this module after successful
//! tool execution. Domain-specific hooks must keep their own gating inside the
//! owning domain module.

use crate::agentic::deep_review::tool_measurement;
use crate::agentic::tools::tool_context_runtime::ToolUseContext;
use bitfun_agent_runtime::post_call_hooks::{
    run_successful_tool_post_call_hooks, HookResult, SuccessfulToolPostCallHookExecutor,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

/// Tracks consecutive same-tool calls per session for stale-strategy detection.
static STALE_TRACKER: std::sync::LazyLock<Mutex<HashMap<String, StaleToolState>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Default)]
struct StaleToolState {
    last_tool: String,
    consecutive_count: u32,
}

/// Remove the stale-tracking entry for a given session.
///
/// Callers (e.g. session lifecycle hooks) should invoke this when a session
/// is completed, deleted, or cancelled so that the global tracker does not
/// grow without bound.
#[allow(dead_code)]
pub(crate) fn remove_stale_tracker_for_session(session_id: &str) {
    if let Ok(mut tracker) = STALE_TRACKER.lock() {
        tracker.remove(session_id);
    }
}

/// Max consecutive same-tool calls before abort.
const STALE_STRATEGY_THRESHOLD: u32 = 3;

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

    fn behavior_guard(
        &mut self,
        tool_name: &str,
        input: &Value,
        context: &ToolUseContext,
    ) -> HookResult {
        // 1. Stale strategy: detect repeated same-tool calls (dead-loop pattern).
        if let Some(session_id) = &context.session_id {
            if let Ok(mut tracker) = STALE_TRACKER.lock() {
                let entry = tracker
                    .entry(session_id.clone())
                    .or_insert_with(StaleToolState::default);
                if entry.last_tool == tool_name {
                    entry.consecutive_count += 1;
                } else {
                    entry.last_tool = tool_name.to_string();
                    entry.consecutive_count = 1;
                }
                if entry.consecutive_count >= STALE_STRATEGY_THRESHOLD {
                    return HookResult::Abort {
                        reason: format!(
                            "Tool '{}' called {} consecutive times without strategy change",
                            tool_name, entry.consecutive_count
                        ),
                        fix_instruction: format!(
                            "Stop retrying {tool}. Read the previous results, identify the root failure cause, and choose a different approach. Do not call {tool} again without a changed strategy.",
                            tool = tool_name
                        ),
                        max_retries: 0,
                    };
                }
            }
        }

        // 2. File read-before-edit: warn when editing without prior context.
        //    Full enforcement requires session-level file-read state tracking;
        //    this guard provides a Should-level prompt injection point.
        if matches!(
            tool_name,
            "Edit" | "Write" | "Delete" | "edit_file" | "write_file" | "delete_file"
        ) {
            if let Some(file_path) = input
                .get("file_path")
                .or_else(|| input.get("path"))
                .and_then(Value::as_str)
            {
                // Guard: editing a file without a known read in this turn
                // is a behavior smell — inject a soft constraint via
                // the result-for-assistant path rather than aborting.
                let _ = file_path; // reserved for read-state cross-check
            }
        }

        // 3. Exit code check: requires post-execution result inspection.
        //    Implemented at the tool pipeline level (exec_command.rs response
        //    handling), not in the post-call hook which fires before result
        //    inspection. Hook here serves as a future integration point.

        HookResult::Continue
    }
}

pub(crate) fn record_successful_tool_call(
    tool_name: &str,
    input: &Value,
    context: &ToolUseContext,
) -> HookResult {
    let mut executor = CorePostCallHookExecutor;
    run_successful_tool_post_call_hooks(tool_name, input, context, &mut executor)
}
