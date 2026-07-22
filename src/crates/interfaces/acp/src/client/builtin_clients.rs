use std::collections::HashMap;

use super::config::{AcpClientConfig, AcpClientConfigFile, AcpClientPermissionMode};

const CLAUDE_ACP_PACKAGE: &str = "@agentclientprotocol/claude-agent-acp";
const CLAUDE_ACP_ARGS: &[&str] = &["--yes", "@agentclientprotocol/claude-agent-acp@latest"];
const LEGACY_CLAUDE_ACP_ARGS: &[&str] = &["--yes", "@zed-industries/claude-code-acp@latest"];
const CODEX_ACP_PACKAGE: &str = "@agentclientprotocol/codex-acp";
const CODEX_ACP_ARGS: &[&str] = &["--yes", "@agentclientprotocol/codex-acp@latest"];
const LEGACY_CODEX_ACP_ARGS: &[&str] = &["--yes", "@zed-industries/codex-acp@latest"];

pub(crate) struct BuiltinAcpClientPreset {
    pub(crate) id: &'static str,
    pub(crate) command: &'static str,
    pub(crate) args: &'static [&'static str],
    pub(crate) tool_command: &'static str,
    /// npm package BitFun can install on the user's behalf. `None` means the
    /// agent is user-managed (BitFun only provides the integration, the user
    /// installs the CLI themselves) — the UI then shows no one-click installer.
    pub(crate) install_package: Option<&'static str>,
    pub(crate) adapter_package: Option<&'static str>,
    pub(crate) adapter_bin: Option<&'static str>,
}

const BUILTIN_ACP_CLIENT_PRESETS: &[BuiltinAcpClientPreset] = &[
    BuiltinAcpClientPreset {
        id: "opencode",
        command: "opencode",
        args: &["acp"],
        tool_command: "opencode",
        install_package: Some("opencode-ai"),
        adapter_package: None,
        adapter_bin: None,
    },
    // Oh My Pi (omp) — a terminal coding agent that speaks ACP natively via
    // `omp acp` (no adapter needed, like opencode). User-managed: omp targets
    // the bun runtime (installed via `bun install -g @oh-my-pi/pi-coding-agent`
    // or `curl -fsSL https://omp.sh/install | sh`), which BitFun's npm-based
    // installer cannot provide — so install_package is None and BitFun only
    // detects `omp` on PATH and launches it. https://github.com/can1357/oh-my-pi
    BuiltinAcpClientPreset {
        id: "omp",
        command: "omp",
        args: &["acp"],
        tool_command: "omp",
        install_package: None,
        adapter_package: None,
        adapter_bin: None,
    },
    BuiltinAcpClientPreset {
        id: "claude-code",
        command: "npx",
        args: CLAUDE_ACP_ARGS,
        tool_command: "claude",
        install_package: Some("@anthropic-ai/claude-code"),
        adapter_package: Some(CLAUDE_ACP_PACKAGE),
        adapter_bin: Some("claude-agent-acp"),
    },
    BuiltinAcpClientPreset {
        id: "codex",
        command: "npx",
        args: CODEX_ACP_ARGS,
        tool_command: "codex",
        install_package: Some("@openai/codex"),
        adapter_package: Some(CODEX_ACP_PACKAGE),
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

pub(crate) fn migrate_legacy_builtin_client_configs(config_file: &mut AcpClientConfigFile) {
    for (client_id, legacy_args, current_args) in [
        ("claude-code", LEGACY_CLAUDE_ACP_ARGS, CLAUDE_ACP_ARGS),
        ("codex", LEGACY_CODEX_ACP_ARGS, CODEX_ACP_ARGS),
    ] {
        let Some(config) = config_file.acp_clients.get_mut(client_id) else {
            continue;
        };
        let uses_legacy_preset = config.command == "npx"
            && config
                .args
                .iter()
                .map(String::as_str)
                .eq(legacy_args.iter().copied());
        if uses_legacy_preset {
            config.args = current_args
                .iter()
                .map(|value| (*value).to_string())
                .collect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_default_config_for_builtin_client() {
        let config = default_config_for_builtin_client("claude-code").expect("builtin config");
        assert!(config.enabled);
        assert_eq!(config.command, "npx");
        assert_eq!(
            config.args,
            vec!["--yes", "@agentclientprotocol/claude-agent-acp@latest"]
        );
    }

    #[test]
    fn omp_is_a_native_acp_preset() {
        let preset = builtin_acp_client_preset("omp").expect("omp preset registered");
        assert_eq!(preset.command, "omp");
        assert_eq!(preset.args, &["acp"]);
        assert_eq!(preset.tool_command, "omp");
        // Native ACP — no adapter package/bin, like opencode.
        assert!(preset.adapter_package.is_none());
        assert!(preset.adapter_bin.is_none());
        // User-managed: BitFun provides no installer for omp.
        assert!(preset.install_package.is_none());

        let config = default_config_for_builtin_client("omp").expect("omp config");
        assert!(config.enabled);
        assert_eq!(config.command, "omp");
        assert_eq!(config.args, vec!["acp"]);
    }

    #[test]
    fn migrates_only_exact_legacy_builtin_commands() {
        let mut config_file = AcpClientConfigFile {
            acp_clients: HashMap::from([
                (
                    "claude-code".to_string(),
                    AcpClientConfig {
                        name: Some("Claude Code".to_string()),
                        command: "npx".to_string(),
                        args: LEGACY_CLAUDE_ACP_ARGS
                            .iter()
                            .map(|value| (*value).to_string())
                            .collect(),
                        env: HashMap::new(),
                        enabled: true,
                        readonly: false,
                        permission_mode: AcpClientPermissionMode::Ask,
                    },
                ),
                (
                    "codex".to_string(),
                    AcpClientConfig {
                        name: Some("Codex".to_string()),
                        command: "npx".to_string(),
                        args: LEGACY_CODEX_ACP_ARGS
                            .iter()
                            .map(|value| (*value).to_string())
                            .collect(),
                        env: HashMap::new(),
                        enabled: true,
                        readonly: false,
                        permission_mode: AcpClientPermissionMode::Ask,
                    },
                ),
                (
                    "custom-codex".to_string(),
                    AcpClientConfig {
                        name: Some("Pinned Codex".to_string()),
                        command: "npx".to_string(),
                        args: vec![
                            "--yes".to_string(),
                            "@zed-industries/codex-acp@0.16.0".to_string(),
                        ],
                        env: HashMap::new(),
                        enabled: true,
                        readonly: false,
                        permission_mode: AcpClientPermissionMode::Ask,
                    },
                ),
            ]),
        };

        migrate_legacy_builtin_client_configs(&mut config_file);

        assert_eq!(
            config_file.acp_clients["claude-code"].args,
            vec!["--yes", "@agentclientprotocol/claude-agent-acp@latest"]
        );
        assert_eq!(
            config_file.acp_clients["codex"].args,
            vec!["--yes", "@agentclientprotocol/codex-acp@latest"]
        );
        assert_eq!(
            config_file.acp_clients["custom-codex"].args,
            vec!["--yes", "@zed-industries/codex-acp@0.16.0"]
        );
    }
}
