use std::collections::HashSet;
use std::sync::RwLock;

use bitfun_runtime_ports::{
    PermissionDecision, PermissionPort, PermissionRequest, PortResult, RuntimeServiceCapability,
    RuntimeServicePort,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CliApprovalPolicy {
    Ask,
    Reject,
    Auto,
}

#[derive(Debug, Default)]
pub(crate) struct CliApprovalController {
    allowed_tools: RwLock<HashSet<String>>,
}

impl CliApprovalController {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn allow_always(&self, tool_name: &str) {
        let tool_name = normalize_tool_name(tool_name);
        if tool_name.is_empty() {
            return;
        }

        self.allowed_tools
            .write()
            .expect("CLI approval controller lock poisoned")
            .insert(tool_name);
    }

    pub(crate) fn is_allowed(&self, tool_name: &str) -> bool {
        let tool_name = normalize_tool_name(tool_name);
        !tool_name.is_empty()
            && self
                .allowed_tools
                .read()
                .expect("CLI approval controller lock poisoned")
                .contains(&tool_name)
    }
}

fn normalize_tool_name(tool_name: &str) -> String {
    tool_name.trim().to_ascii_lowercase()
}

#[derive(Debug)]
pub(crate) struct CliPermissionService {
    policy: CliApprovalPolicy,
}

impl CliPermissionService {
    pub(crate) const fn new(policy: CliApprovalPolicy) -> Self {
        Self { policy }
    }
}

impl RuntimeServicePort for CliPermissionService {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::Permission
    }
}

#[async_trait::async_trait]
impl PermissionPort for CliPermissionService {
    async fn request_permission(
        &self,
        request: PermissionRequest,
    ) -> PortResult<PermissionDecision> {
        Ok(match self.policy {
            CliApprovalPolicy::Auto => PermissionDecision::Allow,
            CliApprovalPolicy::Reject => PermissionDecision::Deny {
                reason: format!(
                    "non-interactive permission rejected: {}:{}",
                    request.scope, request.action
                ),
            },
            CliApprovalPolicy::Ask => PermissionDecision::Deny {
                reason: format!(
                    "interactive approval required: {}:{}",
                    request.scope, request.action
                ),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{CliApprovalController, CliApprovalPolicy, CliPermissionService};
    use bitfun_runtime_ports::{PermissionDecision, PermissionPort, PermissionRequest};

    fn request() -> PermissionRequest {
        PermissionRequest {
            scope: "tool".to_string(),
            action: "run_terminal_cmd".to_string(),
            metadata: serde_json::Map::new(),
        }
    }

    #[tokio::test]
    async fn non_interactive_permission_policy_is_invocation_scoped() {
        let reject = CliPermissionService::new(CliApprovalPolicy::Reject);
        assert!(matches!(
            reject
                .request_permission(request())
                .await
                .expect("decision"),
            PermissionDecision::Deny { .. }
        ));

        let auto = CliPermissionService::new(CliApprovalPolicy::Auto);
        assert_eq!(
            auto.request_permission(request()).await.expect("decision"),
            PermissionDecision::Allow
        );
    }

    #[tokio::test]
    async fn interactive_policy_never_silently_approves() {
        let service = CliPermissionService::new(CliApprovalPolicy::Ask);

        let decision = service
            .request_permission(request())
            .await
            .expect("decision");

        assert!(
            matches!(decision, PermissionDecision::Deny { reason } if reason.contains("interactive"))
        );
    }

    #[test]
    fn allow_always_is_scoped_to_one_controller_and_tool_pattern() {
        let controller = CliApprovalController::new();
        assert!(!controller.is_allowed("run_terminal_cmd"));

        controller.allow_always("run_terminal_cmd");

        assert!(controller.is_allowed("run_terminal_cmd"));
        assert!(controller.is_allowed("RUN_TERMINAL_CMD"));
        assert!(!controller.is_allowed("write_file"));
        assert!(
            !CliApprovalController::new().is_allowed("run_terminal_cmd"),
            "approval must not survive a runtime context"
        );
    }
}
