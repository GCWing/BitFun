//! AI client factory - centrally manages client instances for all models
//!
//! Responsibilities:
//! 1. Create and cache AI clients on demand
//! 2. Manage agent model configuration
//! 3. Invalidate cache when configuration changes
//! 4. Provide global singleton access

use crate::infrastructure::ai::{build_stream_options, AIClient};
use crate::infrastructure::cli_credentials::{
    self, codex::CodexResolver, gemini::GeminiResolver, CredentialResolver,
};
use crate::service::config::types::AuthConfig;
use crate::service::config::{get_global_config_service, ConfigService};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::AIConfig;
use anyhow::{anyhow, Result};
use log::{debug, info, warn};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

pub struct AIClientFactory {
    config_service: Arc<ConfigService>,
    client_cache: RwLock<HashMap<String, Arc<AIClient>>>,
}

impl AIClientFactory {
    fn resolve_model_reference_in_config(
        global_config: &crate::service::config::GlobalConfig,
        model_ref: &str,
    ) -> Option<String> {
        global_config.ai.resolve_model_reference(model_ref)
    }

    fn resolve_model_selection_in_config(
        global_config: &crate::service::config::GlobalConfig,
        model_ref: &str,
    ) -> Option<String> {
        global_config.ai.resolve_model_selection(model_ref)
    }

    fn new(config_service: Arc<ConfigService>) -> Self {
        Self {
            config_service,
            client_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Get the main agent's AI client
    /// Falls back to primary when no dedicated model is configured
    pub async fn get_client_by_agent(&self, agent_name: &str) -> Result<Arc<AIClient>> {
        let global_config: crate::service::config::GlobalConfig =
            self.config_service.get_config(None).await?;

        match global_config.ai.agent_models.get(agent_name) {
            Some(model_id) => self.get_client_resolved(model_id).await,
            None => self.get_client_resolved("primary").await,
        }
    }

    /// Get a functional agent's AI client
    /// Prefer func_agent_models, fall back to agent_models (legacy), then fast
    pub async fn get_client_by_func_agent(&self, func_agent_name: &str) -> Result<Arc<AIClient>> {
        let global_config: crate::service::config::GlobalConfig =
            self.config_service.get_config(None).await?;

        let model_id = global_config
            .ai
            .func_agent_models
            .get(func_agent_name)
            .or_else(|| global_config.ai.agent_models.get(func_agent_name))
            .map(String::as_str)
            .unwrap_or("fast");

        self.get_client_resolved(model_id).await
    }

    pub async fn get_client_by_id(&self, model_id: &str) -> Result<Arc<AIClient>> {
        self.get_or_create_client(model_id).await
    }

    /// Get a client (supports resolving primary/fast)
    pub async fn get_client_resolved(&self, model_id: &str) -> Result<Arc<AIClient>> {
        let global_config: crate::service::config::GlobalConfig =
            self.config_service.get_config(None).await?;

        let resolved_model_id = match model_id {
            "primary" => Self::resolve_model_selection_in_config(&global_config, "primary")
                .ok_or_else(|| anyhow!("Primary model not configured or invalid"))?,
            "fast" => Self::resolve_model_selection_in_config(&global_config, "fast").ok_or_else(
                || anyhow!("Fast model not configured or invalid, and primary model not configured or invalid"),
            )?,
            _ => Self::resolve_model_reference_in_config(&global_config, model_id)
                .unwrap_or_else(|| model_id.to_string()),
        };

        self.get_or_create_client(&resolved_model_id).await
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

    async fn get_or_create_client(&self, model_id: &str) -> Result<Arc<AIClient>> {
        let global_config: crate::service::config::GlobalConfig =
            self.config_service.get_config(None).await?;
        let normalized_model_id = match model_id {
            "primary" | "fast" => Self::resolve_model_selection_in_config(&global_config, model_id)
                .unwrap_or_else(|| model_id.to_string()),
            _ => Self::resolve_model_reference_in_config(&global_config, model_id)
                .unwrap_or_else(|| model_id.to_string()),
        };

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
            if let Some(client) = cache.get(&normalized_model_id) {
                return Ok(client.clone());
            }
        }

        debug!("Creating new AI client: model_id={}", normalized_model_id);
        let model_config = global_config
            .ai
            .models
            .iter()
            .find(|m| {
                m.id == normalized_model_id
                    || m.name == normalized_model_id
                    || m.model_name == normalized_model_id
            })
            .ok_or_else(|| anyhow!("Model configuration not found: {}", normalized_model_id))?;

        if !model_config.enabled {
            return Err(anyhow!(
                "Model '{}' (id={}) is currently disabled; enable it in settings or pick another model",
                model_config.name,
                model_config.id
            ));
        }

        let mut ai_config = AIConfig::try_from(model_config.clone())
            .map_err(|e| anyhow!("AI configuration conversion failed: {}", e))?;
        apply_cli_credential(&model_config.auth, &mut ai_config).await?;

        let proxy_config = if global_config.ai.proxy.enabled {
            Some(global_config.ai.proxy.clone())
        } else {
            None
        };

        let stream_options = build_stream_options(&global_config.ai);
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
            cache.insert(model_config.id.clone(), client.clone());
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

/// Resolve a CLI-credential `AuthConfig` and overlay it onto the runtime
/// `AIConfig`. No-op when `auth == AuthConfig::ApiKey`.
pub async fn apply_cli_credential(auth: &AuthConfig, ai_config: &mut AIConfig) -> Result<()> {
    let resolved = match auth {
        AuthConfig::ApiKey => return Ok(()),
        AuthConfig::CodexCli => CodexResolver.resolve().await?,
        AuthConfig::GeminiCli => GeminiResolver.resolve().await?,
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
    Ok(())
}

/// Discover all locally-available CLI credentials (Codex, Gemini, ...).
pub async fn discover_cli_credentials() -> Vec<cli_credentials::DiscoveredCredential> {
    cli_credentials::discover_all().await
}

#[cfg(test)]
mod tests {
    use super::AIClientFactory;
    use crate::service::config::types::{AIModelConfig, GlobalConfig};

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
    fn resolve_model_reference_supports_id_name_and_model_name() {
        let mut config = GlobalConfig::default();
        config.ai.models = vec![build_model(
            "model-123",
            "Primary Chat",
            "claude-sonnet-4.5",
        )];

        assert_eq!(
            AIClientFactory::resolve_model_reference_in_config(&config, "model-123"),
            Some("model-123".to_string())
        );
        assert_eq!(
            AIClientFactory::resolve_model_reference_in_config(&config, "Primary Chat"),
            Some("model-123".to_string())
        );
        assert_eq!(
            AIClientFactory::resolve_model_reference_in_config(&config, "claude-sonnet-4.5"),
            Some("model-123".to_string())
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
            AIClientFactory::resolve_model_selection_in_config(&config, "fast"),
            Some("model-primary".to_string())
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
            AIClientFactory::resolve_model_selection_in_config(&config, "fast"),
            Some("model-primary".to_string())
        );
    }
}
