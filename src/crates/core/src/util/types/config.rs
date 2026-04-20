use crate::service::config::types::AIModelConfig;
pub use bitfun_ai_adapters::types::resolve_request_url;
pub use bitfun_ai_adapters::types::AIConfig;
use log::warn;

impl TryFrom<AIModelConfig> for AIConfig {
    type Error = String;

    fn try_from(other: AIModelConfig) -> Result<Self, Self::Error> {
        let reasoning_mode = other.effective_reasoning_mode();

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

        let request_url = other
            .request_url
            .clone()
            .filter(|url| !url.is_empty())
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
            reasoning_mode,
            inline_think_in_text: other.inline_think_in_text,
            custom_headers: other.custom_headers,
            custom_headers_mode: other.custom_headers_mode,
            skip_ssl_verify: other.skip_ssl_verify,
            reasoning_effort: other.reasoning_effort,
            thinking_budget_tokens: other.thinking_budget_tokens,
            custom_request_body,
            custom_request_body_mode: other.custom_request_body_mode,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_request_url, AIConfig};
    use crate::service::config::types::{AIModelConfig, ModelCategory, ReasoningMode};

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

    fn base_model_config() -> AIModelConfig {
        AIModelConfig {
            id: "model_1".to_string(),
            name: "Provider".to_string(),
            provider: "openai".to_string(),
            model_name: "test-model".to_string(),
            base_url: "https://example.com/v1".to_string(),
            request_url: Some("https://example.com/v1/chat/completions".to_string()),
            api_key: "key".to_string(),
            context_window: Some(128000),
            max_tokens: Some(4096),
            temperature: None,
            top_p: None,
            enabled: true,
            category: ModelCategory::GeneralChat,
            capabilities: vec![],
            recommended_for: vec![],
            metadata: None,
            enable_thinking_process: false,
            reasoning_mode: None,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            reasoning_effort: None,
            thinking_budget_tokens: None,
            custom_request_body: None,
            custom_request_body_mode: None,
            auth: Default::default(),
        }
    }

    #[test]
    fn compatibility_false_thinking_maps_to_default_mode() {
        let config = AIConfig::try_from(base_model_config()).expect("conversion should succeed");
        assert_eq!(config.reasoning_mode, ReasoningMode::Default);
    }

    #[test]
    fn compatibility_true_thinking_maps_to_enabled_mode() {
        let mut model = base_model_config();
        model.enable_thinking_process = true;

        let config = AIConfig::try_from(model).expect("conversion should succeed");
        assert_eq!(config.reasoning_mode, ReasoningMode::Enabled);
    }
}
