//! Unified error handling
//!
//! Provide unified error types and handling for the whole application

use bitfun_core_types::errors::{
    ai_error_detail_from_message, classify_ai_error_message, AiErrorDetail, ErrorCategory,
};
use serde::Serialize;
use thiserror::Error;

/// Unified error type for the BitFun application
#[derive(Debug, Error, Serialize)]
pub enum BitFunError {
    #[error("Service error: {0}")]
    Service(String),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("Tool error: {0}")]
    Tool(String),

    #[error("AI client error: {0}")]
    AIClient(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Workspace error: {0}")]
    Workspace(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    #[serde(serialize_with = "serialize_io_error")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    #[serde(serialize_with = "serialize_serde_error")]
    Serialization(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Other error: {0}")]
    #[serde(serialize_with = "serialize_anyhow_error")]
    Other(#[from] anyhow::Error),

    #[error("Semaphore acquire error: {0}")]
    Semaphore(String),

    #[error("MCP error: {0}")]
    MCPError(String),

    #[error("Process error: {0}")]
    ProcessError(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("Cancelled: {0}")]
    Cancelled(String),
}

pub type BitFunResult<T> = Result<T, BitFunError>;

// Custom serialization functions for non-serializable error types
fn serialize_io_error<S>(err: &std::io::Error, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&err.to_string())
}

fn serialize_serde_error<S>(err: &serde_json::Error, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&err.to_string())
}

fn serialize_anyhow_error<S>(err: &anyhow::Error, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&err.to_string())
}

impl BitFunError {
    pub fn service<T: Into<String>>(msg: T) -> Self {
        Self::Service(msg.into())
    }

    pub fn agent<T: Into<String>>(msg: T) -> Self {
        Self::Agent(msg.into())
    }

    pub fn tool<T: Into<String>>(msg: T) -> Self {
        Self::Tool(msg.into())
    }

    pub fn config<T: Into<String>>(msg: T) -> Self {
        Self::Configuration(msg.into())
    }

    pub fn validation<T: Into<String>>(msg: T) -> Self {
        Self::Validation(msg.into())
    }

    pub fn ai<T: Into<String>>(msg: T) -> Self {
        Self::AIClient(msg.into())
    }

    pub fn http<T: Into<String>>(msg: T) -> Self {
        Self::Http(msg.into())
    }

    pub fn parse<T: Into<String>>(msg: T) -> Self {
        Self::Deserialization(msg.into())
    }

    pub fn workspace<T: Into<String>>(msg: T) -> Self {
        Self::Workspace(msg.into())
    }

    pub fn serialization<T: Into<String>>(msg: T) -> Self {
        Self::Serialization(serde_json::Error::io(std::io::Error::other(msg.into())))
    }

    pub fn session<T: Into<String>>(msg: T) -> Self {
        Self::Session(msg.into())
    }

    pub fn io<T: Into<String>>(msg: T) -> Self {
        Self::Io(std::io::Error::other(msg.into()))
    }

    pub fn cancelled<T: Into<String>>(msg: T) -> Self {
        Self::Cancelled(msg.into())
    }

    /// Infer an error category from this error for frontend-friendly classification.
    pub fn error_category(&self) -> ErrorCategory {
        match self {
            BitFunError::AIClient(msg) => classify_ai_error_message(msg),
            BitFunError::Timeout(_) => ErrorCategory::Timeout,
            BitFunError::Cancelled(_) => ErrorCategory::Unknown,
            _ => ErrorCategory::Unknown,
        }
    }

    /// Build a structured, provider-agnostic AI error detail for UI recovery.
    pub fn error_detail(&self) -> AiErrorDetail {
        let category = self.error_category();
        let message = self.to_string();
        ai_error_detail_from_message(&message, category)
    }
}

impl From<bitfun_agent_stream::StreamProcessorError> for BitFunError {
    fn from(error: bitfun_agent_stream::StreamProcessorError) -> Self {
        match error {
            bitfun_agent_stream::StreamProcessorError::AiClient(msg) => Self::AIClient(msg),
            bitfun_agent_stream::StreamProcessorError::Cancelled(msg) => Self::Cancelled(msg),
        }
    }
}

impl From<BitFunError> for String {
    fn from(err: BitFunError) -> String {
        err.to_string()
    }
}

impl From<String> for BitFunError {
    fn from(error: String) -> Self {
        BitFunError::Service(error)
    }
}

impl From<&str> for BitFunError {
    fn from(error: &str) -> Self {
        BitFunError::Service(error.to_string())
    }
}
