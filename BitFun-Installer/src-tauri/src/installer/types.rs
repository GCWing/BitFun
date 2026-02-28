use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Installation options passed from the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallOptions {
    /// Target installation directory
    pub install_path: String,
    /// Create a desktop shortcut
    pub desktop_shortcut: bool,
    /// Add to Start Menu
    pub start_menu: bool,
    /// Register right-click context menu ("Open with BitFun")
    pub context_menu: bool,
    /// Add to system PATH
    pub add_to_path: bool,
    /// Launch after installation
    pub launch_after_install: bool,
    /// First-launch app language (zh-CN / en-US)
    pub app_language: String,
    /// First-launch theme preference (BitFun built-in theme id)
    pub theme_preference: String,
    /// Optional first-launch model configuration.
    pub model_config: Option<ModelConfig>,
}

/// Optional model configuration (from installer model step).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelConfig {
    pub provider: String,
    pub api_key: String,
    pub base_url: String,
    pub model_name: String,
    pub format: String,
    #[serde(default)]
    pub config_name: Option<String>,
    #[serde(default)]
    pub custom_request_body: Option<String>,
    #[serde(default)]
    pub skip_ssl_verify: Option<bool>,
    #[serde(default)]
    pub custom_headers: Option<HashMap<String, String>>,
    #[serde(default)]
    pub custom_headers_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTestResult {
    pub success: bool,
    pub response_time_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_response: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_details: Option<String>,
}

/// Progress update sent to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    /// Current step name
    pub step: String,
    /// Progress percentage (0-100)
    pub percent: u32,
    /// Human-readable status message
    pub message: String,
}

/// Disk space information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskSpaceInfo {
    /// Total disk space in bytes
    pub total: u64,
    /// Available disk space in bytes
    pub available: u64,
    /// Required space in bytes (estimated)
    pub required: u64,
    /// Whether there is enough space
    pub sufficient: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            install_path: String::new(),
            desktop_shortcut: true,
            start_menu: true,
            context_menu: true,
            add_to_path: true,
            launch_after_install: true,
            app_language: "zh-CN".to_string(),
            theme_preference: "bitfun-dark".to_string(),
            model_config: None,
        }
    }
}
