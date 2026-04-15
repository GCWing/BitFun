/// Configuration management module
///
/// CLI uses core's GlobalConfig system directly (same as tauri version)
/// Only CLI-specific configuration is kept here (UI, shortcuts, etc.)
use anyhow::Result;
use bitfun_core::infrastructure::APP_CONFIG_DIR_NAME;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// CLI configuration (contains only CLI-specific config)
/// AI model configuration uses core's GlobalConfig
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
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
pub struct UiConfig {
    /// Theme (dark, light, auto)
    pub theme: String,
    /// Show tips
    pub show_tips: bool,
    /// Enable animation
    pub animation: bool,
    /// Color scheme
    pub color_scheme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorConfig {
    /// Auto save sessions
    pub auto_save: bool,
    /// Confirm dangerous operations
    pub confirm_dangerous: bool,
    /// Default Agent
    pub default_agent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    /// Default workspace path
    pub default_path: String,
    /// Excluded file patterns
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutsConfig {
    /// Send message
    pub send_message: String,
    /// Interrupt
    pub interrupt: String,
    /// Menu
    pub menu: String,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            ui: UiConfig {
                theme: "dark".to_string(),
                show_tips: true,
                animation: true,
                color_scheme: "default".to_string(),
            },
            behavior: BehaviorConfig {
                auto_save: true,
                confirm_dangerous: true,
                default_agent: "agentic".to_string(),
            },
            workspace: WorkspaceConfig {
                default_path: ".".to_string(),
                exclude_patterns: vec![
                    "node_modules".to_string(),
                    ".git".to_string(),
                    "target".to_string(),
                    "dist".to_string(),
                ],
            },
            shortcuts: ShortcutsConfig {
                send_message: "Ctrl+D".to_string(),
                interrupt: "Ctrl+C".to_string(),
                menu: "Esc".to_string(),
            },
        }
    }
}

impl CliConfig {
    /// Get configuration file path
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = if cfg!(target_os = "windows") {
            dirs::config_dir()
                .ok_or_else(|| anyhow::anyhow!("Cannot find config directory"))?
                .join(APP_CONFIG_DIR_NAME)
        } else {
            dirs::home_dir()
                .ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?
                .join(".config")
                .join(APP_CONFIG_DIR_NAME)
        };

        Ok(config_dir.join("config.toml"))
    }

    /// Load configuration
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            tracing::info!("Config file not found, using defaults");
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let content = fs::read_to_string(&config_path)?;
        let config: Self = toml::from_str(&content)?;
        tracing::info!("Loaded config: {:?}", config_path);
        Ok(config)
    }

    /// Save configuration
    pub fn save(&self) -> Result<()> {
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
    pub fn config_dir() -> Result<PathBuf> {
        let config_dir = if cfg!(target_os = "windows") {
            dirs::config_dir()
                .ok_or_else(|| anyhow::anyhow!("Cannot find config directory"))?
                .join(APP_CONFIG_DIR_NAME)
        } else {
            dirs::home_dir()
                .ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?
                .join(".config")
                .join(APP_CONFIG_DIR_NAME)
        };

        fs::create_dir_all(&config_dir)?;
        Ok(config_dir)
    }

    /// Get sessions directory
    pub fn sessions_dir() -> Result<PathBuf> {
        let sessions_dir = Self::config_dir()?.join("sessions");
        fs::create_dir_all(&sessions_dir)?;
        Ok(sessions_dir)
    }
}
