/// WebSocket transport adapter.
///
/// Used for Web Server delivery. Agentic event payload projection is owned by
/// `bitfun-events`; this adapter only serializes the allowed legacy WebSocket
/// event set to text messages.
use crate::traits::{TextChunk, ToolEventPayload, TransportAdapter};
use async_trait::async_trait;
use bitfun_events::{
    is_legacy_websocket_agentic_event_type, project_agentic_frontend_event, AgenticEvent,
};
use serde_json::json;
use std::fmt;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum WsMessage {
    Text(String),
    Binary(Vec<u8>),
    Close,
}

#[derive(Clone)]
pub struct WebSocketTransportAdapter {
    tx: mpsc::UnboundedSender<WsMessage>,
}

impl WebSocketTransportAdapter {
    pub fn new(tx: mpsc::UnboundedSender<WsMessage>) -> Self {
        Self { tx }
    }

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
        let Some(projected) = project_agentic_frontend_event(event) else {
            return Ok(());
        };
        if !is_legacy_websocket_agentic_event_type(&projected.event_type) {
            return Ok(());
        }
        self.send_json(projected.legacy_flat_message())?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_events::AgenticEvent as Event;

    #[tokio::test]
    async fn websocket_uses_shared_agentic_frontend_projection() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let adapter = WebSocketTransportAdapter::new(tx);

        adapter
            .emit_event(
                "session-1",
                Event::TextChunk {
                    session_id: "session-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    round_id: "round-1".to_string(),
                    attempt_id: Some("attempt-1".to_string()),
                    attempt_index: Some(1),
                    text: "hello".to_string(),
                },
            )
            .await
            .expect("emit");

        let WsMessage::Text(message) = rx.recv().await.expect("message") else {
            panic!("expected text message");
        };
        let value: serde_json::Value = serde_json::from_str(&message).expect("json");
        assert_eq!(value["type"], "text-chunk");
        assert_eq!(value["sessionId"], "session-1");
        assert_eq!(value["attemptId"], "attempt-1");
        assert_eq!(value["text"], "hello");
    }

    #[tokio::test]
    async fn websocket_keeps_legacy_agentic_event_allowlist() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let adapter = WebSocketTransportAdapter::new(tx);

        adapter
            .emit_event(
                "session-1",
                Event::SessionDeleted {
                    session_id: "session-1".to_string(),
                },
            )
            .await
            .expect("emit");

        assert!(rx.try_recv().is_err());
    }
}
