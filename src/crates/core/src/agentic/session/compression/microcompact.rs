//! Microcompact: lightweight pre-compression that clears old tool results.
//!
//! Before the heavier full-context compression kicks in, microcompact replaces
//! the content of old, compactable tool results with a short placeholder.  This
//! frees significant tokens (tool output is often the largest part of context)
//! while preserving the tool call structure so the model still knows *what* was
//! called and *that* it produced output.
//!
//! Design reference: Claude Code `microCompact.ts` (time-based clearing path).

use crate::agentic::core::{Message, MessageContent};
use crate::agentic::session::{
    EvidenceLedgerEvent, EvidenceLedgerEventStatus, EvidenceLedgerTargetKind,
};
use log::{debug, info};
use std::collections::HashSet;

const CLEARED_PLACEHOLDER: &str = "[Old tool result content cleared]";

/// Tools whose results can be safely cleared after they become stale.
/// These are read/search/write tools whose output is transient context.
fn default_compactable_tools() -> HashSet<&'static str> {
    [
        "Read",
        "Bash",
        "Grep",
        "Glob",
        "WebSearch",
        "WebFetch",
        "Edit",
        "Write",
        "LS",
        "Delete",
        "Git",
        "GetFileDiff",
    ]
    .into_iter()
    .collect()
}

/// Configuration for microcompact behaviour.
pub struct MicrocompactConfig {
    /// Number of most-recent compactable tool results to keep intact.
    pub keep_recent: usize,
    /// Minimum token-usage ratio before microcompact activates.
    pub trigger_ratio: f32,
}

impl Default for MicrocompactConfig {
    fn default() -> Self {
        Self {
            keep_recent: 8,
            trigger_ratio: 0.5,
        }
    }
}

/// Statistics returned after a microcompact pass.
#[derive(Debug, Clone)]
pub struct MicrocompactResult {
    pub tools_cleared: usize,
    pub tools_kept: usize,
    pub evidence_events: Vec<EvidenceLedgerEvent>,
    pub evidence_events_preserved: usize,
}

/// Session/turn scope used when preserving facts for cleared tool results.
#[derive(Debug, Clone, Copy)]
pub struct MicrocompactEvidenceScope<'a> {
    pub session_id: &'a str,
    pub turn_id: &'a str,
}

/// Run microcompact on the message list **in place**.
///
/// Returns `None` if no clearing was performed (e.g. not enough compactable
/// results, or all are within the keep window).
pub fn microcompact_messages(
    messages: &mut [Message],
    config: &MicrocompactConfig,
) -> Option<MicrocompactResult> {
    microcompact_messages_internal(messages, config, None)
}

/// Run microcompact and preserve a ledger event for each cleared tool result.
pub fn microcompact_messages_with_evidence(
    messages: &mut [Message],
    config: &MicrocompactConfig,
    evidence_scope: MicrocompactEvidenceScope<'_>,
) -> Option<MicrocompactResult> {
    microcompact_messages_internal(messages, config, Some(evidence_scope))
}

fn microcompact_messages_internal(
    messages: &mut [Message],
    config: &MicrocompactConfig,
    evidence_scope: Option<MicrocompactEvidenceScope<'_>>,
) -> Option<MicrocompactResult> {
    let compactable = default_compactable_tools();

    // Collect indices of compactable tool-result messages (in encounter order).
    let compactable_indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter_map(|(idx, msg)| {
            if let MessageContent::ToolResult { ref tool_name, .. } = msg.content {
                if compactable.contains(tool_name.as_str()) {
                    return Some(idx);
                }
            }
            None
        })
        .collect();

    if compactable_indices.len() <= config.keep_recent {
        return None;
    }

    // Keep the last `keep_recent` intact; clear everything before that.
    let keep_start = compactable_indices.len() - config.keep_recent;
    let to_clear = &compactable_indices[..keep_start];

    if to_clear.is_empty() {
        return None;
    }

    let mut cleared = 0usize;
    let mut evidence_events = Vec::new();
    for &idx in to_clear {
        let already_cleared = matches!(
            &messages[idx].content,
            MessageContent::ToolResult {
                result_for_assistant,
                ..
            } if result_for_assistant.as_deref() == Some(CLEARED_PLACEHOLDER)
        );
        if already_cleared {
            continue;
        }

        if let Some(scope) = evidence_scope {
            if let Some(event) = build_evidence_event_for_tool_result(&messages[idx], scope) {
                evidence_events.push(event);
            }
        }

        let msg = &mut messages[idx];
        if let MessageContent::ToolResult {
            ref mut result,
            ref mut result_for_assistant,
            ref mut image_attachments,
            ..
        } = msg.content
        {
            *result = serde_json::json!(CLEARED_PLACEHOLDER);
            *result_for_assistant = Some(CLEARED_PLACEHOLDER.to_string());
            *image_attachments = None;
            // Invalidate cached token count so it gets re-estimated.
            msg.metadata.tokens = None;
            cleared += 1;
        }
    }

    if cleared == 0 {
        return None;
    }

    let kept = compactable_indices.len() - cleared;
    let evidence_events_preserved = evidence_events.len();
    info!(
        "Microcompact: cleared {} tool result(s), kept {} recent, preserved {} evidence event(s)",
        cleared, kept, evidence_events_preserved
    );
    debug!(
        "Microcompact details: total_compactable={}, keep_recent={}, cleared={}, evidence_events={}",
        compactable_indices.len(),
        config.keep_recent,
        cleared,
        evidence_events_preserved
    );

    Some(MicrocompactResult {
        tools_cleared: cleared,
        tools_kept: kept,
        evidence_events,
        evidence_events_preserved,
    })
}

fn build_evidence_event_for_tool_result(
    message: &Message,
    scope: MicrocompactEvidenceScope<'_>,
) -> Option<EvidenceLedgerEvent> {
    let MessageContent::ToolResult {
        tool_name,
        result,
        is_error,
        ..
    } = &message.content
    else {
        return None;
    };

    let turn_id = message.metadata.turn_id.as_deref().unwrap_or(scope.turn_id);
    let target_kind = infer_target_kind(tool_name);
    let target = infer_target(tool_name, result);
    let status = infer_event_status(result, *is_error);
    let mut event = EvidenceLedgerEvent::new(
        scope.session_id,
        turn_id,
        tool_name,
        target_kind,
        target,
        status,
        format!(
            "Preserved {} tool result before microcompact clearing.",
            tool_name
        ),
    );

    if let Some(error_kind) = infer_error_kind(result, *is_error) {
        event = event.with_error_kind(error_kind);
    }

    let touched_files = infer_touched_files(tool_name, result);
    if !touched_files.is_empty() {
        event = event.with_touched_files(touched_files);
    }

    if let Some(artifact_path) = infer_artifact_path(result) {
        event = event.with_artifact_path(artifact_path);
    }

    Some(event)
}

fn infer_target_kind(tool_name: &str) -> EvidenceLedgerTargetKind {
    match tool_name {
        "Bash" | "Git" => EvidenceLedgerTargetKind::Command,
        "Read" | "Grep" | "Glob" | "LS" | "Edit" | "Write" | "Delete" | "GetFileDiff" => {
            EvidenceLedgerTargetKind::File
        }
        _ => EvidenceLedgerTargetKind::Unknown,
    }
}

fn infer_target(tool_name: &str, result: &serde_json::Value) -> String {
    match tool_name {
        "Bash" | "Git" => string_field(result, "command")
            .or_else(|| {
                let operation = string_field(result, "operation")?;
                Some(format!("git {}", operation))
            })
            .unwrap_or_else(|| tool_name.to_string()),
        "Read" | "Edit" | "Write" | "Delete" | "GetFileDiff" => string_field(result, "file_path")
            .or_else(|| string_field(result, "path"))
            .unwrap_or_else(|| tool_name.to_string()),
        "Grep" => string_field(result, "pattern")
            .or_else(|| string_field(result, "path"))
            .unwrap_or_else(|| tool_name.to_string()),
        "Glob" => string_field(result, "pattern")
            .or_else(|| string_field(result, "path"))
            .unwrap_or_else(|| tool_name.to_string()),
        "LS" => string_field(result, "path")
            .or_else(|| string_field(result, "directory"))
            .unwrap_or_else(|| tool_name.to_string()),
        _ => string_field(result, "target").unwrap_or_else(|| tool_name.to_string()),
    }
}

fn infer_event_status(result: &serde_json::Value, is_error: bool) -> EvidenceLedgerEventStatus {
    if is_error
        || bool_field(result, "timed_out") == Some(true)
        || bool_field(result, "interrupted") == Some(true)
        || bool_field(result, "success") == Some(false)
        || numeric_field(result, "exit_code").is_some_and(|code| code != 0)
    {
        EvidenceLedgerEventStatus::Failed
    } else {
        EvidenceLedgerEventStatus::Succeeded
    }
}

fn infer_error_kind(result: &serde_json::Value, is_error: bool) -> Option<String> {
    if bool_field(result, "timed_out") == Some(true) {
        return Some("timeout".to_string());
    }
    if bool_field(result, "interrupted") == Some(true) {
        return Some("interrupted".to_string());
    }
    if let Some(exit_code) = numeric_field(result, "exit_code") {
        if exit_code != 0 {
            return Some(format!("exit_code:{}", exit_code));
        }
    }
    if is_error || result.get("error").is_some() || bool_field(result, "success") == Some(false) {
        return Some("tool_error".to_string());
    }
    None
}

fn infer_touched_files(tool_name: &str, result: &serde_json::Value) -> Vec<String> {
    match tool_name {
        "Edit" | "Write" | "Delete" => string_field(result, "file_path")
            .or_else(|| string_field(result, "path"))
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

fn infer_artifact_path(result: &serde_json::Value) -> Option<String> {
    string_field(result, "artifact_path")
        .or_else(|| string_field(result, "output_file"))
        .or_else(|| string_field(result, "transcript_path"))
}

fn string_field(result: &serde_json::Value, key: &str) -> Option<String> {
    result
        .get(key)
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn bool_field(result: &serde_json::Value, key: &str) -> Option<bool> {
    result.get(key).and_then(|value| value.as_bool())
}

fn numeric_field(result: &serde_json::Value, key: &str) -> Option<i64> {
    result.get(key).and_then(|value| value.as_i64())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::core::{Message, ToolResult};
    use serde_json::json;

    fn make_tool_result(tool_name: &str, content: &str) -> Message {
        Message::tool_result(ToolResult {
            tool_id: format!("id_{}", tool_name),
            tool_name: tool_name.to_string(),
            result: serde_json::json!(content),
            result_for_assistant: Some(content.to_string()),
            is_error: false,
            duration_ms: None,
            image_attachments: None,
        })
    }

    fn make_tool_result_with_data(
        tool_name: &str,
        data: serde_json::Value,
        assistant_text: &str,
    ) -> Message {
        Message::tool_result(ToolResult {
            tool_id: format!("id_{}", tool_name),
            tool_name: tool_name.to_string(),
            result: data,
            result_for_assistant: Some(assistant_text.to_string()),
            is_error: false,
            duration_ms: None,
            image_attachments: None,
        })
    }

    #[test]
    fn clears_old_compactable_results() {
        let mut messages = vec![
            Message::user("hello".to_string()),
            Message::assistant("ok".to_string()),
            make_tool_result("Read", "file content 1"),
            make_tool_result("Read", "file content 2"),
            make_tool_result("Grep", "grep output"),
            make_tool_result("Read", "file content 3"),
        ];

        let config = MicrocompactConfig {
            keep_recent: 2,
            trigger_ratio: 0.0,
        };

        let result = microcompact_messages(&mut messages, &config);
        assert!(result.is_some());
        let stats = result.unwrap();
        assert_eq!(stats.tools_cleared, 2);
        assert_eq!(stats.tools_kept, 2);

        // First two tool results should be cleared
        if let MessageContent::ToolResult {
            ref result_for_assistant,
            ..
        } = messages[2].content
        {
            assert_eq!(result_for_assistant.as_deref(), Some(CLEARED_PLACEHOLDER));
        } else {
            panic!("expected ToolResult");
        }

        // Last two should be intact
        if let MessageContent::ToolResult {
            ref result_for_assistant,
            ..
        } = messages[5].content
        {
            assert_ne!(result_for_assistant.as_deref(), Some(CLEARED_PLACEHOLDER));
        } else {
            panic!("expected ToolResult");
        }
    }

    #[test]
    fn skips_non_compactable_tools() {
        let mut messages = vec![
            make_tool_result("TodoWrite", "todo data"),
            make_tool_result("Read", "file content"),
        ];

        let config = MicrocompactConfig {
            keep_recent: 1,
            trigger_ratio: 0.0,
        };

        let result = microcompact_messages(&mut messages, &config);
        assert!(result.is_none());
    }

    #[test]
    fn no_op_when_within_keep_window() {
        let mut messages = vec![make_tool_result("Read", "a"), make_tool_result("Grep", "b")];

        let config = MicrocompactConfig {
            keep_recent: 5,
            trigger_ratio: 0.0,
        };

        let result = microcompact_messages(&mut messages, &config);
        assert!(result.is_none());
    }

    #[test]
    fn idempotent_on_already_cleared() {
        let mut messages = vec![
            make_tool_result("Read", "content 1"),
            make_tool_result("Read", "content 2"),
            make_tool_result("Read", "content 3"),
        ];

        let config = MicrocompactConfig {
            keep_recent: 1,
            trigger_ratio: 0.0,
        };

        let r1 = microcompact_messages(&mut messages, &config);
        assert_eq!(r1.unwrap().tools_cleared, 2);

        // Second pass should be a no-op
        let r2 = microcompact_messages(&mut messages, &config);
        assert!(r2.is_none());
    }

    #[test]
    fn preserves_read_target_before_clearing_tool_result() {
        let mut messages = vec![
            make_tool_result_with_data(
                "Read",
                json!({
                    "file_path": "src/main.rs",
                    "content": "fn main() {}",
                    "success": true
                }),
                "Read lines 1-1 from src/main.rs",
            )
            .with_turn_id("turn-old".to_string()),
            make_tool_result("Read", "recent"),
        ];

        let config = MicrocompactConfig {
            keep_recent: 1,
            trigger_ratio: 0.0,
        };
        let result = microcompact_messages_with_evidence(
            &mut messages,
            &config,
            MicrocompactEvidenceScope {
                session_id: "session-a",
                turn_id: "turn-current",
            },
        )
        .expect("microcompact result");

        assert_eq!(result.tools_cleared, 1);
        assert_eq!(result.evidence_events_preserved, 1);
        assert_eq!(result.evidence_events[0].session_id, "session-a");
        assert_eq!(result.evidence_events[0].turn_id, "turn-old");
        assert_eq!(result.evidence_events[0].tool_name, "Read");
        assert_eq!(
            result.evidence_events[0].target_kind,
            EvidenceLedgerTargetKind::File
        );
        assert_eq!(result.evidence_events[0].target, "src/main.rs");
        assert_eq!(
            result.evidence_events[0].status,
            EvidenceLedgerEventStatus::Succeeded
        );
    }

    #[test]
    fn preserves_failed_command_error_kind_before_clearing() {
        let mut messages = vec![
            make_tool_result_with_data(
                "Bash",
                json!({
                    "command": "cargo test",
                    "success": false,
                    "exit_code": 1,
                    "output": "test failed"
                }),
                "Command failed",
            ),
            make_tool_result("Read", "recent"),
        ];

        let config = MicrocompactConfig {
            keep_recent: 1,
            trigger_ratio: 0.0,
        };
        let result = microcompact_messages_with_evidence(
            &mut messages,
            &config,
            MicrocompactEvidenceScope {
                session_id: "session-a",
                turn_id: "turn-a",
            },
        )
        .expect("microcompact result");

        let event = &result.evidence_events[0];
        assert_eq!(event.target_kind, EvidenceLedgerTargetKind::Command);
        assert_eq!(event.target, "cargo test");
        assert_eq!(event.status, EvidenceLedgerEventStatus::Failed);
        assert_eq!(
            event.exit_code_or_error_kind.as_deref(),
            Some("exit_code:1")
        );
    }

    #[test]
    fn preserves_mutated_file_in_touched_files_before_clearing() {
        let mut messages = vec![
            make_tool_result_with_data(
                "Edit",
                json!({
                    "file_path": "src/lib.rs",
                    "success": true
                }),
                "Successfully edited src/lib.rs",
            ),
            make_tool_result("Read", "recent"),
        ];

        let config = MicrocompactConfig {
            keep_recent: 1,
            trigger_ratio: 0.0,
        };
        let result = microcompact_messages_with_evidence(
            &mut messages,
            &config,
            MicrocompactEvidenceScope {
                session_id: "session-a",
                turn_id: "turn-a",
            },
        )
        .expect("microcompact result");

        assert_eq!(result.evidence_events[0].touched_files, vec!["src/lib.rs"]);
    }
}
