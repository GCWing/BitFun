/// WebSocket transport adapter
///
/// Used for Web Server version, pushes events to browser via WebSocket
use crate::traits::{TextChunk, ToolEventPayload, TransportAdapter};
use async_trait::async_trait;
use bitfun_events::AgenticEvent;
use serde_json::json;
use std::fmt;
use tokio::sync::mpsc;

/// WebSocket message type
#[derive(Debug, Clone)]
pub enum WsMessage {
    Text(String),
    Binary(Vec<u8>),
    Close,
}

/// WebSocket transport adapter
#[derive(Clone)]
pub struct WebSocketTransportAdapter {
    tx: mpsc::UnboundedSender<WsMessage>,
}

impl WebSocketTransportAdapter {
    /// Create a new WebSocket adapter
    pub fn new(tx: mpsc::UnboundedSender<WsMessage>) -> Self {
        Self { tx }
    }

    /// Send JSON message
    fn send_json(&self, value: serde_json::Value) -> anyhow::Result<()> {
        let json_str = serde_json::to_string(&value)?;
        self.tx
            .send(WsMessage::Text(json_str))
            .map_err(|e| anyhow::anyhow!("Failed to send WebSocket message: {}", e))?;
        Ok(())
    }
}

impl fmt::Debug for WebSocketTransportAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebSocketTransportAdapter")
            .field("adapter_type", &"websocket")
            .finish()
    }
}

#[async_trait]
impl TransportAdapter for WebSocketTransportAdapter {
    async fn emit_event(&self, _session_id: &str, event: AgenticEvent) -> anyhow::Result<()> {
        let message = match event {
            AgenticEvent::ImageAnalysisStarted {
                session_id,
                image_count,
                user_input,
                image_metadata,
            } => {
                json!({
                    "type": "image-analysis-started",
                    "sessionId": session_id,
                    "imageCount": image_count,
                    "userInput": user_input,
                    "imageMetadata": image_metadata,
                })
            }
            AgenticEvent::ImageAnalysisCompleted {
                session_id,
                success,
                duration_ms,
            } => {
                json!({
                    "type": "image-analysis-completed",
                    "sessionId": session_id,
                    "success": success,
                    "durationMs": duration_ms,
                })
            }
            AgenticEvent::DialogTurnStarted {
                session_id,
                turn_id,
                turn_index,
                original_user_input,
                user_message_metadata,
                subagent_parent_info,
                ..
            } => {
                json!({
                    "type": "dialog-turn-started",
                    "sessionId": session_id,
                    "turnId": turn_id,
                    "turnIndex": turn_index,
                    "originalUserInput": original_user_input,
                    "userMessageMetadata": user_message_metadata,
                    "subagentParentInfo": subagent_parent_info,
                })
            }
            AgenticEvent::ModelRoundStarted {
                session_id,
                turn_id,
                round_id,
                round_index,
                subagent_parent_info,
            } => {
                json!({
                    "type": "model-round-started",
                    "sessionId": session_id,
                    "turnId": turn_id,
                    "roundId": round_id,
                    "roundIndex": round_index,
                    "subagentParentInfo": subagent_parent_info,
                })
            }
            AgenticEvent::TextChunk {
                session_id,
                turn_id,
                round_id,
                text,
                subagent_parent_info,
            } => {
                json!({
                    "type": "text-chunk",
                    "sessionId": session_id,
                    "turnId": turn_id,
                    "roundId": round_id,
                    "text": text,
                    "subagentParentInfo": subagent_parent_info,
                })
            }
            AgenticEvent::ThinkingChunk {
                session_id,
                turn_id,
                round_id,
                content,
                is_end,
                subagent_parent_info,
            } => {
                json!({
                    "type": "text-chunk",
                    "sessionId": session_id,
                    "turnId": turn_id,
                    "roundId": round_id,
                    "text": content,
                    "contentType": "thinking",
                    "isThinkingEnd": is_end,
                    "subagentParentInfo": subagent_parent_info,
                })
            }
            AgenticEvent::ToolEvent {
                session_id,
                turn_id,
                tool_event,
                subagent_parent_info,
            } => {
                json!({
                    "type": "tool-event",
                    "sessionId": session_id,
                    "turnId": turn_id,
                    "toolEvent": tool_event,
                    "subagentParentInfo": subagent_parent_info,
                })
            }
            AgenticEvent::DialogTurnCompleted {
                session_id,
                turn_id,
                subagent_parent_info,
                ..
            } => {
                json!({
                    "type": "dialog-turn-completed",
                    "sessionId": session_id,
                    "turnId": turn_id,
                    "subagentParentInfo": subagent_parent_info,
                })
            }
            _ => return Ok(()),
        };

        self.send_json(message)?;
        Ok(())
    }

    async fn emit_text_chunk(&self, _session_id: &str, chunk: TextChunk) -> anyhow::Result<()> {
        self.send_json(json!({
            "type": "text-chunk",
            "sessionId": chunk.session_id,
            "turnId": chunk.turn_id,
            "roundId": chunk.round_id,
            "text": chunk.text,
            "timestamp": chunk.timestamp,
        }))?;
        Ok(())
    }

    async fn emit_tool_event(
        &self,
        _session_id: &str,
        event: ToolEventPayload,
    ) -> anyhow::Result<()> {
        self.send_json(json!({
            "type": "tool-event",
            "sessionId": event.session_id,
            "turnId": event.turn_id,
            "toolEvent": {
                "tool_id": event.tool_id,
                "tool_name": event.tool_name,
                "event_type": event.event_type,
                "params": event.params,
                "result": event.result,
                "error": event.error,
                "duration_ms": event.duration_ms,
            }
        }))?;
        Ok(())
    }

    async fn emit_stream_start(
        &self,
        session_id: &str,
        turn_id: &str,
        round_id: &str,
    ) -> anyhow::Result<()> {
        self.send_json(json!({
            "type": "stream-start",
            "sessionId": session_id,
            "turnId": turn_id,
            "roundId": round_id,
        }))?;
        Ok(())
    }

    async fn emit_stream_end(
        &self,
        session_id: &str,
        turn_id: &str,
        round_id: &str,
    ) -> anyhow::Result<()> {
        self.send_json(json!({
            "type": "stream-end",
            "sessionId": session_id,
            "turnId": turn_id,
            "roundId": round_id,
        }))?;
        Ok(())
    }

    async fn emit_generic(
        &self,
        event_name: &str,
        payload: serde_json::Value,
    ) -> anyhow::Result<()> {
        self.send_json(json!({
            "type": event_name,
            "payload": payload,
        }))?;
        Ok(())
    }

    fn adapter_type(&self) -> &str {
        "websocket"
    }
}
