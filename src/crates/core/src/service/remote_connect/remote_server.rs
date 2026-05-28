//! Session bridge: translates remote commands into local session operations.
//!
//! Mobile clients send encrypted commands via the relay (HTTP → WS bridge).
//! The desktop decrypts, dispatches, and returns encrypted responses.
//!
//! Instead of streaming events to the mobile, the desktop maintains an
//! in-memory `RemoteSessionStateTracker` per session. The mobile polls
//! for state changes using the `PollSession` command, receiving only
//! incremental updates (new messages + current active turn snapshot).

use crate::service_agent_runtime::{CoreRemoteSessionTrackerHost, CoreServiceAgentRuntime};
use anyhow::{anyhow, Result};
use log::{debug, error, info};
use serde_json::Value;
use std::sync::{Arc, OnceLock};

use super::encryption;
use bitfun_services_integrations::remote_connect::{
    build_remote_image_contexts, cancel_remote_task, handle_remote_workspace_file_command,
    remote_answer_question_response, remote_assistant_list_response,
    remote_assistant_updated_response, remote_dialog_submit_response, remote_initial_sync_response,
    remote_interaction_accepted_response, remote_messages_response,
    remote_model_catalog_poll_delta, remote_no_change_poll_response,
    remote_persisted_poll_response, remote_recent_workspaces_response,
    remote_session_created_response, remote_session_deleted_response, remote_session_list_response,
    remote_session_model_updated_response, remote_snapshot_poll_response,
    remote_task_cancel_response, remote_workspace_info_response, remote_workspace_updated_response,
    resolve_remote_execution_image_contexts, submit_remote_dialog, RemoteAssistantWorkspaceFacts,
    RemoteCancelTaskRequest, RemoteConnectSubmissionSource, RemoteDialogSubmissionPolicy,
    RemoteDialogSubmissionRequest, RemoteDialogSubmitOutcome, RemoteImageContext,
    RemoteRecentWorkspaceFacts, RemoteSessionMetadata, RemoteSessionTrackerRegistry,
    RemoteWorkspaceFacts, RemoteWorkspaceKind as RemoteConnectWorkspaceKind, RemoteWorkspaceUpdate,
};
pub use bitfun_services_integrations::remote_connect::{
    ActiveTurnSnapshot, AssistantEntry, ChatImageAttachment, ChatMessage, ChatMessageItem,
    ImageAttachment, RecentWorkspaceEntry, RemoteCommand, RemoteDefaultModelsConfig,
    RemoteModelCatalog, RemoteModelConfig, RemoteResponse, RemoteSessionStateTracker,
    RemoteToolStatus, SessionInfo, TrackerEvent,
};

fn remote_workspace_kind(
    kind: crate::service::workspace::WorkspaceKind,
) -> RemoteConnectWorkspaceKind {
    match kind {
        crate::service::workspace::WorkspaceKind::Normal => RemoteConnectWorkspaceKind::Normal,
        crate::service::workspace::WorkspaceKind::Assistant => {
            RemoteConnectWorkspaceKind::Assistant
        }
        crate::service::workspace::WorkspaceKind::Remote => RemoteConnectWorkspaceKind::Remote,
    }
}

fn git_branch_for_workspace_path(path: &std::path::Path) -> Option<String> {
    git2::Repository::open(path).ok().and_then(|repo| {
        repo.head()
            .ok()
            .and_then(|head| head.shorthand().map(String::from))
    })
}

pub type EncryptedPayload = (String, String);

fn resolve_agent_type(mobile_type: Option<&str>) -> &'static str {
    bitfun_services_integrations::remote_connect::resolve_remote_agent_type(mobile_type)
}

/// Convert legacy `ImageAttachment` to unified `ImageContextData`.
pub fn images_to_contexts(
    images: Option<&Vec<ImageAttachment>>,
) -> Vec<crate::agentic::image_analysis::ImageContextData> {
    build_core_image_contexts(images.map(Vec::as_slice))
}

fn build_core_image_contexts(
    images: Option<&[ImageAttachment]>,
) -> Vec<crate::agentic::image_analysis::ImageContextData> {
    build_remote_image_contexts(images)
        .into_iter()
        .map(remote_image_context_to_core)
        .collect()
}

fn remote_image_context_to_core(
    context: RemoteImageContext,
) -> crate::agentic::image_analysis::ImageContextData {
    CoreServiceAgentRuntime::remote_image_context(context)
}

// ── RemoteExecutionDispatcher (global singleton) ────────────────────

/// Shared dispatch layer that owns the session state trackers.
/// Both `RemoteServer` (mobile relay) and the bot use this to
/// dispatch commands through the same path.
pub struct RemoteExecutionDispatcher {
    tracker_registry: RemoteSessionTrackerRegistry,
}

static GLOBAL_DISPATCHER: OnceLock<Arc<RemoteExecutionDispatcher>> = OnceLock::new();

pub fn get_or_init_global_dispatcher() -> Arc<RemoteExecutionDispatcher> {
    GLOBAL_DISPATCHER
        .get_or_init(|| {
            Arc::new(RemoteExecutionDispatcher {
                tracker_registry: RemoteSessionTrackerRegistry::new(),
            })
        })
        .clone()
}

pub fn get_global_dispatcher() -> Option<Arc<RemoteExecutionDispatcher>> {
    GLOBAL_DISPATCHER.get().cloned()
}

impl RemoteExecutionDispatcher {
    /// Ensure a state tracker exists for the given session and return it.
    ///
    /// When the tracker is freshly created and the session already has an active
    /// turn (e.g. a desktop-triggered dialog), the tracker is seeded with the
    /// turn id so that `snapshot_active_turn()` immediately returns a valid
    /// snapshot.  Without this, a late-created tracker would miss the
    /// `DialogTurnStarted` event and the mobile would see no active-turn
    /// overlay until the turn completes.
    pub fn ensure_tracker(&self, session_id: &str) -> Arc<RemoteSessionStateTracker> {
        self.tracker_registry
            .ensure_tracker_with_host(session_id, &CoreRemoteSessionTrackerHost)
    }

    pub fn get_tracker(&self, session_id: &str) -> Option<Arc<RemoteSessionStateTracker>> {
        self.tracker_registry.get_tracker(session_id)
    }

    pub fn remove_tracker(&self, session_id: &str) {
        self.tracker_registry
            .remove_tracker_with_host(session_id, &CoreRemoteSessionTrackerHost);
    }

    /// Dispatch a SendMessage command through the remote-connect runtime owner.
    ///
    /// `bitfun-services-integrations` owns the orchestration order; core supplies
    /// the concrete tracker, session restore, terminal, and scheduler adapters.
    /// When the session is already processing, the message is queued and the current turn
    /// may yield after the current model round for interactive remote sources.
    /// Returns whether this message started immediately or was only queued, plus ids.
    /// If `turn_id` is `None`, one is auto-generated before queueing.
    ///
    /// All platforms (desktop, mobile, bot) use the same `ImageContextData` format.
    pub async fn send_message(
        &self,
        session_id: &str,
        content: String,
        agent_type: Option<&str>,
        image_contexts: Vec<crate::agentic::image_analysis::ImageContextData>,
        source: RemoteConnectSubmissionSource,
        turn_id: Option<String>,
    ) -> std::result::Result<RemoteDialogSubmitOutcome, String> {
        let host = CoreServiceAgentRuntime::remote_dialog_host(self)?;

        submit_remote_dialog(
            &host,
            RemoteDialogSubmissionRequest {
                session_id: session_id.to_string(),
                content,
                agent_type: agent_type.map(ToOwned::to_owned),
                image_contexts,
                policy: RemoteDialogSubmissionPolicy::for_source(source),
                turn_id,
            },
        )
        .await
    }

    /// Cancel a running dialog turn.
    pub async fn cancel_task(
        &self,
        session_id: &str,
        requested_turn_id: Option<&str>,
    ) -> std::result::Result<(), String> {
        let host = CoreServiceAgentRuntime::remote_cancel_host()?;
        cancel_remote_task(
            &host,
            RemoteCancelTaskRequest {
                session_id: session_id.to_string(),
                requested_turn_id: requested_turn_id.map(ToOwned::to_owned),
            },
        )
        .await
    }
}

// ── RemoteServer ───────────────────────────────────────────────────

/// Bridges remote commands to local session operations.
/// Delegates execution and tracker management to the global `RemoteExecutionDispatcher`.
pub struct RemoteServer {
    shared_secret: [u8; 32],
}

impl RemoteServer {
    pub fn new(shared_secret: [u8; 32]) -> Self {
        get_or_init_global_dispatcher();
        Self { shared_secret }
    }

    pub fn shared_secret(&self) -> &[u8; 32] {
        &self.shared_secret
    }

    pub fn decrypt_command(
        &self,
        encrypted_data: &str,
        nonce: &str,
    ) -> Result<(RemoteCommand, Option<String>)> {
        let json = encryption::decrypt_from_base64(&self.shared_secret, encrypted_data, nonce)?;
        let value: Value = serde_json::from_str(&json).map_err(|e| anyhow!("parse json: {e}"))?;
        let request_id = value
            .get("_request_id")
            .and_then(|v| v.as_str())
            .map(String::from);
        let cmd: RemoteCommand =
            serde_json::from_value(value).map_err(|e| anyhow!("parse command: {e}"))?;
        Ok((cmd, request_id))
    }

    pub fn encrypt_response(
        &self,
        response: &RemoteResponse,
        request_id: Option<&str>,
    ) -> Result<EncryptedPayload> {
        let mut value =
            serde_json::to_value(response).map_err(|e| anyhow!("serialize response: {e}"))?;
        if let (Some(id), Some(obj)) = (request_id, value.as_object_mut()) {
            obj.insert("_request_id".to_string(), Value::String(id.to_string()));
        }
        let json = serde_json::to_string(&value).map_err(|e| anyhow!("to_string: {e}"))?;
        encryption::encrypt_to_base64(&self.shared_secret, &json)
    }

    pub async fn dispatch(&self, cmd: &RemoteCommand) -> RemoteResponse {
        match cmd {
            RemoteCommand::Ping => RemoteResponse::Pong,

            RemoteCommand::GetWorkspaceInfo
            | RemoteCommand::ListRecentWorkspaces
            | RemoteCommand::SetWorkspace { .. }
            | RemoteCommand::ListAssistants
            | RemoteCommand::SetAssistant { .. } => self.handle_workspace_command(cmd).await,

            RemoteCommand::ListSessions { .. }
            | RemoteCommand::CreateSession { .. }
            | RemoteCommand::GetModelCatalog { .. }
            | RemoteCommand::SetSessionModel { .. }
            | RemoteCommand::UpdateSessionTitle { .. }
            | RemoteCommand::GetSessionMessages { .. }
            | RemoteCommand::DeleteSession { .. } => self.handle_session_command(cmd).await,

            RemoteCommand::SendMessage { .. }
            | RemoteCommand::CancelTask { .. }
            | RemoteCommand::ConfirmTool { .. }
            | RemoteCommand::RejectTool { .. }
            | RemoteCommand::CancelTool { .. }
            | RemoteCommand::AnswerQuestion { .. } => self.handle_execution_command(cmd).await,

            RemoteCommand::PollSession { .. } => self.handle_poll_command(cmd).await,

            RemoteCommand::ReadFile { .. }
            | RemoteCommand::ReadFileChunk { .. }
            | RemoteCommand::GetFileInfo { .. } => {
                let host = CoreServiceAgentRuntime::remote_workspace_file_host();
                handle_remote_workspace_file_command(&host, cmd).await
            }
        }
    }

    fn ensure_tracker(&self, session_id: &str) -> Arc<RemoteSessionStateTracker> {
        get_or_init_global_dispatcher().ensure_tracker(session_id)
    }

    pub async fn generate_initial_sync(
        &self,
        authenticated_user_id: Option<String>,
    ) -> RemoteResponse {
        use crate::agentic::persistence::PersistenceManager;
        use crate::infrastructure::PathManager;

        let (ws_path, workspace_facts) =
            if let Some(ws_service) = crate::service::workspace::get_global_workspace_service() {
                if let Some(ws) = ws_service.get_current_workspace().await {
                    let p = ws.root_path.clone();
                    (
                        Some(p.clone()),
                        Some(RemoteWorkspaceFacts {
                            path: p.to_string_lossy().to_string(),
                            name: ws.name.clone(),
                            git_branch: git_branch_for_workspace_path(&p),
                            kind: remote_workspace_kind(ws.workspace_kind),
                            assistant_id: ws.assistant_id.clone(),
                        }),
                    )
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

        let ws_name = ws_path
            .as_ref()
            .and_then(|wp| wp.file_name().map(|n| n.to_string_lossy().to_string()));

        let (sessions, has_more) = if let Some(ref wp) = ws_path {
            if let Ok(pm) = PathManager::new() {
                let pm = std::sync::Arc::new(pm);
                if let Ok(store) = PersistenceManager::new(pm) {
                    if let Ok(all_meta) = store.list_session_metadata(wp).await {
                        let total = all_meta.len();
                        let page_size = 100usize;
                        let has_more = total > page_size;
                        let sessions: Vec<RemoteSessionMetadata> = all_meta
                            .into_iter()
                            .take(page_size)
                            .map(|s| RemoteSessionMetadata {
                                session_id: s.session_id,
                                name: s.session_name,
                                agent_type: s.agent_type,
                                created_at_ms: s.created_at,
                                last_active_at_ms: s.last_active_at,
                                turn_count: s.turn_count,
                            })
                            .collect();
                        (sessions, has_more)
                    } else {
                        (vec![], false)
                    }
                } else {
                    (vec![], false)
                }
            } else {
                (vec![], false)
            }
        } else {
            (vec![], false)
        };

        remote_initial_sync_response(
            workspace_facts,
            sessions,
            ws_name.as_deref(),
            has_more,
            authenticated_user_id,
        )
    }

    // ── Poll command handler ────────────────────────────────────────

    async fn handle_poll_command(&self, cmd: &RemoteCommand) -> RemoteResponse {
        let RemoteCommand::PollSession {
            session_id,
            since_version,
            known_msg_count,
            known_model_catalog_version,
        } = cmd
        else {
            return RemoteResponse::Error {
                message: "expected poll_session".into(),
            };
        };

        let tracker = self.ensure_tracker(session_id);
        let current_version = tracker.version();
        let current_model_catalog =
            CoreServiceAgentRuntime::load_remote_model_catalog(Some(session_id))
                .await
                .ok();
        let model_catalog_delta =
            remote_model_catalog_poll_delta(current_model_catalog, *known_model_catalog_version);

        if *since_version == current_version && *since_version > 0 && !model_catalog_delta.changed {
            return remote_no_change_poll_response(current_version);
        }

        // Fast path: during active streaming, only the real-time snapshot
        // changes — persisted messages stay the same.  Skip the expensive
        // disk read and return just the snapshot.
        let needs_persistence = *since_version == 0 || tracker.is_persistence_dirty();

        if !needs_persistence {
            return remote_snapshot_poll_response(
                &tracker,
                current_version,
                model_catalog_delta.catalog,
            );
        }

        let Some(workspace_path) =
            CoreServiceAgentRuntime::resolve_session_workspace_path(session_id).await
        else {
            return RemoteResponse::Error {
                message: format!("Workspace path not available for session: {}", session_id),
            };
        };
        let (all_chat_msgs, _) =
            CoreServiceAgentRuntime::load_remote_chat_messages(&workspace_path, session_id).await;
        let total_msg_count = all_chat_msgs.len();
        let skip = *known_msg_count;
        let new_messages: Vec<ChatMessage> = all_chat_msgs.into_iter().skip(skip).collect();

        remote_persisted_poll_response(
            &tracker,
            current_version,
            new_messages,
            total_msg_count,
            model_catalog_delta.catalog,
        )
    }

    // ── Workspace commands ──────────────────────────────────────────

    async fn handle_workspace_command(&self, cmd: &RemoteCommand) -> RemoteResponse {
        use crate::service::workspace::get_global_workspace_service;

        match cmd {
            RemoteCommand::GetWorkspaceInfo => {
                if let Some(ws_service) = get_global_workspace_service() {
                    if let Some(ws) = ws_service.get_current_workspace().await {
                        let p = ws.root_path.clone();
                        return remote_workspace_info_response(Some(RemoteWorkspaceFacts {
                            path: p.to_string_lossy().to_string(),
                            name: ws.name.clone(),
                            git_branch: git_branch_for_workspace_path(&p),
                            kind: remote_workspace_kind(ws.workspace_kind),
                            assistant_id: ws.assistant_id.clone(),
                        }));
                    }
                }
                remote_workspace_info_response(None)
            }
            RemoteCommand::ListRecentWorkspaces => {
                let ws_service = match get_global_workspace_service() {
                    Some(s) => s,
                    None => {
                        return remote_recent_workspaces_response(vec![]);
                    }
                };
                let recent = ws_service.get_recent_workspaces().await;
                let entries = recent
                    .into_iter()
                    .map(|w| RemoteRecentWorkspaceFacts {
                        path: w.root_path.to_string_lossy().to_string(),
                        name: w.name.clone(),
                        last_opened: w.last_accessed.to_rfc3339(),
                        kind: remote_workspace_kind(w.workspace_kind),
                    })
                    .collect();
                remote_recent_workspaces_response(entries)
            }
            RemoteCommand::SetWorkspace { path } => {
                let ws_service = match get_global_workspace_service() {
                    Some(s) => s,
                    None => {
                        return remote_workspace_updated_response(Err(
                            "Workspace service not available".to_string(),
                        ));
                    }
                };
                let path_buf = std::path::PathBuf::from(path);
                match ws_service.open_workspace(path_buf).await {
                    Ok(info) => {
                        if let Err(e) =
                            crate::service::snapshot::initialize_snapshot_manager_for_workspace(
                                info.root_path.clone(),
                                None,
                            )
                            .await
                        {
                            error!("Failed to initialize snapshot after remote workspace set: {e}");
                        }
                        remote_workspace_updated_response(Ok(RemoteWorkspaceUpdate {
                            path: info.root_path.to_string_lossy().to_string(),
                            name: info.name.clone(),
                        }))
                    }
                    Err(e) => remote_workspace_updated_response(Err(e.to_string())),
                }
            }
            RemoteCommand::ListAssistants => {
                let ws_service = match get_global_workspace_service() {
                    Some(s) => s,
                    None => {
                        return remote_assistant_list_response(vec![]);
                    }
                };
                let assistants = ws_service.get_assistant_workspaces().await;
                let entries = assistants
                    .into_iter()
                    .map(|w| RemoteAssistantWorkspaceFacts {
                        path: w.root_path.to_string_lossy().to_string(),
                        name: w.name.clone(),
                        assistant_id: w.assistant_id.clone(),
                    })
                    .collect();
                remote_assistant_list_response(entries)
            }
            RemoteCommand::SetAssistant { path } => {
                let ws_service = match get_global_workspace_service() {
                    Some(s) => s,
                    None => {
                        return remote_assistant_updated_response(Err(
                            "Workspace service not available".to_string(),
                        ));
                    }
                };
                let path_buf = std::path::PathBuf::from(path);
                match ws_service.open_workspace(path_buf).await {
                    Ok(info) => {
                        if let Err(e) =
                            crate::service::snapshot::initialize_snapshot_manager_for_workspace(
                                info.root_path.clone(),
                                None,
                            )
                            .await
                        {
                            error!("Failed to initialize snapshot after remote assistant set: {e}");
                        }
                        remote_assistant_updated_response(Ok(RemoteWorkspaceUpdate {
                            path: info.root_path.to_string_lossy().to_string(),
                            name: info.name.clone(),
                        }))
                    }
                    Err(e) => remote_assistant_updated_response(Err(e.to_string())),
                }
            }
            _ => RemoteResponse::Error {
                message: "Unknown workspace command".into(),
            },
        }
    }

    // ── Session commands ────────────────────────────────────────────

    async fn handle_session_command(&self, cmd: &RemoteCommand) -> RemoteResponse {
        use crate::agentic::coordination::get_global_coordinator;
        use bitfun_services_integrations::remote_connect::{
            build_remote_session_create_request, RemoteConnectSubmissionSource,
        };

        let coordinator = match get_global_coordinator() {
            Some(c) => c,
            None => {
                return RemoteResponse::Error {
                    message: "Desktop session system not ready".into(),
                };
            }
        };

        match cmd {
            RemoteCommand::ListSessions {
                workspace_path,
                limit,
                offset,
                query,
            } => {
                use crate::agentic::persistence::PersistenceManager;
                use crate::infrastructure::PathManager;

                let page_size = limit.unwrap_or(30).min(100);
                let page_offset = offset.unwrap_or(0);

                let Some(workspace_path) = workspace_path
                    .as_deref()
                    .filter(|path| !path.is_empty())
                    .map(std::path::PathBuf::from)
                else {
                    return RemoteResponse::Error {
                        message: "workspace_path is required for ListSessions".to_string(),
                    };
                };

                let ws_str = workspace_path.to_string_lossy().to_string();
                let workspace_name = workspace_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string());

                if let Ok(pm) = PathManager::new() {
                    let pm = std::sync::Arc::new(pm);
                    match PersistenceManager::new(pm) {
                        Ok(store) => match store.list_session_metadata(&workspace_path).await {
                            Ok(all_meta) => {
                                let query = query
                                    .as_deref()
                                    .map(str::trim)
                                    .filter(|value| !value.is_empty())
                                    .map(str::to_lowercase);
                                let sessions: Vec<RemoteSessionMetadata> = all_meta
                                    .into_iter()
                                    .filter(|session| {
                                        query.as_ref().map_or(true, |query| {
                                            session.session_name.to_lowercase().contains(query)
                                        })
                                    })
                                    .map(|s| RemoteSessionMetadata {
                                        session_id: s.session_id,
                                        name: s.session_name,
                                        agent_type: s.agent_type,
                                        created_at_ms: s.created_at,
                                        last_active_at_ms: s.last_active_at,
                                        turn_count: s.turn_count,
                                    })
                                    .collect();
                                remote_session_list_response(
                                    sessions,
                                    Some(ws_str.as_str()),
                                    workspace_name.as_deref(),
                                    page_size,
                                    page_offset,
                                )
                            }
                            Err(e) => {
                                debug!("Session list read failed for {ws_str}: {e}");
                                RemoteResponse::Error {
                                    message: format!("Failed to list sessions for workspace: {e}"),
                                }
                            }
                        },
                        Err(e) => {
                            debug!("PersistenceManager init failed for {ws_str}: {e}");
                            RemoteResponse::Error {
                                message: format!("Failed to initialize session storage: {e}"),
                            }
                        }
                    }
                } else {
                    RemoteResponse::Error {
                        message: "Failed to initialize path manager".to_string(),
                    }
                }
            }
            RemoteCommand::CreateSession {
                agent_type,
                session_name: custom_name,
                workspace_path: requested_ws_path,
            } => {
                let agent = resolve_agent_type(agent_type.as_deref());
                let is_claw = agent == "Claw";

                let session_name =
                    custom_name
                        .as_deref()
                        .filter(|n| !n.is_empty())
                        .unwrap_or(match agent {
                            "Cowork" => "Remote Cowork Session",
                            "Claw" => "Remote Claw Session",
                            _ => "Remote Code Session",
                        });

                let binding_ws_str = if is_claw {
                    // For Claw sessions, get or create default assistant workspace
                    use crate::service::workspace::get_global_workspace_service;

                    let ws_service = match get_global_workspace_service() {
                        Some(s) => s,
                        None => {
                            return RemoteResponse::Error {
                                message: "Workspace service not available".to_string(),
                            };
                        }
                    };

                    let workspaces = ws_service.get_assistant_workspaces().await;
                    if let Some(default_ws) =
                        workspaces.into_iter().find(|w| w.assistant_id.is_none())
                    {
                        Some(default_ws.root_path.to_string_lossy().to_string())
                    } else {
                        match ws_service.create_assistant_workspace(None).await {
                            Ok(ws_info) => Some(ws_info.root_path.to_string_lossy().to_string()),
                            Err(e) => {
                                return RemoteResponse::Error {
                                    message: format!("Failed to create assistant workspace: {}", e),
                                };
                            }
                        }
                    }
                } else {
                    // For Code/Cowork sessions, use provided workspace
                    requested_ws_path
                        .as_deref()
                        .filter(|path| !path.is_empty())
                        .map(ToOwned::to_owned)
                };

                debug!(
                    "Remote CreateSession: agent={}, requested_ws={:?}, binding_ws={:?}",
                    agent, requested_ws_path, binding_ws_str
                );

                let Some(binding_ws_str) = binding_ws_str else {
                    return RemoteResponse::Error {
                        message: if is_claw {
                            "Failed to get or create assistant workspace".to_string()
                        } else {
                            "workspace_path is required for CreateSession".to_string()
                        },
                    };
                };

                let request = build_remote_session_create_request(
                    session_name,
                    agent,
                    Some(binding_ws_str),
                    RemoteConnectSubmissionSource::Relay,
                );
                let submission_port =
                    CoreServiceAgentRuntime::agent_submission_port(coordinator.as_ref());
                match submission_port.create_session(request).await {
                    Ok(session) => remote_session_created_response(session.session_id),
                    Err(e) => RemoteResponse::Error { message: e.message },
                }
            }
            RemoteCommand::GetModelCatalog { session_id } => {
                match CoreServiceAgentRuntime::load_remote_model_catalog(session_id.as_deref())
                    .await
                {
                    Ok(catalog) => RemoteResponse::ModelCatalog { catalog },
                    Err(message) => RemoteResponse::Error { message },
                }
            }
            RemoteCommand::SetSessionModel {
                session_id,
                model_id,
            } => {
                match CoreServiceAgentRuntime::update_remote_session_model(
                    coordinator.as_ref(),
                    session_id,
                    model_id,
                )
                .await
                {
                    Ok(normalized_model_id) => remote_session_model_updated_response(
                        session_id.clone(),
                        normalized_model_id,
                    ),
                    Err(message) => RemoteResponse::Error { message },
                }
            }
            RemoteCommand::UpdateSessionTitle { session_id, title } => {
                if coordinator
                    .get_session_manager()
                    .get_session(session_id)
                    .is_none()
                {
                    let Some(workspace_path) =
                        CoreServiceAgentRuntime::resolve_session_workspace_path(session_id).await
                    else {
                        return RemoteResponse::Error {
                            message: format!(
                                "Workspace path not available for session: {}",
                                session_id
                            ),
                        };
                    };
                    if let Err(e) = coordinator
                        .restore_session(&workspace_path, session_id)
                        .await
                    {
                        return RemoteResponse::Error {
                            message: format!("Failed to restore session: {e}"),
                        };
                    }
                }

                match coordinator.update_session_title(session_id, title).await {
                    Ok(normalized_title) => RemoteResponse::SessionTitleUpdated {
                        session_id: session_id.clone(),
                        title: normalized_title,
                    },
                    Err(e) => RemoteResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            RemoteCommand::GetSessionMessages {
                session_id,
                limit: _,
                before_message_id: _,
            } => {
                let Some(workspace_path) =
                    CoreServiceAgentRuntime::resolve_session_workspace_path(session_id).await
                else {
                    return RemoteResponse::Error {
                        message: format!(
                            "Workspace path not available for session: {}",
                            session_id
                        ),
                    };
                };
                let (chat_msgs, has_more) =
                    CoreServiceAgentRuntime::load_remote_chat_messages(&workspace_path, session_id)
                        .await;
                remote_messages_response(session_id.clone(), chat_msgs, has_more)
            }
            RemoteCommand::DeleteSession { session_id } => {
                let Some(workspace_path) =
                    CoreServiceAgentRuntime::resolve_session_workspace_path(session_id).await
                else {
                    return RemoteResponse::Error {
                        message: format!(
                            "Workspace path not available for session: {}",
                            session_id
                        ),
                    };
                };

                match coordinator
                    .delete_session(&workspace_path, session_id)
                    .await
                {
                    Ok(_) => {
                        get_or_init_global_dispatcher().remove_tracker(session_id);
                        remote_session_deleted_response(session_id.clone())
                    }
                    Err(e) => RemoteResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            _ => RemoteResponse::Error {
                message: "Unknown session command".into(),
            },
        }
    }

    // ── Execution commands ──────────────────────────────────────────

    async fn handle_execution_command(&self, cmd: &RemoteCommand) -> RemoteResponse {
        use crate::agentic::coordination::get_global_coordinator;

        let dispatcher = get_or_init_global_dispatcher();

        match cmd {
            RemoteCommand::SendMessage {
                session_id,
                content,
                agent_type: requested_agent_type,
                images,
                image_contexts,
            } => {
                // Unified: prefer image_contexts (new format), fall back to legacy images
                let explicit_contexts = image_contexts.clone().map(|contexts| {
                    contexts
                        .into_iter()
                        .map(remote_image_context_to_core)
                        .collect()
                });
                let resolved_contexts = resolve_remote_execution_image_contexts(
                    images.as_ref().map(Vec::as_slice),
                    explicit_contexts,
                    build_core_image_contexts,
                );
                info!(
                    "Remote send_message: session={session_id}, agent_type={}, image_contexts={}",
                    requested_agent_type.as_deref().unwrap_or("agentic"),
                    resolved_contexts.len()
                );
                remote_dialog_submit_response(
                    dispatcher
                        .send_message(
                            session_id,
                            content.clone(),
                            requested_agent_type.as_deref(),
                            resolved_contexts,
                            RemoteConnectSubmissionSource::Relay,
                            None,
                        )
                        .await,
                )
            }
            RemoteCommand::CancelTask {
                session_id,
                turn_id,
            } => remote_task_cancel_response(
                session_id.clone(),
                dispatcher.cancel_task(session_id, turn_id.as_deref()).await,
            ),
            RemoteCommand::ConfirmTool {
                tool_id,
                updated_input,
            } => {
                let coordinator = match get_global_coordinator() {
                    Some(c) => c,
                    None => {
                        return RemoteResponse::Error {
                            message: "Desktop session system not ready".into(),
                        };
                    }
                };
                remote_interaction_accepted_response(
                    "confirm_tool",
                    tool_id.clone(),
                    coordinator
                        .confirm_tool(tool_id, updated_input.clone())
                        .await
                        .map(|_| ())
                        .map_err(|e| e.to_string()),
                )
            }
            RemoteCommand::RejectTool { tool_id, reason } => {
                let coordinator = match get_global_coordinator() {
                    Some(c) => c,
                    None => {
                        return RemoteResponse::Error {
                            message: "Desktop session system not ready".into(),
                        };
                    }
                };
                let reject_reason = reason
                    .clone()
                    .unwrap_or_else(|| "User rejected".to_string());
                remote_interaction_accepted_response(
                    "reject_tool",
                    tool_id.clone(),
                    coordinator
                        .reject_tool(tool_id, reject_reason)
                        .await
                        .map(|_| ())
                        .map_err(|e| e.to_string()),
                )
            }
            RemoteCommand::CancelTool { tool_id, reason } => {
                let coordinator = match get_global_coordinator() {
                    Some(c) => c,
                    None => {
                        return RemoteResponse::Error {
                            message: "Desktop session system not ready".into(),
                        };
                    }
                };
                let cancel_reason = reason
                    .clone()
                    .unwrap_or_else(|| "User cancelled".to_string());
                remote_interaction_accepted_response(
                    "cancel_tool",
                    tool_id.clone(),
                    coordinator
                        .cancel_tool(tool_id, cancel_reason)
                        .await
                        .map(|_| ())
                        .map_err(|e| e.to_string()),
                )
            }
            RemoteCommand::AnswerQuestion { tool_id, answers } => {
                use crate::agentic::tools::user_input_manager::get_user_input_manager;
                let mgr = get_user_input_manager();
                remote_answer_question_response(mgr.send_answer(tool_id, answers.clone()))
            }
            _ => RemoteResponse::Error {
                message: "Unknown execution command".into(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::remote_connect::encryption::KeyPair;
    use bitfun_services_integrations::remote_connect::{
        remote_session_restore_target, resolve_remote_cancel_decision, RemoteCancelDecision,
    };

    #[test]
    fn test_command_round_trip() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let shared = alice.derive_shared_secret(&bob.public_key_bytes());

        let bridge = RemoteServer::new(shared);

        let cmd_json = serde_json::json!({
            "cmd": "send_message",
            "session_id": "sess-123",
            "content": "Hello from mobile!",
            "_request_id": "req_abc"
        });
        let json = cmd_json.to_string();
        let (enc, nonce) = encryption::encrypt_to_base64(&shared, &json).unwrap();
        let (decoded, req_id) = bridge.decrypt_command(&enc, &nonce).unwrap();

        assert_eq!(req_id.as_deref(), Some("req_abc"));
        if let RemoteCommand::SendMessage {
            session_id,
            content,
            ..
        } = decoded
        {
            assert_eq!(session_id, "sess-123");
            assert_eq!(content, "Hello from mobile!");
        } else {
            panic!("unexpected command variant");
        }
    }

    #[test]
    fn test_response_with_request_id() {
        let alice = KeyPair::generate();
        let shared = alice.derive_shared_secret(&alice.public_key_bytes());
        let bridge = RemoteServer::new(shared);

        let resp = RemoteResponse::Pong;
        let (enc, nonce) = bridge.encrypt_response(&resp, Some("req_xyz")).unwrap();

        let json = encryption::decrypt_from_base64(&shared, &enc, &nonce).unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["resp"], "pong");
        assert_eq!(value["_request_id"], "req_xyz");
    }

    #[test]
    fn core_service_agent_runtime_owner_maps_remote_image_context() {
        let metadata = serde_json::json!({ "source": "relay" });
        let context = RemoteImageContext {
            id: "image-1".to_string(),
            image_path: Some("/workspace/screenshot.png".to_string()),
            data_url: None,
            mime_type: "image/png".to_string(),
            metadata: Some(metadata.clone()),
        };

        let mapped =
            crate::service_agent_runtime::CoreServiceAgentRuntime::remote_image_context(context);

        assert_eq!(mapped.id, "image-1");
        assert_eq!(
            mapped.image_path.as_deref(),
            Some("/workspace/screenshot.png")
        );
        assert_eq!(mapped.mime_type, "image/png");
        assert_eq!(mapped.metadata, Some(metadata));
    }

    #[test]
    fn remote_execution_prefers_unified_image_contexts_over_legacy_images() {
        let explicit_context = crate::agentic::image_analysis::ImageContextData {
            id: "ctx-1".to_string(),
            image_path: Some("D:/workspace/project/screenshot.png".to_string()),
            data_url: None,
            mime_type: "image/png".to_string(),
            metadata: Some(serde_json::json!({ "source": "desktop" })),
        };
        let legacy_images = vec![ImageAttachment {
            name: "legacy.png".to_string(),
            data_url: "data:image/png;base64,legacy".to_string(),
        }];

        let resolved = resolve_remote_execution_image_contexts(
            Some(legacy_images.as_slice()),
            Some(vec![explicit_context.clone()]),
            build_core_image_contexts,
        );

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].id, explicit_context.id);
        assert_eq!(resolved[0].image_path, explicit_context.image_path);
        assert!(resolved[0].data_url.is_none());
    }

    #[test]
    fn remote_execution_falls_back_to_legacy_images_as_image_contexts() {
        let legacy_images = vec![ImageAttachment {
            name: "clip.png".to_string(),
            data_url: "data:image/png;base64,abc".to_string(),
        }];

        let resolved = resolve_remote_execution_image_contexts(
            Some(legacy_images.as_slice()),
            None,
            build_core_image_contexts,
        );

        assert_eq!(resolved.len(), 1);
        assert!(resolved[0].id.starts_with("remote_img_"));
        assert_eq!(
            resolved[0].data_url.as_deref(),
            Some("data:image/png;base64,abc")
        );
        assert_eq!(resolved[0].mime_type, "image/png");
        assert_eq!(resolved[0].metadata.as_ref().unwrap()["name"], "clip.png");
    }

    #[test]
    fn remote_cancel_decision_preserves_current_turn_boundaries() {
        assert_eq!(
            resolve_remote_cancel_decision(Some("turn-current"), Some("turn-current")),
            RemoteCancelDecision::CancelCurrent("turn-current".to_string())
        );
        assert_eq!(
            resolve_remote_cancel_decision(Some("turn-current"), None),
            RemoteCancelDecision::CancelCurrent("turn-current".to_string())
        );
        assert_eq!(
            resolve_remote_cancel_decision(Some("turn-current"), Some("turn-stale")),
            RemoteCancelDecision::StaleRequestedTurn
        );
        assert_eq!(
            resolve_remote_cancel_decision(None, Some("turn-finished")),
            RemoteCancelDecision::AlreadyFinished
        );
        assert_eq!(
            resolve_remote_cancel_decision(None, None),
            RemoteCancelDecision::NoRunningTask
        );
    }

    #[test]
    fn remote_restore_target_only_restores_cold_sessions_with_workspace_binding() {
        assert_eq!(
            remote_session_restore_target(false, Some("D:/workspace/project")),
            Some("D:/workspace/project")
        );
        assert_eq!(
            remote_session_restore_target(true, Some("D:/workspace/project")),
            None
        );
        assert_eq!(remote_session_restore_target(false, None), None);
    }

    #[test]
    fn remote_command_snapshot_covers_execution_poll_and_cancel_surfaces() {
        let command = RemoteCommand::SendMessage {
            session_id: "session-1".to_string(),
            content: "hello".to_string(),
            agent_type: Some("code".to_string()),
            images: Some(vec![ImageAttachment {
                name: "clip.png".to_string(),
                data_url: "data:image/png;base64,abc".to_string(),
            }]),
            image_contexts: None,
        };
        let json = serde_json::to_value(command).expect("serialize send command");
        assert_eq!(json["cmd"], "send_message");
        assert_eq!(json["session_id"], "session-1");
        assert_eq!(json["agent_type"], "code");
        assert_eq!(json["images"][0]["name"], "clip.png");
        assert!(json["image_contexts"].is_null());
        assert!(json.get("imageContexts").is_none());

        let cancel = serde_json::to_value(RemoteCommand::CancelTask {
            session_id: "session-1".to_string(),
            turn_id: Some("turn-1".to_string()),
        })
        .expect("serialize cancel command");
        assert_eq!(cancel["cmd"], "cancel_task");
        assert_eq!(cancel["turn_id"], "turn-1");

        let list = serde_json::to_value(RemoteCommand::ListSessions {
            workspace_path: Some("/workspace/project".to_string()),
            limit: Some(30),
            offset: Some(0),
            query: Some("alpha".to_string()),
        })
        .expect("serialize list command");
        assert_eq!(list["cmd"], "list_sessions");
        assert_eq!(list["query"], "alpha");

        let rename = serde_json::to_value(RemoteCommand::UpdateSessionTitle {
            session_id: "session-1".to_string(),
            title: "Renamed session".to_string(),
        })
        .expect("serialize rename command");
        assert_eq!(rename["cmd"], "update_session_title");
        assert_eq!(rename["title"], "Renamed session");

        let poll = serde_json::to_value(RemoteCommand::PollSession {
            session_id: "session-1".to_string(),
            since_version: 7,
            known_msg_count: 3,
            known_model_catalog_version: Some(11),
        })
        .expect("serialize poll command");
        assert_eq!(poll["cmd"], "poll_session");
        assert_eq!(poll["since_version"], 7);
        assert_eq!(poll["known_msg_count"], 3);
        assert_eq!(poll["known_model_catalog_version"], 11);
    }

    #[test]
    fn remote_response_snapshot_preserves_active_turn_and_result_shapes() {
        let active_turn = ActiveTurnSnapshot {
            turn_id: "turn-1".to_string(),
            status: "active".to_string(),
            text: String::new(),
            thinking: String::new(),
            tools: vec![RemoteToolStatus {
                id: "tool-1".to_string(),
                name: "Read".to_string(),
                status: "running".to_string(),
                duration_ms: None,
                start_ms: Some(42),
                input_preview: Some("{\"path\":\"README.md\"}".to_string()),
                tool_input: None,
            }],
            round_index: 2,
            items: Some(vec![ChatMessageItem {
                item_type: "tool".to_string(),
                content: None,
                tool: None,
                is_subagent: None,
            }]),
        };

        let poll = serde_json::to_value(RemoteResponse::SessionPoll {
            version: 8,
            changed: true,
            session_state: Some("running".to_string()),
            title: Some("session title".to_string()),
            new_messages: None,
            total_msg_count: None,
            active_turn: Some(active_turn),
            model_catalog: Box::new(None),
        })
        .expect("serialize poll response");

        assert_eq!(poll["resp"], "session_poll");
        assert_eq!(poll["version"], 8);
        assert_eq!(poll["active_turn"]["turn_id"], "turn-1");
        assert_eq!(
            poll["active_turn"]["tools"][0]["input_preview"],
            "{\"path\":\"README.md\"}"
        );
        assert!(poll.get("new_messages").is_none());

        let sent = serde_json::to_value(RemoteResponse::MessageSent {
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
        })
        .expect("serialize sent response");
        assert_eq!(sent["resp"], "message_sent");
        assert_eq!(sent["turn_id"], "turn-1");

        let cancelled = serde_json::to_value(RemoteResponse::TaskCancelled {
            session_id: "session-1".to_string(),
        })
        .expect("serialize cancelled response");
        assert_eq!(cancelled["resp"], "task_cancelled");
        assert_eq!(cancelled["session_id"], "session-1");

        let title_updated = serde_json::to_value(RemoteResponse::SessionTitleUpdated {
            session_id: "session-1".to_string(),
            title: "Renamed session".to_string(),
        })
        .expect("serialize title response");
        assert_eq!(title_updated["resp"], "session_title_updated");
        assert_eq!(title_updated["title"], "Renamed session");
    }
}
