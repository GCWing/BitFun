mod common;

use bitfun_core::agentic::events::AgenticEvent;
use common::sse_fixture_server::FixtureSseServerOptions;
use common::stream_test_harness::{run_stream_fixture, StreamFixtureProvider};
use serde_json::json;

fn assert_no_stream_failure_event(events: &[AgenticEvent]) {
    let failed_or_cancelled = events.iter().any(|event| {
        matches!(
            event,
            AgenticEvent::DialogTurnFailed { .. } | AgenticEvent::DialogTurnCancelled { .. }
        )
    });
    assert!(
        !failed_or_cancelled,
        "malformed tool arguments should be reported as tool-call errors, not stream failures"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn anthropic_malformed_tool_arguments_are_not_repaired() {
    let output = run_stream_fixture(
        StreamFixtureProvider::Anthropic,
        "stream/anthropic/malformed_tool_arguments_extra_brace.sse",
        FixtureSseServerOptions::default(),
    )
    .await;

    let result = output.result.expect("stream result");
    assert_no_stream_failure_event(&output.events);

    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].tool_id, "toolu_bad_json");
    assert_eq!(result.tool_calls[0].tool_name, "tool_a");
    assert_eq!(result.tool_calls[0].arguments, json!({}));
    assert_eq!(
        result.tool_calls[0].raw_arguments.as_deref(),
        Some("{\"a\":1}}")
    );
    assert!(result.tool_calls[0].is_error);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_malformed_tool_arguments_are_not_repaired() {
    let output = run_stream_fixture(
        StreamFixtureProvider::Responses,
        "stream/responses/malformed_function_call_arguments.sse",
        FixtureSseServerOptions::default(),
    )
    .await;

    let result = output.result.expect("stream result");
    assert_no_stream_failure_event(&output.events);

    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].tool_id, "call_resp_bad_json");
    assert_eq!(result.tool_calls[0].tool_name, "tool_a");
    assert_eq!(result.tool_calls[0].arguments, json!({}));
    assert_eq!(
        result.tool_calls[0].raw_arguments.as_deref(),
        Some("{\"a\":1}}")
    );
    assert!(result.tool_calls[0].is_error);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gemini_string_tool_arguments_are_not_coerced_to_object() {
    let output = run_stream_fixture(
        StreamFixtureProvider::Gemini,
        "stream/gemini/function_call_string_args.sse",
        FixtureSseServerOptions::default(),
    )
    .await;

    let result = output.result.expect("stream result");
    assert_no_stream_failure_event(&output.events);

    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].tool_name, "tool_a");
    assert_eq!(result.tool_calls[0].arguments, json!("git status"));
    assert!(!result.tool_calls[0].is_error);
}
