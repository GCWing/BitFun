use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Gemini API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiResponse {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<super::tool::ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<GeminiUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<Value>,
}

/// Gemini usage stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiUsage {
    #[serde(rename = "promptTokenCount")]
    pub prompt_token_count: u32,
    #[serde(rename = "candidatesTokenCount")]
    pub candidates_token_count: u32,
    #[serde(rename = "totalTokenCount")]
    pub total_token_count: u32,
    #[serde(rename = "reasoningTokenCount")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_token_count: Option<u32>,
    #[serde(rename = "cachedContentTokenCount")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_content_token_count: Option<u32>,
    #[serde(rename = "cacheCreationTokenCount")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_token_count: Option<u32>,
}

/// Structured message codes for localized connection test messaging.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionTestMessageCode {
    ToolCallsNotDetected,
    ImageInputCheckFailed,
}

/// AI connection test result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionTestResult {
    /// Whether the test succeeded
    pub success: bool,
    /// Response time (ms)
    pub response_time_ms: u64,
    /// Model response content (if successful)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_response: Option<String>,
    /// Structured message code for localized frontend messaging
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_code: Option<ConnectionTestMessageCode>,
    /// Raw error or diagnostic details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_details: Option<String>,
}

/// Remote model info discovered from a provider API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteModelInfo {
    /// Provider model identifier (used as the actual model_name).
    pub id: String,
    /// Optional human-readable display name returned by the provider.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::GeminiUsage;

    #[test]
    fn gemini_usage_roundtrips_cache_creation_field() {
        let usage = GeminiUsage {
            prompt_token_count: 100,
            candidates_token_count: 20,
            total_token_count: 120,
            reasoning_token_count: None,
            cached_content_token_count: Some(30),
            cache_creation_token_count: Some(20),
        };
        let json = serde_json::to_string(&usage).expect("serialize");
        assert!(json.contains("\"cacheCreationTokenCount\":20"));

        let parsed: GeminiUsage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.cache_creation_token_count, Some(20));
    }

    #[test]
    fn gemini_usage_legacy_payload_parses_with_new_field_absent() {
        // Records persisted before this plan don't have cacheCreationTokenCount;
        // they must still parse, with the new field defaulting to None.
        let raw = r#"{
            "promptTokenCount": 10,
            "candidatesTokenCount": 5,
            "totalTokenCount": 15,
            "cachedContentTokenCount": 3
        }"#;
        let parsed: GeminiUsage = serde_json::from_str(raw).expect("legacy payload");
        assert_eq!(parsed.cached_content_token_count, Some(3));
        assert_eq!(parsed.cache_creation_token_count, None);
    }
}
