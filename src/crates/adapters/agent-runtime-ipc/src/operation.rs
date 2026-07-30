use bitfun_product_domains::tool_permissions::{PermissionReply, PermissionRequest};
use bitfun_runtime_ports::{
    AgentContextReloadRequest, AgentDialogTurnRequest, AgentSessionCreateRequest,
    AgentSessionCreateResult, AgentSessionListRequest, AgentSessionModeUpdateRequest,
    AgentSessionModelUpdateRequest, AgentSessionSummary, AgentTurnCancellationRequest,
    AgentTurnCancellationResult, SessionTranscript,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeSessionRestoreRequest {
    pub workspace_path: String,
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeSessionRenameRequest {
    pub session_id: String,
    pub session_name: String,
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
    DeleteSession {
        session_id: String,
    },
    UpdateSessionMode {
        request: AgentSessionModeUpdateRequest,
    },
    UpdateSessionModel {
        request: AgentSessionModelUpdateRequest,
    },
    RenameSession {
        request: RuntimeSessionRenameRequest,
    },
    ReloadSessionContext {
        request: AgentContextReloadRequest,
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
            Self::DeleteSession { session_id } => Some(session_id),
            Self::UpdateSessionMode { request } => Some(&request.session_id),
            Self::UpdateSessionModel { request } => Some(&request.session_id),
            Self::RenameSession { request } => Some(&request.session_id),
            Self::ReloadSessionContext { request } => Some(&request.session_id),
            Self::SubmitTurn { request } => Some(&request.session_id),
            Self::CancelTurn { request } => Some(&request.session_id),
            Self::PendingPermissions { session_id }
            | Self::RespondPermission { session_id, .. } => Some(session_id),
            Self::SubmitUserAnswers { request } => Some(&request.session_id),
            Self::Health | Self::ListSessions { .. } | Self::CreateSession { .. } => None,
        }
    }

    pub(crate) fn rules(&self) -> RuntimeIpcOperationRules {
        use RuntimeIpcSessionRequirement::{
            AttachExisting, CurrentController, None, UncontrolledTarget,
        };

        match self {
            Self::Health | Self::ListSessions { .. } => {
                RuntimeIpcOperationRules::new(None, false, false, false)
            }
            Self::CreateSession { .. } => RuntimeIpcOperationRules::new(None, true, true, true),
            Self::RestoreSession { .. } => {
                RuntimeIpcOperationRules::new(AttachExisting, true, true, true)
            }
            Self::DeleteSession { .. } => {
                RuntimeIpcOperationRules::new(UncontrolledTarget, true, true, true)
            }
            Self::UpdateSessionMode { .. }
            | Self::UpdateSessionModel { .. }
            | Self::RenameSession { .. }
            | Self::SubmitTurn { .. } => {
                RuntimeIpcOperationRules::new(CurrentController, true, false, true)
            }
            Self::ReloadSessionContext { .. }
            | Self::CancelTurn { .. }
            | Self::RespondPermission { .. }
            | Self::SubmitUserAnswers { .. } => {
                RuntimeIpcOperationRules::new(CurrentController, false, false, true)
            }
            Self::PendingPermissions { .. } => {
                RuntimeIpcOperationRules::new(CurrentController, false, false, false)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeIpcSessionRequirement {
    None,
    CurrentController,
    AttachExisting,
    UncontrolledTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeIpcOperationRules {
    pub(crate) session_requirement: RuntimeIpcSessionRequirement,
    pub(crate) requires_idle: bool,
    pub(crate) serializes_session_selection: bool,
    pub(crate) side_effecting: bool,
}

impl RuntimeIpcOperationRules {
    const fn new(
        session_requirement: RuntimeIpcSessionRequirement,
        requires_idle: bool,
        serializes_session_selection: bool,
        side_effecting: bool,
    ) -> Self {
        Self {
            session_requirement,
            requires_idle,
            serializes_session_selection,
            side_effecting,
        }
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

#[cfg(test)]
mod tests {
    use super::{RuntimeIpcOperation, RuntimeIpcSessionRequirement, RuntimeSessionRestoreRequest};
    use bitfun_runtime_ports::{AgentContextReloadRequest, AgentContextReloadTarget};

    #[test]
    fn delete_rules_are_fail_closed_for_shared_session_selection() {
        let rules = RuntimeIpcOperation::DeleteSession {
            session_id: "session-2".to_string(),
        }
        .rules();

        assert_eq!(
            rules.session_requirement,
            RuntimeIpcSessionRequirement::UncontrolledTarget
        );
        assert!(rules.requires_idle);
        assert!(rules.serializes_session_selection);
        assert!(rules.side_effecting);
    }

    #[test]
    fn reload_rules_preserve_active_turn_semantics_after_rule_consolidation() {
        let rules = RuntimeIpcOperation::ReloadSessionContext {
            request: AgentContextReloadRequest {
                session_id: "session-1".to_string(),
                target: AgentContextReloadTarget::All,
            },
        }
        .rules();

        assert_eq!(
            rules.session_requirement,
            RuntimeIpcSessionRequirement::CurrentController
        );
        assert!(!rules.requires_idle);
        assert!(!rules.serializes_session_selection);
        assert!(rules.side_effecting);
    }

    #[test]
    fn restore_and_pending_permission_rules_preserve_existing_behavior() {
        let restore = RuntimeIpcOperation::RestoreSession {
            request: RuntimeSessionRestoreRequest {
                workspace_path: "D:/workspace/project".to_string(),
                session_id: "session-2".to_string(),
            },
        }
        .rules();
        assert_eq!(
            restore.session_requirement,
            RuntimeIpcSessionRequirement::AttachExisting
        );
        assert!(restore.requires_idle);
        assert!(restore.serializes_session_selection);
        assert!(restore.side_effecting);

        let pending = RuntimeIpcOperation::PendingPermissions {
            session_id: "session-1".to_string(),
        }
        .rules();
        assert_eq!(
            pending.session_requirement,
            RuntimeIpcSessionRequirement::CurrentController
        );
        assert!(!pending.requires_idle);
        assert!(!pending.serializes_session_selection);
        assert!(!pending.side_effecting);
    }
}
