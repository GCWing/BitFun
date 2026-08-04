//! Server-host app-server wiring: build the in-process `BitfunAppServer` from
//! the product-assembled [`AgentRuntime`] and return a cloneable handle.
//!
//! The containing Web Server is paused, not deprecated. This current partial
//! wiring validates the App Server boundary and protects its live protocol
//! behavior, but it is not yet a production-complete Web/Desktop surface.
//!
//! Under browser-direct App Server over WebSocket, the host does not pair
//! the app-server with an in-process client over `in_memory_pair`. Instead each
//! WebSocket connection is handed straight to [`BitfunAppServer::serve`] via the
//! [`crate::routes::ws_transport`] `Lines` adapter, so the browser connects
//! directly to the in-process app-server over native JSON-RPC. This module only
//! constructs the [`BitfunAppRuntime`] and wraps it in a [`BitfunAppServer`]
//! (cheap `Clone` via the inner `Arc`); `serve` runs once per WS connection.

use bitfun_agent_runtime::sdk::{AgentEventSource, AgentRuntime};
use bitfun_app_server::{BitfunAppRuntime, BitfunAppServer};

/// Build the in-process `BitfunAppServer` for the Server Host.
///
/// Constructs a [`BitfunAppRuntime`] from the product-assembled `runtime` and
/// its `event_source`, wraps it in a [`BitfunAppServer`] (cheap `Clone`), and
/// returns it. The websocket handler clones this handle once per connection and
/// spawns `serve` on a WS-bridged `Lines` transport.
///
/// The caller must keep the runtime services (coordinator, scheduler, ...) and
/// the `EventQueue` the `event_source` was built from alive for as long as the
/// [`BitfunAppServer`] is in use.
pub(crate) fn build(runtime: AgentRuntime, event_source: AgentEventSource) -> BitfunAppServer {
    let app_runtime = BitfunAppRuntime::new(runtime, event_source);
    BitfunAppServer::new(app_runtime)
}
