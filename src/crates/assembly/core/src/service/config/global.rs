//! Global configuration service singleton
//!
//! Provides a global configuration service instance with dynamic updates and synchronization.

use super::service::ConfigService;
use crate::util::errors::*;
#[cfg(feature = "agent-runtime")]
use log::warn;
use log::{debug, info};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::RwLock;

/// Global configuration service singleton.
static GLOBAL_CONFIG_SERVICE: OnceLock<Arc<RwLock<Option<Arc<ConfigService>>>>> = OnceLock::new();

/// Configuration update notification channel.
static CONFIG_UPDATE_SENDER: OnceLock<tokio::sync::broadcast::Sender<ConfigUpdateEvent>> =
    OnceLock::new();

/// Cached RBAC/Warden master switch (R-26).
///
/// Mirrors `ai.rbac_enabled` in the settings document. Kept as a process-level
/// cache so synchronous hot paths (tool restriction gates) can read it without
/// awaiting the config service. Refreshed on config initialize / reload /
/// update; defaults to `true` (mechanism on).
static RBAC_ENABLED_CACHE: AtomicBool = AtomicBool::new(true);

/// Dot-path of the RBAC/Warden master switch inside the settings document.
/// Config paths resolve against the serialized `GlobalConfig`, where `AIConfig`
/// lives under `ai`.
pub(crate) const RBAC_ENABLED_CONFIG_PATH: &str = "ai.rbac_enabled";

/// Current value of the RBAC/Warden master switch (cached, synchronous).
///
/// Hot-path safe: never awaits the config service. The cache is refreshed from
/// the settings document on config initialize / reload / update.
pub fn rbac_enabled() -> bool {
    RBAC_ENABLED_CACHE.load(Ordering::Relaxed)
}

/// Override the cached RBAC/Warden master switch.
///
/// Used by the config service when the settings document changes and by tests.
pub fn set_rbac_enabled(enabled: bool) {
    RBAC_ENABLED_CACHE.store(enabled, Ordering::Relaxed);
}

/// Refresh the cached RBAC/Warden master switch from the global config.
///
/// Best-effort: hosts without an initialized config service keep the default
/// (`true`). Called after config initialize, reload, and service replacement.
pub(crate) async fn refresh_rbac_enabled_cache() {
    let enabled = match get_global_config_service().await {
        Ok(service) => service
            .get_config::<bool>(Some(RBAC_ENABLED_CONFIG_PATH))
            .await
            .unwrap_or(true),
        Err(_) => true,
    };
    RBAC_ENABLED_CACHE.store(enabled, Ordering::Relaxed);
}

/// Cached master switch for external user instruction sources.
///
/// Mirrors `ai.external_instruction_sources` in the settings document. Kept as
/// a process-level cache so synchronous hot paths (instruction context
/// assembly gates) can read it without awaiting the config service. Refreshed
/// on config initialize / reload / update; defaults to `false` (do not load
/// external CLAUDE.md / OpenCode / Codex user instructions), matching the
/// taiji 定制版 default of `ai.external_instruction_sources = false`.
static EXTERNAL_INSTRUCTION_SOURCES_ENABLED_CACHE: AtomicBool = AtomicBool::new(false);

/// Dot-path of the external user instruction sources switch inside the
/// settings document. Config paths resolve against the serialized
/// `GlobalConfig`, where `AIConfig` lives under `ai`.
pub(crate) const EXTERNAL_INSTRUCTION_SOURCES_CONFIG_PATH: &str = "ai.external_instruction_sources";

/// Current value of the external user instruction sources switch (cached,
/// synchronous).
///
/// Hot-path safe: never awaits the config service. The cache is refreshed from
/// the settings document on config initialize / reload / update.
pub fn external_instruction_sources_enabled() -> bool {
    EXTERNAL_INSTRUCTION_SOURCES_ENABLED_CACHE.load(Ordering::Relaxed)
}

/// Override the cached external user instruction sources switch.
///
/// Used by the config service when the settings document changes and by tests.
pub fn set_external_instruction_sources_enabled(enabled: bool) {
    EXTERNAL_INSTRUCTION_SOURCES_ENABLED_CACHE.store(enabled, Ordering::Relaxed);
}

/// Refresh the cached external user instruction sources switch from the global
/// config.
///
/// Best-effort: hosts without an initialized config service keep the default
/// (`false`). Called after config initialize, reload, and service replacement.
pub(crate) async fn refresh_external_instruction_sources_enabled_cache() {
    let enabled = match get_global_config_service().await {
        Ok(service) => service
            .get_config::<bool>(Some(EXTERNAL_INSTRUCTION_SOURCES_CONFIG_PATH))
            .await
            .unwrap_or(false),
        Err(_) => false,
    };
    EXTERNAL_INSTRUCTION_SOURCES_ENABLED_CACHE.store(enabled, Ordering::Relaxed);
}

/// Cached master switch for workspace instruction files.
///
/// Mirrors `ai.workspace_instruction_files` in the settings document. Kept as
/// a process-level cache so synchronous hot paths (User Context assembly
/// gates) can read it without awaiting the config service. Refreshed on config
/// initialize / reload / update; defaults to `false` (do not render project
/// AGENTS.md / CLAUDE.md content), matching the taiji 定制版 default of
/// `ai.workspace_instruction_files = false`.
static WORKSPACE_INSTRUCTION_FILES_ENABLED_CACHE: AtomicBool = AtomicBool::new(false);

/// Dot-path of the workspace instruction files switch inside the settings
/// document. Config paths resolve against the serialized `GlobalConfig`, where
/// `AIConfig` lives under `ai`.
pub(crate) const WORKSPACE_INSTRUCTION_FILES_CONFIG_PATH: &str = "ai.workspace_instruction_files";

/// Current value of the workspace instruction files switch (cached,
/// synchronous).
///
/// Hot-path safe: never awaits the config service. The cache is refreshed from
/// the settings document on config initialize / reload / update.
pub fn workspace_instruction_files_enabled() -> bool {
    WORKSPACE_INSTRUCTION_FILES_ENABLED_CACHE.load(Ordering::Relaxed)
}

/// Override the cached workspace instruction files switch.
///
/// Used by the config service when the settings document changes and by tests.
pub fn set_workspace_instruction_files_enabled(enabled: bool) {
    WORKSPACE_INSTRUCTION_FILES_ENABLED_CACHE.store(enabled, Ordering::Relaxed);
}

/// Refresh the cached workspace instruction files switch from the global
/// config.
///
/// Best-effort: hosts without an initialized config service keep the default
/// (`false`). Called after config initialize, reload, and service replacement.
pub(crate) async fn refresh_workspace_instruction_files_enabled_cache() {
    let enabled = match get_global_config_service().await {
        Ok(service) => service
            .get_config::<bool>(Some(WORKSPACE_INSTRUCTION_FILES_CONFIG_PATH))
            .await
            .unwrap_or(false),
        Err(_) => false,
    };
    WORKSPACE_INSTRUCTION_FILES_ENABLED_CACHE.store(enabled, Ordering::Relaxed);
}

/// Configuration update events.
#[derive(Debug, Clone)]
pub enum ConfigUpdateEvent {
    /// AI model catalog, default slots, or agent-model defaults changed.
    /// Consumers that materialize model bindings should rebuild future-use
    /// projections without mutating already running sessions.
    ModelConfigurationUpdated,
    /// AI model configuration updated.
    AIModelUpdated {
        model_id: String,
        model_name: String,
    },
    /// Default AI model updated.
    DefaultAIModelUpdated {
        model_id: String,
        model_name: String,
    },
    /// Web UI appearance selection updated.
    AppearanceUpdated { appearance_id: String },
    /// Editor configuration updated.
    EditorUpdated,
    /// Terminal configuration updated.
    TerminalUpdated,
    /// Workspace configuration updated.
    WorkspaceUpdated,
    /// App configuration updated.
    AppUpdated,
    /// Configuration fully reloaded.
    ConfigReloaded,
    /// The models.dev reasoning catalog snapshot changed. Session owners use
    /// this to reconcile persisted reasoning preset selections.
    ReasoningCatalogUpdated,
    /// Debug-mode configuration updated.
    DebugModeConfigUpdated {
        /// The new ingest port.
        new_port: u16,
        /// The new log path.
        new_log_path: String,
    },
    /// Runtime log level updated.
    LogLevelUpdated {
        /// New runtime log level.
        new_level: String,
    },
    /// Runtime sensitive diagnostics preference updated.
    LoggingSensitiveDiagnosticsUpdated {
        /// Whether logs may include prompts, payloads, and other sensitive diagnostics.
        include_sensitive_diagnostics: bool,
    },
    /// AI models / default-model slots / agent-model defaults were reconciled
    /// after a model became unavailable (disabled, deleted, or otherwise
    /// invalid). Emitted whenever the config layer had to silently rewrite
    /// `ai.default_models`, `ai.agent_model_defaults`, or `ai.func_agent_models`
    /// so they only reference enabled models.
    ModelsReconciled {
        /// Model ids that just became unusable (disabled or deleted) and that
        /// any active session, default slot, or agent mapping was pointing at
        /// before this reconcile pass.
        invalidated_model_ids: Vec<String>,
        /// Whether `ai.default_models` was rewritten as part of the reconcile.
        default_models_changed: bool,
        /// Whether `ai.func_agent_models` was rewritten as part of the reconcile.
        func_agent_models_changed: bool,
        /// Whether `ai.agent_model_defaults` was rewritten as part of the reconcile.
        agent_model_defaults_changed: bool,
    },
}

/// Global configuration service manager.
pub struct GlobalConfigManager;

impl GlobalConfigManager {
    /// Initializes the global configuration service.
    pub async fn initialize() -> BitFunResult<()> {
        if Self::is_initialized() {
            debug!("Global config service already initialized, skipping");
            return Ok(());
        }

        let (sender, _) = tokio::sync::broadcast::channel(100);
        CONFIG_UPDATE_SENDER.set(sender).map_err(|_| {
            BitFunError::config("Failed to initialize config update sender".to_string())
        })?;

        let config_service = Arc::new(ConfigService::new().await?);
        let service_wrapper = Arc::new(RwLock::new(Some(config_service)));

        GLOBAL_CONFIG_SERVICE.set(service_wrapper).map_err(|_| {
            BitFunError::config("Failed to initialize global config service".to_string())
        })?;

        info!("Global config service initialized");
        refresh_rbac_enabled_cache().await;
        refresh_external_instruction_sources_enabled_cache().await;
        refresh_workspace_instruction_files_enabled_cache().await;

        #[cfg(feature = "agent-runtime")]
        {
            match super::mode_config_canonicalizer::canonicalize_agent_profile_configs().await {
                Ok(report) => {
                    if !report.removed_profile_configs.is_empty()
                        || !report.updated_profiles.is_empty()
                    {
                        info!(
                            "Mode config canonicalization completed: removed_profiles={}, updated_profiles={}",
                            report.removed_profile_configs.len(),
                            report.updated_profiles.len()
                        );
                    }
                }
                Err(e) => {
                    warn!("Mode config canonicalization failed: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Returns the global configuration service instance.
    pub async fn get_service() -> BitFunResult<Arc<ConfigService>> {
        let service_wrapper = GLOBAL_CONFIG_SERVICE.get().ok_or_else(|| {
            BitFunError::config("Global config service not initialized".to_string())
        })?;

        let service_guard = service_wrapper.read().await;
        service_guard
            .as_ref()
            .ok_or_else(|| BitFunError::config("Global config service is None".to_string()))
            .map(Arc::clone)
    }

    /// Updates the global configuration service instance (used for configuration reload).
    pub async fn update_service(new_service: Arc<ConfigService>) -> BitFunResult<()> {
        let service_wrapper = GLOBAL_CONFIG_SERVICE.get().ok_or_else(|| {
            BitFunError::config("Global config service not initialized".to_string())
        })?;

        {
            let mut service_guard = service_wrapper.write().await;
            *service_guard = Some(new_service);
        }

        Self::broadcast_update(ConfigUpdateEvent::ConfigReloaded).await;
        refresh_rbac_enabled_cache().await;
        refresh_external_instruction_sources_enabled_cache().await;
        refresh_workspace_instruction_files_enabled_cache().await;

        debug!("Global config service updated");
        Ok(())
    }

    /// Reloads configuration in-place.
    ///
    /// Re-reads the config from disk into the existing `ConfigService` instance,
    /// preserving the `Arc` pointer so that all holders (e.g. `AppState`) stay in sync.
    pub async fn reload() -> BitFunResult<()> {
        let service = Self::get_service().await?;
        service.reload().await?;
        #[cfg(feature = "agent-runtime")]
        if let Err(error) =
            super::mode_config_canonicalizer::canonicalize_agent_profile_configs().await
        {
            warn!(
                "Mode config canonicalization failed after reload: {}",
                error
            );
        }
        Self::broadcast_update(ConfigUpdateEvent::ConfigReloaded).await;
        refresh_rbac_enabled_cache().await;
        refresh_external_instruction_sources_enabled_cache().await;
        refresh_workspace_instruction_files_enabled_cache().await;
        Ok(())
    }

    /// Subscribes to configuration update events.
    pub fn subscribe_updates() -> Option<tokio::sync::broadcast::Receiver<ConfigUpdateEvent>> {
        CONFIG_UPDATE_SENDER.get().map(|sender| sender.subscribe())
    }

    /// Broadcasts a configuration update event.
    pub async fn broadcast_update(event: ConfigUpdateEvent) {
        if let Some(sender) = CONFIG_UPDATE_SENDER.get() {
            let _ = sender.send(event);
        }
    }

    /// Updates an AI model configuration and broadcasts an event.
    pub async fn update_ai_model(
        &self,
        model_id: &str,
        model: crate::service::config::types::AIModelConfig,
    ) -> BitFunResult<()> {
        let model_name = model.name.clone();
        let service = Self::get_service().await?;
        service.update_ai_model(model_id, model).await?;

        Self::broadcast_update(ConfigUpdateEvent::AIModelUpdated {
            model_id: model_id.to_string(),
            model_name,
        })
        .await;

        Ok(())
    }

    /// Updates the Web UI appearance selection and broadcasts an event.
    pub async fn update_appearance(&self, appearance_id: &str) -> BitFunResult<()> {
        let service = Self::get_service().await?;
        service
            .set_config("appearance.selection", appearance_id)
            .await?;
        let stored_appearance_id: String = service.get_config(Some("appearance.selection")).await?;

        Self::broadcast_update(ConfigUpdateEvent::AppearanceUpdated {
            appearance_id: stored_appearance_id,
        })
        .await;

        Ok(())
    }

    /// Returns whether the configuration service has been initialized.
    pub fn is_initialized() -> bool {
        GLOBAL_CONFIG_SERVICE.get().is_some()
    }
}

/// Convenience helper: get the global configuration service.
pub async fn get_global_config_service() -> BitFunResult<Arc<ConfigService>> {
    GlobalConfigManager::get_service().await
}

/// Convenience helper: initialize the global configuration service.
pub async fn initialize_global_config() -> BitFunResult<()> {
    GlobalConfigManager::initialize().await
}

/// Convenience helper: reload the global configuration.
pub async fn reload_global_config() -> BitFunResult<()> {
    GlobalConfigManager::reload().await
}

/// Convenience helper: subscribe to configuration updates.
pub fn subscribe_config_updates() -> Option<tokio::sync::broadcast::Receiver<ConfigUpdateEvent>> {
    GlobalConfigManager::subscribe_updates()
}
