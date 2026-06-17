//! Deep Review-specific TaskTool adapter helpers.
//!
//! This module adapts generic TaskTool execution to Deep Review policy,
//! manifests, queue events, retry metadata, and report reliability signals.
//! Shared mechanics such as queue wait timing live under
//! `agentic::subagent_runtime`; Deep Review-specific admission and event
//! semantics stay here.

use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::deep_review::queue::extract_retry_after_seconds;
use crate::agentic::deep_review_policy::{
    clear_deep_review_queue_control_for_tool, deep_review_active_reviewer_count,
    deep_review_effective_concurrency_snapshot, deep_review_effective_parallel_instances,
    deep_review_max_retries_per_role, deep_review_queue_control_snapshot,
    record_deep_review_capacity_skip_for_reason,
    record_deep_review_effective_concurrency_capacity_error,
    record_deep_review_runtime_provider_capacity_queue,
    record_deep_review_runtime_provider_capacity_retry,
    record_deep_review_runtime_provider_capacity_retry_success,
    record_deep_review_runtime_queue_wait, try_begin_deep_review_active_reviewer,
    try_begin_deep_review_active_reviewer_for_launch_batch, DeepReviewActiveReviewerGuard,
    DeepReviewCapacityQueueDecision, DeepReviewCapacityQueueReason, DeepReviewConcurrencyPolicy,
    DeepReviewExecutionPolicy, DeepReviewPolicyViolation, DeepReviewSubagentRole,
};
use crate::agentic::events::{
    DeepReviewQueueReason, DeepReviewQueueState, DeepReviewQueueStatus, ErrorCategory,
};
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_agent_runtime::deep_review::task_execution as runtime_task_execution;
pub(crate) use bitfun_agent_runtime::deep_review::task_execution::{
    attach_deep_review_cache, deep_review_incremental_cache_hit_for_task,
    deep_review_incremental_cache_hit_result, deep_review_launch_batch_for_task,
    ensure_deep_review_retry_coverage, prompt_with_deep_review_retry_scope,
    DeepReviewProviderCapacityRetryDecision, DeepReviewProviderCapacityRetryRuntime,
};
pub(crate) use bitfun_agent_runtime::deep_review::{
    DeepReviewLaunchBatchInfo, DeepReviewQueueWaitSkipReason,
};
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[cfg(test)]
const DEEP_REVIEW_QUEUE_POLL_INTERVAL: Duration = Duration::from_millis(10);
#[cfg(not(test))]
const DEEP_REVIEW_QUEUE_POLL_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) enum DeepReviewQueueWaitOutcome {
    Ready {
        guard: DeepReviewActiveReviewerGuard<'static>,
    },
    Skipped {
        queue_elapsed_ms: u64,
        skip_reason: DeepReviewQueueWaitSkipReason,
        capacity_reason: DeepReviewCapacityQueueReason,
    },
}

pub(crate) enum DeepReviewProviderQueueWaitOutcome {
    ReadyToRetry {
        queue_elapsed_ms: u64,
        early_capacity_probe: bool,
    },
    Skipped {
        queue_elapsed_ms: u64,
        skip_reason: DeepReviewQueueWaitSkipReason,
    },
}

pub(crate) fn deep_review_retry_guidance_max_retries(
    effective_policy: Option<&DeepReviewExecutionPolicy>,
    dialog_turn_id: &str,
) -> usize {
    effective_policy
        .map(|policy| policy.max_retries_per_role)
        .unwrap_or_else(|| deep_review_max_retries_per_role(dialog_turn_id))
}

pub(crate) fn should_emit_deep_review_retry_guidance(
    is_partial_timeout: bool,
    is_retry: bool,
    deep_review_subagent_role: Option<DeepReviewSubagentRole>,
) -> bool {
    runtime_task_execution::should_emit_deep_review_retry_guidance(
        is_partial_timeout,
        is_retry,
        deep_review_subagent_role,
    )
}

pub(crate) fn deep_review_retry_guidance(retries_used: usize, max_retries: usize) -> String {
    runtime_task_execution::deep_review_retry_guidance(retries_used, max_retries)
}

pub(crate) fn auto_retry_suppression_reason(code: &str) -> &'static str {
    runtime_task_execution::auto_retry_suppression_reason(code)
}

pub(crate) fn ensure_deep_review_auto_retry_allowed(
    conc_policy: &DeepReviewConcurrencyPolicy,
    elapsed_seconds: Option<u64>,
) -> Result<(), DeepReviewPolicyViolation> {
    runtime_task_execution::ensure_deep_review_auto_retry_allowed(conc_policy, elapsed_seconds)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn deep_review_task_completion_result(
    delegate_target_label: &str,
    result_text: &str,
    context_mode: &str,
    duration_ms: u128,
    is_partial_timeout: bool,
    reason: Option<&str>,
    ledger_event_id: Option<&str>,
    retry_hint: &str,
) -> (Value, String) {
    runtime_task_execution::deep_review_task_completion_result(
        runtime_task_execution::DeepReviewTaskCompletionResultInput {
            delegate_target_label,
            result_text,
            context_mode,
            duration_ms,
            is_partial_timeout,
            reason,
            ledger_event_id,
            retry_hint,
        },
    )
}

pub(crate) fn deep_review_cancelled_reviewer_result(
    subagent_type: &str,
    reason: &str,
    duration_ms: u128,
) -> (Value, String) {
    runtime_task_execution::deep_review_cancelled_reviewer_result(
        subagent_type,
        reason,
        duration_ms,
    )
}

pub(crate) fn queue_reason_to_event_reason(
    reason: DeepReviewCapacityQueueReason,
) -> DeepReviewQueueReason {
    match reason {
        DeepReviewCapacityQueueReason::ProviderRateLimit => {
            DeepReviewQueueReason::ProviderRateLimit
        }
        DeepReviewCapacityQueueReason::ProviderConcurrencyLimit => {
            DeepReviewQueueReason::ProviderConcurrencyLimit
        }
        DeepReviewCapacityQueueReason::RetryAfter => DeepReviewQueueReason::RetryAfter,
        DeepReviewCapacityQueueReason::LocalConcurrencyCap => {
            DeepReviewQueueReason::LocalConcurrencyCap
        }
        DeepReviewCapacityQueueReason::LaunchBatchBlocked => {
            DeepReviewQueueReason::LaunchBatchBlocked
        }
        DeepReviewCapacityQueueReason::TemporaryOverload => {
            DeepReviewQueueReason::TemporaryOverload
        }
    }
}

pub(crate) fn capacity_decision_for_provider_error(
    error: &BitFunError,
) -> DeepReviewCapacityQueueDecision {
    let detail = error.error_detail();
    let error_message = error.to_string();
    let code = detail.provider_code.as_deref().unwrap_or_default();
    let message = detail
        .provider_message
        .as_deref()
        .unwrap_or(error_message.as_str());
    runtime_task_execution::capacity_decision_for_provider_error_facts(
        runtime_task_execution::DeepReviewProviderCapacityErrorFacts {
            provider_code: code,
            provider_message: message,
            retry_after_seconds: extract_retry_after_seconds(&error_message),
            category: match detail.category {
                ErrorCategory::RateLimit => {
                    runtime_task_execution::DeepReviewProviderCapacityErrorCategory::RateLimit
                }
                ErrorCategory::ProviderUnavailable => {
                    runtime_task_execution::DeepReviewProviderCapacityErrorCategory::ProviderUnavailable
                }
                _ => runtime_task_execution::DeepReviewProviderCapacityErrorCategory::Other,
            },
        },
    )
}

pub(crate) fn capacity_skip_result_for_local_queue_outcome(
    dialog_turn_id: &str,
    subagent_type: &str,
    conc_policy: &DeepReviewConcurrencyPolicy,
    capacity_reason: DeepReviewCapacityQueueReason,
    skip_reason: DeepReviewQueueWaitSkipReason,
    queue_elapsed_ms: u64,
    duration_ms: u128,
) -> (Value, String) {
    let effective_parallel_instances = deep_review_effective_concurrency_snapshot(
        dialog_turn_id,
        conc_policy.max_parallel_instances,
    )
    .effective_parallel_instances;
    runtime_task_execution::capacity_skip_result_for_local_queue_outcome(
        subagent_type,
        conc_policy,
        capacity_reason,
        skip_reason,
        queue_elapsed_ms,
        duration_ms,
        effective_parallel_instances,
    )
}

pub(crate) fn capacity_skip_result_for_provider_queue_outcome(
    reason: DeepReviewCapacityQueueReason,
    dialog_turn_id: &str,
    subagent_type: &str,
    conc_policy: &DeepReviewConcurrencyPolicy,
    duration_ms: u128,
    queue_elapsed_ms: u64,
    terminal_skip_reason: Option<DeepReviewQueueWaitSkipReason>,
) -> (Value, String) {
    let snapshot = record_deep_review_effective_concurrency_capacity_error(
        dialog_turn_id,
        conc_policy.max_parallel_instances,
        reason,
        None,
    );
    record_deep_review_capacity_skip_for_reason(dialog_turn_id, reason);
    runtime_task_execution::capacity_skip_result_for_provider_queue_outcome(
        reason,
        subagent_type,
        conc_policy,
        duration_ms,
        queue_elapsed_ms,
        terminal_skip_reason,
        snapshot.effective_parallel_instances,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn emit_queue_state(
    session_id: &str,
    dialog_turn_id: &str,
    tool_id: &str,
    subagent_type: &str,
    status: DeepReviewQueueStatus,
    reason: Option<DeepReviewCapacityQueueReason>,
    queued_reviewer_count: usize,
    active_reviewer_count: usize,
    optional_reviewer_count: Option<usize>,
    effective_parallel_instances: Option<usize>,
    queue_elapsed_ms: u64,
    max_queue_wait_seconds: u64,
) {
    let run_elapsed_ms = matches!(&status, DeepReviewQueueStatus::Running).then_some(0);
    if let Some(coordinator) = get_global_coordinator() {
        coordinator
            .emit_deep_review_queue_state_changed(
                session_id,
                dialog_turn_id,
                DeepReviewQueueState {
                    tool_id: tool_id.to_string(),
                    subagent_type: subagent_type.to_string(),
                    status,
                    reason: reason.map(queue_reason_to_event_reason),
                    queued_reviewer_count,
                    active_reviewer_count: Some(active_reviewer_count),
                    effective_parallel_instances,
                    optional_reviewer_count,
                    queue_elapsed_ms: Some(queue_elapsed_ms),
                    run_elapsed_ms,
                    max_queue_wait_seconds: Some(max_queue_wait_seconds),
                    session_concurrency_high: false,
                },
            )
            .await;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn wait_for_provider_capacity_retry(
    session_id: &str,
    dialog_turn_id: &str,
    tool_id: &str,
    subagent_type: &str,
    conc_policy: &DeepReviewConcurrencyPolicy,
    reason: DeepReviewCapacityQueueReason,
    max_wait_seconds: u64,
    is_optional_reviewer: bool,
) -> DeepReviewProviderQueueWaitOutcome {
    let max_wait = Duration::from_secs(max_wait_seconds);
    let optional_reviewer_count = is_optional_reviewer.then_some(1);
    let initial_active_reviewers = deep_review_active_reviewer_count(dialog_turn_id);
    let mut queue_runtime = runtime_task_execution::DeepReviewProviderCapacityQueueRuntime::start(
        Instant::now(),
        reason,
        max_wait,
        initial_active_reviewers,
        is_optional_reviewer,
    );

    record_deep_review_runtime_provider_capacity_queue(dialog_turn_id, reason);

    loop {
        let now = Instant::now();
        let active_reviewers = deep_review_active_reviewer_count(dialog_turn_id);
        let effective_parallel_instances = deep_review_effective_parallel_instances(
            dialog_turn_id,
            conc_policy.max_parallel_instances,
        );
        let control_snapshot = deep_review_queue_control_snapshot(dialog_turn_id, tool_id);
        let queue_step = queue_runtime.step(
            runtime_task_execution::DeepReviewProviderCapacityQueueRuntimeInput {
                now,
                active_reviewer_count: active_reviewers,
                control_snapshot,
                poll_interval: DEEP_REVIEW_QUEUE_POLL_INTERVAL,
            },
        );

        match queue_step {
            runtime_task_execution::DeepReviewProviderCapacityQueueRuntimeStep::Skipped {
                queue_elapsed_ms,
                skip_reason,
            } => {
                record_deep_review_runtime_queue_wait(dialog_turn_id, queue_elapsed_ms);
                clear_deep_review_queue_control_for_tool(dialog_turn_id, tool_id);
                emit_queue_state(
                    session_id,
                    dialog_turn_id,
                    tool_id,
                    subagent_type,
                    DeepReviewQueueStatus::CapacitySkipped,
                    Some(reason),
                    0,
                    active_reviewers,
                    optional_reviewer_count,
                    Some(effective_parallel_instances),
                    queue_elapsed_ms,
                    max_wait_seconds,
                )
                .await;
                return DeepReviewProviderQueueWaitOutcome::Skipped {
                    queue_elapsed_ms,
                    skip_reason,
                };
            }
            runtime_task_execution::DeepReviewProviderCapacityQueueRuntimeStep::Paused {
                queue_elapsed_ms,
                next_sleep,
            } => {
                emit_queue_state(
                    session_id,
                    dialog_turn_id,
                    tool_id,
                    subagent_type,
                    DeepReviewQueueStatus::PausedByUser,
                    Some(reason),
                    1,
                    active_reviewers,
                    optional_reviewer_count,
                    Some(effective_parallel_instances),
                    queue_elapsed_ms,
                    max_wait_seconds,
                )
                .await;
                sleep(next_sleep).await;
                continue;
            }
            runtime_task_execution::DeepReviewProviderCapacityQueueRuntimeStep::ReadyToRetry {
                queue_elapsed_ms,
                early_capacity_probe,
            } => {
                record_deep_review_runtime_queue_wait(dialog_turn_id, queue_elapsed_ms);
                clear_deep_review_queue_control_for_tool(dialog_turn_id, tool_id);
                emit_queue_state(
                    session_id,
                    dialog_turn_id,
                    tool_id,
                    subagent_type,
                    DeepReviewQueueStatus::Running,
                    Some(reason),
                    0,
                    active_reviewers,
                    optional_reviewer_count,
                    Some(effective_parallel_instances),
                    queue_elapsed_ms,
                    max_wait_seconds,
                )
                .await;
                return DeepReviewProviderQueueWaitOutcome::ReadyToRetry {
                    queue_elapsed_ms,
                    early_capacity_probe,
                };
            }
            runtime_task_execution::DeepReviewProviderCapacityQueueRuntimeStep::Queued {
                queue_elapsed_ms,
                next_sleep,
            } => {
                emit_queue_state(
                    session_id,
                    dialog_turn_id,
                    tool_id,
                    subagent_type,
                    DeepReviewQueueStatus::QueuedForCapacity,
                    Some(reason),
                    1,
                    active_reviewers,
                    optional_reviewer_count,
                    Some(effective_parallel_instances),
                    queue_elapsed_ms,
                    max_wait_seconds,
                )
                .await;
                sleep(next_sleep).await;
            }
        }
    }
}

pub(crate) fn record_provider_capacity_retry(
    dialog_turn_id: &str,
    reason: DeepReviewCapacityQueueReason,
) {
    record_deep_review_runtime_provider_capacity_retry(dialog_turn_id, reason);
}

pub(crate) fn record_provider_capacity_retry_success(
    dialog_turn_id: &str,
    reason: DeepReviewCapacityQueueReason,
) {
    record_deep_review_runtime_provider_capacity_retry_success(dialog_turn_id, reason);
}

pub(crate) fn try_begin_reviewer_admission(
    dialog_turn_id: &str,
    effective_parallel_instances: usize,
    launch_batch_info: Option<&DeepReviewLaunchBatchInfo>,
) -> Result<Option<DeepReviewActiveReviewerGuard<'static>>, DeepReviewPolicyViolation> {
    match launch_batch_info {
        Some(info) => try_begin_deep_review_active_reviewer_for_launch_batch(
            dialog_turn_id,
            effective_parallel_instances,
            info.launch_batch,
            info.packet_id.as_deref(),
        ),
        None => Ok(try_begin_deep_review_active_reviewer(
            dialog_turn_id,
            effective_parallel_instances,
        )),
    }
}

pub(crate) async fn wait_for_reviewer_admission(
    session_id: &str,
    dialog_turn_id: &str,
    tool_id: &str,
    subagent_type: &str,
    conc_policy: &DeepReviewConcurrencyPolicy,
    is_optional_reviewer: bool,
    launch_batch_info: Option<&DeepReviewLaunchBatchInfo>,
) -> BitFunResult<DeepReviewQueueWaitOutcome> {
    let decision = runtime_task_execution::local_reviewer_capacity_queue_decision();
    let local_capacity_reason = decision
        .reason
        .unwrap_or(DeepReviewCapacityQueueReason::LocalConcurrencyCap);
    let max_wait = Duration::from_secs(conc_policy.max_queue_wait_seconds);
    let optional_reviewer_count = is_optional_reviewer.then_some(1);
    let mut queue_runtime = runtime_task_execution::DeepReviewReviewerAdmissionQueueRuntime::start(
        Instant::now(),
        local_capacity_reason,
        max_wait,
        decision.retry_after_seconds,
        is_optional_reviewer,
    );

    loop {
        let now = Instant::now();
        let active_reviewers = deep_review_active_reviewer_count(dialog_turn_id);
        let effective_parallel_instances = deep_review_effective_parallel_instances(
            dialog_turn_id,
            conc_policy.max_parallel_instances,
        );
        let control_snapshot = deep_review_queue_control_snapshot(dialog_turn_id, tool_id);
        let queue_step = queue_runtime.begin_step(
            runtime_task_execution::DeepReviewReviewerAdmissionQueueRuntimeInput {
                now,
                control_snapshot,
                poll_interval: DEEP_REVIEW_QUEUE_POLL_INTERVAL,
            },
        );
        let (queue_elapsed_ms, admission_attempt) = match queue_step {
            runtime_task_execution::DeepReviewReviewerAdmissionQueueRuntimeStep::Skipped {
                queue_elapsed_ms,
                skip_reason,
                capacity_reason,
            } => {
                record_deep_review_runtime_queue_wait(dialog_turn_id, queue_elapsed_ms);
                record_deep_review_capacity_skip_for_reason(dialog_turn_id, capacity_reason);
                clear_deep_review_queue_control_for_tool(dialog_turn_id, tool_id);
                emit_queue_state(
                    session_id,
                    dialog_turn_id,
                    tool_id,
                    subagent_type,
                    DeepReviewQueueStatus::CapacitySkipped,
                    Some(capacity_reason),
                    0,
                    active_reviewers,
                    optional_reviewer_count,
                    Some(effective_parallel_instances),
                    queue_elapsed_ms,
                    conc_policy.max_queue_wait_seconds,
                )
                .await;
                return Ok(DeepReviewQueueWaitOutcome::Skipped {
                    queue_elapsed_ms,
                    skip_reason,
                    capacity_reason,
                });
            }
            runtime_task_execution::DeepReviewReviewerAdmissionQueueRuntimeStep::Paused {
                queue_elapsed_ms,
                capacity_reason,
                next_sleep,
            } => {
                emit_queue_state(
                    session_id,
                    dialog_turn_id,
                    tool_id,
                    subagent_type,
                    DeepReviewQueueStatus::PausedByUser,
                    Some(capacity_reason),
                    1,
                    active_reviewers,
                    optional_reviewer_count,
                    Some(effective_parallel_instances),
                    queue_elapsed_ms,
                    conc_policy.max_queue_wait_seconds,
                )
                .await;
                sleep(next_sleep).await;
                continue;
            }
            runtime_task_execution::DeepReviewReviewerAdmissionQueueRuntimeStep::TryAdmit {
                queue_elapsed_ms,
                attempt,
                ..
            } => (queue_elapsed_ms, attempt),
        };

        let blocked_capacity_reason = match try_begin_reviewer_admission(
            dialog_turn_id,
            effective_parallel_instances,
            launch_batch_info,
        ) {
            Ok(Some(guard)) => {
                let active_reviewer_count = deep_review_active_reviewer_count(dialog_turn_id);
                record_deep_review_runtime_queue_wait(dialog_turn_id, queue_elapsed_ms);
                clear_deep_review_queue_control_for_tool(dialog_turn_id, tool_id);
                emit_queue_state(
                    session_id,
                    dialog_turn_id,
                    tool_id,
                    subagent_type,
                    DeepReviewQueueStatus::Running,
                    None,
                    0,
                    active_reviewer_count,
                    optional_reviewer_count,
                    Some(effective_parallel_instances),
                    queue_elapsed_ms,
                    conc_policy.max_queue_wait_seconds,
                )
                .await;
                return Ok(DeepReviewQueueWaitOutcome::Ready { guard });
            }
            Ok(None) => queue_runtime.local_capacity_reason(),
            Err(violation) if violation.code == "deep_review_launch_batch_blocked" => {
                DeepReviewCapacityQueueReason::LaunchBatchBlocked
            }
            Err(violation) => {
                return Err(BitFunError::tool(format!(
                    "DeepReview Task policy violation: {}",
                    violation.to_tool_error_message()
                )));
            }
        };

        match queue_runtime.after_blocked_attempt(
            admission_attempt,
            blocked_capacity_reason,
            active_reviewers,
            DEEP_REVIEW_QUEUE_POLL_INTERVAL,
        ) {
            runtime_task_execution::DeepReviewReviewerAdmissionQueueRuntimeBlockedStep::CapacityExpired {
                queue_elapsed_ms,
                capacity_reason,
                retry_after_seconds,
            } => {
                let effective_parallel_instances =
                    if capacity_reason == DeepReviewCapacityQueueReason::LaunchBatchBlocked {
                        effective_parallel_instances
                    } else {
                        record_deep_review_effective_concurrency_capacity_error(
                            dialog_turn_id,
                            conc_policy.max_parallel_instances,
                            capacity_reason,
                            retry_after_seconds.map(Duration::from_secs),
                        )
                        .effective_parallel_instances
                    };
                record_deep_review_runtime_queue_wait(dialog_turn_id, queue_elapsed_ms);
                record_deep_review_capacity_skip_for_reason(dialog_turn_id, capacity_reason);
                clear_deep_review_queue_control_for_tool(dialog_turn_id, tool_id);
                emit_queue_state(
                    session_id,
                    dialog_turn_id,
                    tool_id,
                    subagent_type,
                    DeepReviewQueueStatus::CapacitySkipped,
                    Some(capacity_reason),
                    0,
                    active_reviewers,
                    optional_reviewer_count,
                    Some(effective_parallel_instances),
                    queue_elapsed_ms,
                    conc_policy.max_queue_wait_seconds,
                )
                .await;
                return Ok(DeepReviewQueueWaitOutcome::Skipped {
                    queue_elapsed_ms,
                    skip_reason: DeepReviewQueueWaitSkipReason::QueueExpired,
                    capacity_reason,
                });
            }
            runtime_task_execution::DeepReviewReviewerAdmissionQueueRuntimeBlockedStep::Queued {
                queue_elapsed_ms,
                capacity_reason,
                next_sleep,
            } => {
                emit_queue_state(
                    session_id,
                    dialog_turn_id,
                    tool_id,
                    subagent_type,
                    DeepReviewQueueStatus::QueuedForCapacity,
                    Some(capacity_reason),
                    1,
                    active_reviewers,
                    optional_reviewer_count,
                    Some(effective_parallel_instances),
                    queue_elapsed_ms,
                    conc_policy.max_queue_wait_seconds,
                )
                .await;
                sleep(next_sleep).await;
            }
        }
    }
}
