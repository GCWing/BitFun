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
}

/// Run microcompact on the message list **in place**.
///
/// Returns `None` if no clearing was performed (e.g. not enough compactable
/// results, or all are within the keep window).
pub fn microcompact_messages(
    messages: &mut [Message],
    config: &MicrocompactConfig,
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
    for &idx in to_clear {
        let msg = &mut messages[idx];
        if let MessageContent::ToolResult {
            ref mut result,
            ref mut result_for_assistant,
            ref mut image_attachments,
            ..
        } = msg.content
        {
            // Skip if already cleared
            if result_for_assistant.as_deref() == Some(CLEARED_PLACEHOLDER) {
                continue;
            }
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
    info!(
        "Microcompact: cleared {} tool result(s), kept {} recent",
        cleared, kept
    );
    debug!(
        "Microcompact details: total_compactable={}, keep_recent={}, cleared={}",
        compactable_indices.len(),
        config.keep_recent,
        cleared
    );

    Some(MicrocompactResult {
        tools_cleared: cleared,
        tools_kept: kept,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::core::{Message, ToolResult};

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
}
