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

/// Classify an AI client error message into a structured category.
pub fn classify_ai_error_message(msg: &str) -> ErrorCategory {
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

/// Build a structured, provider-agnostic AI error detail for UI recovery.
pub fn ai_error_detail_from_message(message: &str, category: ErrorCategory) -> AiErrorDetail {
    AiErrorDetail {
        category: category.clone(),
        provider: extract_error_field(message, "provider"),
        provider_code: extract_error_field(message, "code"),
        provider_message: extract_error_field(message, "message"),
        request_id: extract_error_field(message, "request_id"),
        http_status: extract_http_status(message),
        retryable: Some(is_retryable_category(&category)),
        action_hints: action_hints_for_category(&category),
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

#[cfg(test)]
mod tests {
    use super::{ai_error_detail_from_message, classify_ai_error_message, ErrorCategory};

    #[test]
    fn classifies_quota_and_provider_unavailable_errors() {
        assert_eq!(
            classify_ai_error_message("Provider error: provider=glm, code=1113, message=余额不足"),
            ErrorCategory::ProviderQuota
        );
        assert_eq!(
            classify_ai_error_message(
                "DeepSeek API error 402 - Insufficient Balance: You have run out of balance"
            ),
            ErrorCategory::ProviderQuota
        );
        assert_eq!(
            classify_ai_error_message(
                "Anthropic API error 529: overloaded_error: Anthropic API is temporarily overloaded"
            ),
            ErrorCategory::ProviderUnavailable
        );
    }

    #[test]
    fn builds_ai_error_detail_from_provider_metadata() {
        let detail = ai_error_detail_from_message(
            r#"AI client error: provider=openai, code=rate_limit_exceeded, message="Too many requests", request_id=req_123, http 429"#,
            ErrorCategory::RateLimit,
        );

        assert_eq!(detail.category, ErrorCategory::RateLimit);
        assert_eq!(detail.provider.as_deref(), Some("openai"));
        assert_eq!(detail.provider_code.as_deref(), Some("rate_limit_exceeded"));
        assert_eq!(
            detail.provider_message.as_deref(),
            Some("Too many requests")
        );
        assert_eq!(detail.request_id.as_deref(), Some("req_123"));
        assert_eq!(detail.http_status, Some(429));
        assert_eq!(detail.retryable, Some(true));
        assert_eq!(
            detail.action_hints,
            vec!["wait_and_retry", "switch_model", "copy_diagnostics"]
        );
    }
}
