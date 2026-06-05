/// Transport Layer - Cross-platform communication traits
///
/// This module defines unified interfaces for cross-platform communication, supports:
/// - CLI (tokio::mpsc channels)
/// - Tauri (app.emit events)
/// - WebSocket/SSE (web server)
use async_trait::async_trait;
use bitfun_events::AgenticEvent;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Transport adapter trait - All platforms must implement this interface
#[async_trait]
pub trait TransportAdapter: Send + Sync + Debug {
    /// Emit agentic event to frontend
    async fn emit_event(&self, session_id: &str, event: AgenticEvent) -> anyhow::Result<()>;

    /// Emit text chunk (streaming output)
    async fn emit_text_chunk(&self, session_id: &str, chunk: TextChunk) -> anyhow::Result<()>;

    /// Emit tool event
    async fn emit_tool_event(
        &self,
        session_id: &str,
        event: ToolEventPayload,
    ) -> anyhow::Result<()>;

    /// Emit stream start event
    async fn emit_stream_start(
        &self,
        session_id: &str,
        turn_id: &str,
        round_id: &str,
    ) -> anyhow::Result<()>;

    /// Emit stream end event
    async fn emit_stream_end(
        &self,
        session_id: &str,
        turn_id: &str,
        round_id: &str,
    ) -> anyhow::Result<()>;

    /// Emit generic event (supports any event type)
    async fn emit_generic(
        &self,
        event_name: &str,
        payload: serde_json::Value,
    ) -> anyhow::Result<()>;

    /// Get adapter type name
    fn adapter_type(&self) -> &str;
}

/// Text chunk data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextChunk {
    pub session_id: String,
    pub turn_id: String,
    pub round_id: String,
    pub text: String,
    pub timestamp: i64,
}

/// Tool event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEventPayload {
    pub session_id: String,
    pub turn_id: String,
    pub tool_id: String,
    pub tool_name: String,
    pub event_type: ToolEventType,
    pub params: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: Option<u64>,
}

/// Tool event type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolEventType {
    Started,
    EarlyDetected,
    ParamsPartial,
    Completed,
    Failed,
    Progress,
    StreamChunk,
    ConfirmationNeeded,
}

/// Stream event wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    pub session_id: String,
    pub turn_id: String,
    pub round_id: String,
    pub event_type: String,
    pub payload: serde_json::Value,
}
