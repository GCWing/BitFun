//! Unified error handling
//!
//! Provide unified error types and handling for the whole application

use bitfun_core_types::errors::{AiErrorDetail, ErrorCategory};
use serde::Serialize;
use thiserror::Error;

/// Unified error type for the BitFun application
#[derive(Debug, Error, Serialize)]
pub enum BitFunError {
    #[error("Service error: {0}")]
    Service(String),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("Tool error: {0}")]
    Tool(String),

    #[error("AI client error: {0}")]
    AIClient(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Workspace error: {0}")]
    Workspace(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    #[serde(serialize_with = "serialize_io_error")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    #[serde(serialize_with = "serialize_serde_error")]
    Serialization(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    #[serde(serialize_with = "serialize_reqwest_error")]
    Http(#[from] reqwest::Error),

    #[error("Other error: {0}")]
    #[serde(serialize_with = "serialize_anyhow_error")]
    Other(#[from] anyhow::Error),

    #[error("Semaphore acquire error: {0}")]
    Semaphore(String),

    #[error("MCP error: {0}")]
    MCPError(String),

    #[error("Process error: {0}")]
    ProcessError(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("Cancelled: {0}")]
    Cancelled(String),
}

pub type BitFunResult<T> = Result<T, BitFunError>;

// Custom serialization functions for non-serializable error types
fn serialize_io_error<S>(err: &std::io::Error, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&err.to_string())
}

fn serialize_serde_error<S>(err: &serde_json::Error, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&err.to_string())
}

fn serialize_reqwest_error<S>(err: &reqwest::Error, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&err.to_string())
}

fn serialize_anyhow_error<S>(err: &anyhow::Error, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&err.to_string())
}

impl BitFunError {
    pub fn service<T: Into<String>>(msg: T) -> Self {
        Self::Service(msg.into())
    }

    pub fn agent<T: Into<String>>(msg: T) -> Self {
        Self::Agent(msg.into())
    }

    pub fn tool<T: Into<String>>(msg: T) -> Self {
        Self::Tool(msg.into())
    }

    pub fn config<T: Into<String>>(msg: T) -> Self {
        Self::Configuration(msg.into())
    }

    pub fn validation<T: Into<String>>(msg: T) -> Self {
        Self::Validation(msg.into())
    }

    pub fn ai<T: Into<String>>(msg: T) -> Self {
        Self::AIClient(msg.into())
    }

    pub fn parse<T: Into<String>>(msg: T) -> Self {
        Self::Deserialization(msg.into())
    }

    pub fn workspace<T: Into<String>>(msg: T) -> Self {
        Self::Workspace(msg.into())
    }

    pub fn serialization<T: Into<String>>(msg: T) -> Self {
        Self::Serialization(serde_json::Error::io(std::io::Error::other(msg.into())))
    }

    pub fn session<T: Into<String>>(msg: T) -> Self {
        Self::Session(msg.into())
    }

    pub fn io<T: Into<String>>(msg: T) -> Self {
        Self::Io(std::io::Error::other(msg.into()))
    }

    pub fn cancelled<T: Into<String>>(msg: T) -> Self {
        Self::Cancelled(msg.into())
    }

    /// Infer an error category from this error for frontend-friendly classification.
    pub fn error_category(&self) -> ErrorCategory {
        match self {
            BitFunError::AIClient(msg) => classify_ai_error(msg),
            BitFunError::Timeout(_) => ErrorCategory::Timeout,
            BitFunError::Cancelled(_) => ErrorCategory::Unknown,
            _ => ErrorCategory::Unknown,
        }
    }

    /// Build a structured, provider-agnostic AI error detail for UI recovery.
    pub fn error_detail(&self) -> AiErrorDetail {
        let category = self.error_category();
        let message = self.to_string();
        AiErrorDetail {
            category: category.clone(),
            provider: extract_error_field(&message, "provider"),
            provider_code: extract_error_field(&message, "code"),
            provider_message: extract_error_field(&message, "message"),
            request_id: extract_error_field(&message, "request_id"),
            http_status: extract_http_status(&message),
            retryable: Some(is_retryable_category(&category)),
            action_hints: action_hints_for_category(&category),
        }
    }
}

/// Classify an AI client error message into a structured category.
fn classify_ai_error(msg: &str) -> ErrorCategory {
    let m = msg.to_lowercase();
    if contains_any(
        &m,
        &[
            "code=1113",
            "\"code\":\"1113\"",
            "insufficient_quota",
            "insufficient quota",
            "insufficient balance",
            "not_enough_balance",
            "not enough balance",
            "exceeded_current_quota_error",
            "exceeded current quota",
            "you exceeded your current quota",
            "no available resource package",
            "无可用资源包",
            "余额不足",
            "账户已欠费",
            "account has exceeded",
            "http 402",
            "error 402",
            "402 - insufficient balance",
        ],
    ) {
        ErrorCategory::ProviderQuota
    } else if contains_any(
        &m,
        &[
            "billing",
            "membership expired",
            "subscription expired",
            "plan expired",
            "套餐已到期",
            "1309",
        ],
    ) {
        ErrorCategory::ProviderBilling
    } else if contains_any(
        &m,
        &[
            "overloaded_error",
            "server overloaded",
            "temporarily overloaded",
            "provider unavailable",
            "service unavailable",
            "http 503",
            "error 503",
            "http 529",
            "error 529",
            "1305",
        ],
    ) {
        ErrorCategory::ProviderUnavailable
    } else if contains_any(
        &m,
        &[
            "content policy",
            "policy blocked",
            "safety",
            "sensitive",
            "content_filter",
            "1301",
            "api 调用被策略阻止",
        ],
    ) {
        ErrorCategory::ContentPolicy
    } else if m.contains("rate limit")
        || m.contains("429")
        || m.contains("too many requests")
        || m.contains("1302")
        || m.contains("concurrency")
        || m.contains("请求并发超额")
    {
        ErrorCategory::RateLimit
    } else if m.contains("authentication")
        || m.contains("401")
        || m.contains("invalid api key")
        || m.contains("incorrect api key")
        || m.contains("unauthorized")
        || m.contains("1000")
        || m.contains("1002")
    {
        ErrorCategory::Auth
    } else if contains_any(
        &m,
        &[
            "permission_error",
            "permission denied",
            "forbidden",
            "not authorized",
            "no permission",
            "无权访问",
            "1220",
        ],
    ) {
        ErrorCategory::Permission
    } else if m.contains("context window")
        || m.contains("token limit")
        || m.contains("max_tokens")
        || m.contains("context length")
    {
        ErrorCategory::ContextOverflow
    } else if contains_any(
        &m,
        &[
            "invalid_request_error",
            "invalid request",
            "bad request",
            "invalid format",
            "invalid parameter",
            "model not found",
            "unsupported model",
            "request too large",
            "http 400",
            "error 400",
            "http 413",
            "error 413",
            "http 422",
            "error 422",
            "1210",
            "1211",
            "435",
        ],
    ) {
        ErrorCategory::InvalidRequest
    } else if m.contains("timeout") || m.contains("timed out") {
        ErrorCategory::Timeout
    } else if m.contains("stream closed")
        || m.contains("sse error")
        || m.contains("connection reset")
        || m.contains("broken pipe")
    {
        ErrorCategory::Network
    } else {
        ErrorCategory::ModelError
    }
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn is_retryable_category(category: &ErrorCategory) -> bool {
    matches!(
        category,
        ErrorCategory::Network
            | ErrorCategory::RateLimit
            | ErrorCategory::Timeout
            | ErrorCategory::ProviderUnavailable
    )
}

fn action_hints_for_category(category: &ErrorCategory) -> Vec<String> {
    let hints: &[&str] = match category {
        ErrorCategory::ProviderQuota | ErrorCategory::ProviderBilling => {
            &["open_model_settings", "switch_model", "copy_diagnostics"]
        }
        ErrorCategory::Auth | ErrorCategory::Permission => {
            &["open_model_settings", "copy_diagnostics"]
        }
        ErrorCategory::RateLimit | ErrorCategory::ProviderUnavailable => {
            &["wait_and_retry", "switch_model", "copy_diagnostics"]
        }
        ErrorCategory::ContextOverflow => &["compress_context", "start_new_chat"],
        ErrorCategory::Network | ErrorCategory::Timeout => {
            &["retry", "switch_model", "copy_diagnostics"]
        }
        ErrorCategory::ContentPolicy | ErrorCategory::InvalidRequest => &["copy_diagnostics"],
        ErrorCategory::ModelError | ErrorCategory::Unknown => {
            &["retry", "switch_model", "copy_diagnostics"]
        }
    };

    hints.iter().map(|hint| (*hint).to_string()).collect()
}

fn extract_error_field(message: &str, field: &str) -> Option<String> {
    let key = format!("{field}=");
    if let Some(start) = message.find(&key) {
        let value_start = start + key.len();
        let value = message[value_start..]
            .split([',', ';'])
            .next()
            .unwrap_or_default()
            .trim()
            .trim_matches('"');
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }

    let json_key = format!("\"{field}\"");
    if let Some(start) = message.find(&json_key) {
        let after_key = &message[start + json_key.len()..];
        if let Some(colon_pos) = after_key.find(':') {
            let after_colon = after_key[colon_pos + 1..].trim_start();
            let value = after_colon
                .trim_start_matches('"')
                .split(['"', ',', '}'])
                .next()
                .unwrap_or_default()
                .trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }

    None
}

fn extract_http_status(message: &str) -> Option<u16> {
    let m = message.to_lowercase();
    for marker in ["http ", "error ", "status "] {
        if let Some(start) = m.find(marker) {
            let digits = m[start + marker.len()..]
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            if let Ok(status) = digits.parse::<u16>() {
                return Some(status);
            }
        }
    }

    None
}

impl From<BitFunError> for String {
    fn from(err: BitFunError) -> String {
        err.to_string()
    }
}

impl From<String> for BitFunError {
    fn from(error: String) -> Self {
        BitFunError::Service(error)
    }
}

impl From<&str> for BitFunError {
    fn from(error: &str) -> Self {
        BitFunError::Service(error.to_string())
    }
}

impl From<tokio::sync::AcquireError> for BitFunError {
    fn from(error: tokio::sync::AcquireError) -> Self {
        BitFunError::Semaphore(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::BitFunError;
    use bitfun_events::agentic::ErrorCategory;

    #[test]
    fn classifies_glm_quota_error_as_provider_quota() {
        let err = BitFunError::AIClient(
            r#"Provider error: provider=glm, code=1113, message=余额不足或无可用资源包,请充值。, request_id=20260425142416"#.to_string(),
        );

        assert_eq!(err.error_category(), ErrorCategory::ProviderQuota);
    }

    #[test]
    fn classifies_deepseek_insufficient_balance_as_provider_quota() {
        let err = BitFunError::AIClient(
            "DeepSeek API error 402 - Insufficient Balance: You have run out of balance"
                .to_string(),
        );

        assert_eq!(err.error_category(), ErrorCategory::ProviderQuota);
    }

    #[test]
    fn classifies_anthropic_overload_as_provider_unavailable() {
        let err = BitFunError::AIClient(
            "Anthropic API error 529: overloaded_error: Anthropic API is temporarily overloaded"
                .to_string(),
        );

        assert_eq!(err.error_category(), ErrorCategory::ProviderUnavailable);
    }

    #[test]
    fn error_detail_extracts_provider_metadata_and_retry_hint() {
        let err = BitFunError::AIClient(
            r#"Provider error: provider=openai, code=rate_limit_exceeded, message="Too many requests", request_id=req_123, http 429"#.to_string(),
        );

        let detail = err.error_detail();

        assert_eq!(detail.category, ErrorCategory::RateLimit);
        assert_eq!(detail.provider.as_deref(), Some("openai"));
        assert_eq!(detail.provider_code.as_deref(), Some("rate_limit_exceeded"));
        assert_eq!(detail.provider_message.as_deref(), Some("Too many requests"));
        assert_eq!(detail.request_id.as_deref(), Some("req_123"));
        assert_eq!(detail.http_status, Some(429));
        assert_eq!(detail.retryable, Some(true));
        assert_eq!(
            detail.action_hints,
            vec!["wait_and_retry", "switch_model", "copy_diagnostics"]
        );
    }
}
