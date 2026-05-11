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
