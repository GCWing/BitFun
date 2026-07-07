//! Debug Mode runtime logging utilities.
//! Provides a shared instrumentation pipeline for desktop/server/cli + web.
//!
//! ## Module Structure
//! - `types` - Types and handlers for the HTTP ingest server (Config, State, Request, Response)
//! - `http_server` - The actual HTTP server implementation (axum-based)

pub mod http_server;
pub mod types;

pub use types::{
    handle_ingest, IngestLogRequest, IngestResponse, IngestServerConfig, IngestServerState,
    DEFAULT_INGEST_PORT,
};

pub use http_server::IngestServerManager;

pub use bitfun_services_integrations::debug_log::{
    append_log_async, DebugLogConfig, DebugLogEntry,
};
