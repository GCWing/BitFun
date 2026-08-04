//! Generic app-server client: drives an [`AppClient`] connection over a
//! host-supplied transport and exposes the agent kernel RPC surface.
//!
//! This module is the client counterpart of [`crate::server::BitfunAppServer`].
//! It is **transport-agnostic**: the host picks the transport (in-memory pair,
//! stdio, websocket, ...) and passes the client half to [`connect`]. The
//! server half is driven by the host through [`BitfunAppServer::serve`], so
//! this client never decides how the server is started -- it only owns the
//! client end of one connection.
//!
//! [`connect`] builds an [`AppClient`] on the given transport, parks the
//! [`ConnectionTo<AppServer>`] for the lifetime of the returned handle, and
//! registers `on_receive_notification` for `agent/event`. Each forwarded
//! [`AgenticEventEnvelope`] is projected to the frontend shape (via
//! [`bitfun_events::project_agentic_frontend_event`]) and fanned out through a
//! [`tokio::sync::broadcast`] channel; consumers receive them via
//! [`AppServerClient::subscribe_events`]. The client never subscribes to the
//! runtime event queue directly, so all agent interfaces uniformly go through
//! the app-server protocol surface.
//!
//! The connection-driving pattern mirrors the ACP client manager
//! (`acp/src/client/manager.rs`): the `connect_with` main function parks the
//! [`ConnectionTo<AppServer>`] through a oneshot and then awaits a shutdown
//! signal so the connection task stays alive for the lifetime of the host.
//!
//! # In-process pairing
//!
//! A host that runs the server and client in the same process pairs them with
//! [`crate::transport::in_memory_channel_pair`]:
//!
//! ```no_run
//! # use bitfun_app_server::{AppClient, AppServer, transport};
//! # use bitfun_app_server::client::connect;
//! # async fn example() -> anyhow::Result<()> {
//! // let (server_transport, client_transport) = transport::in_memory_channel_pair();
//! // let _server = tokio::spawn(async { /* serve BitfunAppServer on server_transport */ });
//! // let client = connect(client_transport).await?;
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;
use std::time::Duration;

use agent_client_protocol::{ConnectionTo, JsonRpcResponse, Result, SentRequest};
use agent_client_protocol::ConnectTo;
use bitfun_events::project_agentic_frontend_event;
use tokio::sync::{broadcast, oneshot};

use crate::schema::{
    CancelTurnMessage, CancelTurnResponse, ClearProjectPermissionGrantsMessage,
    ClearProjectPermissionGrantsResponse, CreateSessionMessage, CreateSessionResponse,
    DeleteSessionMessage, DeleteSessionResponse, GetAgentProfileConfigMessage,
    GetAgentProfileConfigResponse, GetAgentProfileConfigsMessage,
    GetAgentProfileConfigsResponse, GetConfigMessage, GetConfigResponse, GetConfigsMessage,
    GetConfigsResponse, GetModelConfigsMessage, GetModelConfigsResponse, GitBranchesRequest,
    GitGetBranchesMessage, GitGetBranchesResponse, GitGetStatusMessage, GitGetStatusResponse,
    GitIsRepositoryMessage, GitIsRepositoryResponse, GitRepositoryPathRequest,
    I18nGetCurrentLanguageMessage, I18nGetCurrentLanguageResponse, I18nGetConfigMessage,
    I18nGetSupportedLanguagesMessage, I18nGetSupportedLanguagesResponse, I18nSetConfigMessage,
    I18nSetConfigResponse, I18nSetLanguageMessage, I18nSetLanguageResponse,
    ListPendingPermissionRequestsMessage, ListPendingPermissionRequestsResponse,
    ListProjectPermissionAuditMessage, ListProjectPermissionAuditResponse,
    ListProjectPermissionGrantsMessage, ListProjectPermissionGrantsResponse,
    ListSessionsMessage, ListSessionsResponse, PermissionEventNotification,
    RemoveProjectPermissionGrantMessage, RemoveProjectPermissionGrantResponse,
    RespondPermissionBatchMessage, RespondPermissionBatchResponse, RespondPermissionMessage,
    RespondPermissionResponse, RunMessage, RunResponse, SessionEventNotification,
    SetConfigMessage, SetConfigResponse, SubmitDialogTurnMessage, SubmitTurnMessage,
    SubmitTurnResponse,
};
use crate::{AppClient, AppServer};

/// Projected frontend event forwarded to consumers (websocket connections,
/// Tauri event bridge, ...).
#[derive(Debug, Clone)]
pub struct FrontendEvent {
    /// Frontend event name (`agent/event` projection).
    pub event: String,
    /// Projected payload, ready to serialize for the consumer's wire shape.
    pub payload: serde_json::Value,
}

/// Startup grace for the in-process app-server client connection.
const CLIENT_STARTUP_TIMEOUT: Duration = Duration::from_secs(5);

/// Client handle for an app-server connection.
///
/// Cheaply cloneable: each clone shares the connection handle, the event
/// broadcast sender, and the shutdown signal. Use [`AppServerClient::rpc`]
/// (or the typed helpers) to route an agent kernel RPC through the app-server
/// surface, and [`AppServerClient::subscribe_events`] to receive projected
/// runtime events.
///
/// The connection is parked for the lifetime of this handle: the in-process
/// `connect_task` that drives the `AppClient` connection parks on the shutdown
/// receiver, and the `shutdown_tx` lives in the shared handle so the parked
/// main loop only resumes when [`AppServerClient::shutdown`] is called or the
/// last clone is dropped. Hosts that want the connection to live as long as
/// the process (the Server Host pattern) simply hold a clone and never call
/// `shutdown`.
#[derive(Clone)]
pub struct AppServerClient {
    connection: Arc<ConnectionTo<AppServer>>,
    event_tx: broadcast::Sender<FrontendEvent>,
    shutdown_tx: Arc<std::sync::Mutex<Option<oneshot::Sender<()>>>>,
}

impl AppServerClient {
    /// Subscribe to projected frontend events. One receiver per consumer
    /// (websocket connection, Tauri bridge, ...); dropping the last receiver
    /// does not close the server.
    pub fn subscribe_events(&self) -> broadcast::Receiver<FrontendEvent> {
        self.event_tx.subscribe()
    }

    /// Shut down the in-process app-server client connection. Signals the
    /// parked `connect_task` to resume, which lets the connection's background
    /// actors (task/outgoing/incoming/responder) unwind and close the
    /// transport. Safe to call more than once; subsequent calls are no-ops.
    /// Hosts that want the connection to live for the process lifetime simply
    /// never call this.
    pub async fn shutdown(&self) {
        if let Some(tx) = self.shutdown_tx.lock().ok().and_then(|mut guard| guard.take()) {
            let _ = tx.send(());
        }
    }

    /// Create an agent session via `agent/createSession`.
    pub async fn create_session(
        &self,
        request: bitfun_agent_runtime::sdk::AgentSessionCreateRequest,
    ) -> Result<bitfun_agent_runtime::sdk::AgentSessionCreateResult> {
        let CreateSessionResponse(inner) = self
            .rpc(|cx| cx.send_request(CreateSessionMessage(request)))
            .await?;
        Ok(inner)
    }

    /// List agent sessions via `agent/listSessions`.
    pub async fn list_sessions(
        &self,
        request: bitfun_agent_runtime::sdk::AgentSessionListRequest,
    ) -> Result<Vec<bitfun_agent_runtime::sdk::AgentSessionSummary>> {
        let ListSessionsResponse { sessions } = self
            .rpc(|cx| cx.send_request(ListSessionsMessage(request)))
            .await?;
        Ok(sessions)
    }

    /// Delete an agent session via `agent/deleteSession`.
    pub async fn delete_session(
        &self,
        request: bitfun_agent_runtime::sdk::AgentSessionDeleteRequest,
    ) -> Result<()> {
        let DeleteSessionResponse {} = self
            .rpc(|cx| cx.send_request(DeleteSessionMessage(request)))
            .await?;
        Ok(())
    }

    /// Submit a turn via `agent/submitTurn`.
    pub async fn submit_turn(
        &self,
        request: bitfun_agent_runtime::sdk::AgentSubmissionRequest,
    ) -> Result<bitfun_agent_runtime::sdk::AgentSubmissionResult> {
        let SubmitTurnResponse(inner) = self
            .rpc(|cx| cx.send_request(SubmitTurnMessage(request)))
            .await?;
        Ok(inner)
    }

    /// Submit a dialog turn via `agent/submitDialogTurn`.
    ///
    /// This is the operation the desktop `start_dialog_turn` command drives and
    /// the one web/CLI hosts should map `startDialogTurn`-style calls to: it
    /// carries `agentType`/`workspacePath`/`policy`, unlike `submit_turn` which
    /// is a bare message into an existing session. The response reports whether
    /// the turn started immediately or was queued.
    pub async fn submit_dialog_turn(
        &self,
        body: crate::schema::SubmitDialogTurnBody,
    ) -> Result<crate::schema::SubmitDialogTurnResponse> {
        self.rpc(|cx| cx.send_request(SubmitDialogTurnMessage(body)))
            .await
    }

    /// Cancel a turn via `agent/cancelTurn`.
    pub async fn cancel_turn(
        &self,
        request: bitfun_agent_runtime::sdk::AgentTurnCancellationRequest,
    ) -> Result<bitfun_agent_runtime::sdk::AgentTurnCancellationResult> {
        let CancelTurnResponse(inner) = self
            .rpc(|cx| cx.send_request(CancelTurnMessage(request)))
            .await?;
        Ok(inner)
    }

    /// Respond to a permission request via `agent/respondPermission`.
    pub async fn respond_permission(
        &self,
        request_id: &str,
        reply: bitfun_agent_runtime::sdk::PermissionReply,
    ) -> Result<()> {
        let RespondPermissionResponse {} = self
            .rpc(|cx| {
                cx.send_request(RespondPermissionMessage {
                    request_id: request_id.to_string(),
                    reply,
                })
            })
            .await?;
        Ok(())
    }

    /// Respond to a permission request and all requests sharing its reply via
    /// `agent/respondPermissionBatch`. Returns the request ids that shared the
    /// resolved reply.
    pub async fn respond_permission_batch(
        &self,
        request_id: &str,
        reply: bitfun_agent_runtime::sdk::PermissionReply,
    ) -> Result<Vec<String>> {
        let RespondPermissionBatchResponse { request_ids } = self
            .rpc(|cx| {
                cx.send_request(RespondPermissionBatchMessage {
                    request_id: request_id.to_string(),
                    reply,
                })
            })
            .await?;
        Ok(request_ids)
    }

    /// List pending permission requests via `agent/listPendingPermissionRequests`.
    pub async fn list_pending_permission_requests(
        &self,
    ) -> Result<Vec<bitfun_agent_runtime::sdk::PermissionRequest>> {
        let ListPendingPermissionRequestsResponse { requests } = self
            .rpc(|cx| cx.send_request(ListPendingPermissionRequestsMessage {}))
            .await?;
        Ok(requests)
    }

    /// List project permission grants via `agent/listProjectPermissionGrants`.
    pub async fn list_project_permission_grants(
        &self,
        project_id: &str,
    ) -> Result<Vec<bitfun_agent_runtime::sdk::PermissionGrant>> {
        let ListProjectPermissionGrantsResponse { grants } = self
            .rpc(|cx| {
                cx.send_request(ListProjectPermissionGrantsMessage {
                    project_id: project_id.to_string(),
                })
            })
            .await?;
        Ok(grants)
    }

    /// Remove a project permission grant via `agent/removeProjectPermissionGrant`.
    pub async fn remove_project_permission_grant(
        &self,
        key: bitfun_agent_runtime::sdk::PermissionGrantKey,
    ) -> Result<bool> {
        let RemoveProjectPermissionGrantResponse { removed } = self
            .rpc(|cx| cx.send_request(RemoveProjectPermissionGrantMessage(key)))
            .await?;
        Ok(removed)
    }

    /// Clear all permission grants for a project via
    /// `agent/clearProjectPermissionGrants`. Returns the count cleared.
    pub async fn clear_project_permission_grants(
        &self,
        project_id: &str,
    ) -> Result<usize> {
        let ClearProjectPermissionGrantsResponse { cleared } = self
            .rpc(|cx| {
                cx.send_request(ClearProjectPermissionGrantsMessage {
                    project_id: project_id.to_string(),
                })
            })
            .await?;
        Ok(cleared)
    }

    /// List the permission audit log for a project via
    /// `agent/listProjectPermissionAudit`.
    pub async fn list_project_permission_audit(
        &self,
        project_id: &str,
    ) -> Result<Vec<bitfun_agent_runtime::sdk::PermissionAuditRecord>> {
        let ListProjectPermissionAuditResponse { records } = self
            .rpc(|cx| {
                cx.send_request(ListProjectPermissionAuditMessage {
                    project_id: project_id.to_string(),
                })
            })
            .await?;
        Ok(records)
    }

    /// Run an agent turn (create-or-existing) via `agent/run`.
    pub async fn run(&self, request: RunMessage) -> Result<RunResponse> {
        self.rpc(|cx| cx.send_request(request)).await
    }

    /// Check whether a path is a Git repository via `git/isRepository`.
    pub async fn git_is_repository(&self, repository_path: &str) -> Result<bool> {
        let GitIsRepositoryResponse(value) = self
            .rpc(|cx| {
                cx.send_request(GitIsRepositoryMessage(GitRepositoryPathRequest {
                    repository_path: repository_path.to_string(),
                }))
            })
            .await?;
        Ok(value)
    }

    /// Get the working-tree status of a Git repository via `git/getStatus`.
    pub async fn git_get_status(
        &self,
        repository_path: &str,
    ) -> Result<bitfun_core::service::git::GitStatus> {
        let GitGetStatusResponse(status) = self
            .rpc(|cx| {
                cx.send_request(GitGetStatusMessage(GitRepositoryPathRequest {
                    repository_path: repository_path.to_string(),
                }))
            })
            .await?;
        Ok(status)
    }

    /// List the branches of a Git repository via `git/getBranches`.
    pub async fn git_get_branches(
        &self,
        repository_path: &str,
        include_remote: bool,
    ) -> Result<Vec<bitfun_core::service::git::GitBranch>> {
        let GitGetBranchesResponse { branches } = self
            .rpc(|cx| {
                cx.send_request(GitGetBranchesMessage(GitBranchesRequest {
                    repository_path: repository_path.to_string(),
                    include_remote: Some(include_remote),
                }))
            })
            .await?;
        Ok(branches)
    }

    /// List all agent profile configs via `config/getAgentProfileConfigs`.
    pub async fn get_agent_profile_configs(
        &self,
    ) -> Result<std::collections::HashMap<String, bitfun_core::service::config::AgentProfileView>>
    {
        let GetAgentProfileConfigsResponse { profiles } = self
            .rpc(|cx| cx.send_request(GetAgentProfileConfigsMessage {}))
            .await?;
        Ok(profiles)
    }

    /// Get a single agent profile config via `config/getAgentProfileConfig`.
    pub async fn get_agent_profile_config(
        &self,
        agent_id: &str,
    ) -> Result<bitfun_core::service::config::AgentProfileView> {
        let GetAgentProfileConfigResponse(view) = self
            .rpc(|cx| {
                cx.send_request(GetAgentProfileConfigMessage {
                    agent_id: agent_id.to_string(),
                })
            })
            .await?;
        Ok(view)
    }

    /// List all AI model configs via `config/getModelConfigs`.
    pub async fn get_model_configs(
        &self,
    ) -> Result<Vec<bitfun_core::service::config::AIModelConfig>> {
        let GetModelConfigsResponse { models } = self
            .rpc(|cx| cx.send_request(GetModelConfigsMessage {}))
            .await?;
        Ok(models)
    }

    /// Read a single config path via `config/getConfig`. Returns the raw JSON
    /// value at that path (`None` path reads the whole config tree). A missing
    /// path surfaces as an error whose `message` matches the desktop contract
    /// (`Failed to get config: Not found: Config path '<path>' not found`) so
    /// the frontend `ConfigAPI.getConfig` can swallow it into `undefined`.
    pub async fn get_config(&self, path: Option<&str>) -> Result<serde_json::Value> {
        let GetConfigResponse(value) = self
            .rpc(|cx| {
                cx.send_request(GetConfigMessage {
                    path: path.map(str::to_string),
                    skip_retry_on_not_found: false,
                })
            })
            .await?;
        Ok(value)
    }

    /// Read multiple config paths in one batch via `config/getConfigs`.
    /// Returns a path -> value map; paths are deduped server-side the way the
    /// desktop host does. The first not-found path aborts the batch and
    /// surfaces the same not-found message shape as [`get_config`].
    pub async fn get_configs(
        &self,
        paths: &[String],
    ) -> Result<std::collections::BTreeMap<String, serde_json::Value>> {
        let GetConfigsResponse { configs } = self
            .rpc(|cx| {
                cx.send_request(GetConfigsMessage {
                    paths: paths.to_vec(),
                    skip_retry_on_not_found: false,
                })
            })
            .await?;
        Ok(configs)
    }

    /// Write a value at a config path via `config/setConfig` (Track B). The
    /// handler reaches the global config singleton the Desktop host uses.
    pub async fn set_config(&self, path: &str, value: serde_json::Value) -> Result<()> {
        let SetConfigResponse {} = self
            .rpc(|cx| {
                cx.send_request(SetConfigMessage {
                    path: path.to_string(),
                    value,
                })
            })
            .await?;
        Ok(())
    }

    /// Read the current runtime locale id via `i18n/getCurrentLanguage`.
    pub async fn i18n_get_current_language(&self) -> Result<String> {
        let I18nGetCurrentLanguageResponse { language } = self
            .rpc(|cx| cx.send_request(I18nGetCurrentLanguageMessage {}))
            .await?;
        Ok(language)
    }

    /// Set the runtime locale via `i18n/setLanguage` and sync the global
    /// I18nService. Returns the validated locale id that was applied.
    pub async fn i18n_set_language(&self, language: &str) -> Result<String> {
        let I18nSetLanguageResponse { language } = self
            .rpc(|cx| {
                cx.send_request(I18nSetLanguageMessage {
                    language: language.to_string(),
                })
            })
            .await?;
        Ok(language)
    }

    /// Read the i18n config (current/fallback language, autoDetect) via
    /// `i18n/getConfig`.
    pub async fn i18n_get_config(
        &self,
    ) -> Result<crate::schema::I18nGetConfigResponse> {
        let response = self
            .rpc(|cx| cx.send_request(I18nGetConfigMessage {}))
            .await?;
        Ok(response)
    }

    /// Write the i18n config (writes `currentLanguage` and syncs the I18nService)
    /// via `i18n/setConfig`.
    pub async fn i18n_set_config(
        &self,
        current_language: Option<&str>,
        fallback_language: Option<&str>,
        auto_detect: bool,
    ) -> Result<()> {
        let I18nSetConfigResponse {} = self
            .rpc(|cx| {
                cx.send_request(I18nSetConfigMessage {
                    current_language: current_language.map(str::to_string),
                    fallback_language: fallback_language.map(str::to_string),
                    auto_detect,
                })
            })
            .await?;
        Ok(())
    }

    /// List all supported locales via `i18n/getSupportedLanguages`.
    pub async fn i18n_get_supported_languages(
        &self,
    ) -> Result<Vec<crate::schema::I18nLocaleMetadata>> {
        let I18nGetSupportedLanguagesResponse { locales } = self
            .rpc(|cx| cx.send_request(I18nGetSupportedLanguagesMessage {}))
            .await?;
        Ok(locales)
    }

    /// Send a JSON-RPC request through the app-server connection and await its
    /// response. Uses the canonical `on_receiving_result` + oneshot pattern
    /// (mirrors the app-server round-trip tests) so the calling task is not
    /// blocked on the connection driver.
    async fn rpc<F, R>(&self, send: F) -> Result<R>
    where
        F: FnOnce(&ConnectionTo<AppServer>) -> SentRequest<R>,
        R: JsonRpcResponse + Send,
    {
        let sent = send(&self.connection);
        let (tx, rx) = oneshot::channel();
        sent.on_receiving_result(async move |result| {
            tx.send(result)
                .map_err(|_| agent_client_protocol::Error::internal_error())
        })?;
        rx.await
            .map_err(|_| agent_client_protocol::Error::internal_error())?
    }
}

/// Connect an app-server client over a host-supplied transport.
///
/// Drives an [`AppClient`] on `transport`, parks the [`ConnectionTo<AppServer>`]
/// for the lifetime of the returned handle, and fans projected `agent/event`
/// notifications out through the returned [`AppServerClient`]. The host owns the
/// transport and the server half of the connection (for example it spawns
/// [`crate::BitfunAppServer::serve`] on the server half of an
/// [`crate::transport::in_memory_channel_pair`] before calling this).
///
/// The connection is parked for the lifetime of the host: the shutdown signal
/// is intentionally never sent, so the spawned connection task (which owns the
/// transport) stays alive while the [`AppServerClient`] is in use.
///
/// # Errors
///
/// Returns an error if the connection closes or the startup handshake does not
/// complete within the startup grace window.
pub async fn connect(
    transport: impl ConnectTo<AppClient> + 'static,
) -> std::result::Result<AppServerClient, anyhow::Error> {
    // Event fan-out: the client receives `agent/event` notifications forwarded
    // by the app-server, projects each envelope to the frontend shape, and
    // broadcasts it to consumers.
    let (event_tx, _) = broadcast::channel::<FrontendEvent>(1024);
    let event_tx_for_task = event_tx.clone();

    // Park the connection handle through a oneshot, then await a never-sent
    // shutdown signal so `connect_with`'s main_fn keeps the connection alive.
    let (cx_tx, cx_rx) = oneshot::channel::<ConnectionTo<AppServer>>();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let connect_task = tokio::spawn(async move {
        let result = AppClient
            .builder()
            .name("bitfun-app-client")
            .on_receive_notification(
                {
                    let event_tx = event_tx_for_task.clone();
                    async move |notification: SessionEventNotification,
                                _cx: ConnectionTo<AppServer>| {
                        let SessionEventNotification(envelope) = notification;
                        if let Some(projected) = project_agentic_frontend_event(envelope.event) {
                            let _ = event_tx.send(FrontendEvent {
                                event: projected.event_name,
                                payload: projected.payload,
                            });
                        }
                        Ok(())
                    }
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_notification(
                {
                    let event_tx = event_tx_for_task;
                    async move |notification: PermissionEventNotification,
                                _cx: ConnectionTo<AppServer>| {
                        let PermissionEventNotification(event) = notification;
                        // Project the permission lifecycle event to the frontend
                        // `permission://event` channel the desktop host uses, so
                        // consumers can listen on the same name in web and desktop.
                        if let Ok(payload) = serde_json::to_value(&event) {
                            let _ = event_tx.send(FrontendEvent {
                                event: "permission://event".to_string(),
                                payload,
                            });
                        }
                        Ok(())
                    }
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .connect_with(transport, async |cx: ConnectionTo<AppServer>| {
                let _ = cx_tx.send(cx);
                let _ = shutdown_rx.await;
                Ok(())
            })
            .await;
        if let Err(error) = &result {
            log::warn!("App-server client connection ended with error: {:?}", error);
        }
    });

    let connection = match tokio::time::timeout(CLIENT_STARTUP_TIMEOUT, cx_rx).await {
        Ok(Ok(cx)) => cx,
        Ok(Err(_)) => {
            connect_task.abort();
            anyhow::bail!("App-server client connection closed before startup completed");
        }
        Err(_) => {
            connect_task.abort();
            anyhow::bail!("App-server client connection startup timed out");
        }
    };

    // The connection is parked for the lifetime of the host: the `shutdown_tx`
    // lives in the returned handle so the parked `connect_task` main loop only
    // resumes when the host calls `shutdown` (or drops the last clone). The
    // Server Host holds the handle for the process lifetime and never calls
    // `shutdown`, so the connection -- and therefore the in-process app-server
    // `BitfunAppServer` it is paired with -- stays alive as long as the server
    // does. (Previously this was `let _ = shutdown_tx;`, which dropped the
    // sender immediately and let the parked main loop resume right after
    // `connect` returned, killing the connection -- every subsequent RPC then
    // surfaced as `send failed because receiver is gone`.)
    Ok(AppServerClient {
        connection: Arc::new(connection),
        event_tx,
        shutdown_tx: Arc::new(std::sync::Mutex::new(Some(shutdown_tx))),
    })
}
