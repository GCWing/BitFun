use super::*;

pub(crate) use bitfun_core::service::mcp::{
    McpManagementAction as McpServerAction, McpManagementList as McpServerList,
    McpManagementMutation as McpServerMutation, McpManagementServer as McpServerSummary,
    McpManagementTransport as McpTransport,
};

#[derive(Debug, Clone)]
pub(crate) struct ExternalMcpDecisionRequest {
    pub candidate_id: String,
    pub decision_key: String,
    pub approved: bool,
    pub expected_mcp_generation: u64,
    pub expected_preference_revision: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct McpConflictChoiceRequest {
    pub conflict_key: String,
    pub candidate_id: String,
    pub approve_external: bool,
    pub expected_mcp_generation: u64,
    pub expected_preference_revision: u64,
}

pub(crate) struct McpProvider {
    owner: Option<bitfun_core::service::mcp::McpManagementService>,
    unavailable_reason: &'static str,
}

impl McpProvider {
    pub(crate) fn new(service: Option<Arc<bitfun_core::service::mcp::MCPService>>) -> Self {
        Self {
            owner: service.map(bitfun_core::service::mcp::McpManagementService::new),
            unavailable_reason: "MCP management is unavailable",
        }
    }

    pub(crate) fn shared_runtime_unavailable() -> Self {
        Self {
            owner: None,
            unavailable_reason: "MCP management is unavailable in Shared TUI; exit Shared clients, manage MCP in Embedded mode, and restart the Shared Runtime",
        }
    }

    fn owner(&self) -> ManagementResult<&bitfun_core::service::mcp::McpManagementService> {
        self.owner
            .as_ref()
            .ok_or_else(|| ManagementError::unsupported(self.unavailable_reason))
    }

    pub(crate) async fn list(&self, scope: &ManagementScope) -> ManagementResult<McpServerList> {
        let workspace = scope.local_workspace("MCP management")?;
        self.owner()?
            .list(workspace)
            .await
            .map_err(map_mcp_management_error)
    }

    pub(crate) async fn toggle(
        &self,
        scope: &ManagementScope,
        server_id: &str,
    ) -> ManagementResult<()> {
        scope.local_workspace("MCP management")?;
        self.owner()?
            .toggle(server_id)
            .await
            .map_err(map_mcp_management_error)
    }

    pub(crate) async fn add(
        &self,
        scope: &ManagementScope,
        name: &str,
        mutation: McpServerMutation,
    ) -> ManagementResult<()> {
        scope.local_workspace("MCP management")?;
        self.owner()?
            .add(name, mutation)
            .await
            .map_err(map_mcp_management_error)
    }

    pub(crate) async fn delete(
        &self,
        scope: &ManagementScope,
        server_id: &str,
    ) -> ManagementResult<()> {
        scope.local_workspace("MCP management")?;
        self.owner()?
            .delete(server_id)
            .await
            .map_err(map_mcp_management_error)
    }

    pub(crate) async fn decide_external(
        &self,
        scope: &ManagementScope,
        request: ExternalMcpDecisionRequest,
    ) -> ManagementResult<()> {
        let workspace = scope.local_workspace("MCP management")?;
        bitfun_core::external_sources::set_external_mcp_server_decision(
            Some(workspace),
            &request.candidate_id,
            &request.decision_key,
            request.approved,
            request.expected_mcp_generation,
            request.expected_preference_revision,
        )
        .await
        .map(|_| ())
        .map_err(map_external_string_error)
    }

    pub(crate) async fn choose_conflict(
        &self,
        scope: &ManagementScope,
        request: McpConflictChoiceRequest,
    ) -> ManagementResult<()> {
        let workspace = scope.local_workspace("MCP management")?;
        bitfun_core::external_sources::choose_external_mcp_conflict(
            Some(workspace),
            &request.conflict_key,
            &request.candidate_id,
            request.approve_external,
            request.expected_mcp_generation,
            request.expected_preference_revision,
        )
        .await
        .map(|_| ())
        .map_err(map_external_string_error)
    }
}

fn map_mcp_management_error(
    error: bitfun_core::service::mcp::McpManagementError,
) -> ManagementError {
    use bitfun_core::service::mcp::McpManagementErrorKind as Kind;
    match error.kind {
        Kind::InvalidRequest => ManagementError::invalid_request(error.message),
        Kind::Unavailable => ManagementError::unsupported(error.message),
        Kind::Internal => ManagementError::internal(error.message),
    }
}
