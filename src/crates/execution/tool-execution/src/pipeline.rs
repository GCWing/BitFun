//! Provider-neutral tool pipeline planning helpers.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolBatch {
    pub task_ids: Vec<String>,
    pub is_concurrent: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolExecutionErrorClass {
    Retryable,
    Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolRetryAttemptFacts {
    pub attempts: usize,
    pub max_attempts: usize,
    pub error_class: ToolExecutionErrorClass,
}

/// Partition task IDs into execution batches.
///
/// Consecutive concurrency-safe tasks share one concurrent batch; non-safe
/// tasks stay as individual serial batches. This preserves input ordering while
/// allowing adjacent read-only work to run in parallel.
pub fn partition_tool_batches(task_ids: &[String], flags: &[bool]) -> Vec<ToolBatch> {
    let mut batches: Vec<ToolBatch> = Vec::new();

    for (id, &is_safe) in task_ids.iter().zip(flags.iter()) {
        if is_safe {
            if let Some(last) = batches.last_mut() {
                if last.is_concurrent {
                    last.task_ids.push(id.clone());
                    continue;
                }
            }
        }
        batches.push(ToolBatch {
            task_ids: vec![id.clone()],
            is_concurrent: is_safe,
        });
    }

    batches
}

pub fn should_retry_tool_attempt(facts: ToolRetryAttemptFacts) -> bool {
    facts.attempts < facts.max_attempts
        && matches!(facts.error_class, ToolExecutionErrorClass::Retryable)
}

pub fn retry_delay_ms(attempts: usize) -> u64 {
    100 * attempts as u64
}
