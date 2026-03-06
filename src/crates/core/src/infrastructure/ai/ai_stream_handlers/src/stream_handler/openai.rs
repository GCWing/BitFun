use crate::types::openai::OpenAISSEData;
use crate::types::unified::UnifiedResponse;
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
) {
    let mut stream = response.bytes_stream().eventsource();
    let idle_timeout = Duration::from_secs(600);
    // Track whether a chunk with `finish_reason` was received.
    // Some providers (e.g. MiniMax) close the stream after the final chunk
    // without sending `[DONE]`, so we treat `Ok(None)` as a normal termination
    // when a finish_reason has already been seen.
    let mut received_finish_reason = false;

    loop {
        let sse_event = timeout(idle_timeout, stream.next()).await;
        let sse = match sse_event {
            Ok(Some(Ok(sse))) => sse,
            Ok(None) => {
                if received_finish_reason {
                    return;
                }
                let error_msg = "SSE stream closed before response completed";
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            Ok(Some(Err(e))) => {
                let error_msg = format!("SSE stream error: {}", e);
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            Err(_) => {
                let error_msg = format!("SSE stream timeout after {}s", idle_timeout.as_secs());
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        let raw = sse.data;
        trace!("OpenAI SSE: {:?}", raw);
        if let Some(ref tx) = tx_raw_sse {
            let _ = tx.send(raw.clone());
        }
        if raw == "[DONE]" {
            return;
        }

        let event_json: Value = match serde_json::from_str(&raw) {
            Ok(json) => json,
            Err(e) => {
                let error_msg = format!("SSE parsing error: {}, data: {}", e, &raw);
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        if let Some(api_error_message) = extract_sse_api_error_message(&event_json) {
            let error_msg = format!("SSE API error: {}, data: {}", api_error_message, raw);
            error!("{}", error_msg);
            let _ = tx_event.send(Err(anyhow!(error_msg)));
            return;
        }

        if !is_valid_chat_completion_chunk_weak(&event_json) {
            warn!(
                "Skipping non-standard OpenAI SSE event; object={}",
                event_json
                    .get("object")
                    .and_then(|value| value.as_str())
                    .unwrap_or("<missing>")
            );
            continue;
        }

        let sse_data: OpenAISSEData = match serde_json::from_value(event_json) {
            Ok(event) => event,
            Err(e) => {
                let error_msg = format!("SSE data schema error: {}, data: {}", e, &raw);
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        let tool_call_count = sse_data.first_choice_tool_call_count();
        if tool_call_count > 1 {
            warn!(
                "OpenAI SSE chunk contains {} tool calls in the first choice; splitting and sending sequentially",
                tool_call_count
            );
        }

        let has_empty_choices = sse_data.is_choices_empty();
        let unified_responses = sse_data.into_unified_responses();
        trace!("OpenAI unified responses: {:?}", unified_responses);
        if unified_responses.is_empty() {
            if has_empty_choices {
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
            error!("{}", error_msg);
            let _ = tx_event.send(Err(anyhow!(error_msg)));
            return;
        }

        for unified_response in unified_responses {
            if unified_response.finish_reason.is_some() {
                received_finish_reason = true;
            }
            let _ = tx_event.send(Ok(unified_response));
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
