//! Provider-neutral Deep Review task execution decisions.
//!
//! This module owns manifest packet matching, bounded retry validation,
//! provider-capacity retry timing, provider queue step decisions, and
//! TaskTool presentation facts. Product assembly/core keeps concrete
//! task launch, event emission, queue sleeping, and runtime state mutation.

use super::incremental_cache::DeepReviewIncrementalCache;
use super::{
    classify_deep_review_capacity_error, DeepReviewCapacityFailFastReason,
    DeepReviewCapacityQueueDecision, DeepReviewCapacityQueueReason, DeepReviewConcurrencyPolicy,
    DeepReviewPolicyViolation, DeepReviewQueueControlSnapshot, DeepReviewSubagentRole,
};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::time::{Duration, Instant};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepReviewIncrementalCacheHit {
    pub packet_id: String,
    pub cached_output: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepReviewProviderCapacityErrorCategory {
    RateLimit,
    ProviderUnavailable,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeepReviewProviderCapacityErrorFacts<'a> {
    pub provider_code: &'a str,
    pub provider_message: &'a str,
    pub retry_after_seconds: Option<u64>,
    pub category: DeepReviewProviderCapacityErrorCategory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepReviewProviderCapacityQueueStepFacts {
    pub reason: DeepReviewCapacityQueueReason,
    pub queue_expired: bool,
    pub initial_active_reviewer_count: usize,
    pub active_reviewer_count: usize,
    pub control_snapshot: DeepReviewQueueControlSnapshot,
    pub is_optional_reviewer: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepReviewQueueControlStepDecision {
    Skipped {
        skip_reason: DeepReviewQueueWaitSkipReason,
    },
    Paused,
    Continue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepReviewProviderCapacityQueueStepDecision {
    Skipped {
        skip_reason: DeepReviewQueueWaitSkipReason,
    },
    Paused,
    ReadyToRetry {
        early_capacity_probe: bool,
    },
    Queued,
}

#[derive(Debug, Clone)]
pub struct DeepReviewProviderCapacityQueueRuntime {
    reason: DeepReviewCapacityQueueReason,
    timer: QueueWaitTimer,
    max_wait: Duration,
    initial_active_reviewer_count: usize,
    is_optional_reviewer: bool,
}

impl DeepReviewProviderCapacityQueueRuntime {
    pub fn start(
        now: Instant,
        reason: DeepReviewCapacityQueueReason,
        max_wait: Duration,
        initial_active_reviewer_count: usize,
        is_optional_reviewer: bool,
    ) -> Self {
        Self {
            reason,
            timer: QueueWaitTimer::start(now),
            max_wait,
            initial_active_reviewer_count,
            is_optional_reviewer,
        }
    }

    pub fn step(
        &mut self,
        input: DeepReviewProviderCapacityQueueRuntimeInput,
    ) -> DeepReviewProviderCapacityQueueRuntimeStep {
        let queue_snapshot = self.timer.snapshot(input.now);
        let queue_elapsed = queue_snapshot.queue_elapsed;
        let queue_elapsed_ms = queue_snapshot.queue_elapsed_ms;
        let queue_decision =
            decide_provider_capacity_queue_step(DeepReviewProviderCapacityQueueStepFacts {
                reason: self.reason,
                queue_expired: queue_snapshot.is_expired(self.max_wait),
                initial_active_reviewer_count: self.initial_active_reviewer_count,
                active_reviewer_count: input.active_reviewer_count,
                control_snapshot: input.control_snapshot,
                is_optional_reviewer: self.is_optional_reviewer,
            });

        match queue_decision {
            DeepReviewProviderCapacityQueueStepDecision::Skipped { skip_reason } => {
                DeepReviewProviderCapacityQueueRuntimeStep::Skipped {
                    queue_elapsed_ms,
                    skip_reason,
                }
            }
            DeepReviewProviderCapacityQueueStepDecision::Paused => {
                self.timer.pause(input.now);
                DeepReviewProviderCapacityQueueRuntimeStep::Paused {
                    queue_elapsed_ms,
                    next_sleep: input.poll_interval,
                }
            }
            DeepReviewProviderCapacityQueueStepDecision::ReadyToRetry {
                early_capacity_probe,
            } => {
                self.timer.continue_now(input.now);
                DeepReviewProviderCapacityQueueRuntimeStep::ReadyToRetry {
                    queue_elapsed_ms,
                    early_capacity_probe,
                }
            }
            DeepReviewProviderCapacityQueueStepDecision::Queued => {
                self.timer.continue_now(input.now);
                DeepReviewProviderCapacityQueueRuntimeStep::Queued {
                    queue_elapsed_ms,
                    next_sleep: input
                        .poll_interval
                        .min(self.max_wait.saturating_sub(queue_elapsed)),
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepReviewProviderCapacityQueueRuntimeInput {
    pub now: Instant,
    pub active_reviewer_count: usize,
    pub control_snapshot: DeepReviewQueueControlSnapshot,
    pub poll_interval: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepReviewProviderCapacityQueueRuntimeStep {
    Skipped {
        queue_elapsed_ms: u64,
        skip_reason: DeepReviewQueueWaitSkipReason,
    },
    Paused {
        queue_elapsed_ms: u64,
        next_sleep: Duration,
    },
    ReadyToRetry {
        queue_elapsed_ms: u64,
        early_capacity_probe: bool,
    },
    Queued {
        queue_elapsed_ms: u64,
        next_sleep: Duration,
    },
}

#[derive(Debug, Clone)]
pub struct DeepReviewReviewerAdmissionQueueRuntime {
    timer: QueueWaitTimer,
    max_wait: Duration,
    local_capacity_reason: DeepReviewCapacityQueueReason,
    retry_after_seconds: Option<u64>,
    last_wait_reason: DeepReviewCapacityQueueReason,
    last_queue_elapsed: Duration,
    is_optional_reviewer: bool,
}

impl DeepReviewReviewerAdmissionQueueRuntime {
    pub fn start(
        now: Instant,
        local_capacity_reason: DeepReviewCapacityQueueReason,
        max_wait: Duration,
        retry_after_seconds: Option<u64>,
        is_optional_reviewer: bool,
    ) -> Self {
        Self {
            timer: QueueWaitTimer::start(now),
            max_wait,
            local_capacity_reason,
            retry_after_seconds,
            last_wait_reason: local_capacity_reason,
            last_queue_elapsed: Duration::ZERO,
            is_optional_reviewer,
        }
    }

    pub fn begin_step(
        &mut self,
        input: DeepReviewReviewerAdmissionQueueRuntimeInput,
    ) -> DeepReviewReviewerAdmissionQueueRuntimeStep {
        let queue_snapshot = self.timer.snapshot(input.now);
        self.last_queue_elapsed = queue_snapshot.queue_elapsed;
        let queue_elapsed_ms = queue_snapshot.queue_elapsed_ms;
        let current_reason = self.last_wait_reason;

        match decide_queue_control_step(&input.control_snapshot, self.is_optional_reviewer) {
            DeepReviewQueueControlStepDecision::Skipped { skip_reason } => {
                DeepReviewReviewerAdmissionQueueRuntimeStep::Skipped {
                    queue_elapsed_ms,
                    skip_reason,
                    capacity_reason: current_reason,
                }
            }
            DeepReviewQueueControlStepDecision::Paused => {
                self.timer.pause(input.now);
                DeepReviewReviewerAdmissionQueueRuntimeStep::Paused {
                    queue_elapsed_ms,
                    capacity_reason: current_reason,
                    next_sleep: input.poll_interval,
                }
            }
            DeepReviewQueueControlStepDecision::Continue => {
                self.timer.continue_now(input.now);
                DeepReviewReviewerAdmissionQueueRuntimeStep::TryAdmit {
                    queue_elapsed_ms,
                    attempt: DeepReviewReviewerAdmissionTryAdmit {
                        queue_elapsed_ms,
                        queue_expired: queue_snapshot.is_expired(self.max_wait),
                    },
                    capacity_reason: current_reason,
                }
            }
        }
    }

    pub fn after_blocked_attempt(
        &mut self,
        attempt: DeepReviewReviewerAdmissionTryAdmit,
        capacity_reason: DeepReviewCapacityQueueReason,
        active_reviewer_count: usize,
        poll_interval: Duration,
    ) -> DeepReviewReviewerAdmissionQueueRuntimeBlockedStep {
        self.last_wait_reason = capacity_reason;

        match decide_blocked_reviewer_admission_queue_step(
            DeepReviewBlockedReviewerAdmissionQueueStepFacts {
                capacity_reason,
                queue_expired: attempt.queue_expired,
                active_reviewer_count,
            },
        ) {
            DeepReviewBlockedReviewerAdmissionQueueStepDecision::CapacityExpired {
                capacity_reason,
            } => DeepReviewReviewerAdmissionQueueRuntimeBlockedStep::CapacityExpired {
                queue_elapsed_ms: attempt.queue_elapsed_ms,
                capacity_reason,
                retry_after_seconds: (capacity_reason
                    != DeepReviewCapacityQueueReason::LaunchBatchBlocked)
                    .then_some(self.retry_after_seconds)
                    .flatten(),
            },
            DeepReviewBlockedReviewerAdmissionQueueStepDecision::Queued { capacity_reason } => {
                let next_sleep = if attempt.queue_expired {
                    poll_interval
                } else {
                    poll_interval.min(self.max_wait.saturating_sub(self.last_queue_elapsed))
                };
                DeepReviewReviewerAdmissionQueueRuntimeBlockedStep::Queued {
                    queue_elapsed_ms: attempt.queue_elapsed_ms,
                    capacity_reason,
                    next_sleep,
                }
            }
        }
    }

    pub fn local_capacity_reason(&self) -> DeepReviewCapacityQueueReason {
        self.local_capacity_reason
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepReviewReviewerAdmissionQueueRuntimeInput {
    pub now: Instant,
    pub control_snapshot: DeepReviewQueueControlSnapshot,
    pub poll_interval: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepReviewReviewerAdmissionQueueRuntimeStep {
    Skipped {
        queue_elapsed_ms: u64,
        skip_reason: DeepReviewQueueWaitSkipReason,
        capacity_reason: DeepReviewCapacityQueueReason,
    },
    Paused {
        queue_elapsed_ms: u64,
        capacity_reason: DeepReviewCapacityQueueReason,
        next_sleep: Duration,
    },
    TryAdmit {
        queue_elapsed_ms: u64,
        attempt: DeepReviewReviewerAdmissionTryAdmit,
        capacity_reason: DeepReviewCapacityQueueReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeepReviewReviewerAdmissionTryAdmit {
    queue_elapsed_ms: u64,
    queue_expired: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepReviewReviewerAdmissionQueueRuntimeBlockedStep {
    CapacityExpired {
        queue_elapsed_ms: u64,
        capacity_reason: DeepReviewCapacityQueueReason,
        retry_after_seconds: Option<u64>,
    },
    Queued {
        queue_elapsed_ms: u64,
        capacity_reason: DeepReviewCapacityQueueReason,
        next_sleep: Duration,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeepReviewTaskCompletionResultInput<'a> {
    pub delegate_target_label: &'a str,
    pub result_text: &'a str,
    pub context_mode: &'a str,
    pub duration_ms: u128,
    pub is_partial_timeout: bool,
    pub reason: Option<&'a str>,
    pub ledger_event_id: Option<&'a str>,
    pub retry_hint: &'a str,
}

pub fn deep_review_task_completion_result(
    input: DeepReviewTaskCompletionResultInput<'_>,
) -> (Value, String) {
    let status = if input.is_partial_timeout {
        "partial_timeout"
    } else {
        "completed"
    };
    let assistant_message = if input.is_partial_timeout {
        format!(
            "{} timed out with partial result:\n<partial_result status=\"partial_timeout\">\n{}\n</partial_result>{}",
            input.delegate_target_label, input.result_text, input.retry_hint
        )
    } else {
        format!(
            "{} completed successfully with result:\n<result>\n{}\n</result>",
            input.delegate_target_label, input.result_text
        )
    };
    let mut data = json!({
        "duration": input.duration_ms,
        "context_mode": input.context_mode,
        "status": status
    });

    if input.is_partial_timeout {
        data["partial_output"] = json!(input.result_text);
        if let Some(reason) = input.reason {
            data["reason"] = json!(reason);
        }
        if let Some(event_id) = input.ledger_event_id {
            data["ledger_event_id"] = json!(event_id);
        }
    }

    (data, assistant_message)
}

pub fn deep_review_cancelled_reviewer_result(
    subagent_type: &str,
    reason: &str,
    duration_ms: u128,
) -> (Value, String) {
    let duration = u64::try_from(duration_ms).unwrap_or(u64::MAX);
    let reason = if reason.trim().is_empty() {
        "Subagent task was cancelled"
    } else {
        reason.trim()
    };
    let assistant_message = format!(
        "Subagent '{}' was cancelled by the user.\n<result status=\"cancelled\" reason=\"user_cancelled\">Treat this reviewer as cancelled coverage, continue remaining reviewers when useful, and do not relaunch it automatically.</result>",
        subagent_type
    );

    let data = json!({
        "duration": duration,
        "status": "cancelled",
        "reason": reason,
    });

    (data, assistant_message)
}

pub fn should_emit_deep_review_retry_guidance(
    is_partial_timeout: bool,
    is_retry: bool,
    deep_review_subagent_role: Option<DeepReviewSubagentRole>,
) -> bool {
    is_partial_timeout
        && !is_retry
        && matches!(
            deep_review_subagent_role,
            Some(DeepReviewSubagentRole::Reviewer)
        )
}

pub fn deep_review_retry_guidance(retries_used: usize, max_retries: usize) -> String {
    if max_retries == 0 || retries_used >= max_retries {
        return String::new();
    }

    format!(
        "\n\n<retry_guidance>This reviewer timed out. You may retry with 'retry: true' only if you can provide retry_coverage with source_packet_id, source_status='partial_timeout', covered_files, and a smaller retry_scope_files list. Retries used: {}/{}.</retry_guidance>",
        retries_used, max_retries
    )
}

pub fn auto_retry_suppression_reason(code: &str) -> &'static str {
    match code {
        "deep_review_auto_retry_disabled" => "auto_retry_disabled",
        "deep_review_auto_retry_elapsed_guard_exceeded" => "elapsed_guard_exceeded",
        "deep_review_retry_budget_exhausted" => "budget_exhausted",
        "deep_review_retry_without_initial_attempt" => "without_initial_attempt",
        "deep_review_retry_missing_coverage" => "missing_coverage",
        "deep_review_retry_missing_packet_id" => "missing_coverage",
        "deep_review_retry_missing_status" => "missing_coverage",
        "deep_review_retry_non_retryable_status" => "non_retryable_status",
        "deep_review_retry_unknown_packet" => "unknown_packet",
        "deep_review_retry_missing_packet_scope" => "unknown_packet",
        "deep_review_retry_timeout_required" => "timeout_not_reduced",
        "deep_review_retry_timeout_not_reduced" => "timeout_not_reduced",
        "deep_review_retry_empty_scope" => "empty_scope",
        "deep_review_retry_scope_not_reduced" => "scope_not_reduced",
        _ => "invalid_coverage",
    }
}

pub fn ensure_deep_review_auto_retry_allowed(
    conc_policy: &DeepReviewConcurrencyPolicy,
    elapsed_seconds: Option<u64>,
) -> Result<(), DeepReviewPolicyViolation> {
    if !conc_policy.allow_bounded_auto_retry {
        return Err(DeepReviewPolicyViolation::new(
            "deep_review_auto_retry_disabled",
            "DeepReview bounded automatic retry is disabled by Review Team settings",
        ));
    }

    if let Some(elapsed_seconds) = elapsed_seconds {
        if elapsed_seconds > conc_policy.auto_retry_elapsed_guard_seconds {
            return Err(DeepReviewPolicyViolation::new(
                "deep_review_auto_retry_elapsed_guard_exceeded",
                format!(
                    "DeepReview automatic retry elapsed guard exceeded (elapsed: {}s, guard: {}s)",
                    elapsed_seconds, conc_policy.auto_retry_elapsed_guard_seconds
                ),
            ));
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeepReviewBlockedReviewerAdmissionQueueStepFacts {
    pub capacity_reason: DeepReviewCapacityQueueReason,
    pub queue_expired: bool,
    pub active_reviewer_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepReviewBlockedReviewerAdmissionQueueStepDecision {
    CapacityExpired {
        capacity_reason: DeepReviewCapacityQueueReason,
    },
    Queued {
        capacity_reason: DeepReviewCapacityQueueReason,
    },
}

#[derive(Debug, Clone)]
struct QueueWaitTimer {
    started_at: Instant,
    paused_since: Option<Instant>,
    paused_total: Duration,
}

impl QueueWaitTimer {
    fn start(now: Instant) -> Self {
        Self {
            started_at: now,
            paused_since: None,
            paused_total: Duration::ZERO,
        }
    }

    fn snapshot(&self, now: Instant) -> QueueWaitSnapshot {
        let active_pause = self
            .paused_since
            .map(|paused_at| now.saturating_duration_since(paused_at))
            .unwrap_or_default();
        let queue_elapsed = now
            .saturating_duration_since(self.started_at)
            .saturating_sub(self.paused_total)
            .saturating_sub(active_pause);

        QueueWaitSnapshot {
            queue_elapsed,
            queue_elapsed_ms: u64::try_from(queue_elapsed.as_millis()).unwrap_or(u64::MAX),
        }
    }

    fn pause(&mut self, now: Instant) {
        if self.paused_since.is_none() {
            self.paused_since = Some(now);
        }
    }

    fn continue_now(&mut self, now: Instant) {
        if let Some(paused_at) = self.paused_since.take() {
            self.paused_total += now.saturating_duration_since(paused_at);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QueueWaitSnapshot {
    queue_elapsed: Duration,
    queue_elapsed_ms: u64,
}

impl QueueWaitSnapshot {
    fn is_expired(self, max_wait: Duration) -> bool {
        self.queue_elapsed >= max_wait
    }
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

pub fn deep_review_incremental_cache_hit_for_task(
    subagent_type: &str,
    description: Option<&str>,
    run_manifest: Option<&Value>,
) -> Option<DeepReviewIncrementalCacheHit> {
    let manifest = run_manifest?;
    let cache_value = manifest.get("deepReviewCache")?;
    let cache = DeepReviewIncrementalCache::from_value(cache_value);
    if !cache.matches_manifest(manifest) {
        return None;
    }

    let packet_id = deep_review_packet_id_for_cache(subagent_type, description, Some(manifest))?;
    let cached_output = cache.get_packet(&packet_id)?.to_string();
    Some(DeepReviewIncrementalCacheHit {
        packet_id,
        cached_output,
    })
}

pub fn deep_review_incremental_cache_hit_result(
    subagent_type: &str,
    cache_hit: &DeepReviewIncrementalCacheHit,
) -> (Value, String) {
    (
        json!({ "cached": true, "packet_id": &cache_hit.packet_id }),
        format!(
            "Subagent '{}' result (from incremental review cache):\n<result source=\"cache\">\n{}\n</result>",
            subagent_type, cache_hit.cached_output
        ),
    )
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

#[derive(Debug, Clone, Default)]
pub struct DeepReviewProviderCapacityRetryRuntime {
    retry_attempts: usize,
    queue_elapsed_ms: u64,
    last_retry_reason: Option<DeepReviewCapacityQueueReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepReviewProviderCapacityRetryDecision {
    NotQueueable,
    WaitForCapacity {
        reason: DeepReviewCapacityQueueReason,
        max_wait_seconds: u64,
    },
    CapacitySkipped {
        reason: DeepReviewCapacityQueueReason,
        queue_elapsed_ms: u64,
    },
}

impl DeepReviewProviderCapacityRetryRuntime {
    pub fn decide_after_error(
        &self,
        decision: &DeepReviewCapacityQueueDecision,
        conc_policy: &DeepReviewConcurrencyPolicy,
    ) -> DeepReviewProviderCapacityRetryDecision {
        let Some(reason) = decision.queueable.then_some(decision.reason).flatten() else {
            return DeepReviewProviderCapacityRetryDecision::NotQueueable;
        };

        if self.retry_attempts >= DEEP_REVIEW_PROVIDER_CAPACITY_MAX_RETRY_ATTEMPTS {
            return DeepReviewProviderCapacityRetryDecision::CapacitySkipped {
                reason,
                queue_elapsed_ms: self.queue_elapsed_ms,
            };
        }

        match provider_capacity_queue_wait_seconds_for_attempt(
            decision,
            conc_policy,
            self.retry_attempts,
        ) {
            Some(max_wait_seconds) => DeepReviewProviderCapacityRetryDecision::WaitForCapacity {
                reason,
                max_wait_seconds,
            },
            None => DeepReviewProviderCapacityRetryDecision::CapacitySkipped {
                reason,
                queue_elapsed_ms: 0,
            },
        }
    }

    pub fn record_ready_to_retry(
        &mut self,
        reason: DeepReviewCapacityQueueReason,
        queue_elapsed_ms: u64,
        early_capacity_probe: bool,
    ) -> u64 {
        self.queue_elapsed_ms = self.queue_elapsed_ms.saturating_add(queue_elapsed_ms);
        self.last_retry_reason = Some(reason);
        if !early_capacity_probe {
            self.retry_attempts = self.retry_attempts.saturating_add(1);
        }
        self.queue_elapsed_ms
    }

    pub fn record_queue_skipped(&mut self, queue_elapsed_ms: u64) -> u64 {
        self.queue_elapsed_ms = self.queue_elapsed_ms.saturating_add(queue_elapsed_ms);
        self.queue_elapsed_ms
    }

    pub fn last_retry_reason(&self) -> Option<DeepReviewCapacityQueueReason> {
        self.last_retry_reason
    }
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

pub fn local_reviewer_capacity_queue_decision() -> DeepReviewCapacityQueueDecision {
    classify_deep_review_capacity_error(
        "deep_review_concurrency_cap_reached",
        "Maximum parallel reviewer instances reached",
        None,
    )
}

pub fn capacity_decision_for_provider_error_facts(
    facts: DeepReviewProviderCapacityErrorFacts<'_>,
) -> DeepReviewCapacityQueueDecision {
    let decision = classify_deep_review_capacity_error(
        facts.provider_code,
        facts.provider_message,
        facts.retry_after_seconds,
    );
    if decision.queueable
        || decision.fail_fast_reason
            != Some(DeepReviewCapacityFailFastReason::DeterministicProviderError)
    {
        return decision;
    }

    match facts.category {
        DeepReviewProviderCapacityErrorCategory::RateLimit => {
            DeepReviewCapacityQueueDecision::queueable(
                DeepReviewCapacityQueueReason::ProviderRateLimit,
                decision.retry_after_seconds,
            )
        }
        DeepReviewProviderCapacityErrorCategory::ProviderUnavailable => {
            DeepReviewCapacityQueueDecision::queueable(
                DeepReviewCapacityQueueReason::TemporaryOverload,
                decision.retry_after_seconds,
            )
        }
        DeepReviewProviderCapacityErrorCategory::Other => decision,
    }
}

pub fn decide_queue_control_step(
    control_snapshot: &DeepReviewQueueControlSnapshot,
    is_optional_reviewer: bool,
) -> DeepReviewQueueControlStepDecision {
    if control_snapshot.cancelled || (is_optional_reviewer && control_snapshot.skip_optional) {
        return DeepReviewQueueControlStepDecision::Skipped {
            skip_reason: if control_snapshot.cancelled {
                DeepReviewQueueWaitSkipReason::UserCancelled
            } else {
                DeepReviewQueueWaitSkipReason::OptionalSkipped
            },
        };
    }

    if control_snapshot.paused {
        return DeepReviewQueueControlStepDecision::Paused;
    }

    DeepReviewQueueControlStepDecision::Continue
}

pub fn decide_provider_capacity_queue_step(
    facts: DeepReviewProviderCapacityQueueStepFacts,
) -> DeepReviewProviderCapacityQueueStepDecision {
    match decide_queue_control_step(&facts.control_snapshot, facts.is_optional_reviewer) {
        DeepReviewQueueControlStepDecision::Skipped { skip_reason } => {
            return DeepReviewProviderCapacityQueueStepDecision::Skipped { skip_reason };
        }
        DeepReviewQueueControlStepDecision::Paused => {
            return DeepReviewProviderCapacityQueueStepDecision::Paused;
        }
        DeepReviewQueueControlStepDecision::Continue => {}
    }

    if facts.queue_expired {
        return DeepReviewProviderCapacityQueueStepDecision::ReadyToRetry {
            early_capacity_probe: false,
        };
    }

    if provider_capacity_wait_can_wake_on_active_reviewer_release(facts.reason)
        && facts.initial_active_reviewer_count > 0
        && facts.active_reviewer_count < facts.initial_active_reviewer_count
    {
        return DeepReviewProviderCapacityQueueStepDecision::ReadyToRetry {
            early_capacity_probe: true,
        };
    }

    DeepReviewProviderCapacityQueueStepDecision::Queued
}

pub fn decide_blocked_reviewer_admission_queue_step(
    facts: DeepReviewBlockedReviewerAdmissionQueueStepFacts,
) -> DeepReviewBlockedReviewerAdmissionQueueStepDecision {
    if facts.queue_expired && facts.active_reviewer_count == 0 {
        return DeepReviewBlockedReviewerAdmissionQueueStepDecision::CapacityExpired {
            capacity_reason: facts.capacity_reason,
        };
    }

    DeepReviewBlockedReviewerAdmissionQueueStepDecision::Queued {
        capacity_reason: facts.capacity_reason,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn control_snapshot(
        paused: bool,
        cancelled: bool,
        skip_optional: bool,
    ) -> DeepReviewQueueControlSnapshot {
        DeepReviewQueueControlSnapshot {
            paused,
            cancelled,
            skip_optional,
        }
    }

    fn provider_queue_facts(
        reason: DeepReviewCapacityQueueReason,
    ) -> DeepReviewProviderCapacityQueueStepFacts {
        DeepReviewProviderCapacityQueueStepFacts {
            reason,
            queue_expired: false,
            initial_active_reviewer_count: 2,
            active_reviewer_count: 2,
            control_snapshot: control_snapshot(false, false, false),
            is_optional_reviewer: false,
        }
    }

    #[test]
    fn provider_error_decision_uses_structured_category_fallback() {
        let rate_limited =
            capacity_decision_for_provider_error_facts(DeepReviewProviderCapacityErrorFacts {
                provider_code: "provider_specific_code",
                provider_message: "provider returned an unmapped error",
                retry_after_seconds: None,
                category: DeepReviewProviderCapacityErrorCategory::RateLimit,
            });
        assert_eq!(
            rate_limited.reason,
            Some(DeepReviewCapacityQueueReason::ProviderRateLimit)
        );

        let unavailable =
            capacity_decision_for_provider_error_facts(DeepReviewProviderCapacityErrorFacts {
                provider_code: "unknown",
                provider_message: "upstream failed",
                retry_after_seconds: None,
                category: DeepReviewProviderCapacityErrorCategory::ProviderUnavailable,
            });
        assert_eq!(
            unavailable.reason,
            Some(DeepReviewCapacityQueueReason::TemporaryOverload)
        );
    }

    #[test]
    fn provider_error_decision_keeps_quota_fail_fast() {
        let decision =
            capacity_decision_for_provider_error_facts(DeepReviewProviderCapacityErrorFacts {
                provider_code: "1113",
                provider_message: "insufficient quota",
                retry_after_seconds: None,
                category: DeepReviewProviderCapacityErrorCategory::RateLimit,
            });

        assert!(!decision.queueable);
        assert_eq!(
            decision.fail_fast_reason,
            Some(DeepReviewCapacityFailFastReason::BillingOrQuota)
        );
    }

    #[test]
    fn local_reviewer_capacity_decision_stays_queueable() {
        let decision = local_reviewer_capacity_queue_decision();
        assert_eq!(
            decision.reason,
            Some(DeepReviewCapacityQueueReason::LocalConcurrencyCap)
        );
        assert!(decision.queueable);
    }

    fn provider_retry_policy(max_queue_wait_seconds: u64) -> DeepReviewConcurrencyPolicy {
        DeepReviewConcurrencyPolicy {
            max_parallel_instances: 3,
            stagger_seconds: 0,
            max_queue_wait_seconds,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        }
    }

    #[test]
    fn provider_capacity_retry_runtime_owns_backoff_and_attempt_limit() {
        let policy = provider_retry_policy(60);
        let decision =
            classify_deep_review_capacity_error("429", "too many concurrent requests", None);
        let mut runtime = DeepReviewProviderCapacityRetryRuntime::default();

        assert_eq!(
            runtime.decide_after_error(&decision, &policy),
            DeepReviewProviderCapacityRetryDecision::WaitForCapacity {
                reason: DeepReviewCapacityQueueReason::ProviderRateLimit,
                max_wait_seconds: 60,
            }
        );
        assert_eq!(
            runtime.record_ready_to_retry(
                DeepReviewCapacityQueueReason::ProviderRateLimit,
                10,
                false,
            ),
            10
        );

        assert_eq!(
            runtime.decide_after_error(&decision, &policy),
            DeepReviewProviderCapacityRetryDecision::WaitForCapacity {
                reason: DeepReviewCapacityQueueReason::ProviderRateLimit,
                max_wait_seconds: 180,
            }
        );
        runtime.record_ready_to_retry(DeepReviewCapacityQueueReason::ProviderRateLimit, 20, false);
        assert_eq!(
            runtime.decide_after_error(&decision, &policy),
            DeepReviewProviderCapacityRetryDecision::WaitForCapacity {
                reason: DeepReviewCapacityQueueReason::ProviderRateLimit,
                max_wait_seconds: 540,
            }
        );
        runtime.record_ready_to_retry(DeepReviewCapacityQueueReason::ProviderRateLimit, 30, false);

        assert_eq!(
            runtime.decide_after_error(&decision, &policy),
            DeepReviewProviderCapacityRetryDecision::CapacitySkipped {
                reason: DeepReviewCapacityQueueReason::ProviderRateLimit,
                queue_elapsed_ms: 60,
            }
        );
        assert_eq!(
            runtime.last_retry_reason(),
            Some(DeepReviewCapacityQueueReason::ProviderRateLimit)
        );
    }

    #[test]
    fn provider_capacity_retry_runtime_keeps_early_probe_attempt_free() {
        let policy = provider_retry_policy(60);
        let decision =
            classify_deep_review_capacity_error("overloaded", "temporary overload", None);
        let mut runtime = DeepReviewProviderCapacityRetryRuntime::default();

        assert_eq!(
            runtime.decide_after_error(&decision, &policy),
            DeepReviewProviderCapacityRetryDecision::WaitForCapacity {
                reason: DeepReviewCapacityQueueReason::TemporaryOverload,
                max_wait_seconds: 60,
            }
        );
        runtime.record_ready_to_retry(DeepReviewCapacityQueueReason::TemporaryOverload, 15, true);

        assert_eq!(
            runtime.decide_after_error(&decision, &policy),
            DeepReviewProviderCapacityRetryDecision::WaitForCapacity {
                reason: DeepReviewCapacityQueueReason::TemporaryOverload,
                max_wait_seconds: 60,
            }
        );
    }

    #[test]
    fn provider_capacity_retry_runtime_accumulates_skipped_queue_elapsed() {
        let mut runtime = DeepReviewProviderCapacityRetryRuntime::default();

        assert_eq!(runtime.record_queue_skipped(25), 25);
        assert_eq!(runtime.record_queue_skipped(u64::MAX), u64::MAX);
    }

    #[test]
    fn provider_capacity_retry_runtime_rejects_fail_fast_decisions() {
        let policy = provider_retry_policy(60);
        let decision = DeepReviewCapacityQueueDecision::fail_fast(
            DeepReviewCapacityFailFastReason::InvalidModel,
        );
        let runtime = DeepReviewProviderCapacityRetryRuntime::default();

        assert_eq!(
            runtime.decide_after_error(&decision, &policy),
            DeepReviewProviderCapacityRetryDecision::NotQueueable
        );
    }

    #[test]
    fn provider_queue_decision_cancel_skips_before_other_states() {
        let mut facts =
            provider_queue_facts(DeepReviewCapacityQueueReason::ProviderConcurrencyLimit);
        facts.queue_expired = true;
        facts.active_reviewer_count = 1;
        facts.control_snapshot = control_snapshot(true, true, false);

        assert_eq!(
            decide_provider_capacity_queue_step(facts),
            DeepReviewProviderCapacityQueueStepDecision::Skipped {
                skip_reason: DeepReviewQueueWaitSkipReason::UserCancelled
            }
        );
    }

    #[test]
    fn queue_control_decision_prefers_cancel_before_pause() {
        assert_eq!(
            decide_queue_control_step(&control_snapshot(true, true, true), true),
            DeepReviewQueueControlStepDecision::Skipped {
                skip_reason: DeepReviewQueueWaitSkipReason::UserCancelled
            }
        );
    }

    #[test]
    fn provider_queue_decision_optional_skip_only_applies_to_optional_reviewers() {
        let mut mandatory =
            provider_queue_facts(DeepReviewCapacityQueueReason::ProviderConcurrencyLimit);
        mandatory.control_snapshot = control_snapshot(false, false, true);
        assert_eq!(
            decide_provider_capacity_queue_step(mandatory),
            DeepReviewProviderCapacityQueueStepDecision::Queued
        );

        let mut optional =
            provider_queue_facts(DeepReviewCapacityQueueReason::ProviderConcurrencyLimit);
        optional.control_snapshot = control_snapshot(false, false, true);
        optional.is_optional_reviewer = true;
        assert_eq!(
            decide_provider_capacity_queue_step(optional),
            DeepReviewProviderCapacityQueueStepDecision::Skipped {
                skip_reason: DeepReviewQueueWaitSkipReason::OptionalSkipped
            }
        );
    }

    #[test]
    fn incremental_cache_hit_prefers_description_packet() {
        let mut cache = DeepReviewIncrementalCache::new("fp-test-123");
        cache.store_packet(
            "reviewer:ReviewSecurity:group-2-of-2",
            "Found 2 security issues",
        );
        let manifest = json!({
            "incrementalReviewCache": { "fingerprint": "fp-test-123" },
            "deepReviewCache": cache.to_value(),
            "workPackets": [
                {
                    "packetId": "reviewer:ReviewSecurity:group-1-of-2",
                    "phase": "reviewer",
                    "subagentId": "ReviewSecurity"
                },
                {
                    "packetId": "reviewer:ReviewSecurity:group-2-of-2",
                    "phase": "reviewer",
                    "subagentId": "ReviewSecurity"
                }
            ]
        });

        let cache_hit = deep_review_incremental_cache_hit_for_task(
            "ReviewSecurity",
            Some("Security review [packet reviewer:ReviewSecurity:group-2-of-2]"),
            Some(&manifest),
        )
        .expect("description packet should select the matching cache entry");

        assert_eq!(cache_hit.packet_id, "reviewer:ReviewSecurity:group-2-of-2");
        assert_eq!(cache_hit.cached_output, "Found 2 security issues");
        let (data, assistant_message) =
            deep_review_incremental_cache_hit_result("ReviewSecurity", &cache_hit);
        assert_eq!(
            data,
            json!({ "cached": true, "packet_id": "reviewer:ReviewSecurity:group-2-of-2" })
        );
        assert_eq!(
            assistant_message,
            "Subagent 'ReviewSecurity' result (from incremental review cache):\n<result source=\"cache\">\nFound 2 security issues\n</result>"
        );
    }

    #[test]
    fn incremental_cache_hit_uses_unique_manifest_packet_without_description() {
        let mut cache = DeepReviewIncrementalCache::new("fp-test-123");
        cache.store_packet("reviewer:ReviewBusinessLogic", "Logic finding");
        let manifest = json!({
            "incrementalReviewCache": { "fingerprint": "fp-test-123" },
            "deepReviewCache": cache.to_value(),
            "workPackets": [
                {
                    "packetId": "reviewer:ReviewBusinessLogic",
                    "phase": "reviewer",
                    "subagentId": "ReviewBusinessLogic"
                }
            ]
        });

        let cache_hit = deep_review_incremental_cache_hit_for_task(
            "ReviewBusinessLogic",
            Some("Logic review"),
            Some(&manifest),
        )
        .expect("unique packet should be selected without a packet marker");

        assert_eq!(cache_hit.packet_id, "reviewer:ReviewBusinessLogic");
        assert_eq!(cache_hit.cached_output, "Logic finding");
    }

    #[test]
    fn incremental_cache_hit_skips_mismatches_and_ambiguous_packets() {
        let mut cache = DeepReviewIncrementalCache::new("fp-old");
        cache.store_packet("reviewer:ReviewPerformance:group-1-of-2", "Perf finding");
        let fingerprint_mismatch_manifest = json!({
            "incrementalReviewCache": { "fingerprint": "fp-new" },
            "deepReviewCache": cache.to_value(),
            "workPackets": [
                {
                    "packetId": "reviewer:ReviewPerformance:group-1-of-2",
                    "phase": "reviewer",
                    "subagentId": "ReviewPerformance"
                }
            ]
        });
        assert_eq!(
            deep_review_incremental_cache_hit_for_task(
                "ReviewPerformance",
                Some("Performance review"),
                Some(&fingerprint_mismatch_manifest),
            ),
            None
        );

        let mut cache = DeepReviewIncrementalCache::new("fp-test-123");
        cache.store_packet("reviewer:ReviewPerformance:group-1-of-2", "Perf finding");
        let split_packet_manifest = json!({
            "incrementalReviewCache": { "fingerprint": "fp-test-123" },
            "deepReviewCache": cache.to_value(),
            "workPackets": [
                {
                    "packetId": "reviewer:ReviewPerformance:group-1-of-2",
                    "phase": "reviewer",
                    "subagentId": "ReviewPerformance"
                },
                {
                    "packetId": "reviewer:ReviewPerformance:group-2-of-2",
                    "phase": "reviewer",
                    "subagentId": "ReviewPerformance"
                }
            ]
        });
        assert_eq!(
            deep_review_incremental_cache_hit_for_task(
                "ReviewPerformance",
                Some("Performance review"),
                Some(&split_packet_manifest),
            ),
            None
        );
        assert_eq!(
            deep_review_incremental_cache_hit_for_task(
                "ReviewPerformance",
                Some("Performance review [packet reviewer:ReviewSecurity:group-1-of-1]"),
                Some(&split_packet_manifest),
            ),
            None
        );
    }

    #[test]
    fn queue_control_decision_pause_applies_after_skip_checks() {
        assert_eq!(
            decide_queue_control_step(&control_snapshot(true, false, true), false),
            DeepReviewQueueControlStepDecision::Paused
        );
    }

    #[test]
    fn provider_queue_decision_pause_wins_over_expiry_and_active_release() {
        let mut facts =
            provider_queue_facts(DeepReviewCapacityQueueReason::ProviderConcurrencyLimit);
        facts.queue_expired = true;
        facts.active_reviewer_count = 1;
        facts.control_snapshot = control_snapshot(true, false, false);

        assert_eq!(
            decide_provider_capacity_queue_step(facts),
            DeepReviewProviderCapacityQueueStepDecision::Paused
        );
    }

    #[test]
    fn provider_queue_decision_expiry_retries_without_early_probe() {
        let mut facts =
            provider_queue_facts(DeepReviewCapacityQueueReason::ProviderConcurrencyLimit);
        facts.queue_expired = true;
        facts.active_reviewer_count = 2;

        assert_eq!(
            decide_provider_capacity_queue_step(facts),
            DeepReviewProviderCapacityQueueStepDecision::ReadyToRetry {
                early_capacity_probe: false
            }
        );
    }

    #[test]
    fn provider_queue_decision_wakes_when_provider_capacity_can_free() {
        let mut facts =
            provider_queue_facts(DeepReviewCapacityQueueReason::ProviderConcurrencyLimit);
        facts.active_reviewer_count = 1;

        assert_eq!(
            decide_provider_capacity_queue_step(facts),
            DeepReviewProviderCapacityQueueStepDecision::ReadyToRetry {
                early_capacity_probe: true
            }
        );
    }

    #[test]
    fn reviewer_admission_queue_expires_only_without_active_reviewers() {
        assert_eq!(
            decide_blocked_reviewer_admission_queue_step(
                DeepReviewBlockedReviewerAdmissionQueueStepFacts {
                    capacity_reason: DeepReviewCapacityQueueReason::LocalConcurrencyCap,
                    queue_expired: true,
                    active_reviewer_count: 0,
                },
            ),
            DeepReviewBlockedReviewerAdmissionQueueStepDecision::CapacityExpired {
                capacity_reason: DeepReviewCapacityQueueReason::LocalConcurrencyCap
            }
        );

        assert_eq!(
            decide_blocked_reviewer_admission_queue_step(
                DeepReviewBlockedReviewerAdmissionQueueStepFacts {
                    capacity_reason: DeepReviewCapacityQueueReason::LaunchBatchBlocked,
                    queue_expired: true,
                    active_reviewer_count: 1,
                },
            ),
            DeepReviewBlockedReviewerAdmissionQueueStepDecision::Queued {
                capacity_reason: DeepReviewCapacityQueueReason::LaunchBatchBlocked
            }
        );
    }

    #[test]
    fn provider_queue_decision_does_not_wake_retry_after_on_reviewer_release() {
        let mut facts = provider_queue_facts(DeepReviewCapacityQueueReason::RetryAfter);
        facts.active_reviewer_count = 1;

        assert_eq!(
            decide_provider_capacity_queue_step(facts),
            DeepReviewProviderCapacityQueueStepDecision::Queued
        );
    }

    #[test]
    fn provider_queue_decision_requires_existing_active_reviewer_before_wake() {
        let mut facts = provider_queue_facts(DeepReviewCapacityQueueReason::TemporaryOverload);
        facts.initial_active_reviewer_count = 0;
        facts.active_reviewer_count = 0;

        assert_eq!(
            decide_provider_capacity_queue_step(facts),
            DeepReviewProviderCapacityQueueStepDecision::Queued
        );
    }

    #[test]
    fn queue_wait_timer_excludes_paused_duration() {
        let start = Instant::now();
        let mut timer = QueueWaitTimer::start(start);

        let before_pause = start + Duration::from_millis(1_200);
        assert_eq!(
            timer.snapshot(before_pause).queue_elapsed,
            Duration::from_millis(1_200)
        );

        timer.pause(before_pause);
        let during_pause = start + Duration::from_millis(5_200);
        assert_eq!(
            timer.snapshot(during_pause).queue_elapsed,
            Duration::from_millis(1_200)
        );

        timer.continue_now(during_pause);
        let after_resume = start + Duration::from_millis(6_200);
        let snapshot = timer.snapshot(after_resume);
        assert_eq!(snapshot.queue_elapsed, Duration::from_millis(2_200));
        assert_eq!(snapshot.queue_elapsed_ms, 2_200);
    }

    #[test]
    fn queue_wait_timer_pause_and_continue_are_idempotent() {
        let start = Instant::now();
        let mut timer = QueueWaitTimer::start(start);

        let first_pause = start + Duration::from_millis(500);
        let second_pause = start + Duration::from_millis(900);
        timer.pause(first_pause);
        timer.pause(second_pause);

        let resume = start + Duration::from_millis(1_500);
        timer.continue_now(resume);
        timer.continue_now(resume + Duration::from_millis(300));

        let snapshot = timer.snapshot(start + Duration::from_millis(2_000));
        assert_eq!(snapshot.queue_elapsed, Duration::from_millis(1_000));
        assert!(!snapshot.is_expired(Duration::from_millis(1_001)));
        assert!(snapshot.is_expired(Duration::from_millis(1_000)));
    }

    #[test]
    fn provider_capacity_queue_runtime_pauses_without_consuming_wait_budget() {
        let start = Instant::now();
        let mut runtime = DeepReviewProviderCapacityQueueRuntime::start(
            start,
            DeepReviewCapacityQueueReason::ProviderConcurrencyLimit,
            Duration::from_secs(2),
            2,
            false,
        );
        let poll_interval = Duration::from_millis(100);

        assert_eq!(
            runtime.step(DeepReviewProviderCapacityQueueRuntimeInput {
                now: start + Duration::from_millis(500),
                active_reviewer_count: 2,
                control_snapshot: control_snapshot(true, false, false),
                poll_interval,
            }),
            DeepReviewProviderCapacityQueueRuntimeStep::Paused {
                queue_elapsed_ms: 500,
                next_sleep: poll_interval,
            }
        );

        assert_eq!(
            runtime.step(DeepReviewProviderCapacityQueueRuntimeInput {
                now: start + Duration::from_millis(1_500),
                active_reviewer_count: 2,
                control_snapshot: control_snapshot(true, false, false),
                poll_interval,
            }),
            DeepReviewProviderCapacityQueueRuntimeStep::Paused {
                queue_elapsed_ms: 500,
                next_sleep: poll_interval,
            }
        );

        assert_eq!(
            runtime.step(DeepReviewProviderCapacityQueueRuntimeInput {
                now: start + Duration::from_millis(2_500),
                active_reviewer_count: 1,
                control_snapshot: control_snapshot(false, false, false),
                poll_interval,
            }),
            DeepReviewProviderCapacityQueueRuntimeStep::ReadyToRetry {
                queue_elapsed_ms: 500,
                early_capacity_probe: true,
            }
        );
    }

    #[test]
    fn provider_capacity_queue_runtime_limits_sleep_to_remaining_wait() {
        let start = Instant::now();
        let mut runtime = DeepReviewProviderCapacityQueueRuntime::start(
            start,
            DeepReviewCapacityQueueReason::RetryAfter,
            Duration::from_secs(1),
            2,
            false,
        );

        assert_eq!(
            runtime.step(DeepReviewProviderCapacityQueueRuntimeInput {
                now: start + Duration::from_millis(950),
                active_reviewer_count: 2,
                control_snapshot: control_snapshot(false, false, false),
                poll_interval: Duration::from_millis(100),
            }),
            DeepReviewProviderCapacityQueueRuntimeStep::Queued {
                queue_elapsed_ms: 950,
                next_sleep: Duration::from_millis(50),
            }
        );
    }

    #[test]
    fn reviewer_admission_queue_runtime_pauses_without_consuming_wait_budget() {
        let start = Instant::now();
        let mut runtime = DeepReviewReviewerAdmissionQueueRuntime::start(
            start,
            DeepReviewCapacityQueueReason::LocalConcurrencyCap,
            Duration::from_secs(2),
            None,
            false,
        );
        let poll_interval = Duration::from_millis(100);

        assert_eq!(
            runtime.begin_step(DeepReviewReviewerAdmissionQueueRuntimeInput {
                now: start + Duration::from_millis(500),
                control_snapshot: control_snapshot(true, false, false),
                poll_interval,
            }),
            DeepReviewReviewerAdmissionQueueRuntimeStep::Paused {
                queue_elapsed_ms: 500,
                capacity_reason: DeepReviewCapacityQueueReason::LocalConcurrencyCap,
                next_sleep: poll_interval,
            }
        );

        assert_eq!(
            runtime.begin_step(DeepReviewReviewerAdmissionQueueRuntimeInput {
                now: start + Duration::from_millis(1_500),
                control_snapshot: control_snapshot(true, false, false),
                poll_interval,
            }),
            DeepReviewReviewerAdmissionQueueRuntimeStep::Paused {
                queue_elapsed_ms: 500,
                capacity_reason: DeepReviewCapacityQueueReason::LocalConcurrencyCap,
                next_sleep: poll_interval,
            }
        );
    }

    #[test]
    fn reviewer_admission_queue_runtime_limits_sleep_to_remaining_wait() {
        let start = Instant::now();
        let mut runtime = DeepReviewReviewerAdmissionQueueRuntime::start(
            start,
            DeepReviewCapacityQueueReason::LocalConcurrencyCap,
            Duration::from_secs(1),
            Some(3),
            false,
        );
        let poll_interval = Duration::from_millis(100);

        let step = runtime.begin_step(DeepReviewReviewerAdmissionQueueRuntimeInput {
            now: start + Duration::from_millis(950),
            control_snapshot: control_snapshot(false, false, false),
            poll_interval,
        });
        let expected_attempt = DeepReviewReviewerAdmissionTryAdmit {
            queue_elapsed_ms: 950,
            queue_expired: false,
        };
        assert_eq!(
            step,
            DeepReviewReviewerAdmissionQueueRuntimeStep::TryAdmit {
                queue_elapsed_ms: 950,
                attempt: expected_attempt,
                capacity_reason: DeepReviewCapacityQueueReason::LocalConcurrencyCap,
            }
        );

        assert_eq!(
            runtime.after_blocked_attempt(
                expected_attempt,
                DeepReviewCapacityQueueReason::LocalConcurrencyCap,
                1,
                poll_interval,
            ),
            DeepReviewReviewerAdmissionQueueRuntimeBlockedStep::Queued {
                queue_elapsed_ms: 950,
                capacity_reason: DeepReviewCapacityQueueReason::LocalConcurrencyCap,
                next_sleep: Duration::from_millis(50),
            }
        );
    }

    #[test]
    fn reviewer_admission_queue_runtime_expires_with_retry_after_hint() {
        let start = Instant::now();
        let mut runtime = DeepReviewReviewerAdmissionQueueRuntime::start(
            start,
            DeepReviewCapacityQueueReason::LocalConcurrencyCap,
            Duration::from_secs(1),
            Some(3),
            false,
        );

        let step = runtime.begin_step(DeepReviewReviewerAdmissionQueueRuntimeInput {
            now: start + Duration::from_millis(1_000),
            control_snapshot: control_snapshot(false, false, false),
            poll_interval: Duration::from_millis(100),
        });
        let attempt = match step {
            DeepReviewReviewerAdmissionQueueRuntimeStep::TryAdmit { attempt, .. } => attempt,
            other => panic!("expected reviewer admission attempt, got {other:?}"),
        };

        assert_eq!(
            runtime.after_blocked_attempt(
                attempt,
                DeepReviewCapacityQueueReason::LocalConcurrencyCap,
                0,
                Duration::from_millis(100),
            ),
            DeepReviewReviewerAdmissionQueueRuntimeBlockedStep::CapacityExpired {
                queue_elapsed_ms: 1_000,
                capacity_reason: DeepReviewCapacityQueueReason::LocalConcurrencyCap,
                retry_after_seconds: Some(3),
            }
        );
    }

    #[test]
    fn task_completion_result_preserves_completed_message_and_data_shape() {
        let (data, assistant_message) =
            deep_review_task_completion_result(DeepReviewTaskCompletionResultInput {
                delegate_target_label: "ReviewSecurity",
                result_text: "No issues found",
                context_mode: "fresh",
                duration_ms: 42,
                is_partial_timeout: false,
                reason: None,
                ledger_event_id: None,
                retry_hint: "",
            });

        assert_eq!(data["duration"], json!(42));
        assert_eq!(data["context_mode"], "fresh");
        assert_eq!(data["status"], "completed");
        assert!(data.get("partial_output").is_none());
        assert_eq!(
            assistant_message,
            "ReviewSecurity completed successfully with result:\n<result>\nNo issues found\n</result>"
        );
    }

    #[test]
    fn task_completion_result_preserves_partial_timeout_payload() {
        let (data, assistant_message) =
            deep_review_task_completion_result(DeepReviewTaskCompletionResultInput {
                delegate_target_label: "ReviewPerformance",
                result_text: "Partial findings",
                context_mode: "reuse",
                duration_ms: 120,
                is_partial_timeout: true,
                reason: Some("timeout"),
                ledger_event_id: Some("event-1"),
                retry_hint: "\n\n<retry_guidance>retry</retry_guidance>",
            });

        assert_eq!(data["status"], "partial_timeout");
        assert_eq!(data["partial_output"], "Partial findings");
        assert_eq!(data["reason"], "timeout");
        assert_eq!(data["ledger_event_id"], "event-1");
        assert_eq!(
            assistant_message,
            "ReviewPerformance timed out with partial result:\n<partial_result status=\"partial_timeout\">\nPartial findings\n</partial_result>\n\n<retry_guidance>retry</retry_guidance>"
        );
    }

    #[test]
    fn cancelled_reviewer_result_preserves_parent_guidance_and_data_shape() {
        let (data, assistant_message) = deep_review_cancelled_reviewer_result(
            "ReviewArchitecture",
            " Subagent task has been cancelled ",
            42,
        );

        assert_eq!(data["status"], "cancelled");
        assert_eq!(data["reason"], "Subagent task has been cancelled");
        assert_eq!(data["duration"], 42);
        assert!(assistant_message.contains("status=\"cancelled\""));
        assert!(assistant_message.contains("reason=\"user_cancelled\""));
        assert!(assistant_message.contains("do not relaunch it automatically"));
    }

    #[test]
    fn cancelled_reviewer_result_defaults_empty_reason_and_caps_duration() {
        let (data, _assistant_message) =
            deep_review_cancelled_reviewer_result("ReviewSecurity", "  ", u128::MAX);

        assert_eq!(data["status"], "cancelled");
        assert_eq!(data["reason"], "Subagent task was cancelled");
        assert_eq!(data["duration"], u64::MAX);
    }

    #[test]
    fn retry_guidance_policy_applies_only_to_initial_reviewer_timeout() {
        assert!(should_emit_deep_review_retry_guidance(
            true,
            false,
            Some(DeepReviewSubagentRole::Reviewer),
        ));
        assert!(!should_emit_deep_review_retry_guidance(
            false,
            false,
            Some(DeepReviewSubagentRole::Reviewer),
        ));
        assert!(!should_emit_deep_review_retry_guidance(
            true,
            true,
            Some(DeepReviewSubagentRole::Reviewer),
        ));
        assert!(!should_emit_deep_review_retry_guidance(
            true,
            false,
            Some(DeepReviewSubagentRole::Judge),
        ));
    }

    #[test]
    fn retry_guidance_message_preserves_budget_text() {
        assert_eq!(
            deep_review_retry_guidance(1, 3),
            "\n\n<retry_guidance>This reviewer timed out. You may retry with 'retry: true' only if you can provide retry_coverage with source_packet_id, source_status='partial_timeout', covered_files, and a smaller retry_scope_files list. Retries used: 1/3.</retry_guidance>"
        );
        assert!(deep_review_retry_guidance(3, 3).is_empty());
        assert!(deep_review_retry_guidance(0, 0).is_empty());
    }

    #[test]
    fn auto_retry_admission_uses_opt_in_and_elapsed_guard() {
        let mut policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 1,
            stagger_seconds: 0,
            max_queue_wait_seconds: 1,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        };

        let disabled = ensure_deep_review_auto_retry_allowed(&policy, None)
            .expect_err("disabled auto retry should be rejected");
        assert_eq!(disabled.code, "deep_review_auto_retry_disabled");

        policy.allow_bounded_auto_retry = true;
        assert!(ensure_deep_review_auto_retry_allowed(&policy, Some(180)).is_ok());
        let elapsed = ensure_deep_review_auto_retry_allowed(&policy, Some(181))
            .expect_err("elapsed guard should reject late auto retry");
        assert_eq!(
            elapsed.code,
            "deep_review_auto_retry_elapsed_guard_exceeded"
        );
    }

    #[test]
    fn auto_retry_suppression_reason_stays_stable() {
        assert_eq!(
            auto_retry_suppression_reason("deep_review_retry_missing_packet_scope"),
            "unknown_packet"
        );
        assert_eq!(
            auto_retry_suppression_reason("deep_review_retry_timeout_not_reduced"),
            "timeout_not_reduced"
        );
        assert_eq!(
            auto_retry_suppression_reason("unexpected"),
            "invalid_coverage"
        );
    }
}
