mod common;

use bitfun_ai_adapters::providers::{openai::OpenAIMessageConverter, AnthropicMessageConverter};
use bitfun_core::agentic::core::Message as CoreMessage;
use bitfun_core::agentic::events::{AgenticEvent, ToolEventData};
use bitfun_core::util::types::Message as AIMessage;
use common::sse_fixture_server::FixtureSseServerOptions;
use common::stream_test_harness::{
    run_stream_fixture, run_stream_fixture_with_options, StreamFixtureProvider,
    StreamFixtureRunOptions,
};
use serde_json::json;

fn build_replay_assistant_message(
    result: &bitfun_core::agentic::execution::StreamResult,
) -> CoreMessage {
    let reasoning = if result.full_thinking.is_empty() {
        if result.reasoning_content_present {
            Some(String::new())
        } else {
            None
        }
    } else {
        Some(result.full_thinking.clone())
    };

    CoreMessage::assistant_with_reasoning(
        reasoning,
        result.full_text.clone(),
        result.tool_calls.clone(),
    )
    .with_thinking_signature(result.thinking_signature.clone())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replays_structurally_empty_openai_reasoning_content_with_tool_call() {
    let output = run_stream_fixture(
        StreamFixtureProvider::OpenAi,
        "stream/openai/empty_reasoning_content_text_and_tool_call.sse",
        FixtureSseServerOptions::default(),
    )
    .await;

    let result = output.result.expect("stream result");

    assert!(result.reasoning_content_present);
    assert!(result.full_thinking.is_empty());
    assert_eq!(result.full_text, "Let me check that for you.");
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].tool_id, "call_ds_1");
    assert_eq!(result.tool_calls[0].tool_name, "lookup_status");
    assert_eq!(
        result.tool_calls[0].arguments,
        json!({ "ticket_id": "BF-123" })
    );
    assert!(!result.tool_calls[0].is_error);
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
    assert!(
        thinking_chunks.is_empty(),
        "empty reasoning content should not emit visible thinking chunks"
    );

    let tool_events: Vec<(&str, &str)> = output
        .events
        .iter()
        .filter_map(|event| match event {
            AgenticEvent::ToolEvent {
                tool_event:
                    ToolEventData::ParamsPartial {
                        tool_id, params, ..
                    },
                ..
            } => Some((tool_id.as_str(), params.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(
        tool_events,
        vec![
            ("call_ds_1", "{\"ticket_id\":\"BF-"),
            ("call_ds_1", "123\"}")
        ]
    );

    let replay_message: AIMessage = build_replay_assistant_message(&result).into();
    let openai_payload = OpenAIMessageConverter::convert_messages(vec![replay_message]);

    assert_eq!(openai_payload.len(), 1);
    assert_eq!(
        openai_payload[0]["content"],
        json!("Let me check that for you.")
    );
    assert_eq!(openai_payload[0]["reasoning_content"], json!(""));
    assert_eq!(openai_payload[0]["tool_calls"][0]["id"], json!("call_ds_1"));
    assert_eq!(
        openai_payload[0]["tool_calls"][0]["function"]["name"],
        json!("lookup_status")
    );
    assert_eq!(
        openai_payload[0]["tool_calls"][0]["function"]["arguments"],
        json!("{\"ticket_id\":\"BF-123\"}")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replays_structurally_empty_anthropic_thinking_with_signature_and_tool_use() {
    let output = run_stream_fixture_with_options(
        StreamFixtureProvider::Anthropic,
        "stream/anthropic/empty_thinking_signature_text_and_tool_use.sse",
        StreamFixtureRunOptions {
            anthropic_inline_think_in_text: false,
            ..Default::default()
        },
    )
    .await;

    let result = output.result.expect("stream result");

    assert!(result.reasoning_content_present);
    assert!(result.full_thinking.is_empty());
    assert_eq!(result.full_text, "Let me check that for you.");
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].tool_id, "toolu_ds_1");
    assert_eq!(result.tool_calls[0].tool_name, "lookup_status");
    assert_eq!(
        result.tool_calls[0].arguments,
        json!({ "ticket_id": "BF-123" })
    );
    assert!(!result.tool_calls[0].is_error);
    assert_eq!(result.thinking_signature.as_deref(), Some("sig_empty_123"));
    assert_eq!(
        result.usage.as_ref().map(|usage| usage.total_token_count),
        Some(25)
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
    assert!(
        thinking_chunks.is_empty(),
        "empty thinking content should not emit visible thinking chunks"
    );

    let early_detected = output.events.iter().any(|event| {
        matches!(
            event,
            AgenticEvent::ToolEvent {
                tool_event: ToolEventData::EarlyDetected { tool_id, tool_name },
                ..
            } if tool_id == "toolu_ds_1" && tool_name == "lookup_status"
        )
    });
    assert!(
        early_detected,
        "expected tool_use block to trigger early detection"
    );

    let replay_message: AIMessage = build_replay_assistant_message(&result).into();
    let (_, anthropic_messages) = AnthropicMessageConverter::convert_messages(vec![replay_message]);
    let content = anthropic_messages[0]["content"]
        .as_array()
        .expect("assistant content");

    assert_eq!(content[0]["type"], json!("thinking"));
    assert_eq!(content[0]["thinking"], json!(""));
    assert_eq!(content[0]["signature"], json!("sig_empty_123"));
    assert_eq!(content[1]["type"], json!("text"));
    assert_eq!(content[1]["text"], json!("Let me check that for you."));
    assert_eq!(content[2]["type"], json!("tool_use"));
    assert_eq!(content[2]["id"], json!("toolu_ds_1"));
    assert_eq!(content[2]["name"], json!("lookup_status"));
    assert_eq!(content[2]["input"], json!({ "ticket_id": "BF-123" }));
}
