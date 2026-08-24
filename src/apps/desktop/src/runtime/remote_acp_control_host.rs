//! Desktop-owned ACP remote-control host for Remote Connect.
//!
//! Translates `RemoteCommand::Acp*` into `AcpClientService` calls and publishes
//! observation events through the existing `AcpEventPublisher`. The phone never
//! receives ACP process handles or native tool ids.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bitfun_acp::client::{
    is_acp_permission_id, AcpClientService, AcpSessionConfigValue,
    SetAcpSessionConfigOptionRequest, SubmitAcpPermissionResponseRequest,
};
use bitfun_core::agentic::coordination::get_global_coordinator;
use bitfun_core::service::remote_connect::remote_server::get_or_init_global_dispatcher;
use bitfun_core::service::remote_connect::resolve_remote_session_workspace_scope;
use bitfun_core::service::session::SESSION_PROVIDER_ACP;
use bitfun_core_types::SESSION_PROVIDER_METADATA_KEY;
use bitfun_services_integrations::remote_connect::{
    acp_permission_mailbox, acp_permission_now_ms, RemoteAcpCancelOutcome, RemoteAcpCancelRequest,
    RemoteAcpCommandsOutcome, RemoteAcpControlError, RemoteAcpControlRuntimeHost,
    RemoteAcpGetCommandsRequest, RemoteAcpGetOptionsRequest, RemoteAcpGetPlanRequest,
    RemoteAcpOptionsOutcome, RemoteAcpPermissionRespondOutcome, RemoteAcpPermissionRespondRequest,
    RemoteAcpPlanOutcome, RemoteAcpSendOutcome, RemoteAcpSendRequest, RemoteAcpSetOptionRequest,
    RemoteRetryClassification, UNSUPPORTED_REMOTE_CAPABILITY,
};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

use super::acp_projection_writer::AcpSessionScopeRegistrationError;
use super::acp_request_idempotency::{
    claim_idempotent_value, clear_session_idempotency_keys, request_idempotency_key,
    IdempotentClaim,
};
use super::{
    acp_dialog_turn_started_event, AcpDurableProjectionWriter, AcpEventPublisher, AcpTurnMapper,
    DesktopSessionApplication,
};

const ACP_CLIENT_ID_METADATA_KEY: &str = "acpClientId";

fn scope_registration_retry_classification(
    error: &AcpSessionScopeRegistrationError,
) -> RemoteRetryClassification {
    match error {
        AcpSessionScopeRegistrationError::Conflict { .. } => RemoteRetryClassification::Terminal,
        AcpSessionScopeRegistrationError::Recovery(_) => RemoteRetryClassification::Retryable,
    }
}

pub(crate) struct DesktopRemoteAcpControlHost {
    service: Arc<AcpClientService>,
    publisher: Arc<AcpEventPublisher>,
    projection_writer: Arc<AcpDurableProjectionWriter<DesktopSessionApplication>>,
    /// Idempotent request_id → turn_id for AcpSendMessage retries.
    send_by_request: Mutex<HashMap<String, String>>,
    /// Idempotent request_id → cancel outcome for AcpCancelTurn retries.
    cancel_by_request: Mutex<HashMap<String, RemoteAcpCancelOutcome>>,
    cancel_request_lock: AsyncMutex<()>,
    /// Idempotent request_id → options snapshot for AcpSetOption retries.
    set_option_by_request: Mutex<HashMap<String, RemoteAcpOptionsOutcome>>,
    set_option_request_lock: AsyncMutex<()>,
    /// Idempotent request_id → permission outcome for AcpPermissionRespond retries.
    permission_by_request: Mutex<HashMap<String, RemoteAcpPermissionRespondOutcome>>,
    permission_request_lock: AsyncMutex<()>,
}

impl DesktopRemoteAcpControlHost {
    pub(crate) fn new(
        service: Arc<AcpClientService>,
        publisher: Arc<AcpEventPublisher>,
        projection_writer: Arc<AcpDurableProjectionWriter<DesktopSessionApplication>>,
    ) -> Self {
        Self {
            service,
            publisher,
            projection_writer,
            send_by_request: Mutex::new(HashMap::new()),
            cancel_by_request: Mutex::new(HashMap::new()),
            cancel_request_lock: AsyncMutex::new(()),
            set_option_by_request: Mutex::new(HashMap::new()),
            set_option_request_lock: AsyncMutex::new(()),
            permission_by_request: Mutex::new(HashMap::new()),
            permission_request_lock: AsyncMutex::new(()),
        }
    }

    pub(crate) fn clear_session_idempotency(&self, session_id: &str) {
        clear_session_idempotency_keys(
            &mut self.send_by_request.lock().expect("ACP send idempotency"),
            session_id,
        );
        clear_session_idempotency_keys(
            &mut self
                .cancel_by_request
                .lock()
                .expect("ACP cancel idempotency"),
            session_id,
        );
        clear_session_idempotency_keys(
            &mut self
                .set_option_by_request
                .lock()
                .expect("ACP set_option idempotency"),
            session_id,
        );
        clear_session_idempotency_keys(
            &mut self
                .permission_by_request
                .lock()
                .expect("ACP permission idempotency"),
            session_id,
        );
    }

    async fn resolve_session_context(
        &self,
        session_id: &str,
    ) -> Result<ResolvedAcpSession, RemoteAcpControlError> {
        let workspace_scope = resolve_remote_session_workspace_scope(session_id)
            .await
            .ok_or_else(|| {
                RemoteAcpControlError::terminal(
                    session_id,
                    None,
                    "acp_session_not_found",
                    format!("ACP session workspace scope was not found: {session_id}"),
                )
            })?;
        let coordinator = get_global_coordinator().ok_or_else(|| {
            RemoteAcpControlError::terminal(
                session_id,
                None,
                "acp_runtime_unavailable",
                "Conversation coordinator is not available",
            )
        })?;
        let metadata = coordinator
            .get_session_manager()
            .load_session_metadata(&workspace_scope.session_storage_path, session_id)
            .await
            .map_err(|error| {
                RemoteAcpControlError::terminal(
                    session_id,
                    None,
                    "acp_session_metadata_error",
                    error.to_string(),
                )
            })?
            .ok_or_else(|| {
                RemoteAcpControlError::terminal(
                    session_id,
                    None,
                    "acp_session_not_found",
                    format!("ACP session metadata was not found: {session_id}"),
                )
            })?;

        let provider = metadata
            .custom_metadata
            .as_ref()
            .and_then(|custom| custom.get(SESSION_PROVIDER_METADATA_KEY))
            .and_then(serde_json::Value::as_str);
        if provider != Some(SESSION_PROVIDER_ACP) {
            return Err(RemoteAcpControlError::terminal(
                session_id,
                None,
                UNSUPPORTED_REMOTE_CAPABILITY,
                format!("Session is not ACP-controlled: {session_id}"),
            ));
        }

        let client_id = metadata
            .custom_metadata
            .as_ref()
            .and_then(|custom| custom.get(ACP_CLIENT_ID_METADATA_KEY))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .or_else(|| metadata.agent_type.strip_prefix("acp:").map(str::to_string))
            .unwrap_or_else(|| metadata.agent_type.clone());

        Ok(ResolvedAcpSession {
            client_id,
            workspace_path: workspace_scope.workspace_path,
            session_storage_path: workspace_scope.session_storage_path,
            remote_connection_id: workspace_scope.remote_connection_id,
            remote_ssh_host: workspace_scope.remote_ssh_host,
        })
    }
}

struct ResolvedAcpSession {
    client_id: String,
    workspace_path: String,
    session_storage_path: PathBuf,
    remote_connection_id: Option<String>,
    remote_ssh_host: Option<String>,
}

#[async_trait]
impl RemoteAcpControlRuntimeHost for DesktopRemoteAcpControlHost {
    async fn send_message(
        &self,
        request: RemoteAcpSendRequest,
    ) -> Result<RemoteAcpSendOutcome, RemoteAcpControlError> {
        let resolved = self.resolve_session_context(&request.session_id).await?;
        self.projection_writer
            .ensure_session_scope(
                &request.session_id,
                &resolved.workspace_path,
                resolved.remote_connection_id.as_deref(),
                resolved.remote_ssh_host.as_deref(),
            )
            .await
            .map_err(|error| {
                let retry = scope_registration_retry_classification(&error);
                RemoteAcpControlError {
                    session_id: request.session_id.clone(),
                    request_id: request.request_id.clone(),
                    code: "acp_projection_scope_failed".to_string(),
                    message: error.to_string(),
                    retry,
                }
            })?;
        // Subscribe before publishing the first ACP lifecycle event. Poll-created
        // trackers are too late for a send that starts before the phone's first
        // poll, and ACP sessions remain external projections rather than native
        // SessionManager-owned sessions.
        get_or_init_global_dispatcher().ensure_tracker(&request.session_id);
        let candidate_turn_id = format!("acp-remote-{}", Uuid::new_v4());
        let turn_id = if let Some(request_id) = request.request_id.as_deref() {
            let key = request_idempotency_key(&request.session_id, request_id);
            let claim = {
                let mut map = self.send_by_request.lock().expect("ACP send idempotency");
                claim_idempotent_value(&mut map, key, candidate_turn_id.clone())
            };
            match claim {
                IdempotentClaim::Existing(existing) => {
                    return Ok(RemoteAcpSendOutcome {
                        session_id: request.session_id,
                        turn_id: existing,
                        request_id: request.request_id,
                    });
                }
                IdempotentClaim::Claimed(claimed) => claimed,
            }
        } else {
            candidate_turn_id
        };

        let service = self.service.clone();
        let publisher = self.publisher.clone();
        let session_id = request.session_id.clone();
        let content = request.content.clone();
        let client_id = resolved.client_id.clone();
        let workspace_path = resolved.workspace_path.clone();
        let session_storage_path = resolved.session_storage_path.clone();
        let remote_connection_id = resolved.remote_connection_id.clone();

        publisher
            .publish_turn_started(acp_dialog_turn_started_event(
                session_id.clone(),
                turn_id.clone(),
                content.clone(),
                None,
            ))
            .map_err(|error| {
                // Claim happened before side effects; roll it back so a retry can
                // re-claim and publish instead of returning a never-started turn.
                if let Some(request_id) = request.request_id.as_deref() {
                    let key = request_idempotency_key(&request.session_id, request_id);
                    let mut map = self.send_by_request.lock().expect("ACP send idempotency");
                    if map.get(&key).is_some_and(|owned| owned == &turn_id) {
                        map.remove(&key);
                    }
                }
                RemoteAcpControlError::terminal(
                    request.session_id.clone(),
                    request.request_id.clone(),
                    "acp_publish_failed",
                    error.to_string(),
                )
            })?;

        let turn_id_for_task = turn_id.clone();
        let request_id = request.request_id.clone();
        tokio::spawn(async move {
            let mut mapper = AcpTurnMapper::new(
                session_id.clone(),
                turn_id_for_task.clone(),
                client_id.clone(),
            );
            let result = service
                .prompt_agent_stream(
                    &client_id,
                    content,
                    Some(workspace_path),
                    remote_connection_id,
                    session_id.clone(),
                    Some(session_storage_path),
                    None,
                    |event| {
                        let jobs = mapper.map(event)?;
                        publisher
                            .publish_jobs(jobs)
                            .map_err(bitfun_core::util::errors::BitFunError::service)
                    },
                )
                .await;
            if let Err(error) = result {
                let _ = publisher.publish_jobs(mapper.fail(error.to_string()));
                log::error!(
                    "Remote ACP send failed: session_id={session_id}, request_id={request_id:?}, error={error}"
                );
            }
        });

        Ok(RemoteAcpSendOutcome {
            session_id: request.session_id,
            turn_id,
            request_id: request.request_id,
        })
    }

    async fn cancel_turn(
        &self,
        request: RemoteAcpCancelRequest,
    ) -> Result<RemoteAcpCancelOutcome, RemoteAcpControlError> {
        // Serialize claim-through-side-effect so concurrent retries cannot both
        // enter the ACP service before the first outcome is cached.
        let _request_guard = self.cancel_request_lock.lock().await;
        if let Some(request_id) = request.request_id.as_deref() {
            let key = request_idempotency_key(&request.session_id, request_id);
            if let Some(outcome) = self
                .cancel_by_request
                .lock()
                .expect("ACP cancel idempotency")
                .get(&key)
                .cloned()
            {
                return Ok(outcome);
            }
        }

        if let Some(requested_turn_id) = request.turn_id.as_deref() {
            let tracker = get_or_init_global_dispatcher().ensure_tracker(&request.session_id);
            if let Some(active_turn) = tracker.snapshot_active_turn() {
                if active_turn.turn_id != requested_turn_id {
                    return Err(RemoteAcpControlError {
                        session_id: request.session_id.clone(),
                        request_id: request.request_id.clone(),
                        code: "acp_turn_stale".to_string(),
                        message: format!(
                            "ACP active turn changed before cancellation: requested={}, active={}",
                            requested_turn_id, active_turn.turn_id
                        ),
                        retry: RemoteRetryClassification::Stale,
                    });
                }
            }
        }

        let cancelled = self
            .service
            .cancel_bitfun_session(&request.session_id)
            .await
            .map_err(|error| RemoteAcpControlError {
                session_id: request.session_id.clone(),
                request_id: request.request_id.clone(),
                code: "acp_cancel_failed".to_string(),
                message: error.to_string(),
                retry: RemoteRetryClassification::Retryable,
            })?;
        if !cancelled {
            return Err(RemoteAcpControlError {
                session_id: request.session_id.clone(),
                request_id: request.request_id.clone(),
                code: "acp_turn_stale".to_string(),
                message: format!(
                    "No active ACP turn to cancel for session {}",
                    request.session_id
                ),
                retry: RemoteRetryClassification::Stale,
            });
        }
        let outcome = RemoteAcpCancelOutcome {
            session_id: request.session_id.clone(),
            turn_id: request.turn_id,
            request_id: request.request_id.clone(),
        };
        if let Some(request_id) = request.request_id.as_deref() {
            let key = request_idempotency_key(&outcome.session_id, request_id);
            let claim = {
                let mut map = self
                    .cancel_by_request
                    .lock()
                    .expect("ACP cancel idempotency");
                claim_idempotent_value(&mut map, key, outcome.clone())
            };
            if let IdempotentClaim::Existing(existing) = claim {
                return Ok(existing);
            }
        }
        Ok(outcome)
    }

    async fn get_options(
        &self,
        request: RemoteAcpGetOptionsRequest,
    ) -> Result<RemoteAcpOptionsOutcome, RemoteAcpControlError> {
        let resolved = self.resolve_session_context(&request.session_id).await?;
        let options = self
            .service
            .get_session_options(
                &resolved.client_id,
                Some(resolved.workspace_path),
                resolved.remote_connection_id,
                Some(resolved.session_storage_path),
                request.session_id.clone(),
            )
            .await
            .map_err(|error| {
                RemoteAcpControlError::terminal(
                    request.session_id.clone(),
                    request.request_id.clone(),
                    "acp_options_failed",
                    error.to_string(),
                )
            })?;
        Ok(RemoteAcpOptionsOutcome {
            session_id: request.session_id,
            request_id: request.request_id,
            options: serde_json::to_value(options).unwrap_or(serde_json::Value::Null),
        })
    }

    async fn set_option(
        &self,
        request: RemoteAcpSetOptionRequest,
    ) -> Result<RemoteAcpOptionsOutcome, RemoteAcpControlError> {
        // Keep the request-id lookup and ACP mutation in one async critical
        // section so a concurrent retry observes the first cached result.
        let _request_guard = self.set_option_request_lock.lock().await;
        if let Some(request_id) = request.request_id.as_deref() {
            let key = request_idempotency_key(&request.session_id, request_id);
            if let Some(outcome) = self
                .set_option_by_request
                .lock()
                .expect("ACP set_option idempotency")
                .get(&key)
                .cloned()
            {
                return Ok(outcome);
            }
        }

        let resolved = self.resolve_session_context(&request.session_id).await?;
        let value = parse_acp_config_value(&request.value).map_err(|message| {
            RemoteAcpControlError::terminal(
                request.session_id.clone(),
                request.request_id.clone(),
                "acp_invalid_option_value",
                message,
            )
        })?;
        let options = self
            .service
            .set_session_config_option(
                SetAcpSessionConfigOptionRequest {
                    client_id: resolved.client_id,
                    session_id: request.session_id.clone(),
                    workspace_path: Some(resolved.workspace_path),
                    remote_connection_id: resolved.remote_connection_id,
                    remote_ssh_host: resolved.remote_ssh_host,
                    config_id: request.config_id,
                    value,
                },
                Some(resolved.session_storage_path),
            )
            .await
            .map_err(|error| {
                RemoteAcpControlError::terminal(
                    request.session_id.clone(),
                    request.request_id.clone(),
                    "acp_set_option_failed",
                    error.to_string(),
                )
            })?;
        let outcome = RemoteAcpOptionsOutcome {
            session_id: request.session_id,
            request_id: request.request_id.clone(),
            options: serde_json::to_value(options).unwrap_or(serde_json::Value::Null),
        };
        if let Some(request_id) = request.request_id.as_deref() {
            let key = request_idempotency_key(&outcome.session_id, request_id);
            let claim = {
                let mut map = self
                    .set_option_by_request
                    .lock()
                    .expect("ACP set_option idempotency");
                claim_idempotent_value(&mut map, key, outcome.clone())
            };
            if let IdempotentClaim::Existing(existing) = claim {
                return Ok(existing);
            }
        }
        Ok(outcome)
    }

    async fn get_commands(
        &self,
        request: RemoteAcpGetCommandsRequest,
    ) -> Result<RemoteAcpCommandsOutcome, RemoteAcpControlError> {
        let resolved = self.resolve_session_context(&request.session_id).await?;
        let (commands, version) = self
            .service
            .get_session_commands(
                &resolved.client_id,
                Some(resolved.workspace_path),
                resolved.remote_connection_id,
                Some(resolved.session_storage_path),
                request.session_id.clone(),
            )
            .await
            .map_err(|error| {
                RemoteAcpControlError::terminal(
                    request.session_id.clone(),
                    request.request_id.clone(),
                    "acp_commands_failed",
                    error.to_string(),
                )
            })?;
        Ok(RemoteAcpCommandsOutcome {
            session_id: request.session_id,
            request_id: request.request_id,
            commands: serde_json::to_value(commands).unwrap_or_else(|_| serde_json::json!([])),
            version,
        })
    }

    async fn get_plan(
        &self,
        request: RemoteAcpGetPlanRequest,
    ) -> Result<RemoteAcpPlanOutcome, RemoteAcpControlError> {
        let resolved = self.resolve_session_context(&request.session_id).await?;
        let (entries, version) = self
            .service
            .get_session_plan(
                &resolved.client_id,
                Some(resolved.workspace_path),
                resolved.remote_connection_id,
                Some(resolved.session_storage_path),
                request.session_id.clone(),
            )
            .await
            .map_err(|error| {
                RemoteAcpControlError::terminal(
                    request.session_id.clone(),
                    request.request_id.clone(),
                    "acp_plan_failed",
                    error.to_string(),
                )
            })?;
        Ok(RemoteAcpPlanOutcome {
            session_id: request.session_id,
            request_id: request.request_id,
            entries: serde_json::to_value(entries).unwrap_or_else(|_| serde_json::json!([])),
            version,
        })
    }

    async fn permission_respond(
        &self,
        request: RemoteAcpPermissionRespondRequest,
    ) -> Result<RemoteAcpPermissionRespondOutcome, RemoteAcpControlError> {
        // The permission may disappear from the mailbox as soon as the first
        // response resolves. Serialize through caching so a concurrent retry
        // returns that first outcome instead of being misclassified as stale.
        let _request_guard = self.permission_request_lock.lock().await;
        if let Some(request_id) = request.request_id.as_deref() {
            let key = request_idempotency_key(&request.session_id, request_id);
            if let Some(outcome) = self
                .permission_by_request
                .lock()
                .expect("ACP permission idempotency")
                .get(&key)
                .cloned()
            {
                return Ok(outcome);
            }
        }

        let _ = self.resolve_session_context(&request.session_id).await?;
        let mailbox_entry = acp_permission_mailbox()
            .and_then(|mailbox| mailbox.get(&request.permission_id))
            .ok_or_else(|| RemoteAcpControlError {
                session_id: request.session_id.clone(),
                request_id: request.request_id.clone(),
                code: "acp_permission_stale".to_string(),
                message: format!(
                    "ACP permission already resolved or expired: {}",
                    request.permission_id
                ),
                retry: RemoteRetryClassification::Stale,
            })?;
        if mailbox_entry.session_id != request.session_id {
            return Err(RemoteAcpControlError::terminal(
                request.session_id.clone(),
                request.request_id.clone(),
                "acp_permission_session_mismatch",
                format!(
                    "ACP permission belongs to another session: permission_id={}",
                    request.permission_id
                ),
            ));
        }
        if mailbox_entry.expires_at_ms > 0 && mailbox_entry.expires_at_ms <= acp_permission_now_ms()
        {
            return Err(RemoteAcpControlError {
                session_id: request.session_id.clone(),
                request_id: request.request_id.clone(),
                code: "acp_permission_stale".to_string(),
                message: format!("ACP permission expired: {}", request.permission_id),
                retry: RemoteRetryClassification::Stale,
            });
        }
        if !permission_options_contain(&mailbox_entry.options, &request.option_id) {
            return Err(RemoteAcpControlError::terminal(
                request.session_id.clone(),
                request.request_id.clone(),
                "acp_invalid_permission_option",
                format!(
                    "ACP permission option is not pending: permission_id={}, option_id={}",
                    request.permission_id, request.option_id
                ),
            ));
        }
        if !self.service.has_pending_permission(&request.permission_id) {
            return Err(RemoteAcpControlError {
                session_id: request.session_id.clone(),
                request_id: request.request_id.clone(),
                code: "acp_permission_stale".to_string(),
                message: format!(
                    "ACP permission already resolved or expired: {}",
                    request.permission_id
                ),
                retry: RemoteRetryClassification::Stale,
            });
        }
        let response = self
            .service
            .submit_permission_response(SubmitAcpPermissionResponseRequest {
                permission_id: request.permission_id.clone(),
                approve: true,
                option_id: Some(request.option_id),
            })
            .await
            .map_err(|error| {
                RemoteAcpControlError::terminal(
                    request.session_id.clone(),
                    request.request_id.clone(),
                    "acp_permission_respond_failed",
                    error.to_string(),
                )
            })?;
        let outcome = RemoteAcpPermissionRespondOutcome {
            session_id: request.session_id,
            permission_id: response.permission_id,
            request_id: request.request_id.clone(),
            resolved: response.resolved,
        };
        if let Some(request_id) = request.request_id.as_deref() {
            let key = request_idempotency_key(&outcome.session_id, request_id);
            self.permission_by_request
                .lock()
                .expect("ACP permission idempotency")
                .insert(key, outcome.clone());
        }
        Ok(outcome)
    }

    async fn is_acp_session(&self, session_id: &str) -> bool {
        self.resolve_session_context(session_id).await.is_ok()
    }

    async fn is_acp_permission_id(&self, tool_id: &str) -> bool {
        is_acp_permission_id(tool_id) || self.service.has_pending_permission(tool_id)
    }

    fn clear_session_idempotency(&self, session_id: &str) {
        DesktopRemoteAcpControlHost::clear_session_idempotency(self, session_id);
    }
}

fn permission_options_contain(options: &serde_json::Value, option_id: &str) -> bool {
    options.as_array().is_some_and(|entries| {
        entries.iter().any(|entry| {
            entry
                .get("optionId")
                .or_else(|| entry.get("option_id"))
                .or_else(|| entry.get("id"))
                .and_then(serde_json::Value::as_str)
                == Some(option_id)
        })
    })
}

fn parse_acp_config_value(value: &serde_json::Value) -> Result<AcpSessionConfigValue, String> {
    if let Some(object) = value.as_object() {
        match object.get("type").and_then(|v| v.as_str()) {
            Some("select") => {
                let select = object
                    .get("value")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "select option requires string value".to_string())?;
                return Ok(AcpSessionConfigValue::Select {
                    value: select.to_string(),
                });
            }
            Some("boolean") => {
                let boolean = object
                    .get("value")
                    .and_then(|v| v.as_bool())
                    .ok_or_else(|| "boolean option requires bool value".to_string())?;
                return Ok(AcpSessionConfigValue::Boolean { value: boolean });
            }
            _ => {}
        }
        if let Some(select) = object.get("value").and_then(|v| v.as_str()) {
            return Ok(AcpSessionConfigValue::Select {
                value: select.to_string(),
            });
        }
        if let Some(boolean) = object.get("value").and_then(|v| v.as_bool()) {
            return Ok(AcpSessionConfigValue::Boolean { value: boolean });
        }
    }
    if let Some(select) = value.as_str() {
        return Ok(AcpSessionConfigValue::Select {
            value: select.to_string(),
        });
    }
    if let Some(boolean) = value.as_bool() {
        return Ok(AcpSessionConfigValue::Boolean { value: boolean });
    }
    Err(format!("Unsupported ACP option value: {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn method_body<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
        source
            .split(start)
            .nth(1)
            .and_then(|source| source.split(end).next())
            .expect("reviewed remote ACP host method")
    }

    #[test]
    fn scope_registration_errors_have_stable_retry_semantics() {
        assert_eq!(
            scope_registration_retry_classification(&AcpSessionScopeRegistrationError::Conflict {
                session_id: "acp-1".to_string(),
            }),
            RemoteRetryClassification::Terminal
        );
        assert_eq!(
            scope_registration_retry_classification(&AcpSessionScopeRegistrationError::Recovery(
                "load failed".to_string()
            )),
            RemoteRetryClassification::Retryable
        );
    }

    #[test]
    fn send_registers_projection_scope_before_claiming_or_publishing() {
        let source = include_str!("remote_acp_control_host.rs");
        let send = method_body(source, "async fn send_message", "async fn cancel_turn");
        let ensure_scope = send
            .find(".ensure_session_scope(")
            .expect("projection scope registration");
        let claim = send
            .find("claim_idempotent_value")
            .expect("request-id claim");
        let publish = send
            .find(".publish_turn_started(")
            .expect("turn-start publication");

        assert!(ensure_scope < claim);
        assert!(claim < publish);
    }

    #[test]
    fn session_aware_acp_calls_preserve_resolved_remote_scope() {
        let source = include_str!("remote_acp_control_host.rs");
        let send = method_body(source, "async fn send_message", "async fn cancel_turn");
        assert!(send.contains("Some(workspace_path)"));
        assert!(send.contains("remote_connection_id"));
        assert!(send.contains("Some(session_storage_path)"));

        for (start, end) in [
            ("async fn get_options", "async fn set_option"),
            ("async fn get_commands", "async fn get_plan"),
            ("async fn get_plan", "async fn permission_respond"),
        ] {
            let body = method_body(source, start, end);
            assert!(body.contains("Some(resolved.workspace_path)"));
            assert!(body.contains("resolved.remote_connection_id"));
            assert!(body.contains("Some(resolved.session_storage_path)"));
        }

        let set_option = method_body(source, "async fn set_option", "async fn get_commands");
        assert!(set_option.contains("workspace_path: Some(resolved.workspace_path)"));
        assert!(set_option.contains("remote_connection_id: resolved.remote_connection_id"));
        assert!(set_option.contains("remote_ssh_host: resolved.remote_ssh_host"));
        assert!(set_option.contains("Some(resolved.session_storage_path)"));
    }

    #[test]
    fn mutating_remote_commands_serialize_lookup_through_cached_outcome() {
        let source = include_str!("remote_acp_control_host.rs");
        for (start, end, lock, side_effect) in [
            (
                "async fn cancel_turn",
                "async fn get_options",
                "cancel_request_lock.lock().await",
                ".cancel_bitfun_session(",
            ),
            (
                "async fn set_option",
                "async fn get_commands",
                "set_option_request_lock.lock().await",
                ".set_session_config_option(",
            ),
            (
                "async fn permission_respond",
                "async fn is_acp_session",
                "permission_request_lock.lock().await",
                ".submit_permission_response(",
            ),
        ] {
            let body = method_body(source, start, end);
            let lock_index = body.find(lock).expect("operation lock");
            let cache_index = body
                .find("request_idempotency_key")
                .expect("request-id cache lookup");
            let side_effect_index = body.find(side_effect).expect("ACP side effect");
            assert!(lock_index < cache_index);
            assert!(cache_index < side_effect_index);
        }
    }

    #[test]
    fn permission_option_membership_accepts_protocol_aliases_only() {
        let options = serde_json::json!([
            { "optionId": "allow-once" },
            { "option_id": "reject-once" },
            { "id": "allow-always" }
        ]);
        assert!(permission_options_contain(&options, "allow-once"));
        assert!(permission_options_contain(&options, "reject-once"));
        assert!(permission_options_contain(&options, "allow-always"));
        assert!(!permission_options_contain(&options, "native-tool-1"));
        assert!(!permission_options_contain(
            &serde_json::json!({ "optionId": "allow-once" }),
            "allow-once"
        ));
    }
}
