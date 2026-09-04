//! MCP server configuration types.

use crate::util::errors::OpenBitFunError;

use openbitfun_services_integrations::mcp::server::MCPServerConfigValidationError;
pub use openbitfun_services_integrations::mcp::server::{
    MCPServerConfig, MCPServerOAuthConfig, MCPServerTimeouts, MCPServerTransport,
    MCPServerXaaConfig,
};

impl From<MCPServerConfigValidationError> for OpenBitFunError {
    fn from(error: MCPServerConfigValidationError) -> Self {
        Self::Configuration(error.to_string())
    }
}
