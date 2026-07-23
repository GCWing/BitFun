//! Provider-neutral Deep Review report enrichment.
//!
//! This module owns JSON report facts that do not require product session IO,
//! global coordinators, event emitters, or host-specific tool context.

use super::incremental_cache::DeepReviewIncrementalCache;
use super::manifest::{DeepReviewEvidencePack, DeepReviewScopeProfile};
use super::target_evidence::{
    ReviewTargetEvidence, ReviewTargetEvidenceCompleteness, ReviewTargetWorkspaceBinding,
};
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

pub fn fill_deep_review_runtime_tracker_signal(
    input: &mut Value,
    concurrency_limited_count: usize,
) {
    if concurrency_limited_count == 0 {
        return;
    }

    push_reliability_signal_if_missing(
        input,
        json!({
            "kind": "concurrency_limited",
            "severity": "warning",
            "count": concurrency_limited_count,
            "source": "runtime"
        }),
    );
}

pub fn fill_deep_review_cache_update_signals(
    input: &mut Value,
    cache_update: &DeepReviewCacheUpdate,
) {
    if cache_update.hit_count > 0 {
        push_reliability_signal_if_missing(
            input,
            json!({
                "kind": "cache_hit",
                "severity": "info",
                "count": cache_update.hit_count,
                "source": "runtime"
            }),
        );
    }
    if cache_update.miss_count > 0 {
        push_reliability_signal_if_missing(
            input,
            json!({
                "kind": "cache_miss",
                "severity": "info",
                "count": cache_update.miss_count,
                "source": "runtime"
            }),
        );
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

        match ReviewTargetEvidence::from_manifest(manifest) {
            Ok(Some(evidence))
                if evidence.completeness() != ReviewTargetEvidenceCompleteness::Complete
                    || !evidence.limitations().is_empty() =>
            {
                push_reliability_signal_if_missing(
                    input,
                    json!({
                        "kind": "target_evidence_limited",
                        "severity": "warning",
                        "source": "manifest"
                    }),
                );
            }
            Err(_) => push_reliability_signal_if_missing(
                input,
                json!({
                    "kind": "target_evidence_limited",
                    "severity": "warning",
                    "source": "manifest"
                }),
            ),
            _ => {}
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

fn target_evidence_status(run_manifest: Option<&Value>) -> Option<&'static str> {
    let manifest = run_manifest?;
    match ReviewTargetEvidence::from_manifest(manifest) {
        Ok(Some(evidence)) => match evidence.completeness() {
            ReviewTargetEvidenceCompleteness::Stale => Some("stale"),
            ReviewTargetEvidenceCompleteness::Complete
                if evidence.omitted_file_count() == 0
                    && (evidence.source()
                        == super::target_evidence::ReviewTargetEvidenceSource::PullRequest
                        || (evidence.source()
                            == super::target_evidence::ReviewTargetEvidenceSource::GitRange
                            && evidence.workspace_binding()
                                == ReviewTargetWorkspaceBinding::MatchingClean)) =>
            {
                Some("complete")
            }
            ReviewTargetEvidenceCompleteness::Complete
            | ReviewTargetEvidenceCompleteness::Partial
            | ReviewTargetEvidenceCompleteness::Unknown => Some("limited"),
        },
        Ok(None) => None,
        Err(_) => Some("failed"),
    }
}

pub fn apply_review_evidence_guardrail(input: &mut Value, run_manifest: Option<&Value>) {
    if input
        .get("evidence_status")
        .and_then(Value::as_str)
        .is_some_and(|status| status == "failed")
    {
        return;
    }

    let Some(status) = target_evidence_status(run_manifest) else {
        return;
    };
    input["evidence_status"] = json!(status);
    if status == "complete" {
        return;
    }

    let detail = match status {
        "stale" => {
            "Review evidence changed during the run; rerun before relying on a clean result."
        }
        "failed" => "Review evidence was unavailable or invalid; a clean result is not allowed.",
        _ => "Review evidence is limited; a clean result is not allowed.",
    };
    push_reliability_signal_if_missing(
        input,
        json!({
            "kind": "target_evidence_limited",
            "severity": "warning",
            "source": "runtime",
            "detail": detail
        }),
    );
}

pub fn apply_review_runtime_limitation(input: &mut Value, detail: &str) {
    if input.get("evidence_status").and_then(Value::as_str) != Some("failed") {
        input["evidence_status"] = json!("limited");
    }
    push_reliability_signal_if_missing(
        input,
        json!({
            "kind": "target_evidence_limited",
            "severity": "warning",
            "source": "runtime",
            "detail": detail
        }),
    );
}

pub fn apply_review_runtime_stale(input: &mut Value) {
    if input.get("evidence_status").and_then(Value::as_str) != Some("failed") {
        input["evidence_status"] = json!("stale");
    }
    push_reliability_signal_if_missing(
        input,
        json!({
            "kind": "target_evidence_limited",
            "severity": "warning",
            "source": "runtime",
            "detail": "Pull request revisions changed during the Review; rerun before relying on the result."
        }),
    );
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn runtime_tracker_signal_adds_concurrency_limited_warning_once() {
        let mut input = json!({
            "summary": {
                "overall_assessment": "Review completed"
            }
        });

        fill_deep_review_runtime_tracker_signal(&mut input, 3);
        fill_deep_review_runtime_tracker_signal(&mut input, 3);

        assert_eq!(
            input["reliability_signals"],
            json!([{
                "kind": "concurrency_limited",
                "severity": "warning",
                "count": 3,
                "source": "runtime"
            }])
        );
    }

    #[test]
    fn runtime_tracker_signal_ignores_zero_count() {
        let mut input = json!({});

        fill_deep_review_runtime_tracker_signal(&mut input, 0);

        assert!(input.get("reliability_signals").is_none());
    }

    #[test]
    fn target_evidence_limit_has_a_distinct_warning_signal() {
        let manifest = json!({
            "reviewTargetEvidence": {
                "version": 1,
                "source": "git_range",
                "fingerprint": "0123456789abcdef",
                "baseRevision": "1111111111111111111111111111111111111111",
                "headRevision": "2222222222222222222222222222222222222222",
                "completeness": "partial",
                "workspaceBinding": "matching_clean",
                "files": [{
                    "path": "src/lib.rs",
                    "status": "modified",
                    "completeness": "partial"
                }],
                "limitations": ["git_diff_unavailable"]
            }
        });
        let mut input = json!({});

        fill_deep_review_reliability_signals(&mut input, Some(&manifest), None);

        assert_eq!(
            input["reliability_signals"][0]["kind"],
            "target_evidence_limited"
        );
        assert_eq!(input["reliability_signals"][0]["severity"], "warning");
    }

    #[test]
    fn complete_git_range_preserves_clean_recommendation() {
        let manifest = json!({
            "reviewTargetEvidence": {
                "version": 1,
                "source": "git_range",
                "fingerprint": "0123456789abcdef",
                "baseRevision": "1111111111111111111111111111111111111111",
                "headRevision": "2222222222222222222222222222222222222222",
                "completeness": "complete",
                "workspaceBinding": "matching_clean",
                "files": [{
                    "path": "src/lib.rs",
                    "status": "modified",
                    "completeness": "complete"
                }],
                "limitations": [],
                "omittedFileCount": 0
            }
        });
        let mut input = json!({
            "summary": {
                "overall_assessment": "No blocking issues",
                "risk_level": "low",
                "recommended_action": "approve"
            }
        });

        apply_review_evidence_guardrail(&mut input, Some(&manifest));

        assert_eq!(input["evidence_status"], "complete");
        assert_eq!(input["summary"]["recommended_action"], "approve");
        assert_eq!(input["summary"]["risk_level"], "low");
    }

    #[test]
    fn complete_pull_request_evidence_can_report_complete_without_live_workspace_binding() {
        let manifest = json!({
            "reviewTargetEvidence": {
                "version": 1,
                "source": "pull_request",
                "fingerprint": "0123456789abcdef",
                "baseRevision": "1111111111111111111111111111111111111111",
                "headRevision": "2222222222222222222222222222222222222222",
                "completeness": "complete",
                "workspaceBinding": "unavailable",
                "pullRequest": {
                    "remoteId": "origin|https://github.com/example/repo.git",
                    "platform": "github",
                    "host": "github.com",
                    "projectPath": "example/repo",
                    "pullRequestId": "42",
                    "number": 42,
                    "webUrl": "https://github.com/example/repo/pull/42"
                },
                "files": [{
                    "path": "src/lib.rs",
                    "status": "modified",
                    "completeness": "complete"
                }],
                "limitations": [],
                "omittedFileCount": 0
            }
        });
        let mut input = json!({
            "summary": {
                "overall_assessment": "No blocking issues",
                "risk_level": "low",
                "recommended_action": "approve"
            }
        });

        apply_review_evidence_guardrail(&mut input, Some(&manifest));

        assert_eq!(input["evidence_status"], "complete");
        assert_eq!(input["summary"]["recommended_action"], "approve");
    }

    #[test]
    fn dirty_git_binding_is_limited_even_when_the_range_is_complete() {
        let manifest = json!({
            "reviewTargetEvidence": {
                "version": 1,
                "source": "git_range",
                "fingerprint": "0123456789abcdef",
                "baseRevision": "1111111111111111111111111111111111111111",
                "headRevision": "2222222222222222222222222222222222222222",
                "completeness": "complete",
                "workspaceBinding": "matching_dirty",
                "files": [{
                    "path": "src/lib.rs",
                    "status": "modified",
                    "completeness": "complete"
                }],
                "limitations": [],
                "omittedFileCount": 0
            }
        });
        let mut input = json!({
            "summary": {
                "overall_assessment": "No blocking issues",
                "risk_level": "low",
                "recommended_action": "approve"
            }
        });

        apply_review_evidence_guardrail(&mut input, Some(&manifest));

        assert_eq!(input["evidence_status"], "limited");
        assert_eq!(input["summary"]["recommended_action"], "approve");
    }

    #[test]
    fn mutable_workspace_marks_evidence_limited_without_rewriting_the_decision() {
        let manifest = json!({
            "reviewTargetEvidence": {
                "version": 1,
                "source": "workspace",
                "fingerprint": "fedcba9876543210",
                "baseRevision": "1111111111111111111111111111111111111111",
                "headRevision": "WORKTREE",
                "completeness": "complete",
                "workspaceBinding": "matching_dirty",
                "files": [{
                    "path": "src/lib.rs",
                    "status": "modified",
                    "completeness": "complete"
                }],
                "limitations": []
            }
        });
        let mut input = json!({
            "summary": {
                "overall_assessment": "No blocking issues",
                "risk_level": "low",
                "recommended_action": "approve"
            }
        });

        apply_review_evidence_guardrail(&mut input, Some(&manifest));

        assert_eq!(input["evidence_status"], "limited");
        assert_eq!(input["summary"]["recommended_action"], "approve");
        assert_eq!(input["summary"]["risk_level"], "low");
        assert_eq!(
            input["reliability_signals"][0]["kind"],
            "target_evidence_limited"
        );
    }

    #[test]
    fn legacy_manifest_without_target_evidence_preserves_the_report() {
        let manifest = json!({ "reviewMode": "deep", "workPackets": [] });
        let mut input = json!({
            "summary": {
                "overall_assessment": "No blocking issues",
                "risk_level": "low",
                "recommended_action": "approve"
            }
        });

        apply_review_evidence_guardrail(&mut input, Some(&manifest));

        assert!(input.get("evidence_status").is_none());
        assert!(input.get("reliability_signals").is_none());
        assert_eq!(input["summary"]["recommended_action"], "approve");
    }

    #[test]
    fn runtime_stale_overrides_a_prepared_complete_target() {
        let mut input = json!({ "evidence_status": "complete" });

        apply_review_runtime_stale(&mut input);

        assert_eq!(input["evidence_status"], "stale");
        assert_eq!(
            input["reliability_signals"][0]["kind"],
            "target_evidence_limited"
        );
    }

    #[test]
    fn invalid_target_evidence_fails_without_rewriting_the_decision() {
        let manifest = json!({ "reviewTargetEvidence": { "version": 1 } });
        let mut input = json!({
            "summary": {
                "overall_assessment": "Issues found",
                "risk_level": "high",
                "recommended_action": "request_changes"
            }
        });

        apply_review_evidence_guardrail(&mut input, Some(&manifest));

        assert_eq!(input["evidence_status"], "failed");
        assert_eq!(input["summary"]["risk_level"], "high");
        assert_eq!(input["summary"]["recommended_action"], "request_changes");
    }

    #[test]
    fn cache_update_signal_shaping_stays_runtime_owned() {
        let mut input = json!({});
        let cache_update = DeepReviewCacheUpdate {
            value: json!({ "fingerprint": "fp-test", "packets": {} }),
            hit_count: 2,
            miss_count: 1,
        };

        fill_deep_review_cache_update_signals(&mut input, &cache_update);

        assert_eq!(
            input["reliability_signals"],
            json!([
                {
                    "kind": "cache_hit",
                    "severity": "info",
                    "count": 2,
                    "source": "runtime"
                },
                {
                    "kind": "cache_miss",
                    "severity": "info",
                    "count": 1,
                    "source": "runtime"
                }
            ])
        );
    }

    #[test]
    fn incremental_cache_stores_completed_reviewers_by_packet_id() {
        let manifest = json!({
            "incrementalReviewCache": {
                "fingerprint": "fp-review-v2"
            },
            "workPackets": [
                {
                    "packetId": "reviewer:ReviewSecurity:group-1-of-1",
                    "phase": "reviewer",
                    "subagentId": "ReviewSecurity",
                    "displayName": "Security Reviewer"
                },
                {
                    "packetId": "reviewer:ReviewPerformance:group-1-of-1",
                    "phase": "reviewer",
                    "subagentId": "ReviewPerformance",
                    "displayName": "Performance Reviewer"
                }
            ]
        });
        let mut input = json!({
            "summary": {
                "overall_assessment": "Review completed",
                "risk_level": "medium",
                "recommended_action": "request_changes"
            },
            "issues": [],
            "positive_points": [],
            "reviewers": [
                {
                    "name": "Security Reviewer",
                    "specialty": "security",
                    "status": "completed",
                    "summary": "Found one high-risk issue."
                },
                {
                    "name": "Performance Reviewer",
                    "specialty": "performance",
                    "status": "partial_timeout",
                    "summary": "Timed out before completion.",
                    "partial_output": "Large render path was still being checked."
                }
            ]
        });
        fill_deep_review_packet_metadata(&mut input, Some(&manifest));

        let cache_update =
            deep_review_cache_from_completed_reviewers(&input, Some(&manifest), None)
                .expect("completed reviewer should produce cache value");
        let cache = DeepReviewIncrementalCache::from_value(&cache_update.value);

        assert_eq!(cache.fingerprint(), "fp-review-v2");
        assert_eq!(cache_update.hit_count, 0);
        assert_eq!(cache_update.miss_count, 1);
        assert!(cache
            .get_packet("reviewer:ReviewSecurity:group-1-of-1")
            .is_some_and(|output| output.contains("Found one high-risk issue.")));
        assert_eq!(
            cache.get_packet("reviewer:ReviewPerformance:group-1-of-1"),
            None
        );
    }

    #[test]
    fn incremental_cache_replaces_stale_existing_cache() {
        let manifest = json!({
            "incrementalReviewCache": {
                "fingerprint": "fp-new"
            },
            "workPackets": [
                {
                    "packetId": "reviewer:ReviewSecurity",
                    "phase": "reviewer",
                    "subagentId": "ReviewSecurity",
                    "displayName": "Security Reviewer"
                }
            ]
        });
        let mut stale_cache = DeepReviewIncrementalCache::new("fp-old");
        stale_cache.store_packet("reviewer:ReviewSecurity", "stale output");
        let mut input = json!({
            "summary": {
                "overall_assessment": "Review completed",
                "risk_level": "low",
                "recommended_action": "approve"
            },
            "issues": [],
            "positive_points": [],
            "reviewers": [
                {
                    "name": "Security Reviewer",
                    "specialty": "security",
                    "status": "completed",
                    "summary": "Fresh security output."
                }
            ]
        });
        fill_deep_review_packet_metadata(&mut input, Some(&manifest));

        let cache_update = deep_review_cache_from_completed_reviewers(
            &input,
            Some(&manifest),
            Some(&stale_cache.to_value()),
        )
        .expect("completed reviewer should replace stale cache");
        let cache = DeepReviewIncrementalCache::from_value(&cache_update.value);

        assert_eq!(cache.fingerprint(), "fp-new");
        assert_eq!(cache_update.hit_count, 0);
        assert_eq!(cache_update.miss_count, 1);
        assert!(cache
            .get_packet("reviewer:ReviewSecurity")
            .is_some_and(|output| output.contains("Fresh security output.")));
        assert!(!cache
            .get_packet("reviewer:ReviewSecurity")
            .is_some_and(|output| output.contains("stale output")));
    }

    #[test]
    fn incremental_cache_counts_existing_packet_hits() {
        let manifest = json!({
            "incrementalReviewCache": {
                "fingerprint": "fp-existing"
            },
            "workPackets": [
                {
                    "packetId": "reviewer:ReviewSecurity",
                    "phase": "reviewer",
                    "subagentId": "ReviewSecurity",
                    "displayName": "Security Reviewer"
                },
                {
                    "packetId": "reviewer:ReviewPerformance",
                    "phase": "reviewer",
                    "subagentId": "ReviewPerformance",
                    "displayName": "Performance Reviewer"
                }
            ]
        });
        let mut existing_cache = DeepReviewIncrementalCache::new("fp-existing");
        existing_cache.store_packet("reviewer:ReviewSecurity", "cached security output");
        let mut input = json!({
            "summary": {
                "overall_assessment": "Review completed",
                "risk_level": "medium",
                "recommended_action": "request_changes"
            },
            "issues": [],
            "positive_points": [],
            "reviewers": [
                {
                    "name": "Security Reviewer",
                    "specialty": "security",
                    "status": "completed",
                    "summary": "Reused security output."
                },
                {
                    "name": "Performance Reviewer",
                    "specialty": "performance",
                    "status": "completed",
                    "summary": "Fresh performance output."
                }
            ]
        });
        fill_deep_review_packet_metadata(&mut input, Some(&manifest));

        let cache_update = deep_review_cache_from_completed_reviewers(
            &input,
            Some(&manifest),
            Some(&existing_cache.to_value()),
        )
        .expect("completed reviewers should update cache");

        assert_eq!(cache_update.hit_count, 1);
        assert_eq!(cache_update.miss_count, 1);
    }
}
