//! CLI/TUI Agent Runtime SDK client.
//!
//! Keeps CLI session state while product operations remain behind portable
//! Runtime SDK ports.
//! Event consumption is NOT done here — it's done in the chat/exec mode main loops.

use anyhow::Result;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::sync::{broadcast, Mutex};

use bitfun_agent_runtime::sdk::{
    AgentDialogTurnExecution, AgentDialogTurnRequest, AgentEventReceiver, AgentInputAttachment,
    AgentLocalCommandTurnRecordRequest, AgentMessageWorkspaceReferencesRequest, AgentRuntime,
    AgentSessionCompactionRequest, AgentSessionCreateRequest, AgentSessionDeleteRequest,
    AgentSessionForkBeforeTurnRequest, AgentSessionForkRequest, AgentSessionForkResult,
    AgentSessionListRequest, AgentSessionModeUpdateRequest, AgentSessionModelUpdateRequest,
    AgentSessionRenameRequest, AgentSessionRestoreRequest, AgentSessionRevertRequest,
    AgentSessionRevertResult, AgentSessionUsageRequest, AgentTurnCancellationRequest,
    AgentTurnSettlementRequest, AgentUserAnswersRequest, AgentUserShellCommandRequest,
    AgentWorkspaceReference, AgentWorkspaceReferenceSearchRequest,
    AgentWorkspaceReferenceSearchResult, PermissionReply, PermissionRequest,
    PermissionRequestEventReceiver, PortError, PortErrorKind, RuntimeError, SessionTranscript,
    SessionTranscriptRequest, SessionUsageReport, WorkspaceDiffSnapshot,
};
use bitfun_agent_runtime_ipc::{
    RuntimeIpcClient, RuntimeIpcClientError, RuntimeIpcClientEvent, RuntimeIpcErrorCode,
    RuntimeIpcEvent, RuntimeIpcOperation, RuntimeIpcOperationResult,
    RuntimeIpcStreamInvalidationReason, RuntimeSessionForkRequest, RuntimeSessionRenameRequest,
    RuntimeSessionRestoreRequest, RuntimeUserAnswersRequest,
};
use bitfun_events::{AgenticEvent, AgenticEventEnvelope};
use bitfun_runtime_ports::{
    put_agent_workspace_references, AgentSessionSummary, AgentSessionWorkspaceBinding,
    AgentSessionWorkspaceRequest, AgentSubmissionSource, DialogSubmissionPolicy,
    SessionExecutionTarget,
};

use crate::actions::SHARED_TUI_EMBEDDED_HANDOFF;
use crate::diagnostics::with_session_conflict_help;
use crate::runtime::approval::{approval_metadata, CliApprovalPolicy};
use crate::runtime::CliRuntimeContext;

fn shared_restore_error(error: RuntimeIpcClientError) -> anyhow::Error {
    let error = if matches!(&error, RuntimeIpcClientError::Remote(remote) if remote.code == RuntimeIpcErrorCode::FrameTooLarge)
    {
        anyhow::anyhow!(
            "Session history is too large for Shared TUI. {SHARED_TUI_EMBEDDED_HANDOFF}."
        )
    } else {
        anyhow::Error::new(error)
    };
    with_session_conflict_help(error)
}

fn validated_session_summary(
    sessions: &[AgentSessionSummary],
    session_id: &str,
    workspace_path: &Path,
) -> Result<AgentSessionSummary> {
    sessions
        .iter()
        .find(|summary| summary.session_id == session_id)
        .cloned()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Session {session_id} was not found in the current workspace: {}",
                workspace_path.display()
            )
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SessionMigrationNotice {
    Mode {
        previous_id: String,
        restored_id: String,
    },
    Model {
        previous_id: String,
        restored_id: String,
    },
}

impl SessionMigrationNotice {
    pub(crate) fn user_message(&self) -> String {
        let (setting, previous_id, restored_id) = match self {
            Self::Mode {
                previous_id,
                restored_id,
            } => ("mode", previous_id, restored_id),
            Self::Model {
                previous_id,
                restored_id,
            } => ("model", previous_id, restored_id),
        };
        format!(
            "Session {setting} \"{previous_id}\" is unavailable. This session was restored with \"{restored_id}\". Review the {setting} before continuing."
        )
    }
}

fn session_migration_notices(
    previous: &AgentSessionSummary,
    restored: &AgentSessionSummary,
) -> Vec<SessionMigrationNotice> {
    let mut notices = Vec::with_capacity(2);
    if previous.agent_type != restored.agent_type {
        notices.push(SessionMigrationNotice::Mode {
            previous_id: previous.agent_type.clone(),
            restored_id: restored.agent_type.clone(),
        });
    }
    if let (Some(previous_id), Some(restored_id)) =
        (previous.model_id.as_ref(), restored.model_id.as_ref())
    {
        if previous_id != restored_id {
            notices.push(SessionMigrationNotice::Model {
                previous_id: previous_id.clone(),
                restored_id: restored_id.clone(),
            });
        }
    }
    notices
}

#[derive(Debug)]
pub(crate) struct SessionOperationError {
    message: String,
    outcome_unknown: bool,
}

impl fmt::Display for SessionOperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SessionOperationError {}

impl SessionOperationError {
    fn runtime(error: RuntimeError) -> Self {
        let outcome_unknown = matches!(
            &error,
            RuntimeError::Port(port_error)
                if port_error.kind == PortErrorKind::OutcomeUnknown
        );
        Self {
            message: error.into_message(),
            outcome_unknown,
        }
    }

    fn shared(error: RuntimeIpcClientError) -> Self {
        let outcome_unknown = matches!(
            &error,
            RuntimeIpcClientError::Remote(remote)
                if remote.code == RuntimeIpcErrorCode::OutcomeUnknown
        ) || matches!(
            &error,
            RuntimeIpcClientError::Timeout
                | RuntimeIpcClientError::Disconnected
                | RuntimeIpcClientError::UnexpectedResponse
                | RuntimeIpcClientError::Io(_)
        );
        Self {
            message: error.to_string(),
            outcome_unknown,
        }
    }

    fn unexpected(error: anyhow::Error) -> Self {
        Self {
            message: error.to_string(),
            outcome_unknown: true,
        }
    }

    pub(crate) fn outcome_unknown(&self) -> bool {
        self.outcome_unknown
    }
}

#[derive(Clone, Debug)]
struct CliWorkspacePaths {
    project: Option<PathBuf>,
    execution: Option<PathBuf>,
    execution_target: Option<SessionExecutionTarget>,
    remote: bool,
}

impl CliWorkspacePaths {
    fn new(workspace_path: Option<PathBuf>) -> Self {
        Self {
            project: workspace_path.clone(),
            execution: workspace_path,
            execution_target: None,
            remote: false,
        }
    }

    fn execution(&self) -> PathBuf {
        self.execution
            .clone()
            .or_else(|| self.project.clone())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn project(&self) -> PathBuf {
        self.project
            .clone()
            .or_else(|| self.execution.clone())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn apply_binding(&mut self, binding: &AgentSessionWorkspaceBinding) {
        let execution = PathBuf::from(&binding.workspace_path);
        let project = binding
            .project_workspace_path
            .as_deref()
            .filter(|path| !path.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| self.project());
        self.execution = Some(execution);
        self.project = Some(project);
        self.execution_target = binding.execution_target.clone();
        self.remote = binding.remote_connection_id.is_some() || binding.remote_ssh_host.is_some();
    }

    fn reset_execution_to_project(&mut self) -> PathBuf {
        let project = self.project();
        self.execution = Some(project.clone());
        self.execution_target = Some(SessionExecutionTarget::local(
            project.to_string_lossy().to_string(),
        ));
        self.remote = false;
        project
    }

    fn workspace_diff_unavailable_reason(&self) -> Option<&'static str> {
        if self.remote {
            return Some("Workspace diff is unavailable for remote Sessions");
        }
        let execution = self.execution();
        let project = self.project();
        if !same_workspace_location(&execution, &project) {
            return Some(
                "Workspace diff is unavailable when the Session uses a different worktree",
            );
        }
        None
    }
}

fn same_workspace_location(left: &Path, right: &Path) -> bool {
    left == right
        || dunce::canonicalize(left)
            .ok()
            .zip(dunce::canonicalize(right).ok())
            .is_some_and(|(left, right)| left == right)
}

/// CLI-owned client for the portable Agent Runtime SDK.
/// Stateless regarding agent_type; callers pass it per call.
pub(crate) struct CliAgentRuntimeClient {
    backend: CliAgentRuntimeBackend,
    approval_policy: Arc<RwLock<CliApprovalPolicy>>,
    workspace_paths: Arc<RwLock<CliWorkspacePaths>>,
    /// Session ID — uses Mutex for interior mutability
    session_id: Arc<Mutex<Option<String>>>,
    /// Current turn ID (for cancellation)
    current_turn_id: Arc<Mutex<Option<String>>>,
    shared_agent_events: Option<SharedBroadcast<AgenticEventEnvelope>>,
    shared_permission_events:
        Option<SharedBroadcast<bitfun_agent_runtime::sdk::PermissionRequestEvent>>,
    shared_pending_permissions: Arc<RwLock<HashMap<String, PermissionRequest>>>,
}

enum CliAgentRuntimeBackend {
    Embedded(AgentRuntime),
    Shared(RuntimeIpcClient),
}

type SharedBroadcast<T> = Arc<RwLock<Option<broadcast::Sender<T>>>>;

impl CliAgentRuntimeClient {
    pub(crate) fn new(runtime: &CliRuntimeContext, workspace_path: Option<PathBuf>) -> Self {
        Self {
            backend: CliAgentRuntimeBackend::Embedded(runtime.agent_runtime().clone()),
            approval_policy: Arc::new(RwLock::new(runtime.approval_policy())),
            workspace_paths: Arc::new(RwLock::new(CliWorkspacePaths::new(workspace_path))),
            session_id: Arc::new(Mutex::new(None)),
            current_turn_id: Arc::new(Mutex::new(None)),
            shared_agent_events: None,
            shared_permission_events: None,
            shared_pending_permissions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub(crate) fn new_shared(client: RuntimeIpcClient, workspace_path: Option<PathBuf>) -> Self {
        let (agent_sender, _) = broadcast::channel(256);
        let (permission_sender, _) = broadcast::channel(64);
        let shared_agent_events = Arc::new(RwLock::new(Some(agent_sender.clone())));
        let shared_permission_events = Arc::new(RwLock::new(Some(permission_sender.clone())));
        let shared_pending_permissions = Arc::new(RwLock::new(HashMap::new()));
        let session_id = Arc::new(Mutex::new(None));
        spawn_shared_event_bridge(
            client.subscribe_events(),
            agent_sender,
            permission_sender,
            shared_agent_events.clone(),
            shared_permission_events.clone(),
            shared_pending_permissions.clone(),
        );
        Self {
            backend: CliAgentRuntimeBackend::Shared(client),
            approval_policy: Arc::new(RwLock::new(CliApprovalPolicy::Ask)),
            workspace_paths: Arc::new(RwLock::new(CliWorkspacePaths::new(workspace_path))),
            session_id,
            current_turn_id: Arc::new(Mutex::new(None)),
            shared_agent_events: Some(shared_agent_events),
            shared_permission_events: Some(shared_permission_events),
            shared_pending_permissions,
        }
    }

    pub(crate) fn is_shared(&self) -> bool {
        matches!(self.backend, CliAgentRuntimeBackend::Shared(_))
    }

    fn embedded_runtime(&self, operation: &str) -> Result<&AgentRuntime> {
        match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => Ok(runtime),
            CliAgentRuntimeBackend::Shared(_) => Err(anyhow::anyhow!(
                "{operation} is not available in the first Shared TUI slice; use default Embedded `bitfun chat`"
            )),
        }
    }

    pub(crate) fn subscribe_events(&self) -> std::result::Result<AgentEventReceiver, RuntimeError> {
        match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => runtime.subscribe_events(),
            CliAgentRuntimeBackend::Shared(_) => shared_receiver(
                self.shared_agent_events.as_ref(),
                "Shared Runtime agent event stream is unavailable",
            ),
        }
    }

    pub(crate) fn subscribe_permission_requests(
        &self,
    ) -> std::result::Result<PermissionRequestEventReceiver, RuntimeError> {
        match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => runtime.subscribe_permission_requests(),
            CliAgentRuntimeBackend::Shared(_) => shared_receiver(
                self.shared_permission_events.as_ref(),
                "Shared Runtime permission event stream is unavailable",
            ),
        }
    }

    pub(crate) fn pending_permission_requests(
        &self,
    ) -> std::result::Result<Vec<PermissionRequest>, RuntimeError> {
        match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => runtime.pending_permission_requests(),
            CliAgentRuntimeBackend::Shared(_) => Ok(self
                .shared_pending_permissions
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .values()
                .cloned()
                .collect()),
        }
    }

    pub(crate) async fn respond_permission(
        &self,
        request_id: &str,
        reply: PermissionReply,
    ) -> Result<()> {
        match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => runtime
                .respond_permission(request_id, reply)
                .await
                .map_err(|error| anyhow::anyhow!(error.into_message())),
            CliAgentRuntimeBackend::Shared(client) => {
                let session_id = self.require_session_id().await?;
                expect_unit(
                    client
                        .request(RuntimeIpcOperation::RespondPermission {
                            session_id,
                            request_id: request_id.to_string(),
                            reply,
                        })
                        .await?,
                    "respond_permission",
                )
            }
        }
    }

    pub(crate) async fn record_completed_local_command_turn(
        &self,
        request: AgentLocalCommandTurnRecordRequest,
    ) -> Result<()> {
        self.embedded_runtime("recording local command turns")?
            .record_completed_local_command_turn(request)
            .await
            .map_err(|error| anyhow::anyhow!(error.into_message()))
    }

    pub(crate) fn set_approval_policy(&self, policy: CliApprovalPolicy) {
        *self
            .approval_policy
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = policy;
    }

    pub(crate) fn approval_policy(&self) -> CliApprovalPolicy {
        *self
            .approval_policy
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) fn workspace_path_buf(&self) -> PathBuf {
        self.workspace_paths
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .execution()
    }

    pub(crate) fn workspace_path_string(&self) -> String {
        self.workspace_path_buf().to_string_lossy().to_string()
    }

    pub(crate) fn project_workspace_path_buf(&self) -> PathBuf {
        self.workspace_paths
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .project()
    }

    pub(crate) fn project_workspace_path_string(&self) -> String {
        self.project_workspace_path_buf()
            .to_string_lossy()
            .to_string()
    }

    pub(crate) fn set_workspace_binding(&self, binding: &AgentSessionWorkspaceBinding) {
        self.workspace_paths
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .apply_binding(binding);
    }

    fn execution_target(&self) -> Option<SessionExecutionTarget> {
        self.workspace_paths
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .execution_target
            .clone()
    }

    fn reset_execution_to_project_workspace(&self) -> PathBuf {
        self.workspace_paths
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .reset_execution_to_project()
    }

    fn current_workspace_path(&self) -> PathBuf {
        self.project_workspace_path_buf()
    }

    async fn list_sessions_in_workspace(
        &self,
        workspace_path: &Path,
    ) -> Result<Vec<AgentSessionSummary>> {
        let request = AgentSessionListRequest {
            workspace_path: workspace_path.to_string_lossy().to_string(),
            remote_connection_id: None,
            remote_ssh_host: None,
        };
        match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => runtime
                .list_sessions(request)
                .await
                .map_err(|error| anyhow::anyhow!(error.into_message())),
            CliAgentRuntimeBackend::Shared(client) => match client
                .request(RuntimeIpcOperation::ListSessions { request })
                .await?
            {
                RuntimeIpcOperationResult::Sessions { sessions } => Ok(sessions),
                _ => Err(unexpected_shared_result("list_sessions")),
            },
        }
    }

    pub(crate) async fn list_sessions(&self) -> Result<Vec<AgentSessionSummary>> {
        let workspace_path = self.current_workspace_path();
        self.list_sessions_in_workspace(&workspace_path).await
    }

    pub(crate) async fn restore_session_in_current_workspace(
        &self,
        session_id: &str,
    ) -> Result<(
        AgentSessionSummary,
        AgentSessionWorkspaceBinding,
        Vec<SessionMigrationNotice>,
        SessionTranscript,
    )> {
        tracing::info!("Restoring session: {}", session_id);

        let project_workspace = self.current_workspace_path();
        let sessions = self.list_sessions_in_workspace(&project_workspace).await?;
        let previous_summary =
            validated_session_summary(&sessions, session_id, &project_workspace)?;

        let (restored, transcript, shared_pending) = match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => {
                let restored = runtime
                    .restore_session(AgentSessionRestoreRequest {
                        workspace_path: project_workspace.to_string_lossy().to_string(),
                        session_id: session_id.to_string(),
                        include_internal: false,
                        remote_connection_id: None,
                        remote_ssh_host: None,
                    })
                    .await
                    .map(|restored| restored.session)
                    .map_err(anyhow::Error::new)
                    .map_err(with_session_conflict_help)?;
                let transcript = runtime
                    .read_session_transcript(SessionTranscriptRequest {
                        session_id: session_id.to_string(),
                        turn_id: None,
                    })
                    .await
                    .unwrap_or_else(|error| {
                        tracing::warn!(
                            "Failed to read Embedded session transcript: {}",
                            error.into_message()
                        );
                        SessionTranscript {
                            session_id: session_id.to_string(),
                            messages: Vec::new(),
                        }
                    });
                (restored, transcript, None)
            }
            CliAgentRuntimeBackend::Shared(client) => match client
                .request(RuntimeIpcOperation::RestoreSession {
                    request: RuntimeSessionRestoreRequest {
                        workspace_path: project_workspace.to_string_lossy().to_string(),
                        session_id: session_id.to_string(),
                    },
                })
                .await
                .map_err(shared_restore_error)?
            {
                RuntimeIpcOperationResult::SessionRestored {
                    session,
                    transcript,
                    pending_permissions,
                } => (session, transcript, Some(pending_permissions)),
                _ => return Err(unexpected_shared_result("restore_session")),
            },
        };

        let binding = self
            .resolve_session_workspace_binding(session_id, &project_workspace)
            .await?;
        let mut session_id_guard = self.session_id.lock().await;
        let mut turn_id_guard = self.current_turn_id.lock().await;
        *session_id_guard = Some(session_id.to_string());
        *turn_id_guard = None;
        drop(session_id_guard);
        drop(turn_id_guard);
        if let Some(requests) = shared_pending {
            let mut pending = self
                .shared_pending_permissions
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            pending.clear();
            pending.extend(
                requests
                    .into_iter()
                    .map(|request| (request.request_id.clone(), request)),
            );
        }

        let migration_notices = session_migration_notices(&previous_summary, &restored);
        Ok((restored, binding, migration_notices, transcript))
    }

    async fn resolve_session_workspace_binding(
        &self,
        session_id: &str,
        fallback_project_workspace: &Path,
    ) -> Result<AgentSessionWorkspaceBinding> {
        let fallback_project = fallback_project_workspace.to_string_lossy().to_string();
        let resolved = match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => runtime
                .resolve_session_workspace_binding(AgentSessionWorkspaceRequest {
                    session_id: session_id.to_string(),
                })
                .await
                .map_err(|error| anyhow::anyhow!(error.into_message()))?,
            CliAgentRuntimeBackend::Shared(_) => None,
        };
        let binding = resolved.unwrap_or_else(|| AgentSessionWorkspaceBinding {
            workspace_id: None,
            workspace_path: fallback_project.clone(),
            project_workspace_path: Some(fallback_project.clone()),
            execution_target: Some(SessionExecutionTarget::local(fallback_project)),
            remote_connection_id: None,
            remote_ssh_host: None,
        });

        self.set_workspace_binding(&binding);
        Ok(binding)
    }

    pub(crate) async fn session_workspace_binding(
        &self,
        session_id: &str,
    ) -> Result<AgentSessionWorkspaceBinding> {
        let project_workspace = self.project_workspace_path_buf();
        self.resolve_session_workspace_binding(session_id, &project_workspace)
            .await
    }

    pub(crate) async fn delete_session(
        &self,
        session_id: &str,
    ) -> std::result::Result<(), SessionOperationError> {
        match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => runtime
                .delete_session(AgentSessionDeleteRequest {
                    workspace_path: self.project_workspace_path_string(),
                    session_id: session_id.to_string(),
                    remote_connection_id: None,
                    remote_ssh_host: None,
                })
                .await
                .map_err(SessionOperationError::runtime),
            CliAgentRuntimeBackend::Shared(client) => {
                let result = client
                    .request(RuntimeIpcOperation::DeleteSession {
                        session_id: session_id.to_string(),
                    })
                    .await
                    .map_err(SessionOperationError::shared)?;
                expect_unit(result, "delete_session").map_err(SessionOperationError::unexpected)
            }
        }
    }

    pub(crate) async fn update_session_model(
        &self,
        session_id: &str,
        model_id: &str,
    ) -> std::result::Result<(), SessionOperationError> {
        let request = AgentSessionModelUpdateRequest {
            session_id: session_id.to_string(),
            model_id: model_id.to_string(),
        };
        match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => runtime
                .update_session_model(request)
                .await
                .map_err(SessionOperationError::runtime),
            CliAgentRuntimeBackend::Shared(client) => {
                let result = client
                    .request(RuntimeIpcOperation::UpdateSessionModel { request })
                    .await
                    .map_err(SessionOperationError::shared)?;
                expect_unit(result, "update_session_model")
                    .map_err(SessionOperationError::unexpected)
            }
        }
    }

    pub(crate) async fn rename_session(
        &self,
        session_id: &str,
        session_name: &str,
    ) -> std::result::Result<(), SessionOperationError> {
        match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => {
                let request = AgentSessionRenameRequest {
                    workspace_path: self.project_workspace_path_string(),
                    session_id: session_id.to_string(),
                    session_name: session_name.to_string(),
                    remote_connection_id: None,
                    remote_ssh_host: None,
                };
                runtime
                    .rename_session(request)
                    .await
                    .map_err(SessionOperationError::runtime)
            }
            CliAgentRuntimeBackend::Shared(client) => {
                let result = client
                    .request(RuntimeIpcOperation::RenameSession {
                        request: RuntimeSessionRenameRequest {
                            session_id: session_id.to_string(),
                            session_name: session_name.to_string(),
                        },
                    })
                    .await
                    .map_err(SessionOperationError::shared)?;
                expect_unit(result, "rename_session").map_err(SessionOperationError::unexpected)
            }
        }
    }

    pub(crate) async fn update_session_mode(
        &self,
        session_id: &str,
        mode_id: &str,
    ) -> std::result::Result<(), SessionOperationError> {
        let request = AgentSessionModeUpdateRequest {
            session_id: session_id.to_string(),
            mode_id: mode_id.to_string(),
        };
        match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => runtime
                .update_session_mode(request)
                .await
                .map_err(SessionOperationError::runtime),
            CliAgentRuntimeBackend::Shared(client) => {
                let result = client
                    .request(RuntimeIpcOperation::UpdateSessionMode { request })
                    .await
                    .map_err(SessionOperationError::shared)?;
                expect_unit(result, "update_session_mode")
                    .map_err(SessionOperationError::unexpected)
            }
        }
    }

    pub(crate) async fn branch_session_at_latest_turn(
        &self,
        source_session_id: &str,
    ) -> Result<AgentSessionForkResult> {
        self.embedded_runtime("forking sessions")?
            .fork_session(AgentSessionForkRequest {
                workspace_path: self.project_workspace_path_string(),
                source_session_id: source_session_id.to_string(),
                remote_connection_id: None,
                remote_ssh_host: None,
            })
            .await
            .map_err(|error| anyhow::anyhow!(error.into_message()))
    }

    pub(crate) async fn fork_current_session(
        &self,
        before_turn_id: Option<&str>,
    ) -> Result<(
        AgentSessionSummary,
        AgentSessionWorkspaceBinding,
        SessionTranscript,
    )> {
        let source_session_id = self.require_session_id().await?;
        let workspace_path = self.project_workspace_path_string();
        let (session, transcript) = match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => {
                let forked = match before_turn_id {
                    Some(source_turn_id) => {
                        runtime
                            .fork_session_before_turn(AgentSessionForkBeforeTurnRequest {
                                workspace_path: workspace_path.clone(),
                                source_session_id,
                                source_turn_id: source_turn_id.to_string(),
                                remote_connection_id: None,
                                remote_ssh_host: None,
                            })
                            .await
                    }
                    None => {
                        runtime
                            .fork_session(AgentSessionForkRequest {
                                workspace_path: workspace_path.clone(),
                                source_session_id,
                                remote_connection_id: None,
                                remote_ssh_host: None,
                            })
                            .await
                    }
                }
                .map_err(|error| anyhow::anyhow!(error.into_message()))?;
                let restored = runtime
                    .restore_session(AgentSessionRestoreRequest {
                        workspace_path: workspace_path.clone(),
                        session_id: forked.session_id.clone(),
                        include_internal: false,
                        remote_connection_id: None,
                        remote_ssh_host: None,
                    })
                    .await
                    .map_err(|error| anyhow::anyhow!(error.into_message()))?;
                let transcript = runtime
                    .read_session_transcript(SessionTranscriptRequest {
                        session_id: forked.session_id,
                        turn_id: None,
                    })
                    .await
                    .map_err(|error| anyhow::anyhow!(error.into_message()))?;
                (restored.session, transcript)
            }
            CliAgentRuntimeBackend::Shared(client) => match client
                .request(RuntimeIpcOperation::ForkSession {
                    request: RuntimeSessionForkRequest {
                        session_id: source_session_id,
                        before_turn_id: before_turn_id.map(str::to_string),
                    },
                })
                .await?
            {
                RuntimeIpcOperationResult::SessionForked {
                    session,
                    transcript,
                } => (session, transcript),
                _ => return Err(unexpected_shared_result("fork_session")),
            },
        };

        let binding = self
            .resolve_session_workspace_binding(&session.session_id, Path::new(&workspace_path))
            .await?;
        *self.session_id.lock().await = Some(session.session_id.clone());
        *self.current_turn_id.lock().await = None;
        self.shared_pending_permissions
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
        Ok((session, binding, transcript))
    }

    pub(crate) async fn revert_current_session(
        &self,
        undo: bool,
    ) -> Result<AgentSessionRevertResult> {
        let session_id = self.require_session_id().await?;
        let request = AgentSessionRevertRequest {
            workspace_path: self.project_workspace_path_string(),
            session_id: session_id.clone(),
            remote_connection_id: None,
            remote_ssh_host: None,
        };
        let locally_active_turn_id = self.current_turn_id.lock().await.clone();
        let mut reverted = match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => if undo {
                runtime.undo_session(request).await
            } else {
                runtime.redo_session(request).await
            }
            .map_err(|error| anyhow::anyhow!(error.into_message()))?,
            CliAgentRuntimeBackend::Shared(client) => {
                let operation = if undo {
                    RuntimeIpcOperation::UndoSession { request }
                } else {
                    RuntimeIpcOperation::RedoSession { request }
                };
                match client.request(operation).await? {
                    RuntimeIpcOperationResult::SessionReverted { revert }
                        if revert.session_id == session_id =>
                    {
                        revert
                    }
                    _ => return Err(unexpected_shared_result("revert_session")),
                }
            }
        };
        if reverted.session_id != session_id {
            return Err(anyhow::anyhow!(
                "Runtime reverted an unexpected session identity"
            ));
        }
        if let Some(turn_id) = locally_active_turn_id {
            if !reverted.retired_turn_ids.contains(&turn_id) {
                reverted.retired_turn_ids.push(turn_id);
            }
        }
        *self.current_turn_id.lock().await = None;
        Ok(reverted)
    }

    pub(crate) async fn workspace_diff(&self) -> Result<WorkspaceDiffSnapshot> {
        if let Some(reason) = self
            .workspace_paths
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .workspace_diff_unavailable_reason()
        {
            return Err(anyhow::anyhow!(reason));
        }
        match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => runtime
                .workspace_diff()
                .await
                .map_err(|error| anyhow::anyhow!(error.into_message())),
            CliAgentRuntimeBackend::Shared(client) => {
                match client.request(RuntimeIpcOperation::WorkspaceDiff).await? {
                    RuntimeIpcOperationResult::WorkspaceDiff { snapshot } => Ok(snapshot),
                    _ => Err(unexpected_shared_result("workspace_diff")),
                }
            }
        }
    }

    pub(crate) async fn generate_session_usage_report(
        &self,
        request: AgentSessionUsageRequest,
    ) -> Result<SessionUsageReport> {
        self.embedded_runtime("generating session usage")?
            .generate_session_usage(request)
            .await
            .map_err(|error| anyhow::anyhow!(error.into_message()))
    }

    pub(crate) async fn wait_for_turn_settlement(
        &self,
        session_id: &str,
        turn_id: &str,
        wait_timeout_ms: u64,
    ) -> std::result::Result<(), RuntimeError> {
        match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => {
                runtime
                    .wait_for_turn_settlement(AgentTurnSettlementRequest {
                        session_id: session_id.to_string(),
                        turn_id: turn_id.to_string(),
                        wait_timeout_ms,
                    })
                    .await
            }
            CliAgentRuntimeBackend::Shared(_) => Err(RuntimeError::Port(PortError::new(
                PortErrorKind::NotAvailable,
                "turn settlement waiting is not available in the first Shared TUI slice",
            ))),
        }
    }

    fn build_default_session_name() -> String {
        format!(
            "CLI Session - {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        )
    }

    fn is_session_not_found_error(error: &RuntimeError) -> bool {
        matches!(
            error,
            RuntimeError::Port(port_error) if port_error.kind == PortErrorKind::NotFound
        )
    }

    async fn recreate_session_with_id(&self, session_id: &str, agent_type: &str) -> Result<()> {
        let runtime = self.embedded_runtime("recreating sessions with fixed identifiers")?;
        let mut session_name = Self::build_default_session_name();
        let mut effective_agent_type = agent_type.to_string();

        let workspace = self.workspace_path_buf();
        let project_workspace = self.project_workspace_path_buf();
        if let Ok(sessions) = runtime
            .list_sessions(AgentSessionListRequest {
                workspace_path: project_workspace.to_string_lossy().to_string(),
                remote_connection_id: None,
                remote_ssh_host: None,
            })
            .await
        {
            if let Some(summary) = sessions.iter().find(|s| s.session_id == session_id) {
                session_name = summary.session_name.clone();
                effective_agent_type = summary.agent_type.clone();
            }
        }

        runtime
            .create_session_with_id(
                session_id.to_string(),
                AgentSessionCreateRequest {
                    session_name,
                    agent_type: effective_agent_type,
                    workspace_path: Some(workspace.to_string_lossy().to_string()),
                    project_workspace_path: Some(project_workspace.to_string_lossy().to_string()),
                    execution_target: self.execution_target(),
                    workspace_id: None,
                    remote_connection_id: None,
                    remote_ssh_host: None,
                    model_id: None,
                    metadata: serde_json::Map::new(),
                },
            )
            .await
            .map_err(anyhow::Error::new)
            .map_err(with_session_conflict_help)?;

        tracing::info!("Recreated backend session with existing id: {}", session_id);
        Ok(())
    }

    async fn ensure_backend_session_alive(&self, session_id: &str, agent_type: &str) -> Result<()> {
        let runtime = self.embedded_runtime("recovering Embedded sessions")?;
        let project_workspace = self.project_workspace_path_buf();
        match runtime
            .restore_session(AgentSessionRestoreRequest {
                workspace_path: project_workspace.to_string_lossy().to_string(),
                session_id: session_id.to_string(),
                include_internal: false,
                remote_connection_id: None,
                remote_ssh_host: None,
            })
            .await
        {
            Ok(_) => {
                self.resolve_session_workspace_binding(session_id, &project_workspace)
                    .await?;
                tracing::info!("Backend session restored: {}", session_id);
                Ok(())
            }
            Err(error) => {
                let session_not_found = Self::is_session_not_found_error(&error);
                if session_not_found {
                    tracing::warn!(
                        "Session is unavailable, recreating backend session: {}",
                        session_id
                    );
                    self.recreate_session_with_id(session_id, agent_type).await
                } else {
                    Err(with_session_conflict_help(anyhow::Error::new(error)))
                }
            }
        }
    }

    pub(crate) async fn create_session_with_id(
        &self,
        session_id: String,
        agent_type: &str,
    ) -> Result<String> {
        let runtime = self.embedded_runtime("creating sessions with fixed identifiers")?;
        let mut session_id_guard = self.session_id.lock().await;
        let workspace_path = self.workspace_path_string();
        let project_workspace_path = self.project_workspace_path_string();

        let session = runtime
            .create_session_with_id(
                session_id,
                AgentSessionCreateRequest {
                    session_name: Self::build_default_session_name(),
                    agent_type: agent_type.to_string(),
                    workspace_path: Some(workspace_path),
                    project_workspace_path: Some(project_workspace_path),
                    execution_target: self.execution_target(),
                    workspace_id: None,
                    remote_connection_id: None,
                    remote_ssh_host: None,
                    model_id: None,
                    metadata: serde_json::Map::new(),
                },
            )
            .await
            .map_err(anyhow::Error::new)
            .map_err(with_session_conflict_help)?;

        let id = session.session_id.clone();
        *session_id_guard = Some(id.clone());
        tracing::info!("Created runtime session with fixed id: {}", id);

        Ok(id)
    }
}

impl CliAgentRuntimeClient {
    pub(crate) async fn ensure_session(&self, agent_type: &str) -> Result<String> {
        let mut session_id_guard = self.session_id.lock().await;

        if let Some(ref id) = *session_id_guard {
            return Ok(id.clone());
        }

        let request = AgentSessionCreateRequest {
            session_name: Self::build_default_session_name(),
            agent_type: agent_type.to_string(),
            workspace_path: Some(self.workspace_path_string()),
            project_workspace_path: None,
            execution_target: None,
            workspace_id: None,
            remote_connection_id: None,
            remote_ssh_host: None,
            model_id: None,
            metadata: serde_json::Map::new(),
        };
        let session = match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => runtime
                .create_session(request)
                .await
                .map_err(|error| anyhow::anyhow!(error.into_message()))?,
            CliAgentRuntimeBackend::Shared(client) => match client
                .request(RuntimeIpcOperation::CreateSession { request })
                .await?
            {
                RuntimeIpcOperationResult::SessionCreated { session } => session,
                _ => return Err(unexpected_shared_result("create_session")),
            },
        };

        let id = session.session_id.clone();

        *session_id_guard = Some(id.clone());
        drop(session_id_guard);
        self.refresh_shared_pending_permissions().await?;
        tracing::info!("Created core session: {}", id);

        Ok(id)
    }

    pub(crate) async fn start_session_compaction(&self, session_id: &str) -> Result<String> {
        let turn_id = uuid::Uuid::new_v4().to_string();
        let request = AgentSessionCompactionRequest {
            session_id: session_id.to_string(),
            turn_id: turn_id.clone(),
        };
        *self.current_turn_id.lock().await = Some(turn_id.clone());

        let submission: Result<String> = async {
            match &self.backend {
                CliAgentRuntimeBackend::Embedded(runtime) => {
                    let accepted = runtime
                        .start_session_compaction(request)
                        .await
                        .map_err(|error| anyhow::anyhow!(error.into_message()))?;
                    if accepted.session_id != session_id || accepted.turn_id != turn_id {
                        return Err(anyhow::anyhow!(
                            "Runtime accepted manual compaction with an unexpected identity"
                        ));
                    }
                    Ok(accepted.turn_id)
                }
                CliAgentRuntimeBackend::Shared(client) => match client
                    .request(RuntimeIpcOperation::CompactSession { request })
                    .await?
                {
                    RuntimeIpcOperationResult::TurnAccepted {
                        session_id: accepted_session,
                        turn_id: accepted_turn,
                    } if accepted_session == session_id && accepted_turn == turn_id => {
                        Ok(accepted_turn)
                    }
                    _ => return Err(unexpected_shared_result("compact_session")),
                },
            }
        }
        .await;
        if submission.is_err() {
            *self.current_turn_id.lock().await = None;
        }
        submission
    }

    pub(crate) async fn send_message(&self, message: String, agent_type: &str) -> Result<String> {
        self.send_message_with_context(message, Vec::new(), Vec::new(), agent_type)
            .await
    }

    pub(crate) async fn send_message_with_context(
        &self,
        message: String,
        workspace_references: Vec<AgentWorkspaceReference>,
        attachments: Vec<AgentInputAttachment>,
        agent_type: &str,
    ) -> Result<String> {
        if !attachments.is_empty() && self.is_shared() {
            return Err(anyhow::anyhow!(
                crate::actions::shared_tui_image_attachment_error()
            ));
        }
        let session_id = self.ensure_session(agent_type).await?;
        self.submit_dialog_turn_request(
            session_id,
            message,
            None,
            workspace_references,
            attachments,
            AgentDialogTurnExecution::Standard,
            agent_type,
        )
        .await
    }

    pub(crate) async fn send_external_subagent_command(
        &self,
        prompt: String,
        original_command: String,
        ecosystem_id: String,
        logical_id: String,
        agent_type: &str,
    ) -> Result<String> {
        if self.is_shared() {
            return Err(anyhow::anyhow!(
                "External subagent commands require Embedded TUI; Shared TUI does not transport delegated command submissions"
            ));
        }
        let session_id = self.ensure_session(agent_type).await?;
        self.submit_dialog_turn_request(
            session_id,
            prompt,
            Some(original_command),
            Vec::new(),
            Vec::new(),
            AgentDialogTurnExecution::FreshExternalSubagent {
                ecosystem_id,
                logical_id,
            },
            agent_type,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn submit_dialog_turn_request(
        &self,
        session_id: String,
        message: String,
        original_message: Option<String>,
        workspace_references: Vec<AgentWorkspaceReference>,
        attachments: Vec<AgentInputAttachment>,
        execution: AgentDialogTurnExecution,
        agent_type: &str,
    ) -> Result<String> {
        tracing::info!("Sending message to session {}: {}", session_id, message);

        // Generate a turn_id
        let turn_id = uuid::Uuid::new_v4().to_string();

        // Store current turn_id for cancellation
        {
            let mut turn_guard = self.current_turn_id.lock().await;
            *turn_guard = Some(turn_id.clone());
        }

        // Start the dialog turn; events arrive through the shared broadcast source.
        let mut metadata = approval_metadata(self.approval_policy());
        put_agent_workspace_references(&mut metadata, &workspace_references)
            .map_err(|error| anyhow::anyhow!(error.message))?;
        let request = AgentDialogTurnRequest {
            session_id: session_id.clone(),
            message: message.clone(),
            original_message,
            turn_id: Some(turn_id.clone()),
            execution,
            agent_type: agent_type.to_string(),
            // Dialog submission uses this path to locate persisted session
            // state. Execution still comes from the session's resolved binding.
            workspace_path: Some(self.project_workspace_path_string()),
            remote_connection_id: None,
            remote_ssh_host: None,
            policy: DialogSubmissionPolicy::for_source(AgentSubmissionSource::Cli),
            reply_route: None,
            prepended_reminders: Vec::new(),
            attachments,
            metadata,
        };
        let submission: Result<String> = async {
            match &self.backend {
                CliAgentRuntimeBackend::Embedded(runtime) => {
                    let start_result = runtime.submit_dialog_turn(request.clone()).await;
                    if let Err(err) = start_result {
                        let session_not_found = Self::is_session_not_found_error(&err);
                        let error_message = err.into_message();
                        if session_not_found {
                            tracing::warn!(
                                "Session missing when starting turn, attempting recovery and retry: session_id={}, error={}",
                                session_id,
                                error_message
                            );
                            self.ensure_backend_session_alive(&session_id, agent_type)
                                .await?;
                            runtime
                                .submit_dialog_turn(request)
                                .await
                                .map_err(|error| anyhow::anyhow!(error.into_message()))?;
                        } else {
                            return Err(anyhow::anyhow!(error_message));
                        }
                    }
                    Ok(turn_id)
                }
                CliAgentRuntimeBackend::Shared(client) => match client
                    .request(RuntimeIpcOperation::SubmitTurn { request })
                    .await?
                {
                    RuntimeIpcOperationResult::TurnAccepted {
                        session_id: accepted_session,
                        turn_id: accepted_turn,
                    } if accepted_session == session_id => {
                        *self.current_turn_id.lock().await = Some(accepted_turn.clone());
                        Ok(accepted_turn)
                    }
                    _ => Err(unexpected_shared_result("submit_turn")),
                },
            }
        }
        .await;
        if submission.is_err() {
            *self.current_turn_id.lock().await = None;
        }
        submission
    }

    pub(crate) async fn run_user_shell_command(
        &self,
        command: String,
        agent_type: &str,
    ) -> Result<String> {
        let session_id = self.ensure_session(agent_type).await?;
        let turn_id = uuid::Uuid::new_v4().to_string();
        let request = AgentUserShellCommandRequest {
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            command,
        };
        *self.current_turn_id.lock().await = Some(turn_id.clone());

        let submission: Result<String> = async {
            match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => {
                let accepted = match runtime.run_user_shell_command(request.clone()).await {
                    Ok(accepted) => accepted,
                    Err(error) if Self::is_session_not_found_error(&error) => {
                        tracing::warn!(
                            "Session missing when starting Shell turn, attempting recovery and retry: session_id={}",
                            session_id
                        );
                        self.ensure_backend_session_alive(&session_id, agent_type)
                            .await?;
                        runtime
                            .run_user_shell_command(request)
                            .await
                            .map_err(|error| anyhow::anyhow!(error.into_message()))?
                    }
                    Err(error) => return Err(anyhow::anyhow!(error.into_message())),
                };
                if accepted.session_id == session_id && accepted.turn_id == turn_id {
                    Ok(accepted.turn_id)
                } else {
                    Err(anyhow::anyhow!(
                        "Runtime accepted a Shell command with an unexpected identity"
                    ))
                }
            }
            CliAgentRuntimeBackend::Shared(client) => match client
                .request(RuntimeIpcOperation::RunUserShellCommand { request })
                .await
            {
                Ok(RuntimeIpcOperationResult::TurnAccepted {
                    session_id: accepted_session,
                    turn_id: accepted_turn,
                }) if accepted_session == session_id && accepted_turn == turn_id => {
                    Ok(accepted_turn)
                }
                Ok(_) => Err(unexpected_shared_result("run_user_shell_command")),
                Err(error) => Err(anyhow::Error::new(error)),
            },
            }
        }
        .await;
        if submission.is_err() {
            *self.current_turn_id.lock().await = None;
        }
        submission
    }

    pub(crate) async fn search_workspace_references(
        &self,
        query: String,
    ) -> Result<AgentWorkspaceReferenceSearchResult> {
        let session_id = self
            .session_id
            .lock()
            .await
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active session"))?;
        let request = AgentWorkspaceReferenceSearchRequest {
            session_id,
            query,
            limit: 20,
        };
        match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => runtime
                .search_workspace_references(request)
                .await
                .map_err(|error| anyhow::anyhow!(error.into_message())),
            CliAgentRuntimeBackend::Shared(client) => match client
                .request(RuntimeIpcOperation::SearchWorkspaceReferences { request })
                .await?
            {
                RuntimeIpcOperationResult::WorkspaceReferenceSearch { search } => Ok(search),
                _ => Err(unexpected_shared_result("search_workspace_references")),
            },
        }
    }

    pub(crate) async fn workspace_references_for_message(
        &self,
        session_id: String,
        message_id: String,
    ) -> Result<Vec<AgentWorkspaceReference>> {
        let request = AgentMessageWorkspaceReferencesRequest {
            session_id,
            message_id,
        };
        match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => runtime
                .workspace_references_for_message(request)
                .await
                .map_err(|error| anyhow::anyhow!(error.into_message())),
            CliAgentRuntimeBackend::Shared(client) => match client
                .request(RuntimeIpcOperation::WorkspaceReferencesForMessage { request })
                .await?
            {
                RuntimeIpcOperationResult::WorkspaceReferences { references } => Ok(references),
                _ => Err(unexpected_shared_result("workspace_references_for_message")),
            },
        }
    }

    pub(crate) async fn cancel_current_turn(&self) -> Result<()> {
        let session_id = self.session_id.lock().await.clone();
        let turn_id = self.current_turn_id.lock().await.clone();

        if let (Some(session_id), Some(turn_id)) = (session_id, turn_id) {
            tracing::info!("Cancelling turn: session={}, turn={}", session_id, turn_id);
            let request = AgentTurnCancellationRequest {
                session_id,
                turn_id: Some(turn_id.clone()),
                source: Some(AgentSubmissionSource::Cli),
                requester_session_id: None,
                reason: Some("user_cancelled".to_string()),
                wait_timeout_ms: None,
            };
            match &self.backend {
                CliAgentRuntimeBackend::Embedded(runtime) => {
                    runtime
                        .cancel_turn(request)
                        .await
                        .map_err(|error| anyhow::anyhow!(error.into_message()))?;
                }
                CliAgentRuntimeBackend::Shared(client) => match client
                    .request(RuntimeIpcOperation::CancelTurn { request })
                    .await?
                {
                    RuntimeIpcOperationResult::TurnCancelled { .. } => {}
                    _ => return Err(unexpected_shared_result("cancel_turn")),
                },
            }

            let mut turn_id_guard = self.current_turn_id.lock().await;
            if turn_id_guard.as_deref() == Some(turn_id.as_str()) {
                *turn_id_guard = None;
            }
        }

        Ok(())
    }

    pub(crate) async fn create_new_session(&self, agent_type: &str) -> Result<String> {
        let project_workspace = self.reset_execution_to_project_workspace();
        let project_workspace_path = project_workspace.to_string_lossy().to_string();
        let request = AgentSessionCreateRequest {
            session_name: Self::build_default_session_name(),
            agent_type: agent_type.to_string(),
            workspace_path: Some(project_workspace_path.clone()),
            project_workspace_path: Some(project_workspace_path.clone()),
            execution_target: Some(SessionExecutionTarget::local(project_workspace_path)),
            workspace_id: None,
            remote_connection_id: None,
            remote_ssh_host: None,
            model_id: None,
            metadata: serde_json::Map::new(),
        };
        let session = match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => runtime
                .create_session(request)
                .await
                .map_err(|error| anyhow::anyhow!(error.into_message()))?,
            CliAgentRuntimeBackend::Shared(client) => match client
                .request(RuntimeIpcOperation::CreateSession { request })
                .await?
            {
                RuntimeIpcOperationResult::SessionCreated { session } => session,
                _ => return Err(unexpected_shared_result("create_session")),
            },
        };

        let id = session.session_id.clone();

        *self.session_id.lock().await = Some(id.clone());
        *self.current_turn_id.lock().await = None;
        self.shared_pending_permissions
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
        tracing::info!("Created new core session: {}", id);

        Ok(id)
    }

    pub(crate) async fn restore_session(&self, session_id: &str) -> Result<()> {
        self.restore_session_in_current_workspace(session_id)
            .await?;
        Ok(())
    }

    pub(crate) async fn submit_user_answers(
        &self,
        tool_id: &str,
        answers: serde_json::Value,
    ) -> Result<()> {
        tracing::info!("Submitting user answers for tool: {}", tool_id);
        match &self.backend {
            CliAgentRuntimeBackend::Embedded(runtime) => runtime
                .submit_user_answers(AgentUserAnswersRequest {
                    tool_id: tool_id.to_string(),
                    answers,
                })
                .await
                .map_err(|e| anyhow::anyhow!("Submit user answers failed: {}", e.into_message())),
            CliAgentRuntimeBackend::Shared(client) => {
                let session_id = self.require_session_id().await?;
                expect_unit(
                    client
                        .request(RuntimeIpcOperation::SubmitUserAnswers {
                            request: RuntimeUserAnswersRequest {
                                session_id,
                                tool_id: tool_id.to_string(),
                                answers,
                            },
                        })
                        .await?,
                    "submit_user_answers",
                )
            }
        }
    }

    async fn require_session_id(&self) -> Result<String> {
        self.session_id
            .lock()
            .await
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Shared TUI has no attached session"))
    }

    async fn refresh_shared_pending_permissions(&self) -> Result<()> {
        let CliAgentRuntimeBackend::Shared(client) = &self.backend else {
            return Ok(());
        };
        let session_id = self.require_session_id().await?;
        let RuntimeIpcOperationResult::PendingPermissions { requests } = client
            .request(RuntimeIpcOperation::PendingPermissions { session_id })
            .await?
        else {
            return Err(unexpected_shared_result("pending_permissions"));
        };
        let mut pending = self
            .shared_pending_permissions
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        pending.clear();
        pending.extend(
            requests
                .into_iter()
                .map(|request| (request.request_id.clone(), request)),
        );
        Ok(())
    }
}

fn shared_receiver<T: Clone>(
    source: Option<&SharedBroadcast<T>>,
    message: &str,
) -> std::result::Result<broadcast::Receiver<T>, RuntimeError> {
    source
        .and_then(|source| {
            source
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_ref()
                .map(broadcast::Sender::subscribe)
        })
        .ok_or_else(|| RuntimeError::Port(PortError::new(PortErrorKind::NotAvailable, message)))
}

fn spawn_shared_event_bridge(
    mut source: broadcast::Receiver<RuntimeIpcClientEvent>,
    agent_sender: broadcast::Sender<AgenticEventEnvelope>,
    permission_sender: broadcast::Sender<bitfun_agent_runtime::sdk::PermissionRequestEvent>,
    agent_owner: SharedBroadcast<AgenticEventEnvelope>,
    permission_owner: SharedBroadcast<bitfun_agent_runtime::sdk::PermissionRequestEvent>,
    pending: Arc<RwLock<HashMap<String, PermissionRequest>>>,
) {
    tokio::spawn(async move {
        loop {
            match source.recv().await {
                Ok(RuntimeIpcClientEvent::Runtime(RuntimeIpcEvent::Agent { envelope, .. })) => {
                    let _ = agent_sender.send(envelope);
                }
                Ok(RuntimeIpcClientEvent::Runtime(RuntimeIpcEvent::Permission {
                    session_id,
                    mut event,
                })) => {
                    project_routed_permission_event(&mut event, &session_id);
                    match &event {
                        bitfun_agent_runtime::sdk::PermissionRequestEvent::Asked { request } => {
                            pending
                                .write()
                                .unwrap_or_else(|poisoned| poisoned.into_inner())
                                .insert(request.request_id.clone(), request.clone());
                        }
                        bitfun_agent_runtime::sdk::PermissionRequestEvent::Replied {
                            request_id,
                            ..
                        }
                        | bitfun_agent_runtime::sdk::PermissionRequestEvent::Cancelled {
                            request_id,
                            ..
                        } => {
                            pending
                                .write()
                                .unwrap_or_else(|poisoned| poisoned.into_inner())
                                .remove(request_id);
                        }
                    }
                    let _ = permission_sender.send(event);
                }
                Ok(RuntimeIpcClientEvent::Runtime(RuntimeIpcEvent::StreamInvalidated {
                    reason,
                })) => {
                    let event = AgenticEvent::SystemError {
                        session_id: None,
                        error: shared_disconnect_message(Some(reason)),
                        recoverable: false,
                    };
                    let _ = agent_sender.send(AgenticEventEnvelope::new(
                        event,
                        bitfun_events::AgenticEventPriority::Critical,
                    ));
                    break;
                }
                Ok(RuntimeIpcClientEvent::Disconnected)
                | Err(broadcast::error::RecvError::Closed)
                | Err(broadcast::error::RecvError::Lagged(_)) => {
                    let event = AgenticEvent::SystemError {
                        session_id: None,
                        error: shared_disconnect_message(None),
                        recoverable: false,
                    };
                    let _ = agent_sender.send(AgenticEventEnvelope::new(
                        event,
                        bitfun_events::AgenticEventPriority::Critical,
                    ));
                    break;
                }
            }
        }
        *agent_owner
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        *permission_owner
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    });
}

fn shared_disconnect_message(reason: Option<RuntimeIpcStreamInvalidationReason>) -> String {
    if reason == Some(RuntimeIpcStreamInvalidationReason::FrameTooLarge) {
        format!(
            "Shared Runtime event exceeded the supported size; active-turn cancellation was requested. {SHARED_TUI_EMBEDDED_HANDOFF}."
        )
    } else {
        "Shared Runtime connection was lost; this view is no longer authoritative".to_string()
    }
}

fn project_routed_permission_event(
    event: &mut bitfun_agent_runtime::sdk::PermissionRequestEvent,
    routed_session_id: &str,
) {
    let bitfun_agent_runtime::sdk::PermissionRequestEvent::Asked { request } = event else {
        return;
    };
    if request.session_id == routed_session_id {
        return;
    }
    if let Some(delegation) = request.delegation.as_mut() {
        delegation.parent_session_id = routed_session_id.to_string();
    }
}

pub(super) fn expect_unit(result: RuntimeIpcOperationResult, operation: &str) -> Result<()> {
    match result {
        RuntimeIpcOperationResult::Unit => Ok(()),
        _ => Err(unexpected_shared_result(operation)),
    }
}

fn unexpected_shared_result(operation: &str) -> anyhow::Error {
    anyhow::anyhow!("Shared Runtime returned an unexpected result for {operation}")
}

#[cfg(test)]
mod recovery_tests {
    use bitfun_agent_runtime::sdk::{PortError, PortErrorKind, RuntimeError};

    use super::CliAgentRuntimeClient;

    #[test]
    fn session_recovery_requires_structured_not_found_error() {
        let missing_session =
            RuntimeError::Port(PortError::new(PortErrorKind::NotFound, "session not found"));
        let unrelated_backend_error =
            RuntimeError::Port(PortError::new(PortErrorKind::Backend, "model not found"));

        assert!(CliAgentRuntimeClient::is_session_not_found_error(
            &missing_session
        ));
        assert!(!CliAgentRuntimeClient::is_session_not_found_error(
            &unrelated_backend_error
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use bitfun_runtime_ports::{
        AgentSessionSummary, AgentSessionWorkspaceBinding, SessionExecutionTarget,
        SessionExecutionTargetKind, WorktreeLifecycle,
    };

    use bitfun_agent_runtime::sdk::{
        PermissionDelegationContext, PermissionRequest, PermissionRequestEvent,
        PermissionRequestSource, PermissionRequestSourceKind, PortError, PortErrorKind,
        RuntimeError,
    };
    use bitfun_agent_runtime_ipc::{RuntimeIpcClientError, RuntimeIpcError, RuntimeIpcErrorCode};

    use super::{
        project_routed_permission_event, session_migration_notices, shared_disconnect_message,
        shared_restore_error, validated_session_summary, CliWorkspacePaths, SessionMigrationNotice,
        SessionOperationError,
    };
    use bitfun_agent_runtime_ipc::RuntimeIpcStreamInvalidationReason;

    #[test]
    fn oversized_shared_restore_explains_the_embedded_handoff() {
        let error = shared_restore_error(RuntimeIpcClientError::Remote(RuntimeIpcError {
            code: RuntimeIpcErrorCode::FrameTooLarge,
            message: "response too large".to_string(),
        }));
        let message = error.to_string();
        assert!(message.contains("history is too large"));
        assert!(message.contains("default Embedded `bitfun chat`"));
    }

    #[test]
    fn shared_session_update_preserves_unknown_outcome_as_a_typed_fact() {
        let error = SessionOperationError::shared(RuntimeIpcClientError::Remote(RuntimeIpcError {
            code: RuntimeIpcErrorCode::OutcomeUnknown,
            message: "inspect authoritative state before retrying".to_string(),
        }));

        assert!(error.outcome_unknown());
        assert!(error.to_string().contains("OutcomeUnknown"));

        for transport_error in [
            RuntimeIpcClientError::Timeout,
            RuntimeIpcClientError::Disconnected,
            RuntimeIpcClientError::UnexpectedResponse,
        ] {
            assert!(SessionOperationError::shared(transport_error).outcome_unknown());
        }
        assert!(
            SessionOperationError::unexpected(anyhow::anyhow!("unexpected response shape"))
                .outcome_unknown()
        );
        assert!(
            !SessionOperationError::shared(RuntimeIpcClientError::Remote(RuntimeIpcError {
                code: RuntimeIpcErrorCode::InvalidRequest,
                message: "unknown mode".to_string(),
            },))
            .outcome_unknown()
        );
        assert!(
            !SessionOperationError::shared(RuntimeIpcClientError::RequestEncoding(
                bitfun_agent_runtime_ipc::RuntimeIpcIoError::FrameTooLarge {
                    size: 129,
                    max_bytes: 128,
                },
            ))
            .outcome_unknown()
        );
    }

    #[test]
    fn embedded_runtime_unknown_outcome_is_preserved() {
        let error = SessionOperationError::runtime(RuntimeError::Port(PortError::new(
            PortErrorKind::OutcomeUnknown,
            "inspect authoritative state",
        )));

        assert!(error.outcome_unknown());
    }

    #[test]
    fn oversized_shared_event_explains_cancellation_and_handoff() {
        let message =
            shared_disconnect_message(Some(RuntimeIpcStreamInvalidationReason::FrameTooLarge));
        assert!(message.contains("cancellation was requested"));
        assert!(message.contains("default Embedded `bitfun chat`"));
    }

    #[test]
    fn workspace_paths_keep_project_and_execution_roots_separate() {
        let mut paths = CliWorkspacePaths::new(Some("/project".into()));
        let binding = AgentSessionWorkspaceBinding {
            workspace_id: Some("workspace-1".to_string()),
            workspace_path: "/managed-worktree".to_string(),
            project_workspace_path: Some("/project".to_string()),
            execution_target: Some(SessionExecutionTarget {
                kind: SessionExecutionTargetKind::ManagedWorktree,
                worktree_id: Some("worktree-1".to_string()),
                root_path: "/managed-worktree".to_string(),
                base_ref: Some("main".to_string()),
                base_commit: Some("123456789abcdef".to_string()),
                branch: None,
                lifecycle: Some(WorktreeLifecycle::Managed),
            }),
            remote_connection_id: None,
            remote_ssh_host: None,
        };

        paths.apply_binding(&binding);

        assert_eq!(paths.project(), Path::new("/project"));
        assert_eq!(paths.execution(), Path::new("/managed-worktree"));
        assert_eq!(
            paths
                .execution_target
                .as_ref()
                .and_then(|target| target.worktree_id.as_deref()),
            Some("worktree-1")
        );

        assert_eq!(
            paths.reset_execution_to_project(),
            Path::new("/project").to_path_buf()
        );
        assert_eq!(paths.execution(), Path::new("/project"));
        assert!(paths
            .execution_target
            .as_ref()
            .and_then(|target| target.worktree_id.as_ref())
            .is_none());
    }

    #[test]
    fn workspace_diff_fails_closed_for_other_worktrees_and_remote_sessions() {
        let mut paths = CliWorkspacePaths::new(Some("/project".into()));
        assert_eq!(paths.workspace_diff_unavailable_reason(), None);

        paths.apply_binding(&AgentSessionWorkspaceBinding {
            workspace_id: Some("workspace-1".to_string()),
            workspace_path: "/managed-worktree".to_string(),
            project_workspace_path: Some("/project".to_string()),
            execution_target: Some(SessionExecutionTarget {
                kind: SessionExecutionTargetKind::ManagedWorktree,
                worktree_id: Some("worktree-1".to_string()),
                root_path: "/managed-worktree".to_string(),
                base_ref: Some("main".to_string()),
                base_commit: Some("123456789abcdef".to_string()),
                branch: None,
                lifecycle: Some(WorktreeLifecycle::Managed),
            }),
            remote_connection_id: None,
            remote_ssh_host: None,
        });
        assert!(paths
            .workspace_diff_unavailable_reason()
            .is_some_and(|reason| reason.contains("different worktree")));

        paths.apply_binding(&AgentSessionWorkspaceBinding {
            workspace_id: None,
            workspace_path: "/project".to_string(),
            project_workspace_path: Some("/project".to_string()),
            execution_target: Some(SessionExecutionTarget::local("/project")),
            remote_connection_id: Some("remote-1".to_string()),
            remote_ssh_host: Some("example.test".to_string()),
        });
        assert!(paths
            .workspace_diff_unavailable_reason()
            .is_some_and(|reason| reason.contains("remote Sessions")));
    }

    #[test]
    fn model_updates_use_the_runtime_sdk_without_the_core_compatibility_facade() {
        let source = include_str!("runtime_client.rs").replace("\r\n", "\n");
        let compatibility_update =
            ["self.compatibility", "\n            .update_session_model"].concat();

        assert!(source.contains("CliAgentRuntimeBackend::Embedded(runtime)"));
        assert!(source.contains("runtime.update_session_model(request)"));
        assert!(source.contains("CliAgentRuntimeBackend::Shared(client)"));
        assert!(source.contains("RuntimeIpcOperation::UpdateSessionModel { request }"));
        assert!(!source.contains(&compatibility_update));
    }

    #[test]
    fn session_rename_uses_direct_runtime_or_private_shared_ipc() {
        let source = include_str!("runtime_client.rs").replace("\r\n", "\n");
        let rename = source
            .split_once("pub(crate) async fn rename_session(")
            .expect("rename method")
            .1
            .split_once("pub(crate) async fn update_session_mode(")
            .expect("rename method boundary")
            .0;

        assert!(source.contains("pub(crate) async fn rename_session("));
        assert!(rename.contains("CliAgentRuntimeBackend::Embedded(runtime)"));
        assert!(rename.contains(".rename_session(request)"));
        assert!(rename.contains("RuntimeIpcOperation::RenameSession"));
        assert!(!rename.contains("serde_json::to_value"));
        assert!(!rename.contains("serde_json::from_value"));
    }

    #[test]
    fn session_compaction_uses_direct_runtime_or_private_shared_ipc() {
        let source = include_str!("runtime_client.rs").replace("\r\n", "\n");
        let compact = source
            .split_once("pub(crate) async fn start_session_compaction(")
            .expect("compaction method")
            .1
            .split_once("pub(crate) async fn send_message(")
            .expect("compaction method boundary")
            .0;

        assert!(compact.contains("CliAgentRuntimeBackend::Embedded(runtime)"));
        assert!(compact.contains(".start_session_compaction(request)"));
        assert!(compact.contains("RuntimeIpcOperation::CompactSession { request }"));
        assert!(compact.contains("RuntimeIpcOperationResult::TurnAccepted"));
        assert!(!compact.contains("serde_json::to_value"));
        assert!(!compact.contains("serde_json::from_value"));
    }

    #[test]
    fn image_attachments_use_the_runtime_contract_and_fail_before_shared_ipc() {
        let source = include_str!("runtime_client.rs").replace("\r\n", "\n");
        let submission = source
            .split_once("pub(crate) async fn send_message_with_context(")
            .expect("context submission method")
            .1
            .split_once("pub(crate) async fn search_workspace_references(")
            .expect("context submission method boundary")
            .0;

        let shared_rejection = submission
            .find("if !attachments.is_empty() && self.is_shared()")
            .expect("shared attachment rejection");
        let session_creation = submission
            .find("let session_id = self.ensure_session")
            .expect("session creation");
        assert!(shared_rejection < session_creation);
        assert!(submission.contains("attachments,"));
        assert!(!submission.contains("imagePath"));
    }

    #[test]
    fn delegated_external_commands_fail_before_shared_ipc() {
        let source = include_str!("runtime_client.rs").replace("\r\n", "\n");
        let submission = source
            .split_once("pub(crate) async fn send_external_subagent_command(")
            .expect("delegated command submission method")
            .1
            .split_once("async fn submit_dialog_turn_request(")
            .expect("delegated command submission boundary")
            .0;

        let shared_rejection = submission
            .find("if self.is_shared()")
            .expect("shared runtime rejection");
        let session_creation = submission
            .find("let session_id = self.ensure_session")
            .expect("session creation");
        assert!(shared_rejection < session_creation);
        assert!(submission.contains("AgentDialogTurnExecution::FreshExternalSubagent"));
        assert!(!submission.contains("RuntimeIpcOperation::SubmitTurn"));
    }

    #[test]
    fn interactive_session_fork_uses_the_same_runtime_boundary_in_both_deployments() {
        let source = include_str!("runtime_client.rs").replace("\r\n", "\n");
        let fork = source
            .split_once("pub(crate) async fn fork_current_session(")
            .expect("interactive fork method")
            .1
            .split_once("pub(crate) async fn generate_session_usage_report(")
            .expect("interactive fork method boundary")
            .0;

        assert!(fork.contains("CliAgentRuntimeBackend::Embedded(runtime)"));
        assert!(fork.contains(".fork_session_before_turn("));
        assert!(fork.contains(".fork_session(AgentSessionForkRequest"));
        assert!(fork.contains("CliAgentRuntimeBackend::Shared(client)"));
        assert!(fork.contains("RuntimeIpcOperation::ForkSession"));
        assert!(fork.contains("RuntimeIpcOperationResult::SessionForked"));
    }

    #[test]
    fn workspace_diff_uses_the_same_runtime_boundary_in_both_deployments() {
        let source = include_str!("runtime_client.rs").replace("\r\n", "\n");
        let workspace_diff = source
            .split_once("pub(crate) async fn workspace_diff(")
            .expect("workspace diff method")
            .1
            .split_once("pub(crate) async fn generate_session_usage_report(")
            .expect("workspace diff method boundary")
            .0;

        assert!(workspace_diff.contains("workspace_diff_unavailable_reason"));
        assert!(workspace_diff.contains("CliAgentRuntimeBackend::Embedded(runtime)"));
        assert!(workspace_diff.contains(".workspace_diff()"));
        assert!(workspace_diff.contains("CliAgentRuntimeBackend::Shared(client)"));
        assert!(workspace_diff.contains("RuntimeIpcOperation::WorkspaceDiff"));
        assert!(workspace_diff.contains("RuntimeIpcOperationResult::WorkspaceDiff"));
    }

    #[test]
    fn session_revert_uses_the_same_authoritative_result_in_both_deployments() {
        let source = include_str!("runtime_client.rs").replace("\r\n", "\n");
        let revert = source
            .split_once("pub(crate) async fn revert_current_session(")
            .expect("session revert method")
            .1
            .split_once("pub(crate) async fn generate_session_usage_report(")
            .expect("session revert method boundary")
            .0;

        assert!(revert.contains("CliAgentRuntimeBackend::Embedded(runtime)"));
        assert!(revert.contains("runtime.undo_session(request)"));
        assert!(revert.contains("runtime.redo_session(request)"));
        assert!(revert.contains("RuntimeIpcOperation::UndoSession"));
        assert!(revert.contains("RuntimeIpcOperation::RedoSession"));
        assert!(revert.contains("RuntimeIpcOperationResult::SessionReverted"));
    }

    #[test]
    fn session_delete_uses_direct_runtime_or_private_shared_ipc() {
        let source = include_str!("runtime_client.rs").replace("\r\n", "\n");
        let delete = source
            .split_once("pub(crate) async fn delete_session(")
            .expect("delete method")
            .1
            .split_once("pub(crate) async fn update_session_model(")
            .expect("delete method boundary")
            .0;

        assert!(delete.contains("CliAgentRuntimeBackend::Embedded(runtime)"));
        assert!(delete.contains(".delete_session(AgentSessionDeleteRequest {"));
        assert!(delete.contains("CliAgentRuntimeBackend::Shared(client)"));
        assert!(delete.contains("RuntimeIpcOperation::DeleteSession"));
        assert!(!delete.contains("embedded_runtime"));
        assert!(!delete.contains("serde_json::to_value"));
        assert!(!delete.contains("serde_json::from_value"));
    }

    #[test]
    fn mode_updates_use_the_runtime_sdk_without_the_core_compatibility_facade() {
        let source = include_str!("runtime_client.rs").replace("\r\n", "\n");
        let compatibility_update = [
            "self.compatibility",
            "\n            .update_session_agent_type",
        ]
        .concat();

        assert!(source.contains("CliAgentRuntimeBackend::Embedded(runtime)"));
        assert!(source.contains("runtime.update_session_mode(request)"));
        assert!(source.contains("CliAgentRuntimeBackend::Shared(client)"));
        assert!(source.contains("RuntimeIpcOperation::UpdateSessionMode { request }"));
        assert!(!source.contains(&compatibility_update));
    }

    #[test]
    fn agent_events_use_the_runtime_sdk_without_a_core_event_source() {
        let source = include_str!("runtime_client.rs").replace("\r\n", "\n");
        let runtime_subscription = ["runtime", ".subscribe_events()"].concat();
        let core_event_field = ["event_source", ": CliAgent", "EventSource"].concat();
        let core_event_method = ["pub(crate) fn event", "_source("].concat();

        assert!(source.contains(&runtime_subscription));
        assert!(!source.contains(&core_event_field));
        assert!(!source.contains(&core_event_method));
    }

    fn session_summary(session_id: &str) -> AgentSessionSummary {
        AgentSessionSummary {
            session_id: session_id.to_string(),
            session_name: "Workspace session".to_string(),
            agent_type: "agentic".to_string(),
            model_id: None,
            last_user_dialog_agent_type: None,
            last_submitted_agent_type: None,
            turn_count: 1,
            created_at_ms: 1,
            last_active_at_ms: 2,
        }
    }

    #[test]
    fn workspace_restore_validation_accepts_listed_session() {
        let sessions = vec![session_summary("session-in-workspace")];

        let summary = validated_session_summary(
            &sessions,
            "session-in-workspace",
            Path::new("D:/workspace/current"),
        )
        .expect("listed session should be restorable");

        assert_eq!(summary.session_id, "session-in-workspace");
    }

    #[test]
    fn workspace_restore_validation_rejects_session_outside_current_workspace() {
        let sessions = vec![session_summary("different-session")];

        let error = validated_session_summary(
            &sessions,
            "session-from-another-workspace",
            Path::new("D:/workspace/current"),
        )
        .expect_err("a session absent from the workspace-scoped list must be rejected");

        let message = error.to_string();
        assert!(message.contains("session-from-another-workspace"));
        assert!(message.contains("D:/workspace/current"));
    }

    #[test]
    fn restore_reports_a_cli_local_notice_when_core_migrates_the_mode() {
        let previous = AgentSessionSummary {
            agent_type: "removed-mode".to_string(),
            ..session_summary("mode-migration")
        };
        let restored = session_summary("mode-migration");

        let notices = session_migration_notices(&previous, &restored);

        assert_eq!(
            notices,
            vec![SessionMigrationNotice::Mode {
                previous_id: "removed-mode".to_string(),
                restored_id: "agentic".to_string(),
            }]
        );
    }

    #[test]
    fn restore_reports_a_cli_local_notice_when_core_migrates_the_model() {
        let previous = AgentSessionSummary {
            model_id: Some("removed-model".to_string()),
            ..session_summary("model-migration")
        };
        let restored = AgentSessionSummary {
            model_id: Some("auto".to_string()),
            ..session_summary("model-migration")
        };

        let notices = session_migration_notices(&previous, &restored);

        assert_eq!(
            notices,
            vec![SessionMigrationNotice::Model {
                previous_id: "removed-model".to_string(),
                restored_id: "auto".to_string(),
            }]
        );
        assert!(notices[0].user_message().contains("unavailable"));
    }

    #[test]
    fn restore_does_not_report_notices_when_session_settings_are_unchanged() {
        let summary = session_summary("unchanged-mode");

        assert!(session_migration_notices(&summary, &summary).is_empty());
    }

    #[test]
    fn nested_permission_projects_to_the_routed_controller_session() {
        let mut permission = PermissionRequestEvent::Asked {
            request: PermissionRequest {
                request_id: "permission".to_string(),
                round_id: "round".to_string(),
                order: 0,
                tool_call_id: None,
                project_path: None,
                project_id: "project".to_string(),
                session_id: "child".to_string(),
                agent_id: "agentic".to_string(),
                action: "run command".to_string(),
                resources: Vec::new(),
                save_resources: Vec::new(),
                source: PermissionRequestSource {
                    kind: PermissionRequestSourceKind::ToolCall,
                    identity: "shell".to_string(),
                },
                delegation: Some(PermissionDelegationContext {
                    parent_session_id: "child".to_string(),
                    parent_dialog_turn_id: None,
                    parent_tool_call_id: "delegate".to_string(),
                    subagent_type: "general".to_string(),
                }),
                display_metadata: serde_json::Map::new(),
            },
        };
        project_routed_permission_event(&mut permission, "root");
        assert!(
            matches!(permission, PermissionRequestEvent::Asked { request } if request.delegation.as_ref().is_some_and(|delegation| delegation.parent_session_id == "root"))
        );
    }
}
