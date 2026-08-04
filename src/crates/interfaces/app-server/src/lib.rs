//! BitFun generic JSON-RPC app-server surface.
//!
//! This crate owns a protocol-agnostic JSON-RPC server/client scaffold built on
//! [`agent_client_protocol`] using custom roles ([`AppServer`]/[`AppClient`]),
//! instead of the built-in ACP `Agent`/`Client` roles. Consumers register their
//! own `JsonRpcRequest` / `JsonRpcNotification` types; the crate binds no
//! schema method set, unlike [`bitfun_acp`].
//!
//! The optional `agent` module is the Phase 2 wiring: [`BitfunAppServer`]
//! exposes a ready set of agent kernel operations (create / list / delete /
//! submit / run / cancel) over a host-injected [`AgentRuntime`], using the
//! generic `AppServer` role and the schema in [`schema`]. Hosts that want a
//! purely schema-free scaffold can ignore `agent` / `schema` / `server` and
//! register their own message types directly on `AppServer::builder()`.
//!
//! # Example
//!
//! ```no_run
//! use bitfun_app_server::{AppServer, AppClient, transport};
//! use bitfun_app_server::prelude::*;
//! # use serde::{Deserialize, Serialize};
//! #
//! # #[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcRequest)]
//! # #[request(method = "ping", response = Pong)]
//! # struct Ping;
//! # #[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcResponse)]
//! # struct Pong;
//! # async fn run() -> Result<(), agent_client_protocol::Error> {
//! let (server_transport, client_transport) = transport::in_memory_channel_pair();
//! // server: register handlers and connect_to
//! // client: connect_with and send_request
//! # Ok(())
//! # }
//! ```
//!
//! [`bitfun_acp`]: bitfun_acp
//!
//! # Crate boundary (preview)
//!
//! This crate is an **internal interface crate**, not a versioned public API.
//! The server-side surface ([`BitfunAppServer`], [`schema`], [`agent`]) is the
//! production path consumed by the Server Host. The client-side exports
//! ([`AppServerClient`], [`FrontendEvent`], [`connect`]) are test-only
//! utilities -- they have no production consumer and are `#[doc(hidden)]` to
//! avoid implying a stable public SDK. They will be replaced by a proper
//! versioned event envelope and connection protocol in a follow-up.

// Lifted from the default 128: the current bounded request groups already form
// a deep `ChainedHandler` type when `BitfunAppServer::serve` is instantiated by
// the integration tests. This is an implementation limit for the delivered
// surface, not a commitment to absorb every backend API into this crate.
#![recursion_limit = "256"]

pub mod agent;
pub mod client;
pub mod role;
pub mod schema;
pub mod server;
pub mod transport;

pub use agent::BitfunAppRuntime;
pub use agent_client_protocol as protocol;
// `connect`, `AppServerClient`, and `FrontendEvent` are test-only utilities
// with no production consumer. They are `#[doc(hidden)]` to avoid implying a
// versioned public client SDK; they will be replaced by a proper versioned
// event envelope in a follow-up.
#[doc(hidden)]
pub use client::{connect, AppServerClient, FrontendEvent};
pub use role::{AppClient, AppServer};
pub use server::BitfunAppServer;

/// Convenience prelude for consumers building an app-server connection.
pub mod prelude {
    pub use crate::{
        agent, client, schema, server, transport, AppClient, AppServer, BitfunAppRuntime,
        BitfunAppServer,
    };
    pub use agent_client_protocol::{
        Builder, ConnectionTo, Dispatch, Handled, JsonRpcNotification, JsonRpcRequest,
        JsonRpcResponse, Responder, SentRequest,
    };
    // Macro re-exports so callers do not need a direct `agent_client_protocol`
    // path for handler registration.
    pub use agent_client_protocol::{
        on_receive_dispatch, on_receive_notification, on_receive_request,
    };
}
