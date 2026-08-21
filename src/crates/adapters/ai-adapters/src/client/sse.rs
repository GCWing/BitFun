use crate::client::utils::elapsed_ms_u64;
use crate::client::StreamResponse;
use crate::stream::UnifiedResponse;
use crate::trace::{ModelExchangeRequestAttempt, ModelExchangeTraceConfig};
use anyhow::{anyhow, Result};
use bitfun_core_types::errors::AiProviderError;
use chrono::{DateTime, Utc};
use futures::Stream;
use log::{debug, error, warn};
use reqwest::{
    header::{HeaderMap, RETRY_AFTER},
    StatusCode,
};
use std::error::Error as StdError;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_util::sync::CancellationToken;

const BASE_RETRY_DELAY_MS: u64 = 500;
/// Base delay for HTTP 429 / rate-limit retries when `Retry-After` is absent.
///
/// Providers frequently omit `Retry-After` (or send a useless 1s value). The
/// previous general backoff capped at 4s (`attempt.min(3)`), so 10 attempts
/// finished in ~90s and never waited for TPM / RPM windows to recover —
/// especially painful when multiple subagents retry in parallel.
const RATE_LIMIT_BASE_RETRY_DELAY_MS: u64 = 2_000;
/// Cap for the general (non-429) exponential backoff ladder.
const MAX_EXPONENTIAL_DELAY_MS: u64 = 30_000;
/// Maximum exponent shift applied to retry delays (`2^n` multiplier).
const MAX_RETRY_EXPONENT_SHIFT: u32 = 6;
/// Maximum delay applied to a `Retry-After` header value / rate-limit backoff.
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
    raw_body: Option<Vec<u8>>,
    ttft_timeout: Option<Duration>,
) -> StreamSendOutcome
where
    BuildRequest: Fn() -> reqwest::RequestBuilder,
{
    let finalize = |builder: reqwest::RequestBuilder| match raw_body.clone() {
        // A pre-encoded body (e.g. the wasm-encrypted Qoder payload) replaces
        // the JSON serialization entirely.
        Some(bytes) => builder.body(bytes),
        None => builder.json(request_body),
    };
    match ttft_timeout {
        Some(timeout) => {
            match tokio::time::timeout(timeout, finalize(build_request()).send()).await {
                Ok(Ok(response)) => StreamSendOutcome::Response(response),
                Ok(Err(error)) => StreamSendOutcome::Transport(error),
                Err(_) => StreamSendOutcome::TtftTimeout,
            }
        }
        None => match finalize(build_request()).send().await {
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
    let mut message = format!("{} connection failed: {}", label, error);
    let mut source = error.source();
    let mut index = 1;

    while let Some(cause) = source {
        message.push_str(&format!("; cause {}: {}", index, cause));
        source = cause.source();
        index += 1;
    }

    message
}

fn provider_error_code(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let error = value.get("error").unwrap_or(&value);
    ["code", "type", "status"].iter().find_map(|field| {
        error.get(field).and_then(|value| match value {
            serde_json::Value::String(value) => Some(value.clone()),
            serde_json::Value::Number(value) => Some(value.to_string()),
            _ => None,
        })
    })
}

fn http_provider_error(
    label: &str,
    status: StatusCode,
    error_text: &str,
    error_kind: &str,
    retry_after_ms: Option<u64>,
) -> AiProviderError {
    AiProviderError::from_parts(
        format!("{} {} {}: {}", label, error_kind, status, error_text),
        Some(label.to_string()),
        provider_error_code(error_text),
        Some(status.as_u16()),
    )
    .with_retry_after_ms(retry_after_ms)
}

fn exponential_retry_delay_ms(attempt: usize) -> u64 {
    let shift = u32::try_from(attempt)
        .unwrap_or(u32::MAX)
        .min(MAX_RETRY_EXPONENT_SHIFT);
    BASE_RETRY_DELAY_MS
        .saturating_mul(1u64 << shift)
        .min(MAX_EXPONENTIAL_DELAY_MS)
}

fn rate_limit_retry_delay_ms(attempt: usize) -> u64 {
    let shift = u32::try_from(attempt)
        .unwrap_or(u32::MAX)
        .min(MAX_RETRY_EXPONENT_SHIFT);
    RATE_LIMIT_BASE_RETRY_DELAY_MS
        .saturating_mul(1u64 << shift)
        .min(MAX_RETRY_AFTER_DELAY_MS)
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

fn retry_delay_ms(attempt: usize, headers: &HeaderMap, status: StatusCode) -> u64 {
    let fallback = if status == StatusCode::TOO_MANY_REQUESTS {
        rate_limit_retry_delay_ms(attempt)
    } else {
        exponential_retry_delay_ms(attempt)
    };

    match retry_after_delay_ms(headers) {
        // Honor Retry-After, but never let a tiny/zero value defeat the
        // rate-limit ladder (common with aggregator "Retry-After: 1" responses).
        Some(retry_after) if status == StatusCode::TOO_MANY_REQUESTS => {
            retry_after.max(fallback).min(MAX_RETRY_AFTER_DELAY_MS)
        }
        Some(retry_after) if retry_after > 0 => retry_after,
        Some(_) | None => fallback,
    }
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

#[allow(clippy::too_many_arguments)] // request pipeline entry; grouping would churn all callers
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
    execute_sse_request_with_raw_body(
        label,
        url,
        request_body,
        None,
        max_tries,
        ttft_timeout,
        trace,
        build_request,
        build_handler,
    )
    .await
}

/// Variant of [`execute_sse_request`] that replaces the JSON serialization
/// with a pre-encoded body (used by the wasm-encrypted Qoder inference
/// payload).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_sse_request_with_raw_body<BuildRequest, BuildHandler, HandlerFuture>(
    label: &str,
    url: &str,
    request_body: &serde_json::Value,
    raw_body: Option<Vec<u8>>,
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
                    round_attempt: trace.round_attempt().cloned(),
                })
                .await
        } else {
            None
        };
        let request_start_time = std::time::Instant::now();
        let send_outcome =
            send_stream_request(&build_request, request_body, raw_body.clone(), ttft_timeout).await;

        let response = match send_outcome {
            StreamSendOutcome::Response(resp) => {
                let connect_time = elapsed_ms_u64(request_start_time);
                let status = resp.status();
                let http_version = resp.version();
                let headers = resp.headers().clone();

                if status.is_success() {
                    debug!(
                        "{} request connected: {}ms, status: {}, protocol: {:?}, transport_attempt: {}/{}",
                        label,
                        connect_time,
                        status,
                        http_version,
                        attempt + 1,
                        max_tries
                    );
                    resp
                } else {
                    let error_text = resp
                        .text()
                        .await
                        .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
                    let error_kind = if status.is_client_error() {
                        "client error"
                    } else {
                        "error"
                    };
                    let provider_error = http_provider_error(
                        label,
                        status,
                        &error_text,
                        error_kind,
                        retry_after_delay_ms(&headers),
                    );
                    let error = anyhow!(provider_error);
                    warn!(
                        "{} request failed: {}ms, transport_attempt {}/{}, error: {}",
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
                        let delay_ms = retry_delay_ms(attempt, &headers, status);
                        debug!(
                            "Retrying {} after {}ms (transport_attempt {}, status {})",
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
                    "{} request failed: {}ms, transport_attempt {}/{}, error: {}",
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
                        "Retrying {} after {}ms (transport_attempt {})",
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
                    "{} request failed: {}ms, transport_attempt {}/{}, error: {}",
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
                        "Retrying {} after {}ms (transport_attempt {})",
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

    let last_error = last_error.unwrap_or_else(|| anyhow!("Unknown error"));
    let error_context = format!("{} failed after {} attempts", label, max_tries);
    error!("{}: {}", error_context, last_error);
    Err(last_error.context(error_context))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use axum::response::IntoResponse;
    use axum::routing::post;
    use axum::{Json, Router};
    use bitfun_core_types::errors::ErrorCategory;
    use reqwest::header::HeaderValue;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    };

    #[derive(Clone)]
    struct RetryFixtureState {
        attempts: Arc<AtomicUsize>,
    }

    async fn bad_requests_then_success(
        State(state): State<RetryFixtureState>,
        Json(body): Json<serde_json::Value>,
    ) -> impl IntoResponse {
        assert_eq!(body["model"], "configured-model");
        match state.attempts.fetch_add(1, Ordering::SeqCst) {
            0 => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": {
                        "message": "Invalid temperature value",
                        "type": "invalid_request_error",
                        "code": "invalid_parameter"
                    }
                })),
            )
                .into_response(),
            1 => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": {
                        "message": "Maximum context length exceeded",
                        "type": "invalid_request_error",
                        "code": "context_length_exceeded"
                    }
                })),
            )
                .into_response(),
            _ => StatusCode::OK.into_response(),
        }
    }

    async fn forbidden_with_retry_after(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
        assert_eq!(body["model"], "configured-model");
        (
            StatusCode::FORBIDDEN,
            [("retry-after", "7")],
            Json(serde_json::json!({
                "error": {
                    "message": "provider authorization denied",
                    "type": "permission_error",
                    "code": "permission_denied"
                }
            })),
        )
    }

    #[test]
    fn http_error_uses_structured_code_before_generic_message() {
        let error = http_provider_error(
            "OpenAI Responses API",
            StatusCode::BAD_REQUEST,
            r#"{"error":{"code":"context_length_exceeded","message":"Request failed"}}"#,
            "client error",
            None,
        );

        assert_eq!(error.category, ErrorCategory::ContextOverflow);
        assert_eq!(
            error.provider_code.as_deref(),
            Some("context_length_exceeded")
        );
        assert_eq!(error.http_status, Some(400));
    }

    #[test]
    fn no_body_bad_request_is_not_assumed_to_be_context_overflow() {
        let error = http_provider_error(
            "OpenAI Responses API",
            StatusCode::BAD_REQUEST,
            "400 status code (no body)",
            "client error",
            None,
        );

        assert_eq!(error.category, ErrorCategory::InvalidRequest);
    }

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

    #[tokio::test]
    async fn every_bad_request_uses_existing_retry_loop() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route("/chat/completions", post(bad_requests_then_success))
            .with_state(RetryFixtureState {
                attempts: Arc::clone(&attempts),
            });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind retry fixture");
        let address = listener.local_addr().expect("retry fixture address");
        let server_task = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("retry fixture should run");
        });
        let url = format!("http://{address}/chat/completions");
        let client = reqwest::Client::new();
        let request_body = serde_json::json!({"model": "configured-model"});

        let result = execute_sse_request(
            "OpenAI Streaming API",
            &url,
            &request_body,
            3,
            None,
            None,
            || client.post(&url),
            |_response, tx, _tx_raw, _remaining_ttft_timeout| async move {
                drop(tx);
            },
        )
        .await;

        server_task.abort();
        assert!(
            result.is_ok(),
            "ordinary and context-overflow 400 responses should both retry"
        );
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn single_attempt_preserves_retry_after_metadata_for_outer_budget() {
        let app = Router::new().route("/chat/completions", post(forbidden_with_retry_after));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind retry-after fixture");
        let address = listener.local_addr().expect("retry-after fixture address");
        let server_task = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("retry-after fixture should run");
        });
        let url = format!("http://{address}/chat/completions");
        let client = reqwest::Client::new();
        let request_body = serde_json::json!({"model": "configured-model"});

        let result = execute_sse_request(
            "OpenAI Streaming API",
            &url,
            &request_body,
            1,
            None,
            None,
            || client.post(&url),
            |_response, tx, _tx_raw, _remaining_ttft_timeout| async move {
                drop(tx);
            },
        )
        .await;

        server_task.abort();
        let error = match result {
            Ok(_) => panic!("single forbidden response should fail"),
            Err(error) => error,
        };
        let provider_error = error
            .downcast_ref::<AiProviderError>()
            .expect("structured provider error should survive retry context");
        assert_eq!(provider_error.http_status, Some(403));
        assert_eq!(provider_error.retry_after_ms, Some(7_000));
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

        assert_eq!(retry_delay_ms(0, &headers, StatusCode::BAD_GATEWAY), 500);
        assert_eq!(retry_delay_ms(1, &headers, StatusCode::BAD_GATEWAY), 1000);
        assert_eq!(retry_delay_ms(4, &headers, StatusCode::BAD_GATEWAY), 8_000);
        assert_eq!(retry_delay_ms(6, &headers, StatusCode::BAD_GATEWAY), 30_000);
        assert_eq!(retry_delay_ms(8, &headers, StatusCode::BAD_GATEWAY), 30_000);
    }

    #[test]
    fn rate_limit_retry_uses_longer_exponential_backoff() {
        let headers = HeaderMap::new();

        assert_eq!(
            retry_delay_ms(0, &headers, StatusCode::TOO_MANY_REQUESTS),
            2_000
        );
        assert_eq!(
            retry_delay_ms(1, &headers, StatusCode::TOO_MANY_REQUESTS),
            4_000
        );
        assert_eq!(
            retry_delay_ms(3, &headers, StatusCode::TOO_MANY_REQUESTS),
            16_000
        );
        assert_eq!(
            retry_delay_ms(5, &headers, StatusCode::TOO_MANY_REQUESTS),
            60_000
        );
        assert_eq!(
            retry_delay_ms(9, &headers, StatusCode::TOO_MANY_REQUESTS),
            60_000
        );
    }

    #[test]
    fn rate_limit_retry_after_never_undercuts_exponential_floor() {
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, HeaderValue::from_static("1"));

        // Retry-After: 1s must not collapse attempt 3 back to a 1s storm.
        assert_eq!(
            retry_delay_ms(3, &headers, StatusCode::TOO_MANY_REQUESTS),
            16_000
        );
    }

    #[test]
    fn rate_limit_honors_longer_retry_after() {
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, HeaderValue::from_static("45"));

        assert_eq!(
            retry_delay_ms(0, &headers, StatusCode::TOO_MANY_REQUESTS),
            45_000
        );
    }
}
