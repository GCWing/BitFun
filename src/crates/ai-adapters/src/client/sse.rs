use crate::client::utils::elapsed_ms_u64;
use crate::client::StreamResponse;
use crate::stream::UnifiedResponse;
use anyhow::{anyhow, Result};
use log::{debug, error, warn};
use tokio::sync::mpsc;

pub(crate) async fn execute_sse_request<BuildRequest, SpawnHandler>(
    label: &str,
    _url: &str,
    request_body: &serde_json::Value,
    max_tries: usize,
    build_request: BuildRequest,
    spawn_handler: SpawnHandler,
) -> Result<StreamResponse>
where
    BuildRequest: Fn() -> reqwest::RequestBuilder,
    SpawnHandler: Fn(
        reqwest::Response,
        mpsc::UnboundedSender<Result<UnifiedResponse>>,
        Option<mpsc::UnboundedSender<String>>,
    ),
{
    let mut last_error = None;
    let base_wait_time_ms = 500;

    for attempt in 0..max_tries {
        let request_start_time = std::time::Instant::now();
        let response_result = build_request().json(request_body).send().await;

        let response = match response_result {
            Ok(resp) => {
                let connect_time = elapsed_ms_u64(request_start_time);
                let status = resp.status();

                if status.is_client_error() {
                    let error_text = resp
                        .text()
                        .await
                        .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
                    error!("{} client error {}: {}", label, status, error_text);
                    return Err(anyhow!("{} client error {}: {}", label, status, error_text));
                }

                if status.is_success() {
                    debug!(
                        "{} request connected: {}ms, status: {}, attempt: {}/{}",
                        label,
                        connect_time,
                        status,
                        attempt + 1,
                        max_tries
                    );
                    resp
                } else {
                    let error_text = resp
                        .text()
                        .await
                        .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
                    let error = anyhow!("{} error {}: {}", label, status, error_text);
                    warn!(
                        "{} request failed: {}ms, attempt {}/{}, error: {}",
                        label,
                        connect_time,
                        attempt + 1,
                        max_tries,
                        error
                    );
                    last_error = Some(error);

                    if attempt < max_tries - 1 {
                        let delay_ms = base_wait_time_ms * (1 << attempt.min(3));
                        debug!(
                            "Retrying {} after {}ms (attempt {})",
                            label,
                            delay_ms,
                            attempt + 2
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    }
                    continue;
                }
            }
            Err(e) => {
                let connect_time = request_start_time.elapsed().as_millis();
                let error = anyhow!("{} connection failed: {}", label, e);
                warn!(
                    "{} connection failed: {}ms, attempt {}/{}, error: {}",
                    label,
                    connect_time,
                    attempt + 1,
                    max_tries,
                    e
                );
                last_error = Some(error);

                if attempt < max_tries - 1 {
                    let delay_ms = base_wait_time_ms * (1 << attempt.min(3));
                    debug!(
                        "Retrying {} after {}ms (attempt {})",
                        label,
                        delay_ms,
                        attempt + 2
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                continue;
            }
        };

        let (tx, rx) = mpsc::unbounded_channel();
        let (tx_raw, rx_raw) = mpsc::unbounded_channel();
        spawn_handler(response, tx, Some(tx_raw));

        return Ok(StreamResponse {
            stream: Box::pin(tokio_stream::wrappers::UnboundedReceiverStream::new(rx)),
            raw_sse_rx: Some(rx_raw),
        });
    }

    let error_msg = format!(
        "{} failed after {} attempts: {}",
        label,
        max_tries,
        last_error.unwrap_or_else(|| anyhow!("Unknown error"))
    );
    error!("{}", error_msg);
    Err(anyhow!(error_msg))
}
