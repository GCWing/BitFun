//! Typed App Server requests used by the interactive TUI.
//!
//! The payloads reuse stable contract DTOs only. Runtime implementation types
//! are projected into the small wire-specific enums defined in this module by
//! the server adapter.

use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use bitfun_core_types::{ProviderCatalog, SessionUsageReport};
use bitfun_product_domains::tool_permissions::{PermissionReply, PermissionRequest};
use bitfun_runtime_ports::{
    AgentContextReloadRequest, AgentDialogSteerRequest, AgentDialogTurnRequest,
    AgentLocalCommandTurnRecordRequest, AgentLocalCommandTurnRecordResult,
    AgentMessageWorkspaceReferencesRequest, AgentSessionCompactionRequest,
    AgentSessionCreateRequest, AgentSessionCreateResult, AgentSessionDeleteRequest,
    AgentSessionForkBeforeTurnRequest, AgentSessionForkRequest, AgentSessionForkResult,
    AgentSessionLineageCancellationRequest, AgentSessionLineageInspection,
    AgentSessionLineageRequest, AgentSessionLineageSnapshot, AgentSessionLineageTranscriptRequest,
    AgentSessionListRequest, AgentSessionModeUpdateRequest, AgentSessionModelUpdateRequest,
    AgentSessionRenameRequest, AgentSessionRevertRequest, AgentSessionRevertResult,
    AgentSessionSummary, AgentSessionUsageRequest, AgentSessionWorkspaceBinding,
    AgentSessionWorkspaceRequest, AgentTurnCancellationRequest, AgentTurnCancellationResult,
    AgentTurnSettlementRequest, AgentUserShellCommandRequest, AgentUserShellCommandResult,
    AgentWorkspaceReference, AgentWorkspaceReferenceSearchRequest,
    AgentWorkspaceReferenceSearchResult, DialogSubmitOutcome, SessionTranscript,
    SessionTranscriptRequest, WorkspaceDiffSnapshot,
};
use serde::{Deserialize, Serialize};

macro_rules! unit_response {
    ($name:ident) => {
        #[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
        pub struct $name {}
    };
}

/// Provider and reasoning facts needed by TUI model configuration surfaces.
/// API keys and provider-specific execution metadata remain host-owned.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "config/getTuiModelCatalog", response = TuiModelCatalogResponse)]
pub struct TuiModelCatalogRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct TuiModelCatalogResponse {
    pub provider_catalog: ProviderCatalog,
    pub reasoning_presets_by_model: std::collections::BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "agent/listSessions", response = ListSessionsResponse)]
pub struct ListSessionsRequest(pub AgentSessionListRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct ListSessionsResponse {
    pub sessions: Vec<AgentSessionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "session/sync", response = SyncSessionResponse)]
#[serde(rename_all = "camelCase")]
pub struct SyncSessionRequest {
    pub workspace_path: String,
    pub session_id: String,
    #[serde(default)]
    pub include_internal: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct SyncSessionResponse {
    pub session: AgentSessionSummary,
    pub state: SessionRuntimeState,
    pub transcript: SessionTranscript,
    pub workspace_binding: AgentSessionWorkspaceBinding,
    #[serde(default)]
    pub pending_permissions: Vec<PermissionRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
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
#[serde(rename_all = "camelCase")]
pub enum SessionProcessingPhase {
    Starting,
    Compacting,
    Thinking,
    Streaming,
    ToolCalling,
    ToolConfirming,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "session/readTranscript", response = ReadTranscriptResponse)]
pub struct ReadTranscriptRequest(pub SessionTranscriptRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct ReadTranscriptResponse(pub SessionTranscript);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "session/resolveWorkspace", response = ResolveWorkspaceResponse)]
pub struct ResolveWorkspaceRequest(pub AgentSessionWorkspaceRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct ResolveWorkspaceResponse(pub Option<AgentSessionWorkspaceBinding>);

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
#[request(method = "session/recordLocalCommandTurn", response = RecordLocalCommandTurnResponse)]
pub struct RecordLocalCommandTurnRequest(pub AgentLocalCommandTurnRecordRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct RecordLocalCommandTurnResponse(pub AgentLocalCommandTurnRecordResult);

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
#[request(method = "session/rename", response = RenameSessionResponse)]
pub struct RenameSessionRequest(pub AgentSessionRenameRequest);

unit_response!(RenameSessionResponse);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "session/compact", response = CompactSessionResponse)]
pub struct CompactSessionRequest(pub AgentSessionCompactionRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct CompactSessionResponse(pub bitfun_runtime_ports::AgentSessionCompactionResult);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "session/undo", response = RevertSessionResponse)]
pub struct UndoSessionRequest(pub AgentSessionRevertRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "session/redo", response = RevertSessionResponse)]
pub struct RedoSessionRequest(pub AgentSessionRevertRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct RevertSessionResponse(pub AgentSessionRevertResult);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "session/reloadContext", response = ReloadContextResponse)]
pub struct ReloadContextRequest(pub AgentContextReloadRequest);

unit_response!(ReloadContextResponse);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "session/usage", response = SessionUsageResponse)]
pub struct SessionUsageRequest(pub AgentSessionUsageRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct SessionUsageResponse(pub SessionUsageReport);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "session/waitForSettlement", response = WaitForSettlementResponse)]
pub struct WaitForSettlementRequest(pub AgentTurnSettlementRequest);

unit_response!(WaitForSettlementResponse);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "workspace/diff", response = WorkspaceDiffResponse)]
pub struct WorkspaceDiffRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct WorkspaceDiffResponse(pub WorkspaceDiffSnapshot);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "workspace/searchReferences", response = SearchWorkspaceReferencesResponse)]
pub struct SearchWorkspaceReferencesRequest(pub AgentWorkspaceReferenceSearchRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct SearchWorkspaceReferencesResponse(pub AgentWorkspaceReferenceSearchResult);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "workspace/messageReferences", response = MessageReferencesResponse)]
pub struct MessageReferencesRequest(pub AgentMessageWorkspaceReferencesRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct MessageReferencesResponse(pub Vec<AgentWorkspaceReference>);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "session/lineage", response = SessionLineageResponse)]
pub struct SessionLineageRequest(pub AgentSessionLineageRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct SessionLineageResponse(pub Option<AgentSessionLineageSnapshot>);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "session/inspectLineage", response = InspectLineageResponse)]
pub struct InspectLineageRequest(pub AgentSessionLineageTranscriptRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct InspectLineageResponse(pub AgentSessionLineageInspection);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "session/cancelLineage", response = CancelLineageResponse)]
pub struct CancelLineageRequest(pub AgentSessionLineageCancellationRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct CancelLineageResponse(pub AgentTurnCancellationResult);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "session/fork", response = ForkSessionResponse)]
pub struct ForkSessionRequest(pub AgentSessionForkRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "session/forkBeforeTurn", response = ForkSessionResponse)]
pub struct ForkSessionBeforeTurnRequest(pub AgentSessionForkBeforeTurnRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct ForkSessionResponse(pub AgentSessionForkResult);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "session/updateModel", response = UpdateSessionModelResponse)]
pub struct UpdateSessionModelRequest(pub AgentSessionModelUpdateRequest);

unit_response!(UpdateSessionModelResponse);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "session/updateMode", response = UpdateSessionModeResponse)]
pub struct UpdateSessionModeRequest(pub AgentSessionModeUpdateRequest);

unit_response!(UpdateSessionModeResponse);

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
