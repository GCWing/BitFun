//! Concrete HTTP transport for review-platform providers.

use serde::Serialize;
use serde_json::Value;
use std::time::Duration;
use thiserror::Error;

const REVIEW_PLATFORM_TIMEOUT_SECS: u64 = 25;
const HTTP_ERROR_PREVIEW_CHARS: usize = 280;

#[derive(Debug, Error)]
pub enum ReviewHttpError {
    #[error("Failed to create HTTP client: {0}")]
    BuildClient(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Provider API failed: HTTP {status}{message}")]
    Http { status: u16, message: String },
    #[error("Parse error: {0}")]
    Parse(String),
}

#[derive(Clone)]
pub struct ReviewHttpClient {
    inner: reqwest::Client,
}

impl ReviewHttpClient {
    pub fn new_review_platform() -> Result<Self, ReviewHttpError> {
        let inner = reqwest::Client::builder()
            .use_native_tls()
            .timeout(Duration::from_secs(REVIEW_PLATFORM_TIMEOUT_SECS))
            .build()
            .map_err(|error| ReviewHttpError::BuildClient(error.to_string()))?;

        Ok(Self { inner })
    }

    pub fn get(&self, url: &str) -> ReviewHttpRequest {
        ReviewHttpRequest {
            inner: self.inner.get(url),
        }
    }

    pub fn post(&self, url: &str) -> ReviewHttpRequest {
        ReviewHttpRequest {
            inner: self.inner.post(url),
        }
    }

    pub fn put(&self, url: &str) -> ReviewHttpRequest {
        ReviewHttpRequest {
            inner: self.inner.put(url),
        }
    }
}

pub struct ReviewHttpRequest {
    inner: reqwest::RequestBuilder,
}

impl ReviewHttpRequest {
    pub fn header(mut self, name: &str, value: impl ToString) -> Self {
        self.inner = self.inner.header(name, value.to_string());
        self
    }

    pub fn query<T: Serialize + ?Sized>(mut self, query: &T) -> Self {
        self.inner = self.inner.query(query);
        self
    }

    pub fn json<T: Serialize + ?Sized>(mut self, body: &T) -> Self {
        self.inner = self.inner.json(body);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ReviewJsonResponse {
    pub value: Value,
    pub headers: ReviewHttpHeaders,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewHttpHeaders {
    values: Vec<(String, String)>,
}

impl ReviewHttpHeaders {
    pub fn get(&self, name: &str) -> Option<&str> {
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

pub async fn send_json(request: ReviewHttpRequest) -> Result<Value, ReviewHttpError> {
    send_json_response(request)
        .await
        .map(|response| response.value)
}

pub async fn send_json_response(
    request: ReviewHttpRequest,
) -> Result<ReviewJsonResponse, ReviewHttpError> {
    let response = request
        .inner
        .send()
        .await
        .map_err(|error| ReviewHttpError::Network(error.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let message = body.chars().take(HTTP_ERROR_PREVIEW_CHARS).collect();
        return Err(ReviewHttpError::Http {
            status: status.as_u16(),
            message,
        });
    }

    let headers = ReviewHttpHeaders::from_header_map(response.headers());
    let value = response
        .json::<Value>()
        .await
        .map_err(|error| ReviewHttpError::Parse(error.to_string()))?;

    Ok(ReviewJsonResponse { value, headers })
}

pub async fn send_text(request: ReviewHttpRequest) -> Result<String, ReviewHttpError> {
    let response = request
        .inner
        .send()
        .await
        .map_err(|error| ReviewHttpError::Network(error.to_string()))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| ReviewHttpError::Network(error.to_string()))?;

    if !status.is_success() {
        let message = text.chars().take(HTTP_ERROR_PREVIEW_CHARS).collect();
        return Err(ReviewHttpError::Http {
            status: status.as_u16(),
            message,
        });
    }

    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::ReviewHttpHeaders;

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
}
