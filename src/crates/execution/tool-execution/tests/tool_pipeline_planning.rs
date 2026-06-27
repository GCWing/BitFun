use tool_runtime::pipeline::{
    count_tool_states, partition_tool_batches, retry_delay_ms, should_cancel_tool_state,
    should_retry_tool_attempt, summarize_dialog_turn_cancellation,
    tool_call_concurrency_safe_for_batch, SubagentBatchExecutionPolicy, ToolCancellationTokenStore,
    ToolExecutionErrorClass, ToolRetryAttemptFacts, ToolTaskStateKind,
};

#[test]
fn partitions_consecutive_concurrency_safe_tools_into_parallel_batches() {
    let task_ids = vec![
        "read_1".to_string(),
        "read_2".to_string(),
        "write_1".to_string(),
        "grep_1".to_string(),
        "grep_2".to_string(),
        "bash_1".to_string(),
    ];
    let flags = vec![true, true, false, true, true, false];

    let batches = partition_tool_batches(&task_ids, &flags);

    assert_eq!(batches.len(), 4);
    assert_eq!(batches[0].task_ids, vec!["read_1", "read_2"]);
    assert!(batches[0].is_concurrent);
    assert_eq!(batches[1].task_ids, vec!["write_1"]);
    assert!(!batches[1].is_concurrent);
    assert_eq!(batches[2].task_ids, vec!["grep_1", "grep_2"]);
    assert!(batches[2].is_concurrent);
    assert_eq!(batches[3].task_ids, vec!["bash_1"]);
    assert!(!batches[3].is_concurrent);
}

#[test]
fn partitions_serial_tools_as_individual_batches() {
    let task_ids = vec!["write_1".to_string(), "bash_1".to_string()];
    let flags = vec![false, false];

    let batches = partition_tool_batches(&task_ids, &flags);

    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0].task_ids, vec!["write_1"]);
    assert!(!batches[0].is_concurrent);
    assert_eq!(batches[1].task_ids, vec!["bash_1"]);
    assert!(!batches[1].is_concurrent);
}

#[test]
fn subagent_batch_policy_preserves_task_concurrency_contract() {
    assert!(!tool_call_concurrency_safe_for_batch(
        "Task",
        false,
        2,
        SubagentBatchExecutionPolicy::SafeOnly,
    ));
    assert!(tool_call_concurrency_safe_for_batch(
        "Task",
        true,
        1,
        SubagentBatchExecutionPolicy::SafeOnly,
    ));
    assert!(tool_call_concurrency_safe_for_batch(
        "Task",
        false,
        2,
        SubagentBatchExecutionPolicy::ForceParallel,
    ));
    assert!(!tool_call_concurrency_safe_for_batch(
        "Task",
        false,
        1,
        SubagentBatchExecutionPolicy::ForceParallel,
    ));
    assert!(!tool_call_concurrency_safe_for_batch(
        "Task",
        true,
        2,
        SubagentBatchExecutionPolicy::Serial,
    ));
    assert!(tool_call_concurrency_safe_for_batch(
        "Read",
        true,
        0,
        SubagentBatchExecutionPolicy::Serial,
    ));
}

#[test]
fn retry_policy_preserves_attempt_limit_and_error_class_contract() {
    assert!(should_retry_tool_attempt(ToolRetryAttemptFacts {
        attempts: 1,
        max_attempts: 3,
        error_class: ToolExecutionErrorClass::Retryable,
    }));
    assert!(!should_retry_tool_attempt(ToolRetryAttemptFacts {
        attempts: 3,
        max_attempts: 3,
        error_class: ToolExecutionErrorClass::Retryable,
    }));
    assert!(!should_retry_tool_attempt(ToolRetryAttemptFacts {
        attempts: 1,
        max_attempts: 3,
        error_class: ToolExecutionErrorClass::Terminal,
    }));
    assert_eq!(retry_delay_ms(2), 200);
}

#[test]
fn cancellation_policy_preserves_cancellable_and_terminal_state_contract() {
    assert!(should_cancel_tool_state(ToolTaskStateKind::Queued));
    assert!(should_cancel_tool_state(ToolTaskStateKind::Waiting));
    assert!(should_cancel_tool_state(ToolTaskStateKind::Running));
    assert!(should_cancel_tool_state(
        ToolTaskStateKind::AwaitingConfirmation
    ));
    assert!(!should_cancel_tool_state(ToolTaskStateKind::Streaming));
    assert!(!should_cancel_tool_state(ToolTaskStateKind::Completed));
    assert!(!should_cancel_tool_state(ToolTaskStateKind::Failed));
    assert!(!should_cancel_tool_state(ToolTaskStateKind::Rejected));
    assert!(!should_cancel_tool_state(ToolTaskStateKind::Cancelled));

    assert!(ToolTaskStateKind::Completed.is_terminal());
    assert!(ToolTaskStateKind::Failed.is_terminal());
    assert!(ToolTaskStateKind::Rejected.is_terminal());
    assert!(ToolTaskStateKind::Cancelled.is_terminal());
    assert!(!ToolTaskStateKind::Running.is_terminal());
}

#[test]
fn dialog_turn_cancellation_summary_counts_cancelled_and_skipped_tasks() {
    let summary = summarize_dialog_turn_cancellation([
        ToolTaskStateKind::Queued,
        ToolTaskStateKind::Running,
        ToolTaskStateKind::Completed,
        ToolTaskStateKind::Rejected,
        ToolTaskStateKind::Cancelled,
    ]);

    assert_eq!(summary.cancelled, 2);
    assert_eq!(summary.skipped, 3);
}

#[test]
fn cancellation_token_store_cancels_and_removes_tokens() {
    let store = ToolCancellationTokenStore::new();
    let token = tokio_util::sync::CancellationToken::new();

    store.insert("tool-1".to_string(), token.clone());

    assert!(store.has_pending("tool-1"));
    assert!(store.cancel("tool-1"));
    assert!(token.is_cancelled());
    assert!(!store.has_pending("tool-1"));
    assert!(!store.cancel("tool-1"));
}

#[test]
fn state_counts_preserve_pipeline_stats_contract() {
    let counts = count_tool_states([
        ToolTaskStateKind::Queued,
        ToolTaskStateKind::Queued,
        ToolTaskStateKind::Waiting,
        ToolTaskStateKind::Running,
        ToolTaskStateKind::Streaming,
        ToolTaskStateKind::AwaitingConfirmation,
        ToolTaskStateKind::Completed,
        ToolTaskStateKind::Failed,
        ToolTaskStateKind::Rejected,
        ToolTaskStateKind::Cancelled,
    ]);

    assert_eq!(counts.total, 10);
    assert_eq!(counts.queued, 2);
    assert_eq!(counts.waiting, 1);
    assert_eq!(counts.running, 1);
    assert_eq!(counts.streaming, 1);
    assert_eq!(counts.awaiting_confirmation, 1);
    assert_eq!(counts.completed, 1);
    assert_eq!(counts.failed, 1);
    assert_eq!(counts.rejected, 1);
    assert_eq!(counts.cancelled, 1);
}
