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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolTaskStateKind {
    Queued,
    Waiting,
    Running,
    Streaming,
    AwaitingConfirmation,
    Completed,
    Failed,
    Cancelled,
}

impl ToolTaskStateKind {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn is_cancellable(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Waiting | Self::Running | Self::AwaitingConfirmation
        )
    }

    pub fn starts_execution_timer(self) -> bool {
        matches!(self, Self::Running | Self::Streaming)
    }

    pub fn completes_execution_timer(self) -> bool {
        self.is_terminal()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToolStateCounts {
    pub total: usize,
    pub queued: usize,
    pub waiting: usize,
    pub running: usize,
    pub streaming: usize,
    pub awaiting_confirmation: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToolTurnCancellationSummary {
    pub cancelled: usize,
    pub skipped: usize,
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

pub fn should_cancel_tool_state(state: ToolTaskStateKind) -> bool {
    state.is_cancellable()
}

pub fn summarize_dialog_turn_cancellation(
    states: impl IntoIterator<Item = ToolTaskStateKind>,
) -> ToolTurnCancellationSummary {
    states.into_iter().fold(
        ToolTurnCancellationSummary::default(),
        |mut summary, state| {
            if should_cancel_tool_state(state) {
                summary.cancelled += 1;
            } else {
                summary.skipped += 1;
            }
            summary
        },
    )
}

pub fn count_tool_states(states: impl IntoIterator<Item = ToolTaskStateKind>) -> ToolStateCounts {
    let mut counts = ToolStateCounts::default();

    for state in states {
        counts.total += 1;
        match state {
            ToolTaskStateKind::Queued => counts.queued += 1,
            ToolTaskStateKind::Waiting => counts.waiting += 1,
            ToolTaskStateKind::Running => counts.running += 1,
            ToolTaskStateKind::Streaming => counts.streaming += 1,
            ToolTaskStateKind::AwaitingConfirmation => counts.awaiting_confirmation += 1,
            ToolTaskStateKind::Completed => counts.completed += 1,
            ToolTaskStateKind::Failed => counts.failed += 1,
            ToolTaskStateKind::Cancelled => counts.cancelled += 1,
        }
    }

    counts
}
