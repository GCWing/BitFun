use crate::client::utils::elapsed_ms_u64;
use crate::client::StreamResponse;
use crate::stream::UnifiedResponse;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use log::{debug, error, warn};
use reqwest::{
    header::{HeaderMap, RETRY_AFTER},
    StatusCode,
};
use std::time::Duration;
use tokio::sync::mpsc;

const BASE_RETRY_DELAY_MS: u64 = 500;
const MAX_RETRY_AFTER_DELAY_MS: u64 = 10_000;

enum StreamSendOutcome {
    Response(reqwest::Response),
    Transport(reqwest::Error),
    TtftTimeout,
}

async fn send_stream_request<BuildRequest>(
    build_request: BuildRequest,
    request_body: &serde_json::Value,
    ttft_timeout: Option<Duration>,
) -> StreamSendOutcome
where
    BuildRequest: Fn() -> reqwest::RequestBuilder,
{
    match ttft_timeout {
        Some(timeout) => {
            match tokio::time::timeout(timeout, build_request().json(request_body).send()).await {
                Ok(Ok(response)) => StreamSendOutcome::Response(response),
                Ok(Err(error)) => StreamSendOutcome::Transport(error),
                Err(_) => StreamSendOutcome::TtftTimeout,
            }
        }
        None => match build_request().json(request_body).send().await {
            Ok(response) => StreamSendOutcome::Response(response),
            Err(error) => StreamSendOutcome::Transport(error),
        },
    }
}

fn format_ttft_timeout_error(label: &str, ttft_timeout: Option<Duration>) -> String {
    let timeout_secs = ttft_timeout.map(|timeout| timeout.as_secs()).unwrap_or(0);
    format!(
        "{} TTFT timeout after {}s waiting for response headers",
        label, timeout_secs
    )
}

fn format_transport_error(label: &str, error: &reqwest::Error) -> String {
    format!("{} connection failed: {}", label, error)
}

fn is_retryable_http_status(status: StatusCode) -> bool {
    status.is_server_error() || matches!(status.as_u16(), 408 | 409 | 425 | 429)
}

fn exponential_retry_delay_ms(attempt: usize) -> u64 {
    BASE_RETRY_DELAY_MS * (1 << attempt.min(3))
}

fn retry_after_delay_ms(headers: &HeaderMap) -> Option<u64> {
    let value = headers.get(RETRY_AFTER)?.to_str().ok()?.trim();

    if let Ok(seconds) = value.parse::<u64>() {
        return Some(seconds.saturating_mul(1000).min(MAX_RETRY_AFTER_DELAY_MS));
    }

    let retry_at = DateTime::parse_from_rfc2822(value)
        .ok()?
        .with_timezone(&Utc);
    let now = Utc::now();
    if retry_at <= now {
        return Some(0);
    }

    Some(
        retry_at
            .signed_duration_since(now)
            .num_milliseconds()
            .max(0) as u64,
    )
    .map(|delay| delay.min(MAX_RETRY_AFTER_DELAY_MS))
}

fn retry_delay_ms(attempt: usize, headers: &HeaderMap) -> u64 {
    retry_after_delay_ms(headers).unwrap_or_else(|| exponential_retry_delay_ms(attempt))
}

pub(crate) async fn execute_sse_request<BuildRequest, SpawnHandler>(
    label: &str,
    _url: &str,
    request_body: &serde_json::Value,
    max_tries: usize,
    ttft_timeout: Option<Duration>,
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
    for attempt in 0..max_tries {
        let request_start_time = std::time::Instant::now();
        let send_outcome = send_stream_request(&build_request, request_body, ttft_timeout).await;

        let response = match send_outcome {
            StreamSendOutcome::Response(resp) => {
                let connect_time = elapsed_ms_u64(request_start_time);
                let status = resp.status();
                let headers = resp.headers().clone();

                if status.is_client_error() && !is_retryable_http_status(status) {
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
                        let delay_ms = retry_delay_ms(attempt, &headers);
                        debug!(
                            "Retrying {} after {}ms (attempt {}, status {})",
                            label,
                            delay_ms,
                            attempt + 2,
                            status
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    }
                    continue;
                }
            }
            StreamSendOutcome::Transport(e) => {
                let connect_time = request_start_time.elapsed().as_millis();
                let error_msg = format_transport_error(label, &e);
                let error = anyhow!("{}", error_msg);
                warn!(
                    "{} request failed: {}ms, attempt {}/{}, error: {}",
                    label,
                    connect_time,
                    attempt + 1,
                    max_tries,
                    error_msg
                );
                last_error = Some(error);

                if attempt < max_tries - 1 {
                    let delay_ms = exponential_retry_delay_ms(attempt);
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
            StreamSendOutcome::TtftTimeout => {
                let connect_time = request_start_time.elapsed().as_millis();
                let error_msg = format_ttft_timeout_error(label, ttft_timeout);
                let error = anyhow!("{}", error_msg);
                warn!(
                    "{} request failed: {}ms, attempt {}/{}, error: {}",
                    label,
                    connect_time,
                    attempt + 1,
                    max_tries,
                    error_msg
                );
                last_error = Some(error);

                if attempt < max_tries - 1 {
                    let delay_ms = exponential_retry_delay_ms(attempt);
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

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn format_ttft_timeout_error_includes_timeout_seconds() {
        let message = format_ttft_timeout_error(
            "Codex ChatGPT Responses API",
            Some(std::time::Duration::from_secs(30)),
        );

        assert!(message.contains("TTFT timeout after 30s"));
    }

    #[test]
    fn retryable_http_statuses_include_rate_limit_and_server_errors() {
        assert!(is_retryable_http_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable_http_status(StatusCode::REQUEST_TIMEOUT));
        assert!(is_retryable_http_status(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(is_retryable_http_status(StatusCode::BAD_GATEWAY));

        assert!(!is_retryable_http_status(StatusCode::UNAUTHORIZED));
        assert!(!is_retryable_http_status(StatusCode::BAD_REQUEST));
        assert!(!is_retryable_http_status(StatusCode::NOT_FOUND));
    }

    #[test]
    fn retry_after_seconds_is_capped() {
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, HeaderValue::from_static("120"));

        assert_eq!(
            retry_after_delay_ms(&headers),
            Some(MAX_RETRY_AFTER_DELAY_MS)
        );
    }

    #[test]
    fn retry_delay_falls_back_to_exponential_backoff() {
        let headers = HeaderMap::new();

        assert_eq!(retry_delay_ms(0, &headers), 500);
        assert_eq!(retry_delay_ms(1, &headers), 1000);
        assert_eq!(retry_delay_ms(4, &headers), 4000);
    }
}
