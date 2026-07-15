//! Concrete HTTP transport for review-platform providers.

use futures::StreamExt;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;
use thiserror::Error;

const REVIEW_PLATFORM_TIMEOUT_SECS: u64 = 25;
const MAX_REVIEW_PLATFORM_REDIRECTS: usize = 5;
const DEFAULT_JSON_RESPONSE_MAX_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_TEXT_RESPONSE_MAX_BYTES: usize = 1024 * 1024;
const HTTP_ERROR_BODY_MAX_BYTES: usize = 8 * 1024;

#[derive(Debug, Error)]
pub(crate) enum ReviewHttpError {
    #[error("Failed to create HTTP client: {0}")]
    BuildClient(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Provider API failed: HTTP {status}{message}")]
    Http { status: u16, message: String },
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Provider response exceeded the {limit_bytes} byte limit")]
    ResponseTooLarge { limit_bytes: usize },
}

#[derive(Clone)]
pub(crate) struct ReviewHttpClient {
    inner: reqwest::Client,
}

impl ReviewHttpClient {
    pub(crate) fn new_review_platform() -> Result<Self, ReviewHttpError> {
        let inner = reqwest::Client::builder()
            .use_native_tls()
            .redirect(review_redirect_policy())
            .timeout(Duration::from_secs(REVIEW_PLATFORM_TIMEOUT_SECS))
            .build()
            .map_err(|error| ReviewHttpError::BuildClient(error.to_string()))?;

        Ok(Self { inner })
    }

    pub(crate) fn get(&self, url: &str) -> ReviewHttpRequest {
        ReviewHttpRequest {
            inner: self.inner.get(url),
        }
    }

    pub(crate) fn post(&self, url: &str) -> ReviewHttpRequest {
        ReviewHttpRequest {
            inner: self.inner.post(url),
        }
    }

    pub(crate) fn put(&self, url: &str) -> ReviewHttpRequest {
        ReviewHttpRequest {
            inner: self.inner.put(url),
        }
    }
}

fn review_redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= MAX_REVIEW_PLATFORM_REDIRECTS {
            return attempt.error("review provider redirect limit exceeded");
        }
        let Some(previous) = attempt.previous().last() else {
            return attempt.stop();
        };
        if same_origin(previous, attempt.url()) {
            attempt.follow()
        } else {
            attempt.stop()
        }
    })
}

fn same_origin(previous: &reqwest::Url, next: &reqwest::Url) -> bool {
    previous.scheme() == next.scheme()
        && previous.host_str() == next.host_str()
        && previous.port_or_known_default() == next.port_or_known_default()
}

pub(crate) struct ReviewHttpRequest {
    inner: reqwest::RequestBuilder,
}

impl ReviewHttpRequest {
    pub(crate) fn header(mut self, name: &str, value: impl ToString) -> Self {
        self.inner = self.inner.header(name, value.to_string());
        self
    }

    pub(crate) fn query<T: Serialize + ?Sized>(mut self, query: &T) -> Self {
        self.inner = self.inner.query(query);
        self
    }

    pub(crate) fn json<T: Serialize + ?Sized>(mut self, body: &T) -> Self {
        self.inner = self.inner.json(body);
        self
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ReviewJsonResponse {
    pub value: Value,
    pub headers: ReviewHttpHeaders,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReviewTextResponse {
    pub text: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ReviewHttpHeaders {
    values: Vec<(String, String)>,
}

impl ReviewHttpHeaders {
    pub(crate) fn get(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    fn from_header_map(headers: &reqwest::header::HeaderMap) -> Self {
        let values = headers
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_string(), value.to_string()))
            })
            .collect();

        Self { values }
    }
}

pub(crate) async fn send_json(request: ReviewHttpRequest) -> Result<Value, ReviewHttpError> {
    send_json_response(request)
        .await
        .map(|response| response.value)
}

pub(crate) async fn send_json_response(
    request: ReviewHttpRequest,
) -> Result<ReviewJsonResponse, ReviewHttpError> {
    send_json_response_bounded(request, DEFAULT_JSON_RESPONSE_MAX_BYTES).await
}

pub(crate) async fn send_json_response_bounded(
    request: ReviewHttpRequest,
    max_bytes: usize,
) -> Result<ReviewJsonResponse, ReviewHttpError> {
    let response = request
        .inner
        .send()
        .await
        .map_err(|error| ReviewHttpError::Network(error.to_string()))?;

    let status = response.status();
    let headers = ReviewHttpHeaders::from_header_map(response.headers());
    let body_limit = if status.is_success() {
        max_bytes
    } else {
        HTTP_ERROR_BODY_MAX_BYTES
    };
    if response
        .content_length()
        .is_some_and(|content_length| content_length > body_limit as u64)
    {
        if status.is_success() {
            return Err(ReviewHttpError::ResponseTooLarge {
                limit_bytes: max_bytes,
            });
        }
        return Err(ReviewHttpError::Http {
            status: status.as_u16(),
            message: String::new(),
        });
    }

    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| ReviewHttpError::Network(error.to_string()))?;
        if let Err(error) = append_bounded_chunk(&mut body, &chunk, body_limit) {
            if status.is_success() {
                return Err(error);
            }
            break;
        }
    }

    if !status.is_success() {
        return Err(ReviewHttpError::Http {
            status: status.as_u16(),
            message: String::new(),
        });
    }

    let value = serde_json::from_slice::<Value>(&body)
        .map_err(|error| ReviewHttpError::Parse(error.to_string()))?;
    Ok(ReviewJsonResponse { value, headers })
}

fn append_bounded_chunk(
    body: &mut Vec<u8>,
    chunk: &[u8],
    max_bytes: usize,
) -> Result<(), ReviewHttpError> {
    if body.len().saturating_add(chunk.len()) > max_bytes {
        return Err(ReviewHttpError::ResponseTooLarge {
            limit_bytes: max_bytes,
        });
    }
    body.extend_from_slice(chunk);
    Ok(())
}

pub(crate) async fn send_text(
    request: ReviewHttpRequest,
) -> Result<ReviewTextResponse, ReviewHttpError> {
    send_text_bounded(request, DEFAULT_TEXT_RESPONSE_MAX_BYTES).await
}

pub(crate) async fn send_text_bounded(
    request: ReviewHttpRequest,
    max_bytes: usize,
) -> Result<ReviewTextResponse, ReviewHttpError> {
    let response = request
        .inner
        .send()
        .await
        .map_err(|error| ReviewHttpError::Network(error.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        return Err(ReviewHttpError::Http {
            status: status.as_u16(),
            message: String::new(),
        });
    }
    let mut truncated = response
        .content_length()
        .is_some_and(|content_length| content_length > max_bytes as u64);
    let mut body = Vec::with_capacity(max_bytes.min(64 * 1024));
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| ReviewHttpError::Network(error.to_string()))?;
        let remaining = max_bytes.saturating_sub(body.len());
        if chunk.len() > remaining {
            body.extend_from_slice(&chunk[..remaining]);
            truncated = true;
            break;
        }
        body.extend_from_slice(&chunk);
    }

    Ok(ReviewTextResponse {
        text: String::from_utf8_lossy(&body).into_owned(),
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        append_bounded_chunk, send_json_response_bounded, send_text_bounded, ReviewHttpClient,
        ReviewHttpError, ReviewHttpHeaders,
    };
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    fn spawn_mock_server(response: Vec<u8>) -> (String, mpsc::Receiver<Option<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("mock listener should bind");
        listener
            .set_nonblocking(true)
            .expect("mock listener should be nonblocking");
        let address = listener
            .local_addr()
            .expect("mock listener should expose its address");
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(1)))
                            .expect("mock stream should accept read timeout");
                        let mut request = Vec::new();
                        let mut buffer = [0_u8; 1024];
                        loop {
                            match stream.read(&mut buffer) {
                                Ok(0) => break,
                                Ok(read) => {
                                    request.extend_from_slice(&buffer[..read]);
                                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        stream
                            .write_all(&response)
                            .expect("mock response should be written");
                        let _ = sender.send(Some(String::from_utf8_lossy(&request).to_string()));
                        return;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            let _ = sender.send(None);
                            return;
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("mock listener failed: {error}"),
                }
            }
        });
        (format!("http://{address}"), receiver)
    }

    fn spawn_same_origin_redirect_server() -> (String, mpsc::Receiver<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("mock listener should bind");
        let address = listener
            .local_addr()
            .expect("mock listener should expose its address");
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut requests = Vec::new();
            for request_index in 0..2 {
                let (mut stream, _) = listener.accept().expect("mock listener should accept");
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .expect("mock stream should accept read timeout");
                let mut request = Vec::new();
                let mut buffer = [0_u8; 1024];
                loop {
                    match stream.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(read) => {
                            request.extend_from_slice(&buffer[..read]);
                            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                requests.push(String::from_utf8_lossy(&request).to_string());
                let response = if request_index == 0 {
                    format!(
                        "HTTP/1.1 302 Found\r\nLocation: http://{address}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .into_bytes()
                } else {
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}"
                        .to_vec()
                };
                stream
                    .write_all(&response)
                    .expect("mock response should be written");
            }
            let _ = sender.send(requests);
        });
        (format!("http://{address}"), receiver)
    }

    fn chunked_response(status: &str, body: &[u8]) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 {status}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:X}\r\n",
            body.len()
        )
        .into_bytes();
        response.extend_from_slice(body);
        response.extend_from_slice(b"\r\n0\r\n\r\n");
        response
    }

    #[test]
    fn review_headers_are_case_insensitive() {
        let headers = ReviewHttpHeaders {
            values: vec![("X-Next-Page".to_string(), "2".to_string())],
        };

        assert_eq!(headers.get("x-next-page"), Some("2"));
    }

    #[test]
    fn review_headers_return_none_for_missing_value() {
        let headers = ReviewHttpHeaders {
            values: vec![(
                "Link".to_string(),
                "<https://example.com>; rel=\"next\"".to_string(),
            )],
        };

        assert_eq!(headers.get("x-total"), None);
    }

    #[test]
    fn bounded_body_rejects_a_chunk_past_the_limit() {
        let mut body = vec![1, 2];
        let error = append_bounded_chunk(&mut body, &[3, 4], 3).unwrap_err();

        assert!(matches!(
            error,
            ReviewHttpError::ResponseTooLarge { limit_bytes: 3 }
        ));
        assert_eq!(body, vec![1, 2]);
    }

    #[tokio::test]
    async fn review_client_never_follows_cross_origin_or_downgrade_redirects_with_tokens() {
        let (second_url, second_request) = spawn_mock_server(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}"
                .to_vec(),
        );
        let redirect = format!(
            "HTTP/1.1 302 Found\r\nLocation: {second_url}/stolen\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        let (first_url, first_request) = spawn_mock_server(redirect.into_bytes());
        let client = ReviewHttpClient::new_review_platform().expect("review client should build");

        let result = send_json_response_bounded(
            client
                .get(&format!("{first_url}/issue"))
                .header("PRIVATE-TOKEN", "never-forward-this-token"),
            1024,
        )
        .await;

        assert!(matches!(
            result,
            Err(ReviewHttpError::Http { status: 302, .. })
        ));
        let first_request = first_request
            .recv_timeout(Duration::from_secs(3))
            .expect("first server should report")
            .expect("first server should receive a request");
        assert!(first_request
            .to_ascii_lowercase()
            .contains("private-token: never-forward-this-token"));
        assert_eq!(
            second_request
                .recv_timeout(Duration::from_secs(3))
                .expect("second server should report after timeout"),
            None,
            "redirect target must never receive the provider token"
        );
    }

    #[tokio::test]
    async fn review_client_follows_bounded_same_origin_redirects() {
        let (url, requests) = spawn_same_origin_redirect_server();
        let client = ReviewHttpClient::new_review_platform().expect("review client should build");

        let response = send_json_response_bounded(
            client
                .get(&format!("{url}/renamed"))
                .header("PRIVATE-TOKEN", "same-origin-token"),
            1024,
        )
        .await
        .expect("same-origin redirect should remain compatible");

        assert_eq!(response.value, serde_json::json!({}));
        let requests = requests
            .recv_timeout(Duration::from_secs(3))
            .expect("mock server should report both requests");
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|request| request
            .to_ascii_lowercase()
            .contains("private-token: same-origin-token")));
    }

    #[tokio::test]
    async fn bounded_json_rejects_chunked_success_without_content_length() {
        let body = format!("{{\"body\":\"{}\"}}", "x".repeat(128));
        let (url, request) = spawn_mock_server(chunked_response("200 OK", body.as_bytes()));
        let client = ReviewHttpClient::new_review_platform().expect("review client should build");

        let result = send_json_response_bounded(client.get(&format!("{url}/chunked")), 64).await;

        assert!(matches!(
            result,
            Err(ReviewHttpError::ResponseTooLarge { limit_bytes: 64 })
        ));
        assert!(request
            .recv_timeout(Duration::from_secs(3))
            .expect("mock server should report")
            .is_some());
    }

    #[tokio::test]
    async fn error_bodies_are_bounded_and_never_exposed() {
        let secret = "provider-secret-body".repeat(1024);
        let (url, request) = spawn_mock_server(chunked_response(
            "500 Internal Server Error",
            secret.as_bytes(),
        ));
        let client = ReviewHttpClient::new_review_platform().expect("review client should build");

        let result = send_json_response_bounded(client.get(&format!("{url}/failure")), 64).await;

        match result {
            Err(ReviewHttpError::Http { status, message }) => {
                assert_eq!(status, 500);
                assert!(message.is_empty());
            }
            other => panic!("expected sanitized HTTP failure, got {other:?}"),
        }
        assert!(request
            .recv_timeout(Duration::from_secs(3))
            .expect("mock server should report")
            .is_some());
    }

    #[tokio::test]
    async fn bounded_text_truncates_chunked_success_without_content_length() {
        let body = "trace-line\n".repeat(32);
        let (url, request) = spawn_mock_server(chunked_response("200 OK", body.as_bytes()));
        let client = ReviewHttpClient::new_review_platform().expect("review client should build");

        let response = send_text_bounded(client.get(&format!("{url}/trace")), 64)
            .await
            .expect("bounded text should return a partial response");

        assert!(response.truncated);
        assert!(response.text.len() <= 64);
        assert!(request
            .recv_timeout(Duration::from_secs(3))
            .expect("mock server should report")
            .is_some());
    }
}
