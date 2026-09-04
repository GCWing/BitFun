/// Configuration management module
///
/// CLI uses core's GlobalConfig system directly.
/// Only CLI-specific configuration is kept here (UI, shortcuts, etc.)
use anyhow::Result;
use fs2::FileExt;
use openbitfun_core::infrastructure::try_get_path_manager_arc;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// CLI configuration (contains only CLI-specific config)
/// AI model configuration uses core's GlobalConfig
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub(crate) struct CliConfig {
    /// UI configuration
    pub ui: UiConfig,
    /// Behavior configuration
    pub behavior: BehaviorConfig,
    /// Workspace configuration
    pub workspace: WorkspaceConfig,
    /// Shortcuts configuration
    pub shortcuts: ShortcutsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct UiConfig {
    /// Theme (dark, light, auto)
    pub theme: String,
    /// Theme ID (built-in preset name; custom: filename in themes dir without ".json")
    pub theme_id: String,
    /// Show tips
    pub show_tips: bool,
    /// Enable animation
    pub animation: bool,
    /// Color scheme
    pub color_scheme: String,
    /// Show timestamps below user messages.
    pub timestamps: bool,
    /// Default presentation for reasoning blocks.
    pub thinking: ThinkingMode,
    /// Show tool-card details by default.
    pub tool_details: bool,
    /// Emit terminal attention notifications for completed turns and input requests.
    pub notifications: bool,
    /// Escape-sequence backend used for terminal attention notifications.
    pub notification_method: NotificationMethod,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ThinkingMode {
    Show,
    Hide,
}

impl Default for ThinkingMode {
    fn default() -> Self {
        Self::Hide
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum NotificationMethod {
    Auto,
    Osc9,
    Bel,
}

impl Default for NotificationMethod {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct BehaviorConfig {
    /// Auto save sessions
    pub auto_save: bool,
    /// Confirm dangerous operations
    pub confirm_dangerous: bool,
    /// Default Agent
    pub default_agent: String,
    /// Check the official Linux release and fallback mirror for CLI updates.
    pub auto_update: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct WorkspaceConfig {
    /// Default workspace path
    pub default_path: String,
    /// Excluded file patterns
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub(crate) struct ShortcutsConfig {
    /// Explicit legacy override for sending the current input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_message: Option<String>,
    /// Explicit legacy override for interrupting the active turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interrupt: Option<String>,
    /// Explicit legacy override for opening the command palette.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu: Option<String>,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            theme_id: "openbitfun-dark".to_string(),
            show_tips: true,
            animation: true,
            color_scheme: "default".to_string(),
            timestamps: false,
            thinking: ThinkingMode::Hide,
            tool_details: true,
            notifications: false,
            notification_method: NotificationMethod::Auto,
        }
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            auto_save: true,
            confirm_dangerous: true,
            default_agent: "agentic".to_string(),
            auto_update: true,
        }
    }
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            default_path: ".".to_string(),
            exclude_patterns: vec![
                "node_modules".to_string(),
                ".git".to_string(),
                "target".to_string(),
                "dist".to_string(),
            ],
        }
    }
}

impl CliConfig {
    fn normalize_legacy_shortcuts(&mut self) {
        // Older releases generated these values on first launch even though the
        // runtime did not dispatch through them. Only the complete generated
        // tuple is identifiable as legacy output; mixed values are user choices.
        if self.shortcuts.send_message.as_deref() == Some("Ctrl+D")
            && self.shortcuts.interrupt.as_deref() == Some("Ctrl+C")
            && self.shortcuts.menu.as_deref() == Some("Esc")
        {
            self.shortcuts = ShortcutsConfig::default();
        }
    }

    fn resolve_config_dir() -> Result<PathBuf> {
        let path_manager =
            try_get_path_manager_arc().map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(path_manager.user_root_dir().to_path_buf())
    }

    /// Get configuration file path
    pub(crate) fn config_path() -> Result<PathBuf> {
        Ok(Self::resolve_config_dir()?.join("config.toml"))
    }

    /// Load configuration
    pub(crate) fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        let config = Self::load_at(&config_path)?;
        if !config_path.exists() {
            tracing::info!("Config file not found, using defaults");
        } else {
            tracing::info!("Loaded config: {:?}", config_path);
        }
        Ok(config)
    }

    /// Save configuration
    pub(crate) fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        Self::with_config_lock(&config_path, || Self::write_at(&config_path, self))?;
        tracing::info!("Saved config: {:?}", config_path);
        Ok(())
    }

    /// Apply a focused mutation to the latest on-disk snapshot.
    ///
    /// Shared TUI clients keep independent in-memory snapshots, so live settings
    /// must merge under the config lock instead of rewriting a stale full copy.
    pub(crate) fn update<F>(&mut self, update: F) -> Result<()>
    where
        F: FnOnce(&mut Self),
    {
        let config_path = Self::config_path()?;
        let latest = Self::update_at(&config_path, update)?;
        *self = latest;
        tracing::info!("Updated config: {:?}", config_path);
        Ok(())
    }

    fn update_at<F>(config_path: &Path, update: F) -> Result<Self>
    where
        F: FnOnce(&mut Self),
    {
        Self::with_config_lock(config_path, || {
            let mut latest = Self::load_at(config_path)?;
            update(&mut latest);
            Self::write_at(config_path, &latest)?;
            Ok(latest)
        })
    }

    fn load_at(config_path: &Path) -> Result<Self> {
        if !config_path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(config_path)?;
        let mut config: Self = toml::from_str(&content)?;
        config.normalize_legacy_shortcuts();
        Ok(config)
    }

    fn write_at(config_path: &Path, config: &Self) -> Result<()> {
        let parent = config_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Config path has no parent directory"))?;
        fs::create_dir_all(parent)?;
        let content = toml::to_string_pretty(config)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(content.as_bytes())?;
        temporary.as_file_mut().sync_all()?;
        temporary
            .persist(config_path)
            .map_err(|error| error.error)?;
        Ok(())
    }

    fn with_config_lock<T>(config_path: &Path, operation: impl FnOnce() -> Result<T>) -> Result<T> {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let lock_path = config_path.with_extension("toml.lock");
        let lock_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(lock_path)?;
        lock_file.lock_exclusive()?;
        let result = operation();
        let unlock_result = FileExt::unlock(&lock_file);
        result.and_then(|value| {
            unlock_result?;
            Ok(value)
        })
    }

    /// Get configuration directory
    pub(crate) fn config_dir() -> Result<PathBuf> {
        let config_dir = Self::resolve_config_dir()?;

        fs::create_dir_all(&config_dir)?;
        Ok(config_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::{CliConfig, NotificationMethod, ThinkingMode};
    use std::fs;

    #[test]
    fn cli_config_default_composes_owner_defaults() {
        let config = CliConfig::default();
        let serialized = toml::to_string(&config).unwrap();

        assert_eq!(config.ui.theme, "dark");
        assert_eq!(config.ui.theme_id, "openbitfun-dark");
        assert!(config.ui.show_tips);
        assert!(config.ui.animation);
        assert_eq!(config.ui.color_scheme, "default");
        assert!(!config.ui.notifications);
        assert_eq!(config.ui.notification_method, NotificationMethod::Auto);
        assert!(config.behavior.auto_save);
        assert!(config.behavior.confirm_dangerous);
        assert_eq!(config.behavior.default_agent, "agentic");
        assert!(config.behavior.auto_update);
        assert_eq!(config.workspace.default_path, ".");
        assert_eq!(
            config.workspace.exclude_patterns,
            ["node_modules", ".git", "target", "dist"]
        );
        assert_eq!(config.shortcuts.send_message, None);
        assert_eq!(config.shortcuts.interrupt, None);
        assert_eq!(config.shortcuts.menu, None);
        assert!(serialized.contains("timestamps = false"), "{serialized}");
        assert!(serialized.contains("thinking = \"hide\""), "{serialized}");
        assert!(serialized.contains("tool_details = true"), "{serialized}");
        assert!(serialized.contains("notifications = false"), "{serialized}");
        assert!(
            serialized.contains("notification_method = \"auto\""),
            "{serialized}"
        );
    }

    #[test]
    fn missing_transcript_presentation_fields_keep_opencode_compatible_defaults() {
        let config: CliConfig = toml::from_str("[ui]\ntheme = \"light\"\n").unwrap();
        let serialized = toml::to_string(&config).unwrap();

        assert!(serialized.contains("timestamps = false"), "{serialized}");
        assert!(serialized.contains("thinking = \"hide\""), "{serialized}");
        assert!(serialized.contains("tool_details = true"), "{serialized}");
    }

    #[test]
    fn missing_shortcut_fields_are_not_user_choices() {
        let config: CliConfig = toml::from_str("[shortcuts]\n").unwrap();

        assert_eq!(config.shortcuts.send_message, None);
        assert_eq!(config.shortcuts.interrupt, None);
        assert_eq!(config.shortcuts.menu, None);
    }

    #[test]
    fn legacy_generated_shortcuts_are_not_treated_as_user_choices() {
        let mut config: CliConfig = toml::from_str(
            "[shortcuts]\nsend_message = \"Ctrl+D\"\ninterrupt = \"Ctrl+C\"\nmenu = \"Esc\"\n",
        )
        .unwrap();

        config.normalize_legacy_shortcuts();

        assert_eq!(config.shortcuts.send_message, None);
        assert_eq!(config.shortcuts.interrupt, None);
        assert_eq!(config.shortcuts.menu, None);
    }

    #[test]
    fn partial_legacy_shortcut_values_remain_explicit_user_choices() {
        let mut config: CliConfig = toml::from_str(
            "[shortcuts]\nsend_message = \"Ctrl+D\"\ninterrupt = \"Ctrl+X\"\nmenu = \"Esc\"\n",
        )
        .unwrap();

        config.normalize_legacy_shortcuts();

        assert_eq!(config.shortcuts.send_message.as_deref(), Some("Ctrl+D"));
        assert_eq!(config.shortcuts.interrupt.as_deref(), Some("Ctrl+X"));
        assert_eq!(config.shortcuts.menu.as_deref(), Some("Esc"));
    }

    #[test]
    fn legacy_shortcut_values_that_deviate_from_generated_defaults_are_preserved() {
        let mut config: CliConfig = toml::from_str(
            "[shortcuts]\nsend_message = \"Ctrl+S\"\ninterrupt = \"Ctrl+X\"\nmenu = \"Alt+M\"\n",
        )
        .unwrap();

        config.normalize_legacy_shortcuts();

        assert_eq!(config.shortcuts.send_message.as_deref(), Some("Ctrl+S"));
        assert_eq!(config.shortcuts.interrupt.as_deref(), Some("Ctrl+X"));
        assert_eq!(config.shortcuts.menu.as_deref(), Some("Alt+M"));
    }

    #[test]
    fn targeted_updates_merge_with_the_latest_config_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        let mut initial = CliConfig::default();
        initial.ui.theme_id = "custom-theme".to_string();
        fs::write(&path, toml::to_string_pretty(&initial).unwrap()).unwrap();

        CliConfig::update_at(&path, |latest| latest.ui.timestamps = true).unwrap();
        CliConfig::update_at(&path, |latest| latest.ui.thinking = ThinkingMode::Show).unwrap();

        let merged: CliConfig = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(merged.ui.timestamps);
        assert_eq!(merged.ui.thinking, ThinkingMode::Show);
        assert_eq!(merged.ui.theme_id, "custom-theme");
    }
}
