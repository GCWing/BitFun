//! Versioned JSON-RPC contracts for the local SDK Host.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub const JSON_RPC_VERSION: &str = "2.0";
pub const PROTOCOL_VERSION: u32 = 1;

pub const METHOD_INITIALIZE: &str = "initialize";
pub const METHOD_SESSION_CREATE: &str = "session/create";
pub const METHOD_QUERY_START: &str = "query/start";
pub const METHOD_QUERY_CANCEL: &str = "query/cancel";
pub const METHOD_SESSION_CLOSE: &str = "session/close";
pub const METHOD_SHUTDOWN: &str = "shutdown";
pub const NOTIFICATION_QUERY_EVENT: &str = "query/event";
pub const NOTIFICATION_QUERY_RESULT: &str = "query/result";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    String(String),
}

impl RequestId {
    pub fn correlation_id(&self) -> String {
        match self {
            Self::Number(value) => format!("request:number:{value}"),
            Self::String(value) => format!("request:string:{value}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    /// Absent for a JSON-RPC notification. `null` remains invalid at the
    /// transport envelope boundary and is not treated as a notification.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_request_id",
        skip_serializing_if = "Option::is_none"
    )]
    pub id: Option<RequestId>,
    pub method: String,
    #[serde(default = "empty_object")]
    pub params: serde_json::Value,
}

impl JsonRpcRequest {
    pub fn params_as<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.params.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JsonRpcSuccessResponse<T> {
    pub jsonrpc: &'static str,
    pub id: RequestId,
    pub result: T,
}

impl<T> JsonRpcSuccessResponse<T> {
    pub fn new(id: RequestId, result: T) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION,
            id,
            result,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JsonRpcErrorResponse {
    pub jsonrpc: &'static str,
    pub id: Option<RequestId>,
    pub error: JsonRpcErrorObject,
}

impl JsonRpcErrorResponse {
    pub fn new(id: RequestId, rpc_code: i32, message: impl Into<String>, data: ErrorData) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION,
            id: Some(id),
            error: JsonRpcErrorObject {
                code: rpc_code,
                message: message.into(),
                data,
            },
        }
    }

    pub fn parse_error(message: impl Into<String>, correlation_id: impl Into<String>) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION,
            id: None,
            error: JsonRpcErrorObject {
                code: -32700,
                message: message.into(),
                data: ErrorData {
                    code: ErrorCode::InvalidRequest,
                    stage: ErrorStage::Protocol,
                    retryable: false,
                    correlation_id: correlation_id.into(),
                    recovery: None,
                },
            },
        }
    }

    pub fn invalid_request(
        id: Option<RequestId>,
        message: impl Into<String>,
        correlation_id: impl Into<String>,
    ) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION,
            id,
            error: JsonRpcErrorObject {
                code: -32600,
                message: message.into(),
                data: ErrorData {
                    code: ErrorCode::InvalidRequest,
                    stage: ErrorStage::Protocol,
                    retryable: false,
                    correlation_id: correlation_id.into(),
                    recovery: None,
                },
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JsonRpcErrorObject {
    pub code: i32,
    pub message: String,
    pub data: ErrorData,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JsonRpcNotification<T> {
    pub jsonrpc: &'static str,
    pub method: &'static str,
    pub params: T,
}

impl<T> JsonRpcNotification<T> {
    pub fn new(method: &'static str, params: T) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION,
            method,
            params,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: u32,
    pub client_info: ClientInfo,
    pub capabilities: ClientCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ClientCapabilities {
    pub server_notifications: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: u32,
    pub runtime_version: String,
    pub stability: Stability,
    pub capabilities: HostCapabilities,
}

impl InitializeResult {
    pub fn current(runtime_version: impl Into<String>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            runtime_version: runtime_version.into(),
            stability: Stability::NotDelivered,
            capabilities: HostCapabilities::current(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stability {
    /// Internal implementation candidate. It is not a supported SDK surface.
    NotDelivered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostCapabilities {
    pub session_create: bool,
    pub session_create_lifetime: SessionLifetime,
    pub query: bool,
    pub query_cancel: bool,
    pub session_close: bool,
    pub event_stream: bool,
    pub structured_output: bool,
    pub usage: bool,
    pub custom_tools: bool,
    pub permission_callbacks: bool,
    pub hooks: bool,
    pub mcp_configuration: bool,
    pub prestarted_transport: bool,
}

impl HostCapabilities {
    pub const fn current() -> Self {
        Self {
            session_create: true,
            session_create_lifetime: SessionLifetime::Connection,
            query: true,
            query_cancel: true,
            session_close: true,
            event_stream: true,
            structured_output: false,
            usage: false,
            custom_tools: false,
            permission_callbacks: false,
            hooks: false,
            mcp_configuration: false,
            prestarted_transport: false,
        }
    }
}

/// Persistence boundary of a Session visible through the internal Host candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionLifetime {
    /// Created and deleted by this Host connection.
    Connection,
}

fn deserialize_optional_request_id<'de, D>(deserializer: D) -> Result<Option<RequestId>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    RequestId::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    NotInitialized,
    AlreadyInitialized,
    VersionMismatch,
    CapabilityUnavailable,
    NotFound,
    PermissionDenied,
    ActionRequired,
    Authentication,
    RateLimited,
    ProviderQuota,
    ProviderBilling,
    ProviderUnavailable,
    ContextOverflow,
    ContentPolicy,
    Overloaded,
    Timeout,
    Cancelled,
    ProcessLost,
    CleanupRequired,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorStage {
    Protocol,
    Initialize,
    Session,
    Query,
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryAction {
    Initialize,
    Retry,
    UpdateSdk,
    RestartHost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorData {
    pub code: ErrorCode,
    pub stage: ErrorStage,
    pub retryable: bool,
    pub correlation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery: Option<RecoveryAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionCreateParams {
    #[serde(default)]
    pub session_name: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCreateResult {
    pub session_id: String,
    pub session_name: String,
    pub agent: String,
    pub lifetime: SessionLifetime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct QueryStartParams {
    pub prompt: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub session_name: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryStartResult {
    pub query_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub accepted: bool,
    pub created_session: bool,
    pub session_lifetime: SessionLifetime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct QueryCancelParams {
    pub query_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryCancelResult {
    pub query_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub requested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionCloseParams {
    pub session_id: String,
    #[serde(default)]
    pub wait_timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCloseResult {
    pub session_id: String,
    pub unloaded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShutdownParams {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShutdownResult {
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryEventParams {
    pub query_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub sequence: u64,
    pub event: QueryEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QueryEvent {
    AssistantTextDelta { text: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryTerminalStatus {
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResultParams {
    pub query_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub status: QueryTerminalStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<QueryResultError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryResultError {
    pub message: String,
    pub data: ErrorData,
}

impl QueryResultError {
    pub fn new(
        code: ErrorCode,
        retryable: bool,
        recovery: Option<RecoveryAction>,
        query_id: &str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            data: ErrorData {
                code,
                stage: ErrorStage::Query,
                retryable,
                correlation_id: format!("query:{query_id}"),
                recovery,
            },
        }
    }
}

fn empty_object() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}
