//! Tauri transport adapter.
//!
//! This adapter owns only Tauri delivery. Agentic event names and payload
//! shapes are projected by `bitfun-events`.

#[cfg(feature = "tauri-adapter")]
use crate::traits::{TextChunk, ToolEventPayload, TransportAdapter};
use async_trait::async_trait;
use bitfun_events::{project_agentic_frontend_event, AgenticEvent};
use log::warn;
use serde_json::json;
use std::fmt;

#[cfg(feature = "tauri-adapter")]
use tauri::{AppHandle, Emitter};

#[cfg(feature = "tauri-adapter")]
pub struct TauriTransportAdapter {
    app_handle: AppHandle,
}

#[cfg(feature = "tauri-adapter")]
impl TauriTransportAdapter {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

#[cfg(feature = "tauri-adapter")]
impl fmt::Debug for TauriTransportAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TauriTransportAdapter")
            .field("adapter_type", &"tauri")
            .finish()
    }
}

#[cfg(feature = "tauri-adapter")]
#[async_trait]
impl TransportAdapter for TauriTransportAdapter {
    async fn emit_event(&self, _session_id: &str, event: AgenticEvent) -> anyhow::Result<()> {
        let Some(projected) = project_agentic_frontend_event(event) else {
            warn!("Unhandled AgenticEvent type in TauriAdapter");
            return Ok(());
        };
        self.app_handle
            .emit(projected.event_name.as_str(), projected.payload)?;
        Ok(())
    }

    async fn emit_text_chunk(&self, _session_id: &str, chunk: TextChunk) -> anyhow::Result<()> {
        self.app_handle.emit(
            "agentic://text-chunk",
            json!({
                "sessionId": chunk.session_id,
                "turnId": chunk.turn_id,
                "roundId": chunk.round_id,
                "text": chunk.text,
                "timestamp": chunk.timestamp,
            }),
        )?;
        Ok(())
    }

    async fn emit_tool_event(
        &self,
        _session_id: &str,
        event: ToolEventPayload,
    ) -> anyhow::Result<()> {
        self.app_handle.emit(
            "agentic://tool-event",
            json!({
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
            }),
        )?;
        Ok(())
    }

    async fn emit_stream_start(
        &self,
        session_id: &str,
        turn_id: &str,
        round_id: &str,
    ) -> anyhow::Result<()> {
        self.app_handle.emit(
            "agentic://stream-start",
            json!({
                "sessionId": session_id,
                "turnId": turn_id,
                "roundId": round_id,
            }),
        )?;
        Ok(())
    }

    async fn emit_stream_end(
        &self,
        session_id: &str,
        turn_id: &str,
        round_id: &str,
    ) -> anyhow::Result<()> {
        self.app_handle.emit(
            "agentic://stream-end",
            json!({
                "sessionId": session_id,
                "turnId": turn_id,
                "roundId": round_id,
            }),
        )?;
        Ok(())
    }

    async fn emit_generic(
        &self,
        event_name: &str,
        payload: serde_json::Value,
    ) -> anyhow::Result<()> {
        self.app_handle.emit(event_name, payload)?;
        Ok(())
    }

    fn adapter_type(&self) -> &str {
        "tauri"
    }
}
