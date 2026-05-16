/// Configuration management module
/// 
/// CLI uses core's GlobalConfig system directly.
/// Only CLI-specific configuration is kept here (UI, shortcuts, etc.)

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;

/// CLI configuration (contains only CLI-specific config)
/// AI model configuration uses core's GlobalConfig
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
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
#[serde(default)]
pub struct UiConfig {
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
pub struct BehaviorConfig {
    /// Auto save sessions
    pub auto_save: bool,
    /// Confirm dangerous operations
    pub confirm_dangerous: bool,
    /// Default Agent
    pub default_agent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceConfig {
    /// Default workspace path
    pub default_path: String,
    /// Excluded file patterns
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ShortcutsConfig {
    /// Send message
    pub send_message: String,
    /// Interrupt
    pub interrupt: String,
    /// Menu
    pub menu: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            theme_id: "cursor".to_string(),
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

impl Default for ShortcutsConfig {
    fn default() -> Self {
        Self {
            send_message: "Ctrl+D".to_string(),
            interrupt: "Ctrl+C".to_string(),
            menu: "Esc".to_string(),
        }
    }
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            ui: UiConfig::default(),
            behavior: BehaviorConfig::default(),
            workspace: WorkspaceConfig::default(),
            shortcuts: ShortcutsConfig::default(),
        }
    }
}

impl CliConfig {
    /// Get configuration file path
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = if cfg!(target_os = "windows") {
            dirs::config_dir()
                .ok_or_else(|| anyhow::anyhow!("Cannot find config directory"))?
                .join("bitfun")
        } else {
            dirs::home_dir()
                .ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?
                .join(".config")
                .join("bitfun")
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
                .join("bitfun")
        } else {
            dirs::home_dir()
                .ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?
                .join(".config")
                .join("bitfun")
        };
        
        fs::create_dir_all(&config_dir)?;
        Ok(config_dir)
    }

    /// Get sessions directory
    #[allow(dead_code)]
    pub fn sessions_dir() -> Result<PathBuf> {
        let sessions_dir = Self::config_dir()?.join("sessions");
        fs::create_dir_all(&sessions_dir)?;
        Ok(sessions_dir)
    }

}
