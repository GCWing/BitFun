use super::inline_think::InlineThinkParser;
use super::stream_stats::StreamStats;
use super::{next_stream_item, StreamTimeoutController, StreamTimeoutStage, TimedStreamItem};
use crate::stream::types::openai::OpenAISSEData;
use crate::stream::types::unified::UnifiedResponse;
use anyhow::{anyhow, Result};
use bitfun_core_types::errors::AiProviderError;
use eventsource_stream::Eventsource;
use log::{error, trace, warn};
use reqwest::Response;
use serde_json::Value;
use std::time::Duration;
use tokio::sync::mpsc;

const OPENAI_CHAT_COMPLETION_CHUNK_OBJECT: &str = "chat.completion.chunk";
/// MiniMax (and possibly other providers) close a streaming response with a
/// non-streaming `chat.completion` frame instead of a true `chunk`. That final
/// frame is the only one carrying authoritative usage, so we accept it too.
const OPENAI_CHAT_COMPLETION_OBJECT: &str = "chat.completion";
const AI_STREAM_RESPONSE_TARGET: &str = "ai::openai_stream_response";

#[derive(Debug)]
struct OpenAIResponseNormalizer {
    inline_think_parser: InlineThinkParser,
}

impl OpenAIResponseNormalizer {
    fn new(inline_think_in_text: bool) -> Self {
        Self {
            inline_think_parser: InlineThinkParser::new(inline_think_in_text),
        }
    }

    fn normalize_response(&mut self, response: UnifiedResponse) -> Vec<UnifiedResponse> {
        self.inline_think_parser.normalize_response(response)
    }

    fn flush(&mut self) -> Vec<UnifiedResponse> {
        self.inline_think_parser.flush()
    }
}

fn is_valid_chat_completion_chunk_weak(event_json: &Value) -> bool {
    // Standard streaming frames use `chat.completion.chunk`. MiniMax's final
    // SSE frame, however, switches to the non-streaming `chat.completion`
    // shape (choice carries `message` rather than `delta`) and is the ONLY
    // chunk that contains the authoritative usage block. Accept both — the
    // OpenAISSEData deserialization downstream tolerates either choice shape.
    matches!(
        event_json.get("object").and_then(|value| value.as_str()),
        Some(OPENAI_CHAT_COMPLETION_CHUNK_OBJECT) | Some(OPENAI_CHAT_COMPLETION_OBJECT)
    )
}

fn extract_sse_api_error_message(event_json: &Value) -> Option<String> {
    let error = event_json.get("error")?;
    if let Some(message) = error.get("message").and_then(|value| value.as_str()) {
        return Some(message.to_string());
    }
    if let Some(message) = error.as_str() {
        return Some(message.to_string());
    }
    Some("An error occurred during streaming".to_string())
}

fn extract_sse_api_error(event_json: &Value) -> Option<AiProviderError> {
    let error = event_json.get("error")?;
    let code = error
        .get("code")
        .or_else(|| error.get("type"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let message = extract_sse_api_error_message(event_json)?;
    Some(AiProviderError::from_parts(
        message,
        Some("openai".to_string()),
        code,
        None,
    ))
}

/// Convert a byte stream into a structured response stream
///
/// # Arguments
/// * `response` - HTTP response
/// * `tx_event` - parsed event sender
/// * `tx_raw_sse` - optional raw SSE sender (collect raw data for diagnostics)
/// * `qoder_envelope` - when true, each `data:` payload is the Qoder gateway
///   envelope `{"headers":{...},"body":"<inner>","statusCode":"OK",...}`;
///   the inner `body` string (a standard OpenAI SSE chunk or `[DONE]`) is
///   unwrapped before the normal OpenAI parsing. Qoder also appends a
///   terminal `event:finish` with a duration summary that has no `body`
///   field — that event is skipped as non-standard.
pub async fn handle_openai_stream(
    response: Response,
    tx_event: mpsc::UnboundedSender<Result<UnifiedResponse>>,
    tx_raw_sse: Option<mpsc::UnboundedSender<String>>,
    inline_think_in_text: bool,
    ttft_timeout: Option<Duration>,
    idle_timeout: Option<Duration>,
) {
    handle_openai_stream_inner(
        response,
        tx_event,
        tx_raw_sse,
        inline_think_in_text,
        ttft_timeout,
        idle_timeout,
        false,
    )
    .await
}

/// Qoder gateway variant of [`handle_openai_stream`]: unwraps the gateway
/// envelope on every SSE event before OpenAI parsing.
pub async fn handle_qoder_stream(
    response: Response,
    tx_event: mpsc::UnboundedSender<Result<UnifiedResponse>>,
    tx_raw_sse: Option<mpsc::UnboundedSender<String>>,
    inline_think_in_text: bool,
    ttft_timeout: Option<Duration>,
    idle_timeout: Option<Duration>,
) {
    handle_openai_stream_inner(
        response,
        tx_event,
        tx_raw_sse,
        inline_think_in_text,
        ttft_timeout,
        idle_timeout,
        true,
    )
    .await
}

async fn handle_openai_stream_inner(
    response: Response,
    tx_event: mpsc::UnboundedSender<Result<UnifiedResponse>>,
    tx_raw_sse: Option<mpsc::UnboundedSender<String>>,
    inline_think_in_text: bool,
    ttft_timeout: Option<Duration>,
    idle_timeout: Option<Duration>,
    qoder_envelope: bool,
) {
    let mut stream = response.bytes_stream().eventsource();
    let mut stats = StreamStats::new("OpenAI");
    let mut timeout_controller = StreamTimeoutController::new(ttft_timeout, idle_timeout);
    // Track whether a chunk with `finish_reason` was received.
    // Some providers (e.g. MiniMax) close the stream after the final chunk
    // without sending `[DONE]`, so we treat `Ok(None)` as a normal termination
    // when a finish_reason has already been seen.
    let mut received_finish_reason = false;
    let mut normalizer = OpenAIResponseNormalizer::new(inline_think_in_text);

    loop {
        let sse = match next_stream_item(&mut stream, &timeout_controller).await {
            TimedStreamItem::Item(Ok(sse)) => sse,
            TimedStreamItem::End => {
                if received_finish_reason {
                    for normalized_response in normalizer.flush() {
                        stats.record_unified_response(&normalized_response);
                        let _ = tx_event.send(Ok(normalized_response));
                    }
                    stats.log_summary("stream_closed_after_finish_reason");
                    return;
                }
                let error_msg = "SSE stream closed before response completed";
                stats.log_summary("stream_closed_before_completion");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            TimedStreamItem::Item(Err(e)) => {
                let error_msg = format!("SSE stream error: {}", e);
                stats.log_summary("sse_stream_error");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            TimedStreamItem::TimedOut(StreamTimeoutStage::Ttft) => {
                let timeout_secs = ttft_timeout.map(|timeout| timeout.as_secs()).unwrap_or(0);
                let error_msg = format!(
                    "OpenAI stream TTFT timeout after {}s waiting for first effective output",
                    timeout_secs
                );
                stats.log_summary("ttft_timeout");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            TimedStreamItem::TimedOut(StreamTimeoutStage::Idle) => {
                let timeout_secs = idle_timeout.map(|timeout| timeout.as_secs()).unwrap_or(0);
                let error_msg = format!("SSE stream timeout after {}s", timeout_secs);
                stats.log_summary("sse_stream_timeout");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        let raw = sse.data;
        stats.record_sse_event("data");
        trace!(target: AI_STREAM_RESPONSE_TARGET, "OpenAI SSE: {:?}", raw);
        if let Some(ref tx) = tx_raw_sse {
            let _ = tx.send(raw.clone());
        }
        // Qoder gateway wraps each chunk as `{"headers":{...},"body":"<inner>",...}`.
        // Unwrap the inner body string before OpenAI parsing; the terminal
        // `event:finish` duration summary has no body field and is skipped.
        let raw = if qoder_envelope {
            match unwrap_qoder_envelope(&raw) {
                Some(inner) => inner,
                None => continue,
            }
        } else {
            raw
        };
        if raw == "[DONE]" {
            for normalized_response in normalizer.flush() {
                stats.record_unified_response(&normalized_response);
                let _ = tx_event.send(Ok(normalized_response));
            }
            stats.increment("marker:done");
            stats.log_summary("done_marker_received");
            return;
        }

        let event_json: Value = match serde_json::from_str(&raw) {
            Ok(json) => json,
            Err(e) => {
                let error_msg = format!("SSE parsing error: {}, data: {}", e, raw);
                stats.increment("error:sse_parsing");
                stats.log_summary("sse_parsing_error");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        if let Some(mut provider_error) = extract_sse_api_error(&event_json) {
            provider_error.message =
                format!("SSE API error: {}, data: {}", provider_error.message, raw);
            stats.increment("error:api");
            stats.log_summary("sse_api_error");
            error!("{}", provider_error);
            let _ = tx_event.send(Err(anyhow!(provider_error)));
            return;
        }

        if !is_valid_chat_completion_chunk_weak(&event_json) {
            stats.increment("skip:non_standard_event");
            warn!(
                "Skipping non-standard OpenAI SSE event; object={}",
                event_json
                    .get("object")
                    .and_then(|value| value.as_str())
                    .unwrap_or("<missing>")
            );
            continue;
        }

        stats.increment("chunk:chat_completion");
        let sse_data: OpenAISSEData = match serde_json::from_value(event_json) {
            Ok(event) => event,
            Err(e) => {
                let error_msg = format!("SSE data schema error: {}, data: {}", e, raw);
                stats.increment("error:schema");
                stats.log_summary("sse_data_schema_error");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        let tool_call_count = sse_data.first_choice_tool_call_count();
        if tool_call_count > 1 {
            stats.increment("chunk:multi_tool_call");
            warn!(
                "OpenAI SSE chunk contains {} tool calls in the first choice; emitting indexed tool deltas",
                tool_call_count
            );
        }

        let has_empty_choices = sse_data.is_choices_empty();
        let unified_responses = sse_data.into_unified_responses();
        trace!(
            target: AI_STREAM_RESPONSE_TARGET,
            "OpenAI unified responses: {:?}",
            unified_responses
        );
        if unified_responses.is_empty() {
            if has_empty_choices {
                stats.increment("skip:empty_choices_no_usage");
                warn!(
                    "Ignoring OpenAI SSE chunk with empty choices and no usage payload: {}",
                    raw
                );
                // Ignore keepalive/metadata chunks with empty choices and no usage payload.
                continue;
            }
            // Defensive fallback: this should be unreachable if OpenAISSEData::into_unified_responses
            // keeps returning at least one event for all non-empty-choices chunks.
            let error_msg = format!("OpenAI SSE chunk produced no unified events, data: {}", raw);
            stats.increment("error:no_unified_events");
            stats.log_summary("no_unified_events");
            error!("{}", error_msg);
            let _ = tx_event.send(Err(anyhow!(error_msg)));
            return;
        }

        for unified_response in unified_responses {
            let normalized_responses = normalizer.normalize_response(unified_response);
            if normalized_responses.is_empty() {
                continue;
            }

            for normalized_response in normalized_responses {
                timeout_controller.observe_unified_response(&normalized_response);
                if normalized_response.finish_reason.is_some() {
                    received_finish_reason = true;
                }
                stats.record_unified_response(&normalized_response);
                let _ = tx_event.send(Ok(normalized_response));
            }
        }
    }
}

/// Unwraps the Qoder gateway SSE envelope `{"headers":{...},"body":"<inner>",
/// "statusCode":"OK","statusCodeValue":200}` into the inner `body` string
/// (a standard OpenAI SSE chunk or `[DONE]`). Returns `None` for events that
/// carry no `body` field (e.g. the terminal `event:finish` duration summary).
fn unwrap_qoder_envelope(raw: &str) -> Option<String> {
    let envelope: Value = serde_json::from_str(raw).ok()?;
    let body = envelope.get("body")?;
    match body {
        Value::String(inner) => Some(inner.clone()),
        // Some gateways emit the body as a raw JSON object (non-chunk shapes);
        // keep it as-is so the standard parser can classify it.
        _ => Some(body.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        extract_sse_api_error, extract_sse_api_error_message, is_valid_chat_completion_chunk_weak,
        unwrap_qoder_envelope,
    };
    use bitfun_core_types::errors::ErrorCategory;
    use serde_json::Value;

    #[test]
    fn weak_filter_accepts_chat_completion_chunk() {
        let event = serde_json::json!({
            "object": "chat.completion.chunk"
        });
        assert!(is_valid_chat_completion_chunk_weak(&event));
    }

    #[test]
    fn weak_filter_rejects_non_standard_object() {
        let event = serde_json::json!({
            "object": ""
        });
        assert!(!is_valid_chat_completion_chunk_weak(&event));
    }

    #[test]
    fn weak_filter_rejects_missing_object() {
        let event = serde_json::json!({
            "id": "chatcmpl_test"
        });
        assert!(!is_valid_chat_completion_chunk_weak(&event));
    }

    #[test]
    fn weak_filter_accepts_minimax_final_chat_completion_object() {
        // MiniMax's last SSE frame uses `chat.completion` (non-streaming shape)
        // instead of `chat.completion.chunk`. That frame carries the only
        // authoritative usage block, so it must NOT be dropped at the gate.
        let event = serde_json::json!({
            "object": "chat.completion",
            "choices": [{"finish_reason": "stop", "index": 0, "message": {}}],
            "usage": {"prompt_tokens": 45, "completion_tokens": 47, "total_tokens": 92}
        });
        assert!(is_valid_chat_completion_chunk_weak(&event));
    }

    #[test]
    fn extracts_api_error_message_from_object_shape() {
        let event = serde_json::json!({
            "error": {
                "message": "provider error"
            }
        });
        assert_eq!(
            extract_sse_api_error_message(&event).as_deref(),
            Some("provider error")
        );
    }

    #[test]
    fn preserves_context_overflow_code_from_stream_error() {
        let event = serde_json::json!({
            "error": {
                "code": "context_length_exceeded",
                "message": "Request failed"
            }
        });

        let error = extract_sse_api_error(&event).expect("provider error");
        assert_eq!(error.category, ErrorCategory::ContextOverflow);
        assert_eq!(
            error.provider_code.as_deref(),
            Some("context_length_exceeded")
        );
    }

    #[test]
    fn extracts_api_error_message_from_string_shape() {
        let event = serde_json::json!({
            "error": "provider error"
        });
        assert_eq!(
            extract_sse_api_error_message(&event).as_deref(),
            Some("provider error")
        );
    }

    #[test]
    fn returns_none_when_no_error_payload_exists() {
        let event = serde_json::json!({
            "object": "chat.completion.chunk"
        });
        assert!(extract_sse_api_error_message(&event).is_none());
    }

    #[test]
    fn qoder_envelope_unwraps_inner_openai_chunk() {
        let envelope = serde_json::json!({
            "headers": {"Content-Type": ["application/json"]},
            "body": "{\"choices\":[{\"delta\":{\"content\":\"hi\"},\"index\":0}],\"object\":\"chat.completion.chunk\"}",
            "statusCode": "OK",
            "statusCodeValue": 200
        })
        .to_string();
        let inner = unwrap_qoder_envelope(&envelope).expect("envelope unwraps");
        let parsed: Value = serde_json::from_str(&inner).expect("inner is valid JSON");
        assert_eq!(parsed["object"], "chat.completion.chunk");
    }

    #[test]
    fn qoder_envelope_unwraps_done_marker() {
        let envelope = serde_json::json!({
            "headers": {"Content-Type": ["application/json"]},
            "body": "[DONE]",
            "statusCode": "OK",
            "statusCodeValue": 200
        })
        .to_string();
        assert_eq!(unwrap_qoder_envelope(&envelope).as_deref(), Some("[DONE]"));
    }

    #[test]
    fn qoder_finish_event_without_body_is_skipped() {
        // Terminal `event:finish` duration summary has no body field.
        let finish = r#"{"firstTokenDuration":788,"totalDuration":2025,"serverDuration":40}"#;
        assert!(unwrap_qoder_envelope(finish).is_none());
    }
}
