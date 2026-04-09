use crate::service::config::types::AIModelConfig;
use log::warn;
use serde::{Deserialize, Serialize};

fn append_endpoint(base_url: &str, endpoint: &str) -> String {
    let base = base_url.trim();
    if base.is_empty() {
        return endpoint.to_string();
    }
    if base.ends_with(endpoint) {
        return base.to_string();
    }
    format!("{}/{}", base.trim_end_matches('/'), endpoint)
}

fn gemini_base_url(url: &str) -> &str {
    let mut u = url;
    if let Some(pos) = u.find("/v1beta") {
        u = &u[..pos];
    }
    if let Some(pos) = u.find("/models/") {
        u = &u[..pos];
    }
    u.trim_end_matches('/')
}

fn resolve_gemini_request_url(base_url: &str, model_name: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }

    if let Some(stripped) = trimmed.strip_suffix('#') {
        return stripped.trim_end_matches('/').to_string();
    }

    let model = model_name.trim();
    if model.is_empty() {
        return trimmed.to_string();
    }

    let base = gemini_base_url(trimmed);
    format!(
        "{}/v1beta/models/{}:streamGenerateContent?alt=sse",
        base, model
    )
}

fn resolve_request_url(base_url: &str, provider: &str, model_name: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/').to_string();
    if trimmed.is_empty() {
        return String::new();
    }

    if let Some(stripped) = trimmed.strip_suffix('#') {
        return stripped.trim_end_matches('/').to_string();
    }

    match provider.trim().to_ascii_lowercase().as_str() {
        "openai" | "nvidia" | "openrouter" => append_endpoint(&trimmed, "chat/completions"),
        "response" | "responses" => append_endpoint(&trimmed, "responses"),
        "anthropic" => append_endpoint(&trimmed, "v1/messages"),
        "gemini" | "google" => resolve_gemini_request_url(&trimmed, model_name),
        _ => trimmed,
    }
}

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
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub enable_thinking_process: bool,
    pub inline_think_in_text: bool,
    pub custom_headers: Option<std::collections::HashMap<String, String>>,
    /// "replace" (default) or "merge" (defaults first, then custom)
    pub custom_headers_mode: Option<String>,
    pub skip_ssl_verify: bool,
    /// Reasoning effort for OpenAI Responses API ("low", "medium", "high", "xhigh")
    pub reasoning_effort: Option<String>,
    /// Custom JSON overriding default request body fields
    pub custom_request_body: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::resolve_request_url;

    #[test]
    fn resolves_openai_request_url() {
        assert_eq!(
            resolve_request_url("https://api.openai.com/v1", "openai", ""),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn resolves_responses_request_url() {
        assert_eq!(
            resolve_request_url("https://api.openai.com/v1", "responses", ""),
            "https://api.openai.com/v1/responses"
        );
    }

    #[test]
    fn resolves_response_alias_request_url() {
        assert_eq!(
            resolve_request_url("https://api.openai.com/v1", "response", ""),
            "https://api.openai.com/v1/responses"
        );
    }

    #[test]
    fn keeps_forced_request_url() {
        assert_eq!(
            resolve_request_url("https://api.openai.com/v1/responses#", "responses", ""),
            "https://api.openai.com/v1/responses"
        );
    }

    #[test]
    fn resolves_gemini_request_url_with_v1beta() {
        assert_eq!(
            resolve_request_url(
                "https://generativelanguage.googleapis.com/v1beta",
                "gemini",
                "gemini-2.5-pro"
            ),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro:streamGenerateContent?alt=sse"
        );
    }

    #[test]
    fn resolves_gemini_request_url_bare_host() {
        assert_eq!(
            resolve_request_url("https://api.openbitfun.com", "gemini", "gemini-2.5-pro"),
            "https://api.openbitfun.com/v1beta/models/gemini-2.5-pro:streamGenerateContent?alt=sse"
        );
    }

    #[test]
    fn resolves_nvidia_request_url() {
        assert_eq!(
            resolve_request_url("https://integrate.api.nvidia.com/v1", "nvidia", ""),
            "https://integrate.api.nvidia.com/v1/chat/completions"
        );
    }

    #[test]
    fn resolves_openrouter_request_url() {
        assert_eq!(
            resolve_request_url("https://openrouter.ai/api/v1", "openrouter", ""),
            "https://openrouter.ai/api/v1/chat/completions"
        );
    }
}

impl TryFrom<AIModelConfig> for AIConfig {
    type Error = String;
    fn try_from(other: AIModelConfig) -> Result<Self, <Self as TryFrom<AIModelConfig>>::Error> {
        // Parse custom request body (convert JSON string to serde_json::Value)
        let custom_request_body = if let Some(body_str) = &other.custom_request_body {
            match serde_json::from_str::<serde_json::Value>(body_str) {
                Ok(value) => Some(value),
                Err(e) => {
                    warn!(
                        "Failed to parse custom_request_body: {}, config: {}",
                        e, other.name
                    );
                    None
                }
            }
        } else {
            None
        };

        // Use stored request_url if present; otherwise derive from base_url + provider for legacy configs.
        let request_url = other
            .request_url
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| {
                resolve_request_url(&other.base_url, &other.provider, &other.model_name)
            });

        Ok(AIConfig {
            name: other.name.clone(),
            base_url: other.base_url.clone(),
            request_url,
            api_key: other.api_key.clone(),
            model: other.model_name.clone(),
            format: other.provider.clone(),
            context_window: other.context_window.unwrap_or(128128),
            max_tokens: other.max_tokens,
            temperature: other.temperature,
            top_p: other.top_p,
            enable_thinking_process: other.enable_thinking_process,
            inline_think_in_text: other.inline_think_in_text,
            custom_headers: other.custom_headers,
            custom_headers_mode: other.custom_headers_mode,
            skip_ssl_verify: other.skip_ssl_verify,
            reasoning_effort: other.reasoning_effort,
            custom_request_body,
        })
    }
}
