//! Deep Review reviewer budget, retry admission, and runtime accounting.
//!
//! This tracker is deliberately Deep Review-specific. It combines per-turn
//! reviewer/judge budgets, retry budgets, active reviewer counts, effective
//! concurrency learning, capacity diagnostics, and shared-context measurement.
//! Do not move it wholesale to `subagent_runtime`: only isolated mechanics with
//! no Deep Review policy, report, or diagnostic semantics should become generic.

use super::concurrency_policy::{
    DeepReviewEffectiveConcurrencySnapshot, DeepReviewEffectiveConcurrencyState,
};
use super::diagnostics::DeepReviewRuntimeDiagnostics;
use super::execution_policy::{
    reviewer_agent_type_count, DeepReviewExecutionPolicy, DeepReviewPolicyViolation,
    DeepReviewSubagentRole,
};
use super::queue::DeepReviewCapacityQueueReason;
use super::shared_context::{
    normalize_shared_context_file_path, normalize_shared_context_tool_name,
    shared_context_measurement_snapshot_from_uses, DeepReviewSharedContextKey,
    DeepReviewSharedContextMeasurementSnapshot, DeepReviewSharedContextUseRecord,
};
use dashmap::DashMap;
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const BUDGET_TTL: Duration = Duration::from_secs(60 * 60);
const PRUNE_INTERVAL: Duration = Duration::from_secs(300);

#[derive(Debug)]
struct DeepReviewTurnBudget {
    judge_calls: usize,
    /// Tracks total reviewer calls (across all roles) per turn.
    /// Capped by `max_same_role_instances * reviewer_agent_type_count() +
    /// extra_subagent_ids.len()` so the orchestrator cannot spawn an unbounded
    /// number of same-role instances.
    reviewer_calls: usize,
    reviewer_calls_by_subagent: HashMap<String, usize>,
    retries_used_by_subagent: HashMap<String, usize>,
    active_reviewers: usize,
    active_reviewer_launch_batches: BTreeMap<u64, usize>,
    concurrency_cap_rejections: usize,
    capacity_skips: usize,
    shared_context_uses: HashMap<DeepReviewSharedContextKey, DeepReviewSharedContextUseRecord>,
    effective_concurrency: Option<DeepReviewEffectiveConcurrencyState>,
    runtime_diagnostics: DeepReviewRuntimeDiagnostics,
    created_at: Instant,
    updated_at: Instant,
}

impl DeepReviewTurnBudget {
    fn new(now: Instant) -> Self {
        Self {
            judge_calls: 0,
            reviewer_calls: 0,
            reviewer_calls_by_subagent: HashMap::new(),
            retries_used_by_subagent: HashMap::new(),
            active_reviewers: 0,
            active_reviewer_launch_batches: BTreeMap::new(),
            concurrency_cap_rejections: 0,
            capacity_skips: 0,
            shared_context_uses: HashMap::new(),
            effective_concurrency: None,
            runtime_diagnostics: DeepReviewRuntimeDiagnostics::default(),
            created_at: now,
            updated_at: now,
        }
    }

    fn effective_concurrency_mut(
        &mut self,
        configured_max_parallel_instances: usize,
    ) -> &mut DeepReviewEffectiveConcurrencyState {
        let state = self.effective_concurrency.get_or_insert_with(|| {
            DeepReviewEffectiveConcurrencyState::new(configured_max_parallel_instances)
        });
        state.rebase_configured_max(configured_max_parallel_instances);
        state
    }
}

pub struct DeepReviewActiveReviewerGuard<'a> {
    tracker: &'a DeepReviewBudgetTracker,
    parent_dialog_turn_id: String,
    launch_batch: Option<u64>,
    released: bool,
}

impl Drop for DeepReviewActiveReviewerGuard<'_> {
    fn drop(&mut self) {
        if !self.released {
            self.tracker
                .finish_active_reviewer(&self.parent_dialog_turn_id, self.launch_batch);
            self.released = true;
        }
    }
}

pub struct DeepReviewBudgetTracker {
    turns: DashMap<String, DeepReviewTurnBudget>,
    last_pruned_at: Mutex<Instant>,
}

impl Default for DeepReviewBudgetTracker {
    fn default() -> Self {
        Self {
            turns: DashMap::new(),
            last_pruned_at: Mutex::new(Instant::now()),
        }
    }
}

impl DeepReviewBudgetTracker {
    fn record_reason_count(
        counts: &mut std::collections::BTreeMap<String, usize>,
        reason: DeepReviewCapacityQueueReason,
    ) {
        *counts
            .entry(reason.as_snake_case().to_string())
            .or_insert(0) += 1;
    }

    fn update_runtime_diagnostics(
        &self,
        parent_dialog_turn_id: &str,
        update: impl FnOnce(&mut DeepReviewRuntimeDiagnostics),
    ) {
        if parent_dialog_turn_id.trim().is_empty() {
            return;
        }

        let now = Instant::now();
        if let Ok(last_pruned) = self.last_pruned_at.lock() {
            if now.saturating_duration_since(*last_pruned) >= PRUNE_INTERVAL {
                drop(last_pruned);
                self.prune_stale(now);
            }
        }

        let mut budget = self
            .turns
            .entry(parent_dialog_turn_id.to_string())
            .or_insert_with(|| DeepReviewTurnBudget::new(now));
        update(&mut budget.runtime_diagnostics);
        budget.updated_at = now;
    }

    pub fn record_runtime_queue_wait(&self, parent_dialog_turn_id: &str, queue_elapsed_ms: u64) {
        if queue_elapsed_ms == 0 {
            return;
        }
        self.update_runtime_diagnostics(parent_dialog_turn_id, |diagnostics| {
            diagnostics.queue_wait_count = diagnostics.queue_wait_count.saturating_add(1);
            diagnostics.queue_wait_total_ms = diagnostics
                .queue_wait_total_ms
                .saturating_add(queue_elapsed_ms);
            diagnostics.queue_wait_max_ms = diagnostics.queue_wait_max_ms.max(queue_elapsed_ms);
        });
    }

    pub fn record_runtime_provider_capacity_queue(
        &self,
        parent_dialog_turn_id: &str,
        reason: DeepReviewCapacityQueueReason,
    ) {
        self.update_runtime_diagnostics(parent_dialog_turn_id, |diagnostics| {
            diagnostics.provider_capacity_queue_count =
                diagnostics.provider_capacity_queue_count.saturating_add(1);
            Self::record_reason_count(
                &mut diagnostics.provider_capacity_queue_reason_counts,
                reason,
            );
        });
    }

    pub fn record_runtime_provider_capacity_retry(
        &self,
        parent_dialog_turn_id: &str,
        reason: DeepReviewCapacityQueueReason,
    ) {
        self.update_runtime_diagnostics(parent_dialog_turn_id, |diagnostics| {
            diagnostics.provider_capacity_retry_count =
                diagnostics.provider_capacity_retry_count.saturating_add(1);
            Self::record_reason_count(
                &mut diagnostics.provider_capacity_retry_reason_counts,
                reason,
            );
        });
    }

    pub fn record_runtime_provider_capacity_retry_success(
        &self,
        parent_dialog_turn_id: &str,
        reason: DeepReviewCapacityQueueReason,
    ) {
        self.update_runtime_diagnostics(parent_dialog_turn_id, |diagnostics| {
            diagnostics.provider_capacity_retry_success_count = diagnostics
                .provider_capacity_retry_success_count
                .saturating_add(1);
            Self::record_reason_count(
                &mut diagnostics.provider_capacity_retry_success_reason_counts,
                reason,
            );
        });
    }

    pub fn record_runtime_capacity_skip(
        &self,
        parent_dialog_turn_id: &str,
        reason: DeepReviewCapacityQueueReason,
    ) {
        self.update_runtime_diagnostics(parent_dialog_turn_id, |diagnostics| {
            diagnostics.capacity_skip_count = diagnostics.capacity_skip_count.saturating_add(1);
            Self::record_reason_count(&mut diagnostics.capacity_skip_reason_counts, reason);
        });
    }

    pub fn record_runtime_manual_queue_action(&self, parent_dialog_turn_id: &str) {
        self.update_runtime_diagnostics(parent_dialog_turn_id, |diagnostics| {
            diagnostics.manual_queue_action_count =
                diagnostics.manual_queue_action_count.saturating_add(1);
        });
    }

    pub fn record_runtime_manual_retry(&self, parent_dialog_turn_id: &str) {
        self.update_runtime_diagnostics(parent_dialog_turn_id, |diagnostics| {
            diagnostics.manual_retry_count = diagnostics.manual_retry_count.saturating_add(1);
        });
    }

    pub fn record_runtime_auto_retry(&self, parent_dialog_turn_id: &str) {
        self.update_runtime_diagnostics(parent_dialog_turn_id, |diagnostics| {
            diagnostics.auto_retry_count = diagnostics.auto_retry_count.saturating_add(1);
        });
    }

    pub fn record_runtime_auto_retry_suppressed(&self, parent_dialog_turn_id: &str, reason: &str) {
        let reason = reason.trim();
        if reason.is_empty() {
            return;
        }
        self.update_runtime_diagnostics(parent_dialog_turn_id, |diagnostics| {
            *diagnostics
                .auto_retry_suppressed_reason_counts
                .entry(reason.to_string())
                .or_insert(0) += 1;
        });
    }

    pub fn runtime_diagnostics_snapshot(
        &self,
        parent_dialog_turn_id: &str,
    ) -> Option<DeepReviewRuntimeDiagnostics> {
        let budget = self.turns.get(parent_dialog_turn_id)?;
        let mut diagnostics = budget.runtime_diagnostics.clone();
        let shared_context_snapshot =
            shared_context_measurement_snapshot_from_uses(&budget.shared_context_uses);
        diagnostics.merge_shared_context_counts(
            shared_context_snapshot.total_calls,
            shared_context_snapshot.duplicate_calls,
            shared_context_snapshot.duplicate_context_count,
        );
        (!diagnostics.is_empty()).then_some(diagnostics)
    }

    pub fn turn_elapsed_seconds(&self, parent_dialog_turn_id: &str) -> Option<u64> {
        let budget = self.turns.get(parent_dialog_turn_id)?;
        Some(
            Instant::now()
                .saturating_duration_since(budget.created_at)
                .as_secs(),
        )
    }

    pub fn record_shared_context_tool_use(
        &self,
        parent_dialog_turn_id: &str,
        subagent_type: &str,
        tool_name: &str,
        file_path: &str,
    ) -> DeepReviewSharedContextMeasurementSnapshot {
        if parent_dialog_turn_id.trim().is_empty() {
            return DeepReviewSharedContextMeasurementSnapshot::default();
        }
        let Some(tool_name) = normalize_shared_context_tool_name(tool_name) else {
            return self.shared_context_measurement_snapshot(parent_dialog_turn_id);
        };
        let Some(file_path) = normalize_shared_context_file_path(file_path) else {
            return self.shared_context_measurement_snapshot(parent_dialog_turn_id);
        };

        let now = Instant::now();
        if let Ok(last_pruned) = self.last_pruned_at.lock() {
            if now.saturating_duration_since(*last_pruned) >= PRUNE_INTERVAL {
                drop(last_pruned);
                self.prune_stale(now);
            }
        }

        let mut budget = self
            .turns
            .entry(parent_dialog_turn_id.to_string())
            .or_insert_with(|| DeepReviewTurnBudget::new(now));
        let record = budget
            .shared_context_uses
            .entry(DeepReviewSharedContextKey {
                tool_name: tool_name.to_string(),
                file_path,
            })
            .or_default();
        record.call_count = record.call_count.saturating_add(1);
        if !subagent_type.trim().is_empty() {
            record
                .reviewer_types
                .insert(subagent_type.trim().to_string());
        }
        budget.updated_at = now;

        shared_context_measurement_snapshot_from_uses(&budget.shared_context_uses)
    }

    pub fn shared_context_measurement_snapshot(
        &self,
        parent_dialog_turn_id: &str,
    ) -> DeepReviewSharedContextMeasurementSnapshot {
        self.turns
            .get(parent_dialog_turn_id)
            .map(|budget| {
                shared_context_measurement_snapshot_from_uses(&budget.shared_context_uses)
            })
            .unwrap_or_default()
    }

    pub fn record_task(
        &self,
        parent_dialog_turn_id: &str,
        policy: &DeepReviewExecutionPolicy,
        role: DeepReviewSubagentRole,
        subagent_type: &str,
        is_retry: bool,
    ) -> Result<(), DeepReviewPolicyViolation> {
        let now = Instant::now();
        if let Ok(last_pruned) = self.last_pruned_at.lock() {
            if now.saturating_duration_since(*last_pruned) >= PRUNE_INTERVAL {
                drop(last_pruned);
                self.prune_stale(now);
            }
        }

        let mut budget = self
            .turns
            .entry(parent_dialog_turn_id.to_string())
            .or_insert_with(|| DeepReviewTurnBudget::new(now));

        match role {
            DeepReviewSubagentRole::Reviewer => {
                let subagent_type = normalize_budget_subagent_type(subagent_type)?;
                if is_retry {
                    if policy.max_retries_per_role == 0 {
                        return Err(DeepReviewPolicyViolation::new(
                            "deep_review_retry_budget_exhausted",
                            format!(
                                "Retry budget is disabled for DeepReview reviewer '{}'",
                                subagent_type
                            ),
                        ));
                    }
                    if !budget
                        .reviewer_calls_by_subagent
                        .contains_key(subagent_type.as_str())
                    {
                        return Err(DeepReviewPolicyViolation::new(
                            "deep_review_retry_without_initial_attempt",
                            format!(
                                "Cannot retry DeepReview reviewer '{}' before an initial attempt in this turn",
                                subagent_type
                            ),
                        ));
                    }
                    let retry_count = budget
                        .retries_used_by_subagent
                        .entry(subagent_type.clone())
                        .or_insert(0);
                    if *retry_count >= policy.max_retries_per_role {
                        return Err(DeepReviewPolicyViolation::new(
                            "deep_review_retry_budget_exhausted",
                            format!(
                                "Retry budget exhausted for DeepReview reviewer '{}' (max retries: {})",
                                subagent_type, policy.max_retries_per_role
                            ),
                        ));
                    }
                    *retry_count += 1;
                    budget.updated_at = now;
                    return Ok(());
                }

                let max_reviewer_calls = policy.max_same_role_instances
                    * (reviewer_agent_type_count() + policy.extra_subagent_ids.len());
                if budget.reviewer_calls >= max_reviewer_calls {
                    return Err(DeepReviewPolicyViolation::new(
                        "deep_review_reviewer_budget_exhausted",
                        format!(
                            "Reviewer launch budget exhausted for this DeepReview turn (max calls: {})",
                            max_reviewer_calls
                        ),
                    ));
                }
                budget.reviewer_calls += 1;
                *budget
                    .reviewer_calls_by_subagent
                    .entry(subagent_type)
                    .or_insert(0) += 1;
            }
            DeepReviewSubagentRole::Judge => {
                if is_retry {
                    return Err(DeepReviewPolicyViolation::new(
                        "deep_review_judge_retry_disallowed",
                        "ReviewJudge retry is not covered by the reviewer retry budget",
                    ));
                }
                let max_judge_calls = 1;
                if budget.judge_calls >= max_judge_calls {
                    return Err(DeepReviewPolicyViolation::new(
                        "deep_review_judge_budget_exhausted",
                        format!(
                            "ReviewJudge launch budget exhausted for this DeepReview turn (max calls: {})",
                            max_judge_calls
                        ),
                    ));
                }

                budget.judge_calls += 1;
            }
        }

        budget.updated_at = now;
        Ok(())
    }

    pub fn record_concurrency_cap_rejection(&self, parent_dialog_turn_id: &str) {
        if parent_dialog_turn_id.trim().is_empty() {
            return;
        }

        let now = Instant::now();
        if let Ok(last_pruned) = self.last_pruned_at.lock() {
            if now.saturating_duration_since(*last_pruned) >= PRUNE_INTERVAL {
                drop(last_pruned);
                self.prune_stale(now);
            }
        }

        let mut budget = self
            .turns
            .entry(parent_dialog_turn_id.to_string())
            .or_insert_with(|| DeepReviewTurnBudget::new(now));
        budget.concurrency_cap_rejections += 1;
        budget.updated_at = now;
    }

    fn record_capacity_skip_inner(
        &self,
        parent_dialog_turn_id: &str,
        reason: Option<DeepReviewCapacityQueueReason>,
    ) {
        if parent_dialog_turn_id.trim().is_empty() {
            return;
        }

        let now = Instant::now();
        if let Ok(last_pruned) = self.last_pruned_at.lock() {
            if now.saturating_duration_since(*last_pruned) >= PRUNE_INTERVAL {
                drop(last_pruned);
                self.prune_stale(now);
            }
        }

        let mut budget = self
            .turns
            .entry(parent_dialog_turn_id.to_string())
            .or_insert_with(|| DeepReviewTurnBudget::new(now));
        budget.capacity_skips += 1;
        budget.runtime_diagnostics.capacity_skip_count = budget
            .runtime_diagnostics
            .capacity_skip_count
            .saturating_add(1);
        if let Some(reason) = reason {
            Self::record_reason_count(
                &mut budget.runtime_diagnostics.capacity_skip_reason_counts,
                reason,
            );
        }
        budget.updated_at = now;
    }

    pub fn record_capacity_skip(&self, parent_dialog_turn_id: &str) {
        self.record_capacity_skip_inner(parent_dialog_turn_id, None);
    }

    pub fn record_capacity_skip_for_reason(
        &self,
        parent_dialog_turn_id: &str,
        reason: DeepReviewCapacityQueueReason,
    ) {
        self.record_capacity_skip_inner(parent_dialog_turn_id, Some(reason));
    }

    pub fn begin_active_reviewer<'a>(
        &'a self,
        parent_dialog_turn_id: &str,
    ) -> DeepReviewActiveReviewerGuard<'a> {
        let now = Instant::now();
        let mut budget = self
            .turns
            .entry(parent_dialog_turn_id.to_string())
            .or_insert_with(|| DeepReviewTurnBudget::new(now));
        budget.active_reviewers = budget.active_reviewers.saturating_add(1);
        budget.updated_at = now;

        DeepReviewActiveReviewerGuard {
            tracker: self,
            parent_dialog_turn_id: parent_dialog_turn_id.to_string(),
            launch_batch: None,
            released: false,
        }
    }

    pub fn try_begin_active_reviewer<'a>(
        &'a self,
        parent_dialog_turn_id: &str,
        max_active_reviewers: usize,
    ) -> Option<DeepReviewActiveReviewerGuard<'a>> {
        let now = Instant::now();
        let mut budget = self
            .turns
            .entry(parent_dialog_turn_id.to_string())
            .or_insert_with(|| DeepReviewTurnBudget::new(now));
        if budget.active_reviewers >= max_active_reviewers {
            return None;
        }

        budget.active_reviewers = budget.active_reviewers.saturating_add(1);
        budget.updated_at = now;
        Some(DeepReviewActiveReviewerGuard {
            tracker: self,
            parent_dialog_turn_id: parent_dialog_turn_id.to_string(),
            launch_batch: None,
            released: false,
        })
    }

    pub fn try_begin_active_reviewer_for_launch_batch<'a>(
        &'a self,
        parent_dialog_turn_id: &str,
        max_active_reviewers: usize,
        launch_batch: u64,
        _packet_id: Option<&str>,
    ) -> Result<Option<DeepReviewActiveReviewerGuard<'a>>, DeepReviewPolicyViolation> {
        let now = Instant::now();
        let mut budget = self
            .turns
            .entry(parent_dialog_turn_id.to_string())
            .or_insert_with(|| DeepReviewTurnBudget::new(now));

        if budget.active_reviewers >= max_active_reviewers {
            return Ok(None);
        }

        budget.active_reviewers = budget.active_reviewers.saturating_add(1);
        *budget
            .active_reviewer_launch_batches
            .entry(launch_batch)
            .or_insert(0) += 1;
        budget.updated_at = now;
        Ok(Some(DeepReviewActiveReviewerGuard {
            tracker: self,
            parent_dialog_turn_id: parent_dialog_turn_id.to_string(),
            launch_batch: Some(launch_batch),
            released: false,
        }))
    }

    fn finish_active_reviewer(&self, parent_dialog_turn_id: &str, launch_batch: Option<u64>) {
        if let Some(mut budget) = self.turns.get_mut(parent_dialog_turn_id) {
            budget.active_reviewers = budget.active_reviewers.saturating_sub(1);
            if let Some(launch_batch) = launch_batch {
                let should_remove_batch = if let Some(count) =
                    budget.active_reviewer_launch_batches.get_mut(&launch_batch)
                {
                    *count = (*count).saturating_sub(1);
                    *count == 0
                } else {
                    false
                };
                if should_remove_batch {
                    budget.active_reviewer_launch_batches.remove(&launch_batch);
                }
            }
            budget.updated_at = Instant::now();
        }
    }

    fn prune_stale(&self, now: Instant) {
        self.turns
            .retain(|_, budget| now.saturating_duration_since(budget.updated_at) <= BUDGET_TTL);
        if let Ok(mut last_pruned) = self.last_pruned_at.lock() {
            *last_pruned = now;
        }
    }

    /// Explicitly clean up all budget tracking data.
    /// Call this when the application is shutting down or when the review session ends.
    pub fn cleanup(&self) {
        self.turns.clear();
        if let Ok(mut last_pruned) = self.last_pruned_at.lock() {
            *last_pruned = Instant::now();
        }
    }

    /// Returns the number of reviewer calls recorded for a given turn.
    /// Used by the concurrency enforcement to check if a new launch is allowed.
    pub fn active_reviewer_count(&self, parent_dialog_turn_id: &str) -> usize {
        self.turns
            .get(parent_dialog_turn_id)
            .map(|budget| budget.active_reviewers)
            .unwrap_or(0)
    }

    /// Returns true if a judge call has been recorded for a given turn.
    pub fn has_judge_been_launched(&self, parent_dialog_turn_id: &str) -> bool {
        self.turns
            .get(parent_dialog_turn_id)
            .map(|budget| budget.judge_calls > 0)
            .unwrap_or(false)
    }

    pub fn concurrency_cap_rejection_count(&self, parent_dialog_turn_id: &str) -> usize {
        self.turns
            .get(parent_dialog_turn_id)
            .map(|budget| budget.concurrency_cap_rejections)
            .unwrap_or(0)
    }

    pub fn capacity_skip_count(&self, parent_dialog_turn_id: &str) -> usize {
        self.turns
            .get(parent_dialog_turn_id)
            .map(|budget| budget.capacity_skips)
            .unwrap_or(0)
    }

    pub fn retries_used(&self, parent_dialog_turn_id: &str, subagent_type: &str) -> usize {
        self.turns
            .get(parent_dialog_turn_id)
            .map(|budget| {
                budget
                    .retries_used_by_subagent
                    .get(subagent_type)
                    .copied()
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    }

    pub fn effective_concurrency_snapshot(
        &self,
        parent_dialog_turn_id: &str,
        configured_max_parallel_instances: usize,
    ) -> DeepReviewEffectiveConcurrencySnapshot {
        if parent_dialog_turn_id.trim().is_empty() {
            return DeepReviewEffectiveConcurrencyState::new(configured_max_parallel_instances)
                .snapshot(Instant::now());
        }

        let now = Instant::now();
        let mut budget = self
            .turns
            .entry(parent_dialog_turn_id.to_string())
            .or_insert_with(|| DeepReviewTurnBudget::new(now));
        budget.updated_at = now;
        budget
            .effective_concurrency_mut(configured_max_parallel_instances)
            .snapshot(now)
    }

    pub fn effective_parallel_instances(
        &self,
        parent_dialog_turn_id: &str,
        configured_max_parallel_instances: usize,
    ) -> usize {
        self.effective_concurrency_snapshot(
            parent_dialog_turn_id,
            configured_max_parallel_instances,
        )
        .effective_parallel_instances
    }

    pub fn record_effective_concurrency_capacity_error(
        &self,
        parent_dialog_turn_id: &str,
        configured_max_parallel_instances: usize,
        reason: DeepReviewCapacityQueueReason,
        retry_after: Option<Duration>,
    ) -> DeepReviewEffectiveConcurrencySnapshot {
        if parent_dialog_turn_id.trim().is_empty() {
            return DeepReviewEffectiveConcurrencyState::new(configured_max_parallel_instances)
                .snapshot(Instant::now());
        }

        let now = Instant::now();
        let mut budget = self
            .turns
            .entry(parent_dialog_turn_id.to_string())
            .or_insert_with(|| DeepReviewTurnBudget::new(now));
        budget.updated_at = now;
        let snapshot = {
            let state = budget.effective_concurrency_mut(configured_max_parallel_instances);
            state.record_capacity_error(
                matches!(reason, DeepReviewCapacityQueueReason::RetryAfter),
                retry_after,
                now,
            );
            state.snapshot(now)
        };
        budget
            .runtime_diagnostics
            .observe_effective_parallel(snapshot.effective_parallel_instances);
        snapshot
    }

    pub fn record_effective_concurrency_success(
        &self,
        parent_dialog_turn_id: &str,
        configured_max_parallel_instances: usize,
    ) -> DeepReviewEffectiveConcurrencySnapshot {
        if parent_dialog_turn_id.trim().is_empty() {
            return DeepReviewEffectiveConcurrencyState::new(configured_max_parallel_instances)
                .snapshot(Instant::now());
        }

        let now = Instant::now();
        let mut budget = self
            .turns
            .entry(parent_dialog_turn_id.to_string())
            .or_insert_with(|| DeepReviewTurnBudget::new(now));
        budget.updated_at = now;
        let snapshot = {
            let state = budget.effective_concurrency_mut(configured_max_parallel_instances);
            state.record_success(now);
            state.snapshot(now)
        };
        budget
            .runtime_diagnostics
            .observe_effective_parallel(snapshot.effective_parallel_instances);
        snapshot
    }

    pub fn set_effective_concurrency_user_override(
        &self,
        parent_dialog_turn_id: &str,
        configured_max_parallel_instances: usize,
        user_override_parallel_instances: Option<usize>,
    ) -> DeepReviewEffectiveConcurrencySnapshot {
        if parent_dialog_turn_id.trim().is_empty() {
            return DeepReviewEffectiveConcurrencyState::new(configured_max_parallel_instances)
                .snapshot(Instant::now());
        }

        let now = Instant::now();
        let mut budget = self
            .turns
            .entry(parent_dialog_turn_id.to_string())
            .or_insert_with(|| DeepReviewTurnBudget::new(now));
        budget.updated_at = now;
        let snapshot = {
            let state = budget.effective_concurrency_mut(configured_max_parallel_instances);
            state.set_user_override(user_override_parallel_instances);
            state.snapshot(now)
        };
        budget
            .runtime_diagnostics
            .observe_effective_parallel(snapshot.effective_parallel_instances);
        snapshot
    }
}

fn normalize_budget_subagent_type(
    subagent_type: &str,
) -> Result<String, DeepReviewPolicyViolation> {
    let normalized = subagent_type.trim();
    if normalized.is_empty() {
        return Err(DeepReviewPolicyViolation::new(
            "deep_review_subagent_type_missing",
            "DeepReview task budget requires a non-empty subagent type",
        ));
    }

    Ok(normalized.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_batch_admission_allows_later_batch_when_reviewer_capacity_is_free() {
        let tracker = DeepReviewBudgetTracker::default();
        let turn_id = "turn-launch-batch-fill-free-slot";
        let _first_batch = tracker
            .try_begin_active_reviewer_for_launch_batch(turn_id, 2, 1, Some("packet-a"))
            .expect("batch admission should not fail")
            .expect("first reviewer should start");

        let second_batch = tracker
            .try_begin_active_reviewer_for_launch_batch(turn_id, 2, 2, Some("packet-b"))
            .expect("later batch admission should not fail when reviewer capacity is free");

        assert!(
            second_batch.is_some(),
            "later batch should fill a freed reviewer slot instead of waiting for the earlier batch to drain"
        );
    }

    #[test]
    fn launch_batch_admission_allows_same_batch_and_next_batch_after_release() {
        let tracker = DeepReviewBudgetTracker::default();
        let turn_id = "turn-launch-batch-release";
        let first = tracker
            .try_begin_active_reviewer_for_launch_batch(turn_id, 2, 1, Some("packet-a"))
            .expect("first batch should not violate launch order")
            .expect("first reviewer should start");
        let second = tracker
            .try_begin_active_reviewer_for_launch_batch(turn_id, 2, 1, Some("packet-b"))
            .expect("same batch should not violate launch order")
            .expect("second reviewer should start");
        assert!(
            tracker
                .try_begin_active_reviewer_for_launch_batch(turn_id, 2, 1, Some("packet-c"))
                .expect("same batch should not violate launch order")
                .is_none(),
            "same-batch admission should still respect active reviewer capacity"
        );

        drop(first);
        drop(second);

        assert!(tracker
            .try_begin_active_reviewer_for_launch_batch(turn_id, 2, 2, Some("packet-c"))
            .expect("next batch should start after the previous batch releases")
            .is_some());
    }
}
