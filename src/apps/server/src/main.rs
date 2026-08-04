// The app-server's `Builder::on_receive_request(...)` chain monomorphizes
// into a deeply nested `ChainedHandler<ChainedHandler<…>>` (~20 request
// handlers plus a dispatch handler). Computing the resulting connection
// future's layout pushes the trait solver past the default 128 limit, so bump
// it. Matches the compiler's own suggestion in the overflow diagnostic.
#![recursion_limit = "256"]

//! Current entrypoint for the paused Web Server product surface.
//!
//! Development is paused, not retired. The current partial host keeps its live
//! protocol and security behavior stable while App Server becomes the shared
//! protocol boundary; it does not yet provide complete Web/Desktop parity or
//! production readiness.

use anyhow::Result;
/// BitFun Server
///
/// Web server with support for:
/// - RESTful API
/// - WebSocket real-time communication
use axum::{
    http::{HeaderValue, Method, Uri},
    routing::get,
    Json, Router,
};
use clap::Parser;
use serde::Serialize;
use std::{collections::HashSet, net::SocketAddr, path::PathBuf, sync::Arc};
use tower_http::cors::CorsLayer;

#[cfg(feature = "paused-web-server-source-check")]
#[allow(dead_code)]
mod ai_relay;
mod app_server;
mod bootstrap;
mod routes;
#[cfg(feature = "paused-web-server-source-check")]
#[allow(dead_code)]
mod rpc_dispatcher;

#[cfg(feature = "paused-web-server-source-check")]
pub(crate) struct DispatchHostState {
    path_manager: Arc<bitfun_core::infrastructure::PathManager>,
    ssh_manager: Arc<bitfun_core::service::remote_ssh::SSHConnectionManager>,
}

/// Application state
#[derive(Clone)]
pub struct AppState {
    #[cfg(feature = "paused-web-server-source-check")]
    external_workspace_root: Option<PathBuf>,
    allowed_browser_origins: Arc<HashSet<String>>,
    #[cfg(feature = "paused-web-server-source-check")]
    dispatch_host: Option<Arc<DispatchHostState>>,
}

const DEFAULT_ALLOWED_BROWSER_ORIGINS: [&str; 2] =
    ["http://localhost:1422", "http://127.0.0.1:1422"];

#[derive(Debug, Parser)]
#[command(name = "bitfun-server")]
struct ServerArgs {
    /// Initial local project workspace requested for this Server Host.
    #[arg(long, value_name = "PATH")]
    workspace: Option<PathBuf>,

    /// Browser origin allowed to connect to this Server Host. Repeat to allow more than one.
    /// When omitted, only BitFun's local Web development origins are allowed.
    #[arg(long = "allowed-origin", value_name = "ORIGIN")]
    allowed_origins: Vec<String>,
}

/// Health check response
#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    uptime_seconds: u64,
}

/// Health check handler
async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: 0,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!("BitFun Server v{}", env!("CARGO_PKG_VERSION"));

    let args = ServerArgs::parse();
    let workspace_root = args
        .workspace
        .map(|path| {
            if !path.is_absolute() {
                return Err(anyhow::anyhow!("--workspace must be an absolute path"));
            }
            path.canonicalize()
                .map_err(|error| anyhow::anyhow!("Could not open Server workspace: {error}"))
        })
        .transpose()?;

    // Initialize the full agentic stack (coordinator, scheduler, token usage,
    // MCP/config/filesystem services, event queue). This binding is held alive
    // for the lifetime of the server so its services outlive every websocket
    // connection; per-WebSocket App Server serve tasks and Runtime services hold
    // their own Arc clones of the coordinator, scheduler, and event queue.
    let server_state = bootstrap::initialize(
        workspace_root
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
    )
    .await?;

    // Build the agent runtime the same way the Desktop session application does,
    // then build an in-process `BitfunAppServer` for it. Each WebSocket
    // connection is handed straight to `BitfunAppServer::serve` over a WS-bridged
    // `Lines` transport, so the browser
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
    let bitfun_app_server = app_server::build(agent_runtime, event_source);

    tracing::info!(
        "App-server ready; each WebSocket connection drives one in-process serve over native JSON-RPC"
    );

    let configured_origins = if args.allowed_origins.is_empty() {
        DEFAULT_ALLOWED_BROWSER_ORIGINS
            .iter()
            .map(|origin| (*origin).to_string())
            .collect()
    } else {
        args.allowed_origins
    };
    let allowed_browser_origins = configured_origins
        .iter()
        .map(|origin| normalize_browser_origin(origin))
        .collect::<Result<HashSet<_>>>()?;
    let cors_origins = allowed_browser_origins
        .iter()
        .map(|origin| {
            HeaderValue::from_str(origin)
                .map_err(|_| anyhow::anyhow!("--allowed-origin contains an invalid header value"))
        })
        .collect::<Result<Vec<_>>>()?;

    let app_state = AppState {
        #[cfg(feature = "paused-web-server-source-check")]
        external_workspace_root: workspace_root.clone(),
        allowed_browser_origins: Arc::new(allowed_browser_origins),
        #[cfg(feature = "paused-web-server-source-check")]
        dispatch_host: None,
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/info", get(routes::api::api_info))
        .route("/ws", get(routes::websocket::websocket_handler))
        .layer(
            CorsLayer::new()
                .allow_methods([Method::GET])
                .allow_origin(cors_origins),
        )
        // The BitFunAppServer is cloned per WebSocket connection through an axum
        // Extension (cheap Arc clone); each connection spawns its own `serve`
        // over a WS-bridged `Lines` transport.
        .layer(axum::Extension(bitfun_app_server))
        .with_state(app_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    tracing::info!("Server started: http://{}", addr);
    tracing::info!("WebSocket endpoint: ws://{}/ws", addr);
    tracing::info!("Health check: http://{}/health", addr);
    tracing::info!(
        allowed_origin_count = configured_origins.len(),
        "Browser origin policy configured"
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

pub(crate) fn normalize_browser_origin(value: &str) -> Result<String> {
    let trimmed = value.trim();
    let uri = trimmed
        .parse::<Uri>()
        .map_err(|_| anyhow::anyhow!("--allowed-origin must be an HTTP or HTTPS origin"))?;
    let scheme = uri
        .scheme_str()
        .filter(|scheme| {
            scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
        })
        .ok_or_else(|| anyhow::anyhow!("--allowed-origin must use http or https"))?;
    let authority = uri
        .authority()
        .ok_or_else(|| anyhow::anyhow!("--allowed-origin must include a host"))?;
    if uri
        .path_and_query()
        .is_some_and(|path_and_query| path_and_query.as_str() != "/")
    {
        return Err(anyhow::anyhow!(
            "--allowed-origin must not include a path, query, or fragment"
        ));
    }
    Ok(format!("{scheme}://{authority}").to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_origins_are_normalized_for_exact_matching() {
        assert_eq!(
            normalize_browser_origin(" HTTPS://Example.TEST:8443/ ").unwrap(),
            "https://example.test:8443"
        );
    }

    #[test]
    fn browser_origins_reject_non_origins() {
        for invalid in [
            "file:///tmp/index.html",
            "https://example.test/app",
            "https://example.test?mode=web",
            "example.test",
        ] {
            assert!(normalize_browser_origin(invalid).is_err(), "{invalid}");
        }
    }
}
