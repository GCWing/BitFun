//! Global configuration service singleton
//!
//! Provides a global configuration service instance with dynamic updates and synchronization.

use super::service::ConfigService;
use crate::util::errors::*;
use log::{debug, info, warn};
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::RwLock;

/// Global configuration service singleton.
static GLOBAL_CONFIG_SERVICE: OnceLock<Arc<RwLock<Option<Arc<ConfigService>>>>> = OnceLock::new();

/// Configuration update notification channel.
static CONFIG_UPDATE_SENDER: OnceLock<tokio::sync::broadcast::Sender<ConfigUpdateEvent>> =
    OnceLock::new();

/// Configuration update events.
#[derive(Debug, Clone)]
pub enum ConfigUpdateEvent {
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
    /// Theme configuration updated.
    ThemeUpdated { theme_id: String },
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

        match super::tool_config_sync::sync_tool_configs().await {
            Ok(report) => {
                if !report.new_tools.is_empty() || !report.deleted_tools.is_empty() {
                    info!(
                        "Tool config sync completed: {} new, {} deleted, {} updated modes",
                        report.new_tools.len(),
                        report.deleted_tools.len(),
                        report.updated_modes.len()
                    );
                }
            }
            Err(e) => {
                warn!("Tool config sync failed: {}", e);
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
        Self::broadcast_update(ConfigUpdateEvent::ConfigReloaded).await;
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

    /// Updates the theme configuration and broadcasts an event.
    pub async fn update_theme(&self, theme_id: &str) -> BitFunResult<()> {
        let service = Self::get_service().await?;
        service.set_config("theme.id", theme_id).await?;

        Self::broadcast_update(ConfigUpdateEvent::ThemeUpdated {
            theme_id: theme_id.to_string(),
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
