use bitfun_product_domains::tool_permissions::{PermissionReply, PermissionRequest};
use bitfun_runtime_ports::{
    AgentDialogTurnRequest, AgentSessionCreateRequest, AgentSessionCreateResult,
    AgentSessionListRequest, AgentSessionModeUpdateRequest, AgentSessionSummary,
    AgentTurnCancellationRequest, AgentTurnCancellationResult, SessionTranscript,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeSessionRestoreRequest {
    pub workspace_path: String,
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeUserAnswersRequest {
    pub session_id: String,
    pub tool_id: String,
    pub answers: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RuntimeIpcOperation {
    Health,
    ListSessions {
        request: AgentSessionListRequest,
    },
    CreateSession {
        request: AgentSessionCreateRequest,
    },
    RestoreSession {
        request: RuntimeSessionRestoreRequest,
    },
    UpdateSessionMode {
        request: AgentSessionModeUpdateRequest,
    },
    SubmitTurn {
        request: AgentDialogTurnRequest,
    },
    CancelTurn {
        request: AgentTurnCancellationRequest,
    },
    PendingPermissions {
        session_id: String,
    },
    RespondPermission {
        session_id: String,
        request_id: String,
        reply: PermissionReply,
    },
    SubmitUserAnswers {
        request: RuntimeUserAnswersRequest,
    },
}

impl RuntimeIpcOperation {
    pub fn session_id(&self) -> Option<&str> {
        match self {
            Self::RestoreSession { request } => Some(&request.session_id),
            Self::UpdateSessionMode { request } => Some(&request.session_id),
            Self::SubmitTurn { request } => Some(&request.session_id),
            Self::CancelTurn { request } => Some(&request.session_id),
            Self::PendingPermissions { session_id }
            | Self::RespondPermission { session_id, .. } => Some(session_id),
            Self::SubmitUserAnswers { request } => Some(&request.session_id),
            Self::Health | Self::ListSessions { .. } | Self::CreateSession { .. } => None,
        }
    }

    pub fn requires_controller(&self) -> bool {
        matches!(
            self,
            Self::UpdateSessionMode { .. }
                | Self::SubmitTurn { .. }
                | Self::CancelTurn { .. }
                | Self::PendingPermissions { .. }
                | Self::RespondPermission { .. }
                | Self::SubmitUserAnswers { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "result",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RuntimeIpcOperationResult {
    Health {
        instance_identity: String,
        process_id: u32,
    },
    Unit,
    Sessions {
        sessions: Vec<AgentSessionSummary>,
    },
    SessionCreated {
        session: AgentSessionCreateResult,
    },
    SessionRestored {
        session: AgentSessionSummary,
        transcript: SessionTranscript,
        pending_permissions: Vec<PermissionRequest>,
    },
    TurnAccepted {
        session_id: String,
        turn_id: String,
    },
    TurnCancelled {
        cancellation: AgentTurnCancellationResult,
    },
    PendingPermissions {
        requests: Vec<PermissionRequest>,
    },
}
