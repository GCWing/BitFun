//! Deep Review concurrency limits and effective capacity learning.
//!
//! The policy here is product-specific: it learns an effective reviewer cap for
//! Deep Review sessions and stores the Review Team capacity preferences. Shared
//! queue timing or future generic admission primitives belong in
//! `agentic::subagent_runtime` once they are proven independent of Deep Review.

use super::execution_policy::{
    clamp_u64, clamp_usize, reviewer_agent_type_count, DeepReviewExecutionPolicy,
    DeepReviewPolicyViolation, DeepReviewSubagentRole,
};
use serde_json::Value;
use std::time::{Duration, Instant};

const DEFAULT_MAX_PARALLEL_INSTANCES: usize = 4;
const DEFAULT_MAX_QUEUE_WAIT_SECONDS: u64 = 60;
const DEFAULT_AUTO_RETRY_ELAPSED_GUARD_SECONDS: u64 = 180;
const MAX_QUEUE_WAIT_SECONDS: u64 = 600;
const MAX_AUTO_RETRY_ELAPSED_GUARD_SECONDS: u64 = 900;
const EFFECTIVE_CONCURRENCY_RECOVERY_SUCCESS_WINDOW: usize = 3;

/// Dynamic concurrency control for deep review reviewer launches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepReviewConcurrencyPolicy {
    /// Maximum parallel reviewer instances at once.
    pub max_parallel_instances: usize,
    /// Whether to stagger launches (wait N seconds between batches).
    pub stagger_seconds: u64,
    /// Maximum time an over-cap reviewer launch can wait before being skipped.
    pub max_queue_wait_seconds: u64,
    /// Whether to batch extras separately from core reviewers.
    pub batch_extras_separately: bool,
    /// Whether backend-owned bounded automatic reviewer retries may be admitted.
    pub allow_bounded_auto_retry: bool,
    /// Maximum elapsed turn time before backend-owned automatic retries are suppressed.
    pub auto_retry_elapsed_guard_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepReviewEffectiveConcurrencySnapshot {
    pub configured_max_parallel_instances: usize,
    pub learned_parallel_instances: usize,
    pub effective_parallel_instances: usize,
    pub user_override_parallel_instances: Option<usize>,
    pub retry_after_remaining_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub(crate) struct DeepReviewEffectiveConcurrencyState {
    configured_max_parallel_instances: usize,
    learned_parallel_instances: usize,
    user_override_parallel_instances: Option<usize>,
    successful_observation_count: usize,
    retry_after_until: Option<Instant>,
}

impl DeepReviewEffectiveConcurrencyState {
    pub(crate) fn new(configured_max_parallel_instances: usize) -> Self {
        let configured_max_parallel_instances =
            Self::normalize_configured_max(configured_max_parallel_instances);
        Self {
            configured_max_parallel_instances,
            learned_parallel_instances: configured_max_parallel_instances,
            user_override_parallel_instances: None,
            successful_observation_count: 0,
            retry_after_until: None,
        }
    }

    fn normalize_configured_max(configured_max_parallel_instances: usize) -> usize {
        configured_max_parallel_instances.max(1)
    }

    pub(crate) fn rebase_configured_max(&mut self, configured_max_parallel_instances: usize) {
        let configured_max_parallel_instances =
            Self::normalize_configured_max(configured_max_parallel_instances);
        if self.configured_max_parallel_instances == configured_max_parallel_instances {
            return;
        }

        self.configured_max_parallel_instances = configured_max_parallel_instances;
        self.learned_parallel_instances = self
            .learned_parallel_instances
            .clamp(1, configured_max_parallel_instances);
        self.user_override_parallel_instances = self
            .user_override_parallel_instances
            .map(|value| value.clamp(1, configured_max_parallel_instances));
    }

    pub(crate) fn effective_parallel_instances(&self, now: Instant) -> usize {
        if let Some(user_override) = self.user_override_parallel_instances {
            return user_override.clamp(1, self.configured_max_parallel_instances);
        }

        if self
            .retry_after_until
            .is_some_and(|retry_after_until| retry_after_until > now)
        {
            return 1;
        }

        self.learned_parallel_instances
            .clamp(1, self.configured_max_parallel_instances)
    }

    pub(crate) fn record_capacity_error(
        &mut self,
        has_retry_after_hint: bool,
        retry_after: Option<Duration>,
        now: Instant,
    ) {
        self.successful_observation_count = 0;
        self.learned_parallel_instances = self.learned_parallel_instances.saturating_sub(1).max(1);

        if has_retry_after_hint || retry_after.is_some() {
            self.retry_after_until = retry_after.map(|duration| now + duration);
        }
    }

    pub(crate) fn record_success(&mut self, now: Instant) {
        if self
            .retry_after_until
            .is_some_and(|retry_after_until| retry_after_until > now)
        {
            return;
        }
        if self
            .retry_after_until
            .is_some_and(|retry_after_until| retry_after_until <= now)
        {
            self.retry_after_until = None;
        }

        if self.learned_parallel_instances >= self.configured_max_parallel_instances {
            self.successful_observation_count = 0;
            return;
        }

        self.successful_observation_count = self.successful_observation_count.saturating_add(1);
        if self.successful_observation_count >= EFFECTIVE_CONCURRENCY_RECOVERY_SUCCESS_WINDOW {
            self.learned_parallel_instances =
                (self.learned_parallel_instances + 1).min(self.configured_max_parallel_instances);
            self.successful_observation_count = 0;
        }
    }

    pub(crate) fn set_user_override(&mut self, user_override_parallel_instances: Option<usize>) {
        self.user_override_parallel_instances = user_override_parallel_instances
            .map(|value| value.clamp(1, self.configured_max_parallel_instances));
    }

    pub(crate) fn snapshot(&self, now: Instant) -> DeepReviewEffectiveConcurrencySnapshot {
        let retry_after_remaining_ms =
            self.retry_after_until
                .and_then(|retry_after_until| match retry_after_until > now {
                    true => Some(
                        u64::try_from(retry_after_until.duration_since(now).as_millis())
                            .unwrap_or(u64::MAX),
                    ),
                    false => None,
                });

        DeepReviewEffectiveConcurrencySnapshot {
            configured_max_parallel_instances: self.configured_max_parallel_instances,
            learned_parallel_instances: self
                .learned_parallel_instances
                .clamp(1, self.configured_max_parallel_instances),
            effective_parallel_instances: self.effective_parallel_instances(now),
            user_override_parallel_instances: self.user_override_parallel_instances,
            retry_after_remaining_ms,
        }
    }
}

impl Default for DeepReviewConcurrencyPolicy {
    fn default() -> Self {
        Self {
            max_parallel_instances: DEFAULT_MAX_PARALLEL_INSTANCES,
            stagger_seconds: 0,
            max_queue_wait_seconds: DEFAULT_MAX_QUEUE_WAIT_SECONDS,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: DEFAULT_AUTO_RETRY_ELAPSED_GUARD_SECONDS,
        }
    }
}

impl DeepReviewExecutionPolicy {
    /// Extract the concurrency policy from a run manifest, if present.
    pub fn concurrency_policy_from_manifest(
        &self,
        raw_manifest: &Value,
    ) -> DeepReviewConcurrencyPolicy {
        raw_manifest
            .get("concurrencyPolicy")
            .map(DeepReviewConcurrencyPolicy::from_manifest)
            .unwrap_or_default()
    }
}

impl DeepReviewConcurrencyPolicy {
    pub fn from_manifest(raw: &Value) -> Self {
        let Some(obj) = raw.as_object() else {
            return Self::default();
        };

        Self {
            max_parallel_instances: clamp_usize(
                obj.get("maxParallelInstances"),
                1,
                16,
                DEFAULT_MAX_PARALLEL_INSTANCES,
            ),
            stagger_seconds: clamp_u64(obj.get("staggerSeconds"), 0, 60, 0),
            max_queue_wait_seconds: clamp_u64(
                obj.get("maxQueueWaitSeconds"),
                0,
                MAX_QUEUE_WAIT_SECONDS,
                DEFAULT_MAX_QUEUE_WAIT_SECONDS,
            ),
            batch_extras_separately: obj
                .get("batchExtrasSeparately")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            allow_bounded_auto_retry: obj
                .get("allowBoundedAutoRetry")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            auto_retry_elapsed_guard_seconds: clamp_u64(
                obj.get("autoRetryElapsedGuardSeconds"),
                30,
                MAX_AUTO_RETRY_ELAPSED_GUARD_SECONDS,
                DEFAULT_AUTO_RETRY_ELAPSED_GUARD_SECONDS,
            ),
        }
    }

    /// Compute the effective max same-role instances, capped by both
    /// the execution policy's `max_same_role_instances` and the
    /// concurrency policy's `max_parallel_instances / role_count`.
    pub fn effective_max_same_role_instances(&self, policy: &DeepReviewExecutionPolicy) -> usize {
        let role_count = reviewer_agent_type_count() + policy.extra_subagent_ids.len();
        let max_per_role = self.max_parallel_instances / role_count.max(1);
        max_per_role.max(1).min(policy.max_same_role_instances)
    }

    /// Check whether the current number of active launches exceeds the cap.
    /// Returns `Ok(())` if the launch is allowed, or an error describing why not.
    pub fn check_launch_allowed(
        &self,
        active_count: usize,
        role: DeepReviewSubagentRole,
        is_judge_pending: bool,
    ) -> Result<(), DeepReviewPolicyViolation> {
        match role {
            DeepReviewSubagentRole::Reviewer => {
                if active_count >= self.max_parallel_instances {
                    return Err(DeepReviewPolicyViolation::new(
                        "deep_review_concurrency_cap_reached",
                        format!(
                            "Maximum parallel reviewer instances reached ({}/{}). Wait for running reviewers to complete before launching more.",
                            active_count, self.max_parallel_instances
                        ),
                    ));
                }
            }
            DeepReviewSubagentRole::Judge => {
                if active_count > 0 {
                    return Err(DeepReviewPolicyViolation::new(
                        "deep_review_judge_launch_blocked_by_reviewers",
                        format!(
                            "ReviewJudge cannot launch while {} reviewer(s) are still active. Wait for reviewers to complete first.",
                            active_count
                        ),
                    ));
                }
                if is_judge_pending {
                    return Err(DeepReviewPolicyViolation::new(
                        "deep_review_judge_already_pending",
                        "ReviewJudge is already pending or running in this turn.",
                    ));
                }
            }
        }
        Ok(())
    }
}
