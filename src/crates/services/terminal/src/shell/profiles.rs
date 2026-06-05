//! Shell profiles - Shell configuration profiles

use std::collections::HashMap;

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

/// Shell profile manager
#[allow(dead_code)]
pub struct ShellProfileManager {
    /// All profiles
    profiles: HashMap<String, ShellProfile>,

    /// Default profile ID
    default_profile_id: Option<String>,
}

#[allow(dead_code)]
impl ShellProfileManager {
    /// Create a new profile manager
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            default_profile_id: None,
        }
    }

    /// Initialize with detected shells
    pub fn init_from_detected(&mut self) {
        let shells = super::ShellDetector::detect_available_shells();

        for (i, shell) in shells.into_iter().enumerate() {
            let mut profile = ShellProfile::from_detected(&shell);

            // Set first profile as default
            if i == 0 {
                profile.is_default = true;
                self.default_profile_id = Some(profile.id.clone());
            }

            self.profiles.insert(profile.id.clone(), profile);
        }
    }

    /// Add a profile
    pub fn add_profile(&mut self, profile: ShellProfile) {
        if profile.is_default {
            self.default_profile_id = Some(profile.id.clone());
        }
        self.profiles.insert(profile.id.clone(), profile);
    }

    /// Remove a profile
    pub fn remove_profile(&mut self, id: &str) -> Option<ShellProfile> {
        let profile = self.profiles.remove(id)?;

        // If this was the default, clear default
        if self.default_profile_id.as_deref() == Some(id) {
            self.default_profile_id = None;
        }

        Some(profile)
    }

    /// Get a profile by ID
    pub fn get_profile(&self, id: &str) -> Option<&ShellProfile> {
        self.profiles.get(id)
    }

    /// Get the default profile
    pub fn get_default_profile(&self) -> Option<&ShellProfile> {
        self.default_profile_id
            .as_ref()
            .and_then(|id| self.profiles.get(id))
    }

    /// Set the default profile
    pub fn set_default_profile(&mut self, id: &str) -> bool {
        if self.profiles.contains_key(id) {
            // Clear previous default
            if let Some(old_id) = &self.default_profile_id {
                if let Some(old_profile) = self.profiles.get_mut(old_id) {
                    old_profile.is_default = false;
                }
            }

            // Set new default
            if let Some(profile) = self.profiles.get_mut(id) {
                profile.is_default = true;
            }
            self.default_profile_id = Some(id.to_string());
            true
        } else {
            false
        }
    }

    /// List all profiles
    pub fn list_profiles(&self) -> Vec<&ShellProfile> {
        self.profiles.values().collect()
    }

    /// List visible profiles (not hidden)
    pub fn list_visible_profiles(&self) -> Vec<&ShellProfile> {
        self.profiles.values().filter(|p| !p.hidden).collect()
    }
}

impl Default for ShellProfileManager {
    fn default() -> Self {
        let mut manager = Self::new();
        manager.init_from_detected();
        manager
    }
}
