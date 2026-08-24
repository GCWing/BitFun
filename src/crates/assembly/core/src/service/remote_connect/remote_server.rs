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
use serde_json::Value;
use std::sync::{Arc, OnceLock};

use super::encryption;
use bitfun_services_integrations::remote_connect::{
    acp_cancel_response, acp_commands_response, acp_native_session_control_unsupported,
    acp_native_tool_interaction_unsupported, acp_options_response, acp_permission_respond_response,
    acp_permission_respond_unsupported, acp_plan_response, acp_send_response,
    build_remote_image_contexts, cancel_remote_task, generate_remote_initial_sync,
    handle_remote_command, handle_remote_interaction_command, handle_remote_poll_command,
    handle_remote_session_command, handle_remote_workspace_command,
    handle_remote_workspace_file_command, parse_remote_command, submit_remote_dialog,
    RemoteAcpCancelRequest, RemoteAcpControlError, RemoteAcpControlRuntimeHost,
    RemoteAcpGetCommandsRequest, RemoteAcpGetOptionsRequest, RemoteAcpGetPlanRequest,
    RemoteAcpPermissionRespondRequest, RemoteAcpSendRequest, RemoteAcpSetOptionRequest,
    RemoteCancelTaskRequest, RemoteCommandParseError, RemoteCommandRuntimeHost,
    RemoteConnectSubmissionSource, RemoteDialogSubmissionPolicy, RemoteDialogSubmissionRequest,
    RemoteDialogSubmitOutcome, RemoteImageContext, RemoteSessionTrackerRegistry,
    ACP_SESSION_REQUIRES_ACP_CONTROL_MESSAGE, UNSUPPORTED_REMOTE_CAPABILITY,
};
pub use bitfun_services_integrations::remote_connect::{
    ActiveTurnSnapshot, AssistantEntry, ChatImageAttachment, ChatMessage, ChatMessageItem,
    ImageAttachment, RecentWorkspaceEntry, RemoteCommand, RemoteDefaultModelsConfig,
    RemoteModelCatalog, RemoteModelConfig, RemoteResponse, RemoteSessionStateTracker,
    RemoteToolStatus, SessionInfo, TrackerEvent,
};

pub type EncryptedPayload = (String, String);

static ACP_CONTROL_HOST: OnceLock<Arc<dyn RemoteAcpControlRuntimeHost>> = OnceLock::new();

/// Inject the Desktop-owned ACP remote-control adapter. Safe to call once at
/// startup; later calls are ignored so tests/restarts do not panic.
pub fn set_remote_acp_control_host(host: Arc<dyn RemoteAcpControlRuntimeHost>) {
    let _ = ACP_CONTROL_HOST.set(host);
}

pub fn remote_acp_control_host_installed() -> bool {
    ACP_CONTROL_HOST.get().is_some()
}

pub fn clear_remote_acp_control_session(session_id: &str) {
    if let Some(host) = ACP_CONTROL_HOST.get() {
        host.clear_session_idempotency(session_id);
    }
}

fn remote_acp_control_host() -> Option<Arc<dyn RemoteAcpControlRuntimeHost>> {
    ACP_CONTROL_HOST.get().cloned()
}

#[derive(Debug, Clone, PartialEq)]
pub enum DecryptedRemoteEnvelope {
    Command {
        command: RemoteCommand,
        request_id: Option<String>,
    },
    Rejected {
        response: RemoteResponse,
        request_id: Option<String>,
    },
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

/// Shared tracker adapter for remote relay and bot execution paths.
///
/// Command routing lives in `bitfun-services-integrations`; core only keeps the
/// global tracker registry adapter needed by concrete session/runtime hosts.
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

struct CoreRemoteCommandRuntimeHost<'a> {
    dispatcher: &'a RemoteExecutionDispatcher,
}

impl<'a> CoreRemoteCommandRuntimeHost<'a> {
    fn new(dispatcher: &'a RemoteExecutionDispatcher) -> Self {
        Self { dispatcher }
    }
}

#[async_trait::async_trait]
impl RemoteCommandRuntimeHost for CoreRemoteCommandRuntimeHost<'_> {
    type ImageContext = crate::agentic::image_analysis::ImageContextData;

    async fn handle_workspace_command(&self, command: &RemoteCommand) -> RemoteResponse {
        let host = CoreServiceAgentRuntime::remote_workspace_host();
        handle_remote_workspace_command(&host, command).await
    }

    async fn handle_session_command(&self, command: &RemoteCommand) -> RemoteResponse {
        let host = match CoreServiceAgentRuntime::remote_session_host() {
            Ok(host) => host,
            Err(message) => {
                return RemoteResponse::Error {
                    message,
                    code: None,
                }
            }
        };
        handle_remote_session_command(&host, command).await
    }

    async fn handle_poll_command(&self, command: &RemoteCommand) -> RemoteResponse {
        let host = CoreServiceAgentRuntime::remote_poll_host(self.dispatcher);
        handle_remote_poll_command(&host, command).await
    }

    async fn handle_workspace_file_command(&self, command: &RemoteCommand) -> RemoteResponse {
        let host = CoreServiceAgentRuntime::remote_workspace_file_host();
        handle_remote_workspace_file_command(&host, command).await
    }

    async fn handle_interaction_command(&self, command: &RemoteCommand) -> RemoteResponse {
        let host = CoreServiceAgentRuntime::remote_interaction_host();
        handle_remote_interaction_command(&host, command).await
    }

    async fn handle_device_command(&self, command: &RemoteCommand) -> RemoteResponse {
        match command {
            RemoteCommand::DeviceQueryInfo => {
                use bitfun_runtime_ports::RemoteSessionWorkspaceIdentity;
                use bitfun_services_integrations::remote_connect::{
                    RemoteInitialSyncRuntimeHost, RemoteWorkspaceRuntimeHost,
                };

                let workspace_host = CoreServiceAgentRuntime::remote_workspace_host();
                let workspace =
                    RemoteWorkspaceRuntimeHost::current_workspace(&workspace_host).await;
                let session_count = if let Some(ref facts) = workspace {
                    let sync_host = CoreServiceAgentRuntime::remote_initial_sync_host();
                    let identity = RemoteSessionWorkspaceIdentity::from_workspace(facts);
                    match RemoteInitialSyncRuntimeHost::list_session_metadata(
                        &sync_host,
                        std::path::Path::new(&facts.path),
                        identity,
                    )
                    .await
                    {
                        Ok(metadata) => Some(metadata.len()),
                        Err(_) => None,
                    }
                } else {
                    Some(0)
                };

                RemoteResponse::DeviceInfo {
                    device_name: None,
                    workspace_path: workspace.as_ref().map(|facts| facts.path.clone()),
                    workspace_kind: workspace
                        .as_ref()
                        .map(|facts| facts.kind.as_wire_str().to_string()),
                    remote_connection_id: workspace
                        .as_ref()
                        .and_then(|facts| facts.remote_connection_id.clone()),
                    remote_ssh_host: workspace
                        .as_ref()
                        .and_then(|facts| facts.remote_ssh_host.clone()),
                    session_count,
                }
            }
            RemoteCommand::CreateWorkspace { path } => {
                let path = std::path::PathBuf::from(path);
                // Create the directory if it doesn't exist, then open it
                // as a workspace via the workspace manager.
                if let Err(e) = std::fs::create_dir_all(&path) {
                    return RemoteResponse::Error {
                        message: format!("Failed to create workspace directory: {e}"),
                        code: None,
                    };
                }
                // Now delegate to the workspace host to actually open/track it.
                let host = CoreServiceAgentRuntime::remote_workspace_host();
                handle_remote_workspace_command(
                    &host,
                    &RemoteCommand::SetWorkspace {
                        path: path.to_string_lossy().to_string(),
                        remote_connection_id: None,
                        remote_ssh_host: None,
                    },
                )
                .await
            }
            _ => RemoteResponse::Error {
                message: "Unsupported device command".to_string(),
                code: None,
            },
        }
    }

    async fn submit_dialog(
        &self,
        request: RemoteDialogSubmissionRequest<Self::ImageContext>,
    ) -> std::result::Result<RemoteDialogSubmitOutcome, String> {
        let host = CoreServiceAgentRuntime::remote_dialog_host(self.dispatcher)?;
        submit_remote_dialog(&host, request).await
    }

    async fn cancel_task(
        &self,
        request: RemoteCancelTaskRequest,
    ) -> std::result::Result<(), String> {
        let host = CoreServiceAgentRuntime::remote_cancel_host()?;
        cancel_remote_task(&host, request).await
    }

    async fn handle_acp_control_command(&self, command: &RemoteCommand) -> RemoteResponse {
        match command {
            RemoteCommand::AcpPermissionRespond {
                session_id,
                permission_id,
                option_id,
                request_id,
            } => {
                let Some(host) = remote_acp_control_host() else {
                    return acp_permission_respond_unsupported(session_id, request_id.clone());
                };
                acp_permission_respond_response(
                    host.permission_respond(RemoteAcpPermissionRespondRequest {
                        session_id: session_id.clone(),
                        permission_id: permission_id.clone(),
                        option_id: option_id.clone(),
                        request_id: request_id.clone(),
                    })
                    .await,
                )
            }
            RemoteCommand::AcpSendMessage {
                session_id,
                content,
                images,
                image_contexts,
                request_id,
            } => {
                let Some(host) = remote_acp_control_host() else {
                    return RemoteAcpControlError::unsupported(
                        session_id.clone(),
                        request_id.clone(),
                    )
                    .into_response();
                };
                acp_send_response(
                    host.send_message(RemoteAcpSendRequest {
                        session_id: session_id.clone(),
                        content: content.clone(),
                        images: images.clone(),
                        image_contexts: image_contexts.clone(),
                        request_id: request_id.clone(),
                    })
                    .await,
                )
            }
            RemoteCommand::AcpCancelTurn {
                session_id,
                turn_id,
                request_id,
            } => {
                let Some(host) = remote_acp_control_host() else {
                    return RemoteAcpControlError::unsupported(
                        session_id.clone(),
                        request_id.clone(),
                    )
                    .into_response();
                };
                acp_cancel_response(
                    host.cancel_turn(RemoteAcpCancelRequest {
                        session_id: session_id.clone(),
                        turn_id: turn_id.clone(),
                        request_id: request_id.clone(),
                    })
                    .await,
                )
            }
            RemoteCommand::AcpGetOptions {
                session_id,
                request_id,
            } => {
                let Some(host) = remote_acp_control_host() else {
                    return RemoteAcpControlError::unsupported(
                        session_id.clone(),
                        request_id.clone(),
                    )
                    .into_response();
                };
                acp_options_response(
                    host.get_options(RemoteAcpGetOptionsRequest {
                        session_id: session_id.clone(),
                        request_id: request_id.clone(),
                    })
                    .await,
                )
            }
            RemoteCommand::AcpSetOption {
                session_id,
                config_id,
                value,
                request_id,
            } => {
                let Some(host) = remote_acp_control_host() else {
                    return RemoteAcpControlError::unsupported(
                        session_id.clone(),
                        request_id.clone(),
                    )
                    .into_response();
                };
                acp_options_response(
                    host.set_option(RemoteAcpSetOptionRequest {
                        session_id: session_id.clone(),
                        config_id: config_id.clone(),
                        value: value.clone(),
                        request_id: request_id.clone(),
                    })
                    .await,
                )
            }
            RemoteCommand::AcpGetCommands {
                session_id,
                request_id,
            } => {
                let Some(host) = remote_acp_control_host() else {
                    return RemoteAcpControlError::unsupported(
                        session_id.clone(),
                        request_id.clone(),
                    )
                    .into_response();
                };
                acp_commands_response(
                    host.get_commands(RemoteAcpGetCommandsRequest {
                        session_id: session_id.clone(),
                        request_id: request_id.clone(),
                    })
                    .await,
                )
            }
            RemoteCommand::AcpGetPlan {
                session_id,
                request_id,
            } => {
                let Some(host) = remote_acp_control_host() else {
                    return RemoteAcpControlError::unsupported(
                        session_id.clone(),
                        request_id.clone(),
                    )
                    .into_response();
                };
                acp_plan_response(
                    host.get_plan(RemoteAcpGetPlanRequest {
                        session_id: session_id.clone(),
                        request_id: request_id.clone(),
                    })
                    .await,
                )
            }
            _ => RemoteResponse::Error {
                message: format!(
                    "{UNSUPPORTED_REMOTE_CAPABILITY}: {ACP_SESSION_REQUIRES_ACP_CONTROL_MESSAGE}"
                ),
                code: Some(UNSUPPORTED_REMOTE_CAPABILITY.to_string()),
            },
        }
    }

    async fn reject_native_session_control_for_acp(
        &self,
        session_id: &str,
        command_name: &str,
    ) -> Option<RemoteResponse> {
        let host = remote_acp_control_host()?;
        if !host.is_acp_session(session_id).await {
            return None;
        }
        Some(acp_native_session_control_unsupported(
            session_id,
            command_name,
        ))
    }

    async fn reject_native_tool_interaction_for_acp(
        &self,
        session_id: Option<&str>,
        tool_id: &str,
    ) -> Option<RemoteResponse> {
        let host = remote_acp_control_host()?;
        if let Some(session_id) = session_id {
            if host.is_acp_session(session_id).await {
                return Some(acp_native_tool_interaction_unsupported(
                    Some(session_id),
                    tool_id,
                ));
            }
        }
        if host.is_acp_permission_id(tool_id).await {
            return Some(acp_native_tool_interaction_unsupported(session_id, tool_id));
        }
        None
    }

    fn legacy_image_contexts(&self, images: Option<&[ImageAttachment]>) -> Vec<Self::ImageContext> {
        build_core_image_contexts(images)
    }

    fn explicit_image_contexts(
        &self,
        contexts: Vec<RemoteImageContext>,
    ) -> Vec<Self::ImageContext> {
        contexts
            .into_iter()
            .map(remote_image_context_to_core)
            .collect()
    }
}

// ── RemoteServer ───────────────────────────────────────────────────

/// Bridges encrypted remote payloads to the integrations-owned command router.
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
    ) -> Result<DecryptedRemoteEnvelope> {
        let json = encryption::decrypt_from_base64(&self.shared_secret, encrypted_data, nonce)?;
        let value: Value = serde_json::from_str(&json).map_err(|e| anyhow!("parse json: {e}"))?;
        let request_id = value
            .get("_request_id")
            .and_then(|v| v.as_str())
            .map(String::from);
        match parse_remote_command(value) {
            Ok(command) => Ok(DecryptedRemoteEnvelope::Command {
                command,
                request_id,
            }),
            // Both structured rejections must reach the client as a *response*.
            // Dropping `InvalidAcpParams` into the `Err` branch below would make
            // the host answer nothing at all, and the phone's silence probe
            // would then misreport a bad payload as "host too old".
            Err(
                error @ (RemoteCommandParseError::Unsupported { .. }
                | RemoteCommandParseError::InvalidAcpParams { .. }),
            ) => Ok(DecryptedRemoteEnvelope::Rejected {
                response: error.into_remote_response(),
                request_id,
            }),
            Err(RemoteCommandParseError::Invalid { message }) => {
                Err(anyhow!("parse command: {message}"))
            }
        }
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
        let dispatcher = get_or_init_global_dispatcher();
        let host = CoreRemoteCommandRuntimeHost::new(dispatcher.as_ref());
        handle_remote_command(&host, cmd, RemoteConnectSubmissionSource::Relay).await
    }

    pub async fn generate_initial_sync(
        &self,
        authenticated_user_id: Option<String>,
    ) -> RemoteResponse {
        let host = CoreServiceAgentRuntime::remote_initial_sync_host();
        generate_remote_initial_sync(&host, authenticated_user_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::remote_connect::encryption::KeyPair;
    use bitfun_services_integrations::remote_connect::{
        remote_session_restore_target, resolve_remote_cancel_decision,
        resolve_remote_execution_image_contexts, RemoteCancelDecision,
        RemoteDialogWorkspaceBinding,
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
        let DecryptedRemoteEnvelope::Command {
            command: decoded,
            request_id: req_id,
        } = bridge.decrypt_command(&enc, &nonce).unwrap()
        else {
            panic!("send_message should decrypt as a command");
        };

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

    /// Every structured rejection must come back as an answerable *response*
    /// carrying the original `_request_id`. If a malformed ACP payload were
    /// dropped into the transport-error branch instead, the host would go
    /// silent, and the client's silence probe would then misdiagnose a bad
    /// payload as "host predates the ACP command family".
    #[test]
    fn structured_acp_rejections_answer_instead_of_going_silent() {
        let alice = KeyPair::generate();
        let shared = alice.derive_shared_secret(&alice.public_key_bytes());
        let bridge = RemoteServer::new(shared);

        for (payload, expected_code) in [
            (
                // Known ACP command, `session_id` missing.
                serde_json::json!({
                    "cmd": "acp_get_plan",
                    "_request_id": "req_bad_params"
                }),
                bitfun_services_integrations::remote_connect::INVALID_ACP_COMMAND_PARAMS,
            ),
            (
                // Unknown future ACP command name.
                serde_json::json!({
                    "cmd": "acp_not_a_real_command",
                    "_request_id": "req_bad_params"
                }),
                bitfun_services_integrations::remote_connect::UNSUPPORTED_REMOTE_CAPABILITY,
            ),
        ] {
            let (enc, nonce) =
                encryption::encrypt_to_base64(&shared, &payload.to_string()).unwrap();
            let envelope = bridge
                .decrypt_command(&enc, &nonce)
                .expect("a structured rejection is not a transport failure");
            let DecryptedRemoteEnvelope::Rejected {
                response,
                request_id,
            } = envelope
            else {
                panic!("{payload} must be rejected, not accepted as a command");
            };
            assert_eq!(request_id.as_deref(), Some("req_bad_params"));
            match response {
                RemoteResponse::Error { code, .. } => {
                    assert_eq!(code.as_deref(), Some(expected_code), "for {payload}")
                }
                other => panic!("expected a coded error for {payload}, got {other:?}"),
            }
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

    #[tokio::test]
    async fn remote_answer_question_preserves_user_input_manager_path() {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        crate::agentic::tools::user_input_manager::get_user_input_manager()
            .register_channel("question-tool".to_string(), sender);
        let bridge = RemoteServer::new([7; 32]);
        let answers = serde_json::json!({ "choice": "yes" });

        let response = bridge
            .dispatch(&RemoteCommand::AnswerQuestion {
                tool_id: "question-tool".to_string(),
                answers: answers.clone(),
            })
            .await;

        assert_eq!(response, RemoteResponse::AnswerAccepted);
        assert_eq!(receiver.await.unwrap().answers, answers);
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
            image_path: Some("/workspace/project/screenshot.png".to_string()),
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
        let binding = RemoteDialogWorkspaceBinding::local("/workspace/project");

        assert_eq!(
            remote_session_restore_target(false, Some(&binding)),
            Some(binding.clone())
        );
        assert_eq!(remote_session_restore_target(true, Some(&binding)), None);
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
            remote_connection_id: None,
            remote_ssh_host: None,
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
            message_snapshot: None,
            active_turn: Some(active_turn),
            acp_projection: None,
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
