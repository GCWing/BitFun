//! Attribution resolution for usage statistics.
//!
//! Maps a raw `TokenUsageRecord` to the dimensions shown on the usage
//! statistics page: the provider group (分组) and the endpoint label (端点).
//! Resolution prefers the model configuration that was in effect for the
//! request (`model_config_id` -> provider / base URL) and falls back to the
//! bundled models.dev catalog inferred from the effective model name, so
//! records whose config was later deleted still render meaningfully.

use super::types::TokenUsageRecord;
use crate::service::config::types::AIModelConfig;
use bitfun_ai_adapters::models_dev::ModelsDevCatalog;
use bitfun_services_core::token_usage::UsageAttribution;
use std::collections::HashMap;

/// Resolver owning every lookup table used while attributing usage records.
#[derive(Debug, Default)]
pub struct UsageAttributionResolver {
    /// Model configs by `AIModelConfig.id`.
    configs: HashMap<String, AIModelConfig>,
    /// Provider id -> display name from the models.dev catalog.
    provider_names: HashMap<String, String>,
    /// Effective model name -> (provider id, provider api base URL).
    model_providers: HashMap<String, (String, String)>,
}

impl UsageAttributionResolver {
    pub fn new(models_dev: Option<&ModelsDevCatalog>, configs: &[AIModelConfig]) -> Self {
        let mut resolver = Self {
            configs: configs
                .iter()
                .filter(|config| !config.id.is_empty())
                .map(|config| (config.id.clone(), config.clone()))
                .collect(),
            ..Self::default()
        };

        if let Some(catalog) = models_dev {
            for provider_id in catalog.provider_ids() {
                if let Some(facts) = catalog.provider_facts(&provider_id) {
                    resolver
                        .provider_names
                        .insert(provider_id.clone(), facts.name);
                }
            }
            for (provider_id, model) in catalog.all_models() {
                let api = catalog
                    .provider_facts(&provider_id)
                    .and_then(|facts| facts.api)
                    .unwrap_or_default();
                resolver
                    .model_providers
                    .entry(model.id.clone())
                    .or_insert_with(|| (provider_id.clone(), api));
            }
        }

        resolver
    }

    pub fn attribute(&self, record: &TokenUsageRecord) -> UsageAttribution {
        let config = self.configs.get(&record.model_config_id);
        UsageAttribution {
            group: self.resolve_group(record, config),
            endpoint: self.resolve_endpoint(record, config),
        }
    }

    fn resolve_group(&self, record: &TokenUsageRecord, config: Option<&AIModelConfig>) -> String {
        if let Some(config) = config {
            let provider = config.provider.trim();
            if !provider.is_empty() {
                return self
                    .provider_names
                    .get(provider)
                    .cloned()
                    .unwrap_or_else(|| provider.to_string());
            }
        }
        if let Some((provider_id, _)) = self.model_providers.get(&record.effective_model_name) {
            return self
                .provider_names
                .get(provider_id)
                .cloned()
                .unwrap_or_else(|| provider_id.clone());
        }
        "unknown".to_string()
    }

    fn resolve_endpoint(
        &self,
        record: &TokenUsageRecord,
        config: Option<&AIModelConfig>,
    ) -> String {
        if let Some(config) = config {
            if let Some(request_url) = config
                .request_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                return endpoint_from_base_url(request_url, "");
            }
            let base_url = config.base_url.trim();
            if !base_url.is_empty() {
                return endpoint_from_base_url(base_url, provider_endpoint_path(&config.provider));
            }
        }
        if let Some((_, api)) = self.model_providers.get(&record.effective_model_name) {
            if !api.is_empty() {
                return endpoint_from_base_url(api, "/v1/chat/completions");
            }
        }
        "/unknown".to_string()
    }
}

/// Canonical request path for a provider's API format.
fn provider_endpoint_path(provider: &str) -> &'static str {
    let provider = provider.trim().to_ascii_lowercase();
    if provider.contains("anthropic") {
        "/v1/messages"
    } else if provider.contains("gemini") || provider.contains("google") {
        "/v1beta/models:generateContent"
    } else {
        "/v1/chat/completions"
    }
}

/// Build the endpoint label from a base URL (scheme stripped) plus the request
/// path. When `path` is empty the URL is used as-is; a missing chat-completions
/// suffix is appended as a last resort so the label reads like an endpoint.
fn endpoint_from_base_url(base_url: &str, path: &str) -> String {
    let host = strip_scheme(base_url.trim().trim_end_matches('/'));
    if host.is_empty() {
        return "/unknown".to_string();
    }
    if host.ends_with("/chat/completions") || host.ends_with("/v1/messages") {
        return host;
    }
    if path.is_empty() {
        return host;
    }
    // Avoid duplicating "/v1" when the base URL already carries it
    // (e.g. "https://api.example.com/v1" + "/v1/chat/completions").
    if let Some(rest) = path.strip_prefix("/v1") {
        if host.ends_with("/v1") {
            return format!("{host}{rest}");
        }
    }
    format!("{host}{path}")
}

fn strip_scheme(url: &str) -> String {
    let url = url.trim();
    for scheme in ["https://", "http://"] {
        if let Some(rest) = url.strip_prefix(scheme) {
            return rest.trim_end_matches('/').to_string();
        }
    }
    url.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_ai_adapters::models_dev::ModelsDevCatalog;

    fn catalog() -> ModelsDevCatalog {
        ModelsDevCatalog::parse_str(
            r#"{
                "deepseek": {
                    "id": "deepseek", "name": "DeepSeek",
                    "api": "https://api.deepseek.com",
                    "models": {
                        "deepseek-v4-flash": {
                            "modalities": {"input": ["text"], "output": ["text"]},
                            "cost": {"input": 0.27, "output": 1.1, "cache_read": 0.027, "cache_write": 0.27}
                        },
                        "deepseek-v4-pro": {
                            "modalities": {"input": ["text"], "output": ["text"]},
                            "cost": {"input": 2.0, "output": 8.0}
                        }
                    }
                }
            }"#,
        )
        .expect("catalog")
    }

    fn config(
        id: &str,
        provider: &str,
        base_url: &str,
        request_url: Option<&str>,
    ) -> AIModelConfig {
        AIModelConfig {
            id: id.to_string(),
            name: "config".to_string(),
            provider: provider.to_string(),
            model_name: "deepseek-v4-flash".to_string(),
            base_url: base_url.to_string(),
            request_url: request_url.map(|value| value.to_string()),
            api_key: String::new(),
            context_window: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            enabled: true,
            category: crate::service::config::types::ModelCategory::GeneralChat,
            capabilities: Vec::new(),
            recommended_for: Vec::new(),
            metadata: None,
            reasoning: None,
            inline_think_in_text: true,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            custom_request_body: None,
            custom_request_body_mode: None,
            auth: crate::service::config::types::AuthConfig::ApiKey,
        }
    }

    fn record(config_id: &str, model: &str) -> TokenUsageRecord {
        TokenUsageRecord {
            model_config_id: config_id.to_string(),
            effective_model_name: model.to_string(),
            session_id: "session".to_string(),
            turn_id: "turn".to_string(),
            timestamp: chrono::Utc::now(),
            input_tokens: 100,
            output_tokens: 50,
            cached_tokens: 0,
            cached_tokens_available: false,
            cache_write_tokens: 0,
            total_tokens: 150,
            token_details: None,
            is_subagent: false,
        }
    }

    #[test]
    fn config_wins_for_group_and_endpoint() {
        let resolver = UsageAttributionResolver::new(
            Some(&catalog()),
            &[config(
                "cfg-1",
                "deepseek",
                "https://api.deepseek.com",
                Some("https://api.deepseek.com/chat/completions"),
            )],
        );
        let attribution = resolver.attribute(&record("cfg-1", "deepseek-v4-flash"));
        assert_eq!(attribution.group, "DeepSeek");
        assert_eq!(attribution.endpoint, "api.deepseek.com/chat/completions");
    }

    #[test]
    fn request_url_absent_derives_endpoint_from_base_url_and_provider() {
        let resolver = UsageAttributionResolver::new(
            Some(&catalog()),
            &[config(
                "cfg-1",
                "deepseek",
                "https://api.deepseek.com",
                None,
            )],
        );
        let attribution = resolver.attribute(&record("cfg-1", "deepseek-v4-flash"));
        assert_eq!(attribution.endpoint, "api.deepseek.com/v1/chat/completions");
    }

    #[test]
    fn missing_config_falls_back_to_catalog_inference() {
        let resolver = UsageAttributionResolver::new(Some(&catalog()), &[]);
        let attribution = resolver.attribute(&record("deleted-config", "deepseek-v4-pro"));
        assert_eq!(attribution.group, "DeepSeek");
        assert_eq!(attribution.endpoint, "api.deepseek.com/v1/chat/completions");
    }

    #[test]
    fn unknown_model_yields_unknown_attribution() {
        let resolver = UsageAttributionResolver::new(Some(&catalog()), &[]);
        let attribution = resolver.attribute(&record("deleted-config", "custom-model"));
        assert_eq!(attribution.group, "unknown");
        assert_eq!(attribution.endpoint, "/unknown");
    }

    #[test]
    fn empty_catalog_still_resolves_config_provider() {
        let resolver = UsageAttributionResolver::new(
            None,
            &[config(
                "cfg-1",
                "my-provider",
                "https://gateway.example.com/v1",
                None,
            )],
        );
        let attribution = resolver.attribute(&record("cfg-1", "deepseek-v4-flash"));
        assert_eq!(attribution.group, "my-provider");
        assert_eq!(
            attribution.endpoint,
            "gateway.example.com/v1/chat/completions"
        );
    }
}
