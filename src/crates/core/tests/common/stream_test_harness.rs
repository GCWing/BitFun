use super::fixture_loader::load_fixture_bytes;
use super::sse_fixture_server::{FixtureSseServer, FixtureSseServerOptions};
use bitfun_ai_adapters::stream::{
    handle_anthropic_stream, handle_gemini_stream, handle_openai_stream, handle_responses_stream,
    UnifiedResponse,
};
use bitfun_core::agentic::events::{AgenticEvent, EventQueue, EventQueueConfig};
use bitfun_core::agentic::execution::{StreamProcessError, StreamResult};
use bitfun_core::StreamProcessor;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_util::sync::CancellationToken;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum StreamFixtureProvider {
    OpenAi,
    Anthropic,
    Gemini,
    Responses,
}

#[derive(Debug)]
pub struct StreamFixtureRunOutput {
    pub result: Result<StreamResult, StreamProcessError>,
    pub events: Vec<AgenticEvent>,
}

#[derive(Debug, Clone, Copy)]
pub struct StreamFixtureRunOptions {
    pub server_options: FixtureSseServerOptions,
    pub openai_inline_think_in_text: bool,
    pub anthropic_inline_think_in_text: bool,
}

impl Default for StreamFixtureRunOptions {
    fn default() -> Self {
        Self {
            server_options: FixtureSseServerOptions::default(),
            openai_inline_think_in_text: false,
            anthropic_inline_think_in_text: false,
        }
    }
}

pub async fn run_stream_fixture(
    provider: StreamFixtureProvider,
    fixture_relative_path: &str,
    server_options: FixtureSseServerOptions,
) -> StreamFixtureRunOutput {
    run_stream_fixture_with_options(
        provider,
        fixture_relative_path,
        StreamFixtureRunOptions {
            server_options,
            ..Default::default()
        },
    )
    .await
}

pub async fn run_stream_fixture_with_options(
    provider: StreamFixtureProvider,
    fixture_relative_path: &str,
    options: StreamFixtureRunOptions,
) -> StreamFixtureRunOutput {
    let fixture_bytes = load_fixture_bytes(fixture_relative_path);
    let fixture_server = FixtureSseServer::spawn(fixture_bytes, options.server_options).await;

    let response = reqwest::Client::new()
        .get(fixture_server.url())
        .send()
        .await
        .expect("fixture SSE request should succeed")
        .error_for_status()
        .expect("fixture SSE response should be 2xx");

    let (tx_event, rx_event) = mpsc::unbounded_channel::<Result<UnifiedResponse, anyhow::Error>>();
    let (tx_raw_sse, rx_raw_sse) = mpsc::unbounded_channel::<String>();

    match provider {
        StreamFixtureProvider::OpenAi => {
            tokio::spawn(handle_openai_stream(
                response,
                tx_event,
                Some(tx_raw_sse),
                options.openai_inline_think_in_text,
                None,
            ));
        }
        StreamFixtureProvider::Anthropic => {
            tokio::spawn(handle_anthropic_stream(
                response,
                tx_event,
                Some(tx_raw_sse),
                options.anthropic_inline_think_in_text,
                None,
            ));
        }
        StreamFixtureProvider::Gemini => {
            tokio::spawn(handle_gemini_stream(
                response,
                tx_event,
                Some(tx_raw_sse),
                None,
            ));
        }
        StreamFixtureProvider::Responses => {
            tokio::spawn(handle_responses_stream(
                response,
                tx_event,
                Some(tx_raw_sse),
                None,
            ));
        }
    }

    let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let processor = StreamProcessor::new(event_queue.clone());
    let unified_stream = UnboundedReceiverStream::new(rx_event).boxed();
    let cancellation_token = CancellationToken::new();

    let result = processor
        .process_stream(
            unified_stream,
            Some(rx_raw_sse),
            "session_fixture".to_string(),
            "turn_fixture".to_string(),
            "round_fixture".to_string(),
            None,
            &cancellation_token,
        )
        .await;

    let events = drain_all_events(&event_queue).await;

    StreamFixtureRunOutput { result, events }
}

async fn drain_all_events(event_queue: &Arc<EventQueue>) -> Vec<AgenticEvent> {
    let mut events = Vec::new();

    loop {
        let batch = event_queue.dequeue_batch(256).await;
        if batch.is_empty() {
            break;
        }
        events.extend(batch.into_iter().map(|envelope| envelope.event));
    }

    events
}
