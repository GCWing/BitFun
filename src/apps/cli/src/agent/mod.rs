/// Agent integration module
///
/// Wraps interaction with bitfun-core's Agent system
pub mod core_adapter;

// Re-export AgenticSystem for use in other modules
pub use bitfun_core::agentic::system::AgenticSystem;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::session::ToolCall;

/// Agent event
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Start thinking
    Thinking,
    /// Text stream
    TextChunk(String),
    /// Tool call started
    ToolCallStart {
        tool_name: String,
        parameters: serde_json::Value,
    },
    /// Tool call in progress
    ToolCallProgress { tool_name: String, message: String },
    /// Tool call completed
    ToolCallComplete {
        tool_name: String,
        result: String,
        success: bool,
    },
    /// Done
    Done,
    /// Error
    Error(String),
}

/// Agent response
#[derive(Debug, Clone)]
pub struct AgentResponse {
    /// Tool call list
    pub tool_calls: Vec<ToolCall>,
    /// Whether successful
    pub success: bool,
}

/// Agent interface
#[async_trait::async_trait]
pub trait Agent: Send + Sync {
    /// Process user message
    async fn process_message(
        &self,
        message: String,
        event_tx: mpsc::UnboundedSender<AgentEvent>,
    ) -> Result<AgentResponse>;

    /// Get Agent name
    fn name(&self) -> &str;
}
