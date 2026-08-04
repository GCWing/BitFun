//! MCP server configuration types.

use crate::util::errors::BitFunError;

use bitfun_services_integrations::mcp::server::MCPServerConfigValidationError;
pub use bitfun_services_integrations::mcp::server::{
    MCPServerConfig, MCPServerOAuthConfig, MCPServerTimeouts, MCPServerTransport,
    MCPServerXaaConfig,
};

impl From<MCPServerConfigValidationError> for BitFunError {
    fn from(error: MCPServerConfigValidationError) -> Self {
        Self::Configuration(error.to_string())
    }
}
