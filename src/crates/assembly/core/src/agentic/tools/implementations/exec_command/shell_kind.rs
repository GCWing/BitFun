use terminal_core::ShellType;
use tool_runtime::exec_command::ExecCommandShellKind;

pub(super) fn exec_command_shell_kind(shell_type: &ShellType) -> ExecCommandShellKind {
    match shell_type {
        ShellType::Bash => ExecCommandShellKind::Bash,
        ShellType::Zsh => ExecCommandShellKind::Zsh,
        ShellType::Fish => ExecCommandShellKind::Fish,
        ShellType::PowerShell => ExecCommandShellKind::PowerShell,
        ShellType::PowerShellCore => ExecCommandShellKind::PowerShellCore,
        ShellType::Cmd => ExecCommandShellKind::Cmd,
        ShellType::Sh => ExecCommandShellKind::Sh,
        ShellType::Ksh => ExecCommandShellKind::Ksh,
        ShellType::Csh => ExecCommandShellKind::Csh,
        ShellType::Custom(name) => ExecCommandShellKind::Custom(name.clone()),
    }
}

pub(super) fn terminal_shell_type(shell_kind: ExecCommandShellKind) -> ShellType {
    match shell_kind {
        ExecCommandShellKind::Bash => ShellType::Bash,
        ExecCommandShellKind::Zsh => ShellType::Zsh,
        ExecCommandShellKind::Fish => ShellType::Fish,
        ExecCommandShellKind::PowerShell => ShellType::PowerShell,
        ExecCommandShellKind::PowerShellCore => ShellType::PowerShellCore,
        ExecCommandShellKind::Cmd => ShellType::Cmd,
        ExecCommandShellKind::Sh => ShellType::Sh,
        ExecCommandShellKind::Ksh => ShellType::Ksh,
        ExecCommandShellKind::Csh => ShellType::Csh,
        ExecCommandShellKind::Custom(name) => ShellType::Custom(name),
    }
}
