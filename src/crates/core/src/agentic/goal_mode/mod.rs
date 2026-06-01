//! Persisted session thread goals (Codex `/goal` parity).
//!
//! Users set objectives via `/goal <objective>`; the agent manages lifecycle through
//! `create_goal`, `update_goal`, and `get_goal` tools. Runtime auto-continues active
//! goals after idle turns using internal continuation prompts.

mod token_subscriber;

pub use token_subscriber::ThreadGoalTokenSubscriber;

use crate::agentic::core::{InternalReminderKind, Message};
use crate::agentic::session::SessionManager;
use crate::util::errors::{BitFunError, BitFunResult};
pub use bitfun_runtime_ports::{
    validate_thread_goal_objective, SetThreadGoalResult, ThreadGoal, ThreadGoalContinuationPlan,
    ThreadGoalStatus, ThreadGoalToolResponse, GOAL_MODE_METADATA_KEY, MAX_CONTEXT_SUMMARY_CHARS,
    MAX_GOAL_CONTINUATIONS, MAX_THREAD_GOAL_AUTO_CONTINUATIONS, MAX_THREAD_GOAL_OBJECTIVE_CHARS,
    THREAD_GOAL_METADATA_KEY,
};
use log::{info, warn};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use uuid::Uuid;

const CONTINUATION_PROMPT_TEMPLATE: &str = include_str!("templates/continuation.md");
const BUDGET_LIMIT_PROMPT_TEMPLATE: &str = include_str!("templates/budget_limit.md");
const OBJECTIVE_UPDATED_PROMPT_TEMPLATE: &str = include_str!("templates/objective_updated.md");

/// Backoff when scheduling a goal continuation dialog turn fails (network, queue, etc.).
pub const GOAL_CONTINUATION_SUBMIT_RETRY_BASE_DELAY_MS: u64 = 1_000;
pub const GOAL_CONTINUATION_SUBMIT_RETRY_MAX_DELAY_MS: u64 = 30_000;

/// Delay before retrying a failed goal-continuation submit (1s, 2s, 4s, … capped at 30s).
pub fn goal_continuation_submit_retry_delay_ms(retry_count: u32) -> u64 {
    if retry_count == 0 {
        return 0;
    }
    GOAL_CONTINUATION_SUBMIT_RETRY_BASE_DELAY_MS
        .saturating_mul(1_u64 << retry_count.saturating_sub(1).min(5))
        .min(GOAL_CONTINUATION_SUBMIT_RETRY_MAX_DELAY_MS)
}

/// In active thread-goal mode, subagents start without an active timeout.
pub fn effective_subagent_timeout_seconds(
    timeout_seconds: Option<u64>,
    parent_thread_goal_active: bool,
) -> Option<u64> {
    if parent_thread_goal_active {
        None
    } else {
        timeout_seconds.filter(|seconds| *seconds > 0)
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub fn now_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

pub fn thread_goal_patch(goal: &ThreadGoal) -> serde_json::Value {
    serde_json::json!({
        THREAD_GOAL_METADATA_KEY: goal,
    })
}

pub fn clear_thread_goal_patch() -> serde_json::Value {
    serde_json::json!({
        THREAD_GOAL_METADATA_KEY: serde_json::Value::Null,
    })
}

pub fn thread_goal_from_custom_metadata(
    custom_metadata: Option<&serde_json::Value>,
) -> Option<ThreadGoal> {
    if let Some(goal) = custom_metadata
        .and_then(|value| value.get(THREAD_GOAL_METADATA_KEY))
        .and_then(|value| serde_json::from_value::<ThreadGoal>(value.clone()).ok())
    {
        return Some(goal);
    }
    migrate_legacy_goal_mode(custom_metadata)
}

fn migrate_legacy_goal_mode(custom_metadata: Option<&serde_json::Value>) -> Option<ThreadGoal> {
    let legacy = custom_metadata?.get(GOAL_MODE_METADATA_KEY)?;
    let active = legacy.get("active")?.as_bool().unwrap_or(false);
    if !active {
        return None;
    }
    let objective = legacy
        .get("initialGoal")
        .and_then(|value| value.get("goalText"))
        .and_then(|value| value.as_str())
        .or_else(|| legacy.get("goalText").and_then(|value| value.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let session_id = legacy
        .get("sessionId")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let created_at = legacy
        .get("activatedAtMs")
        .and_then(|value| value.as_u64())
        .map(|value| value as i64)
        .unwrap_or_else(now_epoch_seconds);
    Some(ThreadGoal {
        goal_id: Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        objective: objective.to_string(),
        status: ThreadGoalStatus::Active,
        token_budget: None,
        tokens_used: 0,
        time_used_seconds: 0,
        created_at,
        updated_at: created_at,
        auto_continuation_count: 0,
    })
}

/// Skip marking turn start / token accounting for turns that are not goal-driving work.
pub fn should_skip_goal_for_turn(
    user_input: &str,
    user_message_metadata: Option<&serde_json::Value>,
) -> bool {
    if should_skip_goal_turn_accounting(user_input, user_message_metadata) {
        return true;
    }
    if user_message_metadata
        .and_then(|metadata| metadata.get("threadGoalObjectiveUpdated"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        return true;
    }
    false
}

/// Inputs that must not trigger another auto-continuation after the turn ends.
pub fn should_skip_goal_continuation_after_turn(
    user_input: &str,
    user_message_metadata: Option<&serde_json::Value>,
) -> bool {
    should_skip_goal_turn_accounting(user_input, user_message_metadata)
}

fn should_skip_goal_turn_accounting(
    user_input: &str,
    user_message_metadata: Option<&serde_json::Value>,
) -> bool {
    let trimmed = user_input.trim();
    if trimmed.eq_ignore_ascii_case("/compact")
        || trimmed.starts_with("/usage")
        || trimmed.starts_with("/btw")
        || trimmed.starts_with("/goal")
    {
        return true;
    }
    if user_message_metadata
        .and_then(|metadata| metadata.get("maintenanceTurn"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        return true;
    }
    false
}

fn escape_xml_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn render_template(template: &str, replacements: &[(&str, &str)]) -> String {
    let mut rendered = template.to_string();
    for (key, value) in replacements {
        rendered = rendered.replace(&format!("{{{{ {key} }}}}"), value);
    }
    rendered
}

pub fn continuation_prompt(goal: &ThreadGoal) -> String {
    let token_budget = goal
        .token_budget
        .map(|budget| budget.to_string())
        .unwrap_or_else(|| "none".to_string());
    let remaining_tokens = goal
        .remaining_tokens()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unbounded".to_string());
    render_template(
        CONTINUATION_PROMPT_TEMPLATE,
        &[
            ("objective", &escape_xml_text(goal.objective.trim())),
            ("tokens_used", &goal.tokens_used.to_string()),
            ("token_budget", token_budget.as_str()),
            ("remaining_tokens", remaining_tokens.as_str()),
        ],
    )
}

pub fn budget_limit_prompt(goal: &ThreadGoal) -> String {
    let token_budget = goal
        .token_budget
        .map(|budget| budget.to_string())
        .unwrap_or_else(|| "none".to_string());
    render_template(
        BUDGET_LIMIT_PROMPT_TEMPLATE,
        &[
            ("objective", &escape_xml_text(goal.objective.trim())),
            ("tokens_used", &goal.tokens_used.to_string()),
            ("time_used_seconds", &goal.time_used_seconds.to_string()),
            ("token_budget", token_budget.as_str()),
        ],
    )
}

/// Codex-style billable tokens: non-cached input + output.
pub fn billable_tokens_from_counts(
    input_tokens: usize,
    cached_tokens: usize,
    output_tokens: usize,
) -> usize {
    input_tokens.saturating_sub(cached_tokens) + output_tokens
}

pub fn is_usage_limit_error(error: &BitFunError) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    [
        "usage limit",
        "usage_limit",
        "insufficient_quota",
        "insufficient quota",
        "quota exceeded",
        "rate limit",
        "rate_limit",
        "billing",
        "exceeded your current",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

pub fn objective_updated_prompt(goal: &ThreadGoal) -> String {
    let token_budget = goal
        .token_budget
        .map(|budget| budget.to_string())
        .unwrap_or_else(|| "none".to_string());
    let remaining_tokens = goal
        .remaining_tokens()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unbounded".to_string());
    render_template(
        OBJECTIVE_UPDATED_PROMPT_TEMPLATE,
        &[
            ("objective", &escape_xml_text(goal.objective.trim())),
            ("tokens_used", &goal.tokens_used.to_string()),
            ("token_budget", token_budget.as_str()),
            ("remaining_tokens", remaining_tokens.as_str()),
        ],
    )
}

pub fn goal_internal_context_message(prompt: String) -> Message {
    Message::internal_reminder(InternalReminderKind::GoalContinuation, prompt)
}

pub fn goal_objective_updated_message(prompt: String) -> Message {
    Message::internal_reminder(InternalReminderKind::GoalObjectiveUpdated, prompt)
}

pub fn build_objective_updated_plan(goal: &ThreadGoal) -> ThreadGoalContinuationPlan {
    ThreadGoalContinuationPlan {
        prepended_reminders: vec![objective_updated_prompt(goal)],
        display_message: format!("Thread goal updated: {}", goal.objective.trim()),
        user_message_metadata: serde_json::json!({
            "threadGoalObjectiveUpdated": true,
            "goalId": goal.goal_id,
            "objective": goal.objective,
        }),
    }
}

pub fn completion_budget_report(goal: &ThreadGoal) -> Option<String> {
    if goal.token_budget.is_none() && goal.time_used_seconds <= 0 {
        return None;
    }
    Some(
        "Goal achieved. Report final usage from this tool result's structured goal fields. If `goal.tokenBudget` is present, include token usage from `goal.tokensUsed` and `goal.tokenBudget`. If `goal.timeUsedSeconds` is greater than 0, summarize elapsed time in a concise, human-friendly form appropriate to the response language."
            .to_string(),
    )
}

pub fn goal_tool_response(
    goal: Option<ThreadGoal>,
    include_completion_report: bool,
) -> ThreadGoalToolResponse {
    let remaining_tokens = goal.as_ref().and_then(ThreadGoal::remaining_tokens);
    let completion_budget_report = if include_completion_report {
        goal.as_ref()
            .filter(|goal| goal.status == ThreadGoalStatus::Complete)
            .and_then(completion_budget_report)
    } else {
        None
    };
    ThreadGoalToolResponse {
        goal,
        remaining_tokens,
        completion_budget_report,
    }
}

#[derive(Debug)]
struct GoalTurnAccounting {
    turn_id: String,
    baseline_tokens: usize,
    /// Cumulative Codex-style billable tokens for the active dialog turn.
    cumulative_billable: usize,
    active_goal_id: Option<String>,
}

#[derive(Debug)]
struct GoalWallClockAccounting {
    last_accounted_at: Instant,
    active_goal_id: Option<String>,
}

#[derive(Debug, Default)]
pub struct ThreadGoalRuntime {
    accounting: Mutex<GoalRuntimeAccounting>,
    budget_limit_reported_goal_id: Mutex<Option<String>>,
}

#[derive(Debug, Default)]
struct GoalRuntimeAccounting {
    turn: Option<GoalTurnAccounting>,
    wall_clock: GoalWallClockAccounting,
}

impl Default for GoalWallClockAccounting {
    fn default() -> Self {
        Self {
            last_accounted_at: Instant::now(),
            active_goal_id: None,
        }
    }
}

impl ThreadGoalRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn mark_turn_started(&self, turn_id: &str, goal: Option<&ThreadGoal>) {
        let mut accounting = self.accounting.lock().await;
        accounting.turn = Some(GoalTurnAccounting {
            turn_id: turn_id.to_string(),
            baseline_tokens: 0,
            cumulative_billable: 0,
            active_goal_id: None,
        });
        if let Some(goal) = goal.filter(|goal| goal.is_active()) {
            if let Some(turn) = accounting.turn.as_mut() {
                turn.active_goal_id = Some(goal.goal_id.clone());
            }
            accounting.wall_clock.mark_active_goal(goal.goal_id.clone());
        } else {
            accounting.wall_clock.clear_active_goal();
        }
    }

    pub async fn record_round_billable_tokens(&self, turn_id: &str, billable: usize) {
        if billable == 0 {
            return;
        }
        let mut accounting = self.accounting.lock().await;
        let Some(turn) = accounting.turn.as_mut() else {
            return;
        };
        if turn.turn_id != turn_id || turn.active_goal_id.is_none() {
            return;
        }
        turn.cumulative_billable = turn.cumulative_billable.saturating_add(billable);
    }

    pub async fn turn_cumulative_billable_tokens(&self, turn_id: &str) -> usize {
        let accounting = self.accounting.lock().await;
        accounting
            .turn
            .as_ref()
            .filter(|turn| turn.turn_id == turn_id)
            .map(|turn| turn.cumulative_billable)
            .unwrap_or(0)
    }

    pub async fn clear_active_goal(&self, turn_id: Option<&str>) {
        let mut accounting = self.accounting.lock().await;
        if let Some(turn_id) = turn_id {
            if let Some(turn) = accounting.turn.as_mut() {
                if turn.turn_id == turn_id {
                    turn.active_goal_id = None;
                }
            }
        }
        accounting.wall_clock.clear_active_goal();
    }

    pub async fn account_turn_tokens(
        &self,
        turn_id: &str,
        turn_tokens: usize,
        goal: &mut ThreadGoal,
    ) -> bool {
        let mut accounting = self.accounting.lock().await;
        let Some(turn) = accounting.turn.as_mut() else {
            return false;
        };
        if turn.turn_id != turn_id || turn.active_goal_id.as_deref() != Some(goal.goal_id.as_str())
        {
            return false;
        }
        let delta = turn_tokens.saturating_sub(turn.baseline_tokens) as i64;
        turn.baseline_tokens = turn_tokens;
        let time_delta = accounting.wall_clock.time_delta_seconds();
        if delta <= 0 && time_delta <= 0 {
            return false;
        }
        goal.tokens_used = goal.tokens_used.saturating_add(delta.max(0));
        goal.time_used_seconds = goal.time_used_seconds.saturating_add(time_delta);
        goal.updated_at = now_epoch_seconds();
        accounting.wall_clock.mark_accounted(time_delta);
        Self::apply_budget_status(goal)
    }

    fn apply_budget_status(goal: &mut ThreadGoal) -> bool {
        if let Some(budget) = goal.token_budget {
            if goal.tokens_used >= budget && goal.status == ThreadGoalStatus::Active {
                goal.status = ThreadGoalStatus::BudgetLimited;
                return true;
            }
        }
        false
    }
}

impl GoalWallClockAccounting {
    fn time_delta_seconds(&self) -> i64 {
        i64::try_from(self.last_accounted_at.elapsed().as_secs()).unwrap_or(i64::MAX)
    }

    fn mark_accounted(&mut self, accounted_seconds: i64) {
        if accounted_seconds <= 0 {
            return;
        }
        let advance =
            std::time::Duration::from_secs(u64::try_from(accounted_seconds).unwrap_or(u64::MAX));
        self.last_accounted_at = self
            .last_accounted_at
            .checked_add(advance)
            .unwrap_or_else(Instant::now);
    }

    fn mark_active_goal(&mut self, goal_id: String) {
        if self.active_goal_id.as_deref() != Some(goal_id.as_str()) {
            self.last_accounted_at = Instant::now();
            self.active_goal_id = Some(goal_id);
        }
    }

    fn clear_active_goal(&mut self) {
        self.active_goal_id = None;
        self.last_accounted_at = Instant::now();
    }
}

pub struct ThreadGoalStore<'a> {
    session_manager: &'a SessionManager,
}

impl<'a> ThreadGoalStore<'a> {
    pub fn new(session_manager: &'a SessionManager) -> Self {
        Self { session_manager }
    }

    async fn load_metadata(
        &self,
        session_id: &str,
        workspace_path: &Path,
    ) -> BitFunResult<Option<serde_json::Value>> {
        Ok(self
            .session_manager
            .load_session_metadata(workspace_path, session_id)
            .await?
            .and_then(|metadata| metadata.custom_metadata))
    }

    pub async fn get_thread_goal(
        &self,
        session_id: &str,
        workspace_path: &Path,
    ) -> BitFunResult<Option<ThreadGoal>> {
        let metadata = self.load_metadata(session_id, workspace_path).await?;
        Ok(thread_goal_from_custom_metadata(metadata.as_ref()))
    }

    async fn persist_thread_goal(
        &self,
        session_id: &str,
        _workspace_path: &Path,
        goal: Option<ThreadGoal>,
    ) -> BitFunResult<()> {
        let patch = match goal {
            Some(goal) => thread_goal_patch(&goal),
            None => clear_thread_goal_patch(),
        };
        self.session_manager
            .merge_session_custom_metadata(session_id, patch)
            .await
    }

    pub async fn clear_thread_goal(
        &self,
        session_id: &str,
        workspace_path: &Path,
    ) -> BitFunResult<()> {
        self.persist_thread_goal(session_id, workspace_path, None)
            .await
    }

    pub async fn set_thread_goal(
        &self,
        session_id: &str,
        workspace_path: &Path,
        objective: Option<String>,
        status: Option<ThreadGoalStatus>,
        token_budget: Option<Option<i64>>,
        replace_existing: bool,
    ) -> BitFunResult<SetThreadGoalResult> {
        if let Some(budget) = token_budget.flatten() {
            if budget <= 0 {
                return Err(BitFunError::Validation(
                    "goal budgets must be positive when provided".to_string(),
                ));
            }
        }

        let objective = objective.map(|value| value.trim().to_string());
        if let Some(objective) = objective.as_deref() {
            validate_thread_goal_objective(objective).map_err(BitFunError::Validation)?;
        }

        let existing = self.get_thread_goal(session_id, workspace_path).await?;
        let replaced_existing = replace_existing || objective.is_some() && existing.is_some();

        if replace_existing {
            self.clear_thread_goal(session_id, workspace_path).await?;
        }

        let goal = if let Some(objective) = objective {
            if let Some(mut existing) = self.get_thread_goal(session_id, workspace_path).await? {
                let objective_changed = existing.objective != objective;
                existing.objective = objective;
                if objective_changed {
                    existing.auto_continuation_count = 0;
                }
                if let Some(status) = status {
                    existing.status = status;
                }
                if let Some(token_budget) = token_budget {
                    existing.token_budget = token_budget;
                }
                existing.updated_at = now_epoch_seconds();
                existing
            } else {
                let now = now_epoch_seconds();
                ThreadGoal {
                    goal_id: Uuid::new_v4().to_string(),
                    session_id: session_id.to_string(),
                    objective,
                    status: status.unwrap_or(ThreadGoalStatus::Active),
                    token_budget: token_budget.flatten(),
                    tokens_used: 0,
                    time_used_seconds: 0,
                    created_at: now,
                    updated_at: now,
                    auto_continuation_count: 0,
                }
            }
        } else {
            let Some(mut existing) = self.get_thread_goal(session_id, workspace_path).await? else {
                return Err(BitFunError::NotFound(format!(
                    "cannot update goal for session {session_id}: no goal exists"
                )));
            };
            if let Some(status) = status {
                existing.status = status;
            }
            if let Some(token_budget) = token_budget {
                existing.token_budget = token_budget;
            }
            existing.updated_at = now_epoch_seconds();
            existing
        };

        self.persist_thread_goal(session_id, workspace_path, Some(goal.clone()))
            .await?;

        Ok(SetThreadGoalResult {
            goal,
            replaced_existing,
        })
    }

    pub async fn create_thread_goal(
        &self,
        session_id: &str,
        workspace_path: &Path,
        objective: String,
        token_budget: Option<i64>,
    ) -> BitFunResult<ThreadGoal> {
        if self
            .get_thread_goal(session_id, workspace_path)
            .await?
            .is_some()
        {
            return Err(BitFunError::Validation(format!(
                "cannot create a new goal because session {session_id} already has a goal"
            )));
        }
        let result = self
            .set_thread_goal(
                session_id,
                workspace_path,
                Some(objective),
                Some(ThreadGoalStatus::Active),
                Some(token_budget),
                false,
            )
            .await?;
        Ok(result.goal)
    }
}

/// Statuses where the user may explicitly resume auto-continuation toward the goal.
pub fn thread_goal_status_is_resumable(status: ThreadGoalStatus) -> bool {
    matches!(
        status,
        ThreadGoalStatus::Paused | ThreadGoalStatus::Blocked | ThreadGoalStatus::UsageLimited
    )
}

pub fn build_thread_goal_continuation_plan(goal: &ThreadGoal) -> ThreadGoalContinuationPlan {
    let prompt = match goal.status {
        ThreadGoalStatus::BudgetLimited => budget_limit_prompt(goal),
        ThreadGoalStatus::Active => continuation_prompt(goal),
        _ => continuation_prompt(goal),
    };
    ThreadGoalContinuationPlan {
        prepended_reminders: vec![prompt],
        display_message: format!(
            "Thread goal completion check (auto {}/{}): {}",
            goal.auto_continuation_count,
            MAX_THREAD_GOAL_AUTO_CONTINUATIONS,
            goal.objective.trim()
        ),
        user_message_metadata: serde_json::json!({
            "threadGoalContinuation": true,
            "threadGoalContinuationCheck": true,
            "goalId": goal.goal_id,
            "objective": goal.objective,
            "autoContinuationAttempt": goal.auto_continuation_count,
            "autoContinuationMax": MAX_THREAD_GOAL_AUTO_CONTINUATIONS,
        }),
    }
}

pub async fn maybe_build_continuation_after_turn(
    store: &ThreadGoalStore<'_>,
    runtime: &ThreadGoalRuntime,
    session_id: &str,
    workspace_path: &Path,
    turn_id: &str,
    turn_tokens: usize,
    turn_completed: bool,
) -> BitFunResult<Option<ThreadGoalContinuationPlan>> {
    let Some(mut goal) = store.get_thread_goal(session_id, workspace_path).await? else {
        return Ok(None);
    };

    if goal.auto_continuation_count >= MAX_THREAD_GOAL_AUTO_CONTINUATIONS {
        if goal.status == ThreadGoalStatus::Active {
            warn!(
                "Thread goal auto-continuation limit reached; marking blocked: session_id={}, goal_id={}, objective={}",
                session_id, goal.goal_id, goal.objective
            );
            goal.status = ThreadGoalStatus::Blocked;
            goal.updated_at = now_epoch_seconds();
            store
                .persist_thread_goal(session_id, workspace_path, Some(goal))
                .await?;
        }
        return Ok(None);
    }

    if !turn_completed {
        return Ok(None);
    }

    let became_budget_limited = runtime
        .account_turn_tokens(turn_id, turn_tokens, &mut goal)
        .await;
    store
        .persist_thread_goal(session_id, workspace_path, Some(goal.clone()))
        .await?;
    if became_budget_limited {
        let reported = runtime.budget_limit_reported_goal_id.lock().await;
        if reported.as_deref() == Some(goal.goal_id.as_str()) {
            return Ok(None);
        }
        drop(reported);
        *runtime.budget_limit_reported_goal_id.lock().await = Some(goal.goal_id.clone());
        return Ok(Some(build_thread_goal_continuation_plan(&goal)));
    }

    if !goal.is_active() {
        return Ok(None);
    }

    goal.auto_continuation_count = goal.auto_continuation_count.saturating_add(1);
    goal.updated_at = now_epoch_seconds();
    store
        .persist_thread_goal(session_id, workspace_path, Some(goal.clone()))
        .await?;
    info!(
        "Scheduling thread goal auto-continuation: session_id={}, attempt={}/{}, objective={}",
        session_id,
        goal.auto_continuation_count,
        MAX_THREAD_GOAL_AUTO_CONTINUATIONS,
        goal.objective
    );

    Ok(Some(build_thread_goal_continuation_plan(&goal)))
}

pub fn user_facing_thread_goal_error(error: BitFunError) -> BitFunError {
    match error {
        BitFunError::Validation(_) | BitFunError::NotFound(_) => error,
        other => {
            warn!("Thread goal operation failed: {other}");
            BitFunError::Validation(
                "Thread goal operation failed. Check session state and try again.".to_string(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resumable_statuses_match_ui_actions() {
        assert!(thread_goal_status_is_resumable(ThreadGoalStatus::Paused));
        assert!(thread_goal_status_is_resumable(ThreadGoalStatus::Blocked));
        assert!(thread_goal_status_is_resumable(
            ThreadGoalStatus::UsageLimited
        ));
        assert!(!thread_goal_status_is_resumable(ThreadGoalStatus::Active));
        assert!(!thread_goal_status_is_resumable(
            ThreadGoalStatus::BudgetLimited
        ));
        assert!(!thread_goal_status_is_resumable(ThreadGoalStatus::Complete));
    }

    #[test]
    fn continuation_plan_metadata_marks_completion_check() {
        let plan = build_thread_goal_continuation_plan(&ThreadGoal {
            goal_id: "g1".to_string(),
            session_id: "s1".to_string(),
            objective: "sync upstream".to_string(),
            status: ThreadGoalStatus::Active,
            token_budget: None,
            tokens_used: 0,
            time_used_seconds: 0,
            created_at: 1,
            updated_at: 2,
            auto_continuation_count: 2,
        });
        assert!(plan.display_message.contains("completion check"));
        assert!(plan.display_message.contains("2/100"));
        assert_eq!(
            plan.user_message_metadata["threadGoalContinuationCheck"],
            true
        );
        assert_eq!(plan.user_message_metadata["autoContinuationAttempt"], 2);
        assert_eq!(plan.user_message_metadata["autoContinuationMax"], 100);
    }

    #[test]
    fn continuation_prompt_references_update_goal() {
        let prompt = continuation_prompt(&ThreadGoal {
            goal_id: "g1".to_string(),
            session_id: "s1".to_string(),
            objective: "finish stack".to_string(),
            status: ThreadGoalStatus::Active,
            token_budget: Some(10_000),
            tokens_used: 1_000,
            time_used_seconds: 0,
            created_at: 1,
            updated_at: 2,
            auto_continuation_count: 0,
        });
        assert!(prompt.contains("finish stack"));
        assert!(prompt.contains("update_goal"));
    }

    #[test]
    fn should_skip_goal_for_turn_ignores_goal_slash_commands() {
        assert!(should_skip_goal_for_turn("/goal fix bug", None));
        assert!(!should_skip_goal_for_turn("fix bug", None));
    }

    #[test]
    fn should_skip_goal_for_turn_ignores_objective_updated_followup() {
        let metadata = serde_json::json!({ "threadGoalObjectiveUpdated": true });
        assert!(should_skip_goal_for_turn("Adjust work", Some(&metadata)));
    }

    #[test]
    fn objective_updated_turn_still_schedules_continuation_after_turn() {
        let metadata = serde_json::json!({ "threadGoalObjectiveUpdated": true });
        assert!(!should_skip_goal_continuation_after_turn(
            "Adjust work",
            Some(&metadata)
        ));
    }

    #[test]
    fn continuation_turn_chains_until_goal_completes_or_limit() {
        let metadata = serde_json::json!({ "threadGoalContinuation": true });
        assert!(!should_skip_goal_continuation_after_turn(
            "Continue toward goal",
            Some(&metadata)
        ));
    }

    #[test]
    fn max_goal_continuations_matches_legacy_limit() {
        assert_eq!(MAX_GOAL_CONTINUATIONS, 100);
        assert_eq!(MAX_THREAD_GOAL_AUTO_CONTINUATIONS, 100);
    }

    #[test]
    fn goal_continuation_submit_retry_delay_backoff_caps() {
        assert_eq!(goal_continuation_submit_retry_delay_ms(0), 0);
        assert_eq!(goal_continuation_submit_retry_delay_ms(1), 1_000);
        assert_eq!(goal_continuation_submit_retry_delay_ms(2), 2_000);
        assert_eq!(
            goal_continuation_submit_retry_delay_ms(100),
            GOAL_CONTINUATION_SUBMIT_RETRY_MAX_DELAY_MS
        );
    }

    #[test]
    fn billable_tokens_subtracts_cached_input() {
        assert_eq!(billable_tokens_from_counts(1000, 400, 200), 800);
    }

    #[test]
    fn usage_limit_error_detects_quota_messages() {
        assert!(is_usage_limit_error(&BitFunError::AIClient(
            "insufficient_quota: billing hard limit".to_string()
        )));
        assert!(!is_usage_limit_error(&BitFunError::Validation(
            "tool failed".to_string()
        )));
    }

    #[test]
    fn objective_updated_plan_skips_turn_accounting_but_allows_continuation() {
        let metadata = build_objective_updated_plan(&ThreadGoal {
            goal_id: "g1".to_string(),
            session_id: "s1".to_string(),
            objective: "ship feature".to_string(),
            status: ThreadGoalStatus::Active,
            token_budget: None,
            tokens_used: 0,
            time_used_seconds: 0,
            created_at: 1,
            updated_at: 2,
            auto_continuation_count: 0,
        })
        .user_message_metadata;
        assert!(should_skip_goal_for_turn("Adjust work", Some(&metadata)));
        assert!(!should_skip_goal_continuation_after_turn(
            "Adjust work",
            Some(&metadata)
        ));
    }
}
