//! Configuration service implementation
//!
//! Provides comprehensive configuration management functionality.

use super::manager::{ConfigManager, ConfigManagerSettings, ConfigStatistics};
use super::types::*;
use crate::util::errors::*;
use log::{info, warn};
use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration service.
pub struct ConfigService {
    manager: Arc<RwLock<ConfigManager>>,
}

/// Configuration import/export format.
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigExport {
    pub config: GlobalConfig,
    pub export_timestamp: String,
    pub version: String,
}

/// Configuration import result.
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigImportResult {
    pub success: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Configuration health status.
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigHealthStatus {
    pub healthy: bool,
    pub total_providers: usize,
    pub config_directory: std::path::PathBuf,
    pub warnings: Vec<String>,
    pub message: String,
    pub last_modified: chrono::DateTime<chrono::Utc>,
}

impl ConfigService {
    /// Creates a new configuration service.
    pub async fn new() -> BitFunResult<Self> {
        let settings = ConfigManagerSettings::default();
        Self::with_settings(settings).await
    }

    /// Creates a configuration service with custom settings.
    ///
    /// Runs an initial [`Self::reconcile_models`] pass so any pre-existing
    /// persisted config that points at a now-disabled / missing model (e.g.
    /// from before this guard was introduced) is cleaned up on startup.
    pub async fn with_settings(settings: ConfigManagerSettings) -> BitFunResult<Self> {
        let manager = ConfigManager::new(settings).await?;

        let service = Self {
            manager: Arc::new(RwLock::new(manager)),
        };

        if let Err(e) = service.reconcile_models("startup").await {
            warn!("Model reconcile at startup failed: {}", e);
        }

        Ok(service)
    }

    /// Gets a configuration value (supports dot-paths).
    pub async fn get_config<T>(&self, path: Option<&str>) -> BitFunResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let manager = self.manager.read().await;

        if let Some(path) = path {
            manager.get(path)
        } else {
            let config = manager.get_config();
            serde_json::from_value(serde_json::to_value(config)?)
                .map_err(|e| BitFunError::config(format!("Failed to serialize config: {}", e)))
        }
    }

    /// Sets a configuration value (supports dot-paths).
    ///
    /// When the path touches AI models / default model slots / agent-model
    /// mappings, runs [`Self::reconcile_models`] afterwards so the config can
    /// never end up referencing a disabled or deleted model.
    pub async fn set_config<T>(&self, path: &str, value: T) -> BitFunResult<()>
    where
        T: serde::Serialize,
    {
        {
            let mut manager = self.manager.write().await;
            manager.set(path, value).await?;
        }

        let model_configuration_changed = Self::path_touches_models(path);
        if model_configuration_changed {
            if let Err(e) = self.reconcile_models("set_config").await {
                warn!(
                    "Model reconcile after set_config failed: path={}, error={}",
                    path, e
                );
            }
            super::global::GlobalConfigManager::broadcast_update(
                super::global::ConfigUpdateEvent::ModelConfigurationUpdated,
            )
            .await;
        }

        Ok(())
    }

    fn path_touches_models(path: &str) -> bool {
        path == "ai"
            || path.starts_with("ai.models")
            || path.starts_with("ai.default_models")
            || path.starts_with("ai.agent_model_defaults")
            || path.starts_with("ai.func_agent_models")
    }

    /// Resets configuration.
    ///
    /// When the reset target touches AI models (or is a global reset),
    /// triggers [`Self::reconcile_models`] so default-slot / agent-model
    /// references can never linger pointing at a now-missing model.
    pub async fn reset_config(&self, path: Option<&str>) -> BitFunResult<()> {
        {
            let mut manager = self.manager.write().await;
            manager.reset(path).await?;
        }

        let needs_reconcile = match path {
            None => true,
            Some(p) => Self::path_touches_models(p),
        };
        if needs_reconcile {
            if let Err(e) = self.reconcile_models("reset_config").await {
                warn!(
                    "Model reconcile after reset_config failed: path={:?}, error={}",
                    path, e
                );
            }
            super::global::GlobalConfigManager::broadcast_update(
                super::global::ConfigUpdateEvent::ModelConfigurationUpdated,
            )
            .await;
        }

        Ok(())
    }

    /// Validates configuration.
    pub async fn validate_config(&self) -> BitFunResult<ConfigValidationResult> {
        let manager = self.manager.read().await;
        manager.validate_config().await
    }

    /// Exports configuration.
    pub async fn export_config(&self) -> BitFunResult<ConfigExport> {
        let manager = self.manager.read().await;
        let config_value = manager.export_config()?;
        let config: GlobalConfig = serde_json::from_value(config_value)?;

        Ok(ConfigExport {
            config,
            export_timestamp: chrono::Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    /// Imports configuration. Triggers a model reconcile pass on success so an
    /// imported config that references missing / disabled models is brought
    /// back into a self-consistent state.
    pub async fn import_config(&self, export: ConfigExport) -> BitFunResult<ConfigImportResult> {
        self.import_config_data(serde_json::to_value(export.config)?)
            .await
    }

    /// Imports raw configuration JSON. Keeping this boundary raw preserves
    /// legacy fields that are intentionally normalized before deserialization.
    pub async fn import_config_data(
        &self,
        config_data: serde_json::Value,
    ) -> BitFunResult<ConfigImportResult> {
        let import_result = {
            let mut manager = self.manager.write().await;
            manager.import_config(config_data).await
        };

        match import_result {
            Ok(_) => {
                if let Err(e) = self.reconcile_models("import_config").await {
                    warn!("Model reconcile after import_config failed: {}", e);
                }
                super::global::GlobalConfigManager::broadcast_update(
                    super::global::ConfigUpdateEvent::ModelConfigurationUpdated,
                )
                .await;
                Ok(ConfigImportResult {
                    success: true,
                    errors: Vec::new(),
                    warnings: Vec::new(),
                })
            }
            Err(e) => Ok(ConfigImportResult {
                success: false,
                errors: vec![e.to_string()],
                warnings: Vec::new(),
            }),
        }
    }

    /// Returns configuration statistics.
    pub async fn get_statistics(&self) -> ConfigStatistics {
        let manager = self.manager.read().await;
        manager.get_statistics()
    }

    /// Runs a health check.
    pub async fn health_check(&self) -> BitFunResult<ConfigHealthStatus> {
        let manager = self.manager.read().await;
        let stats = manager.get_statistics();
        let validation_result = manager.validate_config().await?;

        let mut warnings = Vec::new();

        for warning in &validation_result.warnings {
            warnings.push(format!("{}: {}", warning.path, warning.message));
        }

        if stats.total_ai_models == 0 {
            warnings.push("No AI models configured".to_string());
        }

        let config: GlobalConfig = self.get_config(None).await?;
        if config.ai.default_models.primary.is_none() {
            warnings.push("Primary model not configured".to_string());
        }

        if !stats.config_directory.exists() {
            return Ok(ConfigHealthStatus {
                healthy: false,
                total_providers: stats.providers_count,
                config_directory: stats.config_directory,
                warnings,
                message: "Configuration directory does not exist".to_string(),
                last_modified: stats.last_modified,
            });
        }

        let healthy = validation_result.valid && stats.total_ai_models > 0;

        Ok(ConfigHealthStatus {
            healthy,
            total_providers: stats.providers_count,
            config_directory: stats.config_directory,
            warnings,
            message: if healthy {
                "Configuration system is healthy".to_string()
            } else {
                "Configuration system has issues".to_string()
            },
            last_modified: stats.last_modified,
        })
    }

    /// Reloads configuration.
    pub async fn reload(&self) -> BitFunResult<()> {
        let settings = ConfigManagerSettings::default();
        let new_manager = ConfigManager::new(settings).await?;

        {
            let mut manager = self.manager.write().await;
            *manager = new_manager;
        }

        info!("Configuration reloaded");

        if let Err(e) = self.reconcile_models("reload").await {
            warn!("Model reconcile after reload failed: {}", e);
        }
        super::global::GlobalConfigManager::broadcast_update(
            super::global::ConfigUpdateEvent::ModelConfigurationUpdated,
        )
        .await;
        Ok(())
    }

    /// Creates a configuration backup.
    pub async fn create_backup(&self) -> BitFunResult<std::path::PathBuf> {
        let manager = self.manager.read().await;
        manager.create_backup().await
    }

    /// Registers a configuration provider.
    pub async fn register_provider(&self, provider: Box<dyn ConfigProvider>) {
        let mut manager = self.manager.write().await;
        manager.register_provider(provider);
    }

    /// Returns all AI model configurations.
    pub async fn get_ai_models(&self) -> BitFunResult<Vec<AIModelConfig>> {
        let config: GlobalConfig = self.get_config(None).await?;
        Ok(config.ai.models)
    }

    /// Adds an AI model configuration.
    pub async fn add_ai_model(&self, model: AIModelConfig) -> BitFunResult<()> {
        let mut config: GlobalConfig = self.get_config(None).await?;
        config.ai.models.push(model);
        self.set_config("ai.models", &config.ai.models).await
    }

    /// Updates an AI model configuration.
    pub async fn update_ai_model(&self, model_id: &str, model: AIModelConfig) -> BitFunResult<()> {
        let mut config: GlobalConfig = self.get_config(None).await?;

        if let Some(existing_model) = config.ai.models.iter_mut().find(|m| m.id == model_id) {
            *existing_model = model;
            self.set_config("ai.models", &config.ai.models).await
        } else {
            Err(BitFunError::config(format!(
                "AI model '{}' not found",
                model_id
            )))
        }
    }

    /// Deletes an AI model configuration.
    pub async fn delete_ai_model(&self, model_id: &str) -> BitFunResult<()> {
        let mut config: GlobalConfig = self.get_config(None).await?;

        let original_len = config.ai.models.len();
        config.ai.models.retain(|m| m.id != model_id);

        if config.ai.models.len() == original_len {
            return Err(BitFunError::config(format!(
                "AI model '{}' not found",
                model_id
            )));
        }

        // Persist the list deletion. The follow-up reconcile pass triggered by
        // `set_config` (and explicitly by `update_ai_model`) is responsible for
        // cleaning every other place the deleted id might still be referenced
        // (default slots, agent / func-agent mappings).
        self.set_config("ai.models", &config.ai.models).await
    }

    /// Bring `ai.default_models`, `ai.agent_model_defaults`, and
    /// `ai.func_agent_models` back into a consistent state with `ai.models`.
    ///
    /// This is the single integrity guard the rest of the system relies on:
    /// - any func-agent mapping pointing at a model that no longer exists or
    ///   that became disabled is dropped;
    /// - `default_models.primary` / `.fast` are repointed to the first enabled
    ///   model when their current target is missing or disabled (or cleared
    ///   when no enabled model exists at all);
    /// - optional capability slots such as `default_models.image_understanding`
    ///   are kept pointed at an enabled model with the matching capability, or
    ///   cleared when no matching model is available;
    /// - on every change, a [`ConfigUpdateEvent::ModelsReconciled`] is
    ///   broadcast so [`SessionManager`](crate::agentic::session::SessionManager)
    ///   and the AI client cache can react in lockstep.
    ///
    /// `caller` is logged for diagnostics (e.g. `set_config`, `update_ai_model`).
    pub async fn reconcile_models(&self, caller: &str) -> BitFunResult<ReconcileModelsReport> {
        let mut config: GlobalConfig = self.get_config(None).await?;

        let enabled_ids: HashSet<String> = config
            .ai
            .models
            .iter()
            .filter(|m| m.enabled)
            .map(|m| m.id.clone())
            .collect();
        let is_active = |reference: &str| -> bool {
            // Special selectors are always considered active; their actual
            // resolution happens at runtime against the (already reconciled)
            // default slots.
            matches!(reference, "auto" | "primary" | "fast") || enabled_ids.contains(reference)
        };

        let classify_invalid = |reference: &str, invalidated: &mut HashSet<String>| -> bool {
            if is_active(reference) {
                return false;
            }
            invalidated.insert(reference.to_string());
            true
        };

        let mut invalidated: HashSet<String> = HashSet::new();
        let mut func_agent_models_changed = false;
        let mut agent_model_defaults_changed = false;
        let mut default_models_changed = false;

        // 1. func_agent_models
        let func_keys_to_remove: Vec<String> = config
            .ai
            .func_agent_models
            .iter()
            .filter_map(|(agent, model_ref)| {
                if classify_invalid(model_ref, &mut invalidated) {
                    Some(agent.clone())
                } else {
                    None
                }
            })
            .collect();
        for agent in func_keys_to_remove {
            warn!(
                "Reconcile ({caller}): clearing ai.func_agent_models[{agent}] because target model is missing or disabled"
            );
            config.ai.func_agent_models.remove(&agent);
            func_agent_models_changed = true;
        }

        // 2. future mode and delegated-subagent defaults
        if classify_invalid(
            config.ai.agent_model_defaults.mode.as_str(),
            &mut invalidated,
        ) {
            warn!(
                "Reconcile ({caller}): resetting ai.agent_model_defaults.mode because target model is missing or disabled"
            );
            config.ai.agent_model_defaults.mode = "auto".to_string();
            agent_model_defaults_changed = true;
        }

        if config
            .ai
            .agent_model_defaults
            .subagents
            .default_selection
            .fixed_model_id()
            .is_some_and(|model_id| classify_invalid(model_id, &mut invalidated))
        {
            warn!(
                "Reconcile ({caller}): resetting ai.agent_model_defaults.subagents.default because target model is missing or disabled"
            );
            config.ai.agent_model_defaults.subagents.default_selection =
                SubagentModelSelection::fixed("fast");
            agent_model_defaults_changed = true;
        }

        let builtin_keys_to_remove: Vec<String> = config
            .ai
            .agent_model_defaults
            .subagents
            .builtin
            .iter()
            .filter_map(|(subagent_id, selection)| {
                selection
                    .fixed_model_id()
                    .filter(|model_id| classify_invalid(model_id, &mut invalidated))
                    .map(|_| subagent_id.clone())
            })
            .collect();
        for subagent_id in builtin_keys_to_remove {
            warn!(
                "Reconcile ({caller}): clearing ai.agent_model_defaults.subagents.builtin[{subagent_id}] because target model is missing or disabled"
            );
            config
                .ai
                .agent_model_defaults
                .subagents
                .builtin
                .remove(&subagent_id);
            agent_model_defaults_changed = true;
        }

        if config
            .ai
            .agent_model_defaults
            .subagents
            .fork
            .fixed_model_id()
            .is_some_and(|model_id| classify_invalid(model_id, &mut invalidated))
        {
            warn!(
                "Reconcile ({caller}): resetting ai.agent_model_defaults.subagents.fork because target model is missing or disabled"
            );
            config.ai.agent_model_defaults.subagents.fork = SubagentModelSelection::Inherit;
            agent_model_defaults_changed = true;
        }

        // 3. default model slots
        let fallback_id = config.ai.first_enabled_model_id();
        let image_understanding_fallback_id = config
            .ai
            .models
            .iter()
            .find(|model| model.enabled && model.supports_image_understanding())
            .map(|model| model.id.clone());
        let mut repoint_default_slot = |slot: &mut Option<String>, slot_name: &str| {
            let needs_fix = match slot.as_deref() {
                Some("") => true,
                Some(value) => !is_active(value),
                None => false,
            };
            if !needs_fix {
                return;
            }

            if let Some(current) = slot.as_deref() {
                classify_invalid(current, &mut invalidated);
            }

            match fallback_id.as_ref() {
                Some(new_id) => {
                    info!(
                        "Reconcile ({caller}): default_models.{slot_name} repointed: {:?} -> {}",
                        slot, new_id
                    );
                    *slot = Some(new_id.clone());
                }
                None => {
                    info!(
                        "Reconcile ({caller}): default_models.{slot_name} cleared (no enabled model available); previous={:?}",
                        slot
                    );
                    *slot = None;
                }
            }
            default_models_changed = true;
        };

        repoint_default_slot(&mut config.ai.default_models.primary, "primary");
        repoint_default_slot(&mut config.ai.default_models.fast, "fast");

        let image_understanding_needs_fix =
            match config.ai.default_models.image_understanding.as_deref() {
                Some("") => true,
                Some(value) => !config.ai.models.iter().any(|model| {
                    model.enabled && model.supports_image_understanding() && model.id == value
                }),
                None => false,
            };
        if image_understanding_needs_fix {
            if let Some(current) = config.ai.default_models.image_understanding.as_deref() {
                classify_invalid(current, &mut invalidated);
            }

            match image_understanding_fallback_id.as_ref() {
                Some(new_id) => {
                    info!(
                        "Reconcile ({caller}): default_models.image_understanding repointed: {:?} -> {}",
                        config.ai.default_models.image_understanding, new_id
                    );
                    config.ai.default_models.image_understanding = Some(new_id.clone());
                }
                None => {
                    info!(
                        "Reconcile ({caller}): default_models.image_understanding cleared (no enabled capable model available); previous={:?}",
                        config.ai.default_models.image_understanding
                    );
                    config.ai.default_models.image_understanding = None;
                }
            }
            default_models_changed = true;
        }

        // Ensure `invalidated` doesn't contain a still-existing-and-enabled ID.
        invalidated.retain(|id| !enabled_ids.contains(id));

        // Persist any changes. We deliberately use the inner manager (and not
        // `self.set_config`) to avoid triggering a recursive reconcile pass.
        if func_agent_models_changed {
            let mut manager = self.manager.write().await;
            manager
                .set("ai.func_agent_models", &config.ai.func_agent_models)
                .await?;
        }
        if agent_model_defaults_changed {
            let mut manager = self.manager.write().await;
            manager
                .set("ai.agent_model_defaults", &config.ai.agent_model_defaults)
                .await?;
        }
        if default_models_changed {
            let mut manager = self.manager.write().await;
            manager
                .set("ai.default_models", &config.ai.default_models)
                .await?;
        }

        let report = ReconcileModelsReport {
            invalidated_model_ids: invalidated.into_iter().collect(),
            default_models_changed,
            func_agent_models_changed,
            agent_model_defaults_changed,
        };

        if report.is_noop() {
            log::debug!("Reconcile ({caller}): no changes");
        } else {
            info!(
                "Reconcile ({caller}): invalidated={:?}, default_changed={}, func_agent_changed={}, agent_defaults_changed={}",
                report.invalidated_model_ids,
                report.default_models_changed,
                report.func_agent_models_changed,
                report.agent_model_defaults_changed
            );
            super::global::GlobalConfigManager::broadcast_update(
                super::global::ConfigUpdateEvent::ModelsReconciled {
                    invalidated_model_ids: report.invalidated_model_ids.clone(),
                    default_models_changed: report.default_models_changed,
                    func_agent_models_changed: report.func_agent_models_changed,
                    agent_model_defaults_changed: report.agent_model_defaults_changed,
                },
            )
            .await;
        }

        Ok(report)
    }
}

#[async_trait::async_trait]
impl bitfun_runtime_ports::ConfigReadPort for ConfigService {
    async fn get_config_value(
        &self,
        key: &str,
    ) -> bitfun_runtime_ports::PortResult<Option<serde_json::Value>> {
        self.get_config::<serde_json::Value>(Some(key))
            .await
            .map(Some)
            .map_err(|error| {
                bitfun_runtime_ports::PortError::new(
                    bitfun_runtime_ports::PortErrorKind::Backend,
                    error.to_string(),
                )
            })
    }
}

/// Outcome of [`ConfigService::reconcile_models`].
#[derive(Debug, Clone, Default)]
pub struct ReconcileModelsReport {
    pub invalidated_model_ids: Vec<String>,
    pub default_models_changed: bool,
    pub func_agent_models_changed: bool,
    pub agent_model_defaults_changed: bool,
}

impl ReconcileModelsReport {
    pub fn is_noop(&self) -> bool {
        self.invalidated_model_ids.is_empty()
            && !self.default_models_changed
            && !self.func_agent_models_changed
            && !self.agent_model_defaults_changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::PathManager;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn model(id: &str, enabled: bool, category: ModelCategory) -> AIModelConfig {
        let capabilities = if matches!(category, ModelCategory::Multimodal) {
            vec![
                ModelCapability::TextChat,
                ModelCapability::ImageUnderstanding,
            ]
        } else {
            vec![ModelCapability::TextChat]
        };

        AIModelConfig {
            id: id.to_string(),
            name: format!("Provider {id}"),
            provider: "openai".to_string(),
            model_name: id.to_string(),
            base_url: "https://example.com/v1".to_string(),
            enabled,
            category,
            capabilities,
            ..Default::default()
        }
    }

    async fn test_service(name: &str) -> (ConfigService, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let user_root = dir.path().join(name);
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(user_root));

        let service = ConfigService::with_settings(ConfigManagerSettings {
            path_manager: Some(path_manager),
            auto_save: true,
            backup_count: 0,
        })
        .await
        .expect("config service");

        (service, dir)
    }

    #[tokio::test]
    async fn reconcile_models_repairs_image_understanding_default_to_capable_model() {
        let (service, _dir) = test_service("vision-default-repair").await;
        let models = vec![
            model("text-model", true, ModelCategory::GeneralChat),
            model("disabled-vision", false, ModelCategory::Multimodal),
            model("active-vision", true, ModelCategory::Multimodal),
        ];

        service
            .set_config("ai.models", &models)
            .await
            .expect("models should save");
        service
            .set_config(
                "ai.default_models",
                &DefaultModelsConfig {
                    primary: Some("text-model".to_string()),
                    image_understanding: Some("disabled-vision".to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("defaults should save");

        let defaults: DefaultModelsConfig = service
            .get_config(Some("ai.default_models"))
            .await
            .expect("defaults should load");
        assert_eq!(defaults.primary.as_deref(), Some("text-model"));
        assert_eq!(
            defaults.image_understanding.as_deref(),
            Some("active-vision"),
            "vision default must not fall back to a text-only model"
        );
    }

    #[tokio::test]
    async fn reconcile_models_resets_invalid_agent_model_defaults() {
        let (service, _dir) = test_service("agent-model-defaults-repair").await;
        service
            .set_config(
                "ai.models",
                &vec![model("old-model", true, ModelCategory::GeneralChat)],
            )
            .await
            .expect("initial model should save");
        service
            .set_config(
                "ai.agent_model_defaults",
                &AgentModelDefaultsConfig {
                    mode: "old-model".to_string(),
                    subagents: SubagentModelDefaultsConfig {
                        default_selection: SubagentModelSelection::fixed("old-model"),
                        builtin: HashMap::from([(
                            "Explore".to_string(),
                            SubagentModelSelection::fixed("old-model"),
                        )]),
                        fork: SubagentModelSelection::fixed("old-model"),
                    },
                },
            )
            .await
            .expect("agent model defaults should save");

        service
            .set_config(
                "ai.models",
                &vec![model("new-model", true, ModelCategory::GeneralChat)],
            )
            .await
            .expect("model replacement should reconcile defaults");

        let defaults: AgentModelDefaultsConfig = service
            .get_config(Some("ai.agent_model_defaults"))
            .await
            .expect("agent model defaults should load");
        assert_eq!(defaults.mode, "auto");
        assert_eq!(
            defaults.subagents.default_selection,
            SubagentModelSelection::fixed("fast")
        );
        assert!(defaults.subagents.builtin.is_empty());
        assert_eq!(defaults.subagents.fork, SubagentModelSelection::Inherit);
    }

    #[tokio::test]
    async fn legacy_theme_id_path_writes_themes_current_only() {
        let (service, _dir) = test_service("legacy-theme-id-path").await;

        service
            .set_config("theme.id", "dark")
            .await
            .expect("legacy theme path should remain a thin compatibility alias");

        let current: String = service
            .get_config(Some("themes.current"))
            .await
            .expect("theme selection should be readable from the TS-owned path");
        assert_eq!(current, "bitfun-dark");

        let export: GlobalConfig = service
            .get_config(None)
            .await
            .expect("full config should load");
        let serialized = serde_json::to_value(export).expect("config should serialize");
        assert!(
            serialized.get("theme").is_none(),
            "legacy path must not recreate the removed Rust GUI theme schema"
        );
    }

    #[tokio::test]
    async fn raw_import_preserves_legacy_theme_id_before_deserialization() {
        let (service, _dir) = test_service("legacy-theme-raw-import").await;
        let mut raw_config =
            serde_json::to_value(GlobalConfig::default()).expect("default config should serialize");
        let raw_object = raw_config
            .as_object_mut()
            .expect("default config should serialize as an object");
        raw_object.remove("themes");
        raw_object.insert(
            "theme".to_string(),
            serde_json::json!({
                "id": "dark",
                "colors": {
                    "background": "#1e1e1e"
                }
            }),
        );

        service
            .import_config_data(raw_config)
            .await
            .expect("raw legacy config should import before old fields are dropped");

        let current: String = service
            .get_config(Some("themes.current"))
            .await
            .expect("legacy theme id should migrate into themes.current");
        assert_eq!(current, "bitfun-dark");

        let export: GlobalConfig = service
            .get_config(None)
            .await
            .expect("full config should load after import");
        let serialized = serde_json::to_value(export).expect("config should serialize");
        assert!(
            serialized.get("theme").is_none(),
            "legacy theme payload should not be exported after import"
        );
    }
}
