//! Generic JSON-RPC roles shared by App Server clients and hosts.

use agent_client_protocol::role::{HasPeer, RemoteStyle};
use agent_client_protocol::{Builder, ConnectionTo, Dispatch, Handled, Role, RoleId};

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AppServer;

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AppClient;

impl Role for AppServer {
    type Counterpart = AppClient;

    async fn default_handle_dispatch_from(
        &self,
        message: Dispatch,
        _connection: ConnectionTo<AppServer>,
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
        AppClient
    }
}

impl AppServer {
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
    pub fn builder(self) -> Builder<AppClient> {
        Builder::new(self)
    }
}

impl HasPeer<AppClient> for AppClient {
    fn remote_style(&self, _peer: AppClient) -> RemoteStyle {
        RemoteStyle::Counterpart
    }
}
