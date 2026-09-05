//! JSON-RPC wire contract for the OpenBitFun app-server surface.
//!
//! Schema types are grouped by product domain while this module re-exports the
//! complete contract. Existing consumers can continue importing types from
//! `openbitfun_app_server::schema` without depending on the internal layout.

pub use openbitfun_app_server_protocol::agent::*;
pub use openbitfun_app_server_protocol::app::*;
pub use openbitfun_app_server_protocol::config::*;
pub use openbitfun_app_server_protocol::event::{
    AgentEventNotification as SessionEventNotification, ConfigEventNotification, ConfigUpdate,
    EventCursor, EventStream, EventStreamState, EventStreamStateNotification,
    FrontendEventNotification, PermissionEventNotification, ResyncDirective, SyncEventsRequest,
    SyncEventsResponse,
};
pub use openbitfun_app_server_protocol::git::*;
pub use openbitfun_app_server_protocol::i18n::*;
pub use openbitfun_app_server_protocol::permission::*;
pub use openbitfun_app_server_protocol::search::*;
pub use openbitfun_app_server_protocol::session::*;
