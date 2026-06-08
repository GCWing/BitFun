use super::budget::{DeepReviewActiveReviewerGuard, DeepReviewBudgetTracker};
use super::concurrency_policy::DeepReviewEffectiveConcurrencySnapshot;
use super::constants::DEFAULT_MAX_RETRIES_PER_ROLE;
use super::diagnostics::DeepReviewRuntimeDiagnostics;
use super::execution_policy::{
    DeepReviewExecutionPolicy, DeepReviewPolicyViolation, DeepReviewSubagentRole,
};
use super::queue::{
    DeepReviewCapacityQueueReason, DeepReviewQueueControlAction, DeepReviewQueueControlSnapshot,
    DeepReviewQueueControlTracker,
};
use super::shared_context::DeepReviewSharedContextMeasurementSnapshot;
use std::sync::LazyLock;
use std::time::Duration;

static GLOBAL_DEEP_REVIEW_BUDGET_TRACKER: LazyLock<DeepReviewBudgetTracker> =
    LazyLock::new(DeepReviewBudgetTracker::default);
static GLOBAL_DEEP_REVIEW_QUEUE_CONTROL_TRACKER: LazyLock<DeepReviewQueueControlTracker> =
    LazyLock::new(DeepReviewQueueControlTracker::default);

pub fn record_deep_review_task_budget(
    parent_dialog_turn_id: &str,
    policy: &DeepReviewExecutionPolicy,
    role: DeepReviewSubagentRole,
    subagent_type: &str,
    is_retry: bool,
) -> Result<(), DeepReviewPolicyViolation> {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_task(
        parent_dialog_turn_id,
        policy,
        role,
        subagent_type,
        is_retry,
    )
}

pub fn record_deep_review_concurrency_cap_rejection(parent_dialog_turn_id: &str) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_concurrency_cap_rejection(parent_dialog_turn_id)
}

pub fn record_deep_review_capacity_skip(parent_dialog_turn_id: &str) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_capacity_skip(parent_dialog_turn_id)
}

pub fn record_deep_review_capacity_skip_for_reason(
    parent_dialog_turn_id: &str,
    reason: DeepReviewCapacityQueueReason,
) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_capacity_skip_for_reason(parent_dialog_turn_id, reason)
}

pub fn record_deep_review_runtime_queue_wait(parent_dialog_turn_id: &str, queue_elapsed_ms: u64) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER
        .record_runtime_queue_wait(parent_dialog_turn_id, queue_elapsed_ms)
}

pub fn record_deep_review_runtime_provider_capacity_queue(
    parent_dialog_turn_id: &str,
    reason: DeepReviewCapacityQueueReason,
) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER
        .record_runtime_provider_capacity_queue(parent_dialog_turn_id, reason)
}

pub fn record_deep_review_runtime_provider_capacity_retry(
    parent_dialog_turn_id: &str,
    reason: DeepReviewCapacityQueueReason,
) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER
        .record_runtime_provider_capacity_retry(parent_dialog_turn_id, reason)
}

pub fn record_deep_review_runtime_provider_capacity_retry_success(
    parent_dialog_turn_id: &str,
    reason: DeepReviewCapacityQueueReason,
) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER
        .record_runtime_provider_capacity_retry_success(parent_dialog_turn_id, reason)
}

pub fn record_deep_review_runtime_capacity_skip(
    parent_dialog_turn_id: &str,
    reason: DeepReviewCapacityQueueReason,
) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_runtime_capacity_skip(parent_dialog_turn_id, reason)
}

pub fn record_deep_review_runtime_manual_queue_action(parent_dialog_turn_id: &str) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_runtime_manual_queue_action(parent_dialog_turn_id)
}

pub fn record_deep_review_runtime_manual_retry(parent_dialog_turn_id: &str) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_runtime_manual_retry(parent_dialog_turn_id)
}

pub fn record_deep_review_runtime_auto_retry(parent_dialog_turn_id: &str) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_runtime_auto_retry(parent_dialog_turn_id)
}

pub fn record_deep_review_runtime_auto_retry_suppressed(parent_dialog_turn_id: &str, reason: &str) {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER
        .record_runtime_auto_retry_suppressed(parent_dialog_turn_id, reason)
}

pub fn record_deep_review_shared_context_tool_use(
    parent_dialog_turn_id: &str,
    subagent_type: &str,
    tool_name: &str,
    file_path: &str,
) -> DeepReviewSharedContextMeasurementSnapshot {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_shared_context_tool_use(
        parent_dialog_turn_id,
        subagent_type,
        tool_name,
        file_path,
    )
}

pub fn deep_review_shared_context_measurement_snapshot(
    parent_dialog_turn_id: &str,
) -> DeepReviewSharedContextMeasurementSnapshot {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.shared_context_measurement_snapshot(parent_dialog_turn_id)
}

pub fn deep_review_runtime_diagnostics_snapshot(
    parent_dialog_turn_id: &str,
) -> Option<DeepReviewRuntimeDiagnostics> {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.runtime_diagnostics_snapshot(parent_dialog_turn_id)
}

pub fn try_begin_deep_review_active_reviewer(
    parent_dialog_turn_id: &str,
    max_active_reviewers: usize,
) -> Option<DeepReviewActiveReviewerGuard<'static>> {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER
        .try_begin_active_reviewer(parent_dialog_turn_id, max_active_reviewers)
}

pub fn try_begin_deep_review_active_reviewer_for_launch_batch(
    parent_dialog_turn_id: &str,
    max_active_reviewers: usize,
    launch_batch: u64,
    packet_id: Option<&str>,
) -> Result<Option<DeepReviewActiveReviewerGuard<'static>>, DeepReviewPolicyViolation> {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.try_begin_active_reviewer_for_launch_batch(
        parent_dialog_turn_id,
        max_active_reviewers,
        launch_batch,
        packet_id,
    )
}

pub fn deep_review_effective_concurrency_snapshot(
    parent_dialog_turn_id: &str,
    configured_max_parallel_instances: usize,
) -> DeepReviewEffectiveConcurrencySnapshot {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER
        .effective_concurrency_snapshot(parent_dialog_turn_id, configured_max_parallel_instances)
}

pub fn deep_review_effective_parallel_instances(
    parent_dialog_turn_id: &str,
    configured_max_parallel_instances: usize,
) -> usize {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER
        .effective_parallel_instances(parent_dialog_turn_id, configured_max_parallel_instances)
}

pub fn record_deep_review_effective_concurrency_capacity_error(
    parent_dialog_turn_id: &str,
    configured_max_parallel_instances: usize,
    reason: DeepReviewCapacityQueueReason,
    retry_after: Option<Duration>,
) -> DeepReviewEffectiveConcurrencySnapshot {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_effective_concurrency_capacity_error(
        parent_dialog_turn_id,
        configured_max_parallel_instances,
        reason,
        retry_after,
    )
}

pub fn record_deep_review_effective_concurrency_success(
    parent_dialog_turn_id: &str,
    configured_max_parallel_instances: usize,
) -> DeepReviewEffectiveConcurrencySnapshot {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.record_effective_concurrency_success(
        parent_dialog_turn_id,
        configured_max_parallel_instances,
    )
}

pub fn set_deep_review_effective_concurrency_user_override(
    parent_dialog_turn_id: &str,
    configured_max_parallel_instances: usize,
    user_override_parallel_instances: Option<usize>,
) -> DeepReviewEffectiveConcurrencySnapshot {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.set_effective_concurrency_user_override(
        parent_dialog_turn_id,
        configured_max_parallel_instances,
        user_override_parallel_instances,
    )
}

/// Returns the number of active reviewer calls for a given turn.
pub fn deep_review_active_reviewer_count(parent_dialog_turn_id: &str) -> usize {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.active_reviewer_count(parent_dialog_turn_id)
}

/// Returns true if a judge has been launched for a given turn.
pub fn deep_review_has_judge_been_launched(parent_dialog_turn_id: &str) -> bool {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.has_judge_been_launched(parent_dialog_turn_id)
}

pub fn deep_review_concurrency_cap_rejection_count(parent_dialog_turn_id: &str) -> usize {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.concurrency_cap_rejection_count(parent_dialog_turn_id)
}

pub fn deep_review_capacity_skip_count(parent_dialog_turn_id: &str) -> usize {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.capacity_skip_count(parent_dialog_turn_id)
}

pub fn apply_deep_review_queue_control(
    parent_dialog_turn_id: &str,
    tool_id: &str,
    action: DeepReviewQueueControlAction,
) -> DeepReviewQueueControlSnapshot {
    GLOBAL_DEEP_REVIEW_QUEUE_CONTROL_TRACKER.apply(parent_dialog_turn_id, tool_id, action)
}

pub fn deep_review_queue_control_snapshot(
    parent_dialog_turn_id: &str,
    tool_id: &str,
) -> DeepReviewQueueControlSnapshot {
    GLOBAL_DEEP_REVIEW_QUEUE_CONTROL_TRACKER.snapshot(parent_dialog_turn_id, tool_id)
}

pub fn clear_deep_review_queue_control_for_tool(parent_dialog_turn_id: &str, tool_id: &str) {
    GLOBAL_DEEP_REVIEW_QUEUE_CONTROL_TRACKER.clear_tool(parent_dialog_turn_id, tool_id)
}

/// Returns the number of retries used for a specific subagent type in a given turn.
pub fn deep_review_retries_used(parent_dialog_turn_id: &str, subagent_type: &str) -> usize {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.retries_used(parent_dialog_turn_id, subagent_type)
}

pub fn deep_review_turn_elapsed_seconds(parent_dialog_turn_id: &str) -> Option<u64> {
    GLOBAL_DEEP_REVIEW_BUDGET_TRACKER.turn_elapsed_seconds(parent_dialog_turn_id)
}

/// Returns the fallback max retries per role when an effective run policy is unavailable.
pub fn deep_review_max_retries_per_role(_parent_dialog_turn_id: &str) -> usize {
    DEFAULT_MAX_RETRIES_PER_ROLE
}
