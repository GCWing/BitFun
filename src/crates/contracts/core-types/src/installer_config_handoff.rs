//! Preferences passed from the installer to the application.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const INSTALLER_CONFIG_HANDOFF_FILE_NAME: &str = "installer-config-handoff.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstallerConfigHandoff {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance_selection: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<InstallerModelHandoff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallerModelHandoff {
    pub name: String,
    pub provider: String,
    pub model_name: String,
    pub base_url: String,
    pub request_url: String,
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_headers: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_headers_mode: Option<String>,
    #[serde(default)]
    pub skip_ssl_verify: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_request_body: Option<String>,
}
