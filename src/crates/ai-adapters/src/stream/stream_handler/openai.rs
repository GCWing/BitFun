use super::inline_think::InlineThinkParser;
use super::stream_stats::StreamStats;
use crate::stream::types::openai::{OpenAISSEData, OpenAIToolCallArgumentsNormalizer};
use crate::stream::types::unified::UnifiedResponse;
use anyhow::{anyhow, Result};
use eventsource_stream::Eventsource;
use futures::StreamExt;
use log::{error, trace, warn};
use reqwest::Response;
use serde_json::Value;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

const OPENAI_CHAT_COMPLETION_CHUNK_OBJECT: &str = "chat.completion.chunk";
const AI_STREAM_RESPONSE_TARGET: &str = "ai::openai_stream_response";

#[derive(Debug)]
struct OpenAIResponseNormalizer {
    tool_arguments_normalizer: OpenAIToolCallArgumentsNormalizer,
    inline_think_parser: InlineThinkParser,
}

impl OpenAIResponseNormalizer {
    fn new(inline_think_in_text: bool) -> Self {
        Self {
            tool_arguments_normalizer: OpenAIToolCallArgumentsNormalizer::default(),
            inline_think_parser: InlineThinkParser::new(inline_think_in_text),
        }
    }

    fn normalize_sse_data(&mut self, sse_data: &mut OpenAISSEData) {
        sse_data.normalize_tool_call_arguments(&mut self.tool_arguments_normalizer);
    }

    fn normalize_response(&mut self, response: UnifiedResponse) -> Vec<UnifiedResponse> {
        self.inline_think_parser.normalize_response(response)
    }

    fn flush(&mut self) -> Vec<UnifiedResponse> {
        self.inline_think_parser.flush()
    }
}

fn is_valid_chat_completion_chunk_weak(event_json: &Value) -> bool {
    matches!(
        event_json.get("object").and_then(|value| value.as_str()),
        Some(OPENAI_CHAT_COMPLETION_CHUNK_OBJECT)
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

/// Convert a byte stream into a structured response stream
///
/// # Arguments
/// * `response` - HTTP response
/// * `tx_event` - parsed event sender
/// * `tx_raw_sse` - optional raw SSE sender (collect raw data for diagnostics)
pub async fn handle_openai_stream(
    response: Response,
    tx_event: mpsc::UnboundedSender<Result<UnifiedResponse>>,
    tx_raw_sse: Option<mpsc::UnboundedSender<String>>,
    inline_think_in_text: bool,
) {
    let mut stream = response.bytes_stream().eventsource();
    let idle_timeout = Duration::from_secs(600);
    let mut stats = StreamStats::new("OpenAI");
    // Track whether a chunk with `finish_reason` was received.
    // Some providers (e.g. MiniMax) close the stream after the final chunk
    // without sending `[DONE]`, so we treat `Ok(None)` as a normal termination
    // when a finish_reason has already been seen.
    let mut received_finish_reason = false;
    let mut normalizer = OpenAIResponseNormalizer::new(inline_think_in_text);

    loop {
        let sse_event = timeout(idle_timeout, stream.next()).await;
        let sse = match sse_event {
            Ok(Some(Ok(sse))) => sse,
            Ok(None) => {
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
            Ok(Some(Err(e))) => {
                let error_msg = format!("SSE stream error: {}", e);
                stats.log_summary("sse_stream_error");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            Err(_) => {
                let error_msg = format!("SSE stream timeout after {}s", idle_timeout.as_secs());
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
                let error_msg = format!("SSE parsing error: {}, data: {}", e, &raw);
                stats.increment("error:sse_parsing");
                stats.log_summary("sse_parsing_error");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        if let Some(api_error_message) = extract_sse_api_error_message(&event_json) {
            let error_msg = format!("SSE API error: {}, data: {}", api_error_message, raw);
            stats.increment("error:api");
            stats.log_summary("sse_api_error");
            error!("{}", error_msg);
            let _ = tx_event.send(Err(anyhow!(error_msg)));
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
        let mut sse_data: OpenAISSEData = match serde_json::from_value(event_json) {
            Ok(event) => event,
            Err(e) => {
                let error_msg = format!("SSE data schema error: {}, data: {}", e, &raw);
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

        normalizer.normalize_sse_data(&mut sse_data);

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
                if normalized_response.finish_reason.is_some() {
                    received_finish_reason = true;
                }
                stats.record_unified_response(&normalized_response);
                let _ = tx_event.send(Ok(normalized_response));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_sse_api_error_message, is_valid_chat_completion_chunk_weak};

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
}
