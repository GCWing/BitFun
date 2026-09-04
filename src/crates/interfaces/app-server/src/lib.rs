//! OpenBitFun generic JSON-RPC App Server host surface.
//!
//! This crate owns a protocol-agnostic JSON-RPC server/client scaffold built on
//! [`agent_client_protocol`] using custom roles ([`AppServer`]/[`AppClient`]),
//! instead of the built-in ACP `Agent`/`Client` roles. Consumers register their
//! own `JsonRpcRequest` / `JsonRpcNotification` types; the crate binds no
//! schema method set, unlike [`openbitfun_acp`].
//!
//! [`OpenBitFunAppServer`] exposes the selected Agent Kernel and management
//! operations over a host-injected runtime. Behavior-light wire contracts are
//! owned by `openbitfun-app-server-protocol`; [`schema`] remains a compatibility
//! re-export for existing server-side imports. Typed clients are owned by the
//! separate `openbitfun-app-server-client` crate.
//!
//! # Example
//!
//! ```no_run
//! use openbitfun_app_server::{AppServer, AppClient, transport};
//! use openbitfun_app_server::prelude::*;
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
//! [`openbitfun_acp`]: openbitfun_acp
//!
//! # Crate boundary
//!
//! This crate is an **internal interface crate**, not a versioned public API.
//! The server-side surface ([`OpenBitFunAppServer`], [`schema`], [`agent`]) is the
//! production path consumed by the Server Host. This crate does not own a
//! second client implementation.

// Lifted from the default 128: the `AppServer` builder chains one
// `ChainedHandler` layer per registered request handler, and with the
// agent-kernel + permission + git + config surface all on one builder the
// monomorphized handler tower overflows the default recursion limit when the
// `agent_kernel` integration test instantiates the full `OpenBitFunAppServer::serve`
// connection. Raise it so the chain keeps compiling as more host-service groups
// land under option C.
#![recursion_limit = "256"]

pub mod agent;
pub mod management;
pub mod role;
pub mod schema;
pub mod server;
pub mod transport;

pub use agent::OpenBitFunAppRuntime;
pub use agent_client_protocol as protocol;
pub use management::{
    AppManagementCapabilities, AppManagementError, AppManagementErrorKind, AppManagementResult,
    AppManagementService, EXTERNAL_HOOKS_CAPABILITY, EXTERNAL_SOURCES_CAPABILITY,
    NATIVE_HOOKS_CAPABILITY,
};
pub use role::{AppClient, AppServer};
pub use server::host_policy::{
    AppServerDisconnect, AppServerHostLimits, AppServerHostPolicy, HostPolicyViolation,
    DEFAULT_EVENT_BUFFER_CAPACITY, DEFAULT_MAX_FRAME_BYTES,
};
pub use server::OpenBitFunAppServer;

/// Convenience prelude for consumers building an app-server connection.
pub mod prelude {
    pub use crate::{
        agent, schema, server, transport, AppClient, AppServer, OpenBitFunAppRuntime,
        OpenBitFunAppServer,
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
