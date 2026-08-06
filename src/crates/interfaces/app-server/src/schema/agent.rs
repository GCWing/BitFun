use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use bitfun_agent_runtime::sdk::{
    AgentDialogTurnExecution, AgentDialogTurnRequest, AgentInputAttachment, AgentRunHandle,
    AgentRunRequest, AgentSessionCreateRequest, AgentSessionCreateResult,
    AgentSessionDeleteRequest, AgentSessionListRequest, AgentSessionSummary,
    AgentSubmissionRequest, AgentSubmissionResult, AgentSubmissionSource,
    AgentTurnCancellationRequest, AgentTurnCancellationResult, DialogSubmissionPolicy,
    DialogSubmitOutcome, SessionSelector,
};
use serde::{Deserialize, Serialize};

/// `agent/createSession` request body (wraps the port request type).
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[request(method = "agent/createSession", response = CreateSessionResponse)]
pub struct CreateSessionMessage(pub AgentSessionCreateRequest);

/// `agent/createSession` response body (wraps the port result type).
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CreateSessionResponse(pub AgentSessionCreateResult);

/// `agent/listSessions` request body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[request(method = "agent/listSessions", response = ListSessionsResponse)]
pub struct ListSessionsMessage(pub AgentSessionListRequest);

/// `agent/listSessions` response body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ListSessionsResponse {
    pub sessions: Vec<AgentSessionSummary>,
}

/// `agent/deleteSession` request body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[request(method = "agent/deleteSession", response = DeleteSessionResponse)]
pub struct DeleteSessionMessage(pub AgentSessionDeleteRequest);

/// `agent/deleteSession` response body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct DeleteSessionResponse {}

/// `agent/submitTurn` request body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[request(method = "agent/submitTurn", response = SubmitTurnResponse)]
pub struct SubmitTurnMessage(pub AgentSubmissionRequest);

/// `agent/submitTurn` response body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SubmitTurnResponse(pub AgentSubmissionResult);

/// `agent/submitDialogTurn` request body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[request(method = "agent/submitDialogTurn", response = SubmitDialogTurnResponse)]
pub struct SubmitDialogTurnMessage(pub SubmitDialogTurnBody);

/// Wire form of [`AgentDialogTurnRequest`] with an optional `policy`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SubmitDialogTurnBody {
    pub session_id: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "AgentDialogTurnExecution::is_standard")]
    pub execution: AgentDialogTurnExecution,
    pub agent_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<DialogSubmissionPolicy>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<AgentInputAttachment>,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

impl SubmitDialogTurnBody {
    /// Build the runtime request, defaulting `policy` to the desktop UI source.
    pub fn to_request(self) -> AgentDialogTurnRequest {
        AgentDialogTurnRequest {
            session_id: self.session_id,
            message: self.message,
            original_message: self.original_message,
            turn_id: self.turn_id,
            execution: self.execution,
            agent_type: self.agent_type,
            workspace_path: self.workspace_path,
            remote_connection_id: self.remote_connection_id,
            remote_ssh_host: self.remote_ssh_host,
            policy: self.policy.unwrap_or_else(|| {
                DialogSubmissionPolicy::for_source(AgentSubmissionSource::DesktopUi)
            }),
            reply_route: None,
            prepended_reminders: Vec::new(),
            attachments: self.attachments,
            metadata: self.metadata,
        }
    }
}

/// `agent/submitDialogTurn` response body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum SubmitDialogTurnResponse {
    Started { session_id: String, turn_id: String },
    Queued { session_id: String, turn_id: String },
}

impl SubmitDialogTurnResponse {
    pub fn from_outcome(outcome: DialogSubmitOutcome) -> Self {
        match outcome {
            DialogSubmitOutcome::Started {
                session_id,
                turn_id,
            } => Self::Started {
                session_id,
                turn_id,
            },
            DialogSubmitOutcome::Queued {
                session_id,
                turn_id,
            } => Self::Queued {
                session_id,
                turn_id,
            },
        }
    }
}

/// `agent/cancelTurn` request body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[request(method = "agent/cancelTurn", response = CancelTurnResponse)]
pub struct CancelTurnMessage(pub AgentTurnCancellationRequest);

/// `agent/cancelTurn` response body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CancelTurnResponse(pub AgentTurnCancellationResult);

/// `agent/run` request body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "agent/run", response = RunResponse)]
pub struct RunMessage {
    pub session: RunSessionSpec,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<AgentSubmissionSource>,
}

/// Wire form of [`SessionSelector`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum RunSessionSpec {
    Existing {
        session_id: String,
    },
    Create {
        session_name: String,
        agent_type: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        workspace_path: Option<String>,
    },
}

impl RunSessionSpec {
    pub fn to_selector(&self) -> SessionSelector {
        match self {
            RunSessionSpec::Existing { session_id } => SessionSelector::existing(session_id),
            RunSessionSpec::Create {
                session_name,
                agent_type,
                workspace_path,
            } => SessionSelector::create(session_name, agent_type, workspace_path.clone()),
        }
    }
}

/// `agent/run` response body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct RunResponse {
    pub session_id: String,
    pub turn_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    #[serde(default)]
    pub accepted: bool,
}

impl RunResponse {
    pub fn from_handle(handle: AgentRunHandle) -> Self {
        Self {
            session_id: handle.session_id,
            turn_id: handle.turn_id,
            agent_type: handle.agent_type,
            accepted: handle.accepted,
        }
    }
}

impl RunMessage {
    pub fn to_run_request(&self) -> AgentRunRequest {
        let mut request = AgentRunRequest::new(self.session.to_selector(), &self.message);
        if let Some(turn_id) = &self.turn_id {
            request = request.with_turn_id(turn_id);
        }
        if let Some(source) = self.source {
            request = request.with_source(source);
        }
        request
    }
}
