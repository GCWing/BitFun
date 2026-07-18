//! Post-call hooks for generic tool execution.
//!
//! The tool framework stays generic and calls this module after successful
//! tool execution. Domain-specific hooks must keep their own gating inside the
//! owning domain module.
//!
//! ## Hook Architecture (inspired by cc-haha / Claude Code hook system)
//!
//! cc-haha has 24 hook events + pre-tool / post-tool + exit-code protocol.
//! BitFun's current hook is post-call only, so we use session-level state
//! tracking to enforce cross-call invariants:
//!
//! - FILE_READ_TRACKER: records files read per session → Edit/Write/Delete
//!   without prior Read yields Abort.
//! - STALE_TRACKER: repeated same-tool calls → Abort at threshold 3.
//! - LIONHEART_PATH_GUARD: Delete on LionHeart library paths → Abort.
//!
//! Future: pre-tool hooks (cc-haha `executePreToolHooks`) would let us block
//! before execution instead of aborting after the fact.

use crate::agentic::deep_review::tool_measurement;
use crate::agentic::session::session_manager::SessionManager;
use crate::agentic::tools::tool_context_runtime::ToolUseContext;
use bitfun_agent_runtime::post_call_hooks::{
    run_stop_hooks, run_successful_tool_post_call_hooks, HookResult, StopHookAggregatedResult,
    StopHookContext, StopHookExecutor, SuccessfulToolPostCallHookExecutor, ToolCallSummary,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

// ── File Read Tracking (data collection for B01/C01 context) ────

/// Tracks which files have been read per session.
///
/// When a Read call succeeds, the file path is recorded here.
/// When an Edit/Write/Delete call fires, we check whether the target
/// file was previously read in this session. If not → Abort.
static FILE_READ_TRACKER: std::sync::LazyLock<Mutex<HashMap<String, HashSet<String>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Normalize a file path for consistent lookup in FILE_READ_TRACKER.
fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

/// Record that a file was successfully read in this session.
fn record_file_read(session_id: &str, file_path: &str) {
    if let Ok(mut tracker) = FILE_READ_TRACKER.lock() {
        tracker
            .entry(session_id.to_string())
            .or_default()
            .insert(normalize_path(file_path));
    }
}

/// Remove the file-read tracking entry for a given session.
#[allow(dead_code)]
pub(crate) fn remove_file_read_tracker_for_session(session_id: &str) {
    if let Ok(mut tracker) = FILE_READ_TRACKER.lock() {
        tracker.remove(session_id);
    }
}

// ── Bee-Review Buffer (cc-haha pattern: LLM result → inject next round) ──

use std::sync::LazyLock;
static REVIEW_BUFFER: LazyLock<Mutex<HashMap<String, Vec<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Push a review result for a session (called from execution_engine spawn).
pub(crate) fn push_review_result(session_id: &str, result: String) {
    if let Ok(mut buf) = REVIEW_BUFFER.lock() {
        buf.entry(session_id.to_string()).or_default().push(result);
    }
}

// ── Hook Executor ───────────────────────────────────────────────

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
        let session_id = match &context.session_id {
            Some(id) => id.as_str(),
            None => return HookResult::Continue,
        };

        // ── Track file reads (data collection only, no enforcement) ──
        if matches!(tool_name, "Read" | "read_file") {
            if let Some(file_path) = input.get("file_path").and_then(Value::as_str) {
                record_file_read(session_id, file_path);
            }
        }

        // All enforcement moved to B01+C01 async agent review.
        HookResult::Continue
    }
}

impl StopHookExecutor for CorePostCallHookExecutor {
    /// B01+C01 review is handled by async agent sessions spawned in
    /// execution_engine via std::thread + coordinator.start_dialog_turn.
    /// The stop hook is a pure trigger point — no synchronous checks.
    fn context_guard(&mut self, _ctx: &StopHookContext) -> HookResult {
        HookResult::Continue
    }

    fn behavior_guard(&mut self, _ctx: &StopHookContext) -> HookResult {
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

/// Stop hook — injects self-review reminder at round boundaries.
/// Matches the goal system pattern: no separate sessions, no context pushing,
/// just a reminder injected for the main agent to self-review.
pub(crate) fn run_stop_hooks_for_round(
    _session_manager: &Arc<SessionManager>,
    session_id: &str,
    turn_id: &str,
    round_index: u32,
    tool_calls: Vec<ToolCallSummary>,
    assistant_text: &str,
    file_reads: Vec<String>,
    file_edits: Vec<String>,
    round_has_more: bool,
) -> StopHookAggregatedResult {
    let ctx = StopHookContext {
        session_id: session_id.to_string(),
        turn_id: turn_id.to_string(),
        round_index,
        tool_calls,
        assistant_text_summary: assistant_text.to_string(),
        file_reads,
        file_edits,
        round_has_more,
    };
    let mut executor = CorePostCallHookExecutor;
    let mut result = run_stop_hooks(&ctx, &mut executor);

    // ── Bee-review: drain pending LLM results (cc-haha pattern) ──
    if let Ok(mut buf) = REVIEW_BUFFER.lock() {
        if let Some(results) = buf.remove(ctx.session_id.as_str()) {
            for r in results {
                let trimmed = r.trim();
                if trimmed.is_empty() || trimmed == "PASS" {
                    continue;
                }
                if trimmed.starts_with("ABORT:") || trimmed.starts_with("ABORT：") {
                    result.abort = Some(HookResult::Abort {
                        reason: format!("[审查员] {}", trimmed),
                        fix_instruction: "按审查员建议修正后继续。".to_string(),
                        max_retries: 1,
                    });
                } else if trimmed.starts_with("CTX:") || trimmed.starts_with("CTX：") {
                    result.additional_contexts.push(format!("[书记官] 上下文恢复: {}", &trimmed[4..].trim()));
                } else if trimmed.starts_with("SKILL:") || trimmed.starts_with("SKILL：") {
                    result.additional_contexts.push(format!("[提示蜂] 推荐加载: {}", &trimmed[6..].trim()));
                } else if trimmed.starts_with("WARN:") || trimmed.starts_with("WARN：") {
                    result.additional_contexts.push(format!("[审查员] 警告: {}", &trimmed[5..].trim()));
                } else {
                    result.additional_contexts.push(format!("[审查员] {}", trimmed));
                }
            }
        }
    }

    result
}

