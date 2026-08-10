//! ACP client runtime port.
//!
//! Core-defined boundary for the dedicated ACP tool family (`acp_control`,
//! `acp_message`, `acp_history`). The tools call these methods through the
//! coordinator-injected port while the desktop host provides the concrete
//! implementation backed by `AcpClientService`, so core keeps no dependency
//! on the ACP crate (architecture boundary).
//!
//! Every request/result is `Serialize + Deserialize` so the boundary can be
//! carried across process and workspace boundaries.

use super::{PortError, PortErrorKind, PortResult, RuntimeServicePort};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// `acp_control` action `create` request.
///
/// Starts a real external ACP client process bound to a persisted session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientCreateRequest {
    /// Registered ACP client id (for example `codex` or `claude-code`).
    pub client_id: String,
    /// Workspace path the external ACP process runs in.
    pub workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
}

/// Result of [`AcpClientPort::create_session`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientCreateResult {
    pub session_id: String,
    pub session_name: String,
    pub agent_type: String,
}

/// One registered ACP client entry from [`AcpClientPort::list_clients`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientSummary {
    pub client_id: String,
    pub name: String,
    /// Aggregated client status (wire string from the ACP service).
    pub status: String,
    pub session_count: usize,
    pub readonly: bool,
}

/// Result of [`AcpClientPort::list_clients`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientListResult {
    pub clients: Vec<AcpClientSummary>,
}

/// `acp_control` action `delete` request.
///
/// Releases the external ACP process/session bound to `session_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientReleaseRequest {
    pub session_id: String,
}

/// `acp_control` action `cancel` request.
///
/// Cancels the running dialog turn of the external ACP session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientCancelRequest {
    pub session_id: String,
}

/// `acp_message` request: forward one message to the external ACP process
/// and synchronously return its response text (true bridge, not a local
/// model consumption path).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientMessageRequest {
    pub session_id: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

/// Result of [`AcpClientPort::send_message`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientMessageResult {
    pub session_id: String,
    /// Full response text produced by the external ACP agent.
    pub response: String,
}

/// One incrementally streamed output chunk of an ACP direct message.
///
/// Mirrors the incremental events of `AcpClientService::prompt_agent_stream`
/// (the desktop implementation translates the ACP crate's stream events into
/// this boundary type), so core tools consume streaming without depending on
/// the ACP crate. `Text` chunks are part of the final response; `Thought`
/// chunks are informational only and do not contribute to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AcpClientStreamChunk {
    /// One incremental text chunk of the external agent's response.
    Text { text: String },
    /// One incremental thought chunk from the external agent.
    Thought { text: String },
    /// The external agent completed its turn.
    Completed,
    /// The external turn was cancelled.
    Cancelled,
}

/// Sink receiving [`AcpClientStreamChunk`] items while a streamed ACP message
/// runs. Unbounded so the producer never drops a chunk when the consumer is
/// temporarily slower (for example while it emits per-chunk UI events).
pub type AcpClientStreamChunkSink = mpsc::UnboundedSender<AcpClientStreamChunk>;

/// `SessionMessage` ACP direct-path request: forward one message to the
/// external ACP agent bound to an internal BitFun session.
///
/// Unlike [`AcpClientMessageRequest`] (which addresses a flow session id of
/// the shape `acp_<client_id>_<uuid>`), this request addresses the internal
/// session id of an `acp__<client_id>` session — the same session identity
/// the `acp__<client_id>__prompt` bridge tool (`AcpAgentTool`) uses, so the
/// external conversation state is shared with the delegated-turn path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientBitfunMessageRequest {
    /// Registered ACP client id (for example `codex` or `claude-code`).
    pub client_id: String,
    /// Internal BitFun session id the external ACP process is bound to.
    pub bitfun_session_id: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

/// `acp_history` request: read the persisted transcript of an ACP session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientHistoryRequest {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
}

/// One transcript entry from [`AcpClientPort::read_history`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientHistoryEntry {
    /// Message role (for example `user` or `assistant`).
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_ms: Option<u64>,
}

/// Result of [`AcpClientPort::read_history`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientHistoryResult {
    pub session_id: String,
    pub entries: Vec<AcpClientHistoryEntry>,
    #[serde(default)]
    pub truncated: bool,
}

/// ACP client runtime port.
///
/// Implementations live on the product host (desktop) and forward every call
/// to the real `AcpClientService`; core tools never touch the ACP crate.
#[async_trait]
pub trait AcpClientPort: RuntimeServicePort + std::fmt::Debug {
    /// Create a persisted ACP flow session and start the external client
    /// process for it. Implementations must roll the record back when the
    /// process start fails so no orphan record is left behind.
    async fn create_session(
        &self,
        request: AcpClientCreateRequest,
    ) -> PortResult<AcpClientCreateResult>;

    /// List registered ACP clients with their current runtime facts.
    async fn list_clients(&self) -> PortResult<AcpClientListResult>;

    /// Release the external ACP process bound to `session_id`.
    async fn release_session(&self, request: AcpClientReleaseRequest) -> PortResult<()>;

    /// Cancel the running dialog turn of the external ACP session.
    async fn cancel_session(&self, request: AcpClientCancelRequest) -> PortResult<()>;

    /// Forward one message through the real channel and return the external
    /// response synchronously.
    async fn send_message(
        &self,
        request: AcpClientMessageRequest,
    ) -> PortResult<AcpClientMessageResult>;

    /// Forward one message through the real channel and stream the external
    /// response incrementally. Text chunks are pushed into `chunk_sink` as
    /// they arrive; the returned result still carries the full response text
    /// (including text that may have been emitted before an early error).
    async fn send_message_stream(
        &self,
        request: AcpClientMessageRequest,
        chunk_sink: AcpClientStreamChunkSink,
    ) -> PortResult<AcpClientMessageResult>;

    /// Forward one message to the external ACP agent bound to an internal
    /// BitFun session (`acp__<client_id>` session) and return the external
    /// response synchronously. This is the `SessionMessage` direct path: no
    /// local model turn is involved, only the port call.
    async fn send_message_to_bitfun_session(
        &self,
        request: AcpClientBitfunMessageRequest,
    ) -> PortResult<AcpClientMessageResult>;

    /// Streaming variant of [`AcpClientPort::send_message_to_bitfun_session`]:
    /// text chunks are pushed into `chunk_sink` as they arrive while the
    /// returned result still carries the full response text.
    async fn send_message_to_bitfun_session_stream(
        &self,
        request: AcpClientBitfunMessageRequest,
        chunk_sink: AcpClientStreamChunkSink,
    ) -> PortResult<AcpClientMessageResult>;

    /// Delete a temporary ACP session: release the external process (if one is
    /// live) and remove the persisted flow-session record for `session_id`.
    /// Used to recycle one-shot (`persistent=false`) ACP sessions created by
    /// the Task tool.
    ///
    /// `workspace_path` is required to resolve the persisted record.
    /// Implementations must reject a `None`/empty value with `InvalidRequest`
    /// rather than silently releasing the process without deleting the record
    /// (a release-only cleanup would leave an orphan record that keeps the
    /// recycled session appearing in listings). Idempotent so a session with
    /// no live process or record is a no-op success.
    async fn delete_session_record(
        &self,
        session_id: String,
        workspace_path: Option<String>,
    ) -> PortResult<()>;

    /// Read the persisted transcript of an ACP session.
    async fn read_history(
        &self,
        request: AcpClientHistoryRequest,
    ) -> PortResult<AcpClientHistoryResult>;
}

/// Error helper: wrap an implementation failure as a backend `PortError`.
pub fn acp_backend_error(message: impl Into<String>) -> PortError {
    PortError::new(PortErrorKind::Backend, message)
}

/// Dependency-free canonical uuid shape guard for flow-session ids.
///
/// ACP flow session ids have the shape `acp_<client_id>_<uuid>`; the trailing
/// segment must be a canonical uuid (length 36, dashed 8-4-4-4-12, hex) so an
/// internal session id that merely starts with `acp_` is never mistaken for a
/// flow session, and an empty client id (`acp__<uuid>`) is rejected.
///
/// Single authoritative implementation (d3-P2-2): the desktop `AcpClientPort`,
/// `SessionMessage` direct-path tool and the Task ACP flow branch all share
/// this guard so the flow-session判定 can never drift between layers.
pub fn looks_like_uuid(segment: &str) -> bool {
    segment.len() == 36
        && segment.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

/// Parse the ACP client id out of a flow session id of the shape
/// `acp_<client_id>_<uuid>`. Returns `None` for any other id shape (including
/// an empty client id). Single authoritative implementation (d3-P2-2).
pub fn acp_flow_client_id_from_session_id(session_id: &str) -> Option<String> {
    let rest = session_id.strip_prefix("acp_")?;
    let (client_id, uuid_segment) = rest.rsplit_once('_')?;
    if client_id.is_empty() || !looks_like_uuid(uuid_segment) {
        return None;
    }
    Some(client_id.to_string())
}

#[cfg(test)]
mod acp_flow_id_tests {
    use super::{acp_flow_client_id_from_session_id, looks_like_uuid};

    #[test]
    fn looks_like_uuid_accepts_only_canonical_shape() {
        assert!(looks_like_uuid("7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b"));
        assert!(!looks_like_uuid("7f0e1a2b3c4d4e5f8a9b0c1d2e3f4a5b"));
        assert!(!looks_like_uuid("7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b-extra"));
        assert!(!looks_like_uuid(""));
        assert!(!looks_like_uuid("7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5"));
    }

    #[test]
    fn acp_flow_client_id_parses_from_flow_session_id() {
        assert_eq!(
            acp_flow_client_id_from_session_id("acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b")
                .as_deref(),
            Some("codex")
        );
        assert_eq!(
            acp_flow_client_id_from_session_id("acp_claude-code_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b")
                .as_deref(),
            Some("claude-code")
        );
    }

    #[test]
    fn acp_flow_client_id_rejects_non_flow_shapes() {
        // 非 acp 前缀
        assert_eq!(acp_flow_client_id_from_session_id("session-123"), None);
        // 前缀但无 uuid 尾段
        assert_eq!(acp_flow_client_id_from_session_id("acp_codebuddy"), None);
        // 空 client id（acp__<uuid>）
        assert_eq!(
            acp_flow_client_id_from_session_id("acp__7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b"),
            None
        );
        // 空串
        assert_eq!(acp_flow_client_id_from_session_id(""), None);
    }
}
