//! ACP remote-control command host and response helpers.
//!
//! Wire commands live on [`super::RemoteCommand`]. This module owns the
//! execution port and the ACP-shaped responses that carry session identity,
//! capability, request id, and retry classification. It must not depend on
//! `bitfun-acp`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    remote_unsupported_response, ImageAttachment, RemoteImageContext, RemoteResponse,
    ACP_SESSION_REQUIRES_ACP_CONTROL_MESSAGE, REMOTE_CAPABILITY_ACP_REMOTE_CONTROL,
    UNSUPPORTED_REMOTE_CAPABILITY,
};

/// Whether a remote ACP command failure is safe to retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteRetryClassification {
    Retryable,
    Terminal,
    Stale,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteAcpSendRequest {
    pub session_id: String,
    pub content: String,
    pub images: Option<Vec<ImageAttachment>>,
    pub image_contexts: Option<Vec<RemoteImageContext>>,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteAcpCancelRequest {
    pub session_id: String,
    pub turn_id: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteAcpGetOptionsRequest {
    pub session_id: String,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteAcpSetOptionRequest {
    pub session_id: String,
    pub config_id: String,
    pub value: Value,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteAcpGetCommandsRequest {
    pub session_id: String,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteAcpGetPlanRequest {
    pub session_id: String,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteAcpPermissionRespondRequest {
    pub session_id: String,
    pub permission_id: String,
    pub option_id: String,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteAcpPermissionRespondOutcome {
    pub session_id: String,
    pub permission_id: String,
    pub request_id: Option<String>,
    pub resolved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteAcpSendOutcome {
    pub session_id: String,
    pub turn_id: String,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteAcpCancelOutcome {
    pub session_id: String,
    pub turn_id: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteAcpOptionsOutcome {
    pub session_id: String,
    pub request_id: Option<String>,
    pub options: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteAcpCommandsOutcome {
    pub session_id: String,
    pub request_id: Option<String>,
    pub commands: Value,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteAcpPlanOutcome {
    pub session_id: String,
    pub request_id: Option<String>,
    pub entries: Value,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteAcpControlError {
    pub session_id: String,
    pub request_id: Option<String>,
    pub code: String,
    pub message: String,
    pub retry: RemoteRetryClassification,
}

impl RemoteAcpControlError {
    pub fn unsupported(session_id: impl Into<String>, request_id: Option<String>) -> Self {
        let session_id = session_id.into();
        Self {
            session_id: session_id.clone(),
            request_id,
            code: UNSUPPORTED_REMOTE_CAPABILITY.to_string(),
            message: format!("{ACP_SESSION_REQUIRES_ACP_CONTROL_MESSAGE} (session={session_id})"),
            retry: RemoteRetryClassification::Terminal,
        }
    }

    pub fn terminal(
        session_id: impl Into<String>,
        request_id: Option<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            request_id,
            code: code.into(),
            message: message.into(),
            retry: RemoteRetryClassification::Terminal,
        }
    }

    pub fn into_response(self) -> RemoteResponse {
        RemoteResponse::AcpCommandError {
            session_id: self.session_id,
            capability: REMOTE_CAPABILITY_ACP_REMOTE_CONTROL.to_string(),
            request_id: self.request_id,
            retry: self.retry,
            code: self.code,
            message: self.message,
        }
    }
}

/// Product host that executes ACP remote-control commands without exposing
/// ACP process handles to the phone.
#[async_trait::async_trait]
pub trait RemoteAcpControlRuntimeHost: Send + Sync {
    async fn send_message(
        &self,
        request: RemoteAcpSendRequest,
    ) -> Result<RemoteAcpSendOutcome, RemoteAcpControlError>;

    async fn cancel_turn(
        &self,
        request: RemoteAcpCancelRequest,
    ) -> Result<RemoteAcpCancelOutcome, RemoteAcpControlError>;

    async fn get_options(
        &self,
        request: RemoteAcpGetOptionsRequest,
    ) -> Result<RemoteAcpOptionsOutcome, RemoteAcpControlError>;

    async fn set_option(
        &self,
        request: RemoteAcpSetOptionRequest,
    ) -> Result<RemoteAcpOptionsOutcome, RemoteAcpControlError>;

    async fn get_commands(
        &self,
        request: RemoteAcpGetCommandsRequest,
    ) -> Result<RemoteAcpCommandsOutcome, RemoteAcpControlError>;

    async fn get_plan(
        &self,
        request: RemoteAcpGetPlanRequest,
    ) -> Result<RemoteAcpPlanOutcome, RemoteAcpControlError>;

    async fn permission_respond(
        &self,
        request: RemoteAcpPermissionRespondRequest,
    ) -> Result<RemoteAcpPermissionRespondOutcome, RemoteAcpControlError>;

    /// True when `session_id` is an ACP session that must not accept native
    /// ConfirmTool / RejectTool (even when only a tool id is present).
    async fn is_acp_session(&self, session_id: &str) -> bool;

    /// True when `tool_id` is an ACP permission id that native Confirm/Reject
    /// must not convert.
    async fn is_acp_permission_id(&self, tool_id: &str) -> bool;

    /// Drop idempotent request caches for a finished ACP session.
    fn clear_session_idempotency(&self, _session_id: &str) {}
}

pub fn acp_permission_respond_response(
    result: Result<RemoteAcpPermissionRespondOutcome, RemoteAcpControlError>,
) -> RemoteResponse {
    match result {
        Ok(outcome) => RemoteResponse::AcpPermissionResolved {
            session_id: outcome.session_id,
            capability: REMOTE_CAPABILITY_ACP_REMOTE_CONTROL.to_string(),
            request_id: outcome.request_id,
            retry: RemoteRetryClassification::Terminal,
            permission_id: outcome.permission_id,
            resolved: outcome.resolved,
        },
        Err(error) => error.into_response(),
    }
}

pub fn acp_permission_respond_unsupported(
    session_id: &str,
    request_id: Option<String>,
) -> RemoteResponse {
    RemoteAcpControlError::terminal(
        session_id,
        request_id,
        UNSUPPORTED_REMOTE_CAPABILITY,
        format!("ACP permission respond requires the Desktop-owned mailbox (session={session_id})"),
    )
    .into_response()
}

pub fn acp_native_tool_interaction_unsupported(
    session_id: Option<&str>,
    tool_id: &str,
) -> RemoteResponse {
    let message = match session_id {
        Some(session_id) => format!(
            "Native tool confirmation is unsupported for ACP sessions (session={session_id}, tool_id={tool_id})"
        ),
        None => format!(
            "Native tool confirmation cannot target ACP permission ids (tool_id={tool_id})"
        ),
    };
    remote_unsupported_response(UNSUPPORTED_REMOTE_CAPABILITY, message)
}

/// Fail loud when a *native* session-scoped control command targets an ACP
/// session. Native cancel and native model selection have no meaning for a
/// session whose turns the Runtime does not own: the ACP agent owns its own
/// turn lifecycle (`acp_cancel_turn`) and its own model/config surface
/// (`acp_get_options` / `acp_set_option`). Silently forwarding these to the
/// native scheduler would admit an ACP session into `SessionManager`, which is
/// exactly the failure §13 tells us to watch for.
pub fn acp_native_session_control_unsupported(
    session_id: &str,
    command_name: &str,
) -> RemoteResponse {
    remote_unsupported_response(
        UNSUPPORTED_REMOTE_CAPABILITY,
        format!(
            "Native `{command_name}` is unsupported for ACP sessions; use the acp_* command family (session={session_id})"
        ),
    )
}

pub fn acp_send_response(
    result: Result<RemoteAcpSendOutcome, RemoteAcpControlError>,
) -> RemoteResponse {
    match result {
        Ok(outcome) => RemoteResponse::AcpMessageSent {
            session_id: outcome.session_id,
            capability: REMOTE_CAPABILITY_ACP_REMOTE_CONTROL.to_string(),
            request_id: outcome.request_id,
            retry: RemoteRetryClassification::Terminal,
            turn_id: outcome.turn_id,
        },
        Err(error) => error.into_response(),
    }
}

pub fn acp_cancel_response(
    result: Result<RemoteAcpCancelOutcome, RemoteAcpControlError>,
) -> RemoteResponse {
    match result {
        Ok(outcome) => RemoteResponse::AcpTurnCancelled {
            session_id: outcome.session_id,
            capability: REMOTE_CAPABILITY_ACP_REMOTE_CONTROL.to_string(),
            request_id: outcome.request_id,
            retry: RemoteRetryClassification::Terminal,
            turn_id: outcome.turn_id,
        },
        Err(error) => error.into_response(),
    }
}

pub fn acp_options_response(
    result: Result<RemoteAcpOptionsOutcome, RemoteAcpControlError>,
) -> RemoteResponse {
    match result {
        Ok(outcome) => RemoteResponse::AcpOptions {
            session_id: outcome.session_id,
            capability: REMOTE_CAPABILITY_ACP_REMOTE_CONTROL.to_string(),
            request_id: outcome.request_id,
            retry: RemoteRetryClassification::Terminal,
            options: outcome.options,
        },
        Err(error) => error.into_response(),
    }
}

pub fn acp_commands_response(
    result: Result<RemoteAcpCommandsOutcome, RemoteAcpControlError>,
) -> RemoteResponse {
    match result {
        Ok(outcome) => RemoteResponse::AcpCommands {
            session_id: outcome.session_id,
            capability: REMOTE_CAPABILITY_ACP_REMOTE_CONTROL.to_string(),
            request_id: outcome.request_id,
            retry: RemoteRetryClassification::Terminal,
            commands: outcome.commands,
            version: outcome.version,
        },
        Err(error) => error.into_response(),
    }
}

pub fn acp_plan_response(
    result: Result<RemoteAcpPlanOutcome, RemoteAcpControlError>,
) -> RemoteResponse {
    match result {
        Ok(outcome) => RemoteResponse::AcpPlan {
            session_id: outcome.session_id,
            capability: REMOTE_CAPABILITY_ACP_REMOTE_CONTROL.to_string(),
            request_id: outcome.request_id,
            retry: RemoteRetryClassification::Terminal,
            entries: outcome.entries,
            version: outcome.version,
        },
        Err(error) => error.into_response(),
    }
}
