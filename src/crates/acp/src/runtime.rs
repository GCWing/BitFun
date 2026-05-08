use std::sync::Arc;

use agent_client_protocol::schema::{
    AgentCapabilities, CancelNotification, Implementation, InitializeRequest, InitializeResponse,
    ListSessionsRequest, ListSessionsResponse, LoadSessionRequest, LoadSessionResponse,
    McpCapabilities, NewSessionRequest, NewSessionResponse, PromptCapabilities, PromptRequest,
    PromptResponse, ProtocolVersion, SessionCapabilities, SessionListCapabilities,
    SetSessionConfigOptionRequest, SetSessionConfigOptionResponse, SetSessionModeRequest,
    SetSessionModeResponse, SetSessionModelRequest, SetSessionModelResponse,
};
use agent_client_protocol::{Client, ConnectionTo, Error, Result};
use async_trait::async_trait;
use bitfun_core::agentic::system::AgenticSystem;
use dashmap::DashMap;

use crate::server::{AcpRuntime, AcpServer};

mod content;
mod events;
mod mcp;
mod model;
mod prompt;
mod session;
mod thinking;

pub struct BitfunAcpRuntime {
    pub(crate) agentic_system: AgenticSystem,
    pub(crate) sessions: DashMap<String, AcpSessionState>,
    pub(crate) connections: DashMap<String, ConnectionTo<Client>>,
}

#[derive(Clone)]
pub(crate) struct AcpSessionState {
    pub(crate) acp_session_id: String,
    pub(crate) bitfun_session_id: String,
    pub(crate) cwd: String,
    pub(crate) mode_id: String,
    pub(crate) model_id: String,
    #[allow(dead_code)]
    pub(crate) mcp_server_ids: Vec<String>,
}

impl BitfunAcpRuntime {
    pub fn new(agentic_system: AgenticSystem) -> Self {
        Self {
            agentic_system,
            sessions: DashMap::new(),
            connections: DashMap::new(),
        }
    }

    pub async fn serve_stdio(agentic_system: AgenticSystem) -> Result<()> {
        AcpServer::new(Arc::new(Self::new(agentic_system)))
            .serve_stdio()
            .await
    }

    pub(crate) fn internal_error(error: impl std::fmt::Display) -> Error {
        Error::internal_error().data(serde_json::json!(error.to_string()))
    }
}

#[async_trait]
impl AcpRuntime for BitfunAcpRuntime {
    async fn initialize(&self, _request: InitializeRequest) -> Result<InitializeResponse> {
        Ok(InitializeResponse::new(ProtocolVersion::V1)
            .agent_capabilities(
                AgentCapabilities::new()
                    .load_session(true)
                    .prompt_capabilities(
                        PromptCapabilities::new().image(true).embedded_context(true),
                    )
                    .mcp_capabilities(McpCapabilities::new().http(true))
                    .session_capabilities(
                        SessionCapabilities::new().list(SessionListCapabilities::new()),
                    ),
            )
            .agent_info(
                Implementation::new("bitfun-acp", env!("CARGO_PKG_VERSION")).title("BitFun"),
            ))
    }

    async fn new_session(
        &self,
        request: NewSessionRequest,
        connection: ConnectionTo<Client>,
    ) -> Result<NewSessionResponse> {
        self.create_session(request, connection).await
    }

    async fn load_session(
        &self,
        request: LoadSessionRequest,
        connection: ConnectionTo<Client>,
    ) -> Result<LoadSessionResponse> {
        self.restore_session(request, connection).await
    }

    async fn list_sessions(&self, request: ListSessionsRequest) -> Result<ListSessionsResponse> {
        self.list_sessions_for_cwd(request).await
    }

    async fn prompt(&self, request: PromptRequest) -> Result<PromptResponse> {
        self.run_prompt(request).await
    }

    async fn cancel(&self, notification: CancelNotification) -> Result<()> {
        self.cancel_prompt(notification).await
    }

    async fn set_session_mode(
        &self,
        request: SetSessionModeRequest,
    ) -> Result<SetSessionModeResponse> {
        self.update_session_mode(request).await
    }

    async fn set_session_config_option(
        &self,
        request: SetSessionConfigOptionRequest,
    ) -> Result<SetSessionConfigOptionResponse> {
        self.update_session_config_option(request).await
    }

    async fn set_session_model(
        &self,
        request: SetSessionModelRequest,
    ) -> Result<SetSessionModelResponse> {
        self.update_session_model(request).await
    }
}
