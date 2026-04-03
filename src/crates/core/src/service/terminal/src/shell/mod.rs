//! Shell module - Shell detection and configuration
//!
//! This module provides shell type detection, profile management,
//! and shell integration script handling.

mod detection;
pub mod integration;
mod profiles;
mod scripts_manager;

pub use detection::ShellDetector;
pub use integration::{
    get_injection_command, get_integration_script_content, get_integration_script_path,
    CommandState, OscSequence, ShellIntegration, ShellIntegrationEvent, ShellIntegrationManager,
};
pub use profiles::ShellProfile;
pub use scripts_manager::ScriptsManager;

use serde::{Deserialize, Serialize};

/// Supported shell types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ShellType {
    /// Bash shell
    Bash,
    /// Zsh shell
    Zsh,
    /// Fish shell
    Fish,
    /// PowerShell (Windows PowerShell)
    PowerShell,
    /// PowerShell Core (cross-platform)
    #[default]
    PowerShellCore,
    /// Windows CMD
    Cmd,
    /// Sh (POSIX shell)
    Sh,
    /// Ksh (Korn shell)
    Ksh,
    /// Csh (C shell)
    Csh,
    /// Custom shell with name
    Custom(String),
}

impl ShellType {
    /// Get the display name for this shell type (platform-specific)
    pub fn name(&self) -> &str {
        match self {
            #[cfg(windows)]
            ShellType::Bash => "Git Bash",
            #[cfg(not(windows))]
            ShellType::Bash => "Bash",
            ShellType::Zsh => "Zsh",
            ShellType::Fish => "Fish",
            ShellType::PowerShell => "Windows PowerShell",
            ShellType::PowerShellCore => "PowerShell 7",
            ShellType::Cmd => "Command Prompt",
            ShellType::Sh => "sh",
            ShellType::Ksh => "Ksh",
            ShellType::Csh => "Csh",
            ShellType::Custom(name) => name,
        }
    }

    /// Get the default executable name for this shell type
    pub fn default_executable(&self) -> &str {
        match self {
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::Fish => "fish",
            ShellType::PowerShell => {
                #[cfg(windows)]
                {
                    "powershell.exe"
                }
                #[cfg(not(windows))]
                {
                    "pwsh"
                }
            }
            ShellType::PowerShellCore => "pwsh",
            ShellType::Cmd => "cmd.exe",
            ShellType::Sh => "sh",
            ShellType::Ksh => "ksh",
            ShellType::Csh => "csh",
            ShellType::Custom(name) => name,
        }
    }

    /// Check if this is a POSIX-compatible shell
    pub fn is_posix(&self) -> bool {
        matches!(
            self,
            ShellType::Bash | ShellType::Zsh | ShellType::Sh | ShellType::Ksh | ShellType::Csh
        )
    }

    /// Check if this shell supports shell integration
    pub fn supports_integration(&self) -> bool {
        matches!(
            self,
            ShellType::Bash
                | ShellType::Zsh
                | ShellType::Fish
                | ShellType::PowerShell
                | ShellType::PowerShellCore
        )
    }

    /// Parse shell type from executable path
    pub fn from_executable(path: &str) -> Self {
        let name = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(path)
            .to_lowercase();

        match name.as_str() {
            "bash" => ShellType::Bash,
            "zsh" => ShellType::Zsh,
            "fish" => ShellType::Fish,
            "powershell" => ShellType::PowerShell,
            "pwsh" => ShellType::PowerShellCore,
            "cmd" => ShellType::Cmd,
            "sh" => ShellType::Sh,
            "ksh" => ShellType::Ksh,
            "csh" | "tcsh" => ShellType::Csh,
            _ => ShellType::Custom(name),
        }
    }
}


impl std::fmt::Display for ShellType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShellType::Bash => write!(f, "bash"),
            ShellType::Zsh => write!(f, "zsh"),
            ShellType::Fish => write!(f, "fish"),
            ShellType::PowerShell => write!(f, "powershell"),
            ShellType::PowerShellCore => write!(f, "pwsh"),
            ShellType::Cmd => write!(f, "cmd"),
            ShellType::Sh => write!(f, "sh"),
            ShellType::Ksh => write!(f, "ksh"),
            ShellType::Csh => write!(f, "csh"),
            ShellType::Custom(name) => write!(f, "{}", name),
        }
    }
}
