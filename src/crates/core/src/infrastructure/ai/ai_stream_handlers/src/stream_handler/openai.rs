use crate::types::openai::OpenAISSEData;
use crate::types::unified::UnifiedResponse;
use anyhow::{anyhow, Error, Result};
use eventsource_stream::Eventsource;
use futures::StreamExt;
use log::{trace, warn};
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
pub async fn handle_openai_stream(
    response: Response,
    tx_event: mpsc::UnboundedSender<Result<UnifiedResponse>>,
    tx_raw_sse: Option<mpsc::UnboundedSender<String>>,
) {
    let mut stream = response.bytes_stream().eventsource();
    let idle_timeout = Duration::from_secs(600);
    let mut received_done = false;
    let response_error: Option<Error> = None;

    loop {
        let sse_event = timeout(idle_timeout, stream.next()).await;
        let sse = match sse_event {
            Ok(Some(Ok(sse))) => sse,
            Ok(None) => {
                if !received_done {
                    let error = response_error
                        .unwrap_or(anyhow!("SSE stream closed before response completed"));
                    warn!("SSE stream ended unexpectedly: {}", error);
                    let _ = tx_event.send(Err(error));
                }
                return;
            }
            Ok(Some(Err(e))) => {
                let error = anyhow!("SSE stream error: {}", e);
                let _ = tx_event.send(Err(error));
                return;
            }
            Err(_) => {
                warn!("SSE stream timeout after {}s", idle_timeout.as_secs());
                let _ = tx_event.send(Err(anyhow!(
                    "SSE stream timeout: idle timeout waiting for SSE"
                )));
                return;
            }
        };

        let raw = sse.data;
        trace!("sse data: {:?}", raw);

        if let Some(ref tx) = tx_raw_sse {
            let _ = tx.send(raw.clone());
        }

        if raw == "[DONE]" {
            received_done = true;
            continue;
        }

        let sse_data: OpenAISSEData = match serde_json::from_str(&raw) {
            Ok(event) => event,
            Err(e) => {
                let _ = tx_event.send(Err(anyhow!("SSE parsing error: {}, data: {}", e, &raw)));
                continue;
            }
        };
        let unified_response: UnifiedResponse = sse_data.into();
        trace!("unified_response: {:?}", unified_response);
        let _ = tx_event.send(Ok(unified_response));
    }
}
