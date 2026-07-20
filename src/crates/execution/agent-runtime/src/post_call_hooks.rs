//! Portable post-call and lifecycle hook routing decisions.
//!
//! Hooks are organized into two tiers:
//!
//! - **Per-tool hooks** (`SuccessfulToolPostCall`, `BehaviorGuard`):
//!   fire after each successful tool call. Fine-grained, single-operation scope.
//!
//! - **Turn-level hooks** (`Stop`):
//!   fire after each dialog round completes. Whole-round scope — can inspect
//!   the cumulative effect of multiple tool calls in a single round.
//!
//! Inspired by cc-haha's Stop hook (used by `/goal` to evaluate progress
//! after every assistant turn) and the LionBuddy V10 supervisor chain
//! (Plan→Do→Check→Act with peer review after each stage).

use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Hook categories that concrete runtime integrations may execute.
///
/// `SuccessfulToolPostCall` / `BehaviorGuard` fire per-tool.
/// `Stop` fires per-round (after the assistant message and all tool
/// results for a round are collected).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RuntimeHookKind {
    SuccessfulToolPostCall,
    DeepReviewSharedContextToolUse,
    BehaviorGuard,
    /// Fires after each dialog round completes (assistant message + tool results).
    /// Carries round-level context including all tool calls in the round.
    Stop,
}

pub const fn successful_tool_post_call_hooks() -> [RuntimeHookKind; 2] {
    [
        RuntimeHookKind::DeepReviewSharedContextToolUse,
        RuntimeHookKind::BehaviorGuard,
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RuntimeHookErrorPolicy {
    FailTurn,
    SkipHook,
    DenyTool,
    RecordWarning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookResult {
    Continue,
    Abort {
        reason: String,
        fix_instruction: String,
        max_retries: u32,
    },
}

impl HookResult {
    pub fn is_abort(&self) -> bool {
        matches!(self, HookResult::Abort { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeHookPlan {
    id: String,
    kind: RuntimeHookKind,
    order: u16,
    timeout_millis: u64,
    error_policy: RuntimeHookErrorPolicy,
}

impl RuntimeHookPlan {
    pub fn new(id: impl Into<String>, kind: RuntimeHookKind) -> Self {
        Self {
            id: id.into(),
            kind,
            order: 100,
            timeout_millis: 1_000,
            error_policy: RuntimeHookErrorPolicy::RecordWarning,
        }
    }

    pub fn with_order(mut self, order: u16) -> Self {
        self.order = order;
        self
    }

    pub fn with_timeout_millis(mut self, timeout_millis: u64) -> Self {
        self.timeout_millis = timeout_millis;
        self
    }

    pub fn with_error_policy(mut self, error_policy: RuntimeHookErrorPolicy) -> Self {
        self.error_policy = error_policy;
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn kind(&self) -> RuntimeHookKind {
        self.kind
    }

    pub const fn order(&self) -> u16 {
        self.order
    }

    pub const fn timeout_millis(&self) -> u64 {
        self.timeout_millis
    }

    pub const fn error_policy(&self) -> RuntimeHookErrorPolicy {
        self.error_policy
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeHookRegistryBuildError {
    #[error("runtime hook id must not be empty")]
    EmptyHookId,
    #[error("runtime hook {hook_id} must declare a non-zero timeout")]
    InvalidTimeoutMillis { hook_id: String },
    #[error("duplicate runtime hook id {hook_id}")]
    DuplicateHookId { hook_id: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeHookRegistryBuilder {
    hooks: Vec<RuntimeHookPlan>,
}

impl RuntimeHookRegistryBuilder {
    pub fn register(mut self, hook: RuntimeHookPlan) -> Self {
        self.hooks.push(hook);
        self
    }

    pub fn build(mut self) -> Result<RuntimeHookRegistry, RuntimeHookRegistryBuildError> {
        let mut hook_ids = HashSet::new();
        for hook in &self.hooks {
            if hook.id.trim().is_empty() {
                return Err(RuntimeHookRegistryBuildError::EmptyHookId);
            }
            if hook.timeout_millis == 0 {
                return Err(RuntimeHookRegistryBuildError::InvalidTimeoutMillis {
                    hook_id: hook.id.clone(),
                });
            }
            if !hook_ids.insert(hook.id.clone()) {
                return Err(RuntimeHookRegistryBuildError::DuplicateHookId {
                    hook_id: hook.id.clone(),
                });
            }
        }
        self.hooks.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(RuntimeHookRegistry { hooks: self.hooks })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeHookRegistry {
    hooks: Vec<RuntimeHookPlan>,
}

impl RuntimeHookRegistry {
    pub fn builder() -> RuntimeHookRegistryBuilder {
        RuntimeHookRegistryBuilder::default()
    }

    pub fn hooks(&self) -> &[RuntimeHookPlan] {
        &self.hooks
    }
}

pub trait SuccessfulToolPostCallHookExecutor<C> {
    fn record_deep_review_shared_context_tool_use(
        &mut self,
        tool_name: &str,
        input: &Value,
        context: &C,
    );

    fn behavior_guard(
        &mut self,
        _tool_name: &str,
        _input: &Value,
        _context: &C,
    ) -> HookResult {
        HookResult::Continue
    }
}

pub fn run_successful_tool_post_call_hooks<C, E>(
    tool_name: &str,
    input: &Value,
    context: &C,
    executor: &mut E,
) -> HookResult
where
    E: SuccessfulToolPostCallHookExecutor<C>,
{
    for hook in successful_tool_post_call_hooks() {
        match hook {
            RuntimeHookKind::DeepReviewSharedContextToolUse => {
                executor.record_deep_review_shared_context_tool_use(tool_name, input, context);
            }
            RuntimeHookKind::BehaviorGuard => {
                let result = executor.behavior_guard(tool_name, input, context);
                if result.is_abort() {
                    return result;
                }
            }
            RuntimeHookKind::SuccessfulToolPostCall => {}
            RuntimeHookKind::Stop => {
                // Stop hooks are handled by run_stop_hooks() at the round level,
                // not by the per-tool post-call dispatch.
            }
        }
    }
    HookResult::Continue
}

// ── Stop (round-level) hooks ────────────────────────────────────

/// Summary of a single tool call within a round, for Stop hook inspection.
#[derive(Debug, Clone)]
pub struct ToolCallSummary {
    pub tool_name: String,
    pub is_error: bool,
}

/// Context passed to Stop hooks after each dialog round completes.
#[derive(Debug, Clone)]
pub struct StopHookContext {
    pub session_id: String,
    pub turn_id: String,
    pub round_index: u32,
    pub tool_calls: Vec<ToolCallSummary>,
    pub assistant_text_summary: String,
    pub file_reads: Vec<String>,
    pub file_edits: Vec<String>,
    pub round_has_more: bool,
}

/// Executor for Stop (round-level) hooks.
///
/// Implementations provide two handlers that mirror the B01/C01 dual-bee
/// pattern from LionBuddy V10:
///
/// - `context_guard` (B01 提示蜂): checks whether the agent had sufficient
///   context to make good decisions. May inject supplemental knowledge.
/// - `behavior_guard` (C01 审查蜂): checks whether the agent violated any
///   iron rules during the round.
pub trait StopHookExecutor {
    /// B01 提示蜂 — context completeness check.
    fn context_guard(&mut self, _ctx: &StopHookContext) -> HookResult {
        HookResult::Continue
    }

    /// C01 审查蜂 — iron-rule violation check.
    fn behavior_guard(&mut self, _ctx: &StopHookContext) -> HookResult {
        HookResult::Continue
    }
}

/// Aggregate result from running all Stop hook handlers.
///
/// Collects the first Abort (if any) and all additional context strings.
#[derive(Debug, Clone, Default)]
pub struct StopHookAggregatedResult {
    pub abort: Option<HookResult>,
    pub additional_contexts: Vec<String>,
}

impl StopHookAggregatedResult {
    pub fn is_abort(&self) -> bool {
        self.abort.as_ref().is_some_and(|r| r.is_abort())
    }
}

/// Run B01 context_guard followed by C01 behavior_guard for a round.
///
/// Returns the aggregated result. If behavior_guard returns Abort, the
/// caller should inject the abort message into the next round.
pub fn run_stop_hooks<E: StopHookExecutor>(
    ctx: &StopHookContext,
    executor: &mut E,
) -> StopHookAggregatedResult {
    let mut aggregated = StopHookAggregatedResult::default();

    // B01 提示蜂: context completeness check (informational, non-blocking)
    match executor.context_guard(ctx) {
        HookResult::Continue => {}
        HookResult::Abort {
            reason,
            fix_instruction,
            ..
        } => {
            aggregated.additional_contexts.push(format!(
                "[B01 提示蜂] 上下文不足: {reason} — {fix_instruction}"
            ));
        }
    }

    // C01 审查蜂: iron-rule violation check (提醒模式，不拦截)
    let c01 = executor.behavior_guard(ctx);
    if c01.is_abort() {
        if let HookResult::Abort {
            reason,
            fix_instruction,
            ..
        } = c01
        {
            aggregated.additional_contexts.push(format!(
                "[C01 审查蜂] 提醒: {reason} — {fix_instruction}"
            ));
        }
    }

    aggregated
}

#[derive(Debug, Clone, Copy)]
pub struct DeepReviewSharedContextToolUseFacts<'a> {
    pub tool_name: &'a str,
    pub input: &'a Value,
    pub custom_data: &'a HashMap<String, Value>,
    pub workspace_root: Option<&'a Path>,
    pub is_remote: bool,
    pub agent_type: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepReviewSharedContextToolUseRecord {
    pub parent_turn_id: String,
    pub subagent_type: String,
    pub tool_name: String,
    pub measured_path: String,
}

pub fn resolve_deep_review_shared_context_tool_use(
    facts: DeepReviewSharedContextToolUseFacts<'_>,
) -> Option<DeepReviewSharedContextToolUseRecord> {
    if !facts.tool_name.eq_ignore_ascii_case("Read")
        && !facts.tool_name.eq_ignore_ascii_case("GetFileDiff")
    {
        return None;
    }
    if !custom_data_str(facts.custom_data, "deep_review_subagent_role")
        .is_some_and(|role| role.eq_ignore_ascii_case("reviewer"))
    {
        return None;
    }
    let parent_turn_id = custom_data_str(facts.custom_data, "deep_review_parent_dialog_turn_id")?;
    let file_path = facts
        .input
        .get("file_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if is_bitfun_runtime_uri(file_path) {
        return None;
    }

    let measured_path = if facts.is_remote {
        None
    } else {
        facts
            .workspace_root
            .and_then(|workspace_root| git_relative_path(workspace_root, file_path))
    }
    .unwrap_or_else(|| file_path.to_string());
    let subagent_type = custom_data_str(facts.custom_data, "deep_review_subagent_type")
        .or(facts.agent_type)
        .unwrap_or("unknown");

    Some(DeepReviewSharedContextToolUseRecord {
        parent_turn_id: parent_turn_id.to_string(),
        subagent_type: subagent_type.to_string(),
        tool_name: facts.tool_name.to_string(),
        measured_path,
    })
}

fn custom_data_str<'a>(custom_data: &'a HashMap<String, Value>, key: &str) -> Option<&'a str> {
    custom_data
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn git_relative_path(workspace_root: &Path, path: &str) -> Option<String> {
    let path = Path::new(path);
    let relative = if path.is_absolute() {
        path.strip_prefix(workspace_root).ok()?
    } else {
        path.strip_prefix(workspace_root).unwrap_or(path)
    };

    Some(relative.to_string_lossy().replace('\\', "/"))
}

fn is_bitfun_runtime_uri(path: &str) -> bool {
    path.trim().starts_with("bitfun://runtime/")
}
