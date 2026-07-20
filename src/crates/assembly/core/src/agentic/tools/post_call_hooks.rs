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
use crate::agentic::tools::tool_context_runtime::ToolUseContext;
use bitfun_agent_runtime::post_call_hooks::{
    run_stop_hooks, run_successful_tool_post_call_hooks, HookResult, StopHookAggregatedResult,
    StopHookContext, StopHookExecutor, SuccessfulToolPostCallHookExecutor, ToolCallSummary,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

// ── Guard 1: Stale Strategy Detection ──────────────────────────

/// Tracks consecutive same-tool calls per session for stale-strategy detection.
static STALE_TRACKER: std::sync::LazyLock<Mutex<HashMap<String, StaleToolState>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Default)]
struct StaleToolState {
    last_tool: String,
    consecutive_count: u32,
}

/// Max consecutive same-tool calls before abort.
const STALE_STRATEGY_THRESHOLD: u32 = 3;

/// Remove the stale-tracking entry for a given session.
#[allow(dead_code)]
pub(crate) fn remove_stale_tracker_for_session(session_id: &str) {
    if let Ok(mut tracker) = STALE_TRACKER.lock() {
        tracker.remove(session_id);
    }
}

// ── Guard 2: Read-before-Edit Enforcement ───────────────────────

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

/// Check whether a file was previously read in this session.
fn was_file_read(session_id: &str, file_path: &str) -> bool {
    FILE_READ_TRACKER
        .lock()
        .ok()
        .and_then(|tracker| {
            tracker
                .get(session_id)
                .map(|files| files.contains(&normalize_path(file_path)))
        })
        .unwrap_or(false)
}

/// Remove the file-read tracking entry for a given session.
#[allow(dead_code)]
pub(crate) fn remove_file_read_tracker_for_session(session_id: &str) {
    if let Ok(mut tracker) = FILE_READ_TRACKER.lock() {
        tracker.remove(session_id);
    }
}

// ── Guard 3: LionHeart Path Protection ──────────────────────────

/// Paths that must never be deleted or modified by AI agents.
const PROTECTED_PATH_PREFIXES: &[&str] = &["e:/lionheart library", "e:\\lionheart library"];

fn is_protected_path(file_path: &str) -> bool {
    let normalized = normalize_path(file_path);
    PROTECTED_PATH_PREFIXES
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
}

// ── Guard 4: Unified Session Cleanup ────────────────────────────

/// Remove all per-session tracking state for a given session.
///
/// Call this from session lifecycle hooks (completion, deletion, cancellation).
#[allow(dead_code)]
pub(crate) fn remove_all_trackers_for_session(session_id: &str) {
    remove_stale_tracker_for_session(session_id);
    remove_file_read_tracker_for_session(session_id);
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

        // ── Track file reads (after successful Read) ──
        if matches!(tool_name, "Read" | "read_file") {
            if let Some(file_path) = input.get("file_path").and_then(Value::as_str) {
                record_file_read(session_id, file_path);
            }
        }

        // ── Guard: Stale strategy detection ──
        if let Ok(mut tracker) = STALE_TRACKER.lock() {
            let entry = tracker
                .entry(session_id.to_string())
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

        // ── Guard: Read-before-Edit / Read-before-Delete enforcement ──
        if matches!(
            tool_name,
            "Edit" | "Write" | "Delete" | "edit_file" | "write_file" | "delete_file"
        ) {
            if let Some(file_path) = input
                .get("file_path")
                .or_else(|| input.get("path"))
                .and_then(Value::as_str)
            {
                // Guard 3a: LionHeart path protection (Delete/Write)
                if matches!(tool_name, "Delete" | "delete_file" | "Write" | "write_file") {
                    if is_protected_path(file_path) {
                        return HookResult::Abort {
                            reason: format!(
                                "Attempted to {} on protected path: {}",
                                tool_name, file_path
                            ),
                            fix_instruction: format!(
                                "E:/LionHeart library/ is the soul mother — absolute red line (Iron Rule 1). Never delete or overwrite files here. This operation is denied.",
                            ),
                            max_retries: 0,
                        };
                    }
                }

                // Guard 3b: Read-before-Edit — hard enforcement
                if !was_file_read(session_id, file_path) {
                    return HookResult::Abort {
                        reason: format!(
                            "Tool '{}' called on '{}' without prior Read in this session",
                            tool_name, file_path
                        ),
                        fix_instruction: format!(
                            "Iron Rule: Read before you Edit. You must call Read on '{}' first to understand its current content, then Edit with exact text from the Read result. Never edit a file from memory.",
                            file_path
                        ),
                        max_retries: 1,
                    };
                }
            }
        }

        // ── Guard: ExecCommand basic safety ──
        if matches!(tool_name, "ExecCommand" | "exec_command" | "Bash") {
            if let Some(cmd) = input.get("cmd").and_then(Value::as_str) {
                // Detect PowerShell + Chinese characters (known corruption pattern)
                let has_chinese = cmd.contains(|c: char| c >= '\u{4e00}' && c <= '\u{9fff}');
                let uses_powershell = cmd.contains("powershell")
                    || cmd.contains("PowerShell")
                    || cmd.contains("pwsh");
                if has_chinese && uses_powershell {
                    return HookResult::Abort {
                        reason: "PowerShell with Chinese characters detected — known to corrupt encoding".to_string(),
                        fix_instruction: "Write the command as a standalone .js or .ps1 script file, then execute the script. Do not inline Chinese characters in PowerShell -Command.".to_string(),
                        max_retries: 1,
                    };
                }
            }
        }

        HookResult::Continue
    }
}

impl StopHookExecutor for CorePostCallHookExecutor {
    /// B01 提示蜂 — context completeness check.
    /// Checks whether the agent had sufficient context this round.
    fn context_guard(&mut self, ctx: &StopHookContext) -> HookResult {
        // Round-level context check: did the agent edit files without reading them?
        for edit in &ctx.file_edits {
            let normalized_edit = edit.replace('\\', "/").trim_end_matches('/').to_lowercase();
            let was_read = ctx.file_reads.iter().any(|r| {
                let nr = r.replace('\\', "/").trim_end_matches('/').to_lowercase();
                nr == normalized_edit
            });
            if !was_read {
                // Already caught by per-tool FILE_READ_TRACKER; here we do
                // round-level aggregation but don't double-abort.
                log::warn!(
                    "[B01 提示蜂] Round {}: file '{}' was edited but not read this round",
                    ctx.round_index,
                    edit
                );
            }
        }
        HookResult::Continue
    }

    /// C01 审查蜂 — iron-rule violation check at round level.
    fn behavior_guard(&mut self, ctx: &StopHookContext) -> HookResult {
        // 1. Read-before-Edit round-level aggregation:
        //    If every single edit this round was unread, that's a systemic violation.
        if !ctx.file_edits.is_empty() && !ctx.file_reads.is_empty() {
            let all_unread = ctx.file_edits.iter().all(|edit| {
                let ne = edit.replace('\\', "/").trim_end_matches('/').to_lowercase();
                !ctx.file_reads.iter().any(|r| {
                    let nr = r.replace('\\', "/").trim_end_matches('/').to_lowercase();
                    nr == ne
                })
            });
            if all_unread && ctx.file_edits.len() >= 2 {
                return HookResult::Abort {
                    reason: format!(
                        "[C01 审查蜂] Round {}: {} files edited but none were read first",
                        ctx.round_index,
                        ctx.file_edits.len()
                    ),
                    fix_instruction: "Iron Rule: Read before you Edit. Read EVERY file you plan to modify BEFORE calling Edit. Do not edit from memory.".to_string(),
                    max_retries: 1,
                };
            }
        }

        // 2. Round-level tool error pattern detection:
        //    If ALL tool calls in the round failed, the agent is stuck.
        if !ctx.tool_calls.is_empty() && ctx.tool_calls.iter().all(|tc| tc.is_error) {
            return HookResult::Abort {
                reason: format!(
                    "[C01 审查蜂] Round {}: all {} tool calls failed — agent is stuck",
                    ctx.round_index,
                    ctx.tool_calls.len()
                ),
                fix_instruction: "All tool calls failed this round. Stop and re-evaluate your approach. What are you trying to achieve? Is there a different way?".to_string(),
                max_retries: 1,
            };
        }

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

/// Convenience function to run B01/C01 stop hooks for a round.
///
/// Called from the execution engine after each round completes.
pub(crate) fn run_stop_hooks_for_round(
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
                    // 审查蜂无拦截权限，ABORT 降级为 WARN
                    result.additional_contexts.push(format!("[审查员] 提醒: {}", &trimmed[6..].trim()));
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
