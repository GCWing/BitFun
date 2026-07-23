//! Shell profiles - Shell configuration profiles

use serde::{Deserialize, Serialize};

use super::ShellType;
use crate::config::ShellConfig;

/// A shell profile with configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellProfile {
    /// Profile ID
    pub id: String,

    /// Display name
    pub name: String,

    /// Shell type
    pub shell_type: ShellType,

    /// Shell configuration
    pub config: ShellConfig,

    /// Whether this is the default profile
    pub is_default: bool,

    /// Icon identifier (optional)
    pub icon: Option<String>,

    /// Color (optional)
    pub color: Option<String>,

    /// Whether this profile is hidden
    pub hidden: bool,
}

impl ShellProfile {
    /// Create a new shell profile
    pub fn new(id: String, name: String, shell_type: ShellType, config: ShellConfig) -> Self {
        Self {
            id,
            name,
            shell_type,
            config,
            is_default: false,
            icon: None,
            color: None,
            hidden: false,
        }
    }

    /// Create a default profile from a detected shell
    pub fn from_detected(shell: &super::detection::DetectedShell) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: shell.display_name.clone(),
            shell_type: shell.shell_type.clone(),
            config: shell.to_config(),
            is_default: false,
            icon: None,
            color: None,
            hidden: false,
        }
    }
}
