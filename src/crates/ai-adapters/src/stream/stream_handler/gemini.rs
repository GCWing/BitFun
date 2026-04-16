use super::stream_stats::StreamStats;
use super::{next_stream_item, TimedStreamItem};
use crate::stream::types::gemini::GeminiSSEData;
use crate::stream::types::unified::UnifiedResponse;
use anyhow::{anyhow, Result};
use eventsource_stream::Eventsource;
use log::{error, trace};
use reqwest::Response;
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;

const AI_STREAM_RESPONSE_TARGET: &str = "ai::gemini_stream_response";

static GEMINI_STREAM_ID_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
struct GeminiToolCallState {
    active_name: Option<String>,
    active_id: Option<String>,
    stream_id: u64,
    next_index: usize,
}

impl GeminiToolCallState {
    fn new() -> Self {
        Self {
            active_name: None,
            active_id: None,
            stream_id: GEMINI_STREAM_ID_SEQ.fetch_add(1, Ordering::Relaxed),
            next_index: 0,
        }
    }

    fn on_non_tool_response(&mut self) {
        self.active_name = None;
        self.active_id = None;
    }

    fn assign_id(&mut self, tool_call: &mut crate::stream::types::unified::UnifiedToolCall) {
        if let Some(existing_id) = tool_call.id.as_ref().filter(|value| !value.is_empty()) {
            self.active_id = Some(existing_id.clone());
            self.active_name = tool_call.name.clone().filter(|value| !value.is_empty());
            return;
        }

        let tool_name = tool_call.name.clone().filter(|value| !value.is_empty());
        let is_same_active_call = self.active_id.is_some() && self.active_name == tool_name;

        if is_same_active_call {
            tool_call.id = None;
            return;
        }

        self.next_index += 1;
        let generated_id = format!("gemini_call_{}_{}", self.stream_id, self.next_index);
        tool_call.id = Some(generated_id.clone());
        self.active_id = Some(generated_id);
        self.active_name = tool_name;
    }
}

fn extract_api_error_message(event_json: &Value) -> Option<String> {
    let error = event_json.get("error")?;
    if let Some(message) = error.get("message").and_then(Value::as_str) {
        return Some(message.to_string());
    }
    if let Some(message) = error.as_str() {
        return Some(message.to_string());
    }
    Some("Gemini streaming request failed".to_string())
}

pub async fn handle_gemini_stream(
    response: Response,
    tx_event: mpsc::UnboundedSender<Result<UnifiedResponse>>,
    tx_raw_sse: Option<mpsc::UnboundedSender<String>>,
    idle_timeout: Option<Duration>,
) {
    let mut stream = response.bytes_stream().eventsource();
    let mut received_finish_reason = false;
    let mut tool_call_state = GeminiToolCallState::new();
    let mut stats = StreamStats::new("Gemini");

    loop {
        let sse = match next_stream_item(&mut stream, idle_timeout).await {
            TimedStreamItem::Item(Ok(sse)) => sse,
            TimedStreamItem::End => {
                if received_finish_reason {
                    stats.log_summary("stream_closed_after_finish_reason");
                    return;
                }
                let error_msg = "Gemini SSE stream closed before response completed";
                stats.log_summary("stream_closed_before_completion");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            TimedStreamItem::Item(Err(e)) => {
                let error_msg = format!("Gemini SSE stream error: {}", e);
                stats.log_summary("sse_stream_error");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            TimedStreamItem::TimedOut => {
                let timeout_secs = idle_timeout.map(|timeout| timeout.as_secs()).unwrap_or(0);
                let error_msg = format!("Gemini SSE stream timeout after {}s", timeout_secs);
                stats.log_summary("sse_stream_timeout");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        let raw = sse.data;
        stats.record_sse_event("data");
        trace!(target: AI_STREAM_RESPONSE_TARGET, "Gemini SSE: {:?}", raw);

        if let Some(ref tx) = tx_raw_sse {
            let _ = tx.send(raw.clone());
        }

        if raw == "[DONE]" {
            stats.increment("marker:done");
            stats.log_summary("done_marker_received");
            return;
        }

        let event_json: Value = match serde_json::from_str(&raw) {
            Ok(json) => json,
            Err(e) => {
                let error_msg = format!("Gemini SSE parsing error: {}, data: {}", e, raw);
                stats.increment("error:sse_parsing");
                stats.log_summary("sse_parsing_error");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        if let Some(message) = extract_api_error_message(&event_json) {
            let error_msg = format!("Gemini SSE API error: {}, data: {}", message, raw);
            stats.increment("error:api");
            stats.log_summary("sse_api_error");
            error!("{}", error_msg);
            let _ = tx_event.send(Err(anyhow!(error_msg)));
            return;
        }

        let sse_data: GeminiSSEData = match serde_json::from_value(event_json) {
            Ok(data) => data,
            Err(e) => {
                let error_msg = format!("Gemini SSE data schema error: {}, data: {}", e, raw);
                stats.increment("error:schema");
                stats.log_summary("sse_data_schema_error");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        let mut unified_responses = sse_data.into_unified_responses();
        if unified_responses.is_empty() {
            stats.increment("skip:empty_unified_responses");
        }
        for unified_response in &mut unified_responses {
            if let Some(tool_call) = unified_response.tool_call.as_mut() {
                tool_call_state.assign_id(tool_call);
            } else {
                tool_call_state.on_non_tool_response();
            }

            if unified_response.finish_reason.is_some() {
                received_finish_reason = true;
                tool_call_state.on_non_tool_response();
            }
        }

        trace!(
            target: AI_STREAM_RESPONSE_TARGET,
            "Gemini unified responses: {:?}",
            unified_responses
        );

        for unified_response in unified_responses {
            stats.record_unified_response(&unified_response);
            let _ = tx_event.send(Ok(unified_response));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GeminiToolCallState;
    use crate::stream::types::unified::UnifiedToolCall;

    #[test]
    fn reuses_active_tool_id_by_omitting_follow_up_ids() {
        let mut state = GeminiToolCallState::new();

        let mut first = UnifiedToolCall {
            tool_call_index: None,
            id: None,
            name: Some("get_weather".to_string()),
            arguments: Some("{\"city\":".to_string()),
            arguments_is_snapshot: false,
        };
        state.assign_id(&mut first);

        let mut second = UnifiedToolCall {
            tool_call_index: None,
            id: None,
            name: Some("get_weather".to_string()),
            arguments: Some("\"Paris\"}".to_string()),
            arguments_is_snapshot: false,
        };
        state.assign_id(&mut second);

        assert!(first
            .id
            .as_deref()
            .is_some_and(|id| id.starts_with("gemini_call_")));
        assert!(second.id.is_none());
    }

    #[test]
    fn clears_active_tool_after_non_tool_response() {
        let mut state = GeminiToolCallState::new();

        let mut first = UnifiedToolCall {
            tool_call_index: None,
            id: None,
            name: Some("get_weather".to_string()),
            arguments: Some("{}".to_string()),
            arguments_is_snapshot: false,
        };
        state.assign_id(&mut first);
        state.on_non_tool_response();

        let mut second = UnifiedToolCall {
            tool_call_index: None,
            id: None,
            name: Some("get_weather".to_string()),
            arguments: Some("{}".to_string()),
            arguments_is_snapshot: false,
        };
        state.assign_id(&mut second);

        let first_id = first.id.expect("first id");
        let second_id = second.id.expect("second id");
        assert!(first_id.starts_with("gemini_call_"));
        assert!(second_id.starts_with("gemini_call_"));
        assert_ne!(first_id, second_id);
    }

    #[test]
    fn generates_unique_prefixes_across_streams() {
        let mut first_state = GeminiToolCallState::new();
        let mut second_state = GeminiToolCallState::new();

        let mut first = UnifiedToolCall {
            tool_call_index: None,
            id: None,
            name: Some("grep".to_string()),
            arguments: Some("{}".to_string()),
            arguments_is_snapshot: false,
        };
        let mut second = UnifiedToolCall {
            tool_call_index: None,
            id: None,
            name: Some("read".to_string()),
            arguments: Some("{}".to_string()),
            arguments_is_snapshot: false,
        };

        first_state.assign_id(&mut first);
        second_state.assign_id(&mut second);

        assert_ne!(first.id, second.id);
    }
}
