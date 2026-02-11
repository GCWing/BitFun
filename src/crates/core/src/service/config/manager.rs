//! Configuration manager implementation
//!
//! A complete configuration management system based on the Provider mechanism.

use super::providers::ConfigProviderRegistry;
use super::types::*;
use crate::infrastructure::{PathManager, try_get_path_manager_arc};
use crate::util::errors::*;
use log::{debug, info, warn};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;

/// Configuration manager.
pub struct ConfigManager {
    config_dir: PathBuf,
    config: GlobalConfig,
    providers: ConfigProviderRegistry,
    config_file: PathBuf,
    path_manager: Arc<PathManager>,
}

/// Configuration manager settings.
#[derive(Debug, Clone)]
pub struct ConfigManagerSettings {
    pub path_manager: Option<Arc<PathManager>>,
    pub auto_save: bool,
    pub backup_count: usize,
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

        let mut manager = Self {
            config_dir,
            config: GlobalConfig::default(),
            providers,
            config_file,
            path_manager,
        };

        manager.load_or_create_config().await?;

        debug!("ConfigManager initialized at {:?}", manager.config_file);
        Ok(manager)
    }

    /// Returns the path manager.
    pub fn path_manager(&self) -> &Arc<PathManager> {
        &self.path_manager
    }

    /// Loads or creates the configuration file.
    async fn load_or_create_config(&mut self) -> BitFunResult<()> {
        if self.config_file.exists() {
            self.load_and_migrate_config().await?;
        } else {
            self.config = self.providers.get_default_config();
            Self::add_default_agent_models_config(&mut self.config.ai.agent_models);
            Self::add_default_func_agent_models_config(&mut self.config.ai.func_agent_models);
            self.config.version = env!("CARGO_PKG_VERSION").to_string();
            self.save_config().await?;
            debug!("Created default config file");
        }

        Ok(())
    }

    /// Loads and migrates configuration.
    async fn load_and_migrate_config(&mut self) -> BitFunResult<()> {
        let content = fs::read_to_string(&self.config_file)
            .await
            .map_err(|e| BitFunError::config(format!("Failed to read config file: {}", e)))?;

        let mut config_value: Value = serde_json::from_str(&content).map_err(|e| {
            BitFunError::config(format!("Failed to parse config file as JSON: {}", e))
        })?;

        let file_version = config_value
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0")
            .to_string();

        let current_version = env!("CARGO_PKG_VERSION").to_string();

        let needs_migration = !versions_match(&file_version, &current_version);
        if needs_migration {
            info!(
                "Config version change detected: {} -> {}",
                file_version, current_version
            );
            config_value = self
                .migrate_config_version(&file_version, config_value)
                .await?;

            if let Some(obj) = config_value.as_object_mut() {
                obj.insert(
                    "version".to_string(),
                    Value::String(current_version.clone()),
                );
            }
        }

        match serde_json::from_value::<GlobalConfig>(config_value.clone()) {
            Ok(mut config) => {
                Self::ensure_models_config(&mut config.ai.models);
                Self::add_default_agent_models_config(&mut config.ai.agent_models);
                Self::add_default_func_agent_models_config(&mut config.ai.func_agent_models);

                self.config = config;

                if needs_migration {
                    self.config.version = current_version;
                    self.save_config().await?;
                    info!("Config migrated and saved");
                } else {
                    debug!("Loaded config from file");
                }

                Ok(())
            }
            Err(e) => {
                warn!(
                    "Config file deserialization failed, starting smart merge: {}",
                    e
                );

                self.smart_merge_config_from_value(config_value).await
            }
        }
    }

    /// Performs a smart merge from a JSON value.
    async fn smart_merge_config_from_value(&mut self, user_value: Value) -> BitFunResult<()> {
        let base_config = self.providers.get_default_config();

        let base_value = serde_json::to_value(&base_config).map_err(|e| {
            BitFunError::config(format!("Failed to serialize default config: {}", e))
        })?;
        let merged_value = deep_merge(base_value, user_value);

        let mut config: GlobalConfig = serde_json::from_value(merged_value).map_err(|e| {
            BitFunError::config(format!("Failed to deserialize merged config: {}", e))
        })?;

        Self::ensure_models_config(&mut config.ai.models);
        Self::add_default_agent_models_config(&mut config.ai.agent_models);
        Self::add_default_func_agent_models_config(&mut config.ai.func_agent_models);

        self.config = config;

        self.config.version = env!("CARGO_PKG_VERSION").to_string();
        self.save_config().await?;
        info!("Config automatically fixed and saved");

        Ok(())
    }

    /// Auto-completes missing fields in model configuration (backward compatible).
    /// Ensures older configurations won't panic.
    fn ensure_models_config(models: &mut Vec<AIModelConfig>) {
        for model in models.iter_mut() {
            model.ensure_category_and_capabilities();
        }
        debug!(
            "Auto-completed category and capabilities for {} models",
            models.len()
        );
    }

    /// Adds default configuration for the primary agents (`agent_models`).
    fn add_default_agent_models_config(
        agent_models: &mut std::collections::HashMap<String, String>,
    ) {
        let agents_using_fast = vec![
            "Explore",
            "FileFinder",
            "GenerateDoc",
            "CodeReview",
        ];
        for key in agents_using_fast {
            if !agent_models.contains_key(key) {
                agent_models.insert(key.to_string(), "fast".to_string());
            }
        }
    }

    /// Adds default configuration for functional agents (`func_agent_models`).
    fn add_default_func_agent_models_config(
        func_agent_models: &mut std::collections::HashMap<String, String>,
    ) {
        let func_agents_using_fast = vec!["compression", "startchat-func-agent", "git-func-agent"];
        for key in func_agents_using_fast {
            if !func_agent_models.contains_key(key) {
                func_agent_models.insert(key.to_string(), "fast".to_string());
            }
        }
    }

    /// Migrates configuration versions.
    async fn migrate_config_version(
        &self,
        from_version: &str,
        mut config: Value,
    ) -> BitFunResult<Value> {
        let migrations: Vec<(&str, &str, fn(Value) -> BitFunResult<Value>)> =
            vec![("0.0.0", "1.0.0", migrate_0_0_0_to_1_0_0)];

        let mut current_version = from_version.to_string();

        for (from, to, migrate_fn) in migrations {
            if version_gte(&current_version, from) && version_lt(&current_version, to) {
                debug!("Executing migration: {} -> {}", from, to);
                config = migrate_fn(config)?;
                current_version = to.to_string();
            }
        }

        Ok(config)
    }

    /// Saves the configuration file.
    async fn save_config(&self) -> BitFunResult<()> {
        let content = serde_json::to_string_pretty(&self.config)
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

        fs::write(&self.config_file, content).await.map_err(|e| {
            BitFunError::config(format!(
                "Failed to write config file {:?}: {}",
                self.config_file, e
            ))
        })?;
        Ok(())
    }

    /// Gets a configuration value (supports dot-paths).
    pub fn get<T>(&self, path: &str) -> BitFunResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
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
        let old_config = self.config.clone();
        let json_value = serde_json::to_value(value)
            .map_err(|e| BitFunError::config(format!("Failed to serialize config value: {}", e)))?;

        self.set_value_by_path(path, json_value)?;
        self.config.last_modified = chrono::Utc::now();

        if let Err(e) = self.validate_config().await {
            self.config = old_config;
            return Err(e);
        }

        self.notify_config_changed(path, &old_config).await?;

        self.save_config().await?;

        Ok(())
    }

    /// Resets configuration (supports dot-paths).
    pub async fn reset(&mut self, path: Option<&str>) -> BitFunResult<()> {
        let old_config = self.config.clone();

        if let Some(path) = path {
            let default_config = self.providers.get_default_config();
            let default_value = self.get_value_by_path_from_config(&default_config, path)?;
            self.set_value_by_path(path, default_value)?;
        } else {
            self.config = self.providers.get_default_config();
        }

        self.config.last_modified = chrono::Utc::now();

        if let Some(path) = path {
            self.notify_config_changed(path, &old_config).await?;
        } else {
            for provider_name in self.providers.get_provider_names() {
                self.notify_config_changed(&provider_name, &old_config)
                    .await?;
            }
        }

        self.save_config().await?;

        Ok(())
    }

    /// Returns the full configuration.
    pub fn get_config(&self) -> &GlobalConfig {
        &self.config
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
        let old_config = self.config.clone();

        let imported_config: GlobalConfig = serde_json::from_value(config_data)
            .map_err(|e| BitFunError::config(format!("Failed to parse imported config: {}", e)))?;

        let validation_result = self.providers.validate_config(&imported_config).await?;
        if !validation_result.valid {
            let error_messages: Vec<String> = validation_result
                .errors
                .iter()
                .map(|e| e.message.clone())
                .collect();
            return Err(BitFunError::validation(format!(
                "Invalid imported config: {}",
                error_messages.join(", ")
            )));
        }

        self.config = imported_config;
        self.config.last_modified = chrono::Utc::now();

        for provider_name in self.providers.get_provider_names() {
            self.notify_config_changed(&provider_name, &old_config)
                .await?;
        }

        self.save_config().await?;

        info!("Successfully imported configuration");
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

        let backup_file = backup_dir.join(format!("config_backup_{}.json", timestamp));

        let content = serde_json::to_string_pretty(&self.config)
            .map_err(|e| BitFunError::config(format!("Failed to serialize backup: {}", e)))?;

        fs::write(&backup_file, content)
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

        let keys: Vec<&str> = path.split('.').collect();
        let mut current = &config_value;

        for key in keys {
            current = current
                .get(key)
                .ok_or_else(|| BitFunError::config(format!("Config path '{}' not found", path)))?;
        }

        Ok(current.clone())
    }

    /// Sets a configuration value by dot-path.
    fn set_value_by_path(&mut self, path: &str, value: serde_json::Value) -> BitFunResult<()> {
        if path.is_empty() {
            self.config = serde_json::from_value(value)
                .map_err(|e| BitFunError::config(format!("Failed to deserialize config: {}", e)))?;
            return Ok(());
        }

        let mut config_value = serde_json::to_value(&self.config)
            .map_err(|e| BitFunError::config(format!("Failed to serialize config: {}", e)))?;

        let keys: Vec<&str> = path.split('.').filter(|k| !k.is_empty()).collect();
        if keys.is_empty() {
            self.config = serde_json::from_value(value)
                .map_err(|e| BitFunError::config(format!("Failed to deserialize config: {}", e)))?;
            return Ok(());
        }

        let last_key = keys.last().unwrap();
        let parent_keys = &keys[..keys.len() - 1];

        let mut current = &mut config_value;
        for key in parent_keys {
            current = current
                .get_mut(key)
                .ok_or_else(|| BitFunError::config(format!("Config path '{}' not found", path)))?;
        }

        if let Some(obj) = current.as_object_mut() {
            obj.insert(last_key.to_string(), value);
        } else {
            return Err(BitFunError::config(format!(
                "Cannot set value at path '{}': parent is not an object",
                path
            )));
        }

        self.config = serde_json::from_value(config_value).map_err(|e| {
            BitFunError::config(format!("Failed to deserialize updated config: {}", e))
        })?;

        Ok(())
    }

    /// Notifies about a configuration change.
    async fn notify_config_changed(
        &self,
        path: &str,
        old_config: &GlobalConfig,
    ) -> BitFunResult<()> {
        self.check_and_broadcast_debug_mode_change(old_config).await;

        self.providers
            .notify_config_changed(path, old_config, &self.config)
            .await
    }

    /// Detects and broadcasts debug-mode configuration changes.
    async fn check_and_broadcast_debug_mode_change(&self, old_config: &GlobalConfig) {
        let old_debug = &old_config.ai.debug_mode_config;
        let new_debug = &self.config.ai.debug_mode_config;

        if old_debug.ingest_port != new_debug.ingest_port
            || old_debug.log_path != new_debug.log_path
        {
            debug!(
                "Debug Mode config change detected: port {} -> {}, log_path {} -> {}",
                old_debug.ingest_port,
                new_debug.ingest_port,
                old_debug.log_path,
                new_debug.log_path
            );

            use super::global::{ConfigUpdateEvent, GlobalConfigManager};
            GlobalConfigManager::broadcast_update(ConfigUpdateEvent::DebugModeConfigUpdated {
                new_port: new_debug.ingest_port,
                new_log_path: new_debug.log_path.clone(),
            })
            .await;
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

/// Returns whether `v1 >= v2`.
pub(crate) fn version_gte(v1: &str, v2: &str) -> bool {
    parse_version(v1) >= parse_version(v2)
}

/// Returns whether `v1 < v2`.
pub(crate) fn version_lt(v1: &str, v2: &str) -> bool {
    parse_version(v1) < parse_version(v2)
}

/// Parses a version string into a tuple `(major, minor, patch)`.
pub(crate) fn parse_version(version: &str) -> (u32, u32, u32) {
    let parts: Vec<&str> = version.split('.').collect();
    let major = parts.get(0).and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

/// Migration function: `0.0.0 -> 1.0.0`.
///
/// This migration is an example showing how to handle configuration upgrades.
pub(crate) fn migrate_0_0_0_to_1_0_0(mut config: Value) -> BitFunResult<Value> {
    debug!("Executing config migration: 0.0.0 -> 1.0.0");

    if let Some(app) = config.get_mut("app").and_then(|v| v.as_object_mut()) {
        if !app.contains_key("ai_experience") {
            app.insert(
                "ai_experience".to_string(),
                serde_json::json!({
                    "enable_session_title_generation": true,
                    "enable_welcome_panel_ai_analysis": false
                }),
            );
        }
    }

    if let Some(ai) = config.get_mut("ai").and_then(|v| v.as_object_mut()) {
        if !ai.contains_key("super_agent_models") {
            ai.insert(
                "super_agent_models".to_string(),
                Value::Object(serde_json::Map::new()),
            );
        }
        if !ai.contains_key("sub_agent_models") {
            ai.insert("sub_agent_models".to_string(), serde_json::json!({}));
        }
        if !ai.contains_key("func_agent_models") {
            let func_keys = ["compression", "startchat-func-agent", "git-func-agent"];
            let mut fa = serde_json::Map::new();
            if let Some(am) = ai.get("agent_models").and_then(|v| v.as_object()) {
                for k in func_keys {
                    if let Some(v) = am.get(k) {
                        fa.insert(k.to_string(), v.clone());
                    }
                }
            }
            ai.insert("func_agent_models".to_string(), Value::Object(fa));
        }
    }

    debug!("Migration 0.0.0 -> 1.0.0 completed");
    Ok(config)
}
