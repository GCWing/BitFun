//! Shell detection - Detect available shells on the system

use std::collections::HashMap;
use std::path::PathBuf;

use super::ShellType;

/// Shell detector for finding available shells
pub struct ShellDetector;

impl ShellDetector {
    /// Detect available shells on the system
    pub fn detect_available_shells() -> Vec<DetectedShell> {
        let mut shells = Vec::new();

        #[cfg(windows)]
        {
            // Check for CMD
            if let Some(shell) = Self::detect_cmd() {
                shells.push(shell);
            }

            // Check for PowerShell
            if let Some(shell) = Self::detect_powershell() {
                shells.push(shell);
            }

            // Check for PowerShell Core
            if let Some(shell) = Self::detect_pwsh() {
                shells.push(shell);
            }

            // Check for Git Bash
            if let Some(shell) = Self::detect_git_bash() {
                shells.push(shell);
            }

            // Check for WSL shells
            // shells.extend(Self::detect_wsl_shells());
        }

        #[cfg(not(windows))]
        {
            // Check for common POSIX shells
            for shell_type in &[
                ShellType::Bash,
                ShellType::Zsh,
                ShellType::Fish,
                ShellType::Sh,
                // ShellType::Ksh,
                // ShellType::Csh,
            ] {
                if let Some(shell) = Self::detect_posix_shell(shell_type) {
                    shells.push(shell);
                }
            }

            // Check for PowerShell Core on non-Windows
            if let Some(shell) = Self::detect_pwsh() {
                shells.push(shell);
            }
        }

        shells
    }

    /// Get the default shell for the current system
    pub fn get_default_shell() -> DetectedShell {
        #[cfg(windows)]
        {
            // Priority: PowerShell Core (pwsh) > Windows PowerShell > Cmd
            Self::detect_pwsh()
                .or_else(Self::detect_powershell)
                .or_else(Self::detect_cmd)
                .unwrap_or_else(|| DetectedShell {
                    shell_type: ShellType::Cmd,
                    path: PathBuf::from("cmd.exe"),
                    version: None,
                    display_name: "Command Prompt".to_string(),
                })
        }

        #[cfg(not(windows))]
        {
            // Try to use $SHELL environment variable
            if let Ok(shell_path) = std::env::var("SHELL") {
                let shell_type = ShellType::from_executable(&shell_path);
                return DetectedShell {
                    shell_type: shell_type.clone(),
                    path: PathBuf::from(&shell_path),
                    version: Self::get_shell_version(&shell_path),
                    display_name: shell_type.to_string(),
                };
            }

            Self::detect_posix_shell(&ShellType::Bash)
                .or_else(|| Self::detect_posix_shell(&ShellType::Sh))
                .unwrap_or_else(|| DetectedShell {
                    shell_type: ShellType::Sh,
                    path: PathBuf::from("/bin/sh"),
                    version: None,
                    display_name: "sh".to_string(),
                })
        }
    }

    #[cfg(windows)]
    fn detect_cmd() -> Option<DetectedShell> {
        let comspec = std::env::var("COMSPEC").ok()?;
        let path = PathBuf::from(&comspec);

        if path.exists() {
            Some(DetectedShell {
                shell_type: ShellType::Cmd,
                path,
                version: None,
                display_name: "Command Prompt".to_string(),
            })
        } else {
            None
        }
    }

    #[cfg(windows)]
    fn detect_powershell() -> Option<DetectedShell> {
        let system32 = std::env::var("SYSTEMROOT").ok()?;
        let path = PathBuf::from(&system32)
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe");

        if path.exists() {
            Some(DetectedShell {
                shell_type: ShellType::PowerShell,
                path,
                version: None,
                display_name: "Windows PowerShell".to_string(),
            })
        } else {
            None
        }
    }

    fn detect_pwsh() -> Option<DetectedShell> {
        // Check common locations for pwsh
        let candidates = if cfg!(windows) {
            vec![
                PathBuf::from(std::env::var("ProgramFiles").unwrap_or_default())
                    .join("PowerShell")
                    .join("7")
                    .join("pwsh.exe"),
            ]
        } else {
            vec![
                PathBuf::from("/usr/local/bin/pwsh"),
                PathBuf::from("/usr/bin/pwsh"),
                PathBuf::from("/opt/microsoft/powershell/7/pwsh"),
            ]
        };

        for path in candidates {
            if path.exists() {
                return Some(DetectedShell {
                    shell_type: ShellType::PowerShellCore,
                    path,
                    version: None,
                    display_name: "PowerShell 7".to_string(),
                });
            }
        }

        // Try which/where command
        Self::find_in_path("pwsh").map(|path| DetectedShell {
            shell_type: ShellType::PowerShellCore,
            path,
            version: None,
            display_name: "PowerShell 7".to_string(),
        })
    }

    #[cfg(windows)]
    pub fn detect_git_bash() -> Option<DetectedShell> {
        // Collect potential Git Bash paths directly (not just directories)
        let mut bash_candidates: Vec<PathBuf> = Vec::new();

        // Method 1: Find git.exe in PATH and derive Git installation directory
        // git.exe is typically at <install_dir>/cmd/git.exe or <install_dir>/bin/git.exe
        if let Some(git_exe_path) = Self::find_in_path("git") {
            if let Some(parent_dir) = git_exe_path.parent() {
                // Check if parent is "cmd" or "bin" directory
                if let Some(dir_name) = parent_dir.file_name().and_then(|n| n.to_str()) {
                    if dir_name.eq_ignore_ascii_case("cmd") || dir_name.eq_ignore_ascii_case("bin")
                    {
                        if let Some(git_install_dir) = parent_dir.parent() {
                            // Only add if it looks like a Git installation (has bin/bash.exe or usr/bin/bash.exe)
                            let bin_bash = git_install_dir.join("bin").join("bash.exe");
                            let usr_bin_bash =
                                git_install_dir.join("usr").join("bin").join("bash.exe");
                            if bin_bash.exists() {
                                bash_candidates.push(bin_bash);
                            }
                            if usr_bin_bash.exists() {
                                bash_candidates.push(usr_bin_bash);
                            }
                        }
                    }
                }
            }
        }

        // Method 2: Check common Git installation locations
        let base_dirs: Vec<Option<String>> = vec![
            std::env::var("ProgramW6432").ok(),
            std::env::var("ProgramFiles").ok(),
            std::env::var("ProgramFiles(x86)").ok(),
            std::env::var("LOCALAPPDATA")
                .map(|p| format!("{}\\Programs", p))
                .ok(),
        ];

        for base_dir in base_dirs.into_iter().flatten() {
            let git_dir = PathBuf::from(&base_dir).join("Git");
            // Git/bin/bash.exe
            bash_candidates.push(git_dir.join("bin").join("bash.exe"));
            // Git/usr/bin/bash.exe (MSYS2 style)
            bash_candidates.push(git_dir.join("usr").join("bin").join("bash.exe"));
            // Using Git for Windows SDK
            let sdk_dir = PathBuf::from(&base_dir);
            bash_candidates.push(sdk_dir.join("usr").join("bin").join("bash.exe"));
        }

        // Method 3: Check Scoop installation
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let scoop_git = PathBuf::from(&user_profile)
                .join("scoop")
                .join("apps")
                .join("git")
                .join("current");
            bash_candidates.push(scoop_git.join("bin").join("bash.exe"));
            bash_candidates.push(scoop_git.join("usr").join("bin").join("bash.exe"));

            // git-with-openssh variant
            let scoop_git_ssh = PathBuf::from(&user_profile)
                .join("scoop")
                .join("apps")
                .join("git-with-openssh")
                .join("current");
            bash_candidates.push(scoop_git_ssh.join("bin").join("bash.exe"));
        }

        // Find the first existing bash.exe
        // Note: We explicitly avoid System32\bash.exe which is WSL, not Git Bash
        for bash_path in bash_candidates {
            if bash_path.exists() {
                // Double-check this is NOT the WSL bash by verifying path doesn't contain System32
                let path_str = bash_path.to_string_lossy().to_lowercase();
                if path_str.contains("system32") || path_str.contains("syswow64") {
                    continue; // Skip WSL bash
                }

                return Some(DetectedShell {
                    shell_type: ShellType::Bash,
                    path: bash_path,
                    version: None,
                    display_name: "Git Bash".to_string(),
                });
            }
        }
        None
    }

    #[cfg(windows)]
    #[allow(dead_code)]
    fn detect_wsl_shells() -> Vec<DetectedShell> {
        // TODO: Enumerate WSL distributions
        Vec::new()
    }

    #[cfg(not(windows))]
    fn detect_posix_shell(shell_type: &ShellType) -> Option<DetectedShell> {
        let executable = shell_type.default_executable();

        // Check common locations
        let candidates = vec![
            PathBuf::from(format!("/usr/local/bin/{}", executable)),
            PathBuf::from(format!("/usr/bin/{}", executable)),
            PathBuf::from(format!("/bin/{}", executable)),
        ];

        for path in candidates {
            if path.exists() {
                let version = Self::get_shell_version(path.to_str().unwrap_or(""));
                return Some(DetectedShell {
                    shell_type: shell_type.clone(),
                    path: path.clone(),
                    version,
                    display_name: shell_type.to_string(),
                });
            }
        }

        None
    }

    fn find_in_path(executable: &str) -> Option<PathBuf> {
        #[cfg(windows)]
        let path_var = std::env::var("PATH").ok()?;
        #[cfg(not(windows))]
        let path_var = std::env::var("PATH").ok()?;

        let sep = if cfg!(windows) { ';' } else { ':' };

        for dir in path_var.split(sep) {
            let candidate = PathBuf::from(dir).join(executable);
            if candidate.exists() {
                return Some(candidate);
            }

            #[cfg(windows)]
            {
                let candidate_exe = PathBuf::from(dir).join(format!("{}.exe", executable));
                if candidate_exe.exists() {
                    return Some(candidate_exe);
                }
            }
        }

        None
    }

    #[allow(dead_code)]
    fn get_shell_version(path: &str) -> Option<String> {
        #[cfg(windows)]
        let output = {
            use std::os::windows::process::CommandExt;

            const CREATE_NO_WINDOW: u32 = 0x0800_0000;

            let mut command = std::process::Command::new(path);
            command.creation_flags(CREATE_NO_WINDOW);
            command.arg("--version").output().ok()?
        };

        #[cfg(not(windows))]
        let output = std::process::Command::new(path)
            .arg("--version")
            .output()
            .ok()?;

        if output.status.success() {
            String::from_utf8(output.stdout)
                .ok()
                .map(|s| s.lines().next().unwrap_or("").to_string())
        } else {
            None
        }
    }
}

/// Information about a detected shell
#[derive(Debug, Clone)]
pub struct DetectedShell {
    /// Shell type
    pub shell_type: ShellType,
    /// Path to the shell executable
    pub path: PathBuf,
    /// Shell version (if detected)
    pub version: Option<String>,
    /// Display name for UI
    pub display_name: String,
}

impl DetectedShell {
    /// Convert to a ShellConfig
    pub fn to_config(&self) -> crate::config::ShellConfig {
        // Shell launch arguments vary by platform:
        //
        // macOS: POSIX shells need login shell mode (-l) because apps launched
        // from Finder/Dock don't inherit shell config. This ensures:
        // - ~/.bash_profile, ~/.zprofile (PATH, environment variables)
        // - ~/.bashrc, ~/.zshrc (prompt colors, aliases)
        //
        // Linux: Desktop environments typically source user profiles correctly,
        // so login shell mode is not needed by default.
        //
        // Windows Git Bash: Uses ['--login', '-i'] (login + interactive) to ensure
        // proper environment setup.

        #[cfg(target_os = "macos")]
        let (args, use_login_shell) = if self.shell_type.is_posix() {
            (vec!["-l".to_string()], true)
        } else {
            (Vec::new(), false)
        };

        #[cfg(target_os = "linux")]
        let (args, use_login_shell) = (Vec::new(), false);

        #[cfg(windows)]
        let (args, use_login_shell) = if matches!(self.shell_type, ShellType::Bash) {
            // Git Bash on Windows: use --login -i
            (vec!["--login".to_string(), "-i".to_string()], true)
        } else {
            (Vec::new(), false)
        };

        crate::config::ShellConfig {
            executable: self.path.to_string_lossy().to_string(),
            args,
            env: HashMap::new(),
            cwd: None,
            login: use_login_shell,
        }
    }
}
