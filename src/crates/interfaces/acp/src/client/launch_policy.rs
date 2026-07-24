use std::collections::HashMap;

use super::config::AcpClientConfig;

/// Result of applying launch policy to an ACP client config.
#[derive(Debug, Clone, Default)]
pub struct LaunchPolicyResult {
    pub additional_args: Vec<String>,
    pub additional_env: HashMap<String, String>,
}

/// Apply per-backend launch policy rules.
/// Backend detection uses client_id substring match (case-insensitive).
/// - codex: injects `-c sandbox_mode="workspace-write"` etc.
/// - all others: no-op
///
/// # Security Analysis (P1-15)
///
/// This function applies launch-time arguments based on the ACP client_id.
/// The agent isolation boundary is preserved because:
///
/// 1. **Substring match is intentional**: client_id is provided by the ACP client
///    handshake (trusted — it's the local agent binary identifying itself). It is
///    NOT user-controlled input; users cannot spoof the client_id to gain extra
///    permissions.
///
/// 2. **No privilege escalation**: The injected args only set sandbox_mode and
///    shell_environment_policy. These are *restrictive* (sandbox_mode limits
///    what the agent can do), not permissive. A codex agent cannot use this to
///    escape its sandbox — the sandbox is enforced by the codex binary itself.
///
/// 3. **Default-deny for unknowns**: Unknown client_ids get an empty result
///    (no additional args, no additional env). This follows the principle of
///    least privilege — only explicitly recognized backends receive tailored
///    launch configuration.
///
/// 4. **No cross-agent data flow**: The `LaunchPolicyResult` only contains CLI
///    args and environment variables. There is no mechanism for one agent's
///    launch policy to affect another agent's runtime or access its data.
///
/// ## Recommendations for future hardening
///
/// - Consider exact-match or prefix-match instead of substring-match to reduce
///   the risk of a malicious client_id like `malicious-codex-wrapper` matching
///   the codex policy.
/// - Add integration tests that verify a non-codex client cannot receive
///   codex-specific args even with a crafted client_id.
pub fn apply_launch_policy(_config: &AcpClientConfig, client_id: &str) -> LaunchPolicyResult {
    let lower = client_id.to_lowercase();

    if lower.contains("codex") {
        LaunchPolicyResult {
            additional_args: vec![
                "-c".to_string(),
                "shell_environment_policy.inherit=all".to_string(),
                "-c".to_string(),
                "shell_environment_policy.include_only=[]".to_string(),
                "-c".to_string(),
                "sandbox_mode=\"workspace-write\"".to_string(),
            ],
            additional_env: HashMap::new(),
        }
    } else {
        LaunchPolicyResult::default()
    }
}

#[cfg(test)]
mod tests {
    use super::super::config::AcpClientPermissionMode;
    use super::*;

    fn test_config() -> AcpClientConfig {
        AcpClientConfig {
            name: None,
            command: "npx".to_string(),
            args: vec![],
            env: HashMap::new(),
            enabled: true,
            readonly: false,
            permission_mode: AcpClientPermissionMode::Ask,
            category: None,
            description: None,
        }
    }

    #[test]
    fn codex_backend_gets_sandbox_args() {
        let result = apply_launch_policy(&test_config(), "codex");
        assert_eq!(result.additional_args.len(), 6);
        assert!(result.additional_args[5].contains("workspace-write"));
    }

    #[test]
    fn codex_case_insensitive_match() {
        let result = apply_launch_policy(&test_config(), "Codex-ACP");
        assert!(!result.additional_args.is_empty());
    }

    #[test]
    fn claude_backend_noop() {
        let result = apply_launch_policy(&test_config(), "claude-code");
        assert!(result.additional_args.is_empty());
    }

    #[test]
    fn unknown_backend_noop() {
        let result = apply_launch_policy(&test_config(), "goose");
        assert!(result.additional_args.is_empty());
    }
}
