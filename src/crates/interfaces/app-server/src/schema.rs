//! JSON-RPC schema for the BitFun agent kernel app-server surface.
//!
//! These messages are the wire contract between an `AppClient` and the
//! [`crate::BitfunAppServer`]. The runtime port types (in
//! `bitfun_runtime_ports`) already derive `Serialize`/`Deserialize`, but they do
//! not implement `agent_client_protocol::JsonRpcResponse`, so each response is
//! wrapped in a newtype that derives `JsonRpcResponse`. The `run` operation
//! additionally maps the non-serde `SessionSelector` / `AgentRunHandle` to
//! wire-friendly types.

use agent_client_protocol::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use bitfun_agent_runtime::sdk::{
    AgentDialogTurnExecution, AgentDialogTurnRequest, AgentInputAttachment, AgentRunHandle,
    AgentRunRequest, AgentSessionCreateRequest, AgentSessionCreateResult,
    AgentSessionDeleteRequest, AgentSessionListRequest, AgentSessionSummary,
    AgentSubmissionRequest, AgentSubmissionResult, AgentSubmissionSource,
    AgentTurnCancellationRequest, AgentTurnCancellationResult, AgenticEventEnvelope,
    DialogSubmissionPolicy, DialogSubmitOutcome, PermissionAuditRecord, PermissionGrant,
    PermissionGrantKey, PermissionReply, PermissionRequest, PermissionRequestEvent,
    SessionSelector,
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

/// `agent/listSessions` response body. The summary vector is wrapped because
/// `JsonRpcRequest::Response` must be a single named type, not a bare `Vec`.
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

/// `agent/deleteSession` response body. The runtime returns `()` so this is a
/// structurally empty success marker.
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
///
/// This is the dialog-turn operation the desktop `start_dialog_turn` command
/// drives (via the SDK's [`AgentRuntime::submit_dialog_turn`]), unlike
/// `agent/submitTurn` which is a bare message into an existing session. The
/// body mirrors [`AgentDialogTurnRequest`] but makes `policy` optional on the
/// wire: web clients (and any caller that does not select a dialog policy)
/// omit it and the server substitutes the desktop default
/// [`DialogSubmissionPolicy::for_source`]`(AgentSubmissionSource::DesktopUi)`,
/// matching how the desktop host synthesizes it (`agentic_api.rs`).
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
    /// Build the runtime request, defaulting `policy` to the desktop UI source
    /// when the caller omitted it, and passing through the remaining fields.
    ///
    /// `reply_route` and `prepended_reminders` are **not** on the wire body
    /// struct, so they are intentionally absent from the client contract.
    /// They are runtime-internal fields on `AgentDialogTurnRequest` and are
    /// defaulted here; a client cannot send them and they cannot be silently
    /// dropped because they are never accepted by deserialization.
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

/// `agent/submitDialogTurn` response body, mapped from the non-serde
/// [`DialogSubmitOutcome`] (which only derives `Debug`/`Clone`/`PartialEq`/`Eq`).
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

// Permission surface ----------------------------------------------------------
//
// These map the `AgentRuntime` permission SDK (`pending_permission_requests`,
// `subscribe_permission_requests`, `respond_permission(_batch)`, project grants
// + audit) onto the app-server wire. The runtime already holds the permission
// manager, so the host injects it as usual via `BitfunAppRuntime`; the
// `respondPermission`/`respondPermissionBatch`/`listPendingPermissionRequests`
// commands are driven from here, and `agent/permissionEvent` notifications carry
// the inbound `PermissionRequestEvent` stream to the client (the desktop host
// today emits these to the UI via `app.emit("permission://event")`; the
// app-server forwards them over the transport instead).

/// `agent/respondPermission` request body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "agent/respondPermission", response = RespondPermissionResponse)]
pub struct RespondPermissionMessage {
    pub request_id: String,
    pub reply: PermissionReply,
}

/// `agent/respondPermission` response body (the SDK returns `()`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RespondPermissionResponse {}

/// `agent/respondPermissionBatch` request body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(
    method = "agent/respondPermissionBatch",
    response = RespondPermissionBatchResponse
)]
pub struct RespondPermissionBatchMessage {
    pub request_id: String,
    pub reply: PermissionReply,
}

/// `agent/respondPermissionBatch` response body: the request ids that shared
/// the resolved reply.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RespondPermissionBatchResponse {
    pub request_ids: Vec<String>,
}

/// `agent/listPendingPermissionRequests` request body (no parameters).
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(
    method = "agent/listPendingPermissionRequests",
    response = ListPendingPermissionRequestsResponse
)]
pub struct ListPendingPermissionRequestsMessage {}

/// `agent/listPendingPermissionRequests` response body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ListPendingPermissionRequestsResponse {
    pub requests: Vec<PermissionRequest>,
}

/// `agent/listProjectPermissionGrants` request body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(
    method = "agent/listProjectPermissionGrants",
    response = ListProjectPermissionGrantsResponse
)]
pub struct ListProjectPermissionGrantsMessage {
    pub project_id: String,
}

/// `agent/listProjectPermissionGrants` response body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ListProjectPermissionGrantsResponse {
    pub grants: Vec<PermissionGrant>,
}

/// `agent/removeProjectPermissionGrant` request body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[request(
    method = "agent/removeProjectPermissionGrant",
    response = RemoveProjectPermissionGrantResponse
)]
pub struct RemoveProjectPermissionGrantMessage(pub PermissionGrantKey);

/// `agent/removeProjectPermissionGrant` response body: whether a grant was
/// removed.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RemoveProjectPermissionGrantResponse {
    pub removed: bool,
}

/// `agent/clearProjectPermissionGrants` request body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(
    method = "agent/clearProjectPermissionGrants",
    response = ClearProjectPermissionGrantsResponse
)]
pub struct ClearProjectPermissionGrantsMessage {
    pub project_id: String,
}

/// `agent/clearProjectPermissionGrants` response body: how many grants cleared.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ClearProjectPermissionGrantsResponse {
    pub cleared: usize,
}

/// `agent/permissionEvent` notification: a permission lifecycle event forwarded
/// to the client. The server drains the runtime permission receiver (the same
/// stream the desktop host emits as `permission://event`) and forwards each
/// [`PermissionRequestEvent`] over the transport; the client fans it out to its
/// consumers. This keeps the permission stream on the app-server protocol surface.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcNotification)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[notification(method = "agent/permissionEvent")]
pub struct PermissionEventNotification(pub PermissionRequestEvent);

/// `agent/listProjectPermissionAudit` request body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(
    method = "agent/listProjectPermissionAudit",
    response = ListProjectPermissionAuditResponse
)]
pub struct ListProjectPermissionAuditMessage {
    pub project_id: String,
}

/// `agent/listProjectPermissionAudit` response body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ListProjectPermissionAuditResponse {
    pub records: Vec<PermissionAuditRecord>,
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

/// `agent/run` request body. `SessionSelector` (in `agent-runtime`) does not
/// derive serde, so the wire form is a discriminated union that maps to it.
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

/// `agent/run` response body, mapped from `AgentRunHandle` (which does not
/// derive serde).
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
        let mut req = AgentRunRequest::new(self.session.to_selector(), &self.message);
        if let Some(turn_id) = &self.turn_id {
            req = req.with_turn_id(turn_id);
        }
        if let Some(source) = self.source {
            req = req.with_source(source);
        }
        req
    }
}

/// `agent/event` notification: a runtime event forwarded to the client.
///
/// `AgenticEventEnvelope` is the exact type the runtime event queue
/// broadcasts to subscribers, and it derives serde. The server forwards each
/// envelope it receives from its injected [`AgentEventSource`] to the client
/// over the app-server transport; the client registers `on_receive_notification`
/// to receive them. This keeps the event stream on the app-server protocol
/// surface instead of letting the client subscribe to the runtime queue directly.
///
/// [`AgentEventSource`]: bitfun_agent_runtime::sdk::AgentEventSource
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcNotification)]
// NOTE(1a): `AgenticEventEnvelope` (bitfun-events) is not yet `TS`-derivable;
// the `agent/event` notification surface is exported in Step 1 Phase 1b once
// the events crate derives `TS` (see docs/plans/step1-ts-rs-integration.md Sec. 5).
#[notification(method = "agent/event")]
pub struct SessionEventNotification(pub AgenticEventEnvelope);

/// `agent/frontendEvent` notification: a runtime or permission event projected
/// to the frontend shape (`agentic://<type>` / `permission://event`) and pushed
/// to the browser by the server's `serve` main loop. Carrying the projected
/// `event` name and `payload` lets the browser `listen(event)` dispatch on the
/// same names it uses today, with zero call-site change. This is the
/// browser-facing event surface under browser-direct ACP-over-WS (Step 2).
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcNotification)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[notification(method = "agent/frontendEvent")]
pub struct FrontendEventNotification {
    /// Frontend event name (e.g. `agentic://session-created`, `permission://event`).
    pub event: String,
    /// Projected payload, already in the frontend's expected shape.
    pub payload: serde_json::Value,
}

// Git service surface ---------------------------------------------------------
//
// Under option C the app-server schema owns the full backend contract, not just
// agent-kernel ops. These `git/*` messages expose the read-only `GitService`
// operations (`bitfun_core::service::git::GitService`, which re-exports
// `bitfun_services_integrations::git::GitService`). The handlers call the
// static `GitService::xxx(&path)` associated functions the same way the
// Desktop host's 583 Tauri commands do -- no service injection is needed for
// static services, only the lifecycle-bound singletons (coordinator/scheduler)
// require injection and those land with the agent-control batches.
//
// Request bodies mirror the Desktop Tauri request types: camelCase wire fields
// (`#[serde(rename_all = "camelCase")]`) so the frontend `GitAPI` call sites
// (`api.invoke('git_get_status', { request: { repositoryPath } })`) deserialize
// unchanged. Responses reuse the core types directly -- they already derive
// serde with snake_case field names, which the frontend `GitStatus`/`GitBranch`
// TS interfaces also use, so no wire wrapping is needed on the response side.
// The method names use the `group/verb` camelCase convention (`git/isRepository`,
// `git/getStatus`) matching the existing `agent/createSession` style; the
// websocket adapter (`websocket-adapter.ts::AGENT_COMMAND_TO_WS_METHOD`)
// translates the frontend snake_case command name to this method.
//
// Scope: read-only operations only in this batch. Write operations
// (`git/addFiles`, `git/commit`, `git/push`, ...) and the remote (SSH) path
// arrive in later batches; the Server Host has no SSH manager, so remote git
// paths surface as `host_capability_unavailable` (the `external_sources` write
// precedent).

/// `git/isRepository` request body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[request(method = "git/isRepository", response = GitIsRepositoryResponse)]
pub struct GitIsRepositoryMessage(pub GitRepositoryPathRequest);

/// `git/isRepository` response body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GitIsRepositoryResponse(pub bool);

/// `git/getStatus` request body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[request(method = "git/getStatus", response = GitGetStatusResponse)]
pub struct GitGetStatusMessage(pub GitRepositoryPathRequest);

/// `git/getStatus` response body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GitGetStatusResponse(pub bitfun_core::service::git::GitStatus);

/// `git/getBranches` request body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[request(method = "git/getBranches", response = GitGetBranchesResponse)]
pub struct GitGetBranchesMessage(pub GitBranchesRequest);

/// `git/getBranches` response body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct GitGetBranchesResponse {
    pub branches: Vec<bitfun_core::service::git::GitBranch>,
}

/// Common camelCase wire shape for a single repository-path request. Mirrors
/// the Desktop `GitRepositoryRequest` (`rename_all = "camelCase"`) so the
/// frontend payload deserializes without field renaming at the call site.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct GitRepositoryPathRequest {
    pub repository_path: String,
}

/// `git/getBranches` wire request: repository path plus the optional
/// `includeRemote` flag. The core `GitService::get_branches` takes a bare `bool`,
/// so an omitted flag defaults to `false` in the handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct GitBranchesRequest {
    pub repository_path: String,
    #[serde(default)]
    pub include_remote: Option<bool>,
}

// Config service surface -----------------------------------------------------
//
// Read-only `ConfigService` / agent-profile canonicalizer operations. Under
// option C these live on the app-server surface alongside the agent-kernel
// and `git/*` groups. The handlers call the global config singletons the same
// way the Desktop host does -- `bitfun_core::service::config::get_global_config_service`
// (an `Arc<ConfigService>` initialized by the host's bootstrap) and the static
// `mode_config_canonicalizer::get_agent_profile_views` -- so no service
// injection is needed, mirroring the static `GitService` pattern.
//
// This batch scopes to the read-only config operations: `getAgentProfileConfigs`
// /`getAgentProfileConfig` (pure canonicalizer), `getModelConfigs`
// (`config_service.get_ai_models`), and `getConfig`/`getConfigs`
// (`config_service.get_config::<Value>`). `getConfig`/`getConfigs` carry the
// "config path not found" retry contract the frontend depends on
// ([ConfigAPI.ts] returns `undefined` on matching errors): the
// [`crate::agent::config_get_error`] helper puts the `BitFunError::NotFound`
// Display text into the JSON-RPC `message` (not just `data`) so the frontend
// substring match hits in web mode the same way it does on desktop. The
// `skipRetryOnNotFound` request field is accepted for contract parity and
// otherwise ignored -- the app-server does not retry; the field steers the
// frontend `ApiClient` retry policy and desktop-side logging.
// `get_skill_configs` depends on the workspace service and lands in a later
// batch.

/// `config/getAgentProfileConfigs` request body (no parameters).
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(
    method = "config/getAgentProfileConfigs",
    response = GetAgentProfileConfigsResponse
)]
pub struct GetAgentProfileConfigsMessage {}

/// `config/getAgentProfileConfigs` response body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct GetAgentProfileConfigsResponse {
    pub profiles: std::collections::HashMap<String, bitfun_core::service::config::AgentProfileView>,
}

/// `config/getAgentProfileConfig` request body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(
    method = "config/getAgentProfileConfig",
    response = GetAgentProfileConfigResponse
)]
pub struct GetAgentProfileConfigMessage {
    pub agent_id: String,
}

/// `config/getAgentProfileConfig` response body. The canonicalizer returns a
/// bare `AgentProfileView` (erroring when the id is unknown), so the wire form
/// is a single value -- no `Option` wrapper.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GetAgentProfileConfigResponse(pub bitfun_core::service::config::AgentProfileView);

/// `config/getModelConfigs` request body (no parameters).
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
// NOTE(1a): deferred to Phase 1b -- `AIModelConfig` carries a `#[serde(from =
// "AIModelConfigCompat")]` migration shim that must derive `TS` first (so the
// generated type matches the deserialized shape, not the in-memory struct).
#[request(method = "config/getModelConfigs", response = GetModelConfigsResponse)]
pub struct GetModelConfigsMessage {}

/// `config/getModelConfigs` response body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct GetModelConfigsResponse {
    pub models: Vec<bitfun_core::service::config::AIModelConfig>,
}

/// `config/getConfig` request body. Mirrors the desktop `GetConfigRequest`
/// (`rename_all = "camelCase"`): `path` is optional (a missing path reads the
/// whole config tree). `skipRetryOnNotFound` is accepted for contract parity
/// and otherwise ignored by the app-server (it does not retry; the field
/// steers the frontend `ApiClient` retry policy and desktop-side logging).
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "config/getConfig", response = GetConfigResponse)]
#[serde(rename_all = "camelCase")]
pub struct GetConfigMessage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "skip_if_false")]
    pub skip_retry_on_not_found: bool,
}

/// `config/getConfig` response body. The config value is an arbitrary JSON
/// tree, so it is surfaced as `serde_json::Value` (the desktop host returns
/// `Value` too -- `config_service.get_config::<Value>(path)`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GetConfigResponse(pub serde_json::Value);

/// `config/getConfigs` request body. Mirrors the desktop `GetConfigsRequest`
/// (`rename_all = "camelCase"`): a list of paths to read in one batch.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "config/getConfigs", response = GetConfigsResponse)]
#[serde(rename_all = "camelCase")]
pub struct GetConfigsMessage {
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "skip_if_false")]
    pub skip_retry_on_not_found: bool,
}

/// `config/getConfigs` response body. Maps to the desktop
/// `BTreeMap<String, Value>` shape; the handler dedupes paths the same way the
/// desktop host does (`config_api.rs::get_configs` skips a path already seen).
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct GetConfigsResponse {
    pub configs: std::collections::BTreeMap<String, serde_json::Value>,
}

/// `config/setConfig` request body (Track B): writes a value at a config path.
/// Mirrors the desktop `SetConfigRequest` (`rename_all = "camelCase"`). The
/// handler reaches the global config singleton (`get_global_config_service`),
/// the same way the Desktop host does -- no service injection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "config/setConfig", response = SetConfigResponse)]
#[serde(rename_all = "camelCase")]
pub struct SetConfigMessage {
    pub path: String,
    pub value: serde_json::Value,
}

/// `config/setConfig` response body (the config service returns `()`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct SetConfigResponse {}

// I18n service surface (Track B) ---------------------------------------------
//
// Read/write the runtime locale via the global config singleton
// (`app.language`) and the global `I18nService` (`sync_global_i18n_service_locale`).
// Locale identifiers are surfaced as plain strings (`zh-CN`, `en-US`, ...) to
// avoid deriving `TS` on the i18n crate's `LocaleId`/`LocaleMetadata` types;
// the supported-languages response carries a project-local wire struct.

/// `i18n/getCurrentLanguage` response body (no parameters -> uses a
/// `JsonRpcRequest` with an empty body + a typed response).
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "i18n/getCurrentLanguage", response = I18nGetCurrentLanguageResponse)]
pub struct I18nGetCurrentLanguageMessage {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct I18nGetCurrentLanguageResponse {
    /// BCP-47-ish locale id, e.g. `zh-CN`. Defaults to `zh-CN` when unset.
    pub language: String,
}

/// `i18n/setLanguage` request body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "i18n/setLanguage", response = I18nSetLanguageResponse)]
#[serde(rename_all = "camelCase")]
pub struct I18nSetLanguageMessage {
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct I18nSetLanguageResponse {
    /// The locale id that was applied (validated against the supported set).
    pub language: String,
}

/// `i18n/getConfig` request body (no parameters).
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "i18n/getConfig", response = I18nGetConfigResponse)]
pub struct I18nGetConfigMessage {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct I18nGetConfigResponse {
    pub current_language: String,
    pub fallback_language: String,
    pub auto_detect: bool,
}

/// `i18n/setConfig` request body: writes `currentLanguage` and syncs the global
/// I18nService. `fallbackLanguage`/`autoDetect` are accepted for contract parity
/// and otherwise ignored (the runtime does not yet store them).
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "i18n/setConfig", response = I18nSetConfigResponse)]
#[serde(rename_all = "camelCase")]
pub struct I18nSetConfigMessage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_language: Option<String>,
    #[serde(default)]
    pub auto_detect: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct I18nSetConfigResponse {}

/// `i18n/getSupportedLanguages` request body (no parameters).
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "i18n/getSupportedLanguages", response = I18nGetSupportedLanguagesResponse)]
pub struct I18nGetSupportedLanguagesMessage {}

/// One supported locale's metadata, projected from `bitfun_core::service::i18n::LocaleMetadata`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct I18nLocaleMetadata {
    pub id: String,
    pub name: String,
    pub english_name: String,
    pub native_name: String,
    pub rtl: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct I18nGetSupportedLanguagesResponse {
    pub locales: Vec<I18nLocaleMetadata>,
}

/// Serializes a `bool` only when it is `true`, so the default `false` for
/// `skipRetryOnNotFound` is omitted from the request wire form (matching the
/// desktop host's `#[serde(default)]` request shape, which also omits it when
/// false).
fn skip_if_false(value: &bool) -> bool {
    !*value
}
