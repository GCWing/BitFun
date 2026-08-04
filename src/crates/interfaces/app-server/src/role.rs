//! Generic JSON-RPC app-server roles built on `agent_client_protocol`.
//!
//! These are protocol-agnostic counterparts of the built-in ACP
//! [`Agent`](agent_client_protocol::Agent)/[`Client`](agent_client_protocol::Client)
//! pair: [`AppServer`] receives requests and sends responses/notifications,
//! [`AppClient`] sends requests and receives responses/notifications. They do
//! not bind any ACP schema; consumers register their own `JsonRpcRequest` /
//! `JsonRpcNotification` types via [`Builder::on_receive_request`] etc.
//!
//! `HasPeer` is implemented per-role on itself because
//! [`ConnectionTo::send_request`] requires `Counterpart: HasPeer<Counterpart>`,
//! matching how the built-in `Client`/`Agent` roles are wired
//! (`impl HasPeer<Client> for Client`, not for `Agent`).

use agent_client_protocol::role::{HasPeer, RemoteStyle};
use agent_client_protocol::{Builder, ConnectionTo, Dispatch, Handled, Role, RoleId};

/// The server role of a generic JSON-RPC app-server connection.
///
/// Use `AppServer.builder()` and register request/notification handlers, then
/// `connect_to(transport)` to serve. Handlers receive a
/// [`ConnectionTo<AppClient>`] for sending notifications back to the client.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AppServer;

/// The client role of a generic JSON-RPC app-server connection.
///
/// Use `AppClient.builder()` and `connect_with(transport, main_fn)` to drive
/// the connection; `main_fn` receives a [`ConnectionTo<AppServer>`] for
/// sending requests/notifications and awaiting responses.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AppClient;

impl Role for AppServer {
    type Counterpart = AppClient;

    async fn default_handle_dispatch_from(
        &self,
        message: Dispatch,
        _connection: ConnectionTo<AppServer>,
    ) -> Result<Handled<Dispatch>, agent_client_protocol::Error> {
        // No default handler: unmatched messages fall through to the caller's
        // `on_receive_dispatch` handler (or are dropped if none is registered).
        Ok(Handled::No {
            message,
            retry: false,
        })
    }

    fn role_id(&self) -> RoleId {
        RoleId::from_singleton(self)
    }

    fn counterpart(&self) -> Self::Counterpart {
        AppClient
    }
}

impl AppServer {
    /// Create a connection builder playing the server role.
    pub fn builder(self) -> Builder<AppServer> {
        Builder::new(self)
    }
}

impl HasPeer<AppServer> for AppServer {
    fn remote_style(&self, _peer: AppServer) -> RemoteStyle {
        RemoteStyle::Counterpart
    }
}

impl Role for AppClient {
    type Counterpart = AppServer;

    async fn default_handle_dispatch_from(
        &self,
        message: Dispatch,
        _connection: ConnectionTo<AppClient>,
    ) -> Result<Handled<Dispatch>, agent_client_protocol::Error> {
        Ok(Handled::No {
            message,
            retry: false,
        })
    }

    fn role_id(&self) -> RoleId {
        RoleId::from_singleton(self)
    }

    fn counterpart(&self) -> Self::Counterpart {
        AppServer
    }
}

impl AppClient {
    /// Create a connection builder playing the client role.
    pub fn builder(self) -> Builder<AppClient> {
        Builder::new(self)
    }
}

impl HasPeer<AppClient> for AppClient {
    fn remote_style(&self, _peer: AppClient) -> RemoteStyle {
        RemoteStyle::Counterpart
    }
}
