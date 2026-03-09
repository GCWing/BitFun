use crate::types::responses::{
    parse_responses_output_item, ResponsesCompleted, ResponsesDone, ResponsesStreamEvent,
};
use crate::types::unified::UnifiedResponse;
use anyhow::{anyhow, Result};
use eventsource_stream::Eventsource;
use futures::StreamExt;
use log::{error, trace};
use reqwest::Response;
use serde_json::Value;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

fn extract_api_error_message(event_json: &Value) -> Option<String> {
    let response = event_json.get("response")?;
    let error = response.get("error")?;

    if error.is_null() {
        return None;
    }

    if let Some(message) = error.get("message").and_then(Value::as_str) {
        return Some(message.to_string());
    }
    if let Some(message) = error.as_str() {
        return Some(message.to_string());
    }

    Some("An error occurred during responses streaming".to_string())
}

pub async fn handle_responses_stream(
    response: Response,
    tx_event: mpsc::UnboundedSender<Result<UnifiedResponse>>,
    tx_raw_sse: Option<mpsc::UnboundedSender<String>>,
) {
    let mut stream = response.bytes_stream().eventsource();
    let idle_timeout = Duration::from_secs(600);
    let received_completion = false;
    let mut received_text_delta = false;

    loop {
        let sse_event = timeout(idle_timeout, stream.next()).await;
        let sse = match sse_event {
            Ok(Some(Ok(sse))) => sse,
            Ok(None) => {
                if received_completion {
                    return;
                }
                let error_msg = "Responses SSE stream closed before response completed";
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            Ok(Some(Err(e))) => {
                let error_msg = format!("Responses SSE stream error: {}", e);
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            Err(_) => {
                let error_msg = format!(
                    "Responses SSE stream timeout after {}s",
                    idle_timeout.as_secs()
                );
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        let raw = sse.data;
        trace!("Responses SSE: {:?}", raw);
        if let Some(ref tx) = tx_raw_sse {
            let _ = tx.send(raw.clone());
        }
        if raw == "[DONE]" {
            return;
        }

        let event_json: Value = match serde_json::from_str(&raw) {
            Ok(json) => json,
            Err(e) => {
                let error_msg = format!("Responses SSE parsing error: {}, data: {}", e, &raw);
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        if let Some(api_error_message) = extract_api_error_message(&event_json) {
            let error_msg = format!("Responses SSE API error: {}, data: {}", api_error_message, raw);
            error!("{}", error_msg);
            let _ = tx_event.send(Err(anyhow!(error_msg)));
            return;
        }

        let event: ResponsesStreamEvent = match serde_json::from_value(event_json) {
            Ok(event) => event,
            Err(e) => {
                let error_msg = format!("Responses SSE schema error: {}, data: {}", e, &raw);
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        match event.kind.as_str() {
            "response.output_text.delta" => {
                if let Some(delta) = event.delta.filter(|delta| !delta.is_empty()) {
                    received_text_delta = true;
                    let _ = tx_event.send(Ok(UnifiedResponse {
                        text: Some(delta),
                        ..Default::default()
                    }));
                }
            }
            "response.reasoning_text.delta" | "response.reasoning_summary_text.delta" => {
                if let Some(delta) = event.delta.filter(|delta| !delta.is_empty()) {
                    let _ = tx_event.send(Ok(UnifiedResponse {
                        reasoning_content: Some(delta),
                        ..Default::default()
                    }));
                }
            }
            "response.output_item.done" => {
                if let Some(item_value) = event.item {
                    if let Some(mut unified_response) = parse_responses_output_item(item_value) {
                        if received_text_delta && unified_response.text.is_some() {
                            unified_response.text = None;
                        }
                        if unified_response.text.is_some() || unified_response.tool_call.is_some() {
                            let _ = tx_event.send(Ok(unified_response));
                        }
                    }
                }
            }
            "response.completed" => {
                match event.response.map(serde_json::from_value::<ResponsesCompleted>) {
                    Some(Ok(response)) => {
                        let _ = tx_event.send(Ok(UnifiedResponse {
                            usage: response.usage.map(Into::into),
                            finish_reason: Some("stop".to_string()),
                            ..Default::default()
                        }));
                        return;
                    }
                    Some(Err(e)) => {
                        let error_msg = format!("Failed to parse response.completed payload: {}", e);
                        error!("{}", error_msg);
                        let _ = tx_event.send(Err(anyhow!(error_msg)));
                        return;
                    }
                    None => {
                        let _ = tx_event.send(Ok(UnifiedResponse {
                            finish_reason: Some("stop".to_string()),
                            ..Default::default()
                        }));
                        return;
                    }
                }
            }
            "response.done" => {
                match event.response.map(serde_json::from_value::<ResponsesDone>) {
                    Some(Ok(response)) => {
                        let _ = tx_event.send(Ok(UnifiedResponse {
                            usage: response.usage.map(Into::into),
                            finish_reason: Some("stop".to_string()),
                            ..Default::default()
                        }));
                        return;
                    }
                    Some(Err(e)) => {
                        let error_msg = format!("Failed to parse response.done payload: {}", e);
                        error!("{}", error_msg);
                        let _ = tx_event.send(Err(anyhow!(error_msg)));
                        return;
                    }
                    None => {
                        let _ = tx_event.send(Ok(UnifiedResponse {
                            finish_reason: Some("stop".to_string()),
                            ..Default::default()
                        }));
                        return;
                    }
                }
            }
            "response.failed" => {
                let error_msg = event
                    .response
                    .as_ref()
                    .and_then(|response| response.get("error"))
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("Responses API returned response.failed")
                    .to_string();
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            "response.incomplete" => {
                let error_msg = event
                    .response
                    .as_ref()
                    .and_then(|response| response.get("incomplete_details"))
                    .and_then(|details| details.get("reason"))
                    .and_then(Value::as_str)
                    .map(|reason| format!("Incomplete response returned, reason: {}", reason))
                    .unwrap_or_else(|| "Incomplete response returned".to_string());
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::extract_api_error_message;
    use serde_json::json;

    #[test]
    fn extracts_api_error_message_from_response_error() {
        let event = json!({
            "type": "response.failed",
            "response": {
                "error": {
                    "message": "provider error"
                }
            }
        });

        assert_eq!(
            extract_api_error_message(&event).as_deref(),
            Some("provider error")
        );
    }

    #[test]
    fn returns_none_when_no_response_error_exists() {
        let event = json!({
            "type": "response.created",
            "response": {
                "id": "resp_1"
            }
        });

        assert!(extract_api_error_message(&event).is_none());
    }

    #[test]
    fn returns_none_when_response_error_is_null() {
        let event = json!({
            "type": "response.created",
            "response": {
                "id": "resp_1",
                "error": null
            }
        });

        assert!(extract_api_error_message(&event).is_none());
    }
}
