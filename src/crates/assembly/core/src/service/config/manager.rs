//! Configuration manager implementation
//!
//! A complete configuration management system based on the Provider mechanism.

use super::normalization::{normalize_typed_config, reconcile_model_references};
use super::providers::ConfigProviderRegistry;
use super::types::*;
use crate::infrastructure::{try_get_path_manager_arc, PathManager};
use crate::util::errors::*;
use log::{debug, info, warn};
use openbitfun_core_types::product_identity;
use openbitfun_services_core::json_store::JsonFileStore;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;

fn invalid_config_error(context: &str, result: &ConfigValidationResult) -> OpenBitFunError {
    let messages = result
        .errors
        .iter()
        .map(|error| format!("{}: {}", error.path, error.message))
        .collect::<Vec<_>>()
        .join(", ");
    OpenBitFunError::validation(format!("{context}: {messages}"))
}

const MIN_OPENBITFUN_CONFIG_VERSION: (u64, u64, u64) = (1, 0, 0);

fn parse_semver_floor(version: &str) -> Option<((u64, u64, u64), bool)> {
    let without_build = version.split_once('+').map_or(version, |(value, _)| value);
    let (core, has_prerelease) = without_build
        .split_once('-')
        .map_or((without_build, false), |(value, _)| (value, true));
    let mut parts = core.split('.');
    let parsed = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    if parts.next().is_some() {
        return None;
    }
    Some((parsed, has_prerelease))
}

pub(crate) fn validate_openbitfun_product_version(
    persisted_product_id: &str,
    version: &str,
    context: &str,
) -> OpenBitFunResult<()> {
    let expected_product_id = product_identity::product_id();
    if persisted_product_id != expected_product_id {
        return Err(OpenBitFunError::validation(format!(
            "{context} product_id must be '{expected_product_id}', found '{persisted_product_id}'"
        )));
    }

    let Some((parsed, has_prerelease)) = parse_semver_floor(version) else {
        return Err(OpenBitFunError::validation(format!(
            "{context} version '{version}' is not a valid OpenBitFun version"
        )));
    };
    if parsed < MIN_OPENBITFUN_CONFIG_VERSION
        || (parsed == MIN_OPENBITFUN_CONFIG_VERSION && has_prerelease)
    {
        return Err(OpenBitFunError::validation(format!(
            "{context} version '{version}' predates OpenBitFun 1.0.0"
        )));
    }
    Ok(())
}

pub(crate) fn validate_current_config_value(value: &Value, context: &str) -> OpenBitFunResult<()> {
    let root = value.as_object().ok_or_else(|| {
        OpenBitFunError::validation(format!("{context} must be a JSON object"))
    })?;
    let product_id = root
        .get("product_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            OpenBitFunError::validation(format!(
                "{context} is missing required string field 'product_id'"
            ))
        })?;
    let schema_version = root
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            OpenBitFunError::validation(format!(
                "{context} is missing required integer field 'schema_version'"
            ))
        })?;
    if schema_version != u64::from(CURRENT_CONFIG_SCHEMA_VERSION) {
        return Err(OpenBitFunError::validation(format!(
            "{context} schema_version must be {CURRENT_CONFIG_SCHEMA_VERSION}, found {schema_version}"
        )));
    }
    let version = root
        .get("version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            OpenBitFunError::validation(format!(
                "{context} is missing required string field 'version'"
            ))
        })?;
    if !root.contains_key("last_modified") {
        return Err(OpenBitFunError::validation(format!(
            "{context} is missing required field 'last_modified'"
        )));
    }
    validate_openbitfun_product_version(product_id, version, context)?;
    reject_retired_config_fields(root, context)
}

fn retired_config_field(context: &str, path: &str) -> OpenBitFunError {
    OpenBitFunError::validation(format!(
        "{context} contains retired pre-OpenBitFun field '{path}'; use the explicit data migration tool instead"
    ))
}

fn reject_retired_config_fields(
    root: &serde_json::Map<String, Value>,
    context: &str,
) -> OpenBitFunResult<()> {
    if let Some(app) = root.get("app").and_then(Value::as_object) {
        for field in ["session", "session_config"] {
            if app.contains_key(field) {
                return Err(retired_config_field(context, &format!("app.{field}")));
            }
        }
        if app
            .get("ai_experience")
            .and_then(Value::as_object)
            .is_some_and(|ai_experience| {
                ai_experience.contains_key("agent_companion_display_mode")
            })
        {
            return Err(retired_config_field(
                context,
                "app.ai_experience.agent_companion_display_mode",
            ));
        }
    }
    if root
        .get("font")
        .and_then(Value::as_object)
        .is_some_and(|font| font.contains_key("flowChat"))
    {
        return Err(retired_config_field(context, "font.flowChat"));
    }

    let Some(ai) = root.get("ai").and_then(Value::as_object) else {
        return Ok(());
    };
    for field in ["agent_models", "skip_tool_confirmation"] {
        if ai.contains_key(field) {
            return Err(retired_config_field(context, &format!("ai.{field}")));
        }
    }
    if ai
        .get("review_teams")
        .and_then(Value::as_object)
        .is_some_and(|review_teams| review_teams.contains_key("rate_limit_status"))
    {
        return Err(retired_config_field(
            context,
            "ai.review_teams.rate_limit_status",
        ));
    }

    if let Some(models) = ai.get("models").and_then(Value::as_array) {
        for (index, model) in models.iter().enumerate() {
            let Some(model) = model.as_object() else {
                continue;
            };
            for field in [
                "enable_thinking_process",
                "reasoning_mode",
                "reasoning_effort",
                "thinking_budget_tokens",
            ] {
                if model.contains_key(field) {
                    return Err(retired_config_field(
                        context,
                        &format!("ai.models[{index}].{field}"),
                    ));
                }
            }
        }
    }

    Ok(())
}

fn reject_protected_metadata_path(path: &str) -> OpenBitFunResult<()> {
    if matches!(
        path,
        "product_id" | "schema_version" | "version" | "last_modified"
    ) {
        return Err(OpenBitFunError::validation(format!(
            "Configuration metadata '{path}' is managed by OpenBitFun and cannot be changed directly"
        )));
    }
    Ok(())
}

fn config_value_for_persistence(config: &GlobalConfig) -> OpenBitFunResult<Value> {
    let mut value = serde_json::to_value(config)
        .map_err(|e| OpenBitFunError::config(format!("Failed to serialize config: {}", e)))?;
    prune_default_ai_tool_argument_json_repair(&mut value);
    prune_default_ai_max_rounds(&mut value);
    prune_default_memories_config(&mut value)?;
    prune_default_web_search_config(&mut value)?;
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

fn prune_default_memories_config(config_value: &mut Value) -> OpenBitFunResult<()> {
    let Some(config_object) = config_value.as_object_mut() else {
        return Ok(());
    };
    let Some(memories_value) = config_object.get_mut("memories") else {
        return Ok(());
    };

    let default_memories = serde_json::to_value(MemoriesConfig::default()).map_err(|e| {
        OpenBitFunError::config(format!(
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

fn prune_default_web_search_config(config_value: &mut Value) -> OpenBitFunResult<()> {
    let Some(ai_config) = config_value.get_mut("ai").and_then(Value::as_object_mut) else {
        return Ok(());
    };
    let Some(web_search) = ai_config
        .get_mut("web_search")
        .and_then(Value::as_object_mut)
    else {
        return Ok(());
    };

    let defaults = serde_json::to_value(WebSearchConfig::default()).map_err(|e| {
        OpenBitFunError::config(format!(
            "Failed to serialize default WebSearch config: {}",
            e
        ))
    })?;
    let defaults = defaults
        .as_object()
        .expect("WebSearchConfig serializes as an object");
    prune_matching_default_fields(web_search, defaults);

    if web_search.is_empty() {
        ai_config.remove("web_search");
    }
    Ok(())
}

fn prune_matching_default_fields(
    current: &mut serde_json::Map<String, Value>,
    defaults: &serde_json::Map<String, Value>,
) {
    for (key, default_value) in defaults {
        let should_remove = match current.get_mut(key) {
            Some(current_value) if current_value == default_value => true,
            Some(Value::Object(current_object)) => {
                let Value::Object(default_object) = default_value else {
                    continue;
                };
                prune_matching_default_fields(current_object, default_object);
                current_object.is_empty()
            }
            _ => false,
        };
        if should_remove {
            current.remove(key);
        }
    }
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
    pub async fn new(settings: ConfigManagerSettings) -> OpenBitFunResult<Self> {
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
            openbitfun_ai_adapters::diagnostics::set_include_sensitive_diagnostics(
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
    pub(crate) async fn reload(&mut self) -> OpenBitFunResult<()> {
        let settings = ConfigManagerSettings {
            path_manager: Some(self.path_manager.clone()),
            auto_save: true,
            backup_count: self.backup_count,
        };
        *self = Self::new(settings).await?;
        Ok(())
    }

    /// Loads or creates the configuration file.
    async fn load_or_create_config(&mut self) -> OpenBitFunResult<()> {
        if self.config_file.exists() {
            self.load_existing_config().await?;
        } else {
            self.create_default_config().await?;
        }

        Ok(())
    }

    /// Creates the first config file using the already initialized defaults.
    async fn create_default_config(&mut self) -> OpenBitFunResult<()> {
        self.config.product_id = product_identity::product_id().to_string();
        self.config.schema_version = CURRENT_CONFIG_SCHEMA_VERSION;
        self.config.version = env!("CARGO_PKG_VERSION").to_string();
        self.config.last_modified = chrono::Utc::now();
        self.save_config().await?;
        debug!("Created default config file");
        Ok(())
    }

    /// Loads an existing OpenBitFun config without rewriting or repairing it.
    async fn load_existing_config(&mut self) -> OpenBitFunResult<()> {
        let content = fs::read_to_string(&self.config_file)
            .await
            .map_err(|e| OpenBitFunError::config(format!("Failed to read config file: {}", e)))?;

        let config_value: Value = serde_json::from_str(&content).map_err(|error| {
            OpenBitFunError::config(format!("Failed to parse config file as JSON: {error}"))
        })?;
        validate_current_config_value(&config_value, "Configuration file")?;

        let config: GlobalConfig = serde_json::from_value(config_value).map_err(|error| {
            OpenBitFunError::config(format!("Failed to deserialize config file: {error}"))
        })?;
        let validation_result = self.providers.validate_config(&config).await?;
        if !validation_result.valid {
            return Err(invalid_config_error(
                "Invalid configuration file",
                &validation_result,
            ));
        }

        self.config = config;
        self.load_diagnostics.clear();
        debug!("Loaded OpenBitFun config from file without rewriting it");
        Ok(())
    }

    /// Saves the configuration file.
    async fn save_config(&self) -> OpenBitFunResult<()> {
        self.persist_config(&self.config).await
    }

    async fn persist_config(&self, config: &GlobalConfig) -> OpenBitFunResult<()> {
        let content = serde_json::to_string_pretty(&config_value_for_persistence(config)?)
            .map_err(|e| OpenBitFunError::config(format!("Config serialization failed: {}", e)))?;

        if let Some(parent) = self.config_file.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await.map_err(|e| {
                    OpenBitFunError::config(format!(
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
                OpenBitFunError::config(format!(
                    "Failed to atomically write config file {:?}: {}",
                    self.config_file, e
                ))
            })?;
        Ok(())
    }

    async fn backup_raw_config(&self, content: &str, reason: &str) -> OpenBitFunResult<PathBuf> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S_%3f");
        let backup_dir = self.config_dir.join("backups");
        fs::create_dir_all(&backup_dir).await.map_err(|e| {
            OpenBitFunError::config(format!("Failed to create backup directory: {e}"))
        })?;
        let backup_file = backup_dir.join(format!(
            "app_{reason}_{timestamp}_{}.json",
            uuid::Uuid::new_v4().simple()
        ));
        fs::write(&backup_file, content)
            .await
            .map_err(|e| OpenBitFunError::config(format!("Failed to write config backup: {e}")))?;
        self.prune_backups(&backup_dir).await?;
        info!(
            "Created config backup: reason={}, path={}",
            reason,
            backup_file.display()
        );
        Ok(backup_file)
    }

    async fn prune_backups(&self, backup_dir: &std::path::Path) -> OpenBitFunResult<()> {
        if self.backup_count == 0 {
            return Ok(());
        }
        let mut entries = fs::read_dir(backup_dir).await.map_err(|e| {
            OpenBitFunError::config(format!("Failed to read backup directory: {e}"))
        })?;
        let mut files = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| OpenBitFunError::config(format!("Failed to enumerate backups: {e}")))?
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
                .map_err(|e| OpenBitFunError::config(format!("Failed to inspect backup: {e}")))?;
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
    pub fn get<T>(&self, path: &str) -> OpenBitFunResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let value = self.get_value_by_path(path)?;
        serde_json::from_value(value).map_err(|e| {
            OpenBitFunError::config(format!(
                "Failed to deserialize config value at '{}': {}",
                path, e
            ))
        })
    }

    /// Sets a configuration value (supports dot-paths).
    pub async fn set<T>(&mut self, path: &str, value: T) -> OpenBitFunResult<()>
    where
        T: serde::Serialize,
    {
        let json_value = serde_json::to_value(value).map_err(|e| {
            OpenBitFunError::config(format!("Failed to serialize config value: {}", e))
        })?;

        reject_protected_metadata_path(path)?;
        let mut config = self.config_with_value(path, json_value)?;
        // Apply capability-driven canonicalization before validation and persistence.
        // Speech/embedding/image-only models must never carry text-generation sentinels.
        normalize_typed_config(&mut config);
        if path.is_empty()
            || path == "ai"
            || path.starts_with("ai.models")
            || path.starts_with("ai.default_models")
            || path.starts_with("ai.agent_model_defaults")
            || path.starts_with("ai.task_models")
        {
            reconcile_model_references(&mut config);
        }

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
    pub async fn reset(&mut self, path: Option<&str>) -> OpenBitFunResult<()> {
        let config = if let Some(path) = path {
            reject_protected_metadata_path(path)?;
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

        self.commit_config(config, path).await
    }

    /// Publish only a configuration that has reached disk. Failed writes must
    /// not change reads, runtime subscribers, or a subsequent unrelated save.
    async fn commit_config(
        &mut self,
        mut config: GlobalConfig,
        path: Option<&str>,
    ) -> OpenBitFunResult<()> {
        config.product_id = product_identity::product_id().to_string();
        config.schema_version = CURRENT_CONFIG_SCHEMA_VERSION;
        config.version = env!("CARGO_PKG_VERSION").to_string();
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
    pub async fn validate_config(&self) -> OpenBitFunResult<ConfigValidationResult> {
        self.providers.validate_config(&self.config).await
    }

    /// Exports configuration.
    pub fn export_config(&self) -> OpenBitFunResult<serde_json::Value> {
        serde_json::to_value(&self.config)
            .map_err(|e| OpenBitFunError::config(format!("Failed to export config: {}", e)))
    }

    /// Imports configuration.
    pub async fn import_config(&mut self, config_data: serde_json::Value) -> OpenBitFunResult<()> {
        validate_current_config_value(&config_data, "Imported configuration")?;
        let imported_config: GlobalConfig = serde_json::from_value(config_data).map_err(|e| {
            OpenBitFunError::config(format!("Failed to parse imported config: {}", e))
        })?;

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
            OpenBitFunError::config(format!("Failed to read config before import backup: {e}"))
        })?;
        self.backup_raw_config(&previous_content, "pre-import")
            .await?;

        self.commit_config(imported_config, None).await?;
        self.load_diagnostics.clear();

        info!("Successfully imported configuration");
        Ok(())
    }

    /// Creates a configuration backup.
    pub async fn create_backup(&self) -> OpenBitFunResult<PathBuf> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_dir = self.config_dir.join("backups");

        if !backup_dir.exists() {
            fs::create_dir_all(&backup_dir).await.map_err(|e| {
                OpenBitFunError::config(format!("Failed to create backup directory: {}", e))
            })?;
        }

        let backup_file = backup_dir.join(format!(
            "config_backup_{}_{}.json",
            timestamp,
            uuid::Uuid::new_v4().simple()
        ));

        let content = serde_json::to_string_pretty(&config_value_for_persistence(&self.config)?)
            .map_err(|e| OpenBitFunError::config(format!("Failed to serialize backup: {}", e)))?;

        JsonFileStore
            .write_text_atomic_create_new(&backup_file, &content)
            .await
            .map_err(|e| OpenBitFunError::config(format!("Failed to write backup: {}", e)))?;

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
    fn get_value_by_path(&self, path: &str) -> OpenBitFunResult<serde_json::Value> {
        self.get_value_by_path_from_config(&self.config, path)
    }

    /// Gets a configuration value by dot-path from the given config.
    fn get_value_by_path_from_config(
        &self,
        config: &GlobalConfig,
        path: &str,
    ) -> OpenBitFunResult<serde_json::Value> {
        let config_value = serde_json::to_value(config)
            .map_err(|e| OpenBitFunError::config(format!("Failed to serialize config: {}", e)))?;

        if path.is_empty() {
            return Ok(config_value);
        }

        let keys: Vec<&str> = path.split('.').collect();
        let mut current = &config_value;

        for key in keys {
            current = current.get(key).ok_or_else(|| {
                OpenBitFunError::NotFound(format!("Config path '{}' not found", path))
            })?;
        }

        Ok(current.clone())
    }

    /// Builds a candidate configuration without changing the committed value.
    fn config_with_value(
        &self,
        path: &str,
        value: serde_json::Value,
    ) -> OpenBitFunResult<GlobalConfig> {
        if path.is_empty() {
            return serde_json::from_value(value).map_err(|e| {
                OpenBitFunError::config(format!("Failed to deserialize config: {}", e))
            });
        }

        let mut config_value = serde_json::to_value(&self.config)
            .map_err(|e| OpenBitFunError::config(format!("Failed to serialize config: {}", e)))?;

        let keys: Vec<&str> = path.split('.').filter(|k| !k.is_empty()).collect();
        if keys.is_empty() {
            return serde_json::from_value(value).map_err(|e| {
                OpenBitFunError::config(format!("Failed to deserialize config: {}", e))
            });
        }

        let last_key = keys.last().ok_or_else(|| {
            OpenBitFunError::config(format!("Config path '{}' does not contain any keys", path))
        })?;
        let parent_keys = &keys[..keys.len() - 1];

        let mut current = &mut config_value;
        for key in parent_keys {
            current = current.get_mut(key).ok_or_else(|| {
                OpenBitFunError::NotFound(format!("Config path '{}' not found", path))
            })?;
        }

        if let Some(obj) = current.as_object_mut() {
            obj.insert(last_key.to_string(), value);
        } else {
            return Err(OpenBitFunError::config(format!(
                "Cannot set value at path '{}': parent is not an object",
                path
            )));
        }

        serde_json::from_value(config_value).map_err(|e| {
            OpenBitFunError::config(format!("Failed to deserialize updated config: {}", e))
        })
    }

    /// Notifies about a configuration change.
    async fn notify_config_changed(
        &self,
        path: &str,
        old_config: &GlobalConfig,
    ) -> OpenBitFunResult<()> {
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
                openbitfun_ai_adapters::diagnostics::set_include_sensitive_diagnostics(new_include);
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

/// Configuration statistics.
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigStatistics {
    pub total_ai_models: usize,
    pub has_default_model: bool,
    pub config_directory: PathBuf,
    pub providers_count: usize,
    pub last_modified: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::{config_value_for_persistence, validate_current_config_value};
    use crate::service::config::types::GlobalConfig;

    #[test]
    fn current_config_contract_requires_openbitfun_identity_and_format() {
        let current = serde_json::to_value(GlobalConfig::default()).unwrap();
        validate_current_config_value(&current, "test config").unwrap();

        for (field, value, expected) in [
            ("product_id", serde_json::json!("other-product"), "product_id"),
            ("schema_version", serde_json::json!(0), "schema_version"),
            ("version", serde_json::json!("0.9.9"), "predates OpenBitFun 1.0.0"),
        ] {
            let mut invalid = current.clone();
            invalid[field] = value;
            let error = validate_current_config_value(&invalid, "test config").unwrap_err();
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn current_config_contract_rejects_missing_identity_metadata() {
        let current = serde_json::to_value(GlobalConfig::default()).unwrap();
        for field in ["product_id", "schema_version", "version", "last_modified"] {
            let mut invalid = current.clone();
            invalid.as_object_mut().unwrap().remove(field);
            let error = validate_current_config_value(&invalid, "test config").unwrap_err();
            assert!(error.to_string().contains(field), "{error}");
        }
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

    #[test]
    fn persistence_omits_default_web_search_config_and_restores_defaults() {
        let config = GlobalConfig::default();
        let value =
            config_value_for_persistence(&config).expect("config should serialize for persistence");

        assert!(value["ai"].get("web_search").is_none());

        let restored: GlobalConfig =
            serde_json::from_value(value).expect("default-sparse config should deserialize");
        assert_eq!(restored.ai.web_search, config.ai.web_search);
    }

    #[test]
    fn persistence_keeps_only_non_default_web_search_fields_and_unknown_extensions() {
        let mut config = GlobalConfig::default();
        let web_search = &mut config.ai.web_search;
        web_search.provider = "openbitfun_search_http".to_string();
        web_search
            .unknown
            .insert("selectionRevision".to_string(), serde_json::json!(7));
        web_search.providers.unknown.insert(
            "future_search".to_string(),
            serde_json::json!({ "endpoint": "https://future.example/search" }),
        );
        let http = &mut web_search.providers.openbitfun_search_http;
        http.endpoint = "https://search.example.com/search".to_string();
        http.auth.mode = "header".to_string();
        http.auth.header_name = "X-Search-Key".to_string();
        http.unknown
            .insert("futureHttpOption".to_string(), serde_json::json!(true));

        let value =
            config_value_for_persistence(&config).expect("config should serialize for persistence");

        assert_eq!(
            value["ai"].get("web_search"),
            Some(&serde_json::json!({
                "provider": "openbitfun_search_http",
                "providers": {
                    "openbitfun_search_http": {
                        "endpoint": "https://search.example.com/search",
                        "auth": {
                            "mode": "header",
                            "headerName": "X-Search-Key"
                        },
                        "futureHttpOption": true
                    },
                    "future_search": {
                        "endpoint": "https://future.example/search"
                    }
                },
                "selectionRevision": 7
            }))
        );

        let restored: GlobalConfig =
            serde_json::from_value(value).expect("sparse config should deserialize");
        assert_eq!(restored.ai.web_search, config.ai.web_search);
    }
}
