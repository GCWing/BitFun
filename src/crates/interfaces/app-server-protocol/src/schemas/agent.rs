//! Agent-domain App Server wire schemas.

use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use bitfun_product_domains::tool_permissions::{PermissionReply, PermissionRequest};
use bitfun_runtime_ports::{
    AgentDialogSteerRequest, AgentDialogTurnRequest, AgentSessionCreateRequest,
    AgentSessionCreateResult, AgentSessionDeleteRequest, AgentSessionListRequest,
    AgentSessionSummary, AgentTurnCancellationRequest, AgentTurnCancellationResult,
    AgentUserShellCommandRequest, AgentUserShellCommandResult, DialogSubmitOutcome,
};
use serde::{Deserialize, Serialize};

macro_rules! unit_response {
    ($name:ident) => {
        #[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
        pub struct $name {}
    };
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "agent/listModes", response = ListAgentModesResponse)]
#[serde(rename_all = "camelCase")]
pub struct ListAgentModesRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub include_external: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct ListAgentModesResponse {
    pub modes: Vec<AgentModeSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentModeSummary {
    pub id: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default)]
    pub is_external: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "agent/listSessions", response = ListSessionsResponse)]
pub struct ListSessionsRequest(pub AgentSessionListRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct ListSessionsResponse {
    pub sessions: Vec<AgentSessionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "agent/steerTurn", response = SteerTurnResponse)]
pub struct SteerTurnRequest(pub AgentDialogSteerRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct SteerTurnResponse {
    pub steering_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "agent/runUserShellCommand", response = RunUserShellCommandResponse)]
pub struct RunUserShellCommandRequest(pub AgentUserShellCommandRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct RunUserShellCommandResponse(pub AgentUserShellCommandResult);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "agent/submitUserAnswers", response = SubmitUserAnswersResponse)]
pub struct SubmitUserAnswersRequest {
    pub tool_id: String,
    pub answers: serde_json::Value,
}

unit_response!(SubmitUserAnswersResponse);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "agent/createSession", response = CreateSessionResponse)]
pub struct CreateSessionRequest(pub AgentSessionCreateRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct CreateSessionResponse(pub AgentSessionCreateResult);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "agent/deleteSession", response = DeleteSessionResponse)]
pub struct DeleteSessionRequest(pub AgentSessionDeleteRequest);

unit_response!(DeleteSessionResponse);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "agent/submitDialogTurn", response = SubmitDialogTurnResponse)]
pub struct SubmitDialogTurnRequest(pub AgentDialogTurnRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum SubmitDialogTurnResponse {
    Started { session_id: String, turn_id: String },
    Queued { session_id: String, turn_id: String },
}

impl From<DialogSubmitOutcome> for SubmitDialogTurnResponse {
    fn from(outcome: DialogSubmitOutcome) -> Self {
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "agent/respondPermission", response = RespondPermissionResponse)]
pub struct RespondPermissionRequest {
    pub request_id: String,
    pub reply: PermissionReply,
}

unit_response!(RespondPermissionResponse);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "agent/listPendingPermissionRequests", response = PendingPermissionsResponse)]
pub struct PendingPermissionsRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct PendingPermissionsResponse {
    pub requests: Vec<PermissionRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "agent/cancelTurn", response = CancelTurnResponse)]
pub struct CancelTurnRequest(pub AgentTurnCancellationRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct CancelTurnResponse(pub AgentTurnCancellationResult);
