use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use bitfun_agent_runtime::sdk::{
    AgentSessionArchiveStateRequest, AgentSessionForkAtTurnRequest,
    AgentSessionForkBeforeTurnRequest, AgentSessionForkRequest, AgentSessionForkResult,
    AgentSessionHarnessProfileUpdateRequest, AgentSessionModeUpdateRequest,
    AgentSessionModelUpdateRequest, AgentSessionRenameRequest,
};
use serde::{Deserialize, Serialize};

macro_rules! empty_response {
    ($name:ident) => {
        #[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
        pub struct $name {}
    };
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "session/rename", response = RenameSessionResponse)]
pub struct RenameSessionMessage(pub AgentSessionRenameRequest);

empty_response!(RenameSessionResponse);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "session/setArchived", response = SetSessionArchivedResponse)]
pub struct SetSessionArchivedMessage(pub AgentSessionArchiveStateRequest);

empty_response!(SetSessionArchivedResponse);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "session/updateModel", response = UpdateSessionModelResponse)]
pub struct UpdateSessionModelMessage(pub AgentSessionModelUpdateRequest);

empty_response!(UpdateSessionModelResponse);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "session/updateMode", response = UpdateSessionModeResponse)]
pub struct UpdateSessionModeMessage(pub AgentSessionModeUpdateRequest);

empty_response!(UpdateSessionModeResponse);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(
    method = "session/updateHarnessProfile",
    response = UpdateSessionHarnessProfileResponse
)]
pub struct UpdateSessionHarnessProfileMessage(pub AgentSessionHarnessProfileUpdateRequest);

empty_response!(UpdateSessionHarnessProfileResponse);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "session/fork", response = ForkSessionResponse)]
pub struct ForkSessionMessage(pub AgentSessionForkRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "session/forkAtTurn", response = ForkSessionResponse)]
pub struct ForkSessionAtTurnMessage(pub AgentSessionForkAtTurnRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "session/forkBeforeTurn", response = ForkSessionResponse)]
pub struct ForkSessionBeforeTurnMessage(pub AgentSessionForkBeforeTurnRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ForkSessionResponse(pub AgentSessionForkResult);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "session/restore", response = RestoreSessionResponse)]
#[serde(rename_all = "camelCase")]
pub struct RestoreSessionMessage {
    pub workspace_path: String,
    pub session_id: String,
    #[serde(default)]
    pub include_internal: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

impl From<RestoreSessionMessage> for bitfun_agent_runtime::sdk::AgentSessionRestoreRequest {
    fn from(value: RestoreSessionMessage) -> Self {
        Self {
            workspace_path: value.workspace_path,
            session_id: value.session_id,
            include_internal: value.include_internal,
            remote_connection_id: value.remote_connection_id,
            remote_ssh_host: value.remote_ssh_host,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct RestoreSessionResponse {
    pub session: bitfun_agent_runtime::sdk::AgentSessionSummary,
    pub state: SessionRuntimeState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", ts(rename_all = "camelCase"))]
pub enum SessionRuntimeState {
    Idle,
    Processing {
        current_turn_id: String,
        phase: SessionProcessingPhase,
    },
    Error {
        error: String,
        recoverable: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum SessionProcessingPhase {
    Starting,
    Compacting,
    Thinking,
    Streaming,
    ToolCalling,
    ToolConfirming,
}

impl From<bitfun_agent_runtime::sdk::AgentSessionRestoreResult> for RestoreSessionResponse {
    fn from(value: bitfun_agent_runtime::sdk::AgentSessionRestoreResult) -> Self {
        Self {
            session: value.session,
            state: value.state.into(),
        }
    }
}

impl From<bitfun_agent_runtime::session_state::SessionState> for SessionRuntimeState {
    fn from(value: bitfun_agent_runtime::session_state::SessionState) -> Self {
        use bitfun_agent_runtime::session_state::SessionState;

        match value {
            SessionState::Idle => Self::Idle,
            SessionState::Processing {
                current_turn_id,
                phase,
            } => Self::Processing {
                current_turn_id,
                phase: phase.into(),
            },
            SessionState::Error { error, recoverable } => Self::Error { error, recoverable },
        }
    }
}

impl From<bitfun_agent_runtime::session_state::ProcessingPhase> for SessionProcessingPhase {
    fn from(value: bitfun_agent_runtime::session_state::ProcessingPhase) -> Self {
        use bitfun_agent_runtime::session_state::ProcessingPhase;

        match value {
            ProcessingPhase::Starting => Self::Starting,
            ProcessingPhase::Compacting => Self::Compacting,
            ProcessingPhase::Thinking => Self::Thinking,
            ProcessingPhase::Streaming => Self::Streaming,
            ProcessingPhase::ToolCalling => Self::ToolCalling,
            ProcessingPhase::ToolConfirming => Self::ToolConfirming,
        }
    }
}

impl From<RestoreSessionResponse> for bitfun_agent_runtime::sdk::AgentSessionRestoreResult {
    fn from(value: RestoreSessionResponse) -> Self {
        Self {
            session: value.session,
            state: value.state.into(),
        }
    }
}

impl From<SessionRuntimeState> for bitfun_agent_runtime::session_state::SessionState {
    fn from(value: SessionRuntimeState) -> Self {
        match value {
            SessionRuntimeState::Idle => Self::Idle,
            SessionRuntimeState::Processing {
                current_turn_id,
                phase,
            } => Self::Processing {
                current_turn_id,
                phase: phase.into(),
            },
            SessionRuntimeState::Error { error, recoverable } => Self::Error { error, recoverable },
        }
    }
}

impl From<SessionProcessingPhase> for bitfun_agent_runtime::session_state::ProcessingPhase {
    fn from(value: SessionProcessingPhase) -> Self {
        match value {
            SessionProcessingPhase::Starting => Self::Starting,
            SessionProcessingPhase::Compacting => Self::Compacting,
            SessionProcessingPhase::Thinking => Self::Thinking,
            SessionProcessingPhase::Streaming => Self::Streaming,
            SessionProcessingPhase::ToolCalling => Self::ToolCalling,
            SessionProcessingPhase::ToolConfirming => Self::ToolConfirming,
        }
    }
}
