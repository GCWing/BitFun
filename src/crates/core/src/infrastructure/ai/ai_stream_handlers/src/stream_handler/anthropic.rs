use crate::types::anthropic::{
    AnthropicSSEError, ContentBlock, ContentBlockDelta, ContentBlockStart, MessageDelta,
    MessageStart, Usage,
};
use crate::types::unified::UnifiedResponse;
use anyhow::{anyhow, Error, Result};
use eventsource_stream::Eventsource;
use futures::StreamExt;
use log::{debug, trace};
use reqwest::Response;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

/// Convert a byte stream into a structured response stream
///
/// # Arguments
/// * `response` - HTTP response
/// * `tx_event` - parsed event sender
/// * `tx_raw_sse` - optional raw SSE sender (collect raw data for diagnostics)
pub async fn handle_anthropic_stream(
    response: Response,
    tx_event: mpsc::UnboundedSender<Result<UnifiedResponse>>,
    tx_raw_sse: Option<mpsc::UnboundedSender<String>>,
) {
    let mut stream = response.bytes_stream().eventsource();
    let idle_timeout = Duration::from_secs(600);
    let mut received_done = false;
    let mut response_error: Option<Error> = None;
    let mut usage = Usage::default();

    loop {
        let sse_event = timeout(idle_timeout, stream.next()).await;
        let sse = match sse_event {
            Ok(Some(Ok(sse))) => sse,
            Ok(None) => {
                if !received_done {
                    let error = response_error.unwrap_or(anyhow!(
                        "SSE Error: stream closed before response completed"
                    ));
                    let _ = tx_event.send(Err(error));
                }
                return;
            }
            Ok(Some(Err(e))) => {
                let error_str = format!("SSE Error: {}", e);
                debug!("{}", error_str);
                let error = anyhow!(error_str);
                let _ = tx_event.send(Err(error));
                return;
            }
            Err(_) => {
                let _ = tx_event.send(Err(anyhow!("SSE Timeout: idle timeout waiting for SSE")));
                return;
            }
        };

        let event_type = sse.event;
        let data = sse.data;
        trace!("sse data: {:?}", data);

        if let Some(ref tx) = tx_raw_sse {
            let _ = tx.send(format!("[{}] {}", event_type, data));
        }

        match event_type.as_str() {
            "message_start" => {
                let message_start: MessageStart = match serde_json::from_str(&data) {
                    Ok(message_start) => message_start,
                    Err(e) => {
                        let err_str = format!("SSE Parsing Error: {e}, data: {}", &data);
                        debug!("{}", err_str);
                        let _ = tx_event.send(Err(anyhow!(err_str)));
                        continue;
                    }
                };
                usage.update(&message_start.message.usage);
            }
            "content_block_start" => {
                let content_block_start: ContentBlockStart = match serde_json::from_str(&data) {
                    Ok(content_block_start) => content_block_start,
                    Err(e) => {
                        let err_str = format!("SSE Parsing Error: {e}, data: {}", &data);
                        debug!("{}", err_str);
                        let _ = tx_event.send(Err(anyhow!(err_str)));
                        continue;
                    }
                };
                if matches!(
                    content_block_start.content_block,
                    ContentBlock::ToolUse { .. }
                ) {
                    let unified_response = UnifiedResponse::from(content_block_start);
                    trace!("unified_response: {:?}", unified_response);
                    let _ = tx_event.send(Ok(unified_response));
                }
            }
            "content_block_delta" => {
                let content_block_delta: ContentBlockDelta = match serde_json::from_str(&data) {
                    Ok(content_block_delta) => content_block_delta,
                    Err(e) => {
                        let err_str = format!("SSE Parsing Error: {e}, data: {}", &data);
                        debug!("{}", err_str);
                        let _ = tx_event.send(Err(anyhow!(err_str)));
                        continue;
                    }
                };
                match UnifiedResponse::try_from(content_block_delta) {
                    Ok(unified_response) => {
                        trace!("unified_response: {:?}", unified_response);
                        let _ = tx_event.send(Ok(unified_response));
                    }
                    Err(e) => {
                        debug!("Skipping invalid content_block_delta: {e}");
                    }
                };
            }
            "message_delta" => {
                let mut message_delta: MessageDelta = match serde_json::from_str(&data) {
                    Ok(message_delta) => message_delta,
                    Err(e) => {
                        let err_str = format!("SSE Parsing Error: {e}, data: {}", &data);
                        debug!("{}", err_str);
                        let _ = tx_event.send(Err(anyhow!(err_str)));
                        continue;
                    }
                };
                usage.update(&message_delta.usage);
                message_delta.usage = usage.clone();
                let unified_response = UnifiedResponse::from(message_delta);
                trace!("unified_response: {:?}", unified_response);
                let _ = tx_event.send(Ok(unified_response));
            }
            "error" => {
                let sse_error: AnthropicSSEError = match serde_json::from_str(&data) {
                    Ok(message_delta) => message_delta,
                    Err(e) => {
                        let err_str = format!("SSE Parsing Error: {e}, data: {}", &data);
                        debug!("{}", err_str);
                        let _ = tx_event.send(Err(anyhow!(err_str)));
                        continue;
                    }
                };
                response_error = Some(anyhow!(String::from(sse_error.error)))
            }
            "message_stop" => {
                received_done = true;
            }
            _ => {}
        }
    }
}
