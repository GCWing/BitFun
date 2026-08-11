//! Product Host policy and asynchronous operation-lifecycle injection points.

use bitfun_app_server_protocol::error::AppServerErrorKind;

/// Host-owned authorization policy applied before requests reach Runtime or
/// management owners.
///
/// Embedded single-client hosts may omit this policy. Shared and remote hosts
/// must inject one that owns their workspace, execution, method, and
/// capability scope.
pub trait AppServerHostPolicy: Send + Sync {
    fn allows_method(&self, method: &str) -> bool;

    /// Authorize the request before it is decoded by a typed handler.
    ///
    /// This is the connection-wide preflight stage. Implementations should
    /// keep checks here independent of Runtime-owned state that a typed
    /// handler may need to resolve first, such as a Session workspace
    /// binding.
    fn authorize_preflight(
        &self,
        method: &str,
        request: &serde_json::Value,
    ) -> Result<(), AppServerHostPolicyError> {
        self.authorize_request(method, request)
    }

    /// Authorize a typed request after any owner-aware state has been
    /// resolved and registered by its handler.
    fn authorize_request(
        &self,
        method: &str,
        request: &serde_json::Value,
    ) -> Result<(), AppServerHostPolicyError>;

    fn allows_capability(&self, capability: &str) -> bool;

    fn allows_external_source_workspace(&self, workspace_path: &str) -> bool;

    fn register_session_binding(
        &self,
        session_id: &str,
        binding: &bitfun_runtime_ports::AgentSessionWorkspaceBinding,
    ) -> Result<(), AppServerHostPolicyError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppServerHostPolicyError {
    pub kind: AppServerErrorKind,
    pub message: String,
    pub capability: Option<String>,
}

impl AppServerHostPolicyError {
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            kind: AppServerErrorKind::InvalidRequest,
            message: message.into(),
            capability: None,
        }
    }

    pub fn unsupported(capability: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: AppServerErrorKind::Unsupported,
            message: message.into(),
            capability: Some(capability.into()),
        }
    }
}

/// Host-owned observer for asynchronous operations that outlive a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppServerOperationKind {
    DialogTurn,
    ContextCompaction,
}

pub trait AppServerOperationObserver: Send + Sync {
    fn operation_started(&self, session_id: &str, turn_id: &str, kind: AppServerOperationKind);
    fn operation_admitted(&self, session_id: &str, turn_id: &str, kind: AppServerOperationKind);
    fn operation_rejected(&self, session_id: &str, turn_id: &str, kind: AppServerOperationKind);
}
