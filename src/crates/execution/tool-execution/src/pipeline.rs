//! Provider-neutral tool pipeline planning helpers.

use bitfun_events::ToolEventData;
use dashmap::DashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolBatch {
    pub task_ids: Vec<String>,
    pub is_concurrent: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubagentBatchExecutionPolicy {
    SafeOnly,
    #[default]
    ForceParallel,
    Serial,
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
    Rejected,
    Cancelled,
}

impl ToolTaskStateKind {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Rejected | Self::Cancelled
        )
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
    pub rejected: usize,
    pub cancelled: usize,
}

#[derive(Debug, Clone)]
pub struct ToolStateEventFacts {
    pub tool_id: String,
    pub tool_name: String,
    pub state: ToolStateEventKind,
}

#[derive(Debug, Clone)]
pub enum ToolStateEventKind {
    Queued {
        position: usize,
    },
    Waiting {
        dependencies: Vec<String>,
    },
    Running {
        params: serde_json::Value,
        timeout_seconds: Option<u64>,
    },
    Streaming {
        chunks_received: usize,
    },
    AwaitingConfirmation {
        params: serde_json::Value,
        timeout_at: Option<u64>,
    },
    Completed {
        result: serde_json::Value,
        result_for_assistant: Option<String>,
        duration_ms: u64,
        queue_wait_ms: Option<u64>,
        preflight_ms: Option<u64>,
        confirmation_wait_ms: Option<u64>,
        execution_ms: Option<u64>,
    },
    Failed {
        error: String,
        duration_ms: Option<u64>,
        queue_wait_ms: Option<u64>,
        preflight_ms: Option<u64>,
        confirmation_wait_ms: Option<u64>,
        execution_ms: Option<u64>,
    },
    Rejected,
    Cancelled {
        reason: String,
        duration_ms: Option<u64>,
        queue_wait_ms: Option<u64>,
        preflight_ms: Option<u64>,
        confirmation_wait_ms: Option<u64>,
        execution_ms: Option<u64>,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToolTurnCancellationSummary {
    pub cancelled: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ToolCancellationTokenStore {
    tokens: Arc<DashMap<String, CancellationToken>>,
}

impl ToolCancellationTokenStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, tool_id: String, token: CancellationToken) {
        self.tokens.insert(tool_id, token);
    }

    pub fn remove(&self, tool_id: &str) -> bool {
        self.tokens.remove(tool_id).is_some()
    }

    pub fn cancel(&self, tool_id: &str) -> bool {
        let Some((_, token)) = self.tokens.remove(tool_id) else {
            return false;
        };
        token.cancel();
        true
    }

    pub fn has_pending(&self, tool_id: &str) -> bool {
        self.tokens.contains_key(tool_id)
    }
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

pub fn tool_call_concurrency_safe_for_batch(
    tool_name: &str,
    tool_is_concurrency_safe: bool,
    same_batch_subagent_call_count: usize,
    subagent_batch_execution_policy: SubagentBatchExecutionPolicy,
) -> bool {
    if tool_name != "Task" {
        return tool_is_concurrency_safe;
    }

    match subagent_batch_execution_policy {
        SubagentBatchExecutionPolicy::SafeOnly => tool_is_concurrency_safe,
        SubagentBatchExecutionPolicy::ForceParallel => {
            same_batch_subagent_call_count > 1 || tool_is_concurrency_safe
        }
        SubagentBatchExecutionPolicy::Serial => false,
    }
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
            ToolTaskStateKind::Rejected => counts.rejected += 1,
            ToolTaskStateKind::Cancelled => counts.cancelled += 1,
        }
    }

    counts
}

pub fn sanitize_tool_result_for_event(result: &serde_json::Value) -> serde_json::Value {
    let mut sanitized = result.clone();
    redact_data_url_in_json(&mut sanitized);
    sanitized
}

pub fn tool_state_event_data(facts: ToolStateEventFacts) -> ToolEventData {
    let ToolStateEventFacts {
        tool_id,
        tool_name,
        state,
    } = facts;

    match state {
        ToolStateEventKind::Queued { position } => ToolEventData::Queued {
            tool_id,
            tool_name,
            position,
        },
        ToolStateEventKind::Waiting { dependencies } => ToolEventData::Waiting {
            tool_id,
            tool_name,
            dependencies,
        },
        ToolStateEventKind::Running {
            params,
            timeout_seconds,
        } => ToolEventData::Started {
            tool_id,
            tool_name,
            params,
            timeout_seconds,
        },
        ToolStateEventKind::Streaming { chunks_received } => ToolEventData::Streaming {
            tool_id,
            tool_name,
            chunks_received,
        },
        ToolStateEventKind::AwaitingConfirmation { params, timeout_at } => {
            ToolEventData::ConfirmationNeeded {
                tool_id,
                tool_name,
                params,
                timeout_at,
            }
        }
        ToolStateEventKind::Completed {
            result,
            result_for_assistant,
            duration_ms,
            queue_wait_ms,
            preflight_ms,
            confirmation_wait_ms,
            execution_ms,
        } => ToolEventData::Completed {
            tool_id,
            tool_name,
            result: sanitize_tool_result_for_event(&result),
            result_for_assistant,
            duration_ms,
            queue_wait_ms,
            preflight_ms,
            confirmation_wait_ms,
            execution_ms,
        },
        ToolStateEventKind::Failed {
            error,
            duration_ms,
            queue_wait_ms,
            preflight_ms,
            confirmation_wait_ms,
            execution_ms,
        } => ToolEventData::Failed {
            tool_id,
            tool_name,
            error,
            duration_ms,
            queue_wait_ms,
            preflight_ms,
            confirmation_wait_ms,
            execution_ms,
        },
        ToolStateEventKind::Rejected => ToolEventData::Rejected { tool_id, tool_name },
        ToolStateEventKind::Cancelled {
            reason,
            duration_ms,
            queue_wait_ms,
            preflight_ms,
            confirmation_wait_ms,
            execution_ms,
        } => ToolEventData::Cancelled {
            tool_id,
            tool_name,
            reason,
            duration_ms,
            queue_wait_ms,
            preflight_ms,
            confirmation_wait_ms,
            execution_ms,
        },
    }
}

fn redact_data_url_in_json(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            let had_data_url = map.remove("data_url").is_some();
            if had_data_url {
                map.insert("has_data_url".to_string(), serde_json::json!(true));
            }
            for child in map.values_mut() {
                redact_data_url_in_json(child);
            }
        }
        serde_json::Value::Array(arr) => {
            for child in arr {
                redact_data_url_in_json(child);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::ToolStateEventKind;
    use super::{sanitize_tool_result_for_event, tool_state_event_data, ToolStateEventFacts};
    use bitfun_events::ToolEventData;
    use serde_json::json;

    #[test]
    fn completed_event_redacts_data_urls_recursively() {
        let data = tool_state_event_data(ToolStateEventFacts {
            tool_id: "tool-1".to_string(),
            tool_name: "Screenshot".to_string(),
            state: ToolStateEventKind::Completed {
                result: json!({
                    "data_url": "data:image/png;base64,AAAA",
                    "nested": [{ "data_url": "data:image/png;base64,BBBB" }]
                }),
                result_for_assistant: Some("done".to_string()),
                duration_ms: 10,
                queue_wait_ms: Some(1),
                preflight_ms: None,
                confirmation_wait_ms: None,
                execution_ms: Some(9),
            },
        });

        let ToolEventData::Completed { result, .. } = data else {
            panic!("expected completed event");
        };
        assert_eq!(result["has_data_url"], true);
        assert!(result.get("data_url").is_none());
        assert_eq!(result["nested"][0]["has_data_url"], true);
        assert!(result["nested"][0].get("data_url").is_none());
    }

    #[test]
    fn rejected_state_maps_to_rejected_event() {
        let data = tool_state_event_data(ToolStateEventFacts {
            tool_id: "tool-1".to_string(),
            tool_name: "ExecCommand".to_string(),
            state: ToolStateEventKind::Rejected,
        });

        let ToolEventData::Rejected { tool_id, tool_name } = data else {
            panic!("expected rejected event");
        };

        assert_eq!(tool_id, "tool-1");
        assert_eq!(tool_name, "ExecCommand");
    }

    #[test]
    fn sanitize_keeps_values_without_data_urls() {
        let result = sanitize_tool_result_for_event(&json!({ "text": "ok" }));

        assert_eq!(result, json!({ "text": "ok" }));
    }
}
