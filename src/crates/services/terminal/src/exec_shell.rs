use crate::shell::{ShellDetector, ShellType};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLocalExecShell {
    pub display_name: String,
    pub path: PathBuf,
    pub shell_type: ShellType,
}

impl ResolvedLocalExecShell {
    fn new(display_name: String, path: PathBuf, shell_type: ShellType) -> Self {
        Self {
            display_name,
            path,
            shell_type,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfiguredShellPreference {
    PowerShellCore,
    PowerShell,
    Bash,
    Cmd,
    Zsh,
    Fish,
    Sh,
    Ksh,
    Csh,
    Unsupported,
}

pub fn resolve_local_exec_shell(configured_shell: Option<&str>) -> ResolvedLocalExecShell {
    let configured = configured_shell.and_then(parse_configured_shell_preference);
    let detected_shells: Vec<_> = ShellDetector::detect_available_shells()
        .into_iter()
        .map(|shell| ResolvedLocalExecShell::new(shell.display_name, shell.path, shell.shell_type))
        .collect();
    let system_default = {
        let shell = ShellDetector::get_default_shell();
        ResolvedLocalExecShell::new(shell.display_name, shell.path, shell.shell_type)
    };

    if cfg!(windows) {
        select_windows_local_exec_shell(configured, &detected_shells, &system_default)
    } else {
        select_non_windows_local_exec_shell(configured, &detected_shells, &system_default)
    }
}

pub fn parse_configured_shell_preference(raw: &str) -> Option<ConfiguredShellPreference> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(trimmed)
        .to_ascii_lowercase();
    let normalized = normalized.trim_end_matches(".exe");

    Some(match normalized {
        "powershellcore" | "pwsh" => ConfiguredShellPreference::PowerShellCore,
        "powershell" | "windowspowershell" => ConfiguredShellPreference::PowerShell,
        "bash" | "gitbash" => ConfiguredShellPreference::Bash,
        "cmd" | "commandprompt" => ConfiguredShellPreference::Cmd,
        "zsh" => ConfiguredShellPreference::Zsh,
        "fish" => ConfiguredShellPreference::Fish,
        "sh" => ConfiguredShellPreference::Sh,
        "ksh" => ConfiguredShellPreference::Ksh,
        "csh" | "tcsh" => ConfiguredShellPreference::Csh,
        _ => ConfiguredShellPreference::Unsupported,
    })
}

fn select_non_windows_local_exec_shell(
    configured: Option<ConfiguredShellPreference>,
    detected_shells: &[ResolvedLocalExecShell],
    system_default: &ResolvedLocalExecShell,
) -> ResolvedLocalExecShell {
    configured
        .and_then(|preference| shell_type_for_supported_preference(preference))
        .and_then(|shell_type| find_detected_shell(detected_shells, shell_type))
        .unwrap_or_else(|| system_default.clone())
}

fn select_windows_local_exec_shell(
    configured: Option<ConfiguredShellPreference>,
    detected_shells: &[ResolvedLocalExecShell],
    system_default: &ResolvedLocalExecShell,
) -> ResolvedLocalExecShell {
    // ExecCommand deliberately narrows Windows shells to the variants whose
    // one-shot command behavior we explicitly support well.
    let pwsh = || find_detected_shell(detected_shells, ShellType::PowerShellCore);
    let powershell = || find_detected_shell(detected_shells, ShellType::PowerShell);
    let bash = || find_detected_shell(detected_shells, ShellType::Bash);

    match configured {
        Some(ConfiguredShellPreference::PowerShellCore) => pwsh()
            .or_else(powershell)
            .or_else(bash)
            .unwrap_or_else(|| system_default.clone()),
        Some(ConfiguredShellPreference::PowerShell) => powershell()
            .or_else(pwsh)
            .or_else(bash)
            .unwrap_or_else(|| system_default.clone()),
        Some(ConfiguredShellPreference::Bash) => bash()
            .or_else(powershell)
            .or_else(pwsh)
            .unwrap_or_else(|| system_default.clone()),
        Some(ConfiguredShellPreference::Cmd) => pwsh()
            .or_else(powershell)
            .or_else(bash)
            .unwrap_or_else(|| system_default.clone()),
        Some(
            ConfiguredShellPreference::Zsh
            | ConfiguredShellPreference::Fish
            | ConfiguredShellPreference::Sh
            | ConfiguredShellPreference::Ksh
            | ConfiguredShellPreference::Csh
            | ConfiguredShellPreference::Unsupported,
        ) => powershell()
            .or_else(pwsh)
            .or_else(bash)
            .unwrap_or_else(|| system_default.clone()),
        None => pwsh()
            .or_else(powershell)
            .or_else(bash)
            .unwrap_or_else(|| system_default.clone()),
    }
}

fn shell_type_for_supported_preference(preference: ConfiguredShellPreference) -> Option<ShellType> {
    Some(match preference {
        ConfiguredShellPreference::PowerShellCore => ShellType::PowerShellCore,
        ConfiguredShellPreference::PowerShell => ShellType::PowerShell,
        ConfiguredShellPreference::Bash => ShellType::Bash,
        ConfiguredShellPreference::Cmd => ShellType::Cmd,
        ConfiguredShellPreference::Zsh => ShellType::Zsh,
        ConfiguredShellPreference::Fish => ShellType::Fish,
        ConfiguredShellPreference::Sh => ShellType::Sh,
        ConfiguredShellPreference::Ksh => ShellType::Ksh,
        ConfiguredShellPreference::Csh => ShellType::Csh,
        ConfiguredShellPreference::Unsupported => return None,
    })
}

fn find_detected_shell(
    detected_shells: &[ResolvedLocalExecShell],
    shell_type: ShellType,
) -> Option<ResolvedLocalExecShell> {
    detected_shells
        .iter()
        .find(|shell| shell.shell_type == shell_type)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::{
        parse_configured_shell_preference, select_non_windows_local_exec_shell,
        select_windows_local_exec_shell, ConfiguredShellPreference, ResolvedLocalExecShell,
    };
    use crate::shell::ShellType;
    use std::path::PathBuf;

    fn shell(name: &str, path: &str, shell_type: ShellType) -> ResolvedLocalExecShell {
        ResolvedLocalExecShell {
            display_name: name.to_string(),
            path: PathBuf::from(path),
            shell_type,
        }
    }

    #[test]
    fn parses_configured_shell_values_from_enum_names_and_paths() {
        assert_eq!(
            parse_configured_shell_preference("PowerShellCore"),
            Some(ConfiguredShellPreference::PowerShellCore)
        );
        assert_eq!(
            parse_configured_shell_preference("C:\\Program Files\\PowerShell\\7\\pwsh.exe"),
            Some(ConfiguredShellPreference::PowerShellCore)
        );
        assert_eq!(
            parse_configured_shell_preference("Cmd"),
            Some(ConfiguredShellPreference::Cmd)
        );
        assert_eq!(
            parse_configured_shell_preference("/usr/bin/bash"),
            Some(ConfiguredShellPreference::Bash)
        );
        assert_eq!(parse_configured_shell_preference(""), None);
    }

    #[test]
    fn windows_cmd_prefers_pwsh_then_powershell() {
        let detected = vec![
            shell(
                "Windows PowerShell",
                "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
                ShellType::PowerShell,
            ),
            shell(
                "PowerShell 7",
                "C:\\Program Files\\PowerShell\\7\\pwsh.exe",
                ShellType::PowerShellCore,
            ),
            shell(
                "Git Bash",
                "C:\\Program Files\\Git\\bin\\bash.exe",
                ShellType::Bash,
            ),
        ];
        let resolved = select_windows_local_exec_shell(
            Some(ConfiguredShellPreference::Cmd),
            &detected,
            &detected[0],
        );

        assert_eq!(resolved.shell_type, ShellType::PowerShellCore);
        assert_eq!(
            resolved.path,
            PathBuf::from("C:\\Program Files\\PowerShell\\7\\pwsh.exe")
        );
    }

    #[test]
    fn windows_pwsh_falls_back_to_powershell_when_pwsh_is_missing() {
        let detected = vec![shell(
            "Windows PowerShell",
            "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
            ShellType::PowerShell,
        )];
        let resolved = select_windows_local_exec_shell(
            Some(ConfiguredShellPreference::PowerShellCore),
            &detected,
            &detected[0],
        );

        assert_eq!(resolved.shell_type, ShellType::PowerShell);
    }

    #[test]
    fn windows_bash_uses_detected_git_bash_path() {
        let detected = vec![
            shell(
                "PowerShell 7",
                "C:\\Program Files\\PowerShell\\7\\pwsh.exe",
                ShellType::PowerShellCore,
            ),
            shell("Git Bash", "D:\\Tools\\Git\\bin\\bash.exe", ShellType::Bash),
        ];
        let resolved = select_windows_local_exec_shell(
            Some(ConfiguredShellPreference::Bash),
            &detected,
            &detected[0],
        );

        assert_eq!(resolved.shell_type, ShellType::Bash);
        assert_eq!(
            resolved.path,
            PathBuf::from("D:\\Tools\\Git\\bin\\bash.exe")
        );
    }

    #[test]
    fn windows_unsupported_shell_falls_back_to_powershell() {
        let detected = vec![
            shell("Git Bash", "D:\\Tools\\Git\\bin\\bash.exe", ShellType::Bash),
            shell(
                "Windows PowerShell",
                "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
                ShellType::PowerShell,
            ),
        ];
        let resolved = select_windows_local_exec_shell(
            Some(ConfiguredShellPreference::Fish),
            &detected,
            &detected[0],
        );

        assert_eq!(resolved.shell_type, ShellType::PowerShell);
    }

    #[test]
    fn windows_auto_prefers_pwsh_then_powershell_then_bash() {
        let detected = vec![
            shell("Git Bash", "D:\\Tools\\Git\\bin\\bash.exe", ShellType::Bash),
            shell(
                "Windows PowerShell",
                "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
                ShellType::PowerShell,
            ),
        ];
        let resolved = select_windows_local_exec_shell(None, &detected, &detected[0]);

        assert_eq!(resolved.shell_type, ShellType::PowerShell);
    }

    #[test]
    fn non_windows_uses_configured_detected_shell_when_available() {
        let detected = vec![
            shell("Bash", "/bin/bash", ShellType::Bash),
            shell("Zsh", "/bin/zsh", ShellType::Zsh),
        ];
        let resolved = select_non_windows_local_exec_shell(
            Some(ConfiguredShellPreference::Zsh),
            &detected,
            &detected[0],
        );

        assert_eq!(resolved.shell_type, ShellType::Zsh);
        assert_eq!(resolved.path, PathBuf::from("/bin/zsh"));
    }
}
