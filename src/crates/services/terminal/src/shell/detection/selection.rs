use std::path::PathBuf;

use super::{path, DetectedShell, ShellCandidate, ShellDetector, ShellDiscoverySource, ShellType};

impl ShellDetector {
    /// Return the preferred local default shell for the current platform.
    pub fn get_default_shell() -> DetectedShell {
        #[cfg(windows)]
        {
            let shells = Self::detect_available_shells();
            return Self::find_in_detected(&shells, &ShellType::PowerShellCore)
                .or_else(|| Self::find_in_detected(&shells, &ShellType::PowerShell))
                .or_else(|| Self::find_in_detected(&shells, &ShellType::Cmd))
                .unwrap_or_else(|| {
                    DetectedShell::fallback(
                        ShellType::Cmd,
                        PathBuf::from("cmd.exe"),
                        "Command Prompt",
                    )
                });
        }
        #[cfg(not(windows))]
        {
            if let Ok(shell_path) = std::env::var("SHELL") {
                if let Some(shell) = Self::resolve_explicit_shell(&shell_path) {
                    return shell;
                }
            }
            let shells = Self::detect_available_shells();
            Self::find_in_detected(&shells, &ShellType::Bash)
                .or_else(|| Self::find_in_detected(&shells, &ShellType::Sh))
                .unwrap_or_else(|| {
                    DetectedShell::fallback(ShellType::Sh, PathBuf::from("/bin/sh"), "sh")
                })
        }
    }

    pub fn find_shell(shell_type: &ShellType) -> Option<DetectedShell> {
        Self::find_in_detected(&Self::detect_available_shells(), shell_type)
    }
    pub fn find_shell_by_id(id: &str) -> Option<DetectedShell> {
        Self::detect_available_shells()
            .into_iter()
            .find(|shell| shell.id == id)
    }

    pub fn resolve_explicit_shell(value: &str) -> Option<DetectedShell> {
        let path = PathBuf::from(value.trim());
        if !path::is_regular_file(&path) {
            return None;
        }
        Self::validate_candidate(ShellCandidate::new(
            path.clone(),
            ShellType::from_executable(path.to_string_lossy().as_ref()),
            ShellDiscoverySource::ExplicitConfig,
        ))
    }

    pub fn resolve_configured_shell(preference: &str) -> Option<DetectedShell> {
        Self::resolve_explicit_shell(preference).or_else(|| {
            Self::shell_type_from_preference(preference)
                .as_ref()
                .and_then(Self::find_shell)
        })
    }

    pub fn shell_type_from_preference(preference: &str) -> Option<ShellType> {
        let trimmed = preference.trim();
        if trimmed.is_empty() {
            return None;
        }
        let name = trimmed
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(trimmed)
            .to_ascii_lowercase();
        let name = name.strip_suffix(".exe").unwrap_or(&name);
        match name {
            "powershellcore" | "pwsh" => Some(ShellType::PowerShellCore),
            "powershell" | "windowspowershell" => Some(ShellType::PowerShell),
            "bash" | "gitbash" => Some(ShellType::Bash),
            "cmd" | "commandprompt" => Some(ShellType::Cmd),
            "zsh" => Some(ShellType::Zsh),
            "fish" => Some(ShellType::Fish),
            "sh" => Some(ShellType::Sh),
            "ksh" => Some(ShellType::Ksh),
            "csh" | "tcsh" => Some(ShellType::Csh),
            _ => None,
        }
    }

    fn find_in_detected(shells: &[DetectedShell], shell_type: &ShellType) -> Option<DetectedShell> {
        shells
            .iter()
            .find(|shell| &shell.shell_type == shell_type)
            .cloned()
    }
}
