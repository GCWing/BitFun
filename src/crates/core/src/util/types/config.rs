use log::warn;
use crate::service::config::types::AIModelConfig;
use serde::{Deserialize, Serialize};

/// AI client configuration (for AI requests)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIConfig {
    pub name: String,
    pub base_url: String,
    /// Actual request URL
    /// Falls back to base_url when absent
    pub request_url: String,
    pub api_key: String,
    pub model: String,
    pub format: String,
    pub context_window: u32,
    pub max_tokens: Option<u32>,
    pub enable_thinking_process: bool,
    pub support_preserved_thinking: bool,
    pub custom_headers: Option<std::collections::HashMap<String, String>>,
    /// "replace" (default) or "merge" (defaults first, then custom)
    pub custom_headers_mode: Option<String>,
    pub skip_ssl_verify: bool,
    /// Custom JSON overriding default request body fields
    pub custom_request_body: Option<serde_json::Value>,
}

impl TryFrom<AIModelConfig> for AIConfig {
    type Error = String;
    fn try_from(other: AIModelConfig) -> Result<Self, <Self as TryFrom<AIModelConfig>>::Error> {
        // Parse custom request body (convert JSON string to serde_json::Value)
        let custom_request_body = if let Some(body_str) = &other.custom_request_body {
            match serde_json::from_str::<serde_json::Value>(body_str) {
                Ok(value) => Some(value),
                Err(e) => {
                    warn!("Failed to parse custom_request_body: {}, config: {}", e, other.name);
                    None
                }
            }
        } else {
            None
        };

        // Use stored request_url if present, otherwise fall back to base_url (legacy configs)
        let request_url = other
            .request_url
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| other.base_url.clone());

        Ok(AIConfig {
            name: other.name.clone(),
            base_url: other.base_url.clone(),
            request_url,
            api_key: other.api_key.clone(),
            model: other.model_name.clone(),
            format: other.provider.clone(),
            context_window: other.context_window.unwrap_or(128128),
            max_tokens: other.max_tokens,
            enable_thinking_process: other.enable_thinking_process,
            support_preserved_thinking: other.support_preserved_thinking,
            custom_headers: other.custom_headers,
            custom_headers_mode: other.custom_headers_mode,
            skip_ssl_verify: other.skip_ssl_verify,
            custom_request_body,
        })
    }
}
