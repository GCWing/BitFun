use bitfun_agent_runtime::deep_review::report::{
    deep_review_cache_from_completed_reviewers, fill_deep_review_packet_metadata,
    fill_deep_review_reliability_signals,
};
use bitfun_agent_runtime::deep_review::task_execution::{
    capacity_decision_for_provider_error_facts, capacity_skip_result_for_local_queue_outcome,
    decide_blocked_reviewer_admission_queue_step, decide_provider_capacity_queue_step,
    deep_review_launch_batch_for_task, deep_review_packet_id_for_cache,
    ensure_deep_review_retry_coverage, local_reviewer_capacity_queue_decision,
    prompt_with_deep_review_retry_scope, provider_capacity_queue_wait_seconds_for_attempt,
    DeepReviewBlockedReviewerAdmissionQueueStepDecision,
    DeepReviewBlockedReviewerAdmissionQueueStepFacts, DeepReviewProviderCapacityErrorCategory,
    DeepReviewProviderCapacityErrorFacts, DeepReviewProviderCapacityQueueStepDecision,
    DeepReviewProviderCapacityQueueStepFacts,
};
use bitfun_agent_runtime::deep_review::{
    append_tool_use_context_data, apply_deep_review_queue_control,
    deep_review_queue_control_snapshot, record_deep_review_shared_context_tool_use,
    ChangeRiskFactors, DeepReviewBudgetTracker, DeepReviewCapacityQueueDecision,
    DeepReviewCapacityQueueReason, DeepReviewConcurrencyPolicy, DeepReviewExecutionPolicy,
    DeepReviewQueueControlAction, DeepReviewQueueControlSnapshot, DeepReviewQueueWaitSkipReason,
    DeepReviewRunManifestGate, DeepReviewStrategyLevel, DeepReviewSubagentRole,
    DeepReviewToolParentContext, REVIEWER_SECURITY_AGENT_TYPE,
};
use serde_json::{json, Value};
use std::collections::HashMap;

#[test]
fn deep_review_policy_owner_exposes_execution_policy_and_manifest_gate() {
    let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
        "strategy_level": "deep",
        "member_strategy_overrides": {
            "ReviewSecurity": "quick"
        }
    })));

    assert_eq!(policy.strategy_level, DeepReviewStrategyLevel::Deep);
    assert_eq!(
        policy
            .member_strategy_overrides
            .get(REVIEWER_SECURITY_AGENT_TYPE),
        Some(&DeepReviewStrategyLevel::Quick)
    );

    let risk = ChangeRiskFactors {
        file_count: 12,
        total_lines_changed: 1_500,
        files_in_security_paths: 2,
        max_cyclomatic_complexity_delta: 0,
        cross_crate_changes: 4,
    };
    let (strategy, rationale) = policy.auto_select_strategy(&risk);
    assert_eq!(strategy, DeepReviewStrategyLevel::Deep);
    assert!(rationale.contains("Deep review recommended"));

    let gate = DeepReviewRunManifestGate::from_value(&json!({
        "reviewMode": "deep",
        "workPackets": [{ "subagentId": "ReviewSecurity" }]
    }))
    .expect("deep manifest gate");
    assert!(gate.ensure_active("ReviewSecurity").is_ok());
}

#[test]
fn deep_review_runtime_owner_tracks_budget_queue_and_shared_context() {
    let tracker = DeepReviewBudgetTracker::default();
    let policy = DeepReviewExecutionPolicy::default();

    tracker
        .record_task(
            "turn-runtime-owner",
            &policy,
            DeepReviewSubagentRole::Reviewer,
            REVIEWER_SECURITY_AGENT_TYPE,
            false,
        )
        .expect("reviewer budget");
    assert_eq!(
        tracker.retries_used("turn-runtime-owner", REVIEWER_SECURITY_AGENT_TYPE),
        0
    );

    let snapshot = apply_deep_review_queue_control(
        "turn-runtime-owner",
        "tool-1",
        DeepReviewQueueControlAction::Pause,
    );
    assert!(snapshot.paused);
    assert!(
        deep_review_queue_control_snapshot("turn-runtime-owner", "tool-1").paused,
        "queue controls are runtime-owned state, not core-owned state"
    );

    let measurement = record_deep_review_shared_context_tool_use(
        "turn-runtime-owner",
        REVIEWER_SECURITY_AGENT_TYPE,
        "Read",
        "src/lib.rs",
    );
    assert_eq!(measurement.total_calls, 1);
}

#[test]
fn deep_review_tool_context_data_injection_stays_provider_neutral() {
    let mut custom_data = HashMap::<String, Value>::new();
    let context_vars = HashMap::from([
        (
            "deep_review_run_manifest".to_string(),
            r#"{"reviewMode":"deep"}"#.to_string(),
        ),
        (
            "deep_review_subagent_role".to_string(),
            "reviewer".to_string(),
        ),
        (
            "deep_review_subagent_type".to_string(),
            "ReviewSecurity".to_string(),
        ),
    ]);

    append_tool_use_context_data(
        &context_vars,
        Some(DeepReviewToolParentContext {
            tool_call_id: "tool-parent",
            session_id: "session-parent",
            dialog_turn_id: "turn-parent",
        }),
        &mut custom_data,
    );

    assert_eq!(
        custom_data
            .get("deep_review_parent_dialog_turn_id")
            .and_then(Value::as_str),
        Some("turn-parent")
    );
    assert_eq!(
        custom_data
            .get("deep_review_run_manifest")
            .and_then(|value| value.get("reviewMode"))
            .and_then(Value::as_str),
        Some("deep")
    );
}

#[test]
fn deep_review_report_owner_enriches_packet_reliability_and_cache_facts() {
    let manifest = json!({
        "reviewMode": "deep",
        "scopeProfile": {
            "reviewDepth": "high_risk_only",
            "riskFocusTags": ["security"],
            "coverageExpectation": "High-risk-only pass."
        },
        "tokenBudget": {
            "largeDiffSummaryFirst": true,
            "estimatedReviewerCalls": 2,
            "skippedReviewerIds": ["ReviewSecurity"]
        },
        "skippedReviewers": [
            { "subagentId": "ReviewSecurity", "reason": "budget_limited" }
        ],
        "incrementalReviewCache": {
            "fingerprint": "fp-runtime-report"
        },
        "workPackets": [
            {
                "packetId": "reviewer:ReviewSecurity",
                "subagentId": "ReviewSecurity"
            }
        ],
        "evidencePack": {
            "sourceText": "forbidden raw source"
        }
    });
    let mut report = json!({
        "reviewers": [
            {
                "name": "ReviewSecurity",
                "status": "completed",
                "partial_output": "Findings"
            },
            {
                "name": "ReviewArchitecture",
                "status": "partial_timeout",
                "partial_output": "Partial findings"
            }
        ],
        "summary": {
            "recommended_action": "block"
        }
    });

    fill_deep_review_packet_metadata(&mut report, Some(&manifest));
    assert_eq!(
        report["reviewers"][0]["packet_id"].as_str(),
        Some("reviewer:ReviewSecurity")
    );
    assert_eq!(
        report["reviewers"][0]["packet_status_source"].as_str(),
        Some("inferred")
    );

    fill_deep_review_reliability_signals(&mut report, Some(&manifest), Some(3));
    let signal_kinds = report["reliability_signals"]
        .as_array()
        .expect("reliability signals")
        .iter()
        .filter_map(|signal| signal.get("kind").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(signal_kinds.contains(&"reduced_scope"));
    assert!(signal_kinds.contains(&"context_pressure"));
    assert!(signal_kinds.contains(&"token_budget_limited"));
    assert!(signal_kinds.contains(&"compression_preserved"));
    assert!(signal_kinds.contains(&"partial_reviewer"));
    assert!(signal_kinds.contains(&"retry_guidance"));
    assert!(signal_kinds.contains(&"user_decision"));

    let cache_update = deep_review_cache_from_completed_reviewers(&report, Some(&manifest), None)
        .expect("cache update");
    assert_eq!(cache_update.hit_count, 0);
    assert_eq!(cache_update.miss_count, 1);
    assert!(cache_update
        .value
        .pointer("/packets/reviewer:ReviewSecurity")
        .is_some());
}

#[test]
fn deep_review_task_execution_owner_preserves_packet_retry_and_queue_contracts() {
    let manifest = json!({
        "reviewMode": "deep",
        "workPackets": [
            {
                "packetId": "security-a",
                "subagentId": "ReviewSecurity",
                "launchBatch": 2,
                "timeoutSeconds": 1200,
                "assignedScope": {
                    "files": ["src/auth.rs", "src/session.rs", "src/token.rs"]
                }
            },
            {
                "packetId": "architecture-a",
                "subagentId": "ReviewArchitecture",
                "assignedScope": {
                    "files": ["src/lib.rs"]
                }
            }
        ]
    });

    assert_eq!(
        deep_review_packet_id_for_cache(
            "ReviewSecurity",
            Some("Review [packet security-a]"),
            Some(&manifest)
        ),
        Some("security-a".to_string())
    );
    assert_eq!(
        deep_review_launch_batch_for_task("ReviewSecurity", None, Some(&manifest))
            .expect("launch batch")
            .launch_batch,
        2
    );

    let retry_input = json!({
        "timeout_seconds": 600,
        "retry_coverage": {
            "source_packet_id": "security-a",
            "source_status": "partial_timeout",
            "covered_files": ["src/auth.rs"],
            "retry_scope_files": ["src/session.rs", "src/token.rs"]
        }
    });
    let retry_scope =
        ensure_deep_review_retry_coverage(&retry_input, "ReviewSecurity", Some(&manifest))
            .expect("bounded retry coverage");
    assert_eq!(retry_scope, vec!["src/session.rs", "src/token.rs"]);
    assert!(
        prompt_with_deep_review_retry_scope("Review only the retry scope.", &retry_scope)
            .contains("<deep_review_retry_scope>")
    );

    let wait_seconds = provider_capacity_queue_wait_seconds_for_attempt(
        &DeepReviewCapacityQueueDecision::queueable(
            DeepReviewCapacityQueueReason::ProviderConcurrencyLimit,
            None,
        ),
        &DeepReviewConcurrencyPolicy {
            max_parallel_instances: 2,
            stagger_seconds: 0,
            max_queue_wait_seconds: 30,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        },
        2,
    );
    assert_eq!(wait_seconds, Some(270));

    let provider_error_decision =
        capacity_decision_for_provider_error_facts(DeepReviewProviderCapacityErrorFacts {
            provider_code: "provider_specific",
            provider_message: "unmapped upstream response",
            retry_after_seconds: None,
            category: DeepReviewProviderCapacityErrorCategory::RateLimit,
        });
    assert_eq!(
        provider_error_decision.reason,
        Some(DeepReviewCapacityQueueReason::ProviderRateLimit)
    );

    assert_eq!(
        local_reviewer_capacity_queue_decision().reason,
        Some(DeepReviewCapacityQueueReason::LocalConcurrencyCap)
    );

    let provider_step =
        decide_provider_capacity_queue_step(DeepReviewProviderCapacityQueueStepFacts {
            reason: DeepReviewCapacityQueueReason::ProviderConcurrencyLimit,
            queue_expired: false,
            initial_active_reviewer_count: 2,
            active_reviewer_count: 1,
            control_snapshot: DeepReviewQueueControlSnapshot {
                paused: false,
                cancelled: false,
                skip_optional: false,
            },
            is_optional_reviewer: false,
        });
    assert_eq!(
        provider_step,
        DeepReviewProviderCapacityQueueStepDecision::ReadyToRetry {
            early_capacity_probe: true
        }
    );

    let admission_step = decide_blocked_reviewer_admission_queue_step(
        DeepReviewBlockedReviewerAdmissionQueueStepFacts {
            capacity_reason: DeepReviewCapacityQueueReason::LaunchBatchBlocked,
            queue_expired: true,
            active_reviewer_count: 0,
        },
    );
    assert_eq!(
        admission_step,
        DeepReviewBlockedReviewerAdmissionQueueStepDecision::CapacityExpired {
            capacity_reason: DeepReviewCapacityQueueReason::LaunchBatchBlocked
        }
    );

    let (payload, message) = capacity_skip_result_for_local_queue_outcome(
        "ReviewSecurity",
        &DeepReviewConcurrencyPolicy {
            max_parallel_instances: 2,
            stagger_seconds: 0,
            max_queue_wait_seconds: 30,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        },
        DeepReviewCapacityQueueReason::LaunchBatchBlocked,
        DeepReviewQueueWaitSkipReason::QueueExpired,
        30_000,
        42,
        2,
    );
    assert_eq!(payload["status"].as_str(), Some("capacity_skipped"));
    assert_eq!(
        payload["capacity_reason"].as_str(),
        Some("launch_batch_blocked")
    );
    assert!(message.contains("previous launch batch did not finish"));
}
