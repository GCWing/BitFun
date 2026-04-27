use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorKind {
    Network,
    Auth,
    RateLimit,
    ContextOverflow,
    Timeout,
    ProviderQuota,
    ProviderBilling,
    ProviderUnavailable,
    Permission,
    InvalidRequest,
    ContentPolicy,
    ModelError,
    Unknown,
}

impl ProviderErrorKind {
    pub fn is_retryable(self) -> bool {
        matches!(
            self,
            ProviderErrorKind::Network
                | ProviderErrorKind::RateLimit
                | ProviderErrorKind::Timeout
                | ProviderErrorKind::ProviderUnavailable
        )
    }

    fn as_str(self) -> &'static str {
        match self {
            ProviderErrorKind::Network => "network",
            ProviderErrorKind::Auth => "auth",
            ProviderErrorKind::RateLimit => "rate_limit",
            ProviderErrorKind::ContextOverflow => "context_overflow",
            ProviderErrorKind::Timeout => "timeout",
            ProviderErrorKind::ProviderQuota => "provider_quota",
            ProviderErrorKind::ProviderBilling => "provider_billing",
            ProviderErrorKind::ProviderUnavailable => "provider_unavailable",
            ProviderErrorKind::Permission => "permission",
            ProviderErrorKind::InvalidRequest => "invalid_request",
            ProviderErrorKind::ContentPolicy => "content_policy",
            ProviderErrorKind::ModelError => "model_error",
            ProviderErrorKind::Unknown => "unknown",
        }
    }
}

impl fmt::Display for ProviderErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderError {
    provider: Option<String>,
    kind: ProviderErrorKind,
    code: Option<String>,
    message: String,
    request_id: Option<String>,
    http_status: Option<u16>,
}

impl ProviderError {
    pub fn builder(message: impl Into<String>) -> ProviderErrorBuilder {
        ProviderErrorBuilder {
            error: ProviderError {
                provider: None,
                kind: ProviderErrorKind::Unknown,
                code: None,
                message: message.into(),
                request_id: None,
                http_status: None,
            },
        }
    }

    pub fn from_error_payload(provider: &str, payload: &Value) -> Option<Self> {
        let error = payload.get("error")?;
        let request_id = payload
            .get("request_id")
            .or_else(|| payload.get("requestId"))
            .and_then(json_scalar_to_string);

        if let Some(message) = error.as_str() {
            return Some(
                Self::builder(message)
                    .provider(provider)
                    .kind(classify_provider_error(None, message, None))
                    .maybe_request_id(request_id)
                    .build(),
            );
        }

        let error_object = error.as_object()?;
        let code = error_object.get("code").and_then(json_scalar_to_string);
        let message = error_object
            .get("message")
            .and_then(|value| value.as_str())
            .or_else(|| error_object.get("error").and_then(|value| value.as_str()))
            .unwrap_or("Provider returned an error");
        let http_status = error_object
            .get("status")
            .and_then(|value| value.as_u64())
            .and_then(|status| u16::try_from(status).ok())
            .or_else(|| {
                payload
                    .get("status")
                    .and_then(|value| value.as_u64())
                    .and_then(|status| u16::try_from(status).ok())
            });

        Some(
            Self::builder(message)
                .provider(provider)
                .kind(classify_provider_error(
                    code.as_deref(),
                    message,
                    http_status,
                ))
                .maybe_code(code)
                .maybe_request_id(request_id)
                .maybe_http_status(http_status)
                .build(),
        )
    }

    pub fn from_http_error(provider: &str, status: u16, body: &str) -> Self {
        let parsed = serde_json::from_str::<Value>(body)
            .ok()
            .and_then(|value| Self::from_error_payload(provider, &value));

        if let Some(error) = parsed {
            return error.with_http_status(status);
        }

        Self::builder(body.trim())
            .provider(provider)
            .kind(classify_provider_error(None, body, Some(status)))
            .http_status(status)
            .build()
    }

    pub fn provider(&self) -> Option<&str> {
        self.provider.as_deref()
    }

    pub fn kind(&self) -> ProviderErrorKind {
        self.kind
    }

    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }

    pub fn http_status(&self) -> Option<u16> {
        self.http_status
    }

    pub fn is_retryable(&self) -> bool {
        self.kind.is_retryable()
    }

    fn with_http_status(mut self, status: u16) -> Self {
        self.http_status = Some(status);
        let kind_with_status =
            classify_provider_error(self.code.as_deref(), &self.message, Some(status));
        if matches!(
            self.kind,
            ProviderErrorKind::Unknown | ProviderErrorKind::ModelError
        ) && !matches!(
            kind_with_status,
            ProviderErrorKind::Unknown | ProviderErrorKind::ModelError
        ) {
            self.kind = kind_with_status;
        }
        self
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Provider error")?;
        if let Some(provider) = &self.provider {
            write!(f, ": provider={provider}")?;
        }
        write!(f, ", kind={}", self.kind)?;
        if let Some(code) = &self.code {
            write!(f, ", code={code}")?;
        }
        if let Some(request_id) = &self.request_id {
            write!(f, ", request_id={request_id}")?;
        }
        if let Some(http_status) = self.http_status {
            write!(f, ", http_status={http_status}")?;
        }
        write!(
            f,
            ", retryable={}, message={}",
            self.is_retryable(),
            self.message
        )
    }
}

impl Error for ProviderError {}

pub struct ProviderErrorBuilder {
    error: ProviderError,
}

impl ProviderErrorBuilder {
    pub fn provider(mut self, provider: impl Into<String>) -> Self {
        self.error.provider = Some(provider.into());
        self
    }

    pub fn kind(mut self, kind: ProviderErrorKind) -> Self {
        self.error.kind = kind;
        self
    }

    pub fn code(mut self, code: impl Into<String>) -> Self {
        self.error.code = Some(code.into());
        self
    }

    pub fn request_id(mut self, request_id: impl Into<String>) -> Self {
        self.error.request_id = Some(request_id.into());
        self
    }

    pub fn http_status(mut self, http_status: u16) -> Self {
        self.error.http_status = Some(http_status);
        self
    }

    pub fn build(self) -> ProviderError {
        self.error
    }

    fn maybe_code(self, code: Option<String>) -> Self {
        if let Some(code) = code {
            self.code(code)
        } else {
            self
        }
    }

    fn maybe_request_id(self, request_id: Option<String>) -> Self {
        if let Some(request_id) = request_id {
            self.request_id(request_id)
        } else {
            self
        }
    }

    fn maybe_http_status(self, http_status: Option<u16>) -> Self {
        if let Some(http_status) = http_status {
            self.http_status(http_status)
        } else {
            self
        }
    }
}

fn json_scalar_to_string(value: &Value) -> Option<String> {
    if let Some(value) = value.as_str() {
        return Some(value.to_string());
    }
    if let Some(value) = value.as_i64() {
        return Some(value.to_string());
    }
    if let Some(value) = value.as_u64() {
        return Some(value.to_string());
    }
    if let Some(value) = value.as_bool() {
        return Some(value.to_string());
    }
    None
}

fn classify_provider_error(
    code: Option<&str>,
    message: &str,
    http_status: Option<u16>,
) -> ProviderErrorKind {
    let message = message.to_lowercase();
    let code = code.unwrap_or_default().to_lowercase();

    if matches!(http_status, Some(401)) || matches!(code.as_str(), "1000" | "1002") {
        ProviderErrorKind::Auth
    } else if matches!(http_status, Some(403)) || code == "1220" {
        ProviderErrorKind::Permission
    } else if matches!(http_status, Some(429)) || code == "1302" || message.contains("rate limit") {
        ProviderErrorKind::RateLimit
    } else if matches!(http_status, Some(402))
        || matches!(code.as_str(), "1113" | "insufficient_quota")
    {
        ProviderErrorKind::ProviderQuota
    } else if code == "1309" || message.contains("billing") || message.contains("subscription") {
        ProviderErrorKind::ProviderBilling
    } else if matches!(http_status, Some(500..=599))
        || code == "1305"
        || message.contains("overloaded")
        || message.contains("temporarily unavailable")
        || message.contains("service unavailable")
    {
        ProviderErrorKind::ProviderUnavailable
    } else if code == "1301"
        || message.contains("content policy")
        || message.contains("content_filter")
    {
        ProviderErrorKind::ContentPolicy
    } else if matches!(http_status, Some(400 | 413 | 422))
        || matches!(code.as_str(), "1210" | "1211" | "435")
        || message.contains("invalid request")
        || message.contains("invalid parameter")
        || message.contains("model not found")
    {
        ProviderErrorKind::InvalidRequest
    } else if message.contains("context window")
        || message.contains("context length")
        || message.contains("token limit")
    {
        ProviderErrorKind::ContextOverflow
    } else if message.contains("timeout") || message.contains("timed out") {
        ProviderErrorKind::Timeout
    } else if message.contains("connection reset")
        || message.contains("broken pipe")
        || message.contains("stream closed")
    {
        ProviderErrorKind::Network
    } else if code.is_empty() && http_status.is_none() {
        ProviderErrorKind::Unknown
    } else {
        ProviderErrorKind::ModelError
    }
}

#[cfg(test)]
mod tests {
    use super::{ProviderError, ProviderErrorKind};

    #[test]
    fn parses_json_http_error_body_into_provider_error() {
        let error = ProviderError::from_http_error(
            "OpenAI Streaming API",
            429,
            r#"{"error":{"code":"rate_limit_exceeded","message":"too many requests"},"request_id":"req_http_429"}"#,
        );

        assert_eq!(error.provider(), Some("OpenAI Streaming API"));
        assert_eq!(error.kind(), ProviderErrorKind::RateLimit);
        assert_eq!(error.code(), Some("rate_limit_exceeded"));
        assert_eq!(error.message(), "too many requests");
        assert_eq!(error.request_id(), Some("req_http_429"));
        assert_eq!(error.http_status(), Some(429));
        assert!(error.is_retryable());
    }

    #[test]
    fn classifies_plain_http_client_error_without_json() {
        let error = ProviderError::from_http_error("Responses API", 401, "unauthorized");

        assert_eq!(error.provider(), Some("Responses API"));
        assert_eq!(error.kind(), ProviderErrorKind::Auth);
        assert_eq!(error.message(), "unauthorized");
        assert_eq!(error.http_status(), Some(401));
        assert!(!error.is_retryable());
    }
}
