use serde::{Deserialize, Serialize};

/// Error category for classifying dialog turn failures.
/// Used by the frontend to show user-friendly error messages without string matching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    /// Network interruption, SSE stream closed, connection reset
    Network,
    /// API authentication failure, invalid/expired key
    Auth,
    /// Rate limit exceeded
    RateLimit,
    /// Conversation exceeds model context window
    ContextOverflow,
    /// Model response timed out
    Timeout,
    /// Provider/account quota, balance, or resource package is exhausted
    ProviderQuota,
    /// Provider billing plan, subscription, or package is invalid or expired
    ProviderBilling,
    /// Provider service is overloaded or temporarily unavailable
    ProviderUnavailable,
    /// API key is valid but does not have access to the requested resource
    Permission,
    /// Request format, parameters, model name, or payload size is invalid
    InvalidRequest,
    /// Provider policy or content safety system blocked the request
    ContentPolicy,
    /// Model returned an error
    ModelError,
    /// Unclassified error
    Unknown,
}

/// Structured AI error details for user-facing recovery and diagnostics.
///
/// Keep this shape provider-agnostic: stable categories drive UI behavior while
/// provider-specific codes/messages remain optional metadata for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiErrorDetail {
    pub category: ErrorCategory,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub action_hints: Vec<String>,
}
