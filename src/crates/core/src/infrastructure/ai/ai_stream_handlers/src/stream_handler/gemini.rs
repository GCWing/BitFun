use crate::types::gemini::GeminiSSEData;
use crate::types::unified::UnifiedResponse;
use anyhow::{anyhow, Result};
use eventsource_stream::Eventsource;
use futures::StreamExt;
use log::{error, trace};
use reqwest::Response;
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

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

    fn assign_id(&mut self, tool_call: &mut crate::types::unified::UnifiedToolCall) {
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
) {
    let mut stream = response.bytes_stream().eventsource();
    let idle_timeout = Duration::from_secs(600);
    let mut received_finish_reason = false;
    let mut tool_call_state = GeminiToolCallState::new();

    loop {
        let sse_event = timeout(idle_timeout, stream.next()).await;
        let sse = match sse_event {
            Ok(Some(Ok(sse))) => sse,
            Ok(None) => {
                if received_finish_reason {
                    return;
                }
                let error_msg = "Gemini SSE stream closed before response completed";
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            Ok(Some(Err(e))) => {
                let error_msg = format!("Gemini SSE stream error: {}", e);
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            Err(_) => {
                let error_msg = format!(
                    "Gemini SSE stream timeout after {}s",
                    idle_timeout.as_secs()
                );
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        let raw = sse.data;
        trace!("Gemini SSE: {:?}", raw);

        if let Some(ref tx) = tx_raw_sse {
            let _ = tx.send(raw.clone());
        }

        if raw == "[DONE]" {
            return;
        }

        let event_json: Value = match serde_json::from_str(&raw) {
            Ok(json) => json,
            Err(e) => {
                let error_msg = format!("Gemini SSE parsing error: {}, data: {}", e, raw);
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        if let Some(message) = extract_api_error_message(&event_json) {
            let error_msg = format!("Gemini SSE API error: {}, data: {}", message, raw);
            error!("{}", error_msg);
            let _ = tx_event.send(Err(anyhow!(error_msg)));
            return;
        }

        let sse_data: GeminiSSEData = match serde_json::from_value(event_json) {
            Ok(data) => data,
            Err(e) => {
                let error_msg = format!("Gemini SSE data schema error: {}, data: {}", e, raw);
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        let mut unified_responses = sse_data.into_unified_responses();
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

        for unified_response in unified_responses {
            let _ = tx_event.send(Ok(unified_response));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GeminiToolCallState;
    use crate::types::unified::UnifiedToolCall;

    #[test]
    fn reuses_active_tool_id_by_omitting_follow_up_ids() {
        let mut state = GeminiToolCallState::new();

        let mut first = UnifiedToolCall {
            id: None,
            name: Some("get_weather".to_string()),
            arguments: Some("{\"city\":".to_string()),
        };
        state.assign_id(&mut first);

        let mut second = UnifiedToolCall {
            id: None,
            name: Some("get_weather".to_string()),
            arguments: Some("\"Paris\"}".to_string()),
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
            id: None,
            name: Some("get_weather".to_string()),
            arguments: Some("{}".to_string()),
        };
        state.assign_id(&mut first);
        state.on_non_tool_response();

        let mut second = UnifiedToolCall {
            id: None,
            name: Some("get_weather".to_string()),
            arguments: Some("{}".to_string()),
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
            id: None,
            name: Some("grep".to_string()),
            arguments: Some("{}".to_string()),
        };
        let mut second = UnifiedToolCall {
            id: None,
            name: Some("read".to_string()),
            arguments: Some("{}".to_string()),
        };

        first_state.assign_id(&mut first);
        second_state.assign_id(&mut second);

        assert_ne!(first.id, second.id);
    }
}
