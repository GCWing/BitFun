use super::*;

impl LaunchReviewAgentTool {
    pub(super) fn deep_review_launch_batch_for_task(
        subagent_type: &str,
        description: Option<&str>,
        run_manifest: Option<&Value>,
    ) -> Option<DeepReviewLaunchBatchInfo> {
        deep_review_task_adapter::deep_review_launch_batch_for_task(
            subagent_type,
            description,
            run_manifest,
        )
    }

    pub(super) fn attach_deep_review_cache(run_manifest: &mut Value, cache_value: Option<Value>) {
        deep_review_task_adapter::attach_deep_review_cache(run_manifest, cache_value);
    }

    pub(super) fn deep_review_retry_guidance_max_retries(
        effective_policy: Option<&DeepReviewExecutionPolicy>,
        dialog_turn_id: &str,
    ) -> usize {
        deep_review_task_adapter::deep_review_retry_guidance_max_retries(
            effective_policy,
            dialog_turn_id,
        )
    }

    pub(super) fn should_emit_deep_review_retry_guidance(
        is_partial_timeout: bool,
        is_retry: bool,
        deep_review_subagent_role: Option<DeepReviewSubagentRole>,
    ) -> bool {
        deep_review_task_adapter::should_emit_deep_review_retry_guidance(
            is_partial_timeout,
            is_retry,
            deep_review_subagent_role,
        )
    }

    pub(super) fn ensure_deep_review_retry_coverage(
        input: &Value,
        subagent_type: &str,
        run_manifest: Option<&Value>,
    ) -> Result<Vec<String>, DeepReviewPolicyViolation> {
        deep_review_task_adapter::ensure_deep_review_retry_coverage(
            input,
            subagent_type,
            run_manifest,
        )
    }

    pub(super) fn auto_retry_suppression_reason(code: &str) -> &'static str {
        deep_review_task_adapter::auto_retry_suppression_reason(code)
    }

    pub(super) fn ensure_deep_review_auto_retry_allowed(
        conc_policy: &DeepReviewConcurrencyPolicy,
        dialog_turn_id: &str,
    ) -> Result<(), DeepReviewPolicyViolation> {
        deep_review_task_adapter::ensure_deep_review_auto_retry_allowed(
            conc_policy,
            deep_review_turn_elapsed_seconds(dialog_turn_id),
        )
    }

    pub(super) fn prompt_with_deep_review_retry_scope(
        prompt: &str,
        retry_scope_files: &[String],
    ) -> String {
        deep_review_task_adapter::prompt_with_deep_review_retry_scope(prompt, retry_scope_files)
    }

    pub(super) fn deep_review_capacity_decision_for_provider_error(
        error: &BitFunError,
    ) -> crate::agentic::deep_review_policy::DeepReviewCapacityQueueDecision {
        deep_review_task_adapter::capacity_decision_for_provider_error(error)
    }

    pub(super) fn deep_review_capacity_skip_result_for_provider_queue_outcome(
        reason: DeepReviewCapacityQueueReason,
        dialog_turn_id: &str,
        subagent_type: &str,
        conc_policy: &DeepReviewConcurrencyPolicy,
        duration_ms: u128,
        queue_elapsed_ms: u64,
        terminal_skip_reason: Option<DeepReviewQueueWaitSkipReason>,
    ) -> (Value, String) {
        deep_review_task_adapter::capacity_skip_result_for_provider_queue_outcome(
            reason,
            dialog_turn_id,
            subagent_type,
            conc_policy,
            duration_ms,
            queue_elapsed_ms,
            terminal_skip_reason,
        )
    }

    pub(super) async fn wait_for_deep_review_provider_capacity_retry(
        session_id: &str,
        dialog_turn_id: &str,
        tool_id: &str,
        subagent_type: &str,
        conc_policy: &DeepReviewConcurrencyPolicy,
        reason: DeepReviewCapacityQueueReason,
        max_wait_seconds: u64,
        is_optional_reviewer: bool,
    ) -> DeepReviewProviderQueueWaitOutcome {
        deep_review_task_adapter::wait_for_provider_capacity_retry(
            session_id,
            dialog_turn_id,
            tool_id,
            subagent_type,
            conc_policy,
            reason,
            max_wait_seconds,
            is_optional_reviewer,
        )
        .await
    }

    pub(super) fn record_deep_review_provider_capacity_retry(
        dialog_turn_id: &str,
        reason: DeepReviewCapacityQueueReason,
    ) {
        deep_review_task_adapter::record_provider_capacity_retry(dialog_turn_id, reason);
    }

    pub(super) fn record_deep_review_provider_capacity_retry_success(
        dialog_turn_id: &str,
        reason: DeepReviewCapacityQueueReason,
    ) {
        deep_review_task_adapter::record_provider_capacity_retry_success(dialog_turn_id, reason);
    }

    pub(super) async fn emit_deep_review_queue_state(
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
        deep_review_task_adapter::emit_queue_state(
            session_id,
            dialog_turn_id,
            tool_id,
            subagent_type,
            status,
            reason,
            queued_reviewer_count,
            active_reviewer_count,
            optional_reviewer_count,
            effective_parallel_instances,
            queue_elapsed_ms,
            max_queue_wait_seconds,
        )
        .await;
    }

    pub(super) fn try_begin_deep_review_reviewer_admission(
        dialog_turn_id: &str,
        effective_parallel_instances: usize,
        launch_batch_info: Option<&DeepReviewLaunchBatchInfo>,
    ) -> Result<Option<DeepReviewActiveReviewerGuard<'static>>, DeepReviewPolicyViolation> {
        deep_review_task_adapter::try_begin_reviewer_admission(
            dialog_turn_id,
            effective_parallel_instances,
            launch_batch_info,
        )
    }

    pub(super) async fn wait_for_deep_review_reviewer_admission(
        session_id: &str,
        dialog_turn_id: &str,
        tool_id: &str,
        subagent_type: &str,
        conc_policy: &DeepReviewConcurrencyPolicy,
        is_optional_reviewer: bool,
        launch_batch_info: Option<&DeepReviewLaunchBatchInfo>,
    ) -> BitFunResult<DeepReviewQueueWaitOutcome> {
        deep_review_task_adapter::wait_for_reviewer_admission(
            session_id,
            dialog_turn_id,
            tool_id,
            subagent_type,
            conc_policy,
            is_optional_reviewer,
            launch_batch_info,
        )
        .await
    }

    pub(super) fn deep_review_local_capacity_skip_tool_result(
        dialog_turn_id: &str,
        subagent_type: &str,
        conc_policy: &DeepReviewConcurrencyPolicy,
        capacity_reason: DeepReviewCapacityQueueReason,
        skip_reason: DeepReviewQueueWaitSkipReason,
        queue_elapsed_ms: u64,
        duration_ms: u128,
    ) -> ToolResult {
        let (data, assistant_message) =
            deep_review_task_adapter::capacity_skip_result_for_local_queue_outcome(
                dialog_turn_id,
                subagent_type,
                conc_policy,
                capacity_reason,
                skip_reason,
                queue_elapsed_ms,
                duration_ms,
            );
        ToolResult::Result {
            data,
            result_for_assistant: Some(assistant_message),
            image_attachments: None,
        }
    }

    pub(super) fn deep_review_cancelled_reviewer_tool_result(
        subagent_type: &str,
        reason: &str,
        duration_ms: u128,
    ) -> ToolResult {
        let (data, result_for_assistant) =
            deep_review_task_adapter::deep_review_cancelled_reviewer_result(
                subagent_type,
                reason,
                duration_ms,
            );

        ToolResult::Result {
            data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }
    }
}
