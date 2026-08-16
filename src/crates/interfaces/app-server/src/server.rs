//! BitFun app-server assembly over the generic `AppServer` role.
//!
//! Request handlers are grouped by product domain under [`handlers`]. This
//! module owns the server lifecycle, handler integration order, transport
//! connection, and event forwarding.

mod event_forwarder;
mod fallback;
mod handlers;
mod wire;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use agent_client_protocol::{ConnectTo, ConnectionTo, Result};

use crate::agent::BitfunAppRuntime;
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
}

impl BitfunAppServer {
    pub fn new(runtime: BitfunAppRuntime) -> Self {
        Self {
            runtime: Arc::new(runtime),
            management: None,
        }
    }

    pub fn with_management(mut self, management: Arc<AppManagementService>) -> Self {
        self.management = Some(management);
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
        let event_state = Arc::new(ConnectionEventState::new());

        AppServer
            .builder()
            .name("bitfun-app-server")
            .with_connection_builder(handlers::app::builder(
                runtime.clone(),
                event_state.clone(),
                management.clone(),
            ))
            .with_connection_builder(handlers::agent::builder(
                runtime.clone(),
                management.clone(),
            ))
            .with_connection_builder(handlers::account::builder(management.clone()))
            .with_connection_builder(handlers::session::builder(runtime.clone()))
            .with_connection_builder(handlers::permission::builder(runtime.clone()))
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
                event_forwarder::run(runtime, management, cx, event_state).await
            })
            .await
    }
}
