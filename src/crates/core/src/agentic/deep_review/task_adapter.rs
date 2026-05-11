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
    classify_deep_review_capacity_error, clear_deep_review_queue_control_for_tool,
    deep_review_active_reviewer_count, deep_review_effective_parallel_instances,
    deep_review_max_retries_per_role, deep_review_queue_control_snapshot,
    record_deep_review_capacity_skip_for_reason,
    record_deep_review_effective_concurrency_capacity_error,
    record_deep_review_runtime_provider_capacity_queue,
    record_deep_review_runtime_provider_capacity_retry,
    record_deep_review_runtime_provider_capacity_retry_success,
    record_deep_review_runtime_queue_wait, try_begin_deep_review_active_reviewer,
    DeepReviewActiveReviewerGuard, DeepReviewCapacityFailFastReason,
    DeepReviewCapacityQueueDecision, DeepReviewCapacityQueueReason, DeepReviewConcurrencyPolicy,
    DeepReviewExecutionPolicy, DeepReviewPolicyViolation,
};
use crate::agentic::events::{
    DeepReviewQueueReason, DeepReviewQueueState, DeepReviewQueueStatus, ErrorCategory,
};
use crate::agentic::subagent_runtime::queue_timing::QueueWaitTimer;
use crate::util::errors::{BitFunError, BitFunResult};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[cfg(test)]
const DEEP_REVIEW_QUEUE_POLL_INTERVAL: Duration = Duration::from_millis(10);
#[cfg(not(test))]
const DEEP_REVIEW_QUEUE_POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeepReviewQueueWaitSkipReason {
    QueueExpired,
    UserCancelled,
    OptionalSkipped,
}

pub(crate) enum DeepReviewQueueWaitOutcome {
    Ready {
        guard: DeepReviewActiveReviewerGuard<'static>,
    },
    Skipped {
        queue_elapsed_ms: u64,
        skip_reason: DeepReviewQueueWaitSkipReason,
    },
}

pub(crate) enum DeepReviewProviderQueueWaitOutcome {
    ReadyToRetry {
        queue_elapsed_ms: u64,
    },
    Skipped {
        queue_elapsed_ms: u64,
        skip_reason: DeepReviewQueueWaitSkipReason,
    },
}

pub(crate) fn string_for_any_key<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

pub(crate) fn value_for_any_key<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| value.get(*key))
}

pub(crate) fn u64_for_any_key(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_u64))
}

pub(crate) fn string_array_for_any_key(
    value: &Value,
    keys: &[&str],
) -> Result<Vec<String>, DeepReviewPolicyViolation> {
    let Some(array) = value_for_any_key(value, keys).and_then(Value::as_array) else {
        return Err(DeepReviewPolicyViolation::new(
            "deep_review_retry_missing_coverage",
            format!("Retry coverage requires array field '{}'", keys[0]),
        ));
    };

    let mut result = Vec::with_capacity(array.len());
    for item in array {
        let Some(path) = item.as_str().map(str::trim).filter(|path| !path.is_empty()) else {
            return Err(DeepReviewPolicyViolation::new(
                "deep_review_retry_invalid_coverage",
                format!(
                    "Retry coverage field '{}' must contain non-empty strings",
                    keys[0]
                ),
            ));
        };
        result.push(path.to_string());
    }

    Ok(result)
}

pub(crate) fn work_packets_from_manifest(run_manifest: Option<&Value>) -> Option<&Vec<Value>> {
    run_manifest?
        .get("workPackets")
        .or_else(|| run_manifest?.get("work_packets"))?
        .as_array()
}

pub(crate) fn packet_id_from_description(description: Option<&str>) -> Option<String> {
    let description = description?;
    let start = description.find("[packet ")? + "[packet ".len();
    let packet_id = description[start..].split(']').next()?.trim();
    (!packet_id.is_empty()).then(|| packet_id.to_string())
}

pub(crate) fn packet_belongs_to_subagent(packet: &Value, subagent_type: &str) -> bool {
    string_for_any_key(
        packet,
        &["subagentId", "subagent_id", "subagentType", "subagent_type"],
    )
    .is_some_and(|value| value == subagent_type)
}

pub(crate) fn packet_id_for_manifest_packet(packet: &Value) -> Option<&str> {
    string_for_any_key(packet, &["packetId", "packet_id"])
}

pub(crate) fn deep_review_packet_id_for_cache(
    subagent_type: &str,
    description: Option<&str>,
    run_manifest: Option<&Value>,
) -> Option<String> {
    let packets = work_packets_from_manifest(run_manifest)?;

    if let Some(description_packet_id) = packet_id_from_description(description) {
        return packets
            .iter()
            .any(|packet| {
                packet_id_for_manifest_packet(packet)
                    .is_some_and(|packet_id| packet_id == description_packet_id)
                    && packet_belongs_to_subagent(packet, subagent_type)
            })
            .then_some(description_packet_id);
    }

    let mut matches = packets.iter().filter_map(|packet| {
        if packet_belongs_to_subagent(packet, subagent_type) {
            packet_id_for_manifest_packet(packet).map(str::to_string)
        } else {
            None
        }
    });
    let packet_id = matches.next()?;
    if matches.next().is_some() {
        None
    } else {
        Some(packet_id)
    }
}

pub(crate) fn attach_deep_review_cache(run_manifest: &mut Value, cache_value: Option<Value>) {
    if run_manifest.get("deepReviewCache").is_some() {
        return;
    }
    let Some(cache_value) = cache_value else {
        return;
    };
    if let Some(object) = run_manifest.as_object_mut() {
        object.insert("deepReviewCache".to_string(), cache_value);
    }
}

pub(crate) fn deep_review_retry_guidance_max_retries(
    effective_policy: Option<&DeepReviewExecutionPolicy>,
    dialog_turn_id: &str,
) -> usize {
    effective_policy
        .map(|policy| policy.max_retries_per_role)
        .unwrap_or_else(|| deep_review_max_retries_per_role(dialog_turn_id))
}

pub(crate) fn manifest_packet_by_id<'a>(
    run_manifest: Option<&'a Value>,
    packet_id: &str,
    subagent_type: &str,
) -> Option<&'a Value> {
    work_packets_from_manifest(run_manifest)?
        .iter()
        .find(|packet| {
            packet_id_for_manifest_packet(packet).is_some_and(|id| id == packet_id)
                && packet_belongs_to_subagent(packet, subagent_type)
        })
}

pub(crate) fn file_paths_for_manifest_packet(
    packet: &Value,
) -> Result<Vec<String>, DeepReviewPolicyViolation> {
    let Some(scope) = value_for_any_key(packet, &["assignedScope", "assigned_scope"]) else {
        return Err(DeepReviewPolicyViolation::new(
            "deep_review_retry_missing_packet_scope",
            "DeepReview retry source packet is missing assigned scope",
        ));
    };
    string_array_for_any_key(scope, &["files"])
}

pub(crate) fn is_retryable_capacity_reason(reason: &str) -> bool {
    matches!(
        reason,
        "local_concurrency_cap"
            | "provider_rate_limit"
            | "provider_concurrency_limit"
            | "retry_after"
            | "temporary_overload"
    )
}

pub(crate) fn ensure_deep_review_retry_coverage(
    input: &Value,
    subagent_type: &str,
    run_manifest: Option<&Value>,
) -> Result<Vec<String>, DeepReviewPolicyViolation> {
    let Some(coverage) = value_for_any_key(input, &["retry_coverage", "retryCoverage"]) else {
        return Err(DeepReviewPolicyViolation::new(
            "deep_review_retry_missing_coverage",
            "DeepReview retry requires structured retry_coverage metadata",
        ));
    };
    let packet_id = string_for_any_key(coverage, &["source_packet_id", "sourcePacketId"])
        .ok_or_else(|| {
            DeepReviewPolicyViolation::new(
                "deep_review_retry_missing_packet_id",
                "DeepReview retry coverage requires source_packet_id",
            )
        })?;
    let source_status = string_for_any_key(coverage, &["source_status", "sourceStatus"])
        .ok_or_else(|| {
            DeepReviewPolicyViolation::new(
                "deep_review_retry_missing_status",
                "DeepReview retry coverage requires source_status",
            )
        })?;
    match source_status {
        "partial_timeout" => {}
        "capacity_skipped" => {
            let capacity_reason =
                string_for_any_key(coverage, &["capacity_reason", "capacityReason"]).unwrap_or("");
            if !is_retryable_capacity_reason(capacity_reason) {
                return Err(DeepReviewPolicyViolation::new(
                    "deep_review_retry_non_retryable_status",
                    format!(
                        "DeepReview retry cannot redispatch non-transient capacity reason '{}'",
                        capacity_reason
                    ),
                ));
            }
        }
        other => {
            return Err(DeepReviewPolicyViolation::new(
                "deep_review_retry_non_retryable_status",
                format!(
                    "DeepReview retry only supports partial_timeout or transient capacity failures, not '{}'",
                    other
                ),
            ));
        }
    }

    let packet =
        manifest_packet_by_id(run_manifest, packet_id, subagent_type).ok_or_else(|| {
            DeepReviewPolicyViolation::new(
                "deep_review_retry_unknown_packet",
                format!(
                    "DeepReview retry source packet '{}' does not match reviewer '{}'",
                    packet_id, subagent_type
                ),
            )
        })?;
    let original_files = file_paths_for_manifest_packet(packet)?;
    ensure_deep_review_retry_timeout(input, packet)?;
    let retry_scope_files =
        string_array_for_any_key(coverage, &["retry_scope_files", "retryScopeFiles"])?;
    let covered_files = string_array_for_any_key(coverage, &["covered_files", "coveredFiles"])?;
    if retry_scope_files.is_empty() {
        return Err(DeepReviewPolicyViolation::new(
            "deep_review_retry_empty_scope",
            "DeepReview retry requires at least one retry_scope_files entry",
        ));
    }

    let original_file_set: HashSet<&str> = original_files.iter().map(String::as_str).collect();
    let mut retry_file_set = HashSet::new();
    for file in &retry_scope_files {
        if !retry_file_set.insert(file.as_str()) {
            return Err(DeepReviewPolicyViolation::new(
                "deep_review_retry_duplicate_scope_file",
                format!("DeepReview retry scope repeats file '{}'", file),
            ));
        }
        if !original_file_set.contains(file.as_str()) {
            return Err(DeepReviewPolicyViolation::new(
                "deep_review_retry_scope_outside_packet",
                format!(
                    "DeepReview retry file '{}' is outside source packet '{}'",
                    file, packet_id
                ),
            ));
        }
    }
    if retry_scope_files.len() >= original_files.len() {
        return Err(DeepReviewPolicyViolation::new(
            "deep_review_retry_scope_not_reduced",
            "DeepReview retry_scope_files must be smaller than the source packet scope",
        ));
    }

    for file in &covered_files {
        if !original_file_set.contains(file.as_str()) {
            return Err(DeepReviewPolicyViolation::new(
                "deep_review_retry_coverage_outside_packet",
                format!(
                    "DeepReview retry covered file '{}' is outside source packet '{}'",
                    file, packet_id
                ),
            ));
        }
        if retry_file_set.contains(file.as_str()) {
            return Err(DeepReviewPolicyViolation::new(
                "deep_review_retry_coverage_overlaps_scope",
                format!(
                    "DeepReview retry covered file '{}' cannot also be in retry_scope_files",
                    file
                ),
            ));
        }
    }

    Ok(retry_scope_files)
}

pub(crate) fn ensure_deep_review_retry_timeout(
    input: &Value,
    packet: &Value,
) -> Result<(), DeepReviewPolicyViolation> {
    let retry_timeout_seconds =
        u64_for_any_key(input, &["timeout_seconds", "timeoutSeconds"]).unwrap_or(0);
    if retry_timeout_seconds == 0 {
        return Err(DeepReviewPolicyViolation::new(
            "deep_review_retry_timeout_required",
            "DeepReview retry requires a positive timeout_seconds value",
        ));
    }

    let source_timeout_seconds =
        u64_for_any_key(packet, &["timeoutSeconds", "timeout_seconds"]).unwrap_or(0);
    if source_timeout_seconds > 0 && retry_timeout_seconds >= source_timeout_seconds {
        return Err(DeepReviewPolicyViolation::new(
            "deep_review_retry_timeout_not_reduced",
            format!(
                "DeepReview retry timeout_seconds ({}) must be lower than source timeout ({})",
                retry_timeout_seconds, source_timeout_seconds
            ),
        ));
    }

    Ok(())
}

pub(crate) fn prompt_with_deep_review_retry_scope(
    prompt: &str,
    retry_scope_files: &[String],
) -> String {
    let mut scoped_prompt = String::new();
    scoped_prompt.push_str("<deep_review_retry_scope>\n");
    scoped_prompt.push_str(
        "This is a bounded DeepReview retry. Review only the following retry_scope_files and treat any other files as background context only:\n",
    );
    for file in retry_scope_files {
        scoped_prompt.push_str("- ");
        scoped_prompt.push_str(file);
        scoped_prompt.push('\n');
    }
    scoped_prompt.push_str("</deep_review_retry_scope>\n\n");
    scoped_prompt.push_str(prompt);
    scoped_prompt
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
        DeepReviewCapacityQueueReason::TemporaryOverload => {
            DeepReviewQueueReason::TemporaryOverload
        }
    }
}

pub(crate) fn queue_reason_to_snake_case(reason: DeepReviewCapacityQueueReason) -> &'static str {
    reason.as_snake_case()
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
    let decision = classify_deep_review_capacity_error(
        code,
        message,
        extract_retry_after_seconds(&error_message),
    );
    if decision.queueable
        || decision.fail_fast_reason
            != Some(DeepReviewCapacityFailFastReason::DeterministicProviderError)
    {
        return decision;
    }

    match detail.category {
        ErrorCategory::RateLimit => DeepReviewCapacityQueueDecision::queueable(
            DeepReviewCapacityQueueReason::ProviderRateLimit,
            decision.retry_after_seconds,
        ),
        ErrorCategory::ProviderUnavailable => DeepReviewCapacityQueueDecision::queueable(
            DeepReviewCapacityQueueReason::TemporaryOverload,
            decision.retry_after_seconds,
        ),
        _ => decision,
    }
}

pub(crate) fn provider_capacity_queue_wait_seconds(
    decision: &DeepReviewCapacityQueueDecision,
    conc_policy: &DeepReviewConcurrencyPolicy,
) -> Option<u64> {
    if !decision.queueable || conc_policy.max_queue_wait_seconds == 0 {
        return None;
    }

    match decision.reason? {
        DeepReviewCapacityQueueReason::ProviderRateLimit
        | DeepReviewCapacityQueueReason::ProviderConcurrencyLimit
        | DeepReviewCapacityQueueReason::RetryAfter
        | DeepReviewCapacityQueueReason::TemporaryOverload => {}
        DeepReviewCapacityQueueReason::LocalConcurrencyCap => return None,
    }

    Some(
        decision
            .retry_after_seconds
            .unwrap_or(conc_policy.max_queue_wait_seconds)
            .min(conc_policy.max_queue_wait_seconds),
    )
    .filter(|seconds| *seconds > 0)
}

pub(crate) fn capacity_skip_result_for_provider_reason(
    reason: DeepReviewCapacityQueueReason,
    dialog_turn_id: &str,
    subagent_type: &str,
    conc_policy: &DeepReviewConcurrencyPolicy,
    duration_ms: u128,
) -> (Value, String) {
    capacity_skip_result_for_provider_queue_outcome(
        reason,
        dialog_turn_id,
        subagent_type,
        conc_policy,
        duration_ms,
        0,
        None,
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

    let duration_ms = u64::try_from(duration_ms).unwrap_or(u64::MAX);
    let reason_code = queue_reason_to_snake_case(reason);
    let queue_skip_reason = match terminal_skip_reason {
        Some(DeepReviewQueueWaitSkipReason::UserCancelled) => "user_cancelled",
        Some(DeepReviewQueueWaitSkipReason::OptionalSkipped) => "optional_skipped",
        Some(DeepReviewQueueWaitSkipReason::QueueExpired) | None => reason_code,
    };
    let assistant_message = match terminal_skip_reason {
        Some(DeepReviewQueueWaitSkipReason::UserCancelled) => format!(
            "Subagent '{}' was skipped because the DeepReview provider capacity queue was cancelled by the user.\n<queue_result status=\"capacity_skipped\" reason=\"user_cancelled\" queue_elapsed_ms=\"{}\" />",
            subagent_type, queue_elapsed_ms
        ),
        Some(DeepReviewQueueWaitSkipReason::OptionalSkipped) => format!(
            "Subagent '{}' was skipped because optional DeepReview provider capacity retries were skipped by the user.\n<queue_result status=\"capacity_skipped\" reason=\"optional_skipped\" queue_elapsed_ms=\"{}\" />",
            subagent_type, queue_elapsed_ms
        ),
        Some(DeepReviewQueueWaitSkipReason::QueueExpired) | None => format!(
            "Subagent '{}' was skipped because the provider reported transient DeepReview capacity pressure.\n<queue_result status=\"capacity_skipped\" reason=\"{}\" queue_elapsed_ms=\"{}\" />",
            subagent_type, reason_code, queue_elapsed_ms
        ),
    };
    let data = json!({
        "duration": duration_ms,
        "status": "capacity_skipped",
        "queue_elapsed_ms": queue_elapsed_ms,
        "max_queue_wait_seconds": conc_policy.max_queue_wait_seconds,
        "queue_skip_reason": queue_skip_reason,
        "provider_capacity_reason": reason_code,
        "effective_parallel_instances": snapshot.effective_parallel_instances
    });

    (data, assistant_message)
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
    let mut queue_timer = QueueWaitTimer::start(Instant::now());
    let max_wait = Duration::from_secs(max_wait_seconds);
    let optional_reviewer_count = is_optional_reviewer.then_some(1);

    record_deep_review_runtime_provider_capacity_queue(dialog_turn_id, reason);

    loop {
        let now = Instant::now();
        let queue_snapshot = queue_timer.snapshot(now);
        let queue_elapsed = queue_snapshot.queue_elapsed;
        let queue_elapsed_ms = queue_snapshot.queue_elapsed_ms;
        let active_reviewers = deep_review_active_reviewer_count(dialog_turn_id);
        let effective_parallel_instances = deep_review_effective_parallel_instances(
            dialog_turn_id,
            conc_policy.max_parallel_instances,
        );
        let control_snapshot = deep_review_queue_control_snapshot(dialog_turn_id, tool_id);

        if control_snapshot.cancelled || (is_optional_reviewer && control_snapshot.skip_optional) {
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
                conc_policy.max_queue_wait_seconds,
            )
            .await;
            return DeepReviewProviderQueueWaitOutcome::Skipped {
                queue_elapsed_ms,
                skip_reason: if control_snapshot.cancelled {
                    DeepReviewQueueWaitSkipReason::UserCancelled
                } else {
                    DeepReviewQueueWaitSkipReason::OptionalSkipped
                },
            };
        }

        if control_snapshot.paused {
            queue_timer.pause(now);
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
                conc_policy.max_queue_wait_seconds,
            )
            .await;
            sleep(DEEP_REVIEW_QUEUE_POLL_INTERVAL).await;
            continue;
        }

        queue_timer.continue_now(now);

        if queue_snapshot.is_expired(max_wait) {
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
                conc_policy.max_queue_wait_seconds,
            )
            .await;
            return DeepReviewProviderQueueWaitOutcome::ReadyToRetry { queue_elapsed_ms };
        }

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
            conc_policy.max_queue_wait_seconds,
        )
        .await;

        let remaining = max_wait.saturating_sub(queue_elapsed);
        sleep(DEEP_REVIEW_QUEUE_POLL_INTERVAL.min(remaining)).await;
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

pub(crate) async fn wait_for_reviewer_capacity(
    session_id: &str,
    dialog_turn_id: &str,
    tool_id: &str,
    subagent_type: &str,
    conc_policy: &DeepReviewConcurrencyPolicy,
    is_optional_reviewer: bool,
) -> BitFunResult<DeepReviewQueueWaitOutcome> {
    let decision = classify_deep_review_capacity_error(
        "deep_review_concurrency_cap_reached",
        "Maximum parallel reviewer instances reached",
        None,
    );
    let reason = decision
        .reason
        .unwrap_or(DeepReviewCapacityQueueReason::LocalConcurrencyCap);
    let mut queue_timer = QueueWaitTimer::start(Instant::now());
    let max_wait = Duration::from_secs(conc_policy.max_queue_wait_seconds);
    let optional_reviewer_count = is_optional_reviewer.then_some(1);

    loop {
        let now = Instant::now();
        let queue_snapshot = queue_timer.snapshot(now);
        let queue_elapsed = queue_snapshot.queue_elapsed;
        let queue_elapsed_ms = queue_snapshot.queue_elapsed_ms;
        let active_reviewers = deep_review_active_reviewer_count(dialog_turn_id);
        let effective_parallel_instances = deep_review_effective_parallel_instances(
            dialog_turn_id,
            conc_policy.max_parallel_instances,
        );

        let control_snapshot = deep_review_queue_control_snapshot(dialog_turn_id, tool_id);
        if control_snapshot.cancelled || (is_optional_reviewer && control_snapshot.skip_optional) {
            record_deep_review_runtime_queue_wait(dialog_turn_id, queue_elapsed_ms);
            record_deep_review_capacity_skip_for_reason(dialog_turn_id, reason);
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
                conc_policy.max_queue_wait_seconds,
            )
            .await;
            return Ok(DeepReviewQueueWaitOutcome::Skipped {
                queue_elapsed_ms,
                skip_reason: if control_snapshot.cancelled {
                    DeepReviewQueueWaitSkipReason::UserCancelled
                } else {
                    DeepReviewQueueWaitSkipReason::OptionalSkipped
                },
            });
        }

        if control_snapshot.paused {
            queue_timer.pause(now);
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
                conc_policy.max_queue_wait_seconds,
            )
            .await;
            sleep(DEEP_REVIEW_QUEUE_POLL_INTERVAL).await;
            continue;
        }

        queue_timer.continue_now(now);

        if let Some(guard) =
            try_begin_deep_review_active_reviewer(dialog_turn_id, effective_parallel_instances)
        {
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

        if queue_snapshot.is_expired(max_wait) {
            let snapshot = record_deep_review_effective_concurrency_capacity_error(
                dialog_turn_id,
                conc_policy.max_parallel_instances,
                reason,
                decision.retry_after_seconds.map(Duration::from_secs),
            );
            record_deep_review_runtime_queue_wait(dialog_turn_id, queue_elapsed_ms);
            record_deep_review_capacity_skip_for_reason(dialog_turn_id, reason);
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
                Some(snapshot.effective_parallel_instances),
                queue_elapsed_ms,
                conc_policy.max_queue_wait_seconds,
            )
            .await;
            return Ok(DeepReviewQueueWaitOutcome::Skipped {
                queue_elapsed_ms,
                skip_reason: DeepReviewQueueWaitSkipReason::QueueExpired,
            });
        }

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
            conc_policy.max_queue_wait_seconds,
        )
        .await;

        let remaining = max_wait.saturating_sub(queue_elapsed);
        sleep(DEEP_REVIEW_QUEUE_POLL_INTERVAL.min(remaining)).await;
    }
}
