use std::sync::Arc;

use openbitfun_core::infrastructure::ai::AIClientFactory;
use openbitfun_core::service::config::{
    model_runtime_binding_fingerprint, AIModelConfig, ConfigService, ModelCapability, ModelCategory,
};
use openbitfun_sdk_host::host::{TemporaryModelInstallError, TemporaryModelInstaller};
use openbitfun_sdk_host::protocol::{TemporaryModelConfig, TemporaryModelProvider};

pub(crate) struct ConfigTemporaryModelInstaller {
    config: Arc<ConfigService>,
}

impl ConfigTemporaryModelInstaller {
    pub(crate) fn new(config: Arc<ConfigService>) -> Self {
        Self { config }
    }
}

fn resolve_temporary_model(
    model: TemporaryModelConfig,
) -> Result<AIModelConfig, TemporaryModelInstallError> {
    let model_name = model.model.trim().to_string();
    if model_name.is_empty() || model.api_key.trim().is_empty() {
        return Err(TemporaryModelInstallError::InvalidModel);
    }

    let (provider_id, default_base_url) = match model.provider {
        TemporaryModelProvider::Openai => ("openai", "https://api.openai.com/v1"),
        TemporaryModelProvider::Responses => ("responses", "https://api.openai.com/v1"),
        TemporaryModelProvider::Anthropic => ("anthropic", "https://api.anthropic.com"),
        TemporaryModelProvider::Gemini => {
            ("gemini", "https://generativelanguage.googleapis.com/v1beta")
        }
    };
    let base_url = model
        .base_url
        .unwrap_or_else(|| default_base_url.to_string());
    let parsed =
        url::Url::parse(&base_url).map_err(|_| TemporaryModelInstallError::InvalidBaseUrl)?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(TemporaryModelInstallError::InvalidBaseUrl);
    }

    let mut config = AIModelConfig {
        id: String::new(),
        name: model_name.clone(),
        provider: provider_id.to_string(),
        model_name,
        base_url,
        api_key: model.api_key,
        enabled: true,
        category: ModelCategory::GeneralChat,
        capabilities: vec![ModelCapability::TextChat],
        ..AIModelConfig::default()
    };
    let fingerprint = model_runtime_binding_fingerprint(&config);
    config.id = format!("sdk:{provider_id}:{}", &fingerprint[..24]);
    Ok(config)
}

#[async_trait::async_trait]
impl TemporaryModelInstaller for ConfigTemporaryModelInstaller {
    async fn install(
        &self,
        model: TemporaryModelConfig,
    ) -> Result<String, TemporaryModelInstallError> {
        let config = resolve_temporary_model(model)?;
        let model_id = config.id.clone();
        self.config
            .install_runtime_ai_model(config)
            .await
            .map_err(|_| TemporaryModelInstallError::Internal)?;
        Ok(model_id)
    }

    async fn remove(&self, model_id: &str) {
        self.config.remove_runtime_ai_model(model_id).await;
        if let Ok(factory) = AIClientFactory::get_global().await {
            factory.invalidate_model(model_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use openbitfun_core::service::config::{ModelCapability, ModelCategory};
    use openbitfun_sdk_host::host::TemporaryModelInstallError;
    use openbitfun_sdk_host::protocol::{TemporaryModelConfig, TemporaryModelProvider};

    use super::resolve_temporary_model;

    fn temporary_model(
        provider: TemporaryModelProvider,
        api_key: &str,
        base_url: Option<&str>,
    ) -> TemporaryModelConfig {
        TemporaryModelConfig {
            provider,
            model: "fixture-model".to_string(),
            api_key: api_key.to_string(),
            base_url: base_url.map(str::to_string),
        }
    }

    #[test]
    fn provider_defaults_and_minimal_model_fields_are_resolved() {
        let cases = [
            (
                TemporaryModelProvider::Openai,
                "openai",
                "https://api.openai.com/v1",
            ),
            (
                TemporaryModelProvider::Responses,
                "responses",
                "https://api.openai.com/v1",
            ),
            (
                TemporaryModelProvider::Anthropic,
                "anthropic",
                "https://api.anthropic.com",
            ),
            (
                TemporaryModelProvider::Gemini,
                "gemini",
                "https://generativelanguage.googleapis.com/v1beta",
            ),
        ];

        for (provider, provider_id, default_url) in cases {
            let model =
                resolve_temporary_model(temporary_model(provider, "fixture-secret", None)).unwrap();
            assert!(model.id.starts_with(&format!("sdk:{provider_id}:")));
            assert_eq!(model.id.len(), "sdk::".len() + provider_id.len() + 24);
            assert_eq!(model.name, "fixture-model");
            assert_eq!(model.provider, provider_id);
            assert_eq!(model.model_name, "fixture-model");
            assert_eq!(model.base_url, default_url);
            assert_eq!(model.api_key, "fixture-secret");
            assert!(model.enabled);
            assert!(matches!(model.category, ModelCategory::GeneralChat));
            assert_eq!(model.capabilities, vec![ModelCapability::TextChat]);
            assert!(model.request_url.is_none());
            assert!(model.context_window.is_none());
            assert!(model.custom_headers.is_none());
            assert!(model.custom_request_body.is_none());
        }
    }

    #[test]
    fn model_id_is_deterministic_across_api_key_rotation() {
        let first = resolve_temporary_model(temporary_model(
            TemporaryModelProvider::Openai,
            "fixture-secret-one",
            Some("http://127.0.0.1:43123/v1"),
        ))
        .unwrap();
        let second = resolve_temporary_model(temporary_model(
            TemporaryModelProvider::Openai,
            "fixture-secret-two",
            Some("http://127.0.0.1:43123/v1"),
        ))
        .unwrap();

        assert_eq!(first.id, second.id);
        assert!(!first.id.contains("fixture-secret"));
    }

    #[test]
    fn invalid_model_values_fail_without_echoing_input() {
        for base_url in [
            "not-a-url",
            "ftp://example.com/v1",
            "https://user:password@example.com/v1",
            "https://example.com/v1?secret=value",
            "https://example.com/v1#fragment",
        ] {
            assert!(matches!(
                resolve_temporary_model(temporary_model(
                    TemporaryModelProvider::Openai,
                    "fixture-secret",
                    Some(base_url),
                )),
                Err(TemporaryModelInstallError::InvalidBaseUrl)
            ));
        }

        for (model, api_key) in [("", "fixture-secret"), ("fixture-model", "   ")] {
            let mut temporary = temporary_model(
                TemporaryModelProvider::Openai,
                api_key,
                Some("https://example.com/v1"),
            );
            temporary.model = model.to_string();
            assert!(matches!(
                resolve_temporary_model(temporary),
                Err(TemporaryModelInstallError::InvalidModel)
            ));
        }
    }
}
