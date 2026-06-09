//! Provider-neutral Deep Review task execution decisions.
//!
//! This module owns manifest packet matching, bounded retry validation,
//! provider-capacity retry timing, and capacity-skipped presentation facts.
//! Product assembly/core keeps concrete task launch, event emission, queue
//! sleeping, and runtime state mutation.

use super::{
    DeepReviewCapacityQueueDecision, DeepReviewCapacityQueueReason, DeepReviewConcurrencyPolicy,
    DeepReviewPolicyViolation,
};
use serde_json::{json, Value};
use std::collections::HashSet;

pub const DEEP_REVIEW_PROVIDER_CAPACITY_MAX_RETRY_ATTEMPTS: usize = 3;
const DEEP_REVIEW_PROVIDER_CAPACITY_BACKOFF_MULTIPLIER: u64 = 3;
const DEEP_REVIEW_PROVIDER_CAPACITY_MAX_BACKOFF_SECONDS: u64 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepReviewQueueWaitSkipReason {
    QueueExpired,
    UserCancelled,
    OptionalSkipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepReviewLaunchBatchInfo {
    pub packet_id: Option<String>,
    pub launch_batch: u64,
}

fn string_for_any_key<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

fn value_for_any_key<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| value.get(*key))
}

fn u64_for_any_key(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_u64))
}

fn string_array_for_any_key(
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

fn work_packets_from_manifest(run_manifest: Option<&Value>) -> Option<&Vec<Value>> {
    run_manifest?
        .get("workPackets")
        .or_else(|| run_manifest?.get("work_packets"))?
        .as_array()
}

fn packet_id_from_description(description: Option<&str>) -> Option<String> {
    let description = description?;
    let start = description.find("[packet ")? + "[packet ".len();
    let packet_id = description[start..].split(']').next()?.trim();
    (!packet_id.is_empty()).then(|| packet_id.to_string())
}

fn packet_belongs_to_subagent(packet: &Value, subagent_type: &str) -> bool {
    string_for_any_key(
        packet,
        &["subagentId", "subagent_id", "subagentType", "subagent_type"],
    )
    .is_some_and(|value| value == subagent_type)
}

fn packet_id_for_manifest_packet(packet: &Value) -> Option<&str> {
    string_for_any_key(packet, &["packetId", "packet_id"])
}

pub fn deep_review_packet_id_for_cache(
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

pub fn attach_deep_review_cache(run_manifest: &mut Value, cache_value: Option<Value>) {
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

fn manifest_packet_by_id<'a>(
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

fn launch_batch_for_manifest_packet(packet: &Value) -> Option<u64> {
    u64_for_any_key(packet, &["launchBatch", "launch_batch"])
        .filter(|launch_batch| *launch_batch > 0)
}

pub fn deep_review_launch_batch_for_task(
    subagent_type: &str,
    description: Option<&str>,
    run_manifest: Option<&Value>,
) -> Option<DeepReviewLaunchBatchInfo> {
    let packet_id = deep_review_packet_id_for_cache(subagent_type, description, run_manifest)?;
    let packet = manifest_packet_by_id(run_manifest, &packet_id, subagent_type)?;
    let launch_batch = launch_batch_for_manifest_packet(packet)?;

    Some(DeepReviewLaunchBatchInfo {
        packet_id: Some(packet_id),
        launch_batch,
    })
}

fn file_paths_for_manifest_packet(
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

fn is_retryable_capacity_reason(reason: &str) -> bool {
    matches!(
        reason,
        "local_concurrency_cap"
            | "launch_batch_blocked"
            | "provider_rate_limit"
            | "provider_concurrency_limit"
            | "retry_after"
            | "temporary_overload"
    )
}

pub fn ensure_deep_review_retry_coverage(
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

fn ensure_deep_review_retry_timeout(
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

pub fn prompt_with_deep_review_retry_scope(prompt: &str, retry_scope_files: &[String]) -> String {
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

pub fn provider_capacity_queue_wait_seconds(
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
        DeepReviewCapacityQueueReason::LocalConcurrencyCap
        | DeepReviewCapacityQueueReason::LaunchBatchBlocked => return None,
    }

    Some(
        decision
            .retry_after_seconds
            .unwrap_or(conc_policy.max_queue_wait_seconds)
            .min(conc_policy.max_queue_wait_seconds),
    )
    .filter(|seconds| *seconds > 0)
}

pub fn provider_capacity_queue_wait_seconds_for_attempt(
    decision: &DeepReviewCapacityQueueDecision,
    conc_policy: &DeepReviewConcurrencyPolicy,
    retry_attempt_index: usize,
) -> Option<u64> {
    let base_wait_seconds = provider_capacity_queue_wait_seconds(decision, conc_policy)?;
    if decision.retry_after_seconds.is_some() {
        return Some(base_wait_seconds);
    }

    let multiplier = DEEP_REVIEW_PROVIDER_CAPACITY_BACKOFF_MULTIPLIER.saturating_pow(
        u32::try_from(retry_attempt_index)
            .unwrap_or(u32::MAX)
            .min(8),
    );
    Some(
        base_wait_seconds
            .saturating_mul(multiplier)
            .min(DEEP_REVIEW_PROVIDER_CAPACITY_MAX_BACKOFF_SECONDS),
    )
    .filter(|seconds| *seconds > 0)
}

pub fn provider_capacity_wait_can_wake_on_active_reviewer_release(
    reason: DeepReviewCapacityQueueReason,
) -> bool {
    matches!(
        reason,
        DeepReviewCapacityQueueReason::ProviderConcurrencyLimit
            | DeepReviewCapacityQueueReason::TemporaryOverload
    )
}

pub fn capacity_skip_result_for_local_queue_outcome(
    subagent_type: &str,
    conc_policy: &DeepReviewConcurrencyPolicy,
    capacity_reason: DeepReviewCapacityQueueReason,
    skip_reason: DeepReviewQueueWaitSkipReason,
    queue_elapsed_ms: u64,
    duration_ms: u128,
    effective_parallel_instances: usize,
) -> (Value, String) {
    let queue_skip_reason = match skip_reason {
        DeepReviewQueueWaitSkipReason::QueueExpired => "queue_expired",
        DeepReviewQueueWaitSkipReason::UserCancelled => "user_cancelled",
        DeepReviewQueueWaitSkipReason::OptionalSkipped => "optional_skipped",
    };
    let capacity_reason_code = capacity_reason.as_snake_case();
    let assistant_message = match skip_reason {
        DeepReviewQueueWaitSkipReason::QueueExpired => {
            let reason_message = match capacity_reason {
                DeepReviewCapacityQueueReason::LaunchBatchBlocked => {
                    "the previous launch batch did not finish before the queue wait limit"
                }
                DeepReviewCapacityQueueReason::LocalConcurrencyCap => {
                    "the local reviewer capacity queue reached its maximum wait"
                }
                _ => "the DeepReview capacity queue reached its maximum wait",
            };
            let recommended_action = match capacity_reason {
                DeepReviewCapacityQueueReason::LaunchBatchBlocked => {
                    "Wait for the earlier reviewer batch to finish or cancel stuck queued reviewers, then retry this packet with a lower max parallel reviewer setting if it repeats."
                }
                _ => {
                    "Run the review again with a lower max parallel reviewer setting or wait for active reviewers to finish."
                }
            };
            format!(
                "Subagent '{}' was skipped because {} ({}s). Recommended action: {}\n<queue_result status=\"capacity_skipped\" reason=\"{}\" queue_elapsed_ms=\"{}\" />",
                subagent_type,
                reason_message,
                conc_policy.max_queue_wait_seconds,
                recommended_action,
                capacity_reason_code,
                queue_elapsed_ms
            )
        }
        DeepReviewQueueWaitSkipReason::UserCancelled => format!(
            "Subagent '{}' was skipped because the DeepReview capacity queue was cancelled by the user.\n<queue_result status=\"capacity_skipped\" reason=\"user_cancelled\" queue_elapsed_ms=\"{}\" />",
            subagent_type, queue_elapsed_ms
        ),
        DeepReviewQueueWaitSkipReason::OptionalSkipped => format!(
            "Subagent '{}' was skipped because optional DeepReview queued reviewers were skipped by the user.\n<queue_result status=\"capacity_skipped\" reason=\"optional_skipped\" queue_elapsed_ms=\"{}\" />",
            subagent_type, queue_elapsed_ms
        ),
    };

    let data = json!({
        "duration": u64::try_from(duration_ms).unwrap_or(u64::MAX),
        "status": "capacity_skipped",
        "queue_elapsed_ms": queue_elapsed_ms,
        "max_queue_wait_seconds": conc_policy.max_queue_wait_seconds,
        "queue_skip_reason": queue_skip_reason,
        "capacity_reason": capacity_reason_code,
        "effective_parallel_instances": effective_parallel_instances
    });

    (data, assistant_message)
}

pub fn capacity_skip_result_for_provider_queue_outcome(
    reason: DeepReviewCapacityQueueReason,
    subagent_type: &str,
    conc_policy: &DeepReviewConcurrencyPolicy,
    duration_ms: u128,
    queue_elapsed_ms: u64,
    terminal_skip_reason: Option<DeepReviewQueueWaitSkipReason>,
    effective_parallel_instances: usize,
) -> (Value, String) {
    let duration_ms = u64::try_from(duration_ms).unwrap_or(u64::MAX);
    let reason_code = reason.as_snake_case();
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
        "effective_parallel_instances": effective_parallel_instances
    });

    (data, assistant_message)
}
