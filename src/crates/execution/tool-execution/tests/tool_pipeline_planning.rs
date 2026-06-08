use tool_runtime::pipeline::{
    partition_tool_batches, retry_delay_ms, should_retry_tool_attempt, ToolExecutionErrorClass,
    ToolRetryAttemptFacts,
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
