//! MCP server management module
//!
//! Manages MCP server process lifecycles, connections, and registration.

pub mod connection;
pub mod manager;
pub mod process;
pub mod registry;

pub use connection::{MCPConnection, MCPConnectionPool};
pub use manager::MCPServerManager;
pub use process::{MCPServerProcess, MCPServerStatus, MCPServerType};
pub use registry::MCPServerRegistry;

/// MCP server configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPServerConfig {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub server_type: MCPServerType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    /// Additional HTTP headers for remote MCP servers (Cursor-style `headers`).
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default = "default_true")]
    pub auto_start: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub location: crate::service::mcp::config::ConfigLocation,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub settings: std::collections::HashMap<String, serde_json::Value>,
}

fn default_true() -> bool {
    true
}

impl MCPServerConfig {
    /// Validates the configuration.
    pub fn validate(&self) -> crate::util::errors::BitFunResult<()> {
        if self.id.is_empty() {
            return Err(crate::util::errors::BitFunError::Configuration(
                "MCP server id cannot be empty".to_string(),
            ));
        }

        if self.name.is_empty() {
            return Err(crate::util::errors::BitFunError::Configuration(
                "MCP server name cannot be empty".to_string(),
            ));
        }

        match self.server_type {
            MCPServerType::Local => {
                if self.command.is_none() {
                    return Err(crate::util::errors::BitFunError::Configuration(format!(
                        "Local MCP server '{}' must have a command",
                        self.id
                    )));
                }
            }
            MCPServerType::Remote => {
                if self.url.is_none() {
                    return Err(crate::util::errors::BitFunError::Configuration(format!(
                        "Remote MCP server '{}' must have a URL",
                        self.id
                    )));
                }
            }
            MCPServerType::Container => {
                if self.command.is_none() {
                    return Err(crate::util::errors::BitFunError::Configuration(format!(
                        "Container MCP server '{}' must have a command",
                        self.id
                    )));
                }
            }
        }

        Ok(())
    }
}
