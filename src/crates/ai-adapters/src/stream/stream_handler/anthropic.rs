use super::inline_think::InlineThinkParser;
use super::stream_stats::StreamStats;
use crate::stream::types::anthropic::{
    AnthropicSSEError, ContentBlock, ContentBlockDelta, ContentBlockStart, MessageDelta,
    MessageStart, Usage,
};
use crate::stream::types::unified::UnifiedResponse;
use anyhow::{anyhow, Result};
use eventsource_stream::Eventsource;
use futures::StreamExt;
use log::{error, trace};
use reqwest::Response;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

const AI_STREAM_RESPONSE_TARGET: &str = "ai::anthropic_stream_response";

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
    inline_think_in_text: bool,
) {
    let mut stream = response.bytes_stream().eventsource();
    let idle_timeout = Duration::from_secs(600);
    let mut usage = Usage::default();
    let mut stats = StreamStats::new("Anthropic");
    let mut inline_think_parser = InlineThinkParser::new(inline_think_in_text);

    loop {
        let sse_event = timeout(idle_timeout, stream.next()).await;
        let sse = match sse_event {
            Ok(Some(Ok(sse))) => sse,
            Ok(None) => {
                let error_msg = "SSE Error: stream closed before response completed";
                stats.log_summary("stream_closed_before_completion");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            Ok(Some(Err(e))) => {
                let error_msg = format!("SSE Error: {}", e);
                stats.log_summary("sse_stream_error");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            Err(_) => {
                let error_msg = "SSE Timeout: idle timeout waiting for SSE";
                stats.log_summary("sse_stream_timeout");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        trace!(target: AI_STREAM_RESPONSE_TARGET, "Anthropic SSE: {:?}", sse);
        let event_type = sse.event;
        let data = sse.data;
        stats.record_sse_event(&event_type);

        if let Some(ref tx) = tx_raw_sse {
            let _ = tx.send(format!("[{}] {}", event_type, data));
        }

        match event_type.as_str() {
            "message_start" => {
                let message_start: MessageStart = match serde_json::from_str(&data) {
                    Ok(message_start) => message_start,
                    Err(e) => {
                        stats.increment("error:sse_parsing");
                        let err_str = format!("SSE Parsing Error: {e}, data: {}", &data);
                        error!("{}", err_str);
                        continue;
                    }
                };
                if let Some(message_usage) = message_start.message.usage {
                    usage.update(&message_usage);
                }
            }
            "content_block_start" => {
                let content_block_start: ContentBlockStart = match serde_json::from_str(&data) {
                    Ok(content_block_start) => content_block_start,
                    Err(e) => {
                        stats.increment("error:sse_parsing");
                        let err_str = format!("SSE Parsing Error: {e}, data: {}", &data);
                        error!("{}", err_str);
                        continue;
                    }
                };
                if matches!(
                    content_block_start.content_block,
                    ContentBlock::ToolUse { .. }
                ) {
                    emit_normalized_response(
                        &mut inline_think_parser,
                        &tx_event,
                        &mut stats,
                        UnifiedResponse::from(content_block_start),
                    );
                }
            }
            "content_block_delta" => {
                let content_block_delta: ContentBlockDelta = match serde_json::from_str(&data) {
                    Ok(content_block_delta) => content_block_delta,
                    Err(e) => {
                        stats.increment("error:sse_parsing");
                        let err_str = format!("SSE Parsing Error: {e}, data: {}", &data);
                        error!("{}", err_str);
                        continue;
                    }
                };
                match UnifiedResponse::try_from(content_block_delta) {
                    Ok(unified_response) => emit_normalized_response(
                        &mut inline_think_parser,
                        &tx_event,
                        &mut stats,
                        unified_response,
                    ),
                    Err(e) => {
                        stats.increment("skip:invalid_content_block_delta");
                        error!("Skipping invalid content_block_delta: {}", e);
                    }
                };
            }
            "message_delta" => {
                let mut message_delta: MessageDelta = match serde_json::from_str(&data) {
                    Ok(message_delta) => message_delta,
                    Err(e) => {
                        stats.increment("error:sse_parsing");
                        let err_str = format!("SSE Parsing Error: {e}, data: {}", &data);
                        error!("{}", err_str);
                        continue;
                    }
                };
                if let Some(delta_usage) = message_delta.usage.as_ref() {
                    usage.update(delta_usage);
                }
                message_delta.usage = if usage.is_empty() {
                    None
                } else {
                    Some(usage.clone())
                };
                emit_normalized_response(
                    &mut inline_think_parser,
                    &tx_event,
                    &mut stats,
                    UnifiedResponse::from(message_delta),
                );
            }
            "error" => {
                let sse_error: AnthropicSSEError = match serde_json::from_str(&data) {
                    Ok(message_delta) => message_delta,
                    Err(e) => {
                        stats.increment("error:sse_parsing");
                        let err_str = format!("SSE Parsing Error: {e}, data: {}", &data);
                        stats.log_summary("sse_parsing_error");
                        error!("{}", err_str);
                        let _ = tx_event.send(Err(anyhow!(err_str)));
                        return;
                    }
                };
                stats.increment("error:api");
                stats.log_summary("error_event_received");
                let _ = tx_event.send(Err(anyhow!(String::from(sse_error.error))));
                return;
            }
            "message_stop" => {
                for unified_response in inline_think_parser.flush() {
                    trace!(
                        target: AI_STREAM_RESPONSE_TARGET,
                        "Anthropic unified response: {:?}",
                        unified_response
                    );
                    stats.record_unified_response(&unified_response);
                    let _ = tx_event.send(Ok(unified_response));
                }
                stats.log_summary("message_stop");
                return;
            }
            _ => {}
        }
    }
}

fn emit_normalized_response(
    inline_think_parser: &mut InlineThinkParser,
    tx_event: &mpsc::UnboundedSender<Result<UnifiedResponse>>,
    stats: &mut StreamStats,
    unified_response: UnifiedResponse,
) {
    for normalized_response in inline_think_parser.normalize_response(unified_response) {
        trace!(
            target: AI_STREAM_RESPONSE_TARGET,
            "Anthropic unified response: {:?}",
            normalized_response
        );
        stats.record_unified_response(&normalized_response);
        let _ = tx_event.send(Ok(normalized_response));
    }
}
