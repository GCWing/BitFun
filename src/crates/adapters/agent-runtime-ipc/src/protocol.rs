use serde::{Deserialize, Serialize};
use std::fmt;

use crate::{RuntimeIpcOperation, RuntimeIpcOperationResult};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeIpcFrame {
    Initialize {
        request_id: u64,
        request: InitializeRequest,
    },
    Initialized {
        request_id: u64,
        result: InitializeResult,
    },
    Request {
        request_id: u64,
        operation: RuntimeIpcOperation,
    },
    Response {
        request_id: u64,
        result: RuntimeIpcOperationResult,
    },
    Error {
        request_id: Option<u64>,
        error: RuntimeIpcError,
    },
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitializeRequest {
    pub protocol_version: u32,
    pub instance_identity: String,
    pub token: String,
    pub client_id: String,
    pub client_version: String,
}

impl fmt::Debug for InitializeRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InitializeRequest")
            .field("protocol_version", &self.protocol_version)
            .field("instance_identity", &self.instance_identity)
            .field("token", &"[REDACTED]")
            .field("client_id", &self.client_id)
            .field("client_version", &self.client_version)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitializeResult {
    pub protocol_version: u32,
    pub instance_identity: String,
    pub server_version: String,
    pub capabilities: RuntimeIpcCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIpcCapabilities {
    pub health: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthResult {
    pub instance_identity: String,
    pub process_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeIpcErrorCode {
    InvalidRequest,
    Unauthorized,
    IncompatibleProtocol,
    WrongInstance,
    FrameTooLarge,
    Unavailable,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIpcError {
    pub code: RuntimeIpcErrorCode,
    pub message: String,
}
