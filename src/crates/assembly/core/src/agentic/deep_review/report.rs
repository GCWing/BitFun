//! Deep Review report product assembly bridge.
//!
//! Provider-neutral packet metadata, reliability-signal shaping, and cache
//! updates are owned by `bitfun-agent-runtime::deep_review::report`. This file
//! keeps core-only context lookup, diagnostics logging, and session metadata IO.

use crate::agentic::agents::get_agent_registry;
use crate::agentic::context_profile::ContextProfilePolicy;
use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::core::CompressionContract;
use crate::agentic::deep_review_policy::{
    deep_review_capacity_skip_count, deep_review_concurrency_cap_rejection_count,
    deep_review_runtime_diagnostics_snapshot, DeepReviewRuntimeDiagnostics,
};
use crate::agentic::tools::framework::ToolUseContext;
use crate::util::errors::BitFunResult;
use bitfun_agent_runtime::deep_review::report as runtime_report;
pub(crate) use bitfun_agent_runtime::deep_review::report::{
    deep_review_cache_from_completed_reviewers, fill_deep_review_packet_metadata,
    push_reliability_signal_if_missing, DeepReviewCacheUpdate,
};
use log::debug;
use serde_json::{json, Value};

pub(crate) fn is_deep_review_context(context: Option<&ToolUseContext>) -> bool {
    context
        .and_then(|context| context.agent_type.as_deref())
        .map(str::trim)
        .is_some_and(|agent_type| agent_type == "DeepReview")
}

pub(crate) fn compression_contract_for_context(
    context: &ToolUseContext,
) -> Option<CompressionContract> {
    let session_id = context.session_id.as_deref()?;
    let coordinator = get_global_coordinator()?;
    let session = coordinator.get_session_manager().get_session(session_id)?;
    let agent_type = Some(session.agent_type.as_str());
    let model_id = session.config.model_id.as_deref();
    let limit = reliability_contract_limit(agent_type, model_id);
    let contract = coordinator
        .get_session_manager()
        .compression_contract_for_session(session_id, limit)?;
    should_report_compression_preserved(
        session.compression_state.compression_count,
        Some(&contract),
    )
    .then_some(contract)
}

pub(crate) fn reliability_contract_limit(
    agent_type: Option<&str>,
    model_id: Option<&str>,
) -> usize {
    let agent_type = agent_type
        .map(str::trim)
        .filter(|agent_type| !agent_type.is_empty())
        .unwrap_or("DeepReview");
    let model_id = model_id
        .map(str::trim)
        .filter(|model_id| !model_id.is_empty())
        .unwrap_or_default();
    let is_review_subagent = get_agent_registry()
        .get_subagent_is_review(agent_type)
        .unwrap_or(false);

    ContextProfilePolicy::for_agent_context_and_model(
        agent_type,
        is_review_subagent,
        model_id,
        model_id,
    )
    .compression_contract_limit
}

pub(crate) fn should_report_compression_preserved(
    compression_count: usize,
    compression_contract: Option<&CompressionContract>,
) -> bool {
    compression_count > 0 && compression_contract.is_some_and(|contract| !contract.is_empty())
}

pub(crate) fn compression_contract_signal_count(contract: &CompressionContract) -> usize {
    contract.touched_files.len()
        + contract.verification_commands.len()
        + contract.blocking_failures.len()
        + contract.subagent_statuses.len()
}

pub(crate) fn fill_deep_review_reliability_signals(
    input: &mut Value,
    run_manifest: Option<&Value>,
    compression_contract: Option<&CompressionContract>,
) {
    let compression_preserved_signal_count = compression_contract
        .filter(|contract| !contract.is_empty())
        .map(compression_contract_signal_count)
        .filter(|count| *count > 0);
    runtime_report::fill_deep_review_reliability_signals(
        input,
        run_manifest,
        compression_preserved_signal_count,
    );
}

pub(crate) fn fill_deep_review_runtime_tracker_signals(
    input: &mut Value,
    dialog_turn_id: Option<&str>,
) {
    let Some(dialog_turn_id) = dialog_turn_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let count = deep_review_concurrency_cap_rejection_count(dialog_turn_id)
        + deep_review_capacity_skip_count(dialog_turn_id);
    if count > 0 {
        runtime_report::push_reliability_signal_if_missing(
            input,
            json!({
                "kind": "concurrency_limited",
                "severity": "warning",
                "count": count,
                "source": "runtime"
            }),
        );
    }
}

pub(crate) fn log_deep_review_runtime_diagnostics(dialog_turn_id: Option<&str>) {
    let Some(dialog_turn_id) = dialog_turn_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let Some(DeepReviewRuntimeDiagnostics {
        queue_wait_count,
        queue_wait_total_ms,
        queue_wait_max_ms,
        provider_capacity_queue_count,
        provider_capacity_retry_count,
        provider_capacity_retry_success_count,
        capacity_skip_count,
        provider_capacity_queue_reason_counts,
        provider_capacity_retry_reason_counts,
        provider_capacity_retry_success_reason_counts,
        capacity_skip_reason_counts,
        effective_parallel_min,
        effective_parallel_final,
        manual_queue_action_count,
        manual_retry_count,
        auto_retry_count,
        auto_retry_suppressed_reason_counts,
        shared_context_total_calls,
        shared_context_duplicate_calls,
        shared_context_duplicate_context_count,
        shared_context_duplicate_savings_candidate_count,
    }) = deep_review_runtime_diagnostics_snapshot(dialog_turn_id)
    else {
        return;
    };
    let auto_retry_suppressed_reason_counts =
        serde_json::to_string(&auto_retry_suppressed_reason_counts)
            .unwrap_or_else(|_| "{}".to_string());
    let provider_capacity_queue_reason_counts =
        serde_json::to_string(&provider_capacity_queue_reason_counts)
            .unwrap_or_else(|_| "{}".to_string());
    let provider_capacity_retry_reason_counts =
        serde_json::to_string(&provider_capacity_retry_reason_counts)
            .unwrap_or_else(|_| "{}".to_string());
    let provider_capacity_retry_success_reason_counts =
        serde_json::to_string(&provider_capacity_retry_success_reason_counts)
            .unwrap_or_else(|_| "{}".to_string());
    let capacity_skip_reason_counts =
        serde_json::to_string(&capacity_skip_reason_counts).unwrap_or_else(|_| "{}".to_string());

    debug!(
        "DeepReview runtime diagnostics: queue_wait_count={}, queue_wait_total_ms={}, queue_wait_max_ms={}, provider_capacity_queue_count={}, provider_capacity_retry_count={}, provider_capacity_retry_success_count={}, capacity_skip_count={}, provider_capacity_queue_reason_counts={}, provider_capacity_retry_reason_counts={}, provider_capacity_retry_success_reason_counts={}, capacity_skip_reason_counts={}, effective_parallel_min={}, effective_parallel_final={}, manual_queue_action_count={}, manual_retry_count={}, auto_retry_count={}, auto_retry_suppressed_reason_counts={}, shared_context_total_calls={}, shared_context_duplicate_calls={}, shared_context_duplicate_context_count={}, shared_context_duplicate_savings_candidate_count={}",
        queue_wait_count,
        queue_wait_total_ms,
        queue_wait_max_ms,
        provider_capacity_queue_count,
        provider_capacity_retry_count,
        provider_capacity_retry_success_count,
        capacity_skip_count,
        provider_capacity_queue_reason_counts,
        provider_capacity_retry_reason_counts,
        provider_capacity_retry_success_reason_counts,
        capacity_skip_reason_counts,
        effective_parallel_min
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
        effective_parallel_final
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
        manual_queue_action_count,
        manual_retry_count,
        auto_retry_count,
        auto_retry_suppressed_reason_counts,
        shared_context_total_calls,
        shared_context_duplicate_calls,
        shared_context_duplicate_context_count,
        shared_context_duplicate_savings_candidate_count
    );
}

pub(crate) async fn persist_deep_review_cache(
    context: &ToolUseContext,
    cache_value: Value,
) -> BitFunResult<()> {
    let Some(session_id) = context.session_id.as_deref() else {
        return Ok(());
    };
    let Some(workspace) = context.workspace.as_ref() else {
        return Ok(());
    };
    let Some(coordinator) = get_global_coordinator() else {
        return Ok(());
    };
    let session_storage_path = workspace.session_storage_path();
    let session_manager = coordinator.get_session_manager();
    let Some(mut metadata) = session_manager
        .load_session_metadata(&session_storage_path, session_id)
        .await?
    else {
        return Ok(());
    };

    metadata.deep_review_cache = Some(cache_value);
    session_manager
        .save_session_metadata(&session_storage_path, &metadata)
        .await
}
