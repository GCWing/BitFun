//! MCP server data contracts.

use crate::mcp::config::ConfigLocation;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;

/// MCP server type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MCPServerType {
    Local,
    Remote,
}

/// MCP server status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MCPServerStatus {
    Uninitialized,
    Starting,
    Connected,
    Healthy,
    NeedsAuth,
    Reconnecting,
    Failed,
    Stopping,
    Stopped,
}

/// MCP server transport.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MCPServerTransport {
    Stdio,
    StreamableHttp,
    Sse,
}

impl MCPServerTransport {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
            Self::StreamableHttp => "streamable-http",
            Self::Sse => "sse",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MCPServerOAuthConfig {
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_metadata_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MCPServerXaaConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audience: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
}

/// MCP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPServerConfig {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub server_type: MCPServerType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<MCPServerTransport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Additional HTTP headers for remote MCP servers (Cursor-style `headers`).
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default = "default_true")]
    pub auto_start: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub location: ConfigLocation,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub settings: HashMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth: Option<MCPServerOAuthConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xaa: Option<MCPServerXaaConfig>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MCPServerConfigValidationError {
    message: String,
}

impl MCPServerConfigValidationError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for MCPServerConfigValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for MCPServerConfigValidationError {}

impl MCPServerConfig {
    pub fn resolved_transport(&self) -> MCPServerTransport {
        self.transport.unwrap_or(match self.server_type {
            MCPServerType::Local => MCPServerTransport::Stdio,
            MCPServerType::Remote => MCPServerTransport::StreamableHttp,
        })
    }

    pub fn validate(&self) -> Result<(), MCPServerConfigValidationError> {
        if self.id.is_empty() {
            return Err(MCPServerConfigValidationError::new(
                "MCP server id cannot be empty",
            ));
        }

        if self.name.is_empty() {
            return Err(MCPServerConfigValidationError::new(
                "MCP server name cannot be empty",
            ));
        }

        let transport = self.resolved_transport();
        match self.server_type {
            MCPServerType::Local => {
                if self.command.is_none() {
                    return Err(MCPServerConfigValidationError::new(format!(
                        "Local MCP server '{}' must have a command",
                        self.id
                    )));
                }

                if transport != MCPServerTransport::Stdio {
                    return Err(MCPServerConfigValidationError::new(format!(
                        "Local MCP server '{}' must use stdio transport, got '{}'",
                        self.id,
                        transport.as_str()
                    )));
                }
            }
            MCPServerType::Remote => {
                if self.url.is_none() {
                    return Err(MCPServerConfigValidationError::new(format!(
                        "Remote MCP server '{}' must have a URL",
                        self.id
                    )));
                }

                if let Some(oauth) = &self.oauth {
                    if let Some(port) = oauth.callback_port {
                        if port == 0 {
                            return Err(MCPServerConfigValidationError::new(format!(
                                "Remote MCP server '{}' OAuth callbackPort must be greater than 0",
                                self.id
                            )));
                        }
                    }
                }

                if !matches!(
                    transport,
                    MCPServerTransport::StreamableHttp | MCPServerTransport::Sse
                ) {
                    return Err(MCPServerConfigValidationError::new(format!(
                        "Remote MCP server '{}' must use streamable-http or sse transport, got '{}'",
                        self.id,
                        transport.as_str()
                    )));
                }
            }
        }

        Ok(())
    }
}
