//! AI client factory - centrally manages client instances for all models
//!
//! Responsibilities:
//! 1. Create and cache AI clients on demand
//! 2. Manage agent model configuration
//! 3. Invalidate cache when configuration changes
//! 4. Provide global singleton access

use crate::infrastructure::ai::{build_stream_options_for_model, AIClient};
use crate::infrastructure::subscription_auth::{self, SubscriptionProvider as AdapterProvider};
use crate::service::config::types::{
    model_runtime_binding_fingerprint, AuthConfig, SubscriptionProvider,
};
use crate::service::config::{get_global_config_service, ConfigService};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::AIConfig;
use anyhow::{anyhow, Result};
use bitfun_ai_adapters::resolve_required_model_selector;
use log::{debug, info, warn};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

pub struct AIClientFactory {
    config_service: Arc<ConfigService>,
    client_cache: RwLock<HashMap<String, CachedAIClient>>,
}

struct CachedAIClient {
    configuration_fingerprint: String,
    client: Arc<AIClient>,
    /// Unix seconds when the resolved subscription credential expires;
    /// `None` for API-key auth or non-expiring credentials.
    credential_expires_at: Option<i64>,
}

/// Once a cached subscription credential is within this window of expiry, the
/// client is rebuilt so `apply_subscription_auth` refreshes the token. Kept
/// equal to the providers' refresh leeway so the rebuilt client always gets a
/// fresh token.
const SUBSCRIPTION_CREDENTIAL_STALE_LEEWAY_SECS: i64 = 5 * 60;

fn now_unix_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn subscription_credential_stale(auth: &AuthConfig, cached: &CachedAIClient) -> bool {
    if !matches!(auth, AuthConfig::Subscription { .. }) {
        return false;
    }
    cached.credential_expires_at.is_some_and(|expires_at| {
        expires_at <= now_unix_secs() + SUBSCRIPTION_CREDENTIAL_STALE_LEEWAY_SECS
    })
}

fn functional_agent_model_selector<'a>(
    ai_config: &'a crate::service::config::types::AIConfig,
    func_agent_name: &str,
) -> &'a str {
    ai_config
        .func_agent_models
        .get(func_agent_name)
        .map(String::as_str)
        .unwrap_or("fast")
}

impl AIClientFactory {
    fn new(config_service: Arc<ConfigService>) -> Self {
        Self {
            config_service,
            client_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Get a functional agent's AI client using its dedicated mapping or fast.
    pub async fn get_client_by_func_agent(&self, func_agent_name: &str) -> Result<Arc<AIClient>> {
        let global_config: crate::service::config::GlobalConfig =
            self.config_service.get_config(None).await?;

        let model_id = functional_agent_model_selector(&global_config.ai, func_agent_name);

        self.get_client_resolved(model_id).await
    }

    pub async fn get_client_by_id(&self, model_id: &str) -> Result<Arc<AIClient>> {
        self.get_or_create_client(model_id, None).await
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

    /// Get a client (supports resolving primary/fast)
    pub async fn get_client_resolved(&self, model_id: &str) -> Result<Arc<AIClient>> {
        let global_config: crate::service::config::GlobalConfig =
            self.config_service.get_config(None).await?;
        let resolved_model_id = resolve_required_model_selector(
            model_id,
            |selector| global_config.ai.resolve_model_selection(selector),
            |model_ref| global_config.ai.resolve_model_reference(model_ref),
        )
        .map_err(|error| anyhow!(error.to_string()))?;

        self.get_or_create_client(&resolved_model_id, None).await
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
        if global_config
            .ai
            .models
            .iter()
            .filter(|model| model.id == normalized_model_id)
            .nth(1)
            .is_some()
        {
            return Err(anyhow!(
                "Multiple model configurations use the same ID: {}",
                normalized_model_id
            ));
        }

        debug!("Creating new AI client: model_id={}", normalized_model_id);
        let mut matching_models = global_config
            .ai
            .models
            .iter()
            .filter(|m| m.id == normalized_model_id);
        let model_config = matching_models
            .next()
            .ok_or_else(|| anyhow!("Model configuration not found: {}", normalized_model_id))?;
        if matching_models.next().is_some() {
            return Err(anyhow!(
                "Multiple model configurations use the same ID: {}",
                normalized_model_id
            ));
        }

        if !model_config.enabled {
            return Err(anyhow!(
                "Model '{}' (id={}) is currently disabled; enable it in settings or pick another model",
                model_config.name,
                model_config.id
            ));
        }

        let configuration_fingerprint = model_runtime_binding_fingerprint(model_config);
        if expected_configuration_fingerprint
            .is_some_and(|expected| expected != configuration_fingerprint)
        {
            return Err(anyhow!(
                "Approved model binding changed for configuration id: {}",
                normalized_model_id
            ));
        }

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
                    && !subscription_credential_stale(&model_config.auth, cached)
                {
                    return Ok(cached.client.clone());
                }
            }
        }

        let mut ai_config = AIConfig::try_from(model_config.clone())
            .map_err(|e| anyhow!("AI configuration conversion failed: {}", e))?;
        let credential_expires_at =
            apply_subscription_auth(&model_config.auth, &mut ai_config).await?;

        let proxy_config = if global_config.ai.proxy.enabled {
            Some(global_config.ai.proxy.clone())
        } else {
            None
        };

        let stream_options = build_stream_options_for_model(&global_config.ai, Some(model_config));
        let client = Arc::new(AIClient::new_with_runtime_options(
            ai_config,
            proxy_config,
            stream_options,
        ));

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
                    client: client.clone(),
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
    pub async fn initialize_global() -> BitFunResult<()> {
        if Self::is_global_initialized() {
            return Ok(());
        }

        info!("Initializing global AIClientFactory...");

        let config_service = get_global_config_service().await.map_err(|e| {
            BitFunError::service(format!("Failed to get global config service: {}", e))
        })?;

        let factory = Arc::new(AIClientFactory::new(config_service));
        let wrapper = Arc::new(tokio::sync::RwLock::new(Some(factory)));

        GLOBAL_AI_CLIENT_FACTORY.set(wrapper).map_err(|_| {
            BitFunError::service("Failed to initialize global AIClientFactory".to_string())
        })?;

        info!("Global AIClientFactory initialized");
        Ok(())
    }

    /// Get the global AIClientFactory instance
    pub async fn get_global() -> BitFunResult<Arc<AIClientFactory>> {
        let wrapper = GLOBAL_AI_CLIENT_FACTORY.get().ok_or_else(|| {
            BitFunError::service(
                "Global AIClientFactory not initialized. Call initialize_global() first."
                    .to_string(),
            )
        })?;

        let guard = wrapper.read().await;
        guard
            .as_ref()
            .ok_or_else(|| BitFunError::service("Global AIClientFactory is None".to_string()))
            .map(Arc::clone)
    }

    pub fn is_global_initialized() -> bool {
        GLOBAL_AI_CLIENT_FACTORY.get().is_some()
    }

    /// Update the global AIClientFactory instance (used for config reload)
    pub async fn update_global(new_factory: Arc<AIClientFactory>) -> BitFunResult<()> {
        let wrapper = GLOBAL_AI_CLIENT_FACTORY.get().ok_or_else(|| {
            BitFunError::service("Global AIClientFactory not initialized".to_string())
        })?;

        {
            let mut guard = wrapper.write().await;
            *guard = Some(new_factory);
        }

        debug!("Global AIClientFactory updated");
        Ok(())
    }
}

pub async fn get_global_ai_client_factory() -> BitFunResult<Arc<AIClientFactory>> {
    AIClientFactory::get_global().await
}

pub async fn initialize_global_ai_client_factory() -> BitFunResult<()> {
    AIClientFactory::initialize_global().await
}

fn to_adapter_provider(provider: SubscriptionProvider) -> AdapterProvider {
    match provider {
        SubscriptionProvider::Codex => AdapterProvider::Codex,
        SubscriptionProvider::Antigravity => AdapterProvider::Antigravity,
        SubscriptionProvider::Opencode => AdapterProvider::Opencode,
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
    let resolved = match auth {
        AuthConfig::ApiKey => return Ok(None),
        AuthConfig::Subscription { provider } => {
            subscription_auth::resolve(to_adapter_provider(*provider))
                .await
                .map_err(|e| {
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

/// List subscription accounts (Codex / Antigravity / OpenCode).
pub async fn list_subscription_accounts() -> Vec<subscription_auth::SubscriptionAccount> {
    subscription_auth::list_accounts().await
}

#[cfg(test)]
mod tests {
    use crate::service::config::types::{
        model_runtime_binding_fingerprint, AIModelConfig, GlobalConfig,
    };
    use bitfun_ai_adapters::{
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
        config.ai.models = ["inherit", "primary", "fast", "auto", "default"]
            .into_iter()
            .map(|id| build_model(id, id, &format!("runtime-{id}")))
            .collect();

        for id in ["inherit", "primary", "fast", "auto", "default"] {
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
    fn auto_model_selectors_normalize_to_primary_for_client_lookup() {
        assert_eq!(classify_model_selector("auto"), ModelSelectorKind::Primary);
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
