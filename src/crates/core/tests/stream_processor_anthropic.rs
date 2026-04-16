mod common;

use bitfun_core::agentic::events::AgenticEvent;
use common::stream_test_harness::{
    run_stream_fixture_with_options, StreamFixtureProvider, StreamFixtureRunOptions,
};

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
