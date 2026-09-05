//! MCP server management module
//!
//! Manages MCP server process lifecycles, connections, and registration.

mod config;
mod connection;
mod manager;
mod process;
mod registry;

pub use config::{
    MCPServerConfig, MCPServerOAuthConfig, MCPServerTimeouts, MCPServerTransport,
    MCPServerXaaConfig,
};
pub use connection::{MCPConnection, MCPConnectionPool};
pub use manager::MCPServerManager;
pub use openbitfun_services_integrations::mcp::server::{MCPServerStatus, MCPServerType};
pub use process::MCPServerProcess;
pub use registry::MCPServerRegistry;
