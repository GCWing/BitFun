//! Server-host app-server wiring: build the in-process `BitfunAppServer` from
//! the product-assembled [`AgentRuntime`] and return a cloneable handle.
//!
//! The containing Web Server was already deprecated before this refactor.
//! This wiring exists to validate the App Server boundary and is not required
//! to provide complete legacy Web/Desktop behavior or production compatibility.
//!
//! Under browser-direct ACP-over-WS (Step 2) the server host no longer pairs
//! the app-server with an in-process client over `in_memory_pair`. Instead each
//! WebSocket connection is handed straight to [`BitfunAppServer::serve`] via the
//! [`crate::routes::ws_transport`] `Lines` adapter, so the browser connects
//! directly to the in-process app-server over native JSON-RPC. This module only
//! constructs the [`BitfunAppRuntime`] and wraps it in a [`BitfunAppServer`]
//! (cheap `Clone` via the inner `Arc`); `serve` runs once per WS connection.

use std::sync::Arc;

use bitfun_agent_runtime::sdk::{AgentEventSource, AgentRuntime};
use bitfun_app_server::{BitfunAppRuntime, BitfunAppServer};

use crate::bootstrap::ServerAppState;

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

/// Lazily assemble the full Agent Runtime and wrap it in an in-process
/// [`BitfunAppServer`] for the Server Host.
///
/// This is the explicit, configuration-driven activation path: the HTTP shell
/// stays dormant (no Agent Runtime) unless the host is started with
/// `--with-runtime`. The returned [`ServerAppState`] binding must be kept alive
/// for the server's lifetime so its services outlive every WebSocket
/// connection; the app-server and spawned tasks hold their own Arc clones of
/// the coordinator, scheduler, and event queue.
pub(crate) async fn build_lazy(
    workspace: Option<String>,
) -> anyhow::Result<(BitfunAppServer, Arc<ServerAppState>)> {
    let server_state = crate::bootstrap::initialize(workspace).await?;

    // Build the agent runtime the same way the Desktop session application does,
    // then build an in-process `BitfunAppServer` for it. Each WebSocket
    // connection is handed straight to `BitfunAppServer::serve` over a WS-bridged
    // `Lines` transport (browser-direct ACP-over-WS, Step 2), so the browser
    // connects directly to the in-process app-server over native JSON-RPC — no
    // shared in-process client, no custom WS envelope.
    let agent_runtime =
        bitfun_core::product_runtime::CoreProductAgentRuntime::build_session_surface(
            server_state.coordinator.clone(),
            server_state.scheduler.clone(),
            server_state.token_usage_service.clone(),
        )
        .map_err(|error| anyhow::anyhow!("Failed to build agent runtime: {error}"))?;
    // The event source wraps the same `EventQueue` the coordinator publishes to;
    // each connection's `serve` main loop subscribes independently and projects
    // runtime events to the frontend shape before pushing them to the browser.
    let event_source =
        bitfun_agent_runtime::sdk::AgentEventSource::new(server_state.event_queue.clone());

    Ok((build(agent_runtime, event_source), server_state))
}
