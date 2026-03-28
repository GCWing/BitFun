use super::{build_structured_compression_summary, CompressionFallbackOptions};
use crate::agentic::core::{
    render_system_reminder, render_user_query, CompressedMessageRole, CompressionEntry,
    CompressionPayload, Message, MessageSemanticKind, ToolCall, ToolResult,
};
use serde_json::json;

fn default_options() -> CompressionFallbackOptions {
    CompressionFallbackOptions {
        max_tokens: 10_000,
        user_chars: 120,
        assistant_chars: 120,
        tool_arg_chars: 80,
        tool_command_chars: 80,
    }
}

#[test]
fn clears_tool_results_from_compressed_history() {
    let assistant = Message::assistant_with_tools(
        "Checking file".to_string(),
        vec![ToolCall {
            tool_id: "tool_1".to_string(),
            tool_name: "Read".to_string(),
            arguments: json!({
                "file_path": "/tmp/demo.rs",
                "start_line": 1,
                "limit": 20
            }),
            is_error: false,
        }],
    );
    let tool_result = Message::tool_result(ToolResult {
        tool_id: "tool_1".to_string(),
        tool_name: "Read".to_string(),
        result: json!({"content": "ignored"}),
        result_for_assistant: Some("Read succeeded with file preview".to_string()),
        is_error: false,
        duration_ms: None,
        image_attachments: None,
    });

    let summary_artifact = build_structured_compression_summary(
        vec![vec![
            Message::user("inspect".to_string()),
            assistant,
            tool_result,
        ]],
        &default_options(),
    );

    let turn = match &summary_artifact.payload.entries[0] {
        CompressionEntry::Turn { messages, .. } => messages,
        _ => panic!("expected turn entry"),
    };
    let assistant_message = turn
        .iter()
        .find(|message| message.role == CompressedMessageRole::Assistant)
        .expect("assistant message");

    assert_eq!(assistant_message.tool_calls.len(), 1);
    assert!(!summary_artifact.summary_text.contains("Tool result:"));
    assert!(!summary_artifact
        .summary_text
        .contains("All tool results have been cleared"));
    assert!(summary_artifact.summary_text.contains("Historical turn 1:"));
}

#[test]
fn reuses_existing_compression_payload_atomically() {
    let prior_summary = "Previous conversation summary".to_string();
    let reminder_message = Message::user(render_system_reminder(&prior_summary))
        .with_semantic_kind(MessageSemanticKind::InternalReminder)
        .with_compression_payload(CompressionPayload::from_summary(prior_summary.clone()));

    let summary_artifact =
        build_structured_compression_summary(vec![vec![reminder_message]], &default_options());

    assert!(matches!(
        &summary_artifact.payload.entries[0],
        CompressionEntry::ModelSummary { text } if text == &prior_summary
    ));
}

#[test]
fn strips_user_query_markup_from_fallback_user_messages() {
    let raw = format!(
        "{}\n{}",
        render_user_query("Implement manual /compact"),
        render_system_reminder("Keep responses concise")
    );

    let summary_artifact =
        build_structured_compression_summary(vec![vec![Message::user(raw)]], &default_options());

    let turn = match &summary_artifact.payload.entries[0] {
        CompressionEntry::Turn { messages, .. } => messages,
        _ => panic!("expected turn entry"),
    };
    let user_message = turn
        .iter()
        .find(|message| message.role == CompressedMessageRole::User)
        .expect("user message");

    assert_eq!(
        user_message.text.as_deref(),
        Some("Implement manual /compact")
    );
    assert!(!summary_artifact.summary_text.contains("<user_query>"));
    assert!(!summary_artifact.summary_text.contains("<system_reminder>"));
}

#[test]
fn drops_system_reminder_only_user_messages_from_fallback_summary() {
    let summary_artifact = build_structured_compression_summary(
        vec![vec![Message::user(render_system_reminder(
            "Summarized context boundary marker",
        ))]],
        &default_options(),
    );

    assert!(summary_artifact.payload.entries.is_empty());
    assert_eq!(
        summary_artifact.summary_text,
        "No detailed historical entries fit within the remaining context budget."
    );
}

#[test]
fn groups_consecutive_assistant_messages_under_single_role_header() {
    let summary_artifact = build_structured_compression_summary(
        vec![vec![
            Message::user("Update the component styling.".to_string()),
            Message::assistant_with_tools(
                "".to_string(),
                vec![ToolCall {
                    tool_id: "tool_1".to_string(),
                    tool_name: "Read".to_string(),
                    arguments: json!({
                        "file_path": "/workspace/example.txt"
                    }),
                    is_error: false,
                }],
            ),
            Message::assistant_with_tools(
                "".to_string(),
                vec![ToolCall {
                    tool_id: "tool_2".to_string(),
                    tool_name: "Edit".to_string(),
                    arguments: json!({
                        "file_path": "/workspace/example.txt",
                        "old_string": "before",
                        "new_string": "after"
                    }),
                    is_error: false,
                }],
            ),
            Message::assistant("Updated the styling changes.".to_string()),
        ]],
        &default_options(),
    );

    let assistant_headers = summary_artifact.summary_text.matches("Assistant:").count();
    assert_eq!(assistant_headers, 1);
    assert!(summary_artifact.summary_text.contains(
        "Assistant:\nTool call: Read {\"file_path\":\"/workspace/example.txt\"}"
    ));
    assert!(summary_artifact
        .summary_text
        .contains("Updated the styling changes."));
}
