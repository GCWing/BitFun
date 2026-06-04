use bitfun_agent_runtime::thread_goal::{
    billable_tokens_from_counts, build_objective_updated_plan, build_set_thread_goal_result,
    build_thread_goal_continuation_plan, clear_thread_goal_patch,
    goal_continuation_submit_retry_delay_ms, goal_tool_response, is_usage_limit_message,
    should_record_thread_goal_token_usage, should_skip_goal_continuation_after_turn,
    should_skip_goal_for_turn, thread_goal_event_payload, thread_goal_from_custom_metadata,
    thread_goal_patch, thread_goal_status_is_resumable, SetThreadGoalRequest,
    ThreadGoalContinuationFacts, ThreadGoalRuntime, ThreadGoalTokenUsageFacts,
};
use bitfun_runtime_ports::{
    ThreadGoal, ThreadGoalStatus, MAX_GOAL_CONTINUATIONS, MAX_THREAD_GOAL_AUTO_CONTINUATIONS,
    THREAD_GOAL_METADATA_KEY,
};

fn goal(status: ThreadGoalStatus) -> ThreadGoal {
    ThreadGoal {
        goal_id: "g1".to_string(),
        session_id: "s1".to_string(),
        objective: "ship feature".to_string(),
        status,
        token_budget: None,
        tokens_used: 0,
        time_used_seconds: 0,
        created_at: 1,
        updated_at: 2,
        auto_continuation_count: 0,
    }
}

#[test]
fn resumable_statuses_match_ui_actions() {
    assert!(thread_goal_status_is_resumable(ThreadGoalStatus::Paused));
    assert!(thread_goal_status_is_resumable(ThreadGoalStatus::Blocked));
    assert!(thread_goal_status_is_resumable(
        ThreadGoalStatus::UsageLimited
    ));
    assert!(!thread_goal_status_is_resumable(ThreadGoalStatus::Active));
    assert!(!thread_goal_status_is_resumable(
        ThreadGoalStatus::BudgetLimited
    ));
    assert!(!thread_goal_status_is_resumable(ThreadGoalStatus::Complete));
}

#[test]
fn set_thread_goal_creates_new_active_goal_with_trimmed_objective() {
    let result = build_set_thread_goal_result(SetThreadGoalRequest {
        session_id: "s1".to_string(),
        existing: None,
        objective: Some("  finish migration  ".to_string()),
        status: Some(ThreadGoalStatus::Active),
        token_budget: Some(Some(5000)),
        replace_existing: false,
        now_epoch_seconds: 10,
        new_goal_id: "goal-new".to_string(),
    })
    .expect("goal should be created");

    assert!(!result.replaced_existing);
    assert_eq!(result.goal.goal_id, "goal-new");
    assert_eq!(result.goal.objective, "finish migration");
    assert_eq!(result.goal.token_budget, Some(5000));
    assert_eq!(result.goal.created_at, 10);
    assert_eq!(result.goal.updated_at, 10);
}

#[test]
fn set_thread_goal_updates_existing_objective_and_resets_continuation_count() {
    let mut existing = goal(ThreadGoalStatus::BudgetLimited);
    existing.objective = "old".to_string();
    existing.auto_continuation_count = 42;

    let result = build_set_thread_goal_result(SetThreadGoalRequest {
        session_id: "s1".to_string(),
        existing: Some(existing),
        objective: Some("new".to_string()),
        status: Some(ThreadGoalStatus::Active),
        token_budget: None,
        replace_existing: false,
        now_epoch_seconds: 11,
        new_goal_id: "unused".to_string(),
    })
    .expect("goal should be updated");

    assert!(result.replaced_existing);
    assert_eq!(result.goal.objective, "new");
    assert_eq!(result.goal.status, ThreadGoalStatus::Active);
    assert_eq!(result.goal.auto_continuation_count, 0);
    assert_eq!(result.goal.updated_at, 11);
}

#[test]
fn set_thread_goal_replaces_existing_goal_when_requested() {
    let mut existing = goal(ThreadGoalStatus::Active);
    existing.goal_id = "goal-old".to_string();
    existing.tokens_used = 50;

    let result = build_set_thread_goal_result(SetThreadGoalRequest {
        session_id: "s1".to_string(),
        existing: Some(existing),
        objective: Some("new objective".to_string()),
        status: Some(ThreadGoalStatus::Active),
        token_budget: Some(Some(1000)),
        replace_existing: true,
        now_epoch_seconds: 12,
        new_goal_id: "goal-new".to_string(),
    })
    .expect("replace should create a new goal");

    assert!(result.replaced_existing);
    assert_eq!(result.goal.goal_id, "goal-new");
    assert_eq!(result.goal.objective, "new objective");
    assert_eq!(result.goal.tokens_used, 0);
    assert_eq!(result.goal.token_budget, Some(1000));
    assert_eq!(result.goal.created_at, 12);
}

#[test]
fn set_thread_goal_rejects_invalid_budget_and_missing_update_target() {
    let invalid_budget = build_set_thread_goal_result(SetThreadGoalRequest {
        session_id: "s1".to_string(),
        existing: None,
        objective: Some("goal".to_string()),
        status: Some(ThreadGoalStatus::Active),
        token_budget: Some(Some(0)),
        replace_existing: false,
        now_epoch_seconds: 1,
        new_goal_id: "g1".to_string(),
    });
    assert!(invalid_budget
        .expect_err("zero budget should fail")
        .to_string()
        .contains("goal budgets must be positive"));

    let missing_goal = build_set_thread_goal_result(SetThreadGoalRequest {
        session_id: "s1".to_string(),
        existing: None,
        objective: None,
        status: Some(ThreadGoalStatus::Complete),
        token_budget: None,
        replace_existing: false,
        now_epoch_seconds: 1,
        new_goal_id: "g1".to_string(),
    });
    assert!(missing_goal
        .expect_err("status update requires existing goal")
        .to_string()
        .contains("no goal exists"));
}

#[test]
fn continuation_outcome_increments_active_goal_and_builds_plan() {
    let runtime = ThreadGoalRuntime::new();
    let goal = goal(ThreadGoalStatus::Active);
    runtime.mark_turn_started("turn-1", Some(&goal));

    let outcome = runtime.continuation_after_turn(
        goal,
        ThreadGoalContinuationFacts {
            turn_id: "turn-1",
            turn_tokens: 0,
            turn_completed: true,
            now_epoch_seconds: 20,
        },
    );

    let persisted = outcome
        .goal_to_persist
        .as_ref()
        .expect("active goal should persist increment");
    assert!(outcome.scheduled_auto_continuation);
    assert_eq!(persisted.auto_continuation_count, 1);
    assert_eq!(persisted.updated_at, 20);
    assert!(outcome
        .plan
        .as_ref()
        .expect("active goal should schedule continuation")
        .display_message
        .contains("1/100"));
}

#[test]
fn continuation_outcome_marks_active_goal_blocked_at_limit() {
    let runtime = ThreadGoalRuntime::new();
    let mut goal = goal(ThreadGoalStatus::Active);
    goal.auto_continuation_count = MAX_THREAD_GOAL_AUTO_CONTINUATIONS;

    let outcome = runtime.continuation_after_turn(
        goal,
        ThreadGoalContinuationFacts {
            turn_id: "turn-1",
            turn_tokens: 0,
            turn_completed: true,
            now_epoch_seconds: 30,
        },
    );

    assert!(outcome.reached_auto_continuation_limit);
    assert_eq!(
        outcome
            .goal_to_persist
            .expect("blocked goal should persist")
            .status,
        ThreadGoalStatus::Blocked
    );
    assert!(outcome.plan.is_none());
}

#[test]
fn continuation_outcome_reports_budget_limit_once_when_tokens_cross_budget() {
    let runtime = ThreadGoalRuntime::new();
    let mut goal = goal(ThreadGoalStatus::Active);
    goal.token_budget = Some(10);
    runtime.mark_turn_started("turn-1", Some(&goal));
    runtime.record_round_billable_tokens("turn-1", 12);

    let outcome = runtime.continuation_after_turn(
        goal,
        ThreadGoalContinuationFacts {
            turn_id: "turn-1",
            turn_tokens: 12,
            turn_completed: true,
            now_epoch_seconds: 40,
        },
    );

    let persisted = outcome
        .goal_to_persist
        .expect("budget-limited goal should persist");
    assert_eq!(persisted.status, ThreadGoalStatus::BudgetLimited);
    assert_eq!(persisted.tokens_used, 12);
    assert!(outcome
        .plan
        .expect("first budget-limit transition should produce a plan")
        .prepended_reminders[0]
        .contains("budget_limited"));
}

#[test]
fn prompt_and_tool_response_contracts_match_thread_goal_wire_shape() {
    let mut complete = goal(ThreadGoalStatus::Complete);
    complete.token_budget = Some(100);
    complete.tokens_used = 80;
    complete.time_used_seconds = 3;

    let response = goal_tool_response(Some(complete), true);
    assert_eq!(response.remaining_tokens, Some(20));
    assert!(response.completion_budget_report.is_some());

    let updated = build_objective_updated_plan(&goal(ThreadGoalStatus::Active));
    assert_eq!(
        updated.user_message_metadata["threadGoalObjectiveUpdated"],
        true
    );

    let plan = build_thread_goal_continuation_plan(&goal(ThreadGoalStatus::Active));
    assert_eq!(plan.user_message_metadata["autoContinuationMax"], 100);
}

#[test]
fn thread_goal_metadata_patch_and_legacy_goal_mode_migration_keep_wire_shape() {
    let active = goal(ThreadGoalStatus::Active);
    let patch = thread_goal_patch(&active);
    assert_eq!(patch[THREAD_GOAL_METADATA_KEY]["goalId"], "g1");
    assert_eq!(patch[THREAD_GOAL_METADATA_KEY]["status"], "active");
    assert_eq!(
        clear_thread_goal_patch()[THREAD_GOAL_METADATA_KEY],
        serde_json::Value::Null
    );

    let restored = thread_goal_from_custom_metadata(Some(&patch), "legacy-id".to_string(), 99)
        .expect("thread_goal metadata should restore");
    assert_eq!(restored, active);

    let legacy = serde_json::json!({
        "goal_mode": {
            "active": true,
            "sessionId": "session-legacy",
            "goalText": "  migrate metadata  ",
            "activatedAtMs": 1234
        }
    });
    let migrated = thread_goal_from_custom_metadata(Some(&legacy), "legacy-id".to_string(), 99)
        .expect("legacy goal mode should migrate");
    assert_eq!(migrated.goal_id, "legacy-id");
    assert_eq!(migrated.session_id, "session-legacy");
    assert_eq!(migrated.objective, "migrate metadata");
    assert_eq!(migrated.status, ThreadGoalStatus::Active);
    assert_eq!(migrated.created_at, 1234);
    assert_eq!(migrated.updated_at, 1234);
}

#[test]
fn thread_goal_event_payload_and_token_usage_filter_preserve_core_delivery_contract() {
    let active = goal(ThreadGoalStatus::Active);
    let payload = thread_goal_event_payload(Some(active.clone()))
        .expect("active goal should serialize for event");
    assert_eq!(payload["goalId"], active.goal_id);
    assert_eq!(payload["status"], "active");
    assert_eq!(thread_goal_event_payload(None), None);

    assert_eq!(
        should_record_thread_goal_token_usage(ThreadGoalTokenUsageFacts {
            input_tokens: 100,
            output_tokens: Some(30),
            cached_tokens: Some(40),
            is_subagent: false,
        }),
        Some(90)
    );
    assert_eq!(
        should_record_thread_goal_token_usage(ThreadGoalTokenUsageFacts {
            input_tokens: 100,
            output_tokens: Some(30),
            cached_tokens: Some(40),
            is_subagent: true,
        }),
        None
    );
    assert_eq!(
        should_record_thread_goal_token_usage(ThreadGoalTokenUsageFacts {
            input_tokens: 0,
            output_tokens: None,
            cached_tokens: None,
            is_subagent: false,
        }),
        None
    );
}

#[test]
fn turn_filtering_and_retry_policies_preserve_goal_mode_semantics() {
    assert!(should_skip_goal_for_turn("/goal fix bug", None));
    assert!(!should_skip_goal_for_turn("fix bug", None));

    let metadata = serde_json::json!({ "threadGoalObjectiveUpdated": true });
    assert!(should_skip_goal_for_turn("Adjust work", Some(&metadata)));
    assert!(!should_skip_goal_continuation_after_turn(
        "Adjust work",
        Some(&metadata)
    ));

    assert_eq!(goal_continuation_submit_retry_delay_ms(0), 0);
    assert_eq!(goal_continuation_submit_retry_delay_ms(1), 1_000);
    assert_eq!(goal_continuation_submit_retry_delay_ms(2), 2_000);
    assert_eq!(goal_continuation_submit_retry_delay_ms(100), 30_000);
    assert_eq!(billable_tokens_from_counts(1000, 400, 200), 800);
    assert!(is_usage_limit_message(
        "insufficient_quota: billing hard limit"
    ));
    assert!(!is_usage_limit_message("tool failed"));
    assert_eq!(MAX_GOAL_CONTINUATIONS, 100);
}
