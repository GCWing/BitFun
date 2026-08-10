//! Provider-neutral formatting for completed subagent Task calls.

use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubagentTaskCompletionResultInput<'a> {
    pub delegate_target_label: &'a str,
    pub result_text: &'a str,
    pub context_mode: &'a str,
    pub duration_ms: u128,
    pub is_partial_timeout: bool,
    pub reason: Option<&'a str>,
    pub ledger_event_id: Option<&'a str>,
    pub partial_timeout_suffix: &'a str,
}

pub fn subagent_task_completion_result(
    input: SubagentTaskCompletionResultInput<'_>,
) -> (Value, String) {
    let status = if input.is_partial_timeout {
        "partial_timeout"
    } else {
        "completed"
    };
    let assistant_message = if input.is_partial_timeout {
        format!(
            "{} timed out with partial result:\n<partial_result status=\"partial_timeout\">\n{}\n</partial_result>{}",
            input.delegate_target_label, input.result_text, input.partial_timeout_suffix
        )
    } else {
        format!(
            "{} completed successfully with result:\n<result>\n{}\n</result>",
            input.delegate_target_label, input.result_text
        )
    };
    let mut data = json!({
        "duration": input.duration_ms,
        "context_mode": input.context_mode,
        "status": status
    });

    if input.is_partial_timeout {
        data["partial_output"] = json!(input.result_text);
        if let Some(reason) = input.reason {
            data["reason"] = json!(reason);
        }
        if let Some(event_id) = input.ledger_event_id {
            data["ledger_event_id"] = json!(event_id);
        }
    }

    (data, assistant_message)
}
