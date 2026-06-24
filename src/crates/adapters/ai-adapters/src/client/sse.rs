use crate::client::utils::elapsed_ms_u64;
use crate::client::StreamResponse;
use crate::stream::UnifiedResponse;
use crate::trace::{ModelExchangeRequestAttempt, ModelExchangeTraceConfig};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use futures::Stream;
use log::{debug, error, warn};
use reqwest::{
    header::{HeaderMap, RETRY_AFTER},
    StatusCode,
};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_util::sync::CancellationToken;

const BASE_RETRY_DELAY_MS: u64 = 500;
/// Maximum delay applied to a `Retry-After` header value.
///
/// Some providers (especially TPM-based rate limits on aggregator platforms
/// like NVIDIA's integrate API) return large `Retry-After` values of 30-60
/// seconds. Capping at 10s caused tight retry loops that burned through the
/// user's request budget without actually waiting for the TPM window to reset.
/// 60s is a reasonable upper bound that respects provider guidance without
/// locking the user into an interminable stall.
const MAX_RETRY_AFTER_DELAY_MS: u64 = 60_000;

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
        "{} TTFT timeout after {}s waiting for first effective stream output",
        label, timeout_secs
    )
}

fn remaining_ttft_timeout(
    started_at: std::time::Instant,
    ttft_timeout: Option<Duration>,
) -> Option<Duration> {
    ttft_timeout.map(|timeout| timeout.saturating_sub(started_at.elapsed()))
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

struct ManagedResponseStream {
    inner: UnboundedReceiverStream<Result<UnifiedResponse>>,
    handler_cancel: CancellationToken,
    handler_task: Option<JoinHandle<()>>,
}

impl ManagedResponseStream {
    fn new(
        rx: mpsc::UnboundedReceiver<Result<UnifiedResponse>>,
        handler_cancel: CancellationToken,
        handler_task: JoinHandle<()>,
    ) -> Self {
        Self {
            inner: UnboundedReceiverStream::new(rx),
            handler_cancel,
            handler_task: Some(handler_task),
        }
    }
}

impl Stream for ManagedResponseStream {
    type Item = Result<UnifiedResponse>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl Drop for ManagedResponseStream {
    fn drop(&mut self) {
        self.handler_cancel.cancel();
        let _ = self.handler_task.take();
    }
}

pub(crate) async fn execute_sse_request<BuildRequest, BuildHandler, HandlerFuture>(
    label: &str,
    url: &str,
    request_body: &serde_json::Value,
    max_tries: usize,
    ttft_timeout: Option<Duration>,
    trace: Option<ModelExchangeTraceConfig>,
    build_request: BuildRequest,
    build_handler: BuildHandler,
) -> Result<StreamResponse>
where
    BuildRequest: Fn() -> reqwest::RequestBuilder,
    BuildHandler: Fn(
        reqwest::Response,
        mpsc::UnboundedSender<Result<UnifiedResponse>>,
        Option<mpsc::UnboundedSender<String>>,
        Option<Duration>,
    ) -> HandlerFuture,
    HandlerFuture: Future<Output = ()> + Send + 'static,
{
    let mut last_error = None;
    for attempt in 0..max_tries {
        let trace_handle = if let Some(trace) = trace.as_ref() {
            trace
                .sink
                .request_attempt_started(&ModelExchangeRequestAttempt {
                    request_url: url.to_string(),
                    request_body: trace.capture_request_body.then(|| request_body.clone()),
                    attempt_number: attempt + 1,
                })
                .await
        } else {
            None
        };
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
                    if let Some(trace) = trace.as_ref() {
                        trace
                            .sink
                            .request_attempt_failed(
                                trace_handle.as_ref(),
                                &format!("{} client error {}: {}", label, status, error_text),
                            )
                            .await;
                    }
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
                    if let Some(trace) = trace.as_ref() {
                        trace
                            .sink
                            .request_attempt_failed(
                                trace_handle.as_ref(),
                                &last_error
                                    .as_ref()
                                    .map(ToString::to_string)
                                    .unwrap_or_else(|| "unknown error".to_string()),
                            )
                            .await;
                    }

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
                if let Some(trace) = trace.as_ref() {
                    trace
                        .sink
                        .request_attempt_failed(trace_handle.as_ref(), &error_msg)
                        .await;
                }

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
                if let Some(trace) = trace.as_ref() {
                    trace
                        .sink
                        .request_attempt_failed(trace_handle.as_ref(), &error_msg)
                        .await;
                }

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
        let remaining_ttft_timeout = remaining_ttft_timeout(request_start_time, ttft_timeout);
        let handler_cancel = CancellationToken::new();
        let handler_cancel_for_task = handler_cancel.clone();
        let handler_future = build_handler(response, tx, Some(tx_raw), remaining_ttft_timeout);
        let handler_task = tokio::spawn(async move {
            tokio::select! {
                _ = handler_cancel_for_task.cancelled() => {}
                _ = handler_future => {}
            }
        });

        return Ok(StreamResponse {
            stream: Box::pin(ManagedResponseStream::new(rx, handler_cancel, handler_task)),
            raw_sse_rx: Some(rx_raw),
            trace_handle,
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
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    #[test]
    fn format_ttft_timeout_error_includes_timeout_seconds() {
        let message = format_ttft_timeout_error(
            "Codex ChatGPT Responses API",
            Some(std::time::Duration::from_secs(30)),
        );

        assert!(message.contains("TTFT timeout after 30s"));
        assert!(message.contains("first effective stream output"));
    }

    #[test]
    fn remaining_ttft_timeout_subtracts_elapsed_request_time() {
        let start = std::time::Instant::now() - Duration::from_secs(2);
        let remaining = remaining_ttft_timeout(start, Some(Duration::from_secs(5)));

        let remaining = remaining.expect("remaining timeout");
        assert!(remaining <= Duration::from_secs(3));
        assert!(remaining > Duration::from_secs(2));
    }

    #[tokio::test]
    async fn managed_response_stream_drop_cancels_handler_task() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let handler_cancel = CancellationToken::new();
        let handler_cancel_for_task = handler_cancel.clone();
        let observed_cancel = Arc::new(AtomicBool::new(false));
        let observed_cancel_for_task = Arc::clone(&observed_cancel);
        let handler_task = tokio::spawn(async move {
            tokio::select! {
                _ = handler_cancel_for_task.cancelled() => {
                    observed_cancel_for_task.store(true, Ordering::SeqCst);
                }
                _ = tokio::time::sleep(Duration::from_secs(60)) => {}
            }
        });

        let stream = ManagedResponseStream::new(rx, handler_cancel, handler_task);
        drop(stream);

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(observed_cancel.load(Ordering::SeqCst));
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
    fn retry_after_preserves_sub_cap_values() {
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, HeaderValue::from_static("45"));

        assert_eq!(retry_after_delay_ms(&headers), Some(45_000));
    }

    #[test]
    fn retry_delay_falls_back_to_exponential_backoff() {
        let headers = HeaderMap::new();

        assert_eq!(retry_delay_ms(0, &headers), 500);
        assert_eq!(retry_delay_ms(1, &headers), 1000);
        assert_eq!(retry_delay_ms(4, &headers), 4000);
    }
}
