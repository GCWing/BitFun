use bitfun_agent_runtime::thread_goal_tools::{
    build_goal_tool_result, parse_create_goal_args, parse_update_goal_status,
};
use bitfun_runtime_ports::{ThreadGoal, ThreadGoalStatus};

fn goal(status: ThreadGoalStatus) -> ThreadGoal {
    ThreadGoal {
        goal_id: "goal-1".to_string(),
        session_id: "session-1".to_string(),
        objective: "finish migration".to_string(),
        status,
        token_budget: Some(100),
        tokens_used: 80,
        time_used_seconds: 3,
        created_at: 1,
        updated_at: 2,
        auto_continuation_count: 0,
    }
}

#[test]
fn update_goal_status_parser_preserves_legacy_values_and_errors() {
    assert_eq!(
        parse_update_goal_status(" complete ").expect("complete should parse"),
        ThreadGoalStatus::Complete
    );
    assert_eq!(
        parse_update_goal_status("BLOCKED").expect("blocked should parse"),
        ThreadGoalStatus::Blocked
    );

    assert_eq!(
        parse_update_goal_status("paused")
            .expect_err("unsupported status should fail")
            .to_string(),
        "update_goal status must be complete or blocked, got paused"
    );
}

#[test]
fn create_goal_args_parser_preserves_snake_case_wire_contract() {
    let args = parse_create_goal_args(serde_json::json!({
        "objective": "ship PR-C",
        "token_budget": 4096
    }))
    .expect("valid create_goal args should parse");

    assert_eq!(args.objective, "ship PR-C");
    assert_eq!(args.token_budget, Some(4096));

    assert!(
        parse_create_goal_args(serde_json::json!({ "token_budget": 4096 }))
            .expect_err("missing objective should fail")
            .to_string()
            .starts_with("invalid create_goal args: missing field `objective`")
    );
}

#[test]
fn goal_tool_result_preserves_empty_and_complete_wire_shape() {
    let empty = build_goal_tool_result(None, false).expect("empty goal should serialize");
    assert_eq!(empty.data, serde_json::json!({}));
    assert_eq!(empty.result_for_assistant, "No thread goal is set.");

    let complete =
        build_goal_tool_result(Some(goal(ThreadGoalStatus::Complete)), true).expect("serializes");
    assert_eq!(complete.data["goal"]["status"], "complete");
    assert_eq!(complete.data["remainingTokens"], 20);
    assert!(complete.data["completionBudgetReport"]
        .as_str()
        .expect("completion report should be string")
        .contains("Goal achieved"));
    assert_eq!(
        complete.result_for_assistant,
        "Thread goal status: complete"
    );
}
