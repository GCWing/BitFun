//! Shared MCP runtime error contracts.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MCPRuntimeErrorKind {
    Configuration,
    Validation,
    Io,
    Serialization,
    Deserialization,
    Process,
    MCP,
    NotFound,
    NotImplemented,
    Timeout,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MCPRuntimeError {
    kind: MCPRuntimeErrorKind,
    message: String,
}

pub type MCPRuntimeResult<T> = Result<T, MCPRuntimeError>;

impl MCPRuntimeError {
    pub fn new(kind: MCPRuntimeErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> MCPRuntimeErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn configuration(message: impl Into<String>) -> Self {
        Self::new(MCPRuntimeErrorKind::Configuration, message)
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(MCPRuntimeErrorKind::Validation, message)
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new(MCPRuntimeErrorKind::Io, message)
    }

    pub fn serialization(message: impl Into<String>) -> Self {
        Self::new(MCPRuntimeErrorKind::Serialization, message)
    }

    pub fn deserialization(message: impl Into<String>) -> Self {
        Self::new(MCPRuntimeErrorKind::Deserialization, message)
    }

    pub fn process(message: impl Into<String>) -> Self {
        Self::new(MCPRuntimeErrorKind::Process, message)
    }

    pub fn mcp(message: impl Into<String>) -> Self {
        Self::new(MCPRuntimeErrorKind::MCP, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(MCPRuntimeErrorKind::NotFound, message)
    }

    pub fn not_implemented(message: impl Into<String>) -> Self {
        Self::new(MCPRuntimeErrorKind::NotImplemented, message)
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new(MCPRuntimeErrorKind::Timeout, message)
    }

    pub fn other(message: impl Into<String>) -> Self {
        Self::new(MCPRuntimeErrorKind::Other, message)
    }
}

impl fmt::Display for MCPRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for MCPRuntimeError {}

impl From<std::io::Error> for MCPRuntimeError {
    fn from(error: std::io::Error) -> Self {
        Self::io(error.to_string())
    }
}

impl From<serde_json::Error> for MCPRuntimeError {
    fn from(error: serde_json::Error) -> Self {
        Self::serialization(error.to_string())
    }
}

impl From<anyhow::Error> for MCPRuntimeError {
    fn from(error: anyhow::Error) -> Self {
        Self::other(error.to_string())
    }
}
