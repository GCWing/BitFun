//! Configuration manager implementation
//!
//! A complete configuration management system based on the Provider mechanism.

use super::normalization::{
    isolate_invalid_ai_models, normalize_config_value, normalize_typed_config,
    reconcile_model_references, reject_unsupported_schema,
};
use super::providers::ConfigProviderRegistry;
use super::types::*;
use crate::infrastructure::{try_get_path_manager_arc, PathManager};
use crate::util::errors::*;
use bitfun_services_core::json_store::JsonFileStore;
use log::{debug, info, warn};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;

fn invalid_config_error(context: &str, result: &ConfigValidationResult) -> BitFunError {
    let messages = result
        .errors
        .iter()
        .map(|error| format!("{}: {}", error.path, error.message))
        .collect::<Vec<_>>()
        .join(", ");
    BitFunError::validation(format!("{context}: {messages}"))
}

fn canonical_config_path(path: &str) -> &str {
    match path {
        "ai.review_teams.rate_limit_status" => "ai.review_team_rate_limit_status",
        _ => path,
    }
}

/// Moves the only trustworthy legacy mode choice into the new default domain.
///
/// Historical global model switching rewrote every `ai.agent_models` entry,
/// including builtin subagents. Only the `agentic` mode entry is used as a
/// migration hint when the new defaults are absent. The entire legacy mapping
/// is removed after normalization.
pub(crate) fn normalize_legacy_agent_model_defaults_config_value(mut config: Value) -> Value {
    let Some(root) = config.as_object_mut() else {
        return config;
    };
    let ai = root
        .entry("ai".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let Some(ai) = ai.as_object_mut() else {
        return config;
    };

    if !ai.contains_key("agent_model_defaults") {
        let mode = ai
            .get("agent_models")
            .and_then(Value::as_object)
            .and_then(|models| models.get("agentic"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|model| !model.is_empty())
            .unwrap_or("primary")
            .to_string();

        let defaults = AgentModelDefaultsConfig {
            mode,
            ..Default::default()
        };
        ai.insert(
            "agent_model_defaults".to_string(),
            serde_json::to_value(defaults).expect("agent model defaults should always serialize"),
        );
    }

    ai.remove("agent_models");

    config
}

/// Moves the retired global confirmation preference into the permission V2
/// interaction setting before Serde drops the legacy field.
pub(crate) fn normalize_legacy_tool_permissions_config_value(mut config: Value) -> Value {
    let Some(root) = config.as_object_mut() else {
        return config;
    };

    let has_tool_permissions = root.contains_key("tool_permissions");
    let legacy_skip_confirmation = root
        .get_mut("ai")
        .and_then(Value::as_object_mut)
        .and_then(|ai| ai.remove("skip_tool_confirmation"))
        .and_then(|value| value.as_bool());

    if !has_tool_permissions {
        if let Some(auto_approve_ask) = legacy_skip_confirmation {
            root.insert(
                "tool_permissions".to_string(),
                serde_json::json!({
                    "policy": {
                        "preset": "ask",
                        "rules": [],
                    },
                    "interaction": {
                        "auto_approve_ask": auto_approve_ask,
                    },
                }),
            );
        }
    }

    config
}

/// Removes retired per-model reasoning controls without inferring a preset.
///
/// The canonical `reasoning` field, when present, remains authoritative. Older
/// controls are intentionally discarded so an upgrade cannot silently create
/// a surprising default preset.
pub(crate) fn strip_removed_model_reasoning_fields(mut config: Value) -> Value {
    let Some(models) = config
        .get_mut("ai")
        .and_then(Value::as_object_mut)
        .and_then(|ai| ai.get_mut("models"))
        .and_then(Value::as_array_mut)
    else {
        return config;
    };

    for model in models {
        let Some(model) = model.as_object_mut() else {
            continue;
        };
        for key in [
            "enable_thinking_process",
            "reasoning_mode",
            "reasoning_effort",
            "thinking_budget_tokens",
        ] {
            model.remove(key);
        }
    }

    config
}

fn config_value_for_persistence(config: &GlobalConfig) -> BitFunResult<Value> {
    let mut value = serde_json::to_value(config)
        .map_err(|e| BitFunError::config(format!("Failed to serialize config: {}", e)))?;
    prune_default_ai_tool_argument_json_repair(&mut value);
    prune_default_ai_max_rounds(&mut value);
    prune_default_memories_config(&mut value)?;
    Ok(value)
}

fn prune_default_ai_tool_argument_json_repair(config_value: &mut Value) {
    let Some(ai_config) = config_value.get_mut("ai").and_then(Value::as_object_mut) else {
        return;
    };

    if ai_config.get("allow_tool_json_repair") == Some(&Value::Bool(true)) {
        ai_config.remove("allow_tool_json_repair");
    }
}

fn prune_default_ai_max_rounds(config_value: &mut Value) {
    let Some(ai_config) = config_value.get_mut("ai").and_then(Value::as_object_mut) else {
        return;
    };

    if ai_config.get("max_rounds").and_then(Value::as_u64)
        == Some(crate::service::config::types::DEFAULT_MAX_ROUNDS as u64)
    {
        ai_config.remove("max_rounds");
    }
}

fn prune_default_memories_config(config_value: &mut Value) -> BitFunResult<()> {
    let Some(config_object) = config_value.as_object_mut() else {
        return Ok(());
    };
    let Some(memories_value) = config_object.get_mut("memories") else {
        return Ok(());
    };

    let default_memories = serde_json::to_value(MemoriesConfig::default()).map_err(|e| {
        BitFunError::config(format!(
            "Failed to serialize default memories config: {}",
            e
        ))
    })?;
    let Some(default_memories_object) = default_memories.as_object() else {
        return Ok(());
    };
    let Some(memories_object) = memories_value.as_object_mut() else {
        return Ok(());
    };

    memories_object.retain(|key, value| default_memories_object.get(key) != Some(value));

    if memories_object.is_empty() {
        config_object.remove("memories");
    }

    Ok(())
}

/// Configuration manager.
pub struct ConfigManager {
    config_dir: PathBuf,
    config: GlobalConfig,
    providers: ConfigProviderRegistry,
    config_file: PathBuf,
    path_manager: Arc<PathManager>,
    backup_count: usize,
    load_diagnostics: Vec<ConfigDiagnostic>,
}

/// Configuration manager settings.
#[derive(Debug, Clone)]
pub struct ConfigManagerSettings {
    pub path_manager: Option<Arc<PathManager>>,
    pub auto_save: bool,
    pub backup_count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfigImportSource {
    Explicit,
    AccountSync,
}

impl Default for ConfigManagerSettings {
    fn default() -> Self {
        Self {
            path_manager: None,
            auto_save: true,
            backup_count: 5,
        }
    }
}

impl ConfigManager {
    /// Creates a new unified configuration manager.
    pub async fn new(settings: ConfigManagerSettings) -> BitFunResult<Self> {
        let path_manager = match settings.path_manager {
            Some(path_manager) => path_manager,
            None => try_get_path_manager_arc()?,
        };

        path_manager.initialize_user_directories().await?;

        let config_dir = path_manager.user_config_dir();
        let config_file = path_manager.app_config_file();

        let providers = ConfigProviderRegistry::new();
        let backup_count = settings.backup_count;

        let mut manager = Self {
            config_dir,
            config: GlobalConfig::default(),
            providers,
            config_file,
            path_manager,
            backup_count,
            load_diagnostics: Vec::new(),
        };

        manager.load_or_create_config().await?;
        #[cfg(feature = "ai-adapter-runtime")]
        {
            bitfun_ai_adapters::diagnostics::set_include_sensitive_diagnostics(
                manager.config.app.logging.include_sensitive_diagnostics,
            );
        }

        debug!("ConfigManager initialized at {:?}", manager.config_file);
        Ok(manager)
    }

    /// Returns the path manager.
    pub fn path_manager(&self) -> &Arc<PathManager> {
        &self.path_manager
    }

    /// Reloads from the same storage root while the service holds its write
    /// lock, so a concurrent save cannot be replaced by an older disk read.
    pub(crate) async fn reload(&mut self) -> BitFunResult<()> {
        let settings = ConfigManagerSettings {
            path_manager: Some(self.path_manager.clone()),
            auto_save: true,
            backup_count: self.backup_count,
        };
        *self = Self::new(settings).await?;
        Ok(())
    }

    /// Loads or creates the configuration file.
    async fn load_or_create_config(&mut self) -> BitFunResult<()> {
        if self.config_file.exists() {
            self.load_existing_config().await?;
        } else {
            self.create_default_config().await?;
        }

        Ok(())
    }

    /// Creates the first config file using the already initialized defaults.
    async fn create_default_config(&mut self) -> BitFunResult<()> {
        self.config.version = env!("CARGO_PKG_VERSION").to_string();
        self.save_config().await?;
        debug!("Created default config file");
        Ok(())
    }

    /// Loads an existing config file and migrates it if needed.
    async fn load_existing_config(&mut self) -> BitFunResult<()> {
        let content = fs::read_to_string(&self.config_file)
            .await
            .map_err(|e| BitFunError::config(format!("Failed to read config file: {}", e)))?;

        let config_value: Value = match serde_json::from_str(&content) {
            Ok(value) => value,
            Err(error) => {
                return self
                    .activate_default_recovery(
                        &content,
                        "invalid-json",
                        format!("Failed to parse config file as JSON: {error}"),
                    )
                    .await;
            }
        };
        let normalized = normalize_config_value(config_value);
        if let Err(error) = reject_unsupported_schema(&normalized.diagnostics) {
            return self
                .activate_default_recovery(&content, "unsupported-schema", error.to_string())
                .await;
        }
        let mut config_value = normalized.value;
        let mut load_diagnostics = normalized.diagnostics;
        let compatibility_normalized = normalized.changed;

        let file_version = config_value
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0")
            .to_string();

        let current_version = env!("CARGO_PKG_VERSION").to_string();

        let app_version_changed = !versions_match(&file_version, &current_version);
        if app_version_changed {
            info!(
                "Config application version updated: {} -> {}",
                file_version, current_version
            );
            if let Some(obj) = config_value.as_object_mut() {
                obj.insert(
                    "version".to_string(),
                    Value::String(current_version.clone()),
                );
            }
        }

        match serde_json::from_value::<GlobalConfig>(config_value.clone()) {
            Ok(mut config) => {
                load_diagnostics.extend(normalize_typed_config(&mut config));
                load_diagnostics.extend(isolate_invalid_ai_models(&mut config).await?);
                load_diagnostics.extend(reconcile_model_references(&mut config).diagnostics);

                self.config = config;

                let validation_result = self.validate_config().await?;
                if !validation_result.valid {
                    return Err(invalid_config_error(
                        "Invalid configuration file",
                        &validation_result,
                    ));
                }

                if compatibility_normalized || !load_diagnostics.is_empty() {
                    self.backup_raw_config(&content, "startup-normalization")
                        .await?;
                }
                if app_version_changed || compatibility_normalized || !load_diagnostics.is_empty() {
                    self.config.version = current_version;
                    self.save_config().await?;
                    info!(
                        "Config normalized and saved: diagnostics={}",
                        load_diagnostics.len()
                    );
                } else {
                    debug!("Loaded config from file");
                }

                self.load_diagnostics = load_diagnostics;

                Ok(())
            }
            Err(e) => {
                warn!(
                    "Config file deserialization failed, starting smart merge: {}",
                    e
                );
                self.backup_raw_config(&content, "pre-smart-merge").await?;

                match self.smart_merge_config_from_value(config_value).await {
                    Ok(()) => {
                        self.load_diagnostics.insert(
                            0,
                            ConfigDiagnostic {
                                path: "$".to_string(),
                                message: format!(
                                    "Repaired an incompatible configuration shape after typed deserialization failed: {e}"
                                ),
                                code: "CONFIG_SHAPE_REPAIRED".to_string(),
                                severity: ConfigDiagnosticSeverity::Warning,
                                recoverability: ConfigDiagnosticRecoverability::AutoFix,
                            },
                        );
                        Ok(())
                    }
                    Err(merge_error) => {
                        self.activate_default_recovery(
                            &content,
                            "invalid-shape",
                            format!(
                                "Config deserialization and smart merge failed: deserialize={e}; merge={merge_error}"
                            ),
                        )
                        .await
                    }
                }
            }
        }
    }

    /// Performs a smart merge from a JSON value.
    async fn smart_merge_config_from_value(&mut self, user_value: Value) -> BitFunResult<()> {
        let user_value = normalize_config_value(user_value).value;
        let base_config = self.providers.get_default_config();

        let base_value = serde_json::to_value(&base_config).map_err(|e| {
            BitFunError::config(format!("Failed to serialize default config: {}", e))
        })?;
        let merged_value = deep_merge(base_value, user_value);

        let mut config: GlobalConfig = serde_json::from_value(merged_value).map_err(|e| {
            BitFunError::config(format!("Failed to deserialize merged config: {}", e))
        })?;

        let mut load_diagnostics = normalize_typed_config(&mut config);
        load_diagnostics.extend(isolate_invalid_ai_models(&mut config).await?);
        load_diagnostics.extend(reconcile_model_references(&mut config).diagnostics);

        self.config = config;

        let validation_result = self.validate_config().await?;
        if !validation_result.valid {
            return Err(invalid_config_error(
                "Invalid merged configuration file",
                &validation_result,
            ));
        }

        self.config.version = env!("CARGO_PKG_VERSION").to_string();
        self.save_config().await?;
        self.load_diagnostics = load_diagnostics;
        info!("Config automatically fixed and saved");

        Ok(())
    }

    async fn activate_default_recovery(
        &mut self,
        raw_content: &str,
        reason: &str,
        message: String,
    ) -> BitFunResult<()> {
        let backup_path = self.backup_raw_config(raw_content, reason).await?;
        self.config = self.providers.get_default_config();
        self.config.version = env!("CARGO_PKG_VERSION").to_string();
        self.config.schema_version = CURRENT_CONFIG_SCHEMA_VERSION;
        self.load_diagnostics = vec![ConfigDiagnostic {
            path: "$".to_string(),
            message: format!(
                "{message}. Started with in-memory defaults; original configuration was preserved at {}",
                backup_path.display()
            ),
            code: "CONFIG_DEFAULT_RECOVERY".to_string(),
            severity: ConfigDiagnosticSeverity::Warning,
            recoverability: ConfigDiagnosticRecoverability::DefaultsUsed,
        }];
        warn!(
            "Configuration recovery activated: reason={}, backup_path={}",
            reason,
            backup_path.display()
        );
        Ok(())
    }

    /// Saves the configuration file.
    async fn save_config(&self) -> BitFunResult<()> {
        self.persist_config(&self.config).await
    }

    async fn persist_config(&self, config: &GlobalConfig) -> BitFunResult<()> {
        let content = serde_json::to_string_pretty(&config_value_for_persistence(config)?)
            .map_err(|e| BitFunError::config(format!("Config serialization failed: {}", e)))?;

        if let Some(parent) = self.config_file.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await.map_err(|e| {
                    BitFunError::config(format!(
                        "Failed to create config directory {:?}: {}",
                        parent, e
                    ))
                })?;
            }
        }

        JsonFileStore
            .write_text_atomic_strict(&self.config_file, &content)
            .await
            .map_err(|e| {
                BitFunError::config(format!(
                    "Failed to atomically write config file {:?}: {}",
                    self.config_file, e
                ))
            })?;
        Ok(())
    }

    async fn backup_raw_config(&self, content: &str, reason: &str) -> BitFunResult<PathBuf> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S_%3f");
        let backup_dir = self.config_dir.join("backups");
        fs::create_dir_all(&backup_dir)
            .await
            .map_err(|e| BitFunError::config(format!("Failed to create backup directory: {e}")))?;
        let backup_file = backup_dir.join(format!(
            "app_{reason}_{timestamp}_{}.json",
            uuid::Uuid::new_v4().simple()
        ));
        fs::write(&backup_file, content)
            .await
            .map_err(|e| BitFunError::config(format!("Failed to write config backup: {e}")))?;
        self.prune_backups(&backup_dir).await?;
        info!(
            "Created config backup: reason={}, path={}",
            reason,
            backup_file.display()
        );
        Ok(backup_file)
    }

    async fn prune_backups(&self, backup_dir: &std::path::Path) -> BitFunResult<()> {
        if self.backup_count == 0 {
            return Ok(());
        }
        let mut entries = fs::read_dir(backup_dir)
            .await
            .map_err(|e| BitFunError::config(format!("Failed to read backup directory: {e}")))?;
        let mut files = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| BitFunError::config(format!("Failed to enumerate backups: {e}")))?
        {
            let is_repair_backup = entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with("app_") && name.ends_with(".json"));
            if !is_repair_backup {
                continue;
            }
            let metadata = entry
                .metadata()
                .await
                .map_err(|e| BitFunError::config(format!("Failed to inspect backup: {e}")))?;
            if metadata.is_file() {
                files.push((metadata.modified().ok(), entry.path()));
            }
        }
        files.sort_by_key(|(modified, _)| *modified);
        let remove_count = files.len().saturating_sub(self.backup_count);
        for (_, path) in files.into_iter().take(remove_count) {
            if let Err(error) = fs::remove_file(&path).await {
                warn!(
                    "Failed to prune old config backup: path={}, error={}",
                    path.display(),
                    error
                );
            }
        }
        Ok(())
    }

    /// Gets a configuration value (supports dot-paths).
    pub fn get<T>(&self, path: &str) -> BitFunResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let path = canonical_config_path(path);
        let value = self.get_value_by_path(path)?;
        serde_json::from_value(value).map_err(|e| {
            BitFunError::config(format!(
                "Failed to deserialize config value at '{}': {}",
                path, e
            ))
        })
    }

    /// Sets a configuration value (supports dot-paths).
    pub async fn set<T>(&mut self, path: &str, value: T) -> BitFunResult<()>
    where
        T: serde::Serialize,
    {
        let json_value = serde_json::to_value(value)
            .map_err(|e| BitFunError::config(format!("Failed to serialize config value: {}", e)))?;

        let path = canonical_config_path(path);
        let mut config = self.config_with_value(path, json_value)?;
        // Apply capability-driven canonicalization before validation and persistence.
        // Speech/embedding/image-only models must never carry text-generation sentinels.
        normalize_typed_config(&mut config);

        let validation_result = self.providers.validate_config(&config).await?;
        if !validation_result.valid {
            return Err(invalid_config_error(
                "Invalid configuration update",
                &validation_result,
            ));
        }

        self.commit_config(config, Some(path)).await
    }

    /// Resets configuration (supports dot-paths).
    pub async fn reset(&mut self, path: Option<&str>) -> BitFunResult<()> {
        let config = if let Some(path) = path {
            let path = canonical_config_path(path);
            let default_config = self.providers.get_default_config();
            let default_value = self.get_value_by_path_from_config(&default_config, path)?;
            self.config_with_value(path, default_value)?
        } else {
            self.providers.get_default_config()
        };

        let validation_result = self.providers.validate_config(&config).await?;
        if !validation_result.valid {
            return Err(invalid_config_error(
                "Invalid configuration reset",
                &validation_result,
            ));
        }

        self.commit_config(config, path.map(canonical_config_path))
            .await
    }

    /// Publish only a configuration that has reached disk. Failed writes must
    /// not change reads, runtime subscribers, or a subsequent unrelated save.
    async fn commit_config(
        &mut self,
        mut config: GlobalConfig,
        path: Option<&str>,
    ) -> BitFunResult<()> {
        config.last_modified = chrono::Utc::now();
        self.persist_config(&config).await?;
        let old_config = std::mem::replace(&mut self.config, config);
        let paths = match path.filter(|path| !path.is_empty()) {
            Some(path) => vec![path.to_string()],
            None => self.providers.get_provider_names(),
        };
        for path in paths {
            // A subscriber failure cannot undo an already committed file. Keep
            // notifying the other providers and report the refresh failure.
            if let Err(error) = self.notify_config_changed(&path, &old_config).await {
                warn!(
                    "Configuration saved but change notification failed: path={}, error={}",
                    path, error
                );
            }
        }
        Ok(())
    }

    /// Returns the full configuration.
    pub fn get_config(&self) -> &GlobalConfig {
        &self.config
    }

    pub fn load_diagnostics(&self) -> &[ConfigDiagnostic] {
        &self.load_diagnostics
    }

    /// Validates configuration.
    pub async fn validate_config(&self) -> BitFunResult<ConfigValidationResult> {
        self.providers.validate_config(&self.config).await
    }

    /// Exports configuration.
    pub fn export_config(&self) -> BitFunResult<serde_json::Value> {
        serde_json::to_value(&self.config)
            .map_err(|e| BitFunError::config(format!("Failed to export config: {}", e)))
    }

    /// Imports configuration.
    pub async fn import_config(&mut self, config_data: serde_json::Value) -> BitFunResult<()> {
        self.import_config_from_source(config_data, ConfigImportSource::Explicit)
            .await
    }

    pub(crate) async fn import_config_from_source(
        &mut self,
        mut config_data: Value,
        source: ConfigImportSource,
    ) -> BitFunResult<()> {
        self.preserve_imported_settings(&mut config_data, source)?;
        let normalized = normalize_config_value(config_data);
        reject_unsupported_schema(&normalized.diagnostics)?;

        let mut imported_config: GlobalConfig = serde_json::from_value(normalized.value)
            .map_err(|e| BitFunError::config(format!("Failed to parse imported config: {}", e)))?;

        let mut import_diagnostics = normalized.diagnostics;
        import_diagnostics.extend(normalize_typed_config(&mut imported_config));
        import_diagnostics.extend(isolate_invalid_ai_models(&mut imported_config).await?);
        import_diagnostics.extend(reconcile_model_references(&mut imported_config).diagnostics);

        let validation_result = self.providers.validate_config(&imported_config).await?;
        if !validation_result.valid {
            return Err(invalid_config_error(
                "Invalid imported config",
                &validation_result,
            ));
        }

        // Imports replace the whole document. Keep the exact previous file
        // recoverable before a cloud apply or an explicit backup restore.
        let previous_content = fs::read_to_string(&self.config_file).await.map_err(|e| {
            BitFunError::config(format!("Failed to read config before import backup: {e}"))
        })?;
        self.backup_raw_config(&previous_content, "pre-import")
            .await?;

        self.commit_config(imported_config, None).await?;
        self.load_diagnostics = import_diagnostics;

        info!("Successfully imported configuration");
        Ok(())
    }

    fn preserve_imported_settings(
        &self,
        config_data: &mut Value,
        source: ConfigImportSource,
    ) -> BitFunResult<()> {
        // Exported account snapshots serialize fixed defaults in full. Raw
        // on-disk backups intentionally elide memory/default AI preferences;
        // their omissions must continue to mean reset when explicitly restored.
        let mut shape = match source {
            ConfigImportSource::AccountSync => serde_json::to_value(GlobalConfig::default())?,
            ConfigImportSource::Explicit => config_value_for_persistence(&GlobalConfig::default())?,
        };
        let root = shape
            .as_object_mut()
            .expect("GlobalConfig serializes as an object");
        for key in ["version", "schema_version", "last_modified"] {
            root.remove(key);
        }
        // This optional record has a non-empty default, but None is omitted on
        // export. Do not mistake that intentional omission for an unknown field.
        shape["app"]["ai_experience"]
            .as_object_mut()
            .unwrap()
            .remove("agent_companion_pet");

        // Let actual legacy values reach their migrations before supplying a
        // local value at the new name; otherwise preservation masks the rename.
        if config_data.pointer("/ai/agent_models").is_some() {
            shape["ai"]
                .as_object_mut()
                .unwrap()
                .remove("agent_model_defaults");
        }
        if config_data.pointer("/ai/skip_tool_confirmation").is_some() {
            shape.as_object_mut().unwrap().remove("tool_permissions");
        }
        preserve_missing_config_fields(
            config_data,
            &serde_json::to_value(&self.config)?,
            &shape,
            "",
        );
        self.preserve_imported_voice_config(config_data, source)
    }

    fn preserve_imported_voice_config(
        &self,
        config_data: &mut Value,
        source: ConfigImportSource,
    ) -> BitFunResult<()> {
        let Some(root) = config_data.as_object_mut() else {
            return Ok(());
        };
        let app = root.entry("app").or_insert_with(|| serde_json::json!({}));
        let Some(app) = app.as_object_mut() else {
            return Ok(());
        };
        let voice = app
            .entry("voice_call")
            .or_insert_with(|| serde_json::json!({}));
        let Some(fields) = voice.as_object_mut() else {
            return Ok(());
        };

        // Older hosts omit this section; newer unconfigured hosts export an
        // empty key. Neither is a request to erase this controller's saved
        // credential. Explicit imports and local set/reset still honor clears.
        if source == ConfigImportSource::AccountSync
            && fields
                .get("api_key")
                .and_then(Value::as_str)
                .is_some_and(|key| key.trim().is_empty())
        {
            fields.remove("api_key");
        }
        *voice = deep_merge(
            serde_json::to_value(&self.config.app.voice_call)?,
            voice.take(),
        );
        Ok(())
    }

    /// Creates a configuration backup.
    pub async fn create_backup(&self) -> BitFunResult<PathBuf> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_dir = self.config_dir.join("backups");

        if !backup_dir.exists() {
            fs::create_dir_all(&backup_dir).await.map_err(|e| {
                BitFunError::config(format!("Failed to create backup directory: {}", e))
            })?;
        }

        let backup_file = backup_dir.join(format!(
            "config_backup_{}_{}.json",
            timestamp,
            uuid::Uuid::new_v4().simple()
        ));

        let content = serde_json::to_string_pretty(&config_value_for_persistence(&self.config)?)
            .map_err(|e| BitFunError::config(format!("Failed to serialize backup: {}", e)))?;

        JsonFileStore
            .write_text_atomic_create_new(&backup_file, &content)
            .await
            .map_err(|e| BitFunError::config(format!("Failed to write backup: {}", e)))?;

        info!("Created config backup: {:?}", backup_file);
        Ok(backup_file)
    }

    /// Registers a configuration provider.
    pub fn register_provider(&mut self, provider: Box<dyn ConfigProvider>) {
        self.providers.register(provider);
    }

    /// Returns configuration statistics.
    pub fn get_statistics(&self) -> ConfigStatistics {
        ConfigStatistics {
            total_ai_models: self.config.ai.models.len(),
            has_default_model: self.config.ai.default_models.primary.is_some(),
            config_directory: self.config_dir.clone(),
            providers_count: self.providers.get_provider_names().len(),
            last_modified: self.config.last_modified,
        }
    }

    /// Gets a configuration value by dot-path.
    fn get_value_by_path(&self, path: &str) -> BitFunResult<serde_json::Value> {
        self.get_value_by_path_from_config(&self.config, path)
    }

    /// Gets a configuration value by dot-path from the given config.
    fn get_value_by_path_from_config(
        &self,
        config: &GlobalConfig,
        path: &str,
    ) -> BitFunResult<serde_json::Value> {
        let config_value = serde_json::to_value(config)
            .map_err(|e| BitFunError::config(format!("Failed to serialize config: {}", e)))?;

        if path.is_empty() {
            return Ok(config_value);
        }

        let keys: Vec<&str> = path.split('.').collect();
        let mut current = &config_value;

        for key in keys {
            current = current.get(key).ok_or_else(|| {
                BitFunError::NotFound(format!("Config path '{}' not found", path))
            })?;
        }

        Ok(current.clone())
    }

    /// Builds a candidate configuration without changing the committed value.
    fn config_with_value(
        &self,
        path: &str,
        value: serde_json::Value,
    ) -> BitFunResult<GlobalConfig> {
        if path.is_empty() {
            return serde_json::from_value(value)
                .map_err(|e| BitFunError::config(format!("Failed to deserialize config: {}", e)));
        }

        let mut config_value = serde_json::to_value(&self.config)
            .map_err(|e| BitFunError::config(format!("Failed to serialize config: {}", e)))?;

        let keys: Vec<&str> = path.split('.').filter(|k| !k.is_empty()).collect();
        if keys.is_empty() {
            return serde_json::from_value(value)
                .map_err(|e| BitFunError::config(format!("Failed to deserialize config: {}", e)));
        }

        let last_key = keys.last().ok_or_else(|| {
            BitFunError::config(format!("Config path '{}' does not contain any keys", path))
        })?;
        let parent_keys = &keys[..keys.len() - 1];

        let mut current = &mut config_value;
        for key in parent_keys {
            current = current.get_mut(key).ok_or_else(|| {
                BitFunError::NotFound(format!("Config path '{}' not found", path))
            })?;
        }

        if let Some(obj) = current.as_object_mut() {
            obj.insert(last_key.to_string(), value);
        } else {
            return Err(BitFunError::config(format!(
                "Cannot set value at path '{}': parent is not an object",
                path
            )));
        }

        serde_json::from_value(config_value).map_err(|e| {
            BitFunError::config(format!("Failed to deserialize updated config: {}", e))
        })
    }

    /// Notifies about a configuration change.
    async fn notify_config_changed(
        &self,
        path: &str,
        old_config: &GlobalConfig,
    ) -> BitFunResult<()> {
        self.check_and_broadcast_app_change(path).await;
        self.check_and_broadcast_log_level_change(old_config).await;
        self.check_and_broadcast_sensitive_diagnostics_change(old_config)
            .await;

        self.providers
            .notify_config_changed(path, old_config, &self.config)
            .await
    }

    /// Detects and broadcasts app-scope configuration changes.
    async fn check_and_broadcast_app_change(&self, path: &str) {
        if path == "app" || path.starts_with("app.") {
            use super::global::{ConfigUpdateEvent, GlobalConfigManager};
            GlobalConfigManager::broadcast_update(ConfigUpdateEvent::AppUpdated).await;
        }
    }

    /// Detects and broadcasts runtime log-level changes.
    async fn check_and_broadcast_log_level_change(&self, old_config: &GlobalConfig) {
        let old_level = old_config.app.logging.level.trim().to_lowercase();
        let new_level = self.config.app.logging.level.trim().to_lowercase();

        if old_level != new_level {
            debug!(
                "App logging level change detected: {} -> {}",
                old_level, new_level
            );

            use super::global::{ConfigUpdateEvent, GlobalConfigManager};
            GlobalConfigManager::broadcast_update(ConfigUpdateEvent::LogLevelUpdated { new_level })
                .await;
        }
    }

    /// Detects and broadcasts runtime sensitive diagnostics changes.
    async fn check_and_broadcast_sensitive_diagnostics_change(&self, old_config: &GlobalConfig) {
        let old_include = old_config.app.logging.include_sensitive_diagnostics;
        let new_include = self.config.app.logging.include_sensitive_diagnostics;

        if old_include != new_include {
            debug!(
                "App logging sensitive diagnostics preference changed: {} -> {}",
                old_include, new_include
            );

            #[cfg(feature = "ai-adapter-runtime")]
            {
                bitfun_ai_adapters::diagnostics::set_include_sensitive_diagnostics(new_include);
            }

            use super::global::{ConfigUpdateEvent, GlobalConfigManager};
            GlobalConfigManager::broadcast_update(
                ConfigUpdateEvent::LoggingSensitiveDiagnosticsUpdated {
                    include_sensitive_diagnostics: new_include,
                },
            )
            .await;
        }
    }
}

/// Fill absent fixed fields only. Arrays, dynamic maps, tagged selections,
/// credentials bound to an endpoint, and permission-policy documents remain
/// authoritative replacements when supplied. A blanket deep merge resurrects
/// deleted entries and changes the meaning of explicit reset payloads.
fn preserve_missing_config_fields(incoming: &mut Value, local: &Value, shape: &Value, path: &str) {
    if matches!(
        path,
        "ai.review_teams"
            | "ai.agent_model_defaults.subagents.builtin"
            | "ai.proxy"
            | "tool_permissions"
    ) {
        return;
    }
    let (Some(incoming), Some(local), Some(shape)) = (
        incoming.as_object_mut(),
        local.as_object(),
        shape.as_object(),
    ) else {
        return;
    };
    if shape.contains_key("kind") || shape.contains_key("type") {
        return;
    }
    for (key, child_shape) in shape {
        let Some(local_value) = local.get(key) else {
            continue;
        };
        if let Some(value) = incoming.get_mut(key) {
            let child_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}.{key}")
            };
            preserve_missing_config_fields(value, local_value, child_shape, &child_path);
        } else {
            incoming.insert(key.clone(), local_value.clone());
        }
    }
}

/// Configuration statistics.
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigStatistics {
    pub total_ai_models: usize,
    pub has_default_model: bool,
    pub config_directory: PathBuf,
    pub providers_count: usize,
    pub last_modified: chrono::DateTime<chrono::Utc>,
}

/// Deeply merges JSON values.
///
/// Merges values from `overlay` into `base`:
/// - For objects, recursively merges all key/value pairs
/// - For other types, `overlay` overwrites `base`
/// - Keeps fields that exist in `base` but not in `overlay`
pub(crate) fn deep_merge(base: Value, overlay: Value) -> Value {
    match (base, overlay) {
        (Value::Object(mut base_obj), Value::Object(overlay_obj)) => {
            for (key, overlay_value) in overlay_obj {
                if let Some(base_value) = base_obj.get(&key) {
                    base_obj.insert(key.clone(), deep_merge(base_value.clone(), overlay_value));
                } else {
                    base_obj.insert(key.clone(), overlay_value);
                }
            }
            Value::Object(base_obj)
        }
        (_, overlay) => overlay,
    }
}

/// Returns whether two versions match.
pub(crate) fn versions_match(v1: &str, v2: &str) -> bool {
    v1 == v2
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_config_path, config_value_for_persistence,
        normalize_legacy_agent_model_defaults_config_value,
        normalize_legacy_tool_permissions_config_value, strip_removed_model_reasoning_fields,
    };
    use crate::service::config::types::GlobalConfig;

    #[test]
    fn canonicalizes_legacy_review_team_auxiliary_paths() {
        assert_eq!(
            canonical_config_path("ai.review_teams.rate_limit_status"),
            "ai.review_team_rate_limit_status"
        );
        assert_eq!(
            canonical_config_path("ai.review_teams.default"),
            "ai.review_teams.default"
        );
    }

    #[test]
    fn removed_model_reasoning_fields_are_stripped_without_creating_a_preset() {
        let normalized = strip_removed_model_reasoning_fields(serde_json::json!({
            "ai": {
                "models": [{
                    "id": "model-1",
                    "reasoning_mode": "adaptive",
                    "reasoning_effort": "high",
                    "thinking_budget_tokens": 12000
                }]
            }
        }));
        let model = &normalized["ai"]["models"][0];

        assert!(model.get("reasoning").is_none());
        assert!(model.get("reasoning_mode").is_none());
        assert!(model.get("reasoning_effort").is_none());
        assert!(model.get("thinking_budget_tokens").is_none());
    }

    #[test]
    fn canonical_model_reasoning_is_preserved_and_removed_fields_are_stripped() {
        let normalized = strip_removed_model_reasoning_fields(serde_json::json!({
            "ai": {
                "models": [{
                    "id": "model-1",
                    "reasoning": {
                        "catalog": { "source": "disabled" },
                        "default_preset": "custom",
                        "presets": [{
                            "id": "custom",
                            "setting": { "type": "effort", "value": "xhigh" }
                        }]
                    },
                    "reasoning_mode": "disabled",
                    "reasoning_effort": "low"
                }]
            }
        }));
        let model = &normalized["ai"]["models"][0];

        assert_eq!(model["reasoning"]["default_preset"], "custom");
        assert_eq!(
            model["reasoning"]["presets"][0]["setting"]["value"],
            "xhigh"
        );
        assert!(model.get("reasoning_mode").is_none());
        assert!(model.get("reasoning_effort").is_none());
    }

    #[test]
    fn legacy_agent_models_only_seed_the_shared_mode_default() {
        let normalized = normalize_legacy_agent_model_defaults_config_value(serde_json::json!({
            "ai": {
                "agent_models": {
                    "agentic": "primary",
                    "Explore": "expensive-model"
                }
            }
        }));

        assert_eq!(normalized["ai"]["agent_model_defaults"]["mode"], "primary");
        assert_eq!(
            normalized["ai"]["agent_model_defaults"]["subagents"]["default"],
            serde_json::json!({ "kind": "fixed", "model_id": "fast" })
        );
        assert_eq!(
            normalized["ai"]["agent_model_defaults"]["subagents"]["builtin"],
            serde_json::json!({
                "GeneralPurpose": { "kind": "fixed", "model_id": "primary" },
                "ResearchSpecialist": { "kind": "inherit" }
            })
        );
        assert_eq!(
            normalized["ai"]["agent_model_defaults"]["subagents"]["fork"],
            serde_json::json!({ "kind": "inherit" })
        );
        assert!(normalized["ai"].get("agent_models").is_none());
    }

    #[test]
    fn current_agent_model_defaults_win_before_legacy_mapping_is_removed() {
        let normalized = normalize_legacy_agent_model_defaults_config_value(serde_json::json!({
            "ai": {
                "agent_models": {
                    "agentic": "legacy-model"
                },
                "agent_model_defaults": {
                    "mode": "current-model",
                    "subagents": {
                        "default": { "kind": "fixed", "model_id": "fast" },
                        "builtin": {
                            "GeneralPurpose": { "kind": "fixed", "model_id": "primary" }
                        },
                        "fork": { "kind": "inherit" }
                    }
                }
            }
        }));

        assert_eq!(
            normalized["ai"]["agent_model_defaults"]["mode"],
            "current-model"
        );
        assert!(normalized["ai"].get("agent_models").is_none());
    }

    #[test]
    fn current_config_without_legacy_mapping_is_unchanged() {
        let config = serde_json::json!({
            "ai": {
                "agent_model_defaults": {
                    "mode": "current-model"
                }
            }
        });

        assert_eq!(
            normalize_legacy_agent_model_defaults_config_value(config.clone()),
            config
        );
    }

    #[test]
    fn legacy_skip_confirmation_migrates_to_auto_approve_and_is_removed() {
        for (skip_tool_confirmation, auto_approve_ask) in [(false, false), (true, true)] {
            let normalized = normalize_legacy_tool_permissions_config_value(serde_json::json!({
                "ai": {
                    "skip_tool_confirmation": skip_tool_confirmation,
                },
            }));

            assert_eq!(
                normalized["tool_permissions"],
                serde_json::json!({
                    "policy": {
                        "preset": "ask",
                        "rules": [],
                    },
                    "interaction": {
                        "auto_approve_ask": auto_approve_ask,
                    },
                })
            );
            assert!(normalized["ai"].get("skip_tool_confirmation").is_none());
        }
    }

    #[test]
    fn current_tool_permissions_win_and_legacy_skip_confirmation_is_removed() {
        let tool_permissions = serde_json::json!({
            "policy": {
                "preset": "full_access",
                "rules": [],
            },
            "interaction": {
                "auto_approve_ask": false,
            },
        });
        let normalized = normalize_legacy_tool_permissions_config_value(serde_json::json!({
            "tool_permissions": tool_permissions.clone(),
            "ai": {
                "skip_tool_confirmation": true,
            },
        }));

        assert_eq!(normalized["tool_permissions"], tool_permissions);
        assert!(normalized["ai"].get("skip_tool_confirmation").is_none());
    }

    #[test]
    fn malformed_legacy_skip_confirmation_is_removed_without_granting_access() {
        let normalized = normalize_legacy_tool_permissions_config_value(serde_json::json!({
            "ai": {
                "skip_tool_confirmation": "true",
            },
        }));

        assert!(normalized.get("tool_permissions").is_none());
        assert!(normalized["ai"].get("skip_tool_confirmation").is_none());
    }

    #[test]
    fn persistence_omits_default_memories_config() {
        let config = GlobalConfig::default();
        let value =
            config_value_for_persistence(&config).expect("config should serialize for persistence");

        assert!(value.get("memories").is_none());
        assert!(value["ai"].get("agent_models").is_none());
        assert!(value["ai"].get("allow_tool_json_repair").is_none());
        assert!(value["ai"].get("max_rounds").is_none());
        assert!(value["ai"].get("task_models").is_none());
    }

    #[test]
    fn persistence_keeps_explicit_max_rounds() {
        let mut config = GlobalConfig::default();
        config.ai.max_rounds = 37;

        let value =
            config_value_for_persistence(&config).expect("config should serialize for persistence");

        assert_eq!(value["ai"].get("max_rounds"), Some(&serde_json::json!(37)));
    }

    #[test]
    fn persistence_keeps_only_non_default_task_model_fields() {
        let mut config = GlobalConfig::default();
        config.ai.task_models.session_title =
            crate::service::config::types::TaskModelSelection::Inherit;

        let value =
            config_value_for_persistence(&config).expect("config should serialize for persistence");

        assert_eq!(
            value["ai"].get("task_models"),
            Some(&serde_json::json!({
                "session_title": { "kind": "inherit" }
            }))
        );
    }

    #[test]
    fn persistence_keeps_disabled_tool_argument_json_repair() {
        let mut config = GlobalConfig::default();
        config.ai.allow_tool_json_repair = false;

        let value =
            config_value_for_persistence(&config).expect("config should serialize for persistence");

        assert_eq!(
            value["ai"].get("allow_tool_json_repair"),
            Some(&serde_json::json!(false))
        );
    }

    #[test]
    fn persistence_keeps_only_non_default_memories_fields() {
        let mut config = GlobalConfig::default();
        config.memories.generate_memories = false;
        config.memories.generate_for_btw_sessions = true;
        config.memories.max_rollouts_per_startup = 12;

        let value =
            config_value_for_persistence(&config).expect("config should serialize for persistence");

        let memories = value
            .get("memories")
            .and_then(|value| value.as_object())
            .expect("memories config should persist as an object");
        assert!(!memories.contains_key("generate_memories"));
        assert_eq!(
            value.get("memories"),
            Some(&serde_json::json!({
                "generate_for_btw_sessions": true,
                "max_rollouts_per_startup": 12
            }))
        );
    }
}
