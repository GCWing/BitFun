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

    if cfg!(windows) {
        resolve_windows_local_exec_shell_with(configured, resolve_detected_shell)
    } else {
        resolve_non_windows_local_exec_shell_with(
            configured,
            resolve_detected_shell,
            resolve_default_shell,
        )
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

fn resolve_non_windows_local_exec_shell_with<FindShell, DefaultShell>(
    configured: Option<ConfiguredShellPreference>,
    mut find_shell: FindShell,
    default_shell: DefaultShell,
) -> ResolvedLocalExecShell
where
    FindShell: FnMut(ShellType) -> Option<ResolvedLocalExecShell>,
    DefaultShell: FnOnce() -> ResolvedLocalExecShell,
{
    if let Some(shell_type) = configured.and_then(shell_type_for_supported_preference) {
        if let Some(shell) = find_shell(shell_type) {
            return shell;
        }
    }
    default_shell()
}

fn resolve_windows_local_exec_shell_with<FindShell>(
    configured: Option<ConfiguredShellPreference>,
    mut find_shell: FindShell,
) -> ResolvedLocalExecShell
where
    FindShell: FnMut(ShellType) -> Option<ResolvedLocalExecShell>,
{
    // ExecCommand deliberately narrows Windows shells to the variants whose
    // one-shot command behavior we explicitly support well.
    let order = match configured {
        Some(ConfiguredShellPreference::PowerShellCore) => [
            ShellType::PowerShellCore,
            ShellType::PowerShell,
            ShellType::Bash,
            ShellType::Cmd,
        ],
        Some(ConfiguredShellPreference::PowerShell) => [
            ShellType::PowerShell,
            ShellType::PowerShellCore,
            ShellType::Bash,
            ShellType::Cmd,
        ],
        Some(ConfiguredShellPreference::Bash) => [
            ShellType::Bash,
            ShellType::PowerShell,
            ShellType::PowerShellCore,
            ShellType::Cmd,
        ],
        Some(ConfiguredShellPreference::Cmd) | None => [
            ShellType::PowerShellCore,
            ShellType::PowerShell,
            ShellType::Bash,
            ShellType::Cmd,
        ],
        Some(
            ConfiguredShellPreference::Zsh
            | ConfiguredShellPreference::Fish
            | ConfiguredShellPreference::Sh
            | ConfiguredShellPreference::Ksh
            | ConfiguredShellPreference::Csh
            | ConfiguredShellPreference::Unsupported,
        ) => [
            ShellType::PowerShell,
            ShellType::PowerShellCore,
            ShellType::Bash,
            ShellType::Cmd,
        ],
    };

    for shell_type in order {
        if let Some(shell) = find_shell(shell_type) {
            return shell;
        }
    }

    ResolvedLocalExecShell::new(
        "Command Prompt".to_string(),
        PathBuf::from("cmd.exe"),
        ShellType::Cmd,
    )
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

fn resolve_detected_shell(shell_type: ShellType) -> Option<ResolvedLocalExecShell> {
    ShellDetector::find_shell(&shell_type)
        .map(|shell| ResolvedLocalExecShell::new(shell.display_name, shell.path, shell.shell_type))
}

fn resolve_default_shell() -> ResolvedLocalExecShell {
    let shell = ShellDetector::get_default_shell();
    ResolvedLocalExecShell::new(shell.display_name, shell.path, shell.shell_type)
}

#[cfg(test)]
mod tests {
    use super::{
        parse_configured_shell_preference, resolve_non_windows_local_exec_shell_with,
        resolve_windows_local_exec_shell_with, ConfiguredShellPreference, ResolvedLocalExecShell,
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

    fn find_in(
        detected: &[ResolvedLocalExecShell],
        shell_type: ShellType,
    ) -> Option<ResolvedLocalExecShell> {
        detected
            .iter()
            .find(|shell| shell.shell_type == shell_type)
            .cloned()
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
        let resolved = resolve_windows_local_exec_shell_with(
            Some(ConfiguredShellPreference::Cmd),
            |shell_type| find_in(&detected, shell_type),
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
        let resolved = resolve_windows_local_exec_shell_with(
            Some(ConfiguredShellPreference::PowerShellCore),
            |shell_type| find_in(&detected, shell_type),
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
        let resolved = resolve_windows_local_exec_shell_with(
            Some(ConfiguredShellPreference::Bash),
            |shell_type| find_in(&detected, shell_type),
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
        let resolved = resolve_windows_local_exec_shell_with(
            Some(ConfiguredShellPreference::Fish),
            |shell_type| find_in(&detected, shell_type),
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
        let resolved = resolve_windows_local_exec_shell_with(None, |shell_type| {
            find_in(&detected, shell_type)
        });

        assert_eq!(resolved.shell_type, ShellType::PowerShell);
    }

    #[test]
    fn non_windows_uses_configured_detected_shell_when_available() {
        let detected = vec![
            shell("Bash", "/bin/bash", ShellType::Bash),
            shell("Zsh", "/bin/zsh", ShellType::Zsh),
        ];
        let resolved = resolve_non_windows_local_exec_shell_with(
            Some(ConfiguredShellPreference::Zsh),
            |shell_type| find_in(&detected, shell_type),
            || detected[0].clone(),
        );

        assert_eq!(resolved.shell_type, ShellType::Zsh);
        assert_eq!(resolved.path, PathBuf::from("/bin/zsh"));
    }

    #[test]
    fn windows_selected_powershell_stops_before_pwsh_and_bash_fallbacks() {
        let expected = shell(
            "Windows PowerShell",
            "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
            ShellType::PowerShell,
        );
        let mut requested = Vec::new();

        let resolved = resolve_windows_local_exec_shell_with(
            Some(ConfiguredShellPreference::PowerShell),
            |shell_type| {
                requested.push(shell_type.clone());
                (shell_type == ShellType::PowerShell).then(|| expected.clone())
            },
        );

        assert_eq!(resolved, expected);
        assert_eq!(requested, vec![ShellType::PowerShell]);
    }

    #[test]
    fn windows_missing_selection_stops_at_first_available_fallback() {
        let expected = shell(
            "Windows PowerShell",
            "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
            ShellType::PowerShell,
        );
        let mut requested = Vec::new();

        let resolved = resolve_windows_local_exec_shell_with(
            Some(ConfiguredShellPreference::PowerShellCore),
            |shell_type| {
                requested.push(shell_type.clone());
                (shell_type == ShellType::PowerShell).then(|| expected.clone())
            },
        );

        assert_eq!(resolved, expected);
        assert_eq!(
            requested,
            vec![ShellType::PowerShellCore, ShellType::PowerShell]
        );
    }
}
