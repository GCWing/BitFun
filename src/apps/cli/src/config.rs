/// Configuration management module
///
/// CLI uses core's GlobalConfig system directly.
/// Only CLI-specific configuration is kept here (UI, shortcuts, etc.)
use anyhow::Result;
use bitfun_core::infrastructure::try_get_path_manager_arc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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
            theme_id: "bitfun-dark".to_string(),
            show_tips: true,
            animation: true,
            color_scheme: "default".to_string(),
        }
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            auto_save: true,
            confirm_dangerous: true,
            default_agent: "agentic".to_string(),
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
        let e2e_storage_guard = matches!(
            std::env::var("BITFUN_E2E_STORAGE_GUARD").ok().as_deref(),
            Some("1") | Some("true") | Some("TRUE")
        );
        if e2e_storage_guard {
            let path_manager =
                try_get_path_manager_arc().map_err(|error| anyhow::anyhow!(error.to_string()))?;
            return Ok(path_manager.user_root_dir().to_path_buf());
        }

        if cfg!(target_os = "windows") {
            dirs::config_dir()
                .ok_or_else(|| anyhow::anyhow!("Cannot find config directory"))
                .map(|path| path.join("bitfun"))
        } else {
            dirs::home_dir()
                .ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))
                .map(|path| path.join(".config").join("bitfun"))
        }
    }

    /// Get configuration file path
    pub(crate) fn config_path() -> Result<PathBuf> {
        Ok(Self::resolve_config_dir()?.join("config.toml"))
    }

    /// Load configuration
    pub(crate) fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            tracing::info!("Config file not found, using defaults");
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path)?;
        let mut config: Self = toml::from_str(&content)?;
        config.normalize_legacy_shortcuts();
        tracing::info!("Loaded config: {:?}", config_path);
        Ok(config)
    }

    /// Save configuration
    pub(crate) fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(&config_path, content)?;
        tracing::info!("Saved config: {:?}", config_path);
        Ok(())
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
    use super::CliConfig;

    #[test]
    fn cli_config_default_composes_owner_defaults() {
        let config = CliConfig::default();

        assert_eq!(config.ui.theme, "dark");
        assert_eq!(config.ui.theme_id, "bitfun-dark");
        assert!(config.ui.show_tips);
        assert!(config.ui.animation);
        assert_eq!(config.ui.color_scheme, "default");
        assert!(config.behavior.auto_save);
        assert!(config.behavior.confirm_dangerous);
        assert_eq!(config.behavior.default_agent, "agentic");
        assert_eq!(config.workspace.default_path, ".");
        assert_eq!(
            config.workspace.exclude_patterns,
            ["node_modules", ".git", "target", "dist"]
        );
        assert_eq!(config.shortcuts.send_message, None);
        assert_eq!(config.shortcuts.interrupt, None);
        assert_eq!(config.shortcuts.menu, None);
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
}
