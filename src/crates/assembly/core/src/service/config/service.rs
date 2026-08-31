//! Configuration service implementation
//!
//! Provides comprehensive configuration management functionality.

use super::manager::{ConfigImportSource, ConfigManager, ConfigManagerSettings, ConfigStatistics};
use super::types::*;
use crate::util::errors::*;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::{watch, RwLock};

/// Configuration service.
pub struct ConfigService {
    manager: Arc<RwLock<ConfigManager>>,
    runtime_ai_models: Arc<RwLock<BTreeMap<String, AIModelConfig>>>,
    local_changes: watch::Sender<()>,
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
            runtime_ai_models: Arc::new(RwLock::new(BTreeMap::new())),
            local_changes: watch::channel(()).0,
        };

        let recovered_with_defaults = service
            .load_diagnostics()
            .await
            .iter()
            .any(|diagnostic| diagnostic.code == "CONFIG_DEFAULT_RECOVERY");
        if !recovered_with_defaults {
            if let Err(e) = service.reconcile_models("startup").await {
                warn!("Model reconcile at startup failed: {}", e);
            }
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

    pub async fn install_runtime_ai_model(&self, model: AIModelConfig) -> BitFunResult<()> {
        if model.id.trim().is_empty() {
            return Err(BitFunError::validation(
                "Runtime model id is required".to_string(),
            ));
        }
        self.runtime_ai_models
            .write()
            .await
            .insert(model.id.clone(), model);
        Ok(())
    }

    pub async fn get_runtime_ai_model(&self, model_id: &str) -> Option<AIModelConfig> {
        self.runtime_ai_models.read().await.get(model_id).cloned()
    }

    pub async fn get_effective_ai_config(&self) -> BitFunResult<AIConfig> {
        let mut ai: AIConfig = self.get_config(Some("ai")).await?;
        for runtime_model in self.runtime_ai_models.read().await.values() {
            if let Some(model) = ai
                .models
                .iter_mut()
                .find(|model| model.id == runtime_model.id)
            {
                *model = runtime_model.clone();
            } else {
                ai.models.push(runtime_model.clone());
            }
        }
        Ok(ai)
    }

    pub async fn remove_runtime_ai_model(&self, model_id: &str) {
        self.runtime_ai_models.write().await.remove(model_id);
    }

    /// Subscribe to successful persisted user mutations. Account sync consumes
    /// this signal so individual UI/CLI mutation adapters cannot forget to mark
    /// settings dirty. Cloud applies and reloads do not echo back as local edits.
    pub fn subscribe_local_changes(&self) -> watch::Receiver<()> {
        self.local_changes.subscribe()
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
        self.after_config_change(path).await;
        Ok(())
    }

    /// Reads, modifies, validates, and persists a section under one write lock.
    /// Use this for mutations of lists/maps or partial changes to a settings
    /// object; taking a snapshot with get_config before set_config can discard
    /// a concurrent save. The closure must not perform IO or await other work.
    pub async fn update_config<T, R>(
        &self,
        path: &str,
        update: impl FnOnce(&mut T) -> BitFunResult<R>,
    ) -> BitFunResult<R>
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        let (result, changed) = {
            let mut manager = self.manager.write().await;
            let before: serde_json::Value = manager.get(path)?;
            let mut value: T = serde_json::from_value(before.clone())?;
            let result = update(&mut value)?;
            let after = serde_json::to_value(value)?;
            let changed = before != after;
            if changed {
                manager.set(path, after).await?;
            }
            (result, changed)
        };
        if changed {
            self.after_config_change(path).await;
        }
        Ok(result)
    }

    async fn after_config_change(&self, path: &str) {
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
        self.local_changes.send_replace(());
    }

    /// Atomically replaces one JSON configuration value when its current value
    /// still matches the caller's snapshot. The read, comparison, and persisted
    /// write share the existing manager write lock.
    #[cfg(any(test, feature = "mcp-runtime"))]
    pub(crate) async fn compare_and_set_json_config(
        &self,
        path: &str,
        expected: Option<serde_json::Value>,
        replacement: serde_json::Value,
    ) -> BitFunResult<bool> {
        let mut manager = self.manager.write().await;
        let current = match manager.get::<serde_json::Value>(path) {
            Ok(value) => Some(value),
            Err(BitFunError::NotFound(_)) => None,
            Err(error) => return Err(error),
        };
        if current != expected {
            return Ok(false);
        }
        manager.set(path, replacement).await?;
        self.local_changes.send_replace(());
        Ok(true)
    }

    fn path_touches_models(path: &str) -> bool {
        path.is_empty()
            || path == "ai"
            || path.starts_with("ai.models")
            || path.starts_with("ai.default_models")
            || path.starts_with("ai.agent_model_defaults")
            || path.starts_with("ai.task_models")
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

        self.local_changes.send_replace(());

        Ok(())
    }

    /// Validates configuration.
    pub async fn validate_config(&self) -> BitFunResult<ConfigValidationResult> {
        let manager = self.manager.read().await;
        let mut result = manager.validate_config().await?;
        result
            .diagnostics
            .extend(manager.load_diagnostics().iter().cloned());
        Ok(result)
    }

    pub async fn load_diagnostics(&self) -> Vec<ConfigDiagnostic> {
        self.manager.read().await.load_diagnostics().to_vec()
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
    /// Missing realtime voice fields retain their local values; explicit empty
    /// credentials in a user-imported backup are still authoritative.
    pub async fn import_config_data(
        &self,
        config_data: serde_json::Value,
    ) -> BitFunResult<ConfigImportResult> {
        self.import_config_data_from_source(config_data, ConfigImportSource::Explicit, None)
            .await
    }

    /// Applies account settings without treating an unconfigured host's empty
    /// realtime voice key as a deletion of this controller's saved credential.
    /// Non-empty synced keys still update normally; explicit imports and local
    /// set/reset operations retain their credential-clearing semantics.
    pub async fn import_account_settings(
        &self,
        config_data: serde_json::Value,
    ) -> BitFunResult<ConfigImportResult> {
        self.import_config_data_from_source(config_data, ConfigImportSource::AccountSync, None)
            .await
    }

    /// A periodic pull may spend seconds on the network. Apply its response
    /// only if the local document still matches the pre-fetch snapshot, with
    /// the comparison and import protected by the same manager write lock.
    pub async fn import_account_settings_if_unchanged(
        &self,
        config_data: serde_json::Value,
        expected_local_config: serde_json::Value,
    ) -> BitFunResult<ConfigImportResult> {
        self.import_config_data_from_source(
            config_data,
            ConfigImportSource::AccountSync,
            Some(expected_local_config),
        )
        .await
    }

    async fn import_config_data_from_source(
        &self,
        config_data: serde_json::Value,
        source: ConfigImportSource,
        expected_local_config: Option<serde_json::Value>,
    ) -> BitFunResult<ConfigImportResult> {
        let import_result = {
            let mut manager = self.manager.write().await;
            if let Some(expected) = expected_local_config {
                if manager.export_config()? != expected {
                    return Ok(ConfigImportResult {
                        success: false,
                        errors: vec!["Local settings changed while cloud settings were being fetched; skipped the stale response".to_string()],
                        warnings: Vec::new(),
                    });
                }
            }
            manager.import_config_from_source(config_data, source).await
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
                if source == ConfigImportSource::Explicit {
                    self.local_changes.send_replace(());
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

        let config = manager.get_config();
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
        {
            let mut manager = self.manager.write().await;
            manager.reload().await?;
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
        self.update_config("ai.models", |models: &mut Vec<AIModelConfig>| {
            models.push(model);
            Ok(())
        })
        .await
    }

    /// Updates an AI model configuration.
    pub async fn update_ai_model(&self, model_id: &str, model: AIModelConfig) -> BitFunResult<()> {
        self.update_config("ai.models", |models: &mut Vec<AIModelConfig>| {
            let existing = models
                .iter_mut()
                .find(|m| m.id == model_id)
                .ok_or_else(|| BitFunError::config(format!("AI model '{}' not found", model_id)))?;
            *existing = model;
            Ok(())
        })
        .await
    }

    /// Deletes an AI model configuration.
    pub async fn delete_ai_model(&self, model_id: &str) -> BitFunResult<()> {
        self.update_config("ai.models", |models: &mut Vec<AIModelConfig>| {
            let original_len = models.len();
            models.retain(|m| m.id != model_id);
            if models.len() == original_len {
                return Err(BitFunError::config(format!(
                    "AI model '{}' not found",
                    model_id
                )));
            }
            Ok(())
        })
        .await
    }

    /// Atomically upserts a pure speech-recognition model, selects it as the
    /// speech default, and switches voice input to the cloud provider.
    pub async fn save_cloud_speech_config(
        &self,
        request: SaveCloudSpeechConfigRequest,
    ) -> BitFunResult<SaveCloudSpeechConfigResult> {
        let name = request.name.trim();
        let base_url = request.base_url.trim().trim_end_matches('/');
        let model_name = request.model_name.trim();
        let api_key = request.api_key.trim();
        if name.is_empty() || base_url.is_empty() || model_name.is_empty() || api_key.is_empty() {
            return Err(BitFunError::validation(
                "Cloud speech name, base URL, model name, and API key are required".to_string(),
            ));
        }
        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            return Err(BitFunError::validation(
                "Cloud speech base URL must use http or https".to_string(),
            ));
        }

        let request_url = request
            .request_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                if base_url.ends_with("/audio/transcriptions") {
                    base_url.to_string()
                } else {
                    format!("{base_url}/audio/transcriptions")
                }
            });
        if !request_url.starts_with("http://") && !request_url.starts_with("https://") {
            return Err(BitFunError::validation(
                "Cloud speech request URL must use http or https".to_string(),
            ));
        }

        let mut manager = self.manager.write().await;
        let mut config = manager.get_config().clone();
        let model_id = request
            .config_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("speech_cloud_{}", uuid::Uuid::new_v4().simple()));
        let existing_index = config
            .ai
            .models
            .iter()
            .position(|model| model.id == model_id);
        let created = existing_index.is_none();
        let existing_metadata = existing_index
            .and_then(|index| config.ai.models[index].metadata.clone())
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        let mut metadata = existing_metadata;
        metadata.insert(
            "speech_provider_preset".to_string(),
            serde_json::Value::String(request.preset.trim().to_string()),
        );
        let model = AIModelConfig {
            id: model_id.clone(),
            name: name.to_string(),
            provider: "openai".to_string(),
            model_name: model_name.to_string(),
            base_url: base_url.to_string(),
            request_url: Some(request_url),
            api_key: api_key.to_string(),
            context_window: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            enabled: true,
            category: ModelCategory::SpeechRecognition,
            capabilities: vec![ModelCapability::SpeechRecognition],
            recommended_for: vec!["voice_input".to_string()],
            metadata: Some(serde_json::Value::Object(metadata)),
            auth: AuthConfig::ApiKey,
            ..AIModelConfig::default()
        };
        match existing_index {
            Some(index) => config.ai.models[index] = model,
            None => config.ai.models.push(model),
        }
        config.ai.default_models.speech_recognition = Some(model_id.clone());
        config.app.ai_experience.voice_input.provider = "cloud".to_string();
        config.app.ai_experience.voice_input.model_id = model_id.clone();

        // The caller may be updating an existing model id. Reconcile all
        // capability-specific slots before the single persistence operation so
        // replacing a text model with a speech-only model cannot leave primary,
        // fast, or agent references pointing at a non-text runtime target.
        super::normalization::reconcile_model_references(&mut config);
        manager.set("", &config).await?;
        drop(manager);

        super::global::GlobalConfigManager::broadcast_update(
            super::global::ConfigUpdateEvent::ModelConfigurationUpdated,
        )
        .await;

        self.local_changes.send_replace(());
        Ok(SaveCloudSpeechConfigResult { model_id, created })
    }

    /// Bring `ai.default_models`, `ai.agent_model_defaults`, and
    /// `ai.task_models` back into a consistent state with `ai.models`.
    ///
    /// This is the single integrity guard the rest of the system relies on:
    /// - invalid task-model selectors are reset to their defaults;
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
        let reconciliation = {
            // Reconciliation writes a full config snapshot. Keep its read and
            // write under one lock so unrelated saves (such as voice keys)
            // cannot be overwritten by the snapshot we read earlier.
            let mut manager = self.manager.write().await;
            let mut config = manager.get_config().clone();
            let reconciliation = super::normalization::reconcile_model_references(&mut config);
            if !reconciliation.is_noop() {
                manager.set("", &config).await?;
            }
            reconciliation
        };

        let report = ReconcileModelsReport {
            invalidated_model_ids: reconciliation.invalidated_model_ids,
            default_models_changed: reconciliation.default_models_changed,
            task_models_changed: reconciliation.task_models_changed,
            agent_model_defaults_changed: reconciliation.agent_model_defaults_changed,
        };

        if report.is_noop() {
            log::debug!("Reconcile ({caller}): no changes");
        } else {
            info!(
                "Reconcile ({caller}): invalidated={:?}, default_changed={}, task_models_changed={}, agent_defaults_changed={}",
                report.invalidated_model_ids,
                report.default_models_changed,
                report.task_models_changed,
                report.agent_model_defaults_changed
            );
            super::global::GlobalConfigManager::broadcast_update(
                super::global::ConfigUpdateEvent::ModelsReconciled {
                    invalidated_model_ids: report.invalidated_model_ids.clone(),
                    default_models_changed: report.default_models_changed,
                    task_models_changed: report.task_models_changed,
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
    pub task_models_changed: bool,
    pub agent_model_defaults_changed: bool,
}

impl ReconcileModelsReport {
    pub fn is_noop(&self) -> bool {
        self.invalidated_model_ids.is_empty()
            && !self.default_models_changed
            && !self.task_models_changed
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

    fn runtime_model(id: &str, key: &str) -> AIModelConfig {
        AIModelConfig {
            id: id.to_string(),
            name: "SDK fixture".to_string(),
            provider: "openai".to_string(),
            model_name: "fixture-model".to_string(),
            base_url: "http://127.0.0.1:43123/v1".to_string(),
            api_key: key.to_string(),
            enabled: true,
            category: ModelCategory::GeneralChat,
            capabilities: vec![ModelCapability::TextChat],
            ..AIModelConfig::default()
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

    fn realtime_voice_fixture() -> VoiceCallConfig {
        VoiceCallConfig {
            api_key: "fixture-realtime-voice-key".to_string(),
            voice: "fixture-voice".to_string(),
            speed: 12,
            loudness: -8,
            microphone_device_id: "fixture-controller-microphone".to_string(),
            ..Default::default()
        }
    }

    async fn restart_test_service(dir: &tempfile::TempDir, name: &str) -> ConfigService {
        ConfigService::with_settings(ConfigManagerSettings {
            path_manager: Some(Arc::new(PathManager::with_user_root_for_tests(
                dir.path().join(name),
            ))),
            auto_save: true,
            backup_count: 0,
        })
        .await
        .expect("restart config service")
    }

    async fn race_config_operations<A: std::future::Future, B: std::future::Future>(
        service: &ConfigService,
        first: A,
        second: B,
    ) -> (A::Output, B::Output) {
        use std::future::poll_fn;
        use std::task::Poll;

        let manager = service.manager.write().await;
        let mut first = std::pin::pin!(first);
        let mut second = std::pin::pin!(second);
        poll_fn(|cx| {
            assert!(first.as_mut().poll(cx).is_pending());
            assert!(second.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        drop(manager);
        tokio::join!(first, second)
    }

    #[tokio::test]
    async fn concurrent_model_additions_keep_both_credentials_after_restart() {
        let name = "concurrent-model-additions";
        let (service, dir) = test_service(name).await;
        let (first, second) = race_config_operations(
            &service,
            service.add_ai_model(runtime_model("first", "first-fixture-key")),
            service.add_ai_model(runtime_model("second", "second-fixture-key")),
        )
        .await;
        first.unwrap();
        second.unwrap();

        let restarted = restart_test_service(&dir, name).await;
        let models = restarted.get_ai_models().await.unwrap();
        assert_eq!(models.len(), 2);
        assert!(models
            .iter()
            .any(|m| m.id == "first" && m.api_key == "first-fixture-key"));
        assert!(models
            .iter()
            .any(|m| m.id == "second" && m.api_key == "second-fixture-key"));
    }

    #[tokio::test]
    async fn concurrent_model_delete_and_update_do_not_resurrect_deleted_credentials() {
        let name = "concurrent-model-delete-update";
        let (service, dir) = test_service(name).await;
        service
            .add_ai_model(runtime_model("keep", "old-fixture-key"))
            .await
            .unwrap();
        service
            .add_ai_model(runtime_model("remove", "deleted-fixture-key"))
            .await
            .unwrap();
        let (deleted, updated) = race_config_operations(
            &service,
            service.delete_ai_model("remove"),
            service.update_ai_model("keep", runtime_model("keep", "new-fixture-key")),
        )
        .await;
        deleted.unwrap();
        updated.unwrap();

        let restarted = restart_test_service(&dir, name).await;
        let models = restarted.get_ai_models().await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "keep");
        assert_eq!(models[0].api_key, "new-fixture-key");
    }

    #[tokio::test]
    async fn failed_config_writes_cannot_leak_into_a_later_successful_save() {
        for reset in [false, true] {
            let name = "failed-config-write";
            let (service, dir) = test_service(name).await;
            service
                .set_config("app.voice_call", realtime_voice_fixture())
                .await
                .unwrap();
            service
                .add_ai_model(runtime_model("keep", "fixture-model-key"))
                .await
                .unwrap();
            let before: serde_json::Value = service.get_config(None).await.unwrap();
            let changes = service.subscribe_local_changes();
            let config_file = dir.path().join(name).join("config/app.json");
            let preserved = config_file.with_extension("preserved.json");
            tokio::fs::rename(&config_file, &preserved).await.unwrap();
            // Replacing a directory with a file fails on every supported OS,
            // without relying on permissions (which root can bypass in CI).
            tokio::fs::create_dir(&config_file).await.unwrap();

            let result = if reset {
                service.reset_config(None).await
            } else {
                service
                    .set_config("app.voice_call.api_key", "unsaved-fixture-key")
                    .await
            };
            assert!(result.is_err());
            assert!(
                !changes.has_changed().unwrap(),
                "Failed writes must not trigger account uploads"
            );
            let after: serde_json::Value = service.get_config(None).await.unwrap();
            assert_eq!(after, before, "A failed write changed the in-memory config");

            tokio::fs::remove_dir(&config_file).await.unwrap();
            tokio::fs::rename(&preserved, &config_file).await.unwrap();
            service.set_config("editor.font_size", 18).await.unwrap();
            let restarted = restart_test_service(&dir, name).await;
            let saved: GlobalConfig = restarted.get_config(None).await.unwrap();
            assert_eq!(
                saved.app.voice_call.api_key,
                realtime_voice_fixture().api_key
            );
            assert_eq!(saved.ai.models[0].api_key, "fixture-model-key");
            assert_eq!(saved.editor.font_size, 18);
        }
    }

    #[tokio::test]
    async fn explicit_backups_are_distinct_even_when_requested_together() {
        let (service, _dir) = test_service("concurrent-backups").await;
        service
            .add_ai_model(runtime_model("keep", "fixture-model-key"))
            .await
            .unwrap();
        let (first, second, third) = tokio::join!(
            service.create_backup(),
            service.create_backup(),
            service.create_backup(),
        );
        let backups = [first.unwrap(), second.unwrap(), third.unwrap()];
        assert_eq!(
            backups
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            3
        );
        for backup in backups {
            let saved: GlobalConfig =
                serde_json::from_slice(&tokio::fs::read(backup).await.unwrap()).unwrap();
            assert_eq!(saved.ai.models[0].api_key, "fixture-model-key");
        }
    }

    #[tokio::test]
    async fn legacy_import_preserves_missing_settings_beyond_realtime_voice() {
        for account_sync in [false, true] {
            let name = "legacy-missing-settings";
            let (service, dir) = test_service(name).await;
            let mut local = GlobalConfig::default();
            local.app.hooks.enabled = false;
            local.app.notifications.permission_request_notify = false;
            local.app.logging.include_sensitive_diagnostics = false;
            local.app.ai_experience.voice_input.provider = "cloud".to_string();
            local.app.ai_experience.voice_input.microphone_device_id =
                "fixture-microphone".to_string();
            local.ai.proxy.enabled = true;
            local.ai.proxy.url = "http://127.0.0.1:12345".to_string();
            local.ai.proxy.password = Some("fixture-proxy-password".to_string());
            local.editor.minimap.enabled = false;
            local.terminal.terminal_panel_position = "bottom".to_string();
            local.memories.use_memories = true;
            service.set_config("", &local).await.unwrap();

            let mut legacy = serde_json::to_value(GlobalConfig::default()).unwrap();
            legacy["version"] = serde_json::json!("0.2.18");
            for (parent, key) in [
                ("/app", "hooks"),
                ("/app/notifications", "permission_request_notify"),
                ("/app/logging", "include_sensitive_diagnostics"),
                ("/app/ai_experience", "voice_input"),
                ("/ai", "proxy"),
                ("/editor", "minimap"),
                ("/terminal", "terminal_panel_position"),
            ] {
                legacy
                    .pointer_mut(parent)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
            }
            if account_sync {
                legacy.as_object_mut().unwrap().remove("memories");
            }
            let imported = if account_sync {
                service.import_account_settings(legacy).await.unwrap()
            } else {
                service.import_config_data(legacy).await.unwrap()
            };
            assert!(imported.success, "{:?}", imported.errors);
            let restarted = restart_test_service(&dir, name).await;
            let saved: GlobalConfig = restarted.get_config(None).await.unwrap();
            assert!(!saved.app.hooks.enabled);
            assert!(!saved.app.notifications.permission_request_notify);
            assert!(!saved.app.logging.include_sensitive_diagnostics);
            assert_eq!(saved.app.ai_experience.voice_input.provider, "cloud");
            assert_eq!(
                saved.app.ai_experience.voice_input.microphone_device_id,
                "fixture-microphone"
            );
            assert_eq!(
                saved.ai.proxy.password.as_deref(),
                Some("fixture-proxy-password")
            );
            assert!(!saved.editor.minimap.enabled);
            assert_eq!(saved.terminal.terminal_panel_position, "bottom");
            assert_eq!(saved.memories.use_memories, account_sync);
        }
    }

    #[tokio::test]
    async fn imports_still_honor_explicit_deletions_and_default_elision_in_backups() {
        for account_sync in [false, true] {
            let (service, _dir) = test_service("import-explicit-deletions").await;
            // A raw backup intentionally omits these default values. Restoring
            // it must still reset them, even though legacy missing fixed fields
            // now retain their local values.
            let backup = service.create_backup().await.unwrap();
            let raw_backup: serde_json::Value =
                serde_json::from_slice(&tokio::fs::read(backup).await.unwrap()).unwrap();
            let mut local = GlobalConfig::default();
            local.mcp_servers =
                Some(serde_json::json!({"mcpServers": {"fixture": {"command": "fixture"}}}));
            local.acp_clients =
                Some(serde_json::json!({"acpClients": {"fixture": {"command": "fixture"}}}));
            local.app.keybindings = Some(serde_json::json!({"version": 1, "overrides": {}}));
            local.plugin = vec![PluginDeclarationConfig::Spec("fixture-plugin".to_string())];
            local
                .ai
                .agent_profiles
                .insert("fixture-profile".to_string(), AgentProfileConfig::default());
            local.ai.agent_model_defaults.subagents.builtin.insert(
                "fixture-override".to_string(),
                SubagentModelSelection::Inherit,
            );
            local.memories.use_memories = true;
            local.ai.allow_tool_json_repair = false;
            local.ai.max_rounds = 42;
            local.app.notifications.enabled = false;
            service.set_config("", &local).await.unwrap();

            let mut incoming = if account_sync {
                serde_json::to_value(GlobalConfig::default()).unwrap()
            } else {
                raw_backup
            };
            incoming["ai"]["review_teams"] = serde_json::json!({});
            incoming["ai"]["agent_model_defaults"]["subagents"]["builtin"] = serde_json::json!({});
            incoming["workspace"]["exclude_patterns"] = serde_json::json!([]);
            let result = if account_sync {
                service.import_account_settings(incoming).await.unwrap()
            } else {
                service.import_config_data(incoming).await.unwrap()
            };
            assert!(result.success, "{:?}", result.errors);
            let saved: GlobalConfig = service.get_config(None).await.unwrap();
            assert!(saved.mcp_servers.is_none());
            assert!(saved.acp_clients.is_none());
            assert!(saved.app.keybindings.is_none());
            assert!(saved.plugin.is_empty());
            assert!(saved.ai.agent_profiles.is_empty());
            assert!(saved.ai.review_teams.is_empty());
            assert!(!saved
                .ai
                .agent_model_defaults
                .subagents
                .builtin
                .contains_key("fixture-override"));
            assert!(!saved
                .ai
                .agent_model_defaults
                .subagents
                .builtin
                .contains_key("GeneralPurpose"));
            assert!(saved.workspace.exclude_patterns.is_empty());
            assert!(!saved.memories.use_memories);
            assert!(saved.ai.allow_tool_json_repair);
            assert_eq!(saved.ai.max_rounds, GlobalConfig::default().ai.max_rounds);
            assert!(saved.app.notifications.enabled);
        }
    }

    #[tokio::test]
    async fn local_change_notifications_cover_mutations_without_echoing_cloud_restores() {
        let (service, _dir) = test_service("config-local-notifications").await;
        let mut changes = service.subscribe_local_changes();
        assert!(!changes.has_changed().unwrap());

        service
            .set_config("app.notifications.enabled", false)
            .await
            .unwrap();
        assert!(changes.has_changed().unwrap());
        changes.borrow_and_update();

        service
            .update_config("ai.skill_settings", |settings: &mut SkillSettingsConfig| {
                settings
                    .globally_disabled_user_skills
                    .push("user::fixture".to_string());
                Ok(())
            })
            .await
            .unwrap();
        assert!(changes.has_changed().unwrap());
        changes.borrow_and_update();

        let snapshot: serde_json::Value = service.get_config(None).await.unwrap();
        assert!(
            service
                .import_account_settings(snapshot.clone())
                .await
                .unwrap()
                .success
        );
        service.reload().await.unwrap();
        assert!(
            !changes.has_changed().unwrap(),
            "Cloud imports must not start an upload feedback loop"
        );

        assert!(service.import_config_data(snapshot).await.unwrap().success);
        assert!(changes.has_changed().unwrap());
        changes.borrow_and_update();

        service
            .reset_config(Some("app.notifications"))
            .await
            .unwrap();
        assert!(changes.has_changed().unwrap());
        changes.borrow_and_update();

        service
            .install_runtime_ai_model(runtime_model("ephemeral", "runtime-fixture-key"))
            .await
            .unwrap();
        assert!(
            !changes.has_changed().unwrap(),
            "Runtime-only credentials must not be synced"
        );

        service.save_cloud_speech_config(serde_json::from_value(serde_json::json!({
            "preset": "custom", "name": "Speech fixture", "baseUrl": "https://example.com/v1",
            "modelName": "speech-fixture", "apiKey": "speech-fixture-key"
        })).unwrap()).await.unwrap();
        assert!(changes.has_changed().unwrap());
    }

    #[tokio::test]
    async fn stale_cloud_pull_cannot_overwrite_a_save_made_during_the_fetch() {
        let name = "config-stale-cloud-pull";
        let (service, dir) = test_service(name).await;
        let before_fetch: serde_json::Value = service.get_config(None).await.unwrap();
        let mut cloud = before_fetch.clone();
        cloud["app"]["voice_call"]["api_key"] = serde_json::json!("old-cloud-fixture-key");

        let (saved, imported) = race_config_operations(
            &service,
            service.set_config("app.voice_call.api_key", "new-local-fixture-key"),
            service.import_account_settings_if_unchanged(cloud.clone(), before_fetch),
        )
        .await;
        saved.unwrap();
        let imported = imported.unwrap();
        assert!(!imported.success);
        assert!(imported.errors[0].contains("Local settings changed"));
        let restarted = restart_test_service(&dir, name).await;
        assert_eq!(
            restarted
                .get_config::<String>(Some("app.voice_call.api_key"))
                .await
                .unwrap(),
            "new-local-fixture-key"
        );

        // A later pull with a current snapshot is still authoritative.
        let current = service.get_config(None).await.unwrap();
        assert!(
            service
                .import_account_settings_if_unchanged(cloud, current)
                .await
                .unwrap()
                .success
        );
        assert_eq!(
            service
                .get_config::<String>(Some("app.voice_call.api_key"))
                .await
                .unwrap(),
            "old-cloud-fixture-key"
        );
    }

    #[tokio::test]
    async fn realtime_voice_survives_legacy_config_import_and_restart() {
        let name = "realtime-voice-legacy-import";
        let (service, dir) = test_service(name).await;
        let voice = realtime_voice_fixture();
        service.set_config("app.voice_call", &voice).await.unwrap();

        // A pre-voice cloud payload still carries the user's model keys.
        let mut legacy = serde_json::to_value(GlobalConfig::default()).unwrap();
        legacy["version"] = serde_json::json!("0.2.18");
        legacy["app"].as_object_mut().unwrap().remove("voice_call");
        legacy["ai"]["models"] =
            serde_json::json!([runtime_model("synced-model", "fixture-synced-model-key")]);
        let imported = service.import_config_data(legacy).await.unwrap();
        assert!(imported.success, "{:?}", imported.errors);

        drop(service);
        let restarted = restart_test_service(&dir, name).await;
        let config: GlobalConfig = restarted.get_config(None).await.unwrap();
        assert_eq!(config.ai.models[0].api_key, "fixture-synced-model-key");
        assert_eq!(
            serde_json::to_value(config.app.voice_call).unwrap(),
            serde_json::to_value(voice).unwrap()
        );
    }

    #[tokio::test]
    async fn realtime_voice_survives_model_reconciliation_racing_a_save() {
        use std::future::{poll_fn, Future};
        use std::task::Poll;

        let name = "realtime-voice-reconcile-race";
        let (service, dir) = test_service(name).await;
        let voice = realtime_voice_fixture();
        let mut manager = service.manager.write().await;
        // Leave the default slots unreconciled, just as a model-list mutation
        // does before its follow-up reconciliation acquires the manager.
        manager
            .set(
                "ai.models",
                vec![runtime_model("configured-model", "fixture-model-key")],
            )
            .await
            .unwrap();

        let mut reconcile = std::pin::pin!(service.reconcile_models("realtime-voice-test"));
        let mut save = std::pin::pin!(service.set_config("app.voice_call", &voice));
        // Queue reconciliation before the save. A read-then-write reconcile
        // releases its snapshot lock and writes after the queued save, losing
        // the key. A single write-locked transaction cannot do that.
        poll_fn(|cx| {
            assert!(reconcile.as_mut().poll(cx).is_pending());
            assert!(save.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        drop(manager);
        let (reconciled, saved) = tokio::join!(reconcile, save);
        assert!(!reconciled.unwrap().is_noop());
        saved.unwrap();

        let restarted = restart_test_service(&dir, name).await;
        let config: GlobalConfig = restarted.get_config(None).await.unwrap();
        assert_eq!(config.app.voice_call.api_key, voice.api_key);
        assert_eq!(
            config.ai.default_models.primary.as_deref(),
            Some("configured-model")
        );
    }

    #[tokio::test]
    async fn realtime_voice_account_sync_preserves_keys_from_missing_or_empty_payloads() {
        for incoming_voice in [
            None,
            Some(serde_json::to_value(VoiceCallConfig::default()).unwrap()),
            Some(serde_json::json!({ "api_key": " \n\t", "enabled": false })),
        ] {
            let name = "realtime-voice-account-compatibility";
            let (service, dir) = test_service(name).await;
            let voice = realtime_voice_fixture();
            service
                .add_ai_model(runtime_model(
                    "obsolete-model",
                    "fixture-obsolete-model-key",
                ))
                .await
                .unwrap();
            service.set_config("app.voice_call", &voice).await.unwrap();
            let mut incoming = serde_json::to_value(GlobalConfig::default()).unwrap();
            let app = incoming["app"].as_object_mut().unwrap();
            match incoming_voice {
                Some(voice) => {
                    app.insert("voice_call".to_string(), voice);
                }
                None => {
                    app.remove("voice_call");
                }
            }
            incoming["ai"]["models"] =
                serde_json::json!([runtime_model("cloud-model", "fixture-cloud-model-key")]);
            let expected_enabled = incoming["app"]["voice_call"]["enabled"]
                .as_bool()
                .unwrap_or(voice.enabled);
            let imported = service.import_account_settings(incoming).await.unwrap();
            assert!(imported.success, "{:?}", imported.errors);
            // This is the reload performed after an account settings apply.
            service.reload().await.unwrap();

            drop(service);
            let restarted = restart_test_service(&dir, name).await;
            let config: GlobalConfig = restarted.get_config(None).await.unwrap();
            assert_eq!(config.app.voice_call.api_key, voice.api_key);
            assert_eq!(config.app.voice_call.enabled, expected_enabled);
            assert_eq!(config.ai.models.len(), 1);
            assert_eq!(config.ai.models[0].id, "cloud-model");
            assert_eq!(config.ai.models[0].api_key, "fixture-cloud-model-key");
        }
    }

    #[tokio::test]
    async fn realtime_voice_account_sync_updates_keys_and_backs_up_the_previous_file() {
        let name = "realtime-voice-account-update";
        let (service, dir) = test_service(name).await;
        service
            .set_config("app.voice_call", realtime_voice_fixture())
            .await
            .unwrap();
        let config_dir = service.get_statistics().await.config_directory;
        let original = tokio::fs::read_to_string(config_dir.join("app.json"))
            .await
            .unwrap();

        let mut incoming = serde_json::to_value(GlobalConfig::default()).unwrap();
        incoming["app"]["voice_call"] = serde_json::json!({
            "api_key": "fixture-updated-voice-key",
            "voice": "fixture-updated-voice"
        });
        let imported = service.import_account_settings(incoming).await.unwrap();
        assert!(imported.success, "{:?}", imported.errors);

        let backup = std::fs::read_dir(config_dir.join("backups"))
            .unwrap()
            .map(Result::unwrap)
            .find(|entry| entry.file_name().to_string_lossy().contains("pre-import"))
            .expect("pre-import backup");
        assert_eq!(
            tokio::fs::read_to_string(backup.path()).await.unwrap(),
            original
        );

        drop(service);
        let restarted = restart_test_service(&dir, name).await;
        let voice: VoiceCallConfig = restarted.get_config(Some("app.voice_call")).await.unwrap();
        assert_eq!(voice.api_key, "fixture-updated-voice-key");
        assert_eq!(voice.voice, "fixture-updated-voice");
        assert_eq!(voice.microphone_device_id, "fixture-controller-microphone");
    }

    #[tokio::test]
    async fn realtime_voice_export_and_backup_restore_preserve_credentials() {
        let name = "realtime-voice-backup-restore";
        let (service, dir) = test_service(name).await;
        let voice = realtime_voice_fixture();
        service.set_config("app.voice_call", &voice).await.unwrap();
        service
            .add_ai_model(runtime_model("configured-model", "fixture-model-key"))
            .await
            .unwrap();
        let export = service.export_config().await.unwrap();
        let backup = service.create_backup().await.unwrap();
        let backup: serde_json::Value =
            serde_json::from_str(&tokio::fs::read_to_string(backup).await.unwrap()).unwrap();
        assert_eq!(export.config.app.voice_call.api_key, voice.api_key);
        assert_eq!(backup["app"]["voice_call"]["api_key"], voice.api_key);

        service.reset_config(Some("app.voice_call")).await.unwrap();
        let restored = service.import_config(export).await.unwrap();
        assert!(restored.success, "{:?}", restored.errors);
        let restored_voice: VoiceCallConfig =
            service.get_config(Some("app.voice_call")).await.unwrap();
        assert_eq!(restored_voice.api_key, voice.api_key);

        service.reset_config(Some("app.voice_call")).await.unwrap();
        let restored = service.import_config_data(backup).await.unwrap();
        assert!(restored.success, "{:?}", restored.errors);
        drop(service);
        let restarted = restart_test_service(&dir, name).await;
        let config: GlobalConfig = restarted.get_config(None).await.unwrap();
        assert_eq!(config.ai.models[0].api_key, "fixture-model-key");
        assert_eq!(
            serde_json::to_value(config.app.voice_call).unwrap(),
            serde_json::to_value(voice).unwrap()
        );
    }

    #[tokio::test]
    async fn realtime_voice_explicit_import_save_and_reset_can_clear_credentials() {
        let name = "realtime-voice-explicit-clear";
        let (service, dir) = test_service(name).await;
        for operation in ["import", "save", "reset"] {
            service
                .set_config("app.voice_call", realtime_voice_fixture())
                .await
                .unwrap();
            match operation {
                "import" => {
                    let incoming = serde_json::to_value(GlobalConfig::default()).unwrap();
                    let imported = service.import_config_data(incoming).await.unwrap();
                    assert!(imported.success, "{:?}", imported.errors);
                }
                "save" => {
                    service
                        .set_config(
                            "app.voice_call",
                            VoiceCallConfig {
                                enabled: false,
                                ..Default::default()
                            },
                        )
                        .await
                        .unwrap();
                }
                _ => service.reset_config(Some("app.voice_call")).await.unwrap(),
            }
            let restarted = restart_test_service(&dir, name).await;
            let voice: VoiceCallConfig =
                restarted.get_config(Some("app.voice_call")).await.unwrap();
            assert!(voice.api_key.is_empty(), "operation={operation}");
        }
    }

    #[tokio::test]
    async fn realtime_voice_survives_model_mutations_and_application_upgrade() {
        let name = "realtime-voice-upgrade";
        let (service, dir) = test_service(name).await;
        let voice = realtime_voice_fixture();
        service.set_config("app.voice_call", &voice).await.unwrap();
        service
            .add_ai_model(runtime_model("keep", "fixture-model-key"))
            .await
            .unwrap();
        service
            .add_ai_model(runtime_model("remove", "fixture-removed-key"))
            .await
            .unwrap();
        service
            .update_ai_model("keep", runtime_model("keep", "fixture-updated-model-key"))
            .await
            .unwrap();
        service.delete_ai_model("remove").await.unwrap();
        service.set_config("version", "0.2.18").await.unwrap();

        drop(service);
        let restarted = restart_test_service(&dir, name).await;
        let config: GlobalConfig = restarted.get_config(None).await.unwrap();
        assert_eq!(config.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(config.app.voice_call.api_key, voice.api_key);
        assert_eq!(config.ai.models.len(), 1);
        assert_eq!(config.ai.models[0].id, "keep");
        assert_eq!(config.ai.models[0].api_key, "fixture-updated-model-key");
    }

    #[tokio::test]
    async fn realtime_voice_survives_reload_racing_a_save() {
        use std::future::{poll_fn, Future};
        use std::task::Poll;

        let name = "realtime-voice-reload-race";
        let (service, dir) = test_service(name).await;
        let voice = realtime_voice_fixture();
        let manager = service.manager.write().await;
        let mut reload = std::pin::pin!(service.reload());
        let mut save = std::pin::pin!(service.set_config("app.voice_call", &voice));
        poll_fn(|cx| {
            assert!(reload.as_mut().poll(cx).is_pending());
            assert!(save.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        drop(manager);
        let (reloaded, saved) = tokio::join!(reload, save);
        reloaded.unwrap();
        saved.unwrap();

        let restarted = restart_test_service(&dir, name).await;
        let config: VoiceCallConfig = restarted.get_config(Some("app.voice_call")).await.unwrap();
        assert_eq!(config.api_key, voice.api_key);
        let in_memory: VoiceCallConfig = service.get_config(Some("app.voice_call")).await.unwrap();
        assert_eq!(in_memory.api_key, voice.api_key);
    }

    #[tokio::test]
    async fn realtime_voice_invalid_import_keeps_memory_and_disk_unchanged() {
        let (service, _dir) = test_service("realtime-voice-invalid-import").await;
        service
            .set_config("app.voice_call", realtime_voice_fixture())
            .await
            .unwrap();
        let config_dir = service.get_statistics().await.config_directory;
        let before = tokio::fs::read_to_string(config_dir.join("app.json"))
            .await
            .unwrap();
        let mut invalid = serde_json::to_value(GlobalConfig::default()).unwrap();
        invalid["app"]["voice_call"]["api_key"] = serde_json::Value::Null;
        let imported = service.import_account_settings(invalid).await.unwrap();
        assert!(!imported.success);
        assert_eq!(
            tokio::fs::read_to_string(config_dir.join("app.json"))
                .await
                .unwrap(),
            before
        );
        let voice: VoiceCallConfig = service.get_config(Some("app.voice_call")).await.unwrap();
        assert_eq!(voice.api_key, realtime_voice_fixture().api_key);
        assert!(!config_dir.join("backups").exists());
    }

    #[tokio::test]
    async fn runtime_ai_model_is_effective_but_never_persisted() {
        let (service, _dir) = test_service("runtime-overlay-test").await;

        service
            .install_runtime_ai_model(runtime_model("sdk:openai:fixture", "fixture-secret"))
            .await
            .unwrap();
        let runtime = service
            .get_runtime_ai_model("sdk:openai:fixture")
            .await
            .unwrap();
        assert_eq!(runtime.api_key, "fixture-secret");
        let effective_ai = service.get_effective_ai_config().await.unwrap();
        assert!(effective_ai
            .models
            .iter()
            .any(|model| model.id == "sdk:openai:fixture"));
        let persisted: GlobalConfig = service.get_config(None).await.unwrap();
        assert!(!persisted
            .ai
            .models
            .iter()
            .any(|model| model.id == "sdk:openai:fixture"));
        let persisted_models: Vec<AIModelConfig> =
            service.get_config(Some("ai.models")).await.unwrap();
        assert!(!persisted_models
            .iter()
            .any(|model| model.id == "sdk:openai:fixture"));
        let persisted_ai: AIConfig = service.get_config(Some("ai")).await.unwrap();
        assert!(!persisted_ai
            .models
            .iter()
            .any(|model| model.id == "sdk:openai:fixture"));

        let export = service.export_config().await.unwrap();
        let export_json = serde_json::to_string(&export).unwrap();
        assert!(!export_json.contains("sdk:openai:fixture"));
        assert!(!export_json.contains("fixture-secret"));

        service
            .reconcile_models("runtime-overlay-test")
            .await
            .unwrap();
        service
            .add_ai_model(model("persisted", true, ModelCategory::GeneralChat))
            .await
            .unwrap();
        let app_file = service
            .get_statistics()
            .await
            .config_directory
            .join("app.json");
        let disk = tokio::fs::read_to_string(app_file).await.unwrap();
        assert!(!disk.contains("sdk:openai:fixture"));
        assert!(!disk.contains("fixture-secret"));

        let backup = service.create_backup().await.unwrap();
        let backup_text = tokio::fs::read_to_string(backup).await.unwrap();
        assert!(!backup_text.contains("sdk:openai:fixture"));
        assert!(!backup_text.contains("fixture-secret"));

        service.remove_runtime_ai_model("sdk:openai:fixture").await;
        let effective = service.get_effective_ai_config().await.unwrap();
        assert!(!effective
            .models
            .iter()
            .any(|model| model.id == "sdk:openai:fixture"));
        assert!(service
            .get_runtime_ai_model("sdk:openai:fixture")
            .await
            .is_none());
    }

    #[tokio::test]
    async fn runtime_ai_model_overlays_a_persisted_duplicate_for_effective_reads() {
        let (service, _dir) = test_service("runtime-overlay-duplicate-test").await;
        service
            .add_ai_model(runtime_model("sdk:openai:duplicate", "persisted-secret"))
            .await
            .unwrap();
        service
            .install_runtime_ai_model(runtime_model("sdk:openai:duplicate", "runtime-secret"))
            .await
            .unwrap();

        let effective = service.get_effective_ai_config().await.unwrap();
        let matches = effective
            .models
            .iter()
            .filter(|model| model.id == "sdk:openai:duplicate")
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].api_key, "runtime-secret");

        let persisted: AIConfig = service.get_config(Some("ai")).await.unwrap();
        let persisted = persisted
            .models
            .iter()
            .find(|model| model.id == "sdk:openai:duplicate")
            .unwrap();
        assert_eq!(persisted.api_key, "persisted-secret");
    }

    #[tokio::test]
    async fn review_team_policy_config_survives_service_restart() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(
            dir.path().join("review-team-concurrency"),
        ));
        let settings = || ConfigManagerSettings {
            path_manager: Some(path_manager.clone()),
            auto_save: true,
            backup_count: 0,
        };

        let service = ConfigService::with_settings(settings())
            .await
            .expect("config service should start");
        service
            .set_config(
                "ai.review_teams.default",
                serde_json::json!({
                    "max_retries_per_role": 2,
                    "max_parallel_reviewers": 1,
                    "max_queue_wait_seconds": 45,
                    "allow_provider_capacity_queue": false,
                    "allow_bounded_auto_retry": true,
                    "auto_retry_elapsed_guard_seconds": 240,
                }),
            )
            .await
            .expect("review team concurrency config should save");

        let persisted: serde_json::Value = serde_json::from_str(
            &tokio::fs::read_to_string(path_manager.app_config_file())
                .await
                .expect("review team config should be persisted"),
        )
        .expect("persisted config should be valid JSON");
        let persisted_team = &persisted["ai"]["review_teams"]["default"];
        assert_eq!(persisted_team["max_retries_per_role"], serde_json::json!(2));
        assert_eq!(
            persisted_team["max_parallel_reviewers"],
            serde_json::json!(1)
        );
        assert_eq!(
            persisted_team["max_queue_wait_seconds"],
            serde_json::json!(45)
        );
        assert_eq!(
            persisted_team["allow_provider_capacity_queue"],
            serde_json::json!(false)
        );
        assert_eq!(
            persisted_team["allow_bounded_auto_retry"],
            serde_json::json!(true)
        );
        assert_eq!(
            persisted_team["auto_retry_elapsed_guard_seconds"],
            serde_json::json!(240)
        );

        drop(service);
        let reloaded_service = ConfigService::with_settings(settings())
            .await
            .expect("config service should reload");
        let reloaded: serde_json::Value = reloaded_service
            .get_config(Some("ai.review_teams.default"))
            .await
            .expect("review team config should be readable after reload");
        assert_eq!(reloaded["max_retries_per_role"], serde_json::json!(2));
        assert_eq!(reloaded["max_parallel_reviewers"], serde_json::json!(1));
        assert_eq!(reloaded["max_queue_wait_seconds"], serde_json::json!(45));
        assert_eq!(
            reloaded["allow_provider_capacity_queue"],
            serde_json::json!(false)
        );
        assert_eq!(
            reloaded["allow_bounded_auto_retry"],
            serde_json::json!(true)
        );
        assert_eq!(
            reloaded["auto_retry_elapsed_guard_seconds"],
            serde_json::json!(240)
        );
    }

    #[tokio::test]
    async fn compare_and_set_json_config_rejects_a_stale_snapshot() {
        let (service, _dir) = test_service("config-cas").await;
        assert!(service
            .compare_and_set_json_config(
                "mcp_servers",
                None,
                serde_json::json!({ "mcpServers": { "first": {} } }),
            )
            .await
            .unwrap());
        assert!(!service
            .compare_and_set_json_config(
                "mcp_servers",
                None,
                serde_json::json!({ "mcpServers": { "stale": {} } }),
            )
            .await
            .unwrap());
        let current = service
            .get_config::<serde_json::Value>(Some("mcp_servers"))
            .await
            .unwrap();
        assert!(current["mcpServers"].get("first").is_some());
        assert!(current["mcpServers"].get("stale").is_none());
    }

    #[tokio::test]
    async fn startup_downgrades_structured_telemetry_without_losing_models() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user_root = dir.path().join("structured-telemetry-compatibility");
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(user_root));
        path_manager
            .initialize_user_directories()
            .await
            .expect("user directories");

        let mut config = GlobalConfig::default();
        config
            .ai
            .models
            .push(model("configured-model", true, ModelCategory::GeneralChat));
        let mut config_value = serde_json::to_value(config).expect("serialize config");
        config_value["app"]["telemetry"] = serde_json::json!({
            "version": 2,
            "level": "basic",
            "sensitive_content_consent": false,
        });
        let original = serde_json::to_string_pretty(&config_value).expect("format config");
        tokio::fs::write(path_manager.app_config_file(), &original)
            .await
            .expect("seed config");

        let service = ConfigService::with_settings(ConfigManagerSettings {
            path_manager: Some(path_manager.clone()),
            auto_save: true,
            backup_count: 5,
        })
        .await
        .expect("config service should recover the telemetry field");

        let loaded: GlobalConfig = service.get_config(None).await.expect("loaded config");
        assert!(loaded
            .ai
            .models
            .iter()
            .any(|configured| configured.id == "configured-model"));

        let diagnostics = service.load_diagnostics().await;
        assert!(!diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "CONFIG_DEFAULT_RECOVERY"));
        let telemetry_diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "CONFIG_TELEMETRY_DOWNGRADED")
            .expect("telemetry compatibility diagnostic");
        assert_eq!(telemetry_diagnostic.path, "app.telemetry");
        assert_eq!(
            telemetry_diagnostic.recoverability,
            ConfigDiagnosticRecoverability::AutoFix
        );

        let persisted: serde_json::Value = serde_json::from_str(
            &tokio::fs::read_to_string(path_manager.app_config_file())
                .await
                .expect("persisted config"),
        )
        .expect("valid persisted config");
        assert_eq!(persisted["app"]["telemetry"], serde_json::json!(false));

        let backups = std::fs::read_dir(path_manager.user_config_dir().join("backups"))
            .expect("backup directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("backup entries");
        assert_eq!(backups.len(), 1);
        assert!(backups[0]
            .file_name()
            .to_string_lossy()
            .contains("startup-normalization"));
        assert_eq!(
            tokio::fs::read_to_string(backups[0].path())
                .await
                .expect("backup content"),
            original
        );
    }

    #[tokio::test]
    async fn startup_repairs_speech_sentinels_and_creates_a_backup() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user_root = dir.path().join("speech-startup-repair");
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(user_root));
        path_manager
            .initialize_user_directories()
            .await
            .expect("user directories");
        let mut config = GlobalConfig::default();
        config.ai.models.push(AIModelConfig {
            id: "speech".to_string(),
            name: "Qwen ASR".to_string(),
            provider: "openai".to_string(),
            model_name: "qwen-asr".to_string(),
            base_url: "https://example.com/v1".to_string(),
            api_key: "secret".to_string(),
            enabled: true,
            category: ModelCategory::SpeechRecognition,
            capabilities: vec![ModelCapability::SpeechRecognition],
            context_window: Some(0),
            max_tokens: Some(0),
            ..Default::default()
        });
        config.ai.default_models.speech_recognition = Some("speech".to_string());
        tokio::fs::write(
            path_manager.app_config_file(),
            serde_json::to_vec_pretty(&config).expect("serialize config"),
        )
        .await
        .expect("seed config");

        let service = ConfigService::with_settings(ConfigManagerSettings {
            path_manager: Some(path_manager.clone()),
            auto_save: true,
            backup_count: 5,
        })
        .await
        .expect("config service should recover");

        let repaired: GlobalConfig = service.get_config(None).await.expect("repaired config");
        let speech = repaired
            .ai
            .models
            .iter()
            .find(|model| model.id == "speech")
            .expect("speech model");
        assert_eq!(speech.context_window, None);
        assert_eq!(speech.max_tokens, None);
        assert_eq!(
            repaired.ai.default_models.speech_recognition.as_deref(),
            Some("speech")
        );
        assert!(service
            .load_diagnostics()
            .await
            .iter()
            .any(|diagnostic| diagnostic.code == "MODEL_FIELD_NOT_APPLICABLE"));
        let backups = std::fs::read_dir(path_manager.user_config_dir().join("backups"))
            .expect("backup directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("backup entries");
        assert_eq!(backups.len(), 1);
    }

    #[tokio::test]
    async fn malformed_json_uses_in_memory_defaults_and_preserves_the_original() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user_root = dir.path().join("invalid-json-recovery");
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(user_root));
        path_manager
            .initialize_user_directories()
            .await
            .expect("user directories");
        let broken = "{\"ai\": {\"models\": [";
        tokio::fs::write(path_manager.app_config_file(), broken)
            .await
            .expect("seed broken config");

        let service = ConfigService::with_settings(ConfigManagerSettings {
            path_manager: Some(path_manager.clone()),
            auto_save: true,
            backup_count: 5,
        })
        .await
        .expect("startup should use defaults");

        assert_eq!(
            tokio::fs::read_to_string(path_manager.app_config_file())
                .await
                .expect("original config"),
            broken
        );
        let diagnostic = service
            .load_diagnostics()
            .await
            .into_iter()
            .find(|diagnostic| diagnostic.code == "CONFIG_DEFAULT_RECOVERY")
            .expect("recovery diagnostic");
        assert!(!diagnostic.message.contains("api_key"));
        let backup = std::fs::read_dir(path_manager.user_config_dir().join("backups"))
            .expect("backup directory")
            .next()
            .expect("backup entry")
            .expect("backup path")
            .path();
        assert_eq!(
            tokio::fs::read_to_string(backup)
                .await
                .expect("backup content"),
            broken
        );
    }

    #[tokio::test]
    async fn cloud_speech_save_updates_all_owned_fields_in_one_persisted_config() {
        let test_name = "atomic-cloud-speech";
        let (service, dir) = test_service(test_name).await;
        let result = service
            .save_cloud_speech_config(SaveCloudSpeechConfigRequest {
                config_id: Some("speech-cloud".to_string()),
                preset: "qwen".to_string(),
                name: "Qwen ASR".to_string(),
                base_url: "https://example.com/v1/".to_string(),
                request_url: None,
                model_name: "qwen-asr".to_string(),
                api_key: "secret".to_string(),
            })
            .await
            .expect("speech config should save");
        assert!(result.created);

        let path_manager = PathManager::with_user_root_for_tests(dir.path().join(test_name));
        let persisted: GlobalConfig = serde_json::from_slice(
            &tokio::fs::read(path_manager.app_config_file())
                .await
                .expect("persisted config"),
        )
        .expect("valid persisted config");
        let model = persisted
            .ai
            .models
            .iter()
            .find(|model| model.id == "speech-cloud")
            .expect("speech model");
        assert_eq!(model.context_window, None);
        assert_eq!(model.max_tokens, None);
        assert_eq!(
            model.request_url.as_deref(),
            Some("https://example.com/v1/audio/transcriptions")
        );
        assert_eq!(
            persisted.ai.default_models.speech_recognition.as_deref(),
            Some("speech-cloud")
        );
        assert_eq!(persisted.app.ai_experience.voice_input.provider, "cloud");
        assert_eq!(
            persisted.app.ai_experience.voice_input.model_id,
            "speech-cloud"
        );
    }

    #[tokio::test]
    async fn cloud_speech_save_reconciles_text_references_when_reusing_a_model_id() {
        let (service, _dir) = test_service("cloud-speech-reused-id").await;
        service
            .set_config(
                "ai.models",
                vec![model("reused-model", true, ModelCategory::GeneralChat)],
            )
            .await
            .expect("text model should save");

        let before: GlobalConfig = service.get_config(None).await.expect("config before save");
        assert_eq!(
            before.ai.default_models.primary.as_deref(),
            Some("reused-model")
        );
        assert_eq!(before.ai.default_models.fast, None);

        let result = service
            .save_cloud_speech_config(SaveCloudSpeechConfigRequest {
                config_id: Some("reused-model".to_string()),
                preset: "custom".to_string(),
                name: "Speech replacement".to_string(),
                base_url: "https://example.com/v1".to_string(),
                request_url: None,
                model_name: "speech-model".to_string(),
                api_key: "secret".to_string(),
            })
            .await
            .expect("speech replacement should save");
        assert!(!result.created);

        let after: GlobalConfig = service.get_config(None).await.expect("config after save");
        assert_eq!(after.ai.default_models.primary, None);
        assert_eq!(after.ai.default_models.fast, None);
        assert_eq!(
            after.ai.default_models.speech_recognition.as_deref(),
            Some("reused-model")
        );
        assert_ne!(
            after.ai.task_models.session_title.fixed_model_id(),
            Some("reused-model")
        );
        assert_ne!(
            after.ai.task_models.git_commit.fixed_model_id(),
            Some("reused-model")
        );
    }

    #[tokio::test]
    async fn clearing_fast_model_persists_unset_and_resolves_to_primary() {
        let test_name = "clear-fast-model";
        let (service, dir) = test_service(test_name).await;
        service
            .set_config(
                "ai.models",
                vec![
                    model("first-text", true, ModelCategory::GeneralChat),
                    model("primary-text", true, ModelCategory::GeneralChat),
                ],
            )
            .await
            .expect("models should save");
        service
            .set_config(
                "ai.default_models",
                &DefaultModelsConfig {
                    primary: Some("primary-text".to_string()),
                    fast: None,
                    ..Default::default()
                },
            )
            .await
            .expect("defaults should save");

        let current: GlobalConfig = service.get_config(None).await.expect("current config");
        assert_eq!(current.ai.default_models.fast, None);
        assert_eq!(
            current.ai.resolve_model_selection("fast").as_deref(),
            Some("primary-text")
        );

        let path_manager = PathManager::with_user_root_for_tests(dir.path().join(test_name));
        let persisted: GlobalConfig = serde_json::from_slice(
            &tokio::fs::read(path_manager.app_config_file())
                .await
                .expect("persisted config"),
        )
        .expect("valid persisted config");
        assert_eq!(persisted.ai.default_models.fast, None);
    }

    #[tokio::test]
    async fn set_config_rejects_invalid_reasoning_and_rolls_back() {
        let (service, _dir) = test_service("invalid-reasoning-set").await;
        let valid_models = vec![model("stable-model", true, ModelCategory::GeneralChat)];
        service
            .set_config("ai.models", &valid_models)
            .await
            .expect("valid models should save");

        let invalid_models = vec![AIModelConfig {
            reasoning: Some(bitfun_core_types::ReasoningConfig {
                presets: vec![bitfun_core_types::ReasoningPreset {
                    id: "bad-budget".to_string(),
                    actions: vec![bitfun_core_types::ReasoningPresetAction::BudgetTokens {
                        value: 0,
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..model("invalid-model", true, ModelCategory::GeneralChat)
        }];

        let error = service
            .set_config("ai.models", &invalid_models)
            .await
            .expect_err("invalid reasoning must be rejected");
        assert!(error
            .to_string()
            .contains("budget_tokens value must be greater than 0"));

        let persisted_models: Vec<AIModelConfig> = service
            .get_config(Some("ai.models"))
            .await
            .expect("models should remain readable");
        assert_eq!(persisted_models.len(), 1);
        assert_eq!(persisted_models[0].id, "stable-model");
    }

    #[tokio::test]
    async fn import_config_rejects_duplicate_reasoning_presets() {
        let (service, _dir) = test_service("invalid-reasoning-import").await;
        let mut config = GlobalConfig::default();
        config.ai.models.push(AIModelConfig {
            reasoning: Some(bitfun_core_types::ReasoningConfig {
                presets: vec![
                    bitfun_core_types::ReasoningPreset {
                        id: "same".to_string(),
                        actions: vec![bitfun_core_types::ReasoningPresetAction::Toggle {
                            enabled: true,
                        }],
                        ..Default::default()
                    },
                    bitfun_core_types::ReasoningPreset {
                        id: "same".to_string(),
                        actions: vec![bitfun_core_types::ReasoningPresetAction::Toggle {
                            enabled: false,
                        }],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..model("duplicate-model", true, ModelCategory::GeneralChat)
        });

        let result = service
            .import_config_data(serde_json::to_value(config).expect("serialize config"))
            .await
            .expect("import should return a structured result");

        assert!(!result.success);
        assert!(result
            .errors
            .join(" ")
            .contains("duplicate preset ID 'same'"));
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
        assert_eq!(defaults.mode, "primary");
        assert_eq!(
            defaults.subagents.default_selection,
            SubagentModelSelection::fixed("fast")
        );
        assert_eq!(
            defaults.subagents.builtin,
            HashMap::from([(
                "ResearchSpecialist".to_string(),
                SubagentModelSelection::Inherit,
            )])
        );
        assert_eq!(defaults.subagents.fork, SubagentModelSelection::Inherit);
    }

    #[tokio::test]
    async fn appearance_selection_round_trips_through_the_typed_config() {
        let (service, _dir) = test_service("appearance-selection").await;

        service
            .set_config("appearance.selection", "bitfun-dark")
            .await
            .expect("appearance selection should save");

        let selection: String = service
            .get_config(Some("appearance.selection"))
            .await
            .expect("appearance selection should load");
        assert_eq!(selection, "bitfun-dark");

        let export: GlobalConfig = service
            .get_config(None)
            .await
            .expect("full config should load");
        let serialized = serde_json::to_value(export).expect("config should serialize");
        assert_eq!(serialized["appearance"]["selection"], "bitfun-dark");
        assert!(serialized.get("theme").is_none());
        assert!(serialized.get("themes").is_none());
    }

    #[tokio::test]
    async fn raw_import_migrates_legacy_skip_confirmation_and_removes_it_from_disk() {
        let test_name = "legacy-skip-confirmation-raw-import";
        let (service, dir) = test_service(test_name).await;
        let mut raw_config =
            serde_json::to_value(GlobalConfig::default()).expect("default config should serialize");
        let raw_object = raw_config
            .as_object_mut()
            .expect("default config should serialize as an object");
        raw_object.remove("tool_permissions");
        raw_object
            .get_mut("ai")
            .and_then(serde_json::Value::as_object_mut)
            .expect("default config should include an AI object")
            .insert(
                "skip_tool_confirmation".to_string(),
                serde_json::Value::Bool(true),
            );

        service
            .import_config_data(raw_config)
            .await
            .expect("legacy confirmation preference should import");

        let permissions: serde_json::Value = service
            .get_config(Some("tool_permissions"))
            .await
            .expect("migrated tool permissions should be readable");
        assert_eq!(permissions["policy"]["preset"], "ask");
        assert_eq!(permissions["interaction"]["auto_approve_ask"], true);

        let path_manager = PathManager::with_user_root_for_tests(dir.path().join(test_name));
        let persisted: serde_json::Value = serde_json::from_str(
            &tokio::fs::read_to_string(path_manager.app_config_file())
                .await
                .expect("migrated config should be persisted"),
        )
        .expect("persisted config should be valid JSON");
        assert!(persisted["ai"].get("skip_tool_confirmation").is_none());
        assert_eq!(
            persisted["tool_permissions"]["interaction"]["auto_approve_ask"],
            true
        );
    }

    #[tokio::test]
    async fn startup_discards_removed_model_reasoning_fields_without_creating_a_preset() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user_root = dir.path().join("legacy-model-reasoning-startup");
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(user_root));
        let settings = || ConfigManagerSettings {
            path_manager: Some(path_manager.clone()),
            auto_save: true,
            backup_count: 0,
        };

        let initial_service = ConfigService::with_settings(settings())
            .await
            .expect("initial config service");
        drop(initial_service);

        let mut raw_config =
            serde_json::to_value(GlobalConfig::default()).expect("default config should serialize");
        raw_config["version"] = serde_json::json!(env!("CARGO_PKG_VERSION"));
        raw_config["ai"]["models"] = serde_json::json!([{
            "id": "legacy-model",
            "name": "Legacy model",
            "provider": "responses",
            "model_name": "gpt-test",
            "base_url": "https://example.com/v1",
            "api_key": "key",
            "enabled": true,
            "reasoning_mode": "enabled",
            "reasoning_effort": "high"
        }]);
        tokio::fs::write(
            path_manager.app_config_file(),
            serde_json::to_string_pretty(&raw_config).expect("legacy config should serialize"),
        )
        .await
        .expect("legacy config should be written");

        let migrated_service = ConfigService::with_settings(settings())
            .await
            .expect("legacy config should reload");
        drop(migrated_service);

        let persisted: serde_json::Value = serde_json::from_str(
            &tokio::fs::read_to_string(path_manager.app_config_file())
                .await
                .expect("migrated config should be persisted"),
        )
        .expect("persisted config should be valid JSON");
        let model = &persisted["ai"]["models"][0];
        assert!(model.get("reasoning").is_none());
        assert!(model.get("reasoning_mode").is_none());
        assert!(model.get("reasoning_effort").is_none());
        assert!(model.get("thinking_budget_tokens").is_none());
        assert!(model.get("enable_thinking_process").is_none());
    }

    #[tokio::test]
    async fn startup_rejects_invalid_canonical_reasoning_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user_root = dir.path().join("invalid-reasoning-startup");
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(user_root));
        let settings = || ConfigManagerSettings {
            path_manager: Some(path_manager.clone()),
            auto_save: true,
            backup_count: 0,
        };

        let initial_service = ConfigService::with_settings(settings())
            .await
            .expect("initial config service");
        drop(initial_service);

        let mut raw_config =
            serde_json::to_value(GlobalConfig::default()).expect("default config should serialize");
        raw_config["version"] = serde_json::json!(env!("CARGO_PKG_VERSION"));
        raw_config["ai"]["models"] = serde_json::json!([{
            "id": "invalid-model",
            "name": "Invalid model",
            "provider": "responses",
            "model_name": "gpt-5.4",
            "base_url": "https://api.openai.com/v1/responses",
            "api_key": "key",
            "enabled": true,
            "reasoning": {
                "catalog": { "source": "disabled" },
                "default_preset": "missing",
                "presets": []
            }
        }]);
        tokio::fs::write(
            path_manager.app_config_file(),
            serde_json::to_string_pretty(&raw_config).expect("invalid config should serialize"),
        )
        .await
        .expect("invalid config should be written");

        let error = match ConfigService::with_settings(settings()).await {
            Ok(_) => panic!("invalid canonical reasoning must fail startup"),
            Err(error) => error,
        };

        assert!(error
            .to_string()
            .contains("default preset 'missing' is not available"));
    }
}
