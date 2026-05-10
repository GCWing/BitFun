mod common;

use bitfun_events::AgenticEvent;
use common::stream_test_harness::{
    run_stream_fixture_with_options, StreamFixtureProvider, StreamFixtureRunOptions,
};
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn anthropic_fixture_parses_inline_think_tags_inside_text_delta() {
    let output = run_stream_fixture_with_options(
        StreamFixtureProvider::Anthropic,
        "stream/anthropic/inline_think_text.sse",
        StreamFixtureRunOptions {
            anthropic_inline_think_in_text: true,
            ..Default::default()
        },
    )
    .await;

    let result = output.result.expect("stream result");

    assert_eq!(
        result.full_thinking,
        "I should inspect the data. Then answer carefully."
    );
    assert_eq!(result.full_text, "Final answer.");
    assert!(result.tool_calls.is_empty());
    assert_eq!(
        result.usage.as_ref().map(|usage| usage.total_token_count),
        Some(10)
    );

    let thinking_chunks: Vec<(&str, bool)> = output
        .events
        .iter()
        .filter_map(|event| match event {
            AgenticEvent::ThinkingChunk {
                content, is_end, ..
            } => Some((content.as_str(), *is_end)),
            _ => None,
        })
        .collect();
    assert_eq!(
        thinking_chunks,
        vec![
            ("I should inspect the data.", false),
            (" Then answer carefully.", false),
            ("", true),
        ]
    );

    let text_chunks: Vec<&str> = output
        .events
        .iter()
        .filter_map(|event| match event {
            AgenticEvent::TextChunk { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text_chunks, vec!["Final answer."]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn anthropic_extended_thinking_sse_produces_reasoning_and_text() {
    let output = run_stream_fixture_with_options(
        StreamFixtureProvider::Anthropic,
        "stream/anthropic/extended_thinking.sse",
        StreamFixtureRunOptions {
            anthropic_inline_think_in_text: false,
            ..Default::default()
        },
    )
    .await;

    let result = output.result.expect("stream result");

    assert_eq!(
        result.full_thinking,
        "Let me reason about this. Step by step."
    );
    assert_eq!(result.full_text, "Here is the answer.");
    assert!(result.tool_calls.is_empty());
    assert_eq!(
        result.usage.as_ref().map(|usage| usage.total_token_count),
        Some(25)
    );
    assert_eq!(result.thinking_signature.as_deref(), Some("sig_abc123"));

    let thinking_chunks: Vec<(&str, bool)> = output
        .events
        .iter()
        .filter_map(|event| match event {
            AgenticEvent::ThinkingChunk {
                content, is_end, ..
            } => Some((content.as_str(), *is_end)),
            _ => None,
        })
        .collect();
    assert_eq!(
        thinking_chunks,
        vec![
            ("Let me reason about this.", false),
            (" Step by step.", false),
            ("", true),
        ]
    );

    let text_chunks: Vec<&str> = output
        .events
        .iter()
        .filter_map(|event| match event {
            AgenticEvent::TextChunk { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text_chunks, vec!["Here is the answer."]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn anthropic_parallel_tool_use_keeps_arguments_separate_by_index() {
    let output = run_stream_fixture_with_options(
        StreamFixtureProvider::Anthropic,
        "stream/anthropic/interleaved_parallel_tool_use.sse",
        StreamFixtureRunOptions {
            anthropic_inline_think_in_text: false,
            ..Default::default()
        },
    )
    .await;

    let result = output.result.expect("stream result");

    assert_eq!(result.tool_calls.len(), 2);
    assert_eq!(result.tool_calls[0].tool_id, "toolu_parallel_0");
    assert_eq!(result.tool_calls[0].tool_name, "tool_a");
    assert_eq!(result.tool_calls[0].arguments, json!({ "a": 1 }));
    assert!(!result.tool_calls[0].is_error);
    assert_eq!(result.tool_calls[1].tool_id, "toolu_parallel_1");
    assert_eq!(result.tool_calls[1].tool_name, "tool_b");
    assert_eq!(result.tool_calls[1].arguments, json!({ "b": 2 }));
    assert!(!result.tool_calls[1].is_error);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn anthropic_malformed_content_block_delta_fails_stream() {
    let output = run_stream_fixture_with_options(
        StreamFixtureProvider::Anthropic,
        "stream/anthropic/malformed_content_block_delta.sse",
        StreamFixtureRunOptions {
            anthropic_inline_think_in_text: false,
            ..Default::default()
        },
    )
    .await;

    let error = output.result.expect_err("malformed SSE delta should fail");
    assert!(
        error.error.to_string().contains("SSE Parsing Error"),
        "unexpected error: {}",
        error.error
    );
}
