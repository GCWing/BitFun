use std::sync::Arc;

use agent_client_protocol::schema::{
    AgentCapabilities, CancelNotification, CloseSessionRequest, CloseSessionResponse,
    Implementation, InitializeRequest, InitializeResponse, ListSessionsRequest,
    ListSessionsResponse, LoadSessionRequest, LoadSessionResponse, McpCapabilities,
    NewSessionRequest, NewSessionResponse, PromptCapabilities, PromptRequest, PromptResponse,
    ProtocolVersion, SessionCapabilities, SessionCloseCapabilities, SessionListCapabilities,
    SetSessionConfigOptionRequest, SetSessionConfigOptionResponse, SetSessionModeRequest,
    SetSessionModeResponse, SetSessionModelRequest, SetSessionModelResponse,
};
use agent_client_protocol::{Client, ConnectionTo, Error, Result};
use async_trait::async_trait;
use bitfun_agent_runtime::sdk::{AgentRuntime, PortErrorKind, RuntimeError};
use bitfun_core::product_runtime::CoreAgentRuntimeCompatibility;
use bitfun_core::util::errors::BitFunError;
use dashmap::mapref::entry::Entry;
use dashmap::DashMap;

use crate::server::{AcpRuntime, AcpServer};

mod content;
mod events;
mod mcp;
mod model;
mod prompt;
mod replay;
mod session;
mod thinking;

pub struct BitfunAcpRuntime {
    pub(crate) agent_runtime: AgentRuntime,
    pub(crate) compatibility: CoreAgentRuntimeCompatibility,
    pub(crate) sessions: DashMap<String, AcpSessionState>,
    pub(crate) connections: DashMap<String, ConnectionTo<Client>>,
    session_transitions: DashMap<String, ()>,
}

#[derive(Clone)]
pub(crate) struct AcpSessionState {
    pub(crate) acp_session_id: String,
    pub(crate) bitfun_session_id: String,
    pub(crate) cwd: String,
    pub(crate) mode_id: String,
    pub(crate) model_id: String,
    pub(crate) mcp_server_ids: Vec<String>,
    pub(crate) lifecycle: Arc<tokio::sync::Mutex<()>>,
}

pub(crate) struct AcpSessionTransition<'a> {
    session_id: String,
    transitions: &'a DashMap<String, ()>,
}

impl<'a> AcpSessionTransition<'a> {
    fn claim(transitions: &'a DashMap<String, ()>, session_id: &str) -> Result<Self> {
        match transitions.entry(session_id.to_string()) {
            Entry::Vacant(entry) => {
                entry.insert(());
                Ok(Self {
                    session_id: session_id.to_string(),
                    transitions,
                })
            }
            Entry::Occupied(_) => Err(Error::internal_error().data(serde_json::json!({
                "state": "session_transition_in_progress",
                "sessionId": session_id,
                "retryable": true,
                "recoveryAction": "Wait for the active session open or close request to finish, then retry the same request"
            }))),
        }
    }
}

impl Drop for AcpSessionTransition<'_> {
    fn drop(&mut self) {
        self.transitions.remove(&self.session_id);
    }
}

impl BitfunAcpRuntime {
    pub fn new(agent_runtime: AgentRuntime, compatibility: CoreAgentRuntimeCompatibility) -> Self {
        Self {
            agent_runtime,
            compatibility,
            sessions: DashMap::new(),
            connections: DashMap::new(),
            session_transitions: DashMap::new(),
        }
    }

    pub async fn serve_stdio(
        agent_runtime: AgentRuntime,
        compatibility: CoreAgentRuntimeCompatibility,
    ) -> Result<()> {
        AcpServer::new(Arc::new(Self::new(agent_runtime, compatibility)))
            .serve_stdio()
            .await
    }

    pub(crate) fn internal_error(error: impl std::fmt::Display) -> Error {
        Error::internal_error().data(serde_json::json!(error.to_string()))
    }

    pub(crate) fn cleanup_required_error(
        session_id: &str,
        stage: &str,
        cleanup_kinds: &[&str],
        core_session_created: bool,
        recovery_action: &str,
    ) -> Error {
        Error::internal_error().data(serde_json::json!({
            "message": "ACP session lifecycle operation failed and automatic cleanup did not complete",
            "sessionId": session_id,
            "stage": stage,
            "cleanupRequired": true,
            "cleanupKinds": cleanup_kinds,
            "coreSessionCreated": core_session_created,
            "recoveryAction": recovery_action
        }))
    }

    pub(crate) fn session_close_incomplete_error(
        session_id: &str,
        stage: &str,
        cause: impl std::fmt::Display,
        cleanup_kinds: &[&str],
    ) -> Error {
        Error::internal_error().data(serde_json::json!({
            "message": "ACP session close did not complete",
            "sessionId": session_id,
            "state": "session_close_incomplete",
            "stage": stage,
            "cause": cause.to_string(),
            "retryable": true,
            "acpSessionRetained": true,
            "persistedSessionPreserved": true,
            "cleanupRequired": !cleanup_kinds.is_empty(),
            "cleanupKinds": cleanup_kinds,
            "recoveryAction": "Retry session/close for the same sessionId; restart the ACP process only if the same failure continues"
        }))
    }

    pub(crate) fn runtime_error(error: RuntimeError) -> Error {
        match error {
            RuntimeError::Port(error) => match error.kind {
                PortErrorKind::InvalidRequest => Error::invalid_params().data(error.message),
                PortErrorKind::NotFound => Error::resource_not_found(None),
                _ => Self::internal_error(error.message),
            },
            other => Self::internal_error(other.into_message()),
        }
    }

    pub(crate) fn session_runtime_error(session_id: &str, error: RuntimeError) -> Error {
        match error {
            RuntimeError::Port(error) if error.kind == PortErrorKind::NotFound => {
                Error::resource_not_found(Some(session_id.to_string()))
            }
            other => Self::runtime_error(other),
        }
    }

    pub(crate) fn session_core_error(session_id: &str, error: BitFunError) -> Error {
        match error {
            BitFunError::NotFound(_) => Error::resource_not_found(Some(session_id.to_string())),
            BitFunError::Validation(message) => Error::invalid_params().data(message),
            other => Self::internal_error(other),
        }
    }

    pub(crate) async fn lock_active_session(
        &self,
        session_id: &str,
    ) -> Result<(AcpSessionState, tokio::sync::OwnedMutexGuard<()>)> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| Error::resource_not_found(Some(session_id.to_string())))?
            .clone();
        let lifecycle_guard = session.lifecycle.clone().lock_owned().await;
        let active_session = self
            .sessions
            .get(session_id)
            .filter(|active| Arc::ptr_eq(&active.lifecycle, &session.lifecycle))
            .ok_or_else(|| Error::resource_not_found(Some(session_id.to_string())))?
            .clone();
        Ok((active_session, lifecycle_guard))
    }

    pub(crate) fn claim_session_transition(
        &self,
        session_id: &str,
    ) -> Result<AcpSessionTransition<'_>> {
        AcpSessionTransition::claim(&self.session_transitions, session_id)
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
                        SessionCapabilities::new()
                            .list(SessionListCapabilities::new())
                            .close(SessionCloseCapabilities::new()),
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

    async fn close_session(&self, request: CloseSessionRequest) -> Result<CloseSessionResponse> {
        self.close_active_session(request).await
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

#[cfg(test)]
mod tests {
    use agent_client_protocol::schema::ErrorCode;
    use bitfun_agent_runtime::sdk::{PortError, PortErrorKind, RuntimeError};
    use dashmap::DashMap;

    use super::{AcpSessionTransition, BitfunAcpRuntime};

    #[test]
    fn invalid_runtime_request_remains_invalid_params_at_the_protocol_boundary() {
        let error = BitfunAcpRuntime::runtime_error(RuntimeError::Port(PortError::new(
            PortErrorKind::InvalidRequest,
            "unknown session mode",
        )));

        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert_eq!(error.data, Some(serde_json::json!("unknown session mode")));
    }

    #[test]
    fn missing_runtime_session_remains_resource_not_found_at_the_protocol_boundary() {
        let error = BitfunAcpRuntime::runtime_error(RuntimeError::Port(PortError::new(
            PortErrorKind::NotFound,
            "Session not found: session-404",
        )));

        assert_eq!(error.code, ErrorCode::ResourceNotFound);
        assert_eq!(error.data, None);
    }

    #[test]
    fn session_runtime_error_uses_the_requested_resource_id() {
        let error = BitfunAcpRuntime::session_runtime_error(
            "session-404",
            RuntimeError::Port(PortError::new(
                PortErrorKind::NotFound,
                "Session not found: session-404",
            )),
        );

        assert_eq!(error.code, ErrorCode::ResourceNotFound);
        assert_eq!(
            error.data,
            Some(serde_json::json!({ "uri": "session-404" }))
        );
    }

    #[test]
    fn backend_runtime_failure_stays_internal_at_the_protocol_boundary() {
        let error = BitfunAcpRuntime::runtime_error(RuntimeError::Port(PortError::new(
            PortErrorKind::Backend,
            "storage unavailable",
        )));

        assert_eq!(error.code, ErrorCode::InternalError);
        assert_eq!(error.data, Some(serde_json::json!("storage unavailable")));
    }

    #[test]
    fn session_transition_claim_rejects_overlap_and_releases_on_drop() {
        let transitions = DashMap::new();
        let first = AcpSessionTransition::claim(&transitions, "session-1")
            .expect("first transition should claim the session");

        let overlapping = match AcpSessionTransition::claim(&transitions, "session-1") {
            Ok(_) => panic!("overlapping load or close must fail before side effects"),
            Err(error) => error,
        };
        assert_eq!(overlapping.code, ErrorCode::InternalError);
        assert_eq!(
            overlapping.data.as_ref().and_then(|data| data.get("state")),
            Some(&serde_json::json!("session_transition_in_progress"))
        );
        assert_eq!(
            overlapping
                .data
                .as_ref()
                .and_then(|data| data.get("retryable")),
            Some(&serde_json::json!(true))
        );

        drop(first);
        AcpSessionTransition::claim(&transitions, "session-1")
            .expect("the claim should be reusable after the transition ends");
    }

    #[test]
    fn incomplete_close_error_exposes_retry_and_ownership_state() {
        let error = BitfunAcpRuntime::session_close_incomplete_error(
            "session-1",
            "ephemeral MCP cleanup",
            "server stop failed",
            &["ephemeralMcp"],
        );
        let data = error.data.expect("close error should carry recovery data");

        assert_eq!(data["state"], "session_close_incomplete");
        assert_eq!(data["retryable"], true);
        assert_eq!(data["acpSessionRetained"], true);
        assert_eq!(data["persistedSessionPreserved"], true);
        assert_eq!(data["cleanupKinds"], serde_json::json!(["ephemeralMcp"]));
    }
}
