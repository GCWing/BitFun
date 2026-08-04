//! BitFun agent kernel server backed by the generic `AppServer` role.
//!
//! [`BitfunAppServer`] wires JSON-RPC handlers for the agent kernel operations
//! (create / list / delete / submit / run / cancel) to a host-injected
//! [`BitfunAppRuntime`]. It mirrors `bitfun_acp::AcpServer` but uses the custom
//! [`AppServer`] role instead of the built-in ACP `Agent` role, so it binds no
//! ACP schema and consumers register their own message types (defined in
//! [`crate::schema`]).
//!
//! Handlers offload runtime calls to background tasks via `cx.spawn` and reply
//! through `responder.respond_with_result`, the same proven pattern as the ACP
//! server. The fallback `on_receive_dispatch` returns `method_not_found` so
//! unregistered methods surface cleanly to the client.
//!
//! The dispatch fallback also recognizes external-source method names (the old
//! Server Host `routes/external_sources.rs` surface) and returns a typed
//! "not available in web mode" error so the frontend can show a clear
//! unsupported-state message instead of a generic `method_not_found`.

use std::sync::Arc;

use agent_client_protocol::{ConnectTo, ConnectionTo, Dispatch, Error, Result};
use bitfun_agent_runtime::sdk::PermissionRequestEvent;
use bitfun_core::service::git::GitService;
use bitfun_events::project_agentic_frontend_event;

/// Method-name substrings for the external-source operations that the old Server
/// Host dispatched via `routes/external_sources.rs`. Under browser-direct ACP
/// these are not yet on the app-server schema; the dispatch fallback returns a
/// typed "not available in web mode" error so the frontend gets a clear signal
/// rather than a bare `method_not_found`.
const EXTERNAL_SOURCE_METHOD_MARKERS: &[&str] = &[
    "external_source",
    "external_tool",
    "external_subagent",
    "external_mcp",
    "external_integration",
];

use crate::agent::{
    bitfun_error, config_get_error, git_service_error, runtime_call, BitfunAppRuntime,
};
use crate::role::{AppClient, AppServer};
use crate::schema::{
    CancelTurnMessage, CancelTurnResponse, ClearProjectPermissionGrantsMessage,
    ClearProjectPermissionGrantsResponse, CreateSessionMessage, CreateSessionResponse,
    DeleteSessionMessage, DeleteSessionResponse, FrontendEventNotification,
    GetAgentProfileConfigMessage, GetAgentProfileConfigResponse, GetAgentProfileConfigsMessage,
    GetAgentProfileConfigsResponse, GetConfigMessage, GetConfigResponse, GetConfigsMessage,
    GetConfigsResponse, GetModelConfigsMessage, GetModelConfigsResponse, GitBranchesRequest,
    GitGetBranchesMessage, GitGetBranchesResponse, GitGetStatusMessage, GitGetStatusResponse,
    GitIsRepositoryMessage, GitIsRepositoryResponse, GitRepositoryPathRequest,
    ListPendingPermissionRequestsMessage, ListPendingPermissionRequestsResponse,
    ListProjectPermissionAuditMessage, ListProjectPermissionAuditResponse,
    ListProjectPermissionGrantsMessage, ListProjectPermissionGrantsResponse, ListSessionsMessage,
    ListSessionsResponse, RemoveProjectPermissionGrantMessage,
    RemoveProjectPermissionGrantResponse, RespondPermissionBatchMessage,
    RespondPermissionBatchResponse, RespondPermissionMessage, RespondPermissionResponse,
    RunMessage, RunResponse, SetConfigMessage, SetConfigResponse,
    SubmitDialogTurnMessage, SubmitDialogTurnResponse, SubmitTurnMessage, SubmitTurnResponse,
    I18nGetCurrentLanguageMessage, I18nGetCurrentLanguageResponse, I18nGetConfigMessage,
    I18nGetConfigResponse, I18nGetSupportedLanguagesMessage, I18nGetSupportedLanguagesResponse,
    I18nLocaleMetadata, I18nSetConfigMessage, I18nSetConfigResponse, I18nSetLanguageMessage,
    I18nSetLanguageResponse,
};

/// BitFun agent kernel server over the generic app-server role.
///
/// Holds a shared [`BitfunAppRuntime`]. Clone is cheap (Arc clone), so a host
/// can build one server and `serve` it on multiple transports, or keep a clone
/// around to spawn event-forwarding tasks.
#[derive(Clone)]
pub struct BitfunAppServer {
    runtime: Arc<BitfunAppRuntime>,
}

impl BitfunAppServer {
    pub fn new(runtime: BitfunAppRuntime) -> Self {
        Self {
            runtime: Arc::new(runtime),
        }
    }

    /// Shared runtime handle, for callers that want to spawn side tasks such
    /// as an event-forwarding loop on the same runtime.
    pub fn runtime(&self) -> &BitfunAppRuntime {
        &self.runtime
    }

    /// Serve the agent kernel surface on a transport. The transport must
    /// implement `ConnectTo<AppServer>` (for example the
    /// [`crate::transport::in_memory_channel_pair`] server half, or `ByteStreams`).
    pub async fn serve(self, transport: impl ConnectTo<AppServer> + 'static) -> Result<()> {
        let runtime = self.runtime;

        AppServer
            .builder()
            .name("bitfun-app-server")
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: CreateSessionMessage, responder, _cx| {
                        responder.respond_with_result(runtime_call(
                            runtime
                                .runtime()
                                .create_session(request.0)
                                .await
                                .map(CreateSessionResponse),
                        ))
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: ListSessionsMessage, responder, _cx| {
                        let sessions =
                            runtime_call(runtime.runtime().list_sessions(request.0).await)?;
                        responder.respond(ListSessionsResponse { sessions })
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: DeleteSessionMessage, responder, _cx| {
                        runtime_call(runtime.runtime().delete_session(request.0).await)?;
                        responder.respond(DeleteSessionResponse {})
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: SubmitTurnMessage, responder, _cx| {
                        let session_id = request.0.session_id.clone();
                        let result = runtime
                            .runtime()
                            .submit_turn(request.0)
                            .await
                            .map(SubmitTurnResponse)
                            .map_err(|err| {
                                BitfunAppRuntime::session_runtime_error(&session_id, err)
                            });
                        responder.respond_with_result(result)
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: SubmitDialogTurnMessage, responder, _cx| {
                        let session_id = request.0.session_id.clone();
                        let result = runtime
                            .runtime()
                            .submit_dialog_turn(request.0.to_request())
                            .await
                            .map(SubmitDialogTurnResponse::from_outcome)
                            .map_err(|err| {
                                BitfunAppRuntime::session_runtime_error(&session_id, err)
                            });
                        responder.respond_with_result(result)
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: RunMessage, responder, _cx| {
                        let run_request = request.to_run_request();
                        let handle = runtime_call(runtime.runtime().run(run_request).await)?;
                        responder.respond(RunResponse::from_handle(handle))
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: CancelTurnMessage, responder, _cx| {
                        responder.respond_with_result(runtime_call(
                            runtime
                                .runtime()
                                .cancel_turn(request.0)
                                .await
                                .map(CancelTurnResponse),
                        ))
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: RespondPermissionMessage, responder, _cx| {
                        runtime_call(
                            runtime
                                .runtime()
                                .respond_permission(&request.request_id, request.reply)
                                .await,
                        )?;
                        responder.respond(RespondPermissionResponse {})
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: RespondPermissionBatchMessage, responder, _cx| {
                        let request_ids = runtime_call(
                            runtime
                                .runtime()
                                .respond_permission_batch(&request.request_id, request.reply)
                                .await,
                        )?;
                        responder.respond(RespondPermissionBatchResponse { request_ids })
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |_request: ListPendingPermissionRequestsMessage,
                                responder,
                                _cx| {
                        let requests =
                            runtime_call(runtime.runtime().pending_permission_requests())?;
                        responder.respond(ListPendingPermissionRequestsResponse { requests })
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: ListProjectPermissionGrantsMessage,
                                responder,
                                _cx| {
                        let grants = runtime_call(
                            runtime
                                .runtime()
                                .list_project_permission_grants(&request.project_id)
                                .await,
                        )?;
                        responder.respond(ListProjectPermissionGrantsResponse { grants })
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: RemoveProjectPermissionGrantMessage,
                                responder,
                                _cx| {
                        let removed = runtime_call(
                            runtime
                                .runtime()
                                .remove_project_permission_grant(request.0)
                                .await,
                        )?;
                        responder.respond(RemoveProjectPermissionGrantResponse { removed })
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: ClearProjectPermissionGrantsMessage,
                                responder,
                                _cx| {
                        let cleared = runtime_call(
                            runtime
                                .runtime()
                                .clear_project_permission_grants(&request.project_id)
                                .await,
                        )?;
                        responder.respond(ClearProjectPermissionGrantsResponse { cleared })
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: ListProjectPermissionAuditMessage,
                                responder,
                                _cx| {
                        let records = runtime_call(
                            runtime
                                .runtime()
                                .list_project_permission_audit(&request.project_id)
                                .await,
                        )?;
                        responder.respond(ListProjectPermissionAuditResponse { records })
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: GitIsRepositoryMessage, responder, _cx| {
                    let GitRepositoryPathRequest { repository_path } = request.0;
                    let result = GitService::is_repository(&repository_path)
                        .await
                        .map(GitIsRepositoryResponse)
                        .map_err(git_service_error);
                    responder.respond_with_result(result)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: GitGetStatusMessage, responder, _cx| {
                    let GitRepositoryPathRequest { repository_path } = request.0;
                    let result = GitService::get_status(&repository_path)
                        .await
                        .map(GitGetStatusResponse)
                        .map_err(git_service_error);
                    responder.respond_with_result(result)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: GitGetBranchesMessage, responder, _cx| {
                    let GitBranchesRequest {
                        repository_path,
                        include_remote,
                    } = request.0;
                    let include_remote = include_remote.unwrap_or(false);
                    let result = GitService::get_branches(&repository_path, include_remote)
                        .await
                        .map(|branches| GitGetBranchesResponse { branches })
                        .map_err(git_service_error);
                    responder.respond_with_result(result)
                },
                agent_client_protocol::on_receive_request!(),
            )
            // Config service: read-only agent-profile and model-config reads.
            // The handlers reach the global config singletons the Desktop host
            // also uses -- `mode_config_canonicalizer::get_agent_profile_*`
            // (static) and `get_global_config_service` (`config_service
            // .get_ai_models`) -- so no injection is needed, mirroring the
            // static `GitService` pattern.
            .on_receive_request(
                async move |_request: GetAgentProfileConfigsMessage, responder, _cx| {
                    let result =
                        bitfun_core::service::config::mode_config_canonicalizer::get_agent_profile_views()
                            .await
                            .map(|profiles| GetAgentProfileConfigsResponse { profiles })
                            .map_err(bitfun_error);
                    responder.respond_with_result(result)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: GetAgentProfileConfigMessage, responder, _cx| {
                    let result =
                        bitfun_core::service::config::mode_config_canonicalizer::get_agent_profile_view(
                            &request.agent_id,
                        )
                        .await
                        .map(GetAgentProfileConfigResponse)
                        .map_err(bitfun_error);
                    responder.respond_with_result(result)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |_request: GetModelConfigsMessage, responder, _cx| {
                    let result = async {
                        let config_service =
                            bitfun_core::service::config::get_global_config_service().await?;
                        config_service.get_ai_models().await
                    }
                    .await
                    .map(|models| GetModelConfigsResponse { models })
                    .map_err(bitfun_error);
                    responder.respond_with_result(result)
                },
                agent_client_protocol::on_receive_request!(),
            )
            // `config/getConfig` / `config/getConfigs` -- single + batched
            // config-path reads. `ConfigService::get_config::<Value>(path)`
            // returns the raw JSON tree at that path; a missing path errors with
            // `BitFunError::NotFound("Config path '<path>' not found")`. The
            // `config_get_error` mapper puts that Display text into the JSON-RPC
            // `message` (not just `data`) so the frontend `ConfigAPI.getConfig`
            // substring match (`not found:` + `config path` + `'<path>'`) hits
            // and swallows the error into `undefined` the same way it does on
            // desktop. `skipRetryOnNotFound` rides along unbranched -- the
            // app-server does not retry; the field steers frontend retry policy
            // and desktop logging.
            .on_receive_request(
               async move |request: GetConfigMessage, responder, _cx| {
                    log::debug!("server getConfig request: {:?}", request);
                    let result = async {
                        let config_service =
                            bitfun_core::service::config::get_global_config_service().await?;
                        config_service
                            .get_config::<serde_json::Value>(request.path.as_deref())
                            .await
                    }
                    .await
                    .map(GetConfigResponse)
                    .map_err(config_get_error);
                    responder.respond_with_result(result)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: GetConfigsMessage, responder, _cx| {
                    let result = async {
                        let config_service =
                            bitfun_core::service::config::get_global_config_service().await?;
                        // Dedupe paths the same way the desktop host does
                        // (`config_api.rs::get_configs` skips a path already
                        // seen), preserving first-seen order in the map.
                        let mut configs = std::collections::BTreeMap::new();
                        for path in request.paths {
                            if configs.contains_key(&path) {
                                continue;
                            }
                            let value = config_service
                                .get_config::<serde_json::Value>(Some(path.as_str()))
                                .await?;
                            configs.insert(path, value);
                        }
                        Ok(configs)
                    }
                    .await
                    .map(|configs| GetConfigsResponse { configs })
                    .map_err(config_get_error);
                    responder.respond_with_result(result)
                },
                agent_client_protocol::on_receive_request!(),
            )
            // `config/setConfig` (Track B): write a value at a config path via
            // the global config singleton -- the same accessor the Desktop host
            // uses (`config_api.rs::set_config`). No service injection.
            .on_receive_request(
                async move |request: SetConfigMessage, responder, _cx| {
                    let result = async {
                        let config_service =
                            bitfun_core::service::config::get_global_config_service().await?;
                        config_service
                            .set_config::<serde_json::Value>(&request.path, request.value)
                            .await
                    }
                    .await
                    .map(|()| SetConfigResponse {})
                    .map_err(bitfun_error);
                    responder.respond_with_result(result)
                },
                agent_client_protocol::on_receive_request!(),
            )
            // I18n service surface (Track B): read/write the runtime locale via
            // the global config singleton (`app.language`) + the global
            // I18nService (`sync_global_i18n_service_locale`). Locale ids are
            // validated against `LocaleId::from_str`; an unsupported id surfaces
            // as an `invalid_request`-style error.
            .on_receive_request(
                async move |_request: I18nGetCurrentLanguageMessage, responder, _cx| {
                    let result = async {
                        let config_service =
                            bitfun_core::service::config::get_global_config_service().await?;
                        let lang: String = config_service
                            .get_config::<String>(Some("app.language"))
                            .await
                            .unwrap_or_else(|_| "zh-CN".to_string());
                        Ok::<_, bitfun_core::BitFunError>(lang)
                    }
                    .await
                    .map(|language| I18nGetCurrentLanguageResponse { language })
                    .map_err(bitfun_error);
                    responder.respond_with_result(result)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: I18nSetLanguageMessage, responder, _cx| {
                    let result = async {
                        let locale_id =
                            bitfun_core::service::i18n::LocaleId::from_str(&request.language)
                                .ok_or_else(|| {
                                    bitfun_core::BitFunError::validation(format!(
                                        "Unsupported language: {}",
                                        request.language
                                    ))
                                })?;
                        let config_service =
                            bitfun_core::service::config::get_global_config_service().await?;
                        config_service
                            .set_config("app.language", locale_id.as_str())
                            .await?;
                        // Sync the global I18nService; a non-initialized
                        // service logs but is not fatal (matches the host
                        // dispatcher's behavior).
                        let _ = bitfun_core::service::i18n::sync_global_i18n_service_locale(
                            locale_id,
                        )
                        .await;
                        Ok::<_, bitfun_core::BitFunError>(locale_id.as_str().to_string())
                    }
                    .await
                    .map(|language| I18nSetLanguageResponse { language })
                    .map_err(bitfun_error);
                    responder.respond_with_result(result)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |_request: I18nGetConfigMessage, responder, _cx| {
                    let result = async {
                        let config_service =
                            bitfun_core::service::config::get_global_config_service().await?;
                        let current_language = config_service
                            .get_config::<String>(Some("app.language"))
                            .await
                            .unwrap_or_else(|_| "zh-CN".to_string());
                        Ok::<_, bitfun_core::BitFunError>(I18nGetConfigResponse {
                            current_language,
                            fallback_language: "en-US".to_string(),
                            auto_detect: false,
                        })
                    }
                    .await
                    .map_err(bitfun_error);
                    responder.respond_with_result(result)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: I18nSetConfigMessage, responder, _cx| {
                    let result = async {
                        if let Some(language) = request.current_language.as_deref() {
                            let locale_id =
                                bitfun_core::service::i18n::LocaleId::from_str(language)
                                    .ok_or_else(|| {
                                        bitfun_core::BitFunError::validation(format!(
                                            "Unsupported language: {}",
                                            language
                                        ))
                                    })?;
                            let config_service =
                                bitfun_core::service::config::get_global_config_service().await?;
                            config_service
                                .set_config("app.language", locale_id.as_str())
                                .await?;
                            let _ = bitfun_core::service::i18n::sync_global_i18n_service_locale(
                                locale_id,
                            )
                            .await;
                        }
                        Ok::<_, bitfun_core::BitFunError>(())
                    }
                    .await
                    .map(|()| I18nSetConfigResponse {})
                    .map_err(bitfun_error);
                    responder.respond_with_result(result)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |_request: I18nGetSupportedLanguagesMessage, responder, _cx| {
                    let locales = bitfun_core::service::i18n::LocaleMetadata::all()
                        .into_iter()
                        .map(|locale| I18nLocaleMetadata {
                            id: locale.id.as_str().to_string(),
                            name: locale.name,
                            english_name: locale.english_name,
                            native_name: locale.native_name,
                            rtl: locale.rtl,
                        })
                        .collect();
                    responder.respond(I18nGetSupportedLanguagesResponse { locales })
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_dispatch(
                async move |message: Dispatch, cx: ConnectionTo<AppClient>| {
                    // Extract the method name so external-source commands get a
                    // typed "not available in web mode" error instead of a
                    // bare method_not_found.
                    let method = match &message {
                        Dispatch::Request(req, _) => req.method().to_string(),
                        _ => String::new(),
                    };
                    let is_external_source = EXTERNAL_SOURCE_METHOD_MARKERS
                        .iter()
                        .any(|marker| method.contains(marker));
                    let error = if is_external_source {
                        Error::method_not_found().data(serde_json::json!({
                            "capability": "external_sources",
                            "reason": "not_available_in_web_mode",
                            "message": "External source operations are not yet available in web mode. Use the desktop host."
                        }))
                    } else {
                        Error::method_not_found()
                    };
                    message.respond_with_error(error, cx)
                },
                agent_client_protocol::on_receive_dispatch!(),
            )
            // Drive the connection with a `main_fn` instead of `connect_to` so the
            // server can forward runtime events to the client as `agent/event`
            // notifications. This loop runs concurrently with the request handlers
            // above and parks the connection for its lifetime; `connect_with`
            // cancels it when the transport closes (same lifecycle as the
            // `connect_to` pending-main pattern).
            .connect_with(transport, async move |cx: ConnectionTo<AppClient>| {
                let mut rx = runtime.event_source().subscribe();
                // The permission receiver carries the same lifecycle stream the
                // desktop host emits as `permission://event`; forward each event
                // as an `agent/permissionEvent` notification so the client never
                // subscribes to the runtime permission stream directly. If the
                // runtime has no permission manager the subscription fails -- the
                // permission commands still work, this connection just receives
                // no permission push, so we drain runtime events only.
                let mut permission_rx = runtime
                    .runtime()
                    .subscribe_permission_requests()
                    .ok();
                loop {
                    let permission_recv = async {
                        match &mut permission_rx {
                            Some(receiver) => Some(receiver.recv().await),
                            // No permission stream available: park forever so
                            // this select! arm never fires.
                            None => std::future::pending::<
                                Option<
                                    Result<
                                        PermissionRequestEvent,
                                        tokio::sync::broadcast::error::RecvError,
                                    >,
                                >,
                            >()
                            .await,
                        }
                    };
                    tokio::select! {
                        recv = rx.recv() => match recv {
                            Ok(envelope) => {
                                // Project the runtime event to the frontend
                                // (`agentic://<type>`) shape the browser listens on
                                // today, and push it as a `agent/frontendEvent`
                                // notification. The browser's WS adapter dispatches on
                                // `params.event`, so its existing `listen(...)` call
                                // sites stay unchanged under browser-direct ACP.
                                if let Some(projected) =
                                    project_agentic_frontend_event(envelope.event)
                                {
                                    let notification = FrontendEventNotification {
                                        event: projected.event_name,
                                        payload: projected.payload,
                                    };
                                    if let Err(error) = cx.send_notification(notification) {
                                        log::warn!(
                                            "App-server event forwarder failed to send a notification: {:?} -- skipping this event",
                                            error
                                        );
                                        continue;
                                    }
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                                log::warn!(
                                    "App-server event forwarder lagged behind the runtime queue: {} events missed",
                                    missed
                                );
                                continue;
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                log::warn!(
                                    "App-server event forwarder stream closed -- serve main loop exiting (client RPCs will now fail with 'receiver is gone')"
                                );
                                break;
                            }
                        },
                        recv = permission_recv => match recv {
                            Some(Ok(event)) => {
                                // Project the permission lifecycle event to the
                                // `permission://event` channel the browser listens on
                                // (same name as the desktop host's
                                // `app.emit("permission://event")`), and push it as a
                                // `agent/frontendEvent` notification. The payload is the
                                // serialized `PermissionRequestEvent` (the same shape
                                // `client.rs` projected to before Step 2).
                                if let Ok(payload) = serde_json::to_value(&event) {
                                    let notification = FrontendEventNotification {
                                        event: "permission://event".to_string(),
                                        payload,
                                    };
                                    if let Err(error) = cx.send_notification(notification) {
                                        log::warn!(
                                            "App-server permission event forwarder failed to send a notification: {:?} -- skipping this event",
                                            error
                                        );
                                        continue;
                                    }
                                }
                            }
                            Some(Err(
                                tokio::sync::broadcast::error::RecvError::Lagged(missed),
                            )) => {
                                log::warn!(
                                    "App-server permission event forwarder lagged: {} events missed",
                                    missed
                                );
                                continue;
                            }
                            // Closed: drop the permission stream but keep
                            // forwarding runtime events for the connection's life.
                            Some(Err(
                                tokio::sync::broadcast::error::RecvError::Closed,
                            )) => {
                                permission_rx = None;
                            }
                            None => {}
                        },
                    }
                }
                Ok(())
            })
            .await
    }
}
