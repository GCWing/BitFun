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
