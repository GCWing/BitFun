/// CLI transport adapter
///
/// Uses tokio::mpsc channel to send events to CLI TUI renderer
use crate::traits::{TextChunk, ToolEventPayload, TransportAdapter};
use async_trait::async_trait;
use bitfun_events::AgenticEvent;
use serde::{Deserialize, Serialize};
use std::fmt;
use tokio::sync::mpsc;

/// CLI event type (for TUI rendering)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CliEvent {
    TextChunk(TextChunk),
    ToolEvent(ToolEventPayload),
    StreamStart {
        session_id: String,
        turn_id: String,
        round_id: String,
    },
    StreamEnd {
        session_id: String,
        turn_id: String,
        round_id: String,
    },
    DialogTurnStarted {
        session_id: String,
        turn_id: String,
    },
    DialogTurnCompleted {
        session_id: String,
        turn_id: String,
    },
    /// Generic event (for LSP, file watch, etc.)
    Generic {
        event_name: String,
        payload: serde_json::Value,
    },
}

/// CLI transport adapter
#[derive(Clone)]
pub struct CliTransportAdapter {
    tx: mpsc::UnboundedSender<CliEvent>,
}

impl CliTransportAdapter {
    /// Create a new CLI adapter
    pub fn new(tx: mpsc::UnboundedSender<CliEvent>) -> Self {
        Self { tx }
    }

    /// Create channel and get receiver (for creating TUI renderer)
    pub fn create_channel() -> (Self, mpsc::UnboundedReceiver<CliEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self::new(tx), rx)
    }
}

impl fmt::Debug for CliTransportAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CliTransportAdapter")
            .field("adapter_type", &"cli")
            .finish()
    }
}

#[async_trait]
impl TransportAdapter for CliTransportAdapter {
    async fn emit_event(&self, _session_id: &str, event: AgenticEvent) -> anyhow::Result<()> {
        let cli_event = match event {
            AgenticEvent::TextChunk {
                session_id,
                turn_id,
                round_id,
                text,
                ..
            } => CliEvent::TextChunk(TextChunk {
                session_id,
                turn_id,
                round_id,
                text,
                timestamp: chrono::Utc::now().timestamp_millis(),
            }),
            AgenticEvent::DialogTurnStarted {
                session_id,
                turn_id,
                ..
            } => CliEvent::DialogTurnStarted {
                session_id,
                turn_id,
            },
            AgenticEvent::DialogTurnCompleted {
                session_id,
                turn_id,
                ..
            } => CliEvent::DialogTurnCompleted {
                session_id,
                turn_id,
            },
            _ => return Ok(()),
        };

        self.tx
            .send(cli_event)
            .map_err(|e| anyhow::anyhow!("Failed to send CLI event: {}", e))?;

        Ok(())
    }

    async fn emit_text_chunk(&self, _session_id: &str, chunk: TextChunk) -> anyhow::Result<()> {
        self.tx
            .send(CliEvent::TextChunk(chunk))
            .map_err(|e| anyhow::anyhow!("Failed to send text chunk: {}", e))?;
        Ok(())
    }

    async fn emit_tool_event(
        &self,
        _session_id: &str,
        event: ToolEventPayload,
    ) -> anyhow::Result<()> {
        self.tx
            .send(CliEvent::ToolEvent(event))
            .map_err(|e| anyhow::anyhow!("Failed to send tool event: {}", e))?;
        Ok(())
    }

    async fn emit_stream_start(
        &self,
        session_id: &str,
        turn_id: &str,
        round_id: &str,
    ) -> anyhow::Result<()> {
        self.tx
            .send(CliEvent::StreamStart {
                session_id: session_id.to_string(),
                turn_id: turn_id.to_string(),
                round_id: round_id.to_string(),
            })
            .map_err(|e| anyhow::anyhow!("Failed to send stream start: {}", e))?;
        Ok(())
    }

    async fn emit_stream_end(
        &self,
        session_id: &str,
        turn_id: &str,
        round_id: &str,
    ) -> anyhow::Result<()> {
        self.tx
            .send(CliEvent::StreamEnd {
                session_id: session_id.to_string(),
                turn_id: turn_id.to_string(),
                round_id: round_id.to_string(),
            })
            .map_err(|e| anyhow::anyhow!("Failed to send stream end: {}", e))?;
        Ok(())
    }

    async fn emit_generic(
        &self,
        event_name: &str,
        payload: serde_json::Value,
    ) -> anyhow::Result<()> {
        self.tx
            .send(CliEvent::Generic {
                event_name: event_name.to_string(),
                payload,
            })
            .map_err(|e| anyhow::anyhow!("Failed to send generic event: {}", e))?;
        Ok(())
    }

    fn adapter_type(&self) -> &str {
        "cli"
    }
}
