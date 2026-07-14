use std::path::PathBuf;

#[cfg(windows)]
use super::DetectedShell;
use super::{ShellCandidate, ShellDetector, ShellDiscoverySource, ShellType};

#[cfg(windows)]
pub(super) fn windows_command_candidates() -> Vec<ShellCandidate> {
    std::env::var("COMSPEC")
        .ok()
        .map(PathBuf::from)
        .map(|path| ShellCandidate::new(path, ShellType::Cmd, ShellDiscoverySource::SystemInstall))
        .into_iter()
        .collect()
}

#[cfg(windows)]
pub(super) fn windows_powershell_candidates() -> Vec<ShellCandidate> {
    std::env::var("SYSTEMROOT")
        .ok()
        .map(|root| {
            PathBuf::from(root)
                .join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe")
        })
        .map(|path| {
            ShellCandidate::new(
                path,
                ShellType::PowerShell,
                ShellDiscoverySource::SystemInstall,
            )
        })
        .into_iter()
        .collect()
}

#[cfg(windows)]
pub(super) fn windows_pwsh_candidates() -> Vec<ShellCandidate> {
    let mut candidates = ShellDetector::find_all_in_path("pwsh")
        .into_iter()
        .map(|path| {
            ShellCandidate::new(path, ShellType::PowerShellCore, ShellDiscoverySource::Path)
        })
        .collect::<Vec<_>>();
    candidates.extend(windows_pwsh_location_candidates(
        std::env::var("LOCALAPPDATA").ok().map(PathBuf::from),
        std::env::var("USERPROFILE").ok().map(PathBuf::from),
        std::env::var("ProgramW6432").ok().map(PathBuf::from),
        std::env::var("ProgramFiles").ok().map(PathBuf::from),
        std::env::var("ProgramFiles(x86)").ok().map(PathBuf::from),
    ));
    candidates
}

#[cfg(windows)]
pub(super) fn windows_pwsh_location_candidates(
    local: Option<PathBuf>,
    user: Option<PathBuf>,
    w6432: Option<PathBuf>,
    program_files: Option<PathBuf>,
    program_files_x86: Option<PathBuf>,
) -> Vec<ShellCandidate> {
    let mut candidates = Vec::new();
    if let Some(local) = local {
        candidates.push(ShellCandidate::new(
            local.join("Microsoft").join("WindowsApps").join("pwsh.exe"),
            ShellType::PowerShellCore,
            ShellDiscoverySource::WindowsAppExecutionAlias,
        ));
        candidates.push(ShellCandidate::new(
            local
                .join("Programs")
                .join("PowerShell")
                .join("7")
                .join("pwsh.exe"),
            ShellType::PowerShellCore,
            ShellDiscoverySource::UserInstall,
        ));
    }
    if let Some(user) = user {
        candidates.push(ShellCandidate::new(
            user.join("scoop")
                .join("apps")
                .join("pwsh")
                .join("current")
                .join("pwsh.exe"),
            ShellType::PowerShellCore,
            ShellDiscoverySource::PackageManager,
        ));
    }
    for directory in [w6432, program_files, program_files_x86]
        .into_iter()
        .flatten()
    {
        candidates.push(ShellCandidate::new(
            directory.join("PowerShell").join("7").join("pwsh.exe"),
            ShellType::PowerShellCore,
            ShellDiscoverySource::SystemInstall,
        ));
    }
    candidates
}

#[cfg(not(windows))]
pub(super) fn posix_shell_candidates() -> Vec<ShellCandidate> {
    let mut candidates = Vec::new();
    for shell_type in [
        ShellType::Bash,
        ShellType::Zsh,
        ShellType::Fish,
        ShellType::Sh,
    ] {
        let executable = shell_type.default_executable();
        candidates.extend(
            ShellDetector::find_all_in_path(executable)
                .into_iter()
                .map(|path| {
                    ShellCandidate::new(path, shell_type.clone(), ShellDiscoverySource::Path)
                }),
        );
        for directory in ["/usr/local/bin", "/usr/bin", "/bin"] {
            candidates.push(ShellCandidate::new(
                PathBuf::from(directory).join(executable),
                shell_type.clone(),
                ShellDiscoverySource::SystemInstall,
            ));
        }
    }
    candidates
}

#[cfg(not(windows))]
pub(super) fn non_windows_pwsh_candidates() -> Vec<ShellCandidate> {
    let mut candidates = ShellDetector::find_all_in_path("pwsh")
        .into_iter()
        .map(|path| {
            ShellCandidate::new(path, ShellType::PowerShellCore, ShellDiscoverySource::Path)
        })
        .collect::<Vec<_>>();
    for path in [
        PathBuf::from("/usr/local/bin/pwsh"),
        PathBuf::from("/usr/bin/pwsh"),
        PathBuf::from("/opt/microsoft/powershell/7/pwsh"),
    ] {
        candidates.push(ShellCandidate::new(
            path,
            ShellType::PowerShellCore,
            ShellDiscoverySource::SystemInstall,
        ));
    }
    candidates
}

#[cfg(windows)]
pub(super) fn detect_git_bash() -> Option<DetectedShell> {
    let mut paths = Vec::new();
    if let Some(git) = ShellDetector::find_all_in_path("git")
        .into_iter()
        .find(|path| path.is_file())
    {
        if let Some(parent) = git.parent() {
            if parent
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.eq_ignore_ascii_case("cmd") || name.eq_ignore_ascii_case("bin")
                })
            {
                if let Some(install) = parent.parent() {
                    paths.extend([
                        install.join("bin").join("bash.exe"),
                        install.join("usr").join("bin").join("bash.exe"),
                    ]);
                }
            }
        }
    }
    for base in [
        std::env::var("ProgramW6432").ok(),
        std::env::var("ProgramFiles").ok(),
        std::env::var("ProgramFiles(x86)").ok(),
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|path| format!("{path}\\Programs")),
    ]
    .into_iter()
    .flatten()
    {
        let git = PathBuf::from(&base).join("Git");
        paths.extend([
            git.join("bin").join("bash.exe"),
            git.join("usr").join("bin").join("bash.exe"),
            PathBuf::from(base).join("usr").join("bin").join("bash.exe"),
        ]);
    }
    if let Ok(user) = std::env::var("USERPROFILE") {
        let apps = PathBuf::from(user).join("scoop").join("apps");
        paths.extend([
            apps.join("git")
                .join("current")
                .join("bin")
                .join("bash.exe"),
            apps.join("git")
                .join("current")
                .join("usr")
                .join("bin")
                .join("bash.exe"),
            apps.join("git-with-openssh")
                .join("current")
                .join("bin")
                .join("bash.exe"),
        ]);
    }
    paths.into_iter().find_map(|path| {
        let value = path.to_string_lossy().to_ascii_lowercase();
        if value.contains("system32") || value.contains("syswow64") {
            None
        } else {
            ShellDetector::validate_candidate(ShellCandidate::new(
                path,
                ShellType::Bash,
                ShellDiscoverySource::SystemInstall,
            ))
        }
    })
}
