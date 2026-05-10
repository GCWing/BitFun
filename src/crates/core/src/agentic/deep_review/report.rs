//! Deep Review report enrichment, diagnostics logging, and cache write-through.
//!
//! Report enrichment must be honest about queue skips, retries, reduced-depth
//! coverage, evidence hints, and cache reuse. Standard Code Review output
//! should only receive Deep Review-only metadata when the tool context proves a
//! Deep Review run is active.

use crate::agentic::agents::get_agent_registry;
use crate::agentic::context_profile::ContextProfilePolicy;
use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::core::CompressionContract;
use crate::agentic::deep_review::manifest::{DeepReviewEvidencePack, DeepReviewScopeProfile};
use crate::agentic::deep_review_policy::{
    deep_review_capacity_skip_count, deep_review_concurrency_cap_rejection_count,
    deep_review_runtime_diagnostics_snapshot, DeepReviewIncrementalCache,
    DeepReviewRuntimeDiagnostics,
};
use crate::agentic::tools::framework::ToolUseContext;
use crate::util::errors::BitFunResult;
use log::debug;
use serde_json::{json, Value};
use std::collections::HashSet;

pub(crate) struct DeepReviewCacheUpdate {
    pub(crate) value: Value,
    pub(crate) hit_count: usize,
    pub(crate) miss_count: usize,
}

pub(crate) fn is_deep_review_context(context: Option<&ToolUseContext>) -> bool {
    context
        .and_then(|context| context.agent_type.as_deref())
        .map(str::trim)
        .is_some_and(|agent_type| agent_type == "DeepReview")
}

pub(crate) fn normalized_non_empty_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn packet_string_field<'a>(packet: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| packet.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(crate) fn reviewer_match_tokens(reviewer: &Value) -> Vec<String> {
    ["name", "specialty"]
        .iter()
        .filter_map(|key| normalized_non_empty_string(reviewer.get(*key)))
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

pub(crate) fn packet_match_tokens(packet: &Value) -> Vec<String> {
    [
        &["subagentId", "subagent_id", "subagent_type"][..],
        &["displayName", "display_name"][..],
        &["roleName", "role"][..],
    ]
    .iter()
    .filter_map(|keys| packet_string_field(packet, keys))
    .map(|value| value.to_ascii_lowercase())
    .collect()
}

pub(crate) fn infer_unique_packet_id_for_reviewer(
    reviewer: &Value,
    run_manifest: Option<&Value>,
) -> Option<String> {
    let reviewer_tokens = reviewer_match_tokens(reviewer);
    if reviewer_tokens.is_empty() {
        return None;
    }

    let manifest = run_manifest?;
    let packets = manifest
        .get("workPackets")
        .or_else(|| manifest.get("work_packets"))?
        .as_array()?;
    let mut matches = packets.iter().filter_map(|packet| {
        let packet_id = packet_string_field(packet, &["packetId", "packet_id"])?;
        let packet_tokens = packet_match_tokens(packet);
        let matched = packet_tokens
            .iter()
            .any(|packet_token| reviewer_tokens.iter().any(|token| token == packet_token));
        matched.then(|| packet_id.to_string())
    });
    let first = matches.next()?;
    if matches.next().is_some() {
        None
    } else {
        Some(first)
    }
}

pub(crate) fn fill_deep_review_packet_metadata(input: &mut Value, run_manifest: Option<&Value>) {
    let Some(reviewers) = input.get_mut("reviewers").and_then(Value::as_array_mut) else {
        return;
    };

    for reviewer in reviewers {
        let packet_id = normalized_non_empty_string(reviewer.get("packet_id"));
        let packet_status_source =
            normalized_non_empty_string(reviewer.get("packet_status_source"));
        let inferred_packet_id = if packet_id.is_none() {
            infer_unique_packet_id_for_reviewer(reviewer, run_manifest)
        } else {
            None
        };

        let Some(object) = reviewer.as_object_mut() else {
            continue;
        };

        if packet_id.is_some() {
            if packet_status_source.is_none() {
                object.insert("packet_status_source".to_string(), json!("reported"));
            }
        } else if let Some(inferred_packet_id) = inferred_packet_id {
            object.insert("packet_id".to_string(), json!(inferred_packet_id));
            object.insert("packet_status_source".to_string(), json!("inferred"));
        } else if packet_status_source.is_none() {
            object.insert("packet_status_source".to_string(), json!("missing"));
        }
    }
}

pub(crate) fn value_for_any_key<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| value.get(*key))
}

pub(crate) fn bool_for_any_key(value: &Value, keys: &[&str]) -> bool {
    value_for_any_key(value, keys)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(crate) fn u64_for_any_key(value: &Value, keys: &[&str]) -> Option<u64> {
    value_for_any_key(value, keys).and_then(Value::as_u64)
}

pub(crate) fn has_non_empty_array_for_any_key(value: &Value, keys: &[&str]) -> bool {
    value_for_any_key(value, keys)
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
}

pub(crate) fn count_partial_reviewers(input: &Value) -> usize {
    input
        .get("reviewers")
        .and_then(Value::as_array)
        .map(|reviewers| {
            reviewers
                .iter()
                .filter(|reviewer| {
                    let status = reviewer
                        .get("status")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .unwrap_or_default();
                    let has_partial_output = reviewer
                        .get("partial_output")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .is_some_and(|output| !output.is_empty());
                    status == "partial_timeout"
                        || (matches!(status, "timed_out" | "cancelled_by_user")
                            && has_partial_output)
                })
                .count()
        })
        .unwrap_or(0)
}

pub(crate) fn count_manifest_skipped_reviewers(run_manifest: Option<&Value>) -> usize {
    run_manifest
        .and_then(|manifest| {
            value_for_any_key(manifest, &["skippedReviewers", "skipped_reviewers"])
        })
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

pub(crate) fn count_token_budget_limited_reviewers(run_manifest: Option<&Value>) -> usize {
    let Some(manifest) = run_manifest else {
        return 0;
    };
    let mut skipped_by_budget = HashSet::new();

    if let Some(skipped_ids) = value_for_any_key(manifest, &["tokenBudget", "token_budget"])
        .and_then(|token_budget| {
            value_for_any_key(
                token_budget,
                &["skippedReviewerIds", "skipped_reviewer_ids"],
            )
        })
        .and_then(Value::as_array)
    {
        for value in skipped_ids {
            if let Some(id) = value.as_str().map(str::trim).filter(|id| !id.is_empty()) {
                skipped_by_budget.insert(id.to_string());
            }
        }
    }

    if let Some(skipped_reviewers) =
        value_for_any_key(manifest, &["skippedReviewers", "skipped_reviewers"])
            .and_then(Value::as_array)
    {
        for reviewer in skipped_reviewers {
            let reason = packet_string_field(reviewer, &["reason"]);
            if reason != Some("budget_limited") {
                continue;
            }
            if let Some(id) = packet_string_field(reviewer, &["subagentId", "subagent_id"]) {
                skipped_by_budget.insert(id.to_string());
            }
        }
    }

    skipped_by_budget.len()
}

pub(crate) fn count_decision_items(input: &Value) -> usize {
    let needs_decision_count = input
        .pointer("/report_sections/remediation_groups/needs_decision")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .count()
        })
        .unwrap_or(0);
    if needs_decision_count > 0 {
        return needs_decision_count;
    }

    let recommended_action = input
        .pointer("/summary/recommended_action")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    usize::from(recommended_action == "block")
}

pub(crate) fn has_reliability_signal(input: &Value, kind: &str) -> bool {
    input
        .get("reliability_signals")
        .and_then(Value::as_array)
        .is_some_and(|signals| {
            signals.iter().any(|signal| {
                signal
                    .get("kind")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value == kind)
            })
        })
}

pub(crate) fn push_reliability_signal_if_missing(input: &mut Value, signal: Value) {
    let Some(kind) = signal.get("kind").and_then(Value::as_str) else {
        return;
    };
    if has_reliability_signal(input, kind) {
        return;
    }
    if !input
        .get("reliability_signals")
        .is_some_and(Value::is_array)
    {
        input["reliability_signals"] = json!([]);
    }
    if let Some(signals) = input
        .get_mut("reliability_signals")
        .and_then(Value::as_array_mut)
    {
        signals.push(signal);
    }
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
    if let Some(scope_profile) = run_manifest.and_then(DeepReviewScopeProfile::from_manifest) {
        if scope_profile.is_reduced_depth() {
            let mut signal = json!({
                "kind": "reduced_scope",
                "severity": "info",
                "source": "manifest"
            });
            if let Some(detail) = scope_profile.coverage_expectation() {
                signal["detail"] = json!(detail);
            }
            push_reliability_signal_if_missing(input, signal);
        }
    }

    if let Some(manifest) = run_manifest {
        if let Err(error) = DeepReviewEvidencePack::from_manifest(manifest) {
            push_reliability_signal_if_missing(
                input,
                json!({
                    "kind": "context_pressure",
                    "severity": "warning",
                    "source": "manifest",
                    "detail": format!("Evidence pack ignored: {}", error)
                }),
            );
        }
    }

    if let Some(token_budget) = run_manifest
        .and_then(|manifest| value_for_any_key(manifest, &["tokenBudget", "token_budget"]))
    {
        let has_context_pressure =
            bool_for_any_key(
                token_budget,
                &["largeDiffSummaryFirst", "large_diff_summary_first"],
            ) || has_non_empty_array_for_any_key(token_budget, &["warnings"]);
        if has_context_pressure {
            let count = u64_for_any_key(
                token_budget,
                &["estimatedReviewerCalls", "estimated_reviewer_calls"],
            )
            .unwrap_or(0);
            push_reliability_signal_if_missing(
                input,
                json!({
                    "kind": "context_pressure",
                    "severity": "info",
                    "count": count,
                    "source": "runtime"
                }),
            );
        }
    }

    let skipped_reviewer_count = count_manifest_skipped_reviewers(run_manifest);
    if skipped_reviewer_count > 0 {
        push_reliability_signal_if_missing(
            input,
            json!({
                "kind": "skipped_reviewers",
                "severity": "info",
                "count": skipped_reviewer_count,
                "source": "manifest"
            }),
        );
    }

    let token_budget_limited_reviewer_count = count_token_budget_limited_reviewers(run_manifest);
    if token_budget_limited_reviewer_count > 0 {
        push_reliability_signal_if_missing(
            input,
            json!({
                "kind": "token_budget_limited",
                "severity": "warning",
                "count": token_budget_limited_reviewer_count,
                "source": "manifest"
            }),
        );
    }

    if let Some(contract) = compression_contract.filter(|contract| !contract.is_empty()) {
        let count = compression_contract_signal_count(contract);
        if count > 0 {
            push_reliability_signal_if_missing(
                input,
                json!({
                    "kind": "compression_preserved",
                    "severity": "info",
                    "count": count,
                    "source": "runtime"
                }),
            );
        }
    }

    let partial_reviewer_count = count_partial_reviewers(input);
    if partial_reviewer_count > 0 {
        push_reliability_signal_if_missing(
            input,
            json!({
                "kind": "partial_reviewer",
                "severity": "warning",
                "count": partial_reviewer_count,
                "source": "runtime"
            }),
        );
    }

    if partial_reviewer_count > 0 {
        push_reliability_signal_if_missing(
            input,
            json!({
                "kind": "retry_guidance",
                "severity": "warning",
                "count": partial_reviewer_count,
                "source": "runtime"
            }),
        );
    }

    let decision_item_count = count_decision_items(input);
    if decision_item_count > 0 {
        push_reliability_signal_if_missing(
            input,
            json!({
                "kind": "user_decision",
                "severity": "action",
                "count": decision_item_count,
                "source": "report"
            }),
        );
    }
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
        push_reliability_signal_if_missing(
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

pub(crate) fn deep_review_cache_fingerprint(run_manifest: Option<&Value>) -> Option<String> {
    let manifest = run_manifest?;
    let cache_config = value_for_any_key(
        manifest,
        &["incrementalReviewCache", "incremental_review_cache"],
    )?;
    packet_string_field(cache_config, &["fingerprint"]).map(str::to_string)
}

pub(crate) fn deep_review_cache_from_completed_reviewers(
    input: &Value,
    run_manifest: Option<&Value>,
    existing_cache: Option<&Value>,
) -> Option<DeepReviewCacheUpdate> {
    let fingerprint = deep_review_cache_fingerprint(run_manifest)?;
    let matching_existing_cache = existing_cache
        .map(DeepReviewIncrementalCache::from_value)
        .filter(|cache| cache.fingerprint() == fingerprint);
    let mut cache = matching_existing_cache
        .clone()
        .unwrap_or_else(|| DeepReviewIncrementalCache::new(&fingerprint));
    let mut stored_count = 0usize;
    let mut hit_count = 0usize;
    let mut miss_count = 0usize;

    if let Some(reviewers) = input.get("reviewers").and_then(Value::as_array) {
        for reviewer in reviewers {
            let is_completed = reviewer
                .get("status")
                .and_then(Value::as_str)
                .map(str::trim)
                .is_some_and(|status| status == "completed");
            if !is_completed {
                continue;
            }
            let Some(packet_id) = normalized_non_empty_string(reviewer.get("packet_id")) else {
                continue;
            };
            if matching_existing_cache
                .as_ref()
                .and_then(|cache| cache.get_packet(&packet_id))
                .is_some()
            {
                hit_count += 1;
            } else {
                miss_count += 1;
            }
            let output = serde_json::to_string(reviewer).unwrap_or_else(|_| reviewer.to_string());
            cache.store_packet(&packet_id, &output);
            stored_count += 1;
        }
    }

    (stored_count > 0).then(|| DeepReviewCacheUpdate {
        value: cache.to_value(),
        hit_count,
        miss_count,
    })
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
