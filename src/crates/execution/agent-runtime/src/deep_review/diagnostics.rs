//! Content-free Deep Review runtime diagnostics counters.
//!
//! These counters are safe to surface in reports and logs because they record
//! aggregate counts, durations, and reason labels only. They must not store
//! source text, diffs, reviewer output, provider raw bodies, or full file paths.

use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepReviewRuntimeDiagnostics {
    pub queue_wait_count: usize,
    pub queue_wait_total_ms: u64,
    pub queue_wait_max_ms: u64,
    pub provider_capacity_queue_count: usize,
    pub provider_capacity_retry_count: usize,
    pub provider_capacity_retry_success_count: usize,
    pub capacity_skip_count: usize,
    pub provider_capacity_queue_reason_counts: BTreeMap<String, usize>,
    pub provider_capacity_retry_reason_counts: BTreeMap<String, usize>,
    pub provider_capacity_retry_success_reason_counts: BTreeMap<String, usize>,
    pub capacity_skip_reason_counts: BTreeMap<String, usize>,
    pub effective_parallel_min: Option<usize>,
    pub effective_parallel_final: Option<usize>,
    pub manual_queue_action_count: usize,
    pub manual_retry_count: usize,
    pub auto_retry_count: usize,
    pub auto_retry_suppressed_reason_counts: BTreeMap<String, usize>,
    pub shared_context_total_calls: usize,
    pub shared_context_duplicate_calls: usize,
    pub shared_context_duplicate_context_count: usize,
    pub shared_context_duplicate_savings_candidate_count: usize,
}

impl DeepReviewRuntimeDiagnostics {
    pub(crate) fn is_empty(&self) -> bool {
        self.queue_wait_count == 0
            && self.queue_wait_total_ms == 0
            && self.queue_wait_max_ms == 0
            && self.provider_capacity_queue_count == 0
            && self.provider_capacity_retry_count == 0
            && self.provider_capacity_retry_success_count == 0
            && self.capacity_skip_count == 0
            && self.provider_capacity_queue_reason_counts.is_empty()
            && self.provider_capacity_retry_reason_counts.is_empty()
            && self
                .provider_capacity_retry_success_reason_counts
                .is_empty()
            && self.capacity_skip_reason_counts.is_empty()
            && self.effective_parallel_min.is_none()
            && self.effective_parallel_final.is_none()
            && self.manual_queue_action_count == 0
            && self.manual_retry_count == 0
            && self.auto_retry_count == 0
            && self.auto_retry_suppressed_reason_counts.is_empty()
            && self.shared_context_total_calls == 0
            && self.shared_context_duplicate_calls == 0
            && self.shared_context_duplicate_context_count == 0
            && self.shared_context_duplicate_savings_candidate_count == 0
    }

    pub(crate) fn observe_effective_parallel(&mut self, effective_parallel_instances: usize) {
        self.effective_parallel_min = Some(
            self.effective_parallel_min
                .map_or(effective_parallel_instances, |current| {
                    current.min(effective_parallel_instances)
                }),
        );
        self.effective_parallel_final = Some(effective_parallel_instances);
    }

    pub(crate) fn merge_shared_context_counts(
        &mut self,
        total_calls: usize,
        duplicate_calls: usize,
        duplicate_context_count: usize,
    ) {
        self.shared_context_total_calls = total_calls;
        self.shared_context_duplicate_calls = duplicate_calls;
        self.shared_context_duplicate_context_count = duplicate_context_count;
        self.shared_context_duplicate_savings_candidate_count = duplicate_calls;
    }
}

fn map_log_value(map: &BTreeMap<String, usize>) -> String {
    serde_json::to_string(map).unwrap_or_else(|_| "{}".to_string())
}

fn optional_usize_log_value(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

pub fn deep_review_runtime_diagnostics_log_line(
    diagnostics: &DeepReviewRuntimeDiagnostics,
) -> String {
    format!(
        "DeepReview runtime diagnostics: queue_wait_count={}, queue_wait_total_ms={}, queue_wait_max_ms={}, provider_capacity_queue_count={}, provider_capacity_retry_count={}, provider_capacity_retry_success_count={}, capacity_skip_count={}, provider_capacity_queue_reason_counts={}, provider_capacity_retry_reason_counts={}, provider_capacity_retry_success_reason_counts={}, capacity_skip_reason_counts={}, effective_parallel_min={}, effective_parallel_final={}, manual_queue_action_count={}, manual_retry_count={}, auto_retry_count={}, auto_retry_suppressed_reason_counts={}, shared_context_total_calls={}, shared_context_duplicate_calls={}, shared_context_duplicate_context_count={}, shared_context_duplicate_savings_candidate_count={}",
        diagnostics.queue_wait_count,
        diagnostics.queue_wait_total_ms,
        diagnostics.queue_wait_max_ms,
        diagnostics.provider_capacity_queue_count,
        diagnostics.provider_capacity_retry_count,
        diagnostics.provider_capacity_retry_success_count,
        diagnostics.capacity_skip_count,
        map_log_value(&diagnostics.provider_capacity_queue_reason_counts),
        map_log_value(&diagnostics.provider_capacity_retry_reason_counts),
        map_log_value(&diagnostics.provider_capacity_retry_success_reason_counts),
        map_log_value(&diagnostics.capacity_skip_reason_counts),
        optional_usize_log_value(diagnostics.effective_parallel_min),
        optional_usize_log_value(diagnostics.effective_parallel_final),
        diagnostics.manual_queue_action_count,
        diagnostics.manual_retry_count,
        diagnostics.auto_retry_count,
        map_log_value(&diagnostics.auto_retry_suppressed_reason_counts),
        diagnostics.shared_context_total_calls,
        diagnostics.shared_context_duplicate_calls,
        diagnostics.shared_context_duplicate_context_count,
        diagnostics.shared_context_duplicate_savings_candidate_count
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_log_line_preserves_field_names_and_serialized_counts() {
        let diagnostics = DeepReviewRuntimeDiagnostics {
            queue_wait_count: 2,
            queue_wait_total_ms: 120,
            queue_wait_max_ms: 90,
            provider_capacity_queue_count: 1,
            provider_capacity_retry_count: 3,
            provider_capacity_retry_success_count: 1,
            capacity_skip_count: 4,
            provider_capacity_queue_reason_counts: BTreeMap::from([(
                "provider_rate_limit".to_string(),
                1,
            )]),
            provider_capacity_retry_reason_counts: BTreeMap::from([(
                "temporary_overload".to_string(),
                3,
            )]),
            provider_capacity_retry_success_reason_counts: BTreeMap::from([(
                "temporary_overload".to_string(),
                1,
            )]),
            capacity_skip_reason_counts: BTreeMap::from([("local_concurrency_cap".to_string(), 4)]),
            effective_parallel_min: Some(1),
            effective_parallel_final: Some(2),
            manual_queue_action_count: 5,
            manual_retry_count: 6,
            auto_retry_count: 7,
            auto_retry_suppressed_reason_counts: BTreeMap::from([(
                "budget_exhausted".to_string(),
                2,
            )]),
            shared_context_total_calls: 8,
            shared_context_duplicate_calls: 9,
            shared_context_duplicate_context_count: 10,
            shared_context_duplicate_savings_candidate_count: 11,
        };

        let line = deep_review_runtime_diagnostics_log_line(&diagnostics);

        assert!(line.starts_with("DeepReview runtime diagnostics: queue_wait_count=2"));
        assert!(line.contains("provider_capacity_queue_reason_counts={\"provider_rate_limit\":1}"));
        assert!(line.contains("capacity_skip_reason_counts={\"local_concurrency_cap\":4}"));
        assert!(line.contains("effective_parallel_min=1"));
        assert!(line.contains("effective_parallel_final=2"));
        assert!(line.contains("shared_context_duplicate_savings_candidate_count=11"));
    }

    #[test]
    fn diagnostics_log_line_uses_none_for_missing_effective_parallel() {
        let diagnostics = DeepReviewRuntimeDiagnostics::default();

        let line = deep_review_runtime_diagnostics_log_line(&diagnostics);

        assert!(line.contains("effective_parallel_min=none"));
        assert!(line.contains("effective_parallel_final=none"));
    }
}
