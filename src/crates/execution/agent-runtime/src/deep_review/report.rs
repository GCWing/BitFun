//! Provider-neutral Deep Review report enrichment.
//!
//! This module owns JSON report facts that do not require product session IO,
//! global coordinators, event emitters, or host-specific tool context.

use super::incremental_cache::DeepReviewIncrementalCache;
use super::manifest::{DeepReviewEvidencePack, DeepReviewScopeProfile};
use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepReviewCacheUpdate {
    pub value: Value,
    pub hit_count: usize,
    pub miss_count: usize,
}

fn normalized_non_empty_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn packet_string_field<'a>(packet: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| packet.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn reviewer_match_tokens(reviewer: &Value) -> Vec<String> {
    ["name", "specialty"]
        .iter()
        .filter_map(|key| normalized_non_empty_string(reviewer.get(*key)))
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn packet_match_tokens(packet: &Value) -> Vec<String> {
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

fn infer_unique_packet_id_for_reviewer(
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

pub fn fill_deep_review_packet_metadata(input: &mut Value, run_manifest: Option<&Value>) {
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

fn value_for_any_key<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| value.get(*key))
}

fn bool_for_any_key(value: &Value, keys: &[&str]) -> bool {
    value_for_any_key(value, keys)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn u64_for_any_key(value: &Value, keys: &[&str]) -> Option<u64> {
    value_for_any_key(value, keys).and_then(Value::as_u64)
}

fn has_non_empty_array_for_any_key(value: &Value, keys: &[&str]) -> bool {
    value_for_any_key(value, keys)
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
}

fn count_partial_reviewers(input: &Value) -> usize {
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

fn count_manifest_skipped_reviewers(run_manifest: Option<&Value>) -> usize {
    run_manifest
        .and_then(|manifest| {
            value_for_any_key(manifest, &["skippedReviewers", "skipped_reviewers"])
        })
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

fn count_token_budget_limited_reviewers(run_manifest: Option<&Value>) -> usize {
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

fn count_decision_items(input: &Value) -> usize {
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

fn has_reliability_signal(input: &Value, kind: &str) -> bool {
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

pub fn push_reliability_signal_if_missing(input: &mut Value, signal: Value) {
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

pub fn fill_deep_review_reliability_signals(
    input: &mut Value,
    run_manifest: Option<&Value>,
    compression_preserved_signal_count: Option<usize>,
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

    if let Some(count) = compression_preserved_signal_count.filter(|count| *count > 0) {
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

fn deep_review_cache_fingerprint(run_manifest: Option<&Value>) -> Option<String> {
    let manifest = run_manifest?;
    let cache_config = value_for_any_key(
        manifest,
        &["incrementalReviewCache", "incremental_review_cache"],
    )?;
    packet_string_field(cache_config, &["fingerprint"]).map(str::to_string)
}

pub fn deep_review_cache_from_completed_reviewers(
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
