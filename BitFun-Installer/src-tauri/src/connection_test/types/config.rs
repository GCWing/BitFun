//! Copied from `bitfun_core::util::types::config` (installer-local; no bitfun_core).

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

/// Same rules as `bitfun_core::util::types::config::resolve_request_url`.
pub fn resolve_request_url(base_url: &str, provider: &str, model_name: &str) -> String {
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
