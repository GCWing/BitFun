//! AI client factory - centrally manages client instances for all models
//!
//! Responsibilities:
//! 1. Create and cache AI clients on demand
//! 2. Manage agent model configuration
//! 3. Invalidate cache when configuration changes
//! 4. Provide global singleton access

use crate::infrastructure::ai::reasoning_catalog::{
    apply_default_reasoning_preset, apply_selected_reasoning_preset,
    load_models_dev_reasoning_catalog, project_model_reasoning_catalog,
    resolve_default_reasoning_preset,
};
use crate::infrastructure::ai::{build_stream_options_for_model, AIClient};
#[cfg(feature = "subscription-auth")]
use crate::infrastructure::subscription_auth::{
    self, OpenCodePlan as AdapterOpenCodePlan, SubscriptionHttpOptions,
    SubscriptionProvider as AdapterProvider,
};
use crate::service::config::types::{model_runtime_binding_fingerprint, AuthConfig};
#[cfg(feature = "subscription-auth")]
use crate::service::config::types::{OpenCodePlan, SubscriptionProvider};
use crate::service::config::{get_global_config_service, ConfigService};
use crate::util::errors::{OpenBitFunError, OpenBitFunResult};
use crate::util::types::AIConfig;
use anyhow::{anyhow, Result};
use log::{debug, info, warn};
use openbitfun_ai_adapters::resolve_required_model_selector;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

pub struct AIClientFactory {
    config_service: Arc<ConfigService>,
    client_cache: RwLock<HashMap<String, CachedAIClient>>,
}

struct CachedAIClient {
    configuration_fingerprint: String,
    default_reasoning_preset: Option<openbitfun_core_types::ReasoningPresetDescriptor>,
    client: Arc<AIClient>,
    /// Unix seconds when the resolved subscription credential expires.
    #[cfg(feature = "subscription-auth")]
    credential_expires_at: Option<i64>,
}

/// Once a cached subscription credential is within this window of expiry, the
/// client is rebuilt so subscription authentication refreshes the token. Kept
/// equal to the providers' refresh leeway so the rebuilt client always gets a
/// fresh token.
#[cfg(feature = "subscription-auth")]
const SUBSCRIPTION_CREDENTIAL_STALE_LEEWAY_SECS: i64 = 5 * 60;

#[cfg(feature = "subscription-auth")]
fn now_unix_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(feature = "subscription-auth")]
fn subscription_credential_stale(auth: &AuthConfig, cached: &CachedAIClient) -> bool {
    if !matches!(auth, AuthConfig::Subscription { .. }) {
        return false;
    }
    cached.credential_expires_at.is_some_and(|expires_at| {
        expires_at <= now_unix_secs() + SUBSCRIPTION_CREDENTIAL_STALE_LEEWAY_SECS
    })
}

#[cfg(not(feature = "subscription-auth"))]
fn subscription_credential_stale(auth: &AuthConfig, _cached: &CachedAIClient) -> bool {
    matches!(auth, AuthConfig::Subscription { .. })
}

impl AIClientFactory {
    fn new(config_service: Arc<ConfigService>) -> Self {
        Self {
            config_service,
            client_cache: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get_client_by_id(&self, model_id: &str) -> Result<Arc<AIClient>> {
        self.get_or_create_client(model_id, None).await
    }

    /// Resolve the fixed model configured for Git commit-message generation.
    pub async fn get_git_commit_task_client(&self) -> Result<Arc<AIClient>> {
        let global_config: crate::service::config::GlobalConfig =
            self.config_service.get_config(None).await?;
        let model_id = global_config
            .ai
            .task_models
            .git_commit
            .fixed_model_id()
            .ok_or_else(|| anyhow!("Git commit task model cannot inherit a session model"))?;
        self.get_client_resolved(model_id).await
    }

    /// Resolves an immutable concrete model id only when its current runtime
    /// identity still matches the user-approved binding.
    pub async fn get_client_by_approved_binding(
        &self,
        model_id: &str,
        configuration_fingerprint: &str,
    ) -> Result<Arc<AIClient>> {
        self.get_or_create_client(model_id, Some(configuration_fingerprint))
            .await
    }

    /// Resolve an approved base client and apply one session preset without
    /// inserting the derived client into the factory cache.
    pub async fn get_client_by_approved_binding_with_reasoning_preset(
        &self,
        model_id: &str,
        configuration_fingerprint: &str,
        reasoning_preset: Option<&str>,
    ) -> Result<Arc<AIClient>> {
        let client = self
            .get_client_by_approved_binding(model_id, configuration_fingerprint)
            .await?;
        self.apply_session_reasoning_preset(model_id, client, reasoning_preset)
            .await
    }

    /// Get a client (supports resolving primary/fast)
    pub async fn get_client_resolved(&self, model_id: &str) -> Result<Arc<AIClient>> {
        let resolved_model_id = self.resolve_model_id(model_id).await?;
        self.get_or_create_client(&resolved_model_id, None).await
    }

    /// Resolve a base client and apply one session preset without caching the
    /// derived client. An unknown preset fails closed to the model default.
    pub async fn get_client_resolved_with_reasoning_preset(
        &self,
        model_id: &str,
        reasoning_preset: Option<&str>,
    ) -> Result<Arc<AIClient>> {
        let resolved_model_id = self.resolve_model_id(model_id).await?;
        let client = self.get_or_create_client(&resolved_model_id, None).await?;
        self.apply_session_reasoning_preset(&resolved_model_id, client, reasoning_preset)
            .await
    }

    async fn resolve_model_id(&self, model_id: &str) -> Result<String> {
        let ai_config = self.config_service.get_effective_ai_config().await?;
        resolve_required_model_selector(
            model_id,
            |selector| ai_config.resolve_model_selection(selector),
            |model_ref| ai_config.resolve_model_reference(model_ref),
        )
        .map_err(|error| anyhow!(error.to_string()))
    }

    async fn apply_session_reasoning_preset(
        &self,
        model_id: &str,
        client: Arc<AIClient>,
        reasoning_preset: Option<&str>,
    ) -> Result<Arc<AIClient>> {
        let Some(reasoning_preset) = reasoning_preset
            .map(str::trim)
            .filter(|preset| !preset.is_empty())
        else {
            return Ok(client);
        };
        let ai_config = self.config_service.get_effective_ai_config().await?;
        let model = ai_config
            .models
            .iter()
            .find(|model| model.id == model_id)
            .ok_or_else(|| anyhow!("Model configuration not found: {}", model_id))?;
        let models_dev = load_models_dev_reasoning_catalog().await;
        let projection = project_model_reasoning_catalog(model, models_dev.catalog.as_deref());
        let Some(client) = apply_selected_reasoning_preset(&client, &projection, reasoning_preset)
        else {
            warn!(
                "Session reasoning preset is not available for the resolved model; falling back to model default: model_id={}, preset_id={}",
                model_id, reasoning_preset
            );
            return Ok(client);
        };

        Ok(Arc::new(client))
    }

    pub fn invalidate_cache(&self) {
        let mut cache = match self.client_cache.write() {
            Ok(cache) => cache,
            Err(poisoned) => {
                warn!("AI client cache write lock poisoned during invalidate_cache, recovering");
                poisoned.into_inner()
            }
        };
        let count = cache.len();
        cache.clear();
        info!("AI client cache cleared (removed {} clients)", count);
    }

    pub fn get_cache_size(&self) -> usize {
        let cache = match self.client_cache.read() {
            Ok(cache) => cache,
            Err(poisoned) => {
                warn!("AI client cache read lock poisoned during get_cache_size, recovering");
                poisoned.into_inner()
            }
        };
        cache.len()
    }

    pub fn invalidate_model(&self, model_id: &str) {
        let mut cache = match self.client_cache.write() {
            Ok(cache) => cache,
            Err(poisoned) => {
                warn!("AI client cache write lock poisoned during invalidate_model, recovering");
                poisoned.into_inner()
            }
        };
        if cache.remove(model_id).is_some() {
            debug!("Client cache cleared for model: {}", model_id);
        }
    }

    async fn get_or_create_client(
        &self,
        model_id: &str,
        expected_configuration_fingerprint: Option<&str>,
    ) -> Result<Arc<AIClient>> {
        let global_config: crate::service::config::GlobalConfig =
            self.config_service.get_config(None).await?;
        let normalized_model_id = model_id.trim().to_string();
        if normalized_model_id.is_empty() {
            return Err(anyhow!("Model configuration id is empty"));
        }
        debug!("Creating new AI client: model_id={}", normalized_model_id);
        let model_config = if let Some(runtime_model) = self
            .config_service
            .get_runtime_ai_model(&normalized_model_id)
            .await
        {
            runtime_model
        } else {
            let mut matching_models = global_config
                .ai
                .models
                .iter()
                .filter(|model| model.id == normalized_model_id);
            let model = matching_models
                .next()
                .cloned()
                .ok_or_else(|| anyhow!("Model configuration not found: {}", normalized_model_id))?;
            if matching_models.next().is_some() {
                return Err(anyhow!(
                    "Multiple model configurations use the same ID: {}",
                    normalized_model_id
                ));
            }
            model
        };

        if !model_config.enabled {
            return Err(anyhow!(
                "Model '{}' (id={}) is currently disabled; enable it in settings or pick another model",
                model_config.name,
                model_config.id
            ));
        }

        let configuration_fingerprint = model_runtime_binding_fingerprint(&model_config);
        if expected_configuration_fingerprint
            .is_some_and(|expected| expected != configuration_fingerprint)
        {
            return Err(anyhow!(
                "Approved model binding changed for configuration id: {}",
                normalized_model_id
            ));
        }

        let models_dev = load_models_dev_reasoning_catalog().await;
        let reasoning_projection =
            project_model_reasoning_catalog(&model_config, models_dev.catalog.as_deref());
        let default_reasoning_preset =
            resolve_default_reasoning_preset(&reasoning_projection).cloned();

        {
            let cache = match self.client_cache.read() {
                Ok(cache) => cache,
                Err(poisoned) => {
                    warn!(
                        "AI client cache read lock poisoned during get_or_create_client, recovering"
                    );
                    poisoned.into_inner()
                }
            };
            if let Some(cached) = cache.get(&normalized_model_id) {
                if cached.configuration_fingerprint == configuration_fingerprint
                    && cached.default_reasoning_preset == default_reasoning_preset
                    && !subscription_credential_stale(&model_config.auth, cached)
                {
                    return Ok(cached.client.clone());
                }
            }
        }

        let mut ai_config = AIConfig::try_from(model_config.clone())
            .map_err(|e| anyhow!("AI configuration conversion failed: {}", e))?;
        let skip_ssl_verify = ai_config.skip_ssl_verify;
        let proxy_config = if global_config.ai.proxy.enabled {
            Some(global_config.ai.proxy.clone())
        } else {
            None
        };
        let credential_expires_at = apply_configured_auth(
            &model_config.auth,
            &mut ai_config,
            proxy_config.clone(),
            skip_ssl_verify,
        )
        .await?;
        #[cfg(not(feature = "subscription-auth"))]
        let _ = credential_expires_at;

        let stream_options = build_stream_options_for_model(&global_config.ai, Some(&model_config));
        let client = apply_default_reasoning_preset(
            AIClient::new_with_runtime_options(ai_config, proxy_config, stream_options),
            &reasoning_projection,
        );
        let client = Arc::new(client);

        {
            let mut cache = match self.client_cache.write() {
                Ok(cache) => cache,
                Err(poisoned) => {
                    warn!(
                        "AI client cache write lock poisoned during get_or_create_client, recovering"
                    );
                    poisoned.into_inner()
                }
            };
            cache.insert(
                model_config.id.clone(),
                CachedAIClient {
                    configuration_fingerprint,
                    default_reasoning_preset,
                    client: client.clone(),
                    #[cfg(feature = "subscription-auth")]
                    credential_expires_at,
                },
            );
        }

        debug!(
            "AI client created: model_id={}, name={}",
            model_config.id, model_config.name
        );

        Ok(client)
    }
}

static GLOBAL_AI_CLIENT_FACTORY: OnceLock<Arc<tokio::sync::RwLock<Option<Arc<AIClientFactory>>>>> =
    OnceLock::new();

impl AIClientFactory {
    /// Initialize the global AIClientFactory singleton
    pub async fn initialize_global() -> OpenBitFunResult<()> {
        if Self::is_global_initialized() {
            return Ok(());
        }

        info!("Initializing global AIClientFactory...");

        let config_service = get_global_config_service().await.map_err(|e| {
            OpenBitFunError::service(format!("Failed to get global config service: {}", e))
        })?;

        let factory = Arc::new(AIClientFactory::new(config_service));
        let wrapper = Arc::new(tokio::sync::RwLock::new(Some(factory)));

        GLOBAL_AI_CLIENT_FACTORY.set(wrapper).map_err(|_| {
            OpenBitFunError::service("Failed to initialize global AIClientFactory".to_string())
        })?;

        info!("Global AIClientFactory initialized");
        Ok(())
    }

    /// Get the global AIClientFactory instance
    pub async fn get_global() -> OpenBitFunResult<Arc<AIClientFactory>> {
        let wrapper = GLOBAL_AI_CLIENT_FACTORY.get().ok_or_else(|| {
            OpenBitFunError::service(
                "Global AIClientFactory not initialized. Call initialize_global() first."
                    .to_string(),
            )
        })?;

        let guard = wrapper.read().await;
        guard
            .as_ref()
            .ok_or_else(|| OpenBitFunError::service("Global AIClientFactory is None".to_string()))
            .map(Arc::clone)
    }

    pub fn is_global_initialized() -> bool {
        GLOBAL_AI_CLIENT_FACTORY.get().is_some()
    }

    /// Update the global AIClientFactory instance (used for config reload)
    pub async fn update_global(new_factory: Arc<AIClientFactory>) -> OpenBitFunResult<()> {
        let wrapper = GLOBAL_AI_CLIENT_FACTORY.get().ok_or_else(|| {
            OpenBitFunError::service("Global AIClientFactory not initialized".to_string())
        })?;

        {
            let mut guard = wrapper.write().await;
            *guard = Some(new_factory);
        }

        debug!("Global AIClientFactory updated");
        Ok(())
    }
}

pub async fn get_global_ai_client_factory() -> OpenBitFunResult<Arc<AIClientFactory>> {
    AIClientFactory::get_global().await
}

pub async fn initialize_global_ai_client_factory() -> OpenBitFunResult<()> {
    AIClientFactory::initialize_global().await
}

#[cfg(feature = "subscription-auth")]
fn to_adapter_provider(provider: SubscriptionProvider) -> AdapterProvider {
    match provider {
        SubscriptionProvider::Codex => AdapterProvider::Codex,
        SubscriptionProvider::Antigravity => AdapterProvider::Antigravity,
        SubscriptionProvider::Opencode => AdapterProvider::Opencode,
        SubscriptionProvider::Grok => AdapterProvider::Grok,
        SubscriptionProvider::Hermes => AdapterProvider::Hermes,
    }
}

#[cfg(feature = "subscription-auth")]
fn to_adapter_opencode_plan(plan: OpenCodePlan) -> AdapterOpenCodePlan {
    match plan {
        OpenCodePlan::Zen => AdapterOpenCodePlan::Zen,
        OpenCodePlan::Go => AdapterOpenCodePlan::Go,
    }
}

/// Resolve a subscription `AuthConfig` and overlay it onto the runtime
/// `AIConfig`. No-op when `auth == AuthConfig::ApiKey`. Returns the resolved
/// credential's expiry (Unix seconds) so callers can invalidate cached
/// clients before the token goes stale.
pub async fn apply_subscription_auth(
    auth: &AuthConfig,
    ai_config: &mut AIConfig,
) -> Result<Option<i64>> {
    #[cfg(feature = "subscription-auth")]
    return apply_subscription_auth_with_options(
        auth,
        ai_config,
        &SubscriptionHttpOptions::default(),
    )
    .await;

    #[cfg(not(feature = "subscription-auth"))]
    {
        let _ = ai_config;
        match auth {
            AuthConfig::ApiKey => Ok(None),
            AuthConfig::Subscription { .. } => Err(anyhow!(
                "Subscription authentication is not available in this product build"
            )),
        }
    }
}

#[cfg(feature = "subscription-auth")]
async fn apply_configured_auth(
    auth: &AuthConfig,
    ai_config: &mut AIConfig,
    proxy_config: Option<openbitfun_core_types::ProxyConfig>,
    skip_ssl_verify: bool,
) -> Result<Option<i64>> {
    let options = SubscriptionHttpOptions::new(proxy_config, skip_ssl_verify);
    apply_subscription_auth_with_options(auth, ai_config, &options).await
}

#[cfg(not(feature = "subscription-auth"))]
async fn apply_configured_auth(
    auth: &AuthConfig,
    ai_config: &mut AIConfig,
    _proxy_config: Option<openbitfun_core_types::ProxyConfig>,
    _skip_ssl_verify: bool,
) -> Result<Option<i64>> {
    apply_subscription_auth(auth, ai_config).await
}

/// Resolves subscription authentication with an explicit transport policy.
#[cfg(feature = "subscription-auth")]
pub async fn apply_subscription_auth_with_options(
    auth: &AuthConfig,
    ai_config: &mut AIConfig,
    options: &SubscriptionHttpOptions,
) -> Result<Option<i64>> {
    let resolved = match auth {
        AuthConfig::ApiKey => return Ok(None),
        AuthConfig::Subscription { provider, plan } => {
            let adapter_provider = to_adapter_provider(*provider);
            if let Some(model) =
                subscription_auth::runtime_model_override(adapter_provider, &ai_config.model)
            {
                ai_config.model = model.to_string();
            }
            let resolved = match (*provider, *plan) {
                (SubscriptionProvider::Opencode, Some(plan)) => {
                    subscription_auth::resolve_opencode_with_options(
                        to_adapter_opencode_plan(plan),
                        &ai_config.format,
                        options,
                    )
                    .await
                }
                (SubscriptionProvider::Grok, None) => {
                    subscription_auth::resolve_grok_with_options(&ai_config.model, options).await
                }
                (SubscriptionProvider::Hermes, None) => {
                    subscription_auth::resolve_hermes_with_options(&ai_config.model, options).await
                }
                (_, None) => {
                    subscription_auth::resolve_with_options(adapter_provider, options).await
                }
                (_, Some(plan)) => Err(anyhow!(
                    "OpenCode plan {plan:?} cannot be used with provider {provider:?}"
                )),
            };
            resolved.map_err(|e| {
                anyhow!(
                    "Failed to resolve {provider:?} subscription credential: {e:#}. \
                     Subscription logins are stored on the local machine and are not \
                     available in remote workspaces."
                )
            })?
        }
    };

    ai_config.api_key = resolved.api_key;
    if let Some(base) = resolved.base_url {
        ai_config.base_url = base;
    }
    if let Some(req) = resolved.request_url {
        ai_config.request_url = req;
    }
    if let Some(format) = resolved.format {
        ai_config.format = format;
    }
    if !resolved.extra_headers.is_empty() {
        let merged = match ai_config.custom_headers.take() {
            Some(mut existing) => {
                for (k, v) in resolved.extra_headers {
                    existing.insert(k, v);
                }
                existing
            }
            None => resolved.extra_headers,
        };
        ai_config.custom_headers = Some(merged);
        // Default to merge so adapter-specific headers (Authorization etc.) are
        // still applied alongside the injected ones.
        if ai_config.custom_headers_mode.is_none() {
            ai_config.custom_headers_mode = Some("merge".to_string());
        }
    }
    Ok(resolved.expires_at)
}

/// List subscription accounts (Codex / Antigravity / OpenCode / xAI / Hermes).
#[cfg(feature = "subscription-auth")]
pub async fn list_subscription_accounts() -> Vec<subscription_auth::SubscriptionAccount> {
    subscription_auth::list_accounts().await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{apply_subscription_auth, AIClientFactory};
    use crate::infrastructure::PathManager;
    #[cfg(not(feature = "subscription-auth"))]
    use crate::service::config::types::SubscriptionProvider;
    use crate::service::config::types::{
        model_runtime_binding_fingerprint, AIModelConfig, AuthConfig, GlobalConfig,
    };
    use crate::service::config::{ConfigManagerSettings, ConfigService};
    use crate::util::types::AIConfig;
    use openbitfun_ai_adapters::{
        classify_model_selector, resolve_required_model_selector, ModelSelectorKind,
    };

    fn build_model(id: &str, name: &str, model_name: &str) -> AIModelConfig {
        AIModelConfig {
            id: id.to_string(),
            name: name.to_string(),
            model_name: model_name.to_string(),
            provider: "anthropic".to_string(),
            enabled: true,
            ..Default::default()
        }
    }

    fn test_runtime_ai_config() -> AIConfig {
        AIConfig {
            name: "test".to_string(),
            base_url: "https://example.test".to_string(),
            request_url: String::new(),
            api_key: "unchanged".to_string(),
            model: "test-model".to_string(),
            format: "openai".to_string(),
            context_window: 4096,
            max_tokens: None,
            temperature: None,
            top_p: None,
            inline_think_in_text: false,
            custom_headers: None,
            custom_headers_mode: None,
            skip_ssl_verify: false,
            custom_request_body: None,
            custom_request_body_mode: None,
        }
    }

    #[tokio::test]
    async fn runtime_model_is_available_to_ai_client_factory() {
        let dir = tempfile::tempdir().expect("temporary config directory");
        let config = Arc::new(
            ConfigService::with_settings(ConfigManagerSettings {
                path_manager: Some(Arc::new(PathManager::with_user_root_for_tests(
                    dir.path().join("runtime-client"),
                ))),
                auto_save: true,
                backup_count: 0,
            })
            .await
            .expect("test ConfigService"),
        );
        let mut model = build_model("sdk:openai:fixture", "SDK fixture", "fixture-model");
        model.provider = "openai".to_string();
        model.base_url = "http://127.0.0.1:43123/v1".to_string();
        model.api_key = "fixture-secret".to_string();
        config.install_runtime_ai_model(model).await.unwrap();

        AIClientFactory::new(config)
            .get_client_by_id("sdk:openai:fixture")
            .await
            .expect("runtime model should resolve through the AI client factory");
    }

    #[cfg(feature = "subscription-auth")]
    #[tokio::test]
    async fn api_key_auth_remains_a_noop_when_subscription_support_is_compiled() {
        let mut config = test_runtime_ai_config();

        let expires_at = apply_subscription_auth(&AuthConfig::ApiKey, &mut config)
            .await
            .expect("API-key auth");

        assert_eq!(expires_at, None);
        assert_eq!(config.api_key, "unchanged");
        assert_eq!(config.base_url, "https://example.test");
    }

    #[cfg(not(feature = "subscription-auth"))]
    #[tokio::test]
    async fn subscription_auth_fails_closed_when_not_compiled() {
        let auth = AuthConfig::Subscription {
            provider: SubscriptionProvider::Codex,
            plan: None,
        };
        let mut config = test_runtime_ai_config();

        let error = apply_subscription_auth(&auth, &mut config)
            .await
            .expect_err("subscription auth must not degrade to an API-key client");

        assert!(error
            .to_string()
            .contains("Subscription authentication is not available"));
        assert_eq!(config.api_key, "unchanged");
    }

    #[test]
    fn resolve_model_reference_requires_a_config_id() {
        let mut config = GlobalConfig::default();
        config.ai.models = vec![build_model(
            "model-123",
            "Primary Chat",
            "claude-sonnet-4.5",
        )];

        assert_eq!(
            config.ai.resolve_model_reference("model-123"),
            Some("model-123".to_string())
        );
        assert_eq!(config.ai.resolve_model_reference("Primary Chat"), None);
        assert_eq!(config.ai.resolve_model_reference("claude-sonnet-4.5"), None);

        config.ai.models.push(build_model(
            "model-123",
            "Duplicate Config",
            "claude-sonnet-4.5-duplicate",
        ));
        assert_eq!(config.ai.resolve_model_reference("model-123"), None);
    }

    #[test]
    fn concrete_reserved_model_ids_remain_exact_config_references() {
        let mut config = GlobalConfig::default();
        config.ai.models = ["inherit", "primary", "fast", "default"]
            .into_iter()
            .map(|id| build_model(id, id, &format!("runtime-{id}")))
            .collect();

        for id in ["inherit", "primary", "fast", "default"] {
            assert_eq!(
                config.ai.resolve_model_reference(id),
                Some(id.to_string()),
                "approved concrete ids must bypass selector classification"
            );
        }
    }

    #[test]
    fn runtime_binding_fingerprint_tracks_identity_but_not_secret_rotation() {
        let mut model = build_model("model-123", "Provider", "runtime-model");
        model.base_url = "https://models.example/v1".to_string();
        model.api_key = "secret-one".to_string();
        let first = model_runtime_binding_fingerprint(&model);

        model.api_key = "secret-two".to_string();
        assert_eq!(model_runtime_binding_fingerprint(&model), first);

        model.base_url = "https://models.example/v2".to_string();
        assert_ne!(model_runtime_binding_fingerprint(&model), first);
    }

    #[test]
    fn default_and_empty_selectors_normalize_to_primary_for_client_lookup() {
        assert_eq!(
            classify_model_selector(" default "),
            ModelSelectorKind::Primary
        );
        assert_eq!(classify_model_selector(""), ModelSelectorKind::Primary);
        assert_eq!(
            classify_model_selector("model-primary"),
            ModelSelectorKind::Explicit("model-primary".to_string())
        );
    }

    #[test]
    fn resolve_fast_selection_falls_back_to_primary_when_fast_missing() {
        let mut config = GlobalConfig::default();
        config.ai.models = vec![build_model(
            "model-primary",
            "Primary Chat",
            "claude-sonnet-4.5",
        )];
        config.ai.default_models.primary = Some("model-primary".to_string());

        assert_eq!(
            resolve_required_model_selector(
                "fast",
                |selector| config.ai.resolve_model_selection(selector),
                |model_ref| config.ai.resolve_model_reference(model_ref),
            )
            .expect("fast should fall back to primary"),
            "model-primary"
        );
    }

    #[test]
    fn resolve_fast_selection_falls_back_to_primary_when_fast_is_stale() {
        let mut config = GlobalConfig::default();
        config.ai.models = vec![build_model(
            "model-primary",
            "Primary Chat",
            "claude-sonnet-4.5",
        )];
        config.ai.default_models.primary = Some("model-primary".to_string());
        config.ai.default_models.fast = Some("deleted-fast-model".to_string());

        assert_eq!(
            resolve_required_model_selector(
                "fast",
                |selector| config.ai.resolve_model_selection(selector),
                |model_ref| config.ai.resolve_model_reference(model_ref),
            )
            .expect("stale fast should fall back to primary"),
            "model-primary"
        );
    }
}
