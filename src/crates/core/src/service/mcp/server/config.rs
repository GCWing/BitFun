//! MCP server configuration types.

use crate::util::errors::BitFunError;

pub use bitfun_services_integrations::mcp::server::{
    MCPServerConfig, MCPServerConfigValidationError, MCPServerOAuthConfig, MCPServerTransport,
    MCPServerXaaConfig,
};

impl From<MCPServerConfigValidationError> for BitFunError {
    fn from(error: MCPServerConfigValidationError) -> Self {
        Self::Configuration(error.to_string())
    }
}
