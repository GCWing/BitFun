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

        if Self::path_touches_models(path) {
            if let Err(e) = self.reconcile_models("set_config").await {
                warn!(
                    "Model reconcile after set_config failed: path={}, error={}",
                    path, e
                );
            }
        }

        Ok(())
    }

    fn path_touches_models(path: &str) -> bool {
        path == "ai"
            || path.starts_with("ai.models")
            || path.starts_with("ai.default_models")
            || path.starts_with("ai.agent_models")
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
        let import_result = {
            let mut manager = self.manager.write().await;
            manager
                .import_config(serde_json::to_value(export.config)?)
                .await
        };

        match import_result {
            Ok(_) => {
                if let Err(e) = self.reconcile_models("import_config").await {
                    warn!("Model reconcile after import_config failed: {}", e);
                }
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

    /// Bring `ai.default_models`, `ai.agent_models`, and `ai.func_agent_models`
    /// back into a consistent state with `ai.models`.
    ///
    /// This is the single integrity guard the rest of the system relies on:
    /// - any agent / func-agent mapping pointing at a model that no longer
    ///   exists or that became disabled is dropped;
    /// - `default_models.primary` / `.fast` are repointed to the first enabled
    ///   model when their current target is missing or disabled (or cleared
    ///   when no enabled model exists at all);
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
        let known_ids: HashSet<String> = config.ai.models.iter().map(|m| m.id.clone()).collect();

        // Precompute lookup tables so the closures below do not need to
        // borrow `config.ai` (which would conflict with the later mutations
        // of `config.ai.agent_models` / `config.ai.default_models`).
        let mut active_refs: HashSet<String> = HashSet::new();
        let mut any_ref_to_id: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for m in &config.ai.models {
            any_ref_to_id
                .entry(m.id.clone())
                .or_insert_with(|| m.id.clone());
            any_ref_to_id
                .entry(m.name.clone())
                .or_insert_with(|| m.id.clone());
            any_ref_to_id
                .entry(m.model_name.clone())
                .or_insert_with(|| m.id.clone());
            if m.enabled {
                active_refs.insert(m.id.clone());
                active_refs.insert(m.name.clone());
                active_refs.insert(m.model_name.clone());
            }
        }
        let is_active = |reference: &str| -> bool {
            // Special selectors are always considered active; their actual
            // resolution happens at runtime against the (already reconciled)
            // default slots.
            matches!(reference, "auto" | "primary" | "fast") || active_refs.contains(reference)
        };

        let classify_invalid = |reference: &str, invalidated: &mut HashSet<String>| -> bool {
            if is_active(reference) {
                return false;
            }
            // Resolve back to the canonical id (if the reference is by
            // name / model_name pointing at a now-disabled model) so we
            // can report a stable identifier.
            let canonical = any_ref_to_id
                .get(reference)
                .cloned()
                .unwrap_or_else(|| reference.to_string());
            invalidated.insert(canonical);
            true
        };

        let mut invalidated: HashSet<String> = HashSet::new();
        let mut agent_models_changed = false;
        let mut default_models_changed = false;

        // 1. agent_models
        let agent_keys_to_remove: Vec<String> = config
            .ai
            .agent_models
            .iter()
            .filter_map(|(agent, model_ref)| {
                if classify_invalid(model_ref, &mut invalidated) {
                    Some(agent.clone())
                } else {
                    None
                }
            })
            .collect();
        for agent in agent_keys_to_remove {
            warn!(
                "Reconcile ({caller}): clearing ai.agent_models[{agent}] because target model is missing or disabled"
            );
            config.ai.agent_models.remove(&agent);
            agent_models_changed = true;
        }

        // 2. func_agent_models
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
            agent_models_changed = true;
        }

        // 3. default_models.primary / .fast
        let fallback_id = config.ai.first_enabled_model_id();
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

        // Ensure `invalidated` doesn't contain a still-existing-and-enabled id
        // (defensive: classify_invalid only inserts for inactive refs, but a
        // callsite could have re-resolved via name).
        invalidated.retain(|id| !enabled_ids.contains(id));

        // Persist any changes. We deliberately use the inner manager (and not
        // `self.set_config`) to avoid triggering a recursive reconcile pass.
        if agent_models_changed {
            let mut manager = self.manager.write().await;
            manager
                .set("ai.agent_models", &config.ai.agent_models)
                .await?;
            manager
                .set("ai.func_agent_models", &config.ai.func_agent_models)
                .await?;
        }
        if default_models_changed {
            let mut manager = self.manager.write().await;
            manager
                .set("ai.default_models", &config.ai.default_models)
                .await?;
        }

        let _ = known_ids; // currently unused, kept for future diagnostics

        let report = ReconcileModelsReport {
            invalidated_model_ids: invalidated.into_iter().collect(),
            default_models_changed,
            agent_models_changed,
        };

        if report.is_noop() {
            log::debug!("Reconcile ({caller}): no changes");
        } else {
            info!(
                "Reconcile ({caller}): invalidated={:?}, default_changed={}, agent_changed={}",
                report.invalidated_model_ids,
                report.default_models_changed,
                report.agent_models_changed
            );
            super::global::GlobalConfigManager::broadcast_update(
                super::global::ConfigUpdateEvent::ModelsReconciled {
                    invalidated_model_ids: report.invalidated_model_ids.clone(),
                    default_models_changed: report.default_models_changed,
                    agent_models_changed: report.agent_models_changed,
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
    pub agent_models_changed: bool,
}

impl ReconcileModelsReport {
    pub fn is_noop(&self) -> bool {
        self.invalidated_model_ids.is_empty()
            && !self.default_models_changed
            && !self.agent_models_changed
    }
}
