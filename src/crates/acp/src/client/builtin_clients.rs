use std::collections::HashMap;

use super::config::{AcpClientConfig, AcpClientPermissionMode};

pub(crate) struct BuiltinAcpClientPreset {
    pub(crate) id: &'static str,
    pub(crate) command: &'static str,
    pub(crate) args: &'static [&'static str],
    pub(crate) tool_command: &'static str,
    pub(crate) install_package: &'static str,
    pub(crate) adapter_package: Option<&'static str>,
    pub(crate) adapter_bin: Option<&'static str>,
}

const BUILTIN_ACP_CLIENT_PRESETS: &[BuiltinAcpClientPreset] = &[
    BuiltinAcpClientPreset {
        id: "opencode",
        command: "opencode",
        args: &["acp"],
        tool_command: "opencode",
        install_package: "opencode-ai",
        adapter_package: None,
        adapter_bin: None,
    },
    BuiltinAcpClientPreset {
        id: "claude-code",
        command: "npx",
        args: &["--yes", "@zed-industries/claude-code-acp@latest"],
        tool_command: "claude",
        install_package: "@anthropic-ai/claude-code",
        adapter_package: Some("@zed-industries/claude-code-acp"),
        adapter_bin: Some("claude-code-acp"),
    },
    BuiltinAcpClientPreset {
        id: "codex",
        command: "npx",
        args: &["--yes", "@zed-industries/codex-acp@latest"],
        tool_command: "codex",
        install_package: "@openai/codex",
        adapter_package: Some("@zed-industries/codex-acp"),
        adapter_bin: Some("codex-acp"),
    },
];

pub(crate) fn builtin_client_ids() -> impl Iterator<Item = &'static str> {
    BUILTIN_ACP_CLIENT_PRESETS.iter().map(|preset| preset.id)
}

pub(crate) fn builtin_acp_client_preset(
    client_id: &str,
) -> Option<&'static BuiltinAcpClientPreset> {
    BUILTIN_ACP_CLIENT_PRESETS
        .iter()
        .find(|preset| preset.id == client_id)
}

pub(crate) fn supported_remote_acp_clients() -> String {
    builtin_client_ids().collect::<Vec<_>>().join(", ")
}

pub(crate) fn default_config_for_builtin_client(client_id: &str) -> Option<AcpClientConfig> {
    let preset = builtin_acp_client_preset(client_id)?;
    Some(AcpClientConfig {
        name: None,
        command: preset.command.to_string(),
        args: preset
            .args
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        env: HashMap::new(),
        enabled: true,
        readonly: false,
        permission_mode: AcpClientPermissionMode::Ask,
    })
}

pub(crate) fn remote_command_for_builtin_client(client_id: &str) -> Option<String> {
    let preset = builtin_acp_client_preset(client_id)?;
    Some(render_shell_command(preset.command, preset.args))
}

pub(crate) fn remote_command_for_builtin_client_in_workspace(
    client_id: &str,
    workspace_path: &str,
) -> Option<String> {
    let command = remote_command_for_builtin_client(client_id)?;
    let workspace_path = workspace_path.trim();
    if workspace_path.is_empty() {
        return Some(command);
    }
    Some(format!(
        "cd {} && {}",
        shell_escape(workspace_path),
        command
    ))
}

fn render_shell_command(command: &str, args: &[&str]) -> String {
    std::iter::once(command)
        .chain(args.iter().copied())
        .map(shell_escape)
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_escape(value: &str) -> String {
    if value.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '-' | '_' | ':' | '=' | '@')
    }) {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_remote_builtin_command_for_opencode() {
        assert_eq!(
            remote_command_for_builtin_client("opencode").as_deref(),
            Some("opencode acp")
        );
    }

    #[test]
    fn builds_remote_builtin_command_for_npx_adapter() {
        assert_eq!(
            remote_command_for_builtin_client("codex").as_deref(),
            Some("npx --yes @zed-industries/codex-acp@latest")
        );
    }

    #[test]
    fn returns_default_config_for_builtin_client() {
        let config = default_config_for_builtin_client("claude-code").expect("builtin config");
        assert!(config.enabled);
        assert_eq!(config.command, "npx");
        assert_eq!(
            config.args,
            vec!["--yes", "@zed-industries/claude-code-acp@latest"]
        );
    }

    #[test]
    fn shell_escape_quotes_spaces() {
        assert_eq!(shell_escape("hello world"), "'hello world'");
    }

    #[test]
    fn builds_remote_builtin_command_in_workspace() {
        assert_eq!(
            remote_command_for_builtin_client_in_workspace("opencode", "/tmp/my repo").as_deref(),
            Some("cd '/tmp/my repo' && opencode acp")
        );
    }
}
