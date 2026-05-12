use super::inline_think::InlineThinkParser;
use super::stream_stats::StreamStats;
use super::{next_stream_item, TimedStreamItem};
use crate::stream::types::anthropic::{
    AnthropicSSEError, ContentBlock, ContentBlockDelta, ContentBlockStart, MessageDelta,
    MessageStart, Usage,
};
use crate::stream::types::unified::UnifiedResponse;
use anyhow::{anyhow, Result};
use eventsource_stream::Eventsource;
use log::{error, trace};
use reqwest::Response;
use std::time::Duration;
use tokio::sync::mpsc;

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
    idle_timeout: Option<Duration>,
) {
    let mut stream = response.bytes_stream().eventsource();
    let mut usage = Usage::default();
    let mut stats = StreamStats::new("Anthropic");
    let mut inline_think_parser = InlineThinkParser::new(inline_think_in_text);
    let mut received_finish_reason = false;

    loop {
        let sse = match next_stream_item(&mut stream, idle_timeout).await {
            TimedStreamItem::Item(Ok(sse)) => sse,
            TimedStreamItem::End => {
                if received_finish_reason {
                    for unified_response in inline_think_parser.flush() {
                        trace!(
                            target: AI_STREAM_RESPONSE_TARGET,
                            "Anthropic unified response: {:?}",
                            unified_response
                        );
                        stats.record_unified_response(&unified_response);
                        let _ = tx_event.send(Ok(unified_response));
                    }
                    stats.log_summary("stream_closed_after_finish_reason");
                    return;
                }
                let error_msg = "SSE Error: stream closed before response completed";
                stats.log_summary("stream_closed_before_completion");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            TimedStreamItem::Item(Err(e)) => {
                let error_msg = format!("SSE Error: {}", e);
                stats.log_summary("sse_stream_error");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            TimedStreamItem::TimedOut => {
                let timeout_secs = idle_timeout.map(|timeout| timeout.as_secs()).unwrap_or(0);
                let error_msg = format!(
                    "SSE Timeout: idle timeout waiting for SSE after {}s",
                    timeout_secs
                );
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

        if let Some(error_msg) = format_provider_error_from_sse_message(&event_type, &data) {
            stats.increment("error:provider_message");
            stats.log_summary("provider_error_message_received");
            error!("{}", error_msg);
            let _ = tx_event.send(Err(anyhow!(error_msg)));
            return;
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
                        stats.log_summary("sse_parsing_error");
                        error!("{}", err_str);
                        let _ = tx_event.send(Err(anyhow!(err_str)));
                        return;
                    }
                };
                // Emit for Thinking and ToolUse content_block_start events.
                // Note: For Thinking blocks, the Anthropic protocol sends signature=null
                // in content_block_start and the actual signature in a subsequent
                // signature_delta event. Both emit a UnifiedResponse; the downstream
                // processor correctly overwrites the initial null signature.
                if matches!(
                    content_block_start.content_block,
                    ContentBlock::Thinking { .. } | ContentBlock::ToolUse { .. }
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
                        stats.log_summary("sse_parsing_error");
                        error!("{}", err_str);
                        let _ = tx_event.send(Err(anyhow!(err_str)));
                        return;
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
                let unified_response = UnifiedResponse::from(message_delta);
                if unified_response.finish_reason.is_some() {
                    received_finish_reason = true;
                }
                emit_normalized_response(
                    &mut inline_think_parser,
                    &tx_event,
                    &mut stats,
                    unified_response,
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

fn format_provider_error_from_sse_message(event_type: &str, data: &str) -> Option<String> {
    if event_type != "message" {
        return None;
    }

    let value: serde_json::Value = serde_json::from_str(data).ok()?;
    let error = value.get("error")?.as_object()?;
    let code = error
        .get("code")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .or_else(|| error.get("code").map(|value| value.to_string()))?;
    let message = error
        .get("message")
        .and_then(|value| value.as_str())
        .unwrap_or("Provider returned an error");
    let request_id = value
        .get("request_id")
        .or_else(|| value.get("requestId"))
        .and_then(|value| value.as_str());

    let mut formatted = format!(
        "Provider error: provider=anthropic_compatible, code={}, message={}",
        code, message
    );
    if let Some(request_id) = request_id {
        formatted.push_str(&format!(", request_id={}", request_id));
    }

    Some(formatted)
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

#[cfg(test)]
mod tests {
    use super::format_provider_error_from_sse_message;

    #[test]
    fn extracts_glm_business_error_from_message_event() {
        let raw = r#"{"error":{"code":"1113","message":"余额不足或无可用资源包,请充值。"},"request_id":"20260425142416"}"#;

        let formatted = format_provider_error_from_sse_message("message", raw).unwrap();

        assert!(formatted.contains("Provider error"));
        assert!(formatted.contains("code=1113"));
        assert!(formatted.contains("余额不足或无可用资源包"));
        assert!(formatted.contains("request_id=20260425142416"));
    }

    #[test]
    fn ignores_regular_anthropic_delta_events() {
        let raw = r#"{"type":"message_delta","delta":{"stop_reason":null}}"#;

        assert!(format_provider_error_from_sse_message("message_delta", raw).is_none());
    }
}
