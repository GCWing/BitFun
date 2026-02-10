//! Shell Integration Scripts Manager
//!
//! Manages shell integration scripts with hash-based update detection.
//! Scripts are generated once and shared across all terminal sessions.

use super::ShellType;
use log::info;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};

// Embedded script contents
const BASH_SCRIPT: &str = include_str!("scripts/shellIntegration-bash.sh");
const ZSH_SCRIPT: &str = include_str!("scripts/shellIntegration-rc.zsh");
const FISH_SCRIPT: &str = include_str!("scripts/shellIntegration.fish");
const POWERSHELL_SCRIPT: &str = include_str!("scripts/shellIntegration.ps1");

/// Manages shell integration scripts with hash-based update detection
pub struct ScriptsManager {
    /// Directory where scripts are stored
    scripts_dir: PathBuf,
}

impl ScriptsManager {
    /// Create a new ScriptsManager with optional custom directory
    ///
    /// If `scripts_dir` is None, uses the default location:
    /// - Linux: `~/.cache/bitfun_terminal/scripts/`
    /// - Windows: `%LOCALAPPDATA%\bitfun_terminal\scripts\`
    /// - macOS: `~/Library/Caches/bitfun_terminal/scripts/`
    pub fn new(scripts_dir: Option<PathBuf>) -> Self {
        let dir = scripts_dir.unwrap_or_else(Self::default_scripts_dir);
        Self { scripts_dir: dir }
    }

    /// Get the default scripts directory
    fn default_scripts_dir() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("bitfun_terminal")
            .join("scripts")
    }

    /// Get the scripts directory path
    pub fn scripts_dir(&self) -> &Path {
        &self.scripts_dir
    }

    /// Ensure scripts are up-to-date
    ///
    /// Checks the content hash and regenerates scripts only if needed.
    /// Returns Ok(true) if scripts were regenerated, Ok(false) if already up-to-date.
    pub fn ensure_scripts(&self) -> io::Result<bool> {
        let hash = Self::compute_hash();
        let hash_file = self.scripts_dir.join(".hash");

        // Check existing hash
        if hash_file.exists() {
            if let Ok(existing) = fs::read_to_string(&hash_file) {
                if existing.trim() == hash {
                    return Ok(false);
                }
            }
        }

        info!(
            "Generating shell integration scripts at {:?}",
            self.scripts_dir
        );

        // Clean and recreate directory
        if self.scripts_dir.exists() {
            fs::remove_dir_all(&self.scripts_dir)?;
        }
        fs::create_dir_all(&self.scripts_dir)?;

        // Write scripts
        fs::write(self.scripts_dir.join("bash.sh"), BASH_SCRIPT)?;
        fs::write(self.scripts_dir.join("powershell.ps1"), POWERSHELL_SCRIPT)?;
        fs::write(self.scripts_dir.join("fish.fish"), FISH_SCRIPT)?;

        // Zsh needs ZDOTDIR structure (directory with .zshrc inside)
        let zsh_dir = self.scripts_dir.join("zsh");
        fs::create_dir_all(&zsh_dir)?;
        fs::write(zsh_dir.join(".zshrc"), ZSH_SCRIPT)?;

        // Write hash file
        fs::write(&hash_file, &hash)?;

        info!("Shell integration scripts generated successfully");
        Ok(true)
    }

    /// Compute hash of all script contents
    fn compute_hash() -> String {
        let mut hasher = DefaultHasher::new();
        BASH_SCRIPT.hash(&mut hasher);
        ZSH_SCRIPT.hash(&mut hasher);
        FISH_SCRIPT.hash(&mut hasher);
        POWERSHELL_SCRIPT.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Get the script path for a given shell type
    ///
    /// Returns None for shells that don't support integration or use inline scripts.
    pub fn get_script_path(&self, shell_type: &ShellType) -> Option<PathBuf> {
        match shell_type {
            ShellType::Bash => Some(self.scripts_dir.join("bash.sh")),
            ShellType::Zsh => Some(self.scripts_dir.join("zsh")), // ZDOTDIR path
            ShellType::Fish => Some(self.scripts_dir.join("fish.fish")),
            ShellType::PowerShell | ShellType::PowerShellCore => {
                Some(self.scripts_dir.join("powershell.ps1"))
            }
            _ => None,
        }
    }

    /// Get the raw script content for a given shell type
    ///
    /// This returns the embedded script content directly, useful for shells
    /// that support inline script injection (like Fish).
    pub fn get_script_content(shell_type: &ShellType) -> Option<&'static str> {
        match shell_type {
            ShellType::Bash => Some(BASH_SCRIPT),
            ShellType::Zsh => Some(ZSH_SCRIPT),
            ShellType::Fish => Some(FISH_SCRIPT),
            ShellType::PowerShell | ShellType::PowerShellCore => Some(POWERSHELL_SCRIPT),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash_is_deterministic() {
        let hash1 = ScriptsManager::compute_hash();
        let hash2 = ScriptsManager::compute_hash();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_get_script_path() {
        let manager = ScriptsManager::new(Some(PathBuf::from("/tmp/test_scripts")));

        assert_eq!(
            manager.get_script_path(&ShellType::Bash),
            Some(PathBuf::from("/tmp/test_scripts/bash.sh"))
        );
        assert_eq!(
            manager.get_script_path(&ShellType::Zsh),
            Some(PathBuf::from("/tmp/test_scripts/zsh"))
        );
        assert_eq!(
            manager.get_script_path(&ShellType::PowerShell),
            Some(PathBuf::from("/tmp/test_scripts/powershell.ps1"))
        );
        assert_eq!(manager.get_script_path(&ShellType::Cmd), None);
    }

    #[test]
    fn test_get_script_content() {
        assert!(ScriptsManager::get_script_content(&ShellType::Bash).is_some());
        assert!(ScriptsManager::get_script_content(&ShellType::Zsh).is_some());
        assert!(ScriptsManager::get_script_content(&ShellType::Fish).is_some());
        assert!(ScriptsManager::get_script_content(&ShellType::PowerShell).is_some());
        assert!(ScriptsManager::get_script_content(&ShellType::Cmd).is_none());
    }
}
