//! OpenBitFun app-server assembly over the generic `AppServer` role.
//!
//! Request handlers are grouped by product domain under [`handlers`]. This
//! module owns the server lifecycle, handler integration order, transport
//! connection, and event forwarding.

mod event_forwarder;
mod fallback;
mod handlers;
pub mod host_policy;
mod wire;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use agent_client_protocol::{ConnectTo, ConnectionTo, Result};

use crate::agent::OpenBitFunAppRuntime;
use crate::management::AppManagementService;
use crate::role::{AppClient, AppServer};

static NEXT_CONNECTION_ID: AtomicU64 = AtomicU64::new(1);

pub(super) struct ConnectionEventState {
    id: String,
    agent_sequence: AtomicU64,
    permission_sequence: AtomicU64,
    config_sequence: AtomicU64,
    external_source_sequence: AtomicU64,
}

impl ConnectionEventState {
    fn new() -> Self {
        Self {
            id: format!(
                "app-server-{}",
                NEXT_CONNECTION_ID.fetch_add(1, Ordering::Relaxed)
            ),
            agent_sequence: AtomicU64::new(0),
            permission_sequence: AtomicU64::new(0),
            config_sequence: AtomicU64::new(0),
            external_source_sequence: AtomicU64::new(0),
        }
    }

    pub(super) fn cursor(
        &self,
        stream: openbitfun_app_server_protocol::event::EventStream,
    ) -> openbitfun_app_server_protocol::event::EventCursor {
        let sequence = match stream {
            openbitfun_app_server_protocol::event::EventStream::Agent => &self.agent_sequence,
            openbitfun_app_server_protocol::event::EventStream::Permission => {
                &self.permission_sequence
            }
            openbitfun_app_server_protocol::event::EventStream::Config => &self.config_sequence,
            openbitfun_app_server_protocol::event::EventStream::ExternalSource => {
                &self.external_source_sequence
            }
        };
        openbitfun_app_server_protocol::event::EventCursor {
            connection_id: self.id.clone(),
            stream,
            sequence: sequence.load(Ordering::Acquire),
        }
    }

    pub(super) fn next_cursor(
        &self,
        stream: openbitfun_app_server_protocol::event::EventStream,
    ) -> openbitfun_app_server_protocol::event::EventCursor {
        let sequence = match stream {
            openbitfun_app_server_protocol::event::EventStream::Agent => &self.agent_sequence,
            openbitfun_app_server_protocol::event::EventStream::Permission => {
                &self.permission_sequence
            }
            openbitfun_app_server_protocol::event::EventStream::Config => &self.config_sequence,
            openbitfun_app_server_protocol::event::EventStream::ExternalSource => {
                &self.external_source_sequence
            }
        };
        openbitfun_app_server_protocol::event::EventCursor {
            connection_id: self.id.clone(),
            stream,
            sequence: sequence.fetch_add(1, Ordering::AcqRel) + 1,
        }
    }
}

/// OpenBitFun agent kernel server over the generic app-server role.
///
/// Hosts may inject an [`AppServerHostPolicy`] (identity + canonical
/// workspace scope + method allowlist, enforced fail-closed as the first
/// connection layer), the [`AppServerHostLimits`] advertised and enforced by
/// the Host transport, and a disconnect [`host_policy::AppServerDisconnect`] signal that ends
/// event forwarding and returns `serve` when the transport observes EOF or a
/// transport failure. Hosts that inject none of them keep the pre-existing
/// open surface.
#[derive(Clone)]
pub struct OpenBitFunAppServer {
    runtime: Arc<OpenBitFunAppRuntime>,
    management: Option<Arc<AppManagementService>>,
    host_policy: Option<Arc<host_policy::AppServerHostPolicy>>,
    host_limits: host_policy::AppServerHostLimits,
    disconnect: Option<Arc<host_policy::AppServerDisconnect>>,
}

impl OpenBitFunAppServer {
    pub fn new(runtime: OpenBitFunAppRuntime) -> Self {
        Self {
            runtime: Arc::new(runtime),
            management: None,
            host_policy: None,
            host_limits: host_policy::AppServerHostLimits::default(),
            disconnect: None,
        }
    }

    pub fn with_management(mut self, management: Arc<AppManagementService>) -> Self {
        self.management = Some(management);
        self
    }

    /// Inject the Host-owned connection policy (canonical workspace scope,
    /// identity, and the explicit method allowlist). Every request and
    /// notification is checked against it before any domain handler runs.
    pub fn with_host_policy(mut self, policy: host_policy::AppServerHostPolicy) -> Self {
        self.host_policy = Some(Arc::new(policy));
        self
    }

    /// Select the transport limits advertised in `app/initialize`. The Host
    /// is responsible for enforcing them at its transport reader.
    pub fn with_host_limits(mut self, limits: host_policy::AppServerHostLimits) -> Self {
        self.host_limits = limits;
        self
    }

    /// Observe a Host transport disconnect (for example stdin EOF). When the
    /// signal fires, event forwarding stops and `serve` returns so the Host
    /// can run its deterministic disconnect lifecycle.
    pub fn with_disconnect(mut self, disconnect: Arc<host_policy::AppServerDisconnect>) -> Self {
        self.disconnect = Some(disconnect);
        self
    }

    /// Return the shared runtime used by this server.
    pub fn runtime(&self) -> &OpenBitFunAppRuntime {
        &self.runtime
    }

    /// Serve the complete app-server surface on the supplied transport.
    pub async fn serve(self, transport: impl ConnectTo<AppServer> + 'static) -> Result<()> {
        let runtime = self.runtime;
        let management = self.management;
        let host_policy = self.host_policy;
        let host_limits = self.host_limits;
        let disconnect = self.disconnect;
        let event_state = Arc::new(ConnectionEventState::new());

        AppServer
            .builder()
            .name("openbitfun-app-server")
            .with_connection_builder(host_policy::builder(host_policy.clone()))
            .with_connection_builder(handlers::app::builder(
                runtime.clone(),
                event_state.clone(),
                management.clone(),
                host_policy,
                host_limits,
            ))
            .with_connection_builder(handlers::agent::builder(
                runtime.clone(),
                management.clone(),
            ))
            .with_connection_builder(handlers::account::builder(management.clone()))
            .with_connection_builder(handlers::session::builder(runtime.clone()))
            .with_connection_builder(handlers::permission::builder(runtime.clone()))
            .with_connection_builder(handlers::search::builder(runtime.clone()))
            .with_connection_builder(handlers::workspace::builder(runtime.clone()))
            .with_connection_builder(handlers::worktree::builder(management.clone()))
            .with_connection_builder(handlers::model::builder(management.clone()))
            .with_connection_builder(handlers::skill::builder(management.clone()))
            .with_connection_builder(handlers::subagent::builder(management.clone()))
            .with_connection_builder(handlers::mcp::builder(management.clone()))
            .with_connection_builder(handlers::external_source::builder(management.clone()))
            .with_connection_builder(handlers::hook::builder(management.clone()))
            .with_connection_builder(handlers::git::builder())
            .with_connection_builder(handlers::config::builder())
            .with_connection_builder(handlers::i18n::builder())
            .with_connection_builder(fallback::builder())
            .connect_with(transport, async move |cx: ConnectionTo<AppClient>| {
                event_forwarder::run(runtime, management, cx, event_state, disconnect).await
            })
            .await
    }
}
