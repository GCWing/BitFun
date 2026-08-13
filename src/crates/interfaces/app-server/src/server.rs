//! BitFun app-server assembly over the generic `AppServer` role.
//!
//! Request handlers are grouped by product domain under [`handlers`]. This
//! module owns the server lifecycle, handler integration order, transport
//! connection, and event forwarding.

mod authorization;
mod event_forwarder;
mod fallback;
mod handlers;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::{collections::HashSet, sync::RwLock};

use agent_client_protocol::{ConnectTo, ConnectionTo, Result};
use bitfun_product_domains::tool_permissions::PermissionRequest;
use bitfun_runtime_ports::AgentSessionWorkspaceBinding;
use serde::Serialize;

use crate::agent::BitfunAppRuntime;
use crate::host::{AppServerHostPolicy, AppServerOperationKind, AppServerOperationObserver};
use crate::management::AppManagementService;
use crate::role::{AppClient, AppServer};

static NEXT_CONNECTION_ID: AtomicU64 = AtomicU64::new(1);

pub(super) struct ConnectionEventState {
    id: String,
    require_session_subscription: bool,
    local_management_scope: AtomicBool,
    session_subscriptions: RwLock<HashSet<String>>,
    permission_subscriptions: RwLock<HashSet<String>>,
    agent_sequence: AtomicU64,
    permission_sequence: AtomicU64,
    config_sequence: AtomicU64,
    external_source_sequence: AtomicU64,
    host_policy: Option<Arc<dyn AppServerHostPolicy>>,
    operation_observer: Option<Arc<dyn AppServerOperationObserver>>,
}

impl ConnectionEventState {
    fn new(
        require_session_subscription: bool,
        host_policy: Option<Arc<dyn AppServerHostPolicy>>,
        operation_observer: Option<Arc<dyn AppServerOperationObserver>>,
    ) -> Self {
        Self {
            id: format!(
                "app-server-{}",
                NEXT_CONNECTION_ID.fetch_add(1, Ordering::Relaxed)
            ),
            require_session_subscription,
            local_management_scope: AtomicBool::new(true),
            session_subscriptions: RwLock::new(HashSet::new()),
            permission_subscriptions: RwLock::new(HashSet::new()),
            agent_sequence: AtomicU64::new(0),
            permission_sequence: AtomicU64::new(0),
            config_sequence: AtomicU64::new(0),
            external_source_sequence: AtomicU64::new(0),
            host_policy,
            operation_observer,
        }
    }

    pub(super) fn authorize_request<T>(&self, request: &T) -> agent_client_protocol::Result<()>
    where
        T: agent_client_protocol::JsonRpcMessage + Serialize,
    {
        let Some(policy) = &self.host_policy else {
            return Ok(());
        };
        let value = serde_json::to_value(request)
            .map_err(|_| agent_client_protocol::Error::internal_error())?;
        policy
            .authorize_request(request.method(), &value)
            .map_err(host_policy_error)
    }

    pub(super) fn preflight_request(
        &self,
        method: &str,
        request: &serde_json::Value,
    ) -> agent_client_protocol::Result<()> {
        let Some(policy) = &self.host_policy else {
            return Ok(());
        };
        policy
            .authorize_preflight(method, request)
            .map_err(host_policy_error)
    }

    pub(super) fn allows_capability(&self, capability: &str) -> bool {
        self.host_policy
            .as_ref()
            .is_none_or(|policy| policy.allows_capability(capability))
    }

    pub(super) fn allows_method(&self, method: &str) -> bool {
        self.host_policy
            .as_ref()
            .is_none_or(|policy| policy.allows_method(method))
    }

    pub(super) fn allows_external_source_workspace(&self, workspace_path: &str) -> bool {
        self.host_policy
            .as_ref()
            .is_none_or(|policy| policy.allows_external_source_workspace(workspace_path))
    }

    pub(super) fn register_session_binding(
        &self,
        session_id: &str,
        binding: &AgentSessionWorkspaceBinding,
    ) -> agent_client_protocol::Result<()> {
        let Some(policy) = &self.host_policy else {
            return Ok(());
        };
        policy
            .register_session_binding(session_id, binding)
            .map_err(host_policy_error)
    }

    pub(super) fn require_resolved_session_binding(
        &self,
        session_id: &str,
        binding: Option<&AgentSessionWorkspaceBinding>,
    ) -> agent_client_protocol::Result<()> {
        match binding {
            Some(binding) => self.register_session_binding(session_id, binding),
            None if self.host_policy.is_some() => Err(host_policy_error(
                crate::host::AppServerHostPolicyError::invalid_request(
                    "The Session has no authoritative workspace binding for this Host",
                ),
            )),
            None => Ok(()),
        }
    }

    pub(super) fn enforces_host_policy(&self) -> bool {
        self.host_policy.is_some()
    }

    pub(super) async fn authorize_session_request<T>(
        &self,
        runtime: &BitfunAppRuntime,
        request: &T,
        session_ids: &[&str],
    ) -> agent_client_protocol::Result<()>
    where
        T: agent_client_protocol::JsonRpcMessage + Serialize,
    {
        if let Some(policy) = &self.host_policy {
            if !policy.allows_method(request.method()) {
                return Err(host_policy_error(
                    crate::host::AppServerHostPolicyError::unsupported(
                        "shared.method",
                        format!("The Host does not expose {}", request.method()),
                    ),
                ));
            }
            for session_id in session_ids {
                self.register_authoritative_session_binding(runtime, session_id)
                    .await?;
            }
        }
        self.authorize_request(request)
    }

    pub(super) async fn register_authoritative_session_binding(
        &self,
        runtime: &BitfunAppRuntime,
        session_id: &str,
    ) -> agent_client_protocol::Result<()> {
        if self.host_policy.is_none() {
            return Ok(());
        }
        let binding = crate::agent::runtime_call(
            runtime
                .runtime()
                .resolve_session_workspace_binding(
                    bitfun_runtime_ports::AgentSessionWorkspaceRequest {
                        session_id: session_id.to_string(),
                    },
                )
                .await,
        )?
        .ok_or_else(|| {
            host_policy_error(crate::host::AppServerHostPolicyError::invalid_request(
                "The Session is unavailable in this Shared Host workspace scope",
            ))
        })?;
        self.register_session_binding(session_id, &binding)
    }

    pub(super) fn operation_started(
        &self,
        session_id: &str,
        turn_id: &str,
        kind: AppServerOperationKind,
    ) {
        if let Some(observer) = &self.operation_observer {
            observer.operation_started(session_id, turn_id, kind);
        }
    }

    pub(super) fn operation_admitted(
        &self,
        session_id: &str,
        turn_id: &str,
        kind: AppServerOperationKind,
    ) {
        if let Some(observer) = &self.operation_observer {
            observer.operation_admitted(session_id, turn_id, kind);
        }
    }

    pub(super) fn operation_rejected(
        &self,
        session_id: &str,
        turn_id: &str,
        kind: AppServerOperationKind,
    ) {
        if let Some(observer) = &self.operation_observer {
            observer.operation_rejected(session_id, turn_id, kind);
        }
    }

    pub(super) fn subscribe_session(&self, session_id: impl Into<String>) {
        self.session_subscriptions
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(session_id.into());
    }

    pub(super) fn set_management_scope_from_binding(&self, binding: &AgentSessionWorkspaceBinding) {
        self.local_management_scope.store(
            binding.remote_connection_id.is_none() && binding.remote_ssh_host.is_none(),
            Ordering::Release,
        );
    }

    pub(super) fn set_local_management_scope(&self, allowed: bool) {
        self.local_management_scope
            .store(allowed, Ordering::Release);
    }

    pub(super) fn allows_local_management(&self) -> bool {
        self.local_management_scope.load(Ordering::Acquire)
    }

    pub(super) fn unsubscribe_session(&self, session_id: &str) {
        self.session_subscriptions
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(session_id);
    }

    pub(super) fn accepts_session(&self, session_id: &str) -> bool {
        !self.require_session_subscription
            || self
                .session_subscriptions
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .contains(session_id)
    }

    pub(super) fn subscribe_descendant(&self, parent_session_id: &str, session_id: &str) {
        if self.accepts_session(parent_session_id) {
            self.subscribe_session(session_id.to_string());
        }
    }

    pub(super) fn remember_permission(&self, request_id: impl Into<String>) {
        self.permission_subscriptions
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(request_id.into());
    }

    pub(super) fn accepts_permission(&self, request: &PermissionRequest) -> bool {
        self.accepts_session(&request.session_id)
            || request
                .delegation
                .as_ref()
                .is_some_and(|delegation| self.accepts_session(&delegation.parent_session_id))
    }

    pub(super) fn filter_pending_permissions(
        &self,
        requests: Vec<PermissionRequest>,
    ) -> Vec<PermissionRequest> {
        requests
            .into_iter()
            .filter(|request| {
                let accepted = self.accepts_permission(request);
                if accepted {
                    self.remember_permission(request.request_id.clone());
                }
                accepted
            })
            .collect()
    }

    pub(super) fn filter_session_permissions(
        &self,
        requests: Vec<PermissionRequest>,
        session_id: &str,
    ) -> Vec<PermissionRequest> {
        requests
            .into_iter()
            .filter(|request| {
                let accepted = request.session_id == session_id
                    || request
                        .delegation
                        .as_ref()
                        .is_some_and(|delegation| delegation.parent_session_id == session_id);
                if accepted {
                    self.remember_permission(request.request_id.clone());
                }
                accepted
            })
            .collect()
    }

    pub(super) fn can_respond_permission(&self, request_id: &str) -> bool {
        !self.require_session_subscription
            || self
                .permission_subscriptions
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .contains(request_id)
    }

    pub(super) fn take_permission(&self, request_id: &str) -> bool {
        let removed = self
            .permission_subscriptions
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(request_id);
        !self.require_session_subscription || removed
    }

    pub(super) fn cursor(
        &self,
        stream: bitfun_app_server_protocol::event::EventStream,
    ) -> bitfun_app_server_protocol::event::EventCursor {
        let sequence = match stream {
            bitfun_app_server_protocol::event::EventStream::Agent => &self.agent_sequence,
            bitfun_app_server_protocol::event::EventStream::Permission => &self.permission_sequence,
            bitfun_app_server_protocol::event::EventStream::Config => &self.config_sequence,
            bitfun_app_server_protocol::event::EventStream::ExternalSource => {
                &self.external_source_sequence
            }
        };
        bitfun_app_server_protocol::event::EventCursor {
            connection_id: self.id.clone(),
            stream,
            sequence: sequence.load(Ordering::Acquire),
        }
    }

    pub(super) fn next_cursor(
        &self,
        stream: bitfun_app_server_protocol::event::EventStream,
    ) -> bitfun_app_server_protocol::event::EventCursor {
        let sequence = match stream {
            bitfun_app_server_protocol::event::EventStream::Agent => &self.agent_sequence,
            bitfun_app_server_protocol::event::EventStream::Permission => &self.permission_sequence,
            bitfun_app_server_protocol::event::EventStream::Config => &self.config_sequence,
            bitfun_app_server_protocol::event::EventStream::ExternalSource => {
                &self.external_source_sequence
            }
        };
        bitfun_app_server_protocol::event::EventCursor {
            connection_id: self.id.clone(),
            stream,
            sequence: sequence.fetch_add(1, Ordering::AcqRel) + 1,
        }
    }
}

/// BitFun agent kernel server over the generic app-server role.
#[derive(Clone)]
pub struct BitfunAppServer {
    runtime: Arc<BitfunAppRuntime>,
    management: Option<Arc<AppManagementService>>,
    host_policy: Option<Arc<dyn AppServerHostPolicy>>,
    operation_observer: Option<Arc<dyn AppServerOperationObserver>>,
    require_session_subscription: bool,
    transport_limits: bitfun_app_server_protocol::app::TransportLimits,
}

impl BitfunAppServer {
    pub fn new(runtime: BitfunAppRuntime) -> Self {
        Self {
            runtime: Arc::new(runtime),
            management: None,
            host_policy: None,
            operation_observer: None,
            require_session_subscription: false,
            transport_limits: bitfun_app_server_protocol::app::TransportLimits {
                max_request_bytes: 16 * 1024 * 1024,
                max_response_bytes: 16 * 1024 * 1024,
                max_frame_bytes: 16 * 1024 * 1024,
                event_buffer_capacity: 1024,
            },
        }
    }

    pub fn with_management(mut self, management: Arc<AppManagementService>) -> Self {
        self.management = Some(management);
        self
    }

    pub fn with_host_policy(mut self, policy: Arc<dyn AppServerHostPolicy>) -> Self {
        self.host_policy = Some(policy);
        self
    }

    pub fn with_operation_observer(
        mut self,
        observer: Arc<dyn AppServerOperationObserver>,
    ) -> Self {
        self.operation_observer = Some(observer);
        self
    }

    /// Restrict Session and Permission notifications to Sessions explicitly
    /// subscribed by this connection. Shared multi-client hosts enable this;
    /// private Embedded hosts retain the existing all-events behavior.
    pub fn require_session_subscriptions(mut self, required: bool) -> Self {
        self.require_session_subscription = required;
        self
    }

    /// Set the limits owned by the concrete Host transport.
    pub fn with_transport_limits(
        mut self,
        limits: bitfun_app_server_protocol::app::TransportLimits,
    ) -> Self {
        self.transport_limits = limits;
        self
    }

    /// Return the shared runtime used by this server.
    pub fn runtime(&self) -> &BitfunAppRuntime {
        &self.runtime
    }

    /// Serve the complete app-server surface on the supplied transport.
    pub async fn serve(self, transport: impl ConnectTo<AppServer> + 'static) -> Result<()> {
        let runtime = self.runtime;
        let management = self.management;
        let transport_limits = self.transport_limits;
        let event_state = Arc::new(ConnectionEventState::new(
            self.require_session_subscription,
            self.host_policy,
            self.operation_observer,
        ));

        AppServer
            .builder()
            .name("bitfun-app-server")
            .with_connection_builder(authorization::builder(event_state.clone()))
            .with_connection_builder(handlers::app::builder(
                runtime.clone(),
                event_state.clone(),
                management.clone(),
                transport_limits,
            ))
            .with_connection_builder(handlers::agent::builder(
                runtime.clone(),
                management.clone(),
                event_state.clone(),
            ))
            .with_connection_builder(handlers::account::builder(
                management.clone(),
                event_state.clone(),
            ))
            .with_connection_builder(handlers::session::builder(
                runtime.clone(),
                event_state.clone(),
            ))
            .with_connection_builder(handlers::permission::builder(
                runtime.clone(),
                event_state.clone(),
            ))
            .with_connection_builder(handlers::workspace::builder(
                runtime.clone(),
                event_state.clone(),
            ))
            .with_connection_builder(handlers::worktree::builder(
                runtime.clone(),
                management.clone(),
                event_state.clone(),
            ))
            .with_connection_builder(handlers::model::builder(
                management.clone(),
                event_state.clone(),
            ))
            .with_connection_builder(handlers::skill::builder(
                management.clone(),
                event_state.clone(),
            ))
            .with_connection_builder(handlers::subagent::builder(
                management.clone(),
                event_state.clone(),
            ))
            .with_connection_builder(handlers::mcp::builder(
                management.clone(),
                event_state.clone(),
            ))
            .with_connection_builder(handlers::external_source::builder(
                management.clone(),
                event_state.clone(),
            ))
            .with_connection_builder(handlers::hook::builder(
                management.clone(),
                event_state.clone(),
            ))
            .with_connection_builder(handlers::git::builder())
            .with_connection_builder(handlers::config::builder())
            .with_connection_builder(handlers::i18n::builder())
            .with_connection_builder(fallback::builder())
            .connect_with(transport, async move |cx: ConnectionTo<AppClient>| {
                event_forwarder::run(runtime, management, cx, event_state).await
            })
            .await
    }
}

fn host_policy_error(error: crate::host::AppServerHostPolicyError) -> agent_client_protocol::Error {
    use bitfun_app_server_protocol::error::AppServerErrorData;

    agent_client_protocol::Error::new(error.kind.json_rpc_code() as i32, error.message).data(
        serde_json::to_value(AppServerErrorData {
            kind: error.kind,
            retryable: false,
            outcome_unknown: false,
            capability: error.capability,
            request_id: None,
        })
        .unwrap_or(serde_json::Value::Null),
    )
}
