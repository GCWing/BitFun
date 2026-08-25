//! Versioned JSON-RPC contracts for the local SDK Host.

use bitfun_core_types::SessionExecutionTarget;
use bitfun_runtime_ports::AgentSessionCreateResult;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub const JSON_RPC_VERSION: &str = "2.0";
pub const PROTOCOL_VERSION: u32 = 6;

pub const METHOD_INITIALIZE: &str = "initialize";
pub const METHOD_SESSION_CREATE: &str = "session/create";
pub const METHOD_SESSION_RESUME: &str = "session/resume";
pub const METHOD_QUERY_START: &str = "query/start";
pub const METHOD_QUERY_CANCEL: &str = "query/cancel";
pub const METHOD_PERMISSION_RESPOND: &str = "permission/respond";
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

#[derive(Clone, PartialEq, Serialize, Deserialize)]
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

impl std::fmt::Debug for JsonRpcRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JsonRpcRequest")
            .field("jsonrpc", &self.jsonrpc)
            .field("id", &self.id)
            .field("method", &self.method)
            .field("params", &"<redacted>")
            .finish()
    }
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
                    operation_id: None,
                    causation_id: None,
                    outcome_certainty: OutcomeCertainty::NotStarted,
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
                    operation_id: None,
                    causation_id: None,
                    outcome_certainty: OutcomeCertainty::NotStarted,
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

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: u32,
    pub client_info: ClientInfo,
    pub capabilities: ClientCapabilities,
    pub model: TemporaryModelConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum TemporaryModelProvider {
    Openai,
    Responses,
    Anthropic,
    Gemini,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TemporaryModelConfig {
    pub provider: TemporaryModelProvider,
    pub model: String,
    pub api_key: String,
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    #[serde(default)]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ClientCapabilities {
    pub server_notifications: bool,
    pub permission_responses: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: u32,
    pub runtime_version: String,
    pub stability: Stability,
    pub capabilities: HostCapabilities,
    pub model_id: String,
}

impl InitializeResult {
    pub fn current(runtime_version: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            runtime_version: runtime_version.into(),
            stability: Stability::NotDelivered,
            capabilities: HostCapabilities::current(),
            model_id: model_id.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum Stability {
    /// Internal implementation candidate. It is not a supported SDK surface.
    NotDelivered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct HostCapabilities {
    pub session_create: bool,
    pub session_create_lifetime: SessionLifetime,
    pub session_resume: bool,
    pub query: bool,
    pub query_cancel: bool,
    pub session_close: bool,
    pub event_stream: bool,
    pub tool_events: bool,
    pub image_input: bool,
    pub structured_output: bool,
    pub usage: bool,
    pub custom_tools: bool,
    pub permission_responses: bool,
    pub hooks: bool,
    pub mcp_configuration: bool,
    pub prestarted_transport: bool,
}

impl HostCapabilities {
    pub const fn current() -> Self {
        Self {
            session_create: true,
            session_create_lifetime: SessionLifetime::Durable,
            session_resume: true,
            query: true,
            query_cancel: true,
            session_close: true,
            event_stream: true,
            tool_events: true,
            image_input: true,
            structured_output: true,
            usage: true,
            custom_tools: false,
            permission_responses: true,
            hooks: false,
            mcp_configuration: false,
            prestarted_transport: false,
        }
    }
}

/// Persistence boundary of a Session visible through the internal Host candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum SessionLifetime {
    /// Created and deleted by this Host connection.
    Connection,
    /// Persisted by the Agent Runtime and resumable by a later Host process.
    Durable,
}

fn deserialize_optional_request_id<'de, D>(deserializer: D) -> Result<Option<RequestId>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    RequestId::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum ErrorStage {
    Protocol,
    Initialize,
    Session,
    Query,
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum RecoveryAction {
    Initialize,
    Retry,
    UpdateSdk,
    RestartHost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ErrorData {
    pub code: ErrorCode,
    pub stage: ErrorStage,
    pub retryable: bool,
    pub correlation_id: String,
    #[cfg_attr(feature = "ts", ts(optional))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    #[cfg_attr(feature = "ts", ts(optional))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<String>,
    pub outcome_certainty: OutcomeCertainty,
    #[cfg_attr(feature = "ts", ts(optional))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery: Option<RecoveryAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum OutcomeCertainty {
    NotStarted,
    Committed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionCreateParams {
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    #[serde(default)]
    pub session_name: Option<String>,
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    #[serde(default)]
    pub agent: Option<String>,
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    #[serde(default)]
    pub cwd: Option<String>,
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionResumeParams {
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SessionCreateResult {
    pub session_id: String,
    pub session_name: String,
    pub agent: String,
    pub lifetime: SessionLifetime,
    #[cfg_attr(feature = "ts", ts(optional))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[cfg_attr(feature = "ts", ts(optional))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[cfg_attr(feature = "ts", ts(optional))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_workspace_path: Option<String>,
    #[cfg_attr(feature = "ts", ts(optional))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_target: Option<SessionExecutionTarget>,
}

impl SessionCreateResult {
    pub fn from_runtime(created: AgentSessionCreateResult, lifetime: SessionLifetime) -> Self {
        Self {
            session_id: created.session_id,
            session_name: created.session_name,
            agent: created.agent_type,
            lifetime,
            workspace_path: created.workspace_path,
            workspace_id: created.workspace_id,
            project_workspace_path: created.project_workspace_path,
            execution_target: created.execution_target,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct QueryStartParams {
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<String>,
    #[cfg_attr(feature = "ts", ts(optional, type = "Record<string, unknown>"))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    #[serde(default)]
    pub session_id: Option<String>,
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    #[serde(default)]
    pub session_name: Option<String>,
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    #[serde(default)]
    pub agent: Option<String>,
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    #[serde(default)]
    pub cwd: Option<String>,
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct QueryStartResult {
    pub query_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub operation_id: String,
    pub accepted: bool,
    pub created_session: bool,
    pub session_lifetime: SessionLifetime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct QueryCancelParams {
    pub query_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct QueryCancelResult {
    pub query_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub operation_id: String,
    pub requested: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecision {
    AllowOnce,
    AllowAlways,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PermissionRespondParams {
    pub query_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub operation_id: String,
    pub request_id: String,
    pub decision: PermissionDecision,
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    #[serde(default)]
    pub feedback: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PermissionRespondResult {
    pub request_id: String,
    pub accepted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum PermissionSourceKind {
    ToolCall,
    Provider,
    Extension,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PermissionSource {
    pub kind: PermissionSourceKind,
    pub identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionCloseParams {
    pub session_id: String,
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    #[serde(default)]
    pub wait_timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SessionCloseResult {
    pub session_id: String,
    pub unloaded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct ShutdownParams {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ShutdownResult {
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct QueryEventParams {
    pub query_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub operation_id: String,
    pub sequence: u64,
    pub event: QueryEvent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum QueryEvent {
    AssistantTextDelta {
        text: String,
    },
    ToolEvent {
        tool_call_id: String,
        tool_name: String,
        status: ToolEventStatus,
        #[cfg_attr(feature = "ts", ts(optional))]
        #[serde(skip_serializing_if = "Option::is_none")]
        progress: Option<f32>,
        #[cfg_attr(feature = "ts", ts(optional))]
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
    PermissionRequest {
        request_id: String,
        action: String,
        resources: Vec<String>,
        source: PermissionSource,
        #[cfg_attr(feature = "ts", ts(optional))]
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_call_id: Option<String>,
        response_timeout_ms: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum ToolEventStatus {
    Started,
    Progress,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum QueryTerminalStatus {
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct QueryResultParams {
    pub query_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub operation_id: String,
    pub status: QueryTerminalStatus,
    pub output: QueryOutput,
    #[cfg_attr(feature = "ts", ts(optional))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<QueryUsage>,
    #[cfg_attr(feature = "ts", ts(optional))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<QueryResultError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct QueryUsage {
    pub input_tokens: usize,
    #[cfg_attr(feature = "ts", ts(optional))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<usize>,
    pub total_tokens: usize,
    #[cfg_attr(feature = "ts", ts(optional))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct QueryOutput {
    pub text: String,
    #[cfg_attr(feature = "ts", ts(optional, type = "unknown"))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
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
                operation_id: None,
                causation_id: None,
                outcome_certainty: match code {
                    ErrorCode::CleanupRequired | ErrorCode::ProcessLost => {
                        OutcomeCertainty::Unknown
                    }
                    _ => OutcomeCertainty::Committed,
                },
                recovery,
            },
        }
    }
}

fn empty_object() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

#[cfg(test)]
mod tests {
    use super::{SessionCreateResult, SessionLifetime};
    use bitfun_core_types::SessionExecutionTarget;
    use bitfun_runtime_ports::AgentSessionCreateResult;

    #[test]
    fn session_create_result_preserves_runtime_placement_facts() {
        let mut created = AgentSessionCreateResult::new("session_1", "Main", "agentic");
        created.workspace_path = Some("/worktrees/session_1".to_string());
        created.workspace_id = Some("workspace_1".to_string());
        created.project_workspace_path = Some("/workspace/project".to_string());
        created.execution_target = Some(SessionExecutionTarget::local("/worktrees/session_1"));

        let result = SessionCreateResult::from_runtime(created, SessionLifetime::Connection);
        let json = serde_json::to_value(result).expect("serialize SDK Host create result");

        assert_eq!(json["sessionId"], "session_1");
        assert_eq!(json["sessionName"], "Main");
        assert_eq!(json["agent"], "agentic");
        assert!(json.get("agentType").is_none());
        assert_eq!(json["workspacePath"], "/worktrees/session_1");
        assert_eq!(json["workspaceId"], "workspace_1");
        assert_eq!(json["projectWorkspacePath"], "/workspace/project");
        assert_eq!(json["executionTarget"]["kind"], "local");
        assert_eq!(json["lifetime"], "connection");
    }
}
