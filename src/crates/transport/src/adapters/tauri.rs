//! Tauri transport adapter
//!
//! Uses Tauri's app.emit() system to send events to frontend
//! Maintains compatibility with current implementation

#[cfg(feature = "tauri-adapter")]
use crate::traits::{TextChunk, ToolEventPayload, TransportAdapter};
use async_trait::async_trait;
use bitfun_events::AgenticEvent;
use log::warn;
use serde_json::json;
use std::fmt;

#[cfg(feature = "tauri-adapter")]
use tauri::{AppHandle, Emitter};

/// Tauri transport adapter
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
        match event {
            AgenticEvent::SessionCreated {
                session_id,
                session_name,
                agent_type,
                workspace_path,
            } => {
                self.app_handle.emit(
                    "agentic://session-created",
                    json!({
                        "sessionId": session_id,
                        "sessionName": session_name,
                        "agentType": agent_type,
                        "workspacePath": workspace_path,
                    }),
                )?;
            }
            AgenticEvent::SessionDeleted { session_id } => {
                self.app_handle.emit(
                    "agentic://session-deleted",
                    json!({
                        "sessionId": session_id,
                    }),
                )?;
            }
            AgenticEvent::ImageAnalysisStarted {
                session_id,
                image_count,
                user_input,
                image_metadata,
            } => {
                self.app_handle.emit(
                    "agentic://image-analysis-started",
                    json!({
                        "sessionId": session_id,
                        "imageCount": image_count,
                        "userInput": user_input,
                        "imageMetadata": image_metadata,
                    }),
                )?;
            }
            AgenticEvent::ImageAnalysisCompleted {
                session_id,
                success,
                duration_ms,
            } => {
                self.app_handle.emit(
                    "agentic://image-analysis-completed",
                    json!({
                        "sessionId": session_id,
                        "success": success,
                        "durationMs": duration_ms,
                    }),
                )?;
            }
            AgenticEvent::DialogTurnStarted {
                session_id,
                turn_id,
                turn_index,
                user_input,
                original_user_input,
                user_message_metadata,
                subagent_parent_info,
            } => {
                self.app_handle.emit(
                    "agentic://dialog-turn-started",
                    json!({
                        "sessionId": session_id,
                        "turnId": turn_id,
                        "turnIndex": turn_index,
                        "userInput": user_input,
                        "originalUserInput": original_user_input,
                        "userMessageMetadata": user_message_metadata,
                        "subagentParentInfo": subagent_parent_info,
                    }),
                )?;
            }
            AgenticEvent::ModelRoundStarted {
                session_id,
                turn_id,
                round_id,
                round_index,
                subagent_parent_info,
            } => {
                self.app_handle.emit(
                    "agentic://model-round-started",
                    json!({
                        "sessionId": session_id,
                        "turnId": turn_id,
                        "roundId": round_id,
                        "roundIndex": round_index,
                        "subagentParentInfo": subagent_parent_info,
                    }),
                )?;
            }
            AgenticEvent::TextChunk {
                session_id,
                turn_id,
                round_id,
                text,
                subagent_parent_info,
            } => {
                self.app_handle.emit(
                    "agentic://text-chunk",
                    json!({
                        "sessionId": session_id,
                        "turnId": turn_id,
                        "roundId": round_id,
                        "text": text,
                        "subagentParentInfo": subagent_parent_info,
                    }),
                )?;
            }
            AgenticEvent::ThinkingChunk {
                session_id,
                turn_id,
                round_id,
                content,
                is_end,
                subagent_parent_info,
            } => {
                self.app_handle.emit(
                    "agentic://text-chunk",
                    json!({
                        "sessionId": session_id,
                        "turnId": turn_id,
                        "roundId": round_id,
                        "text": content,
                        "contentType": "thinking",
                        "isThinkingEnd": is_end,
                        "subagentParentInfo": subagent_parent_info,
                    }),
                )?;
            }
            AgenticEvent::ToolEvent {
                session_id,
                turn_id,
                tool_event,
                subagent_parent_info,
            } => {
                self.app_handle.emit(
                    "agentic://tool-event",
                    json!({
                        "sessionId": session_id,
                        "turnId": turn_id,
                        "toolEvent": tool_event,
                        "subagentParentInfo": subagent_parent_info,
                    }),
                )?;
            }
            AgenticEvent::DialogTurnCompleted {
                session_id,
                turn_id,
                subagent_parent_info,
                ..
            } => {
                self.app_handle.emit(
                    "agentic://dialog-turn-completed",
                    json!({
                        "sessionId": session_id,
                        "turnId": turn_id,
                        "subagentParentInfo": subagent_parent_info,
                    }),
                )?;
            }
            AgenticEvent::SessionTitleGenerated {
                session_id,
                title,
                method,
            } => {
                self.app_handle.emit(
                    "session_title_generated",
                    json!({
                        "sessionId": session_id,
                        "title": title,
                        "method": method,
                        "timestamp": chrono::Utc::now().timestamp_millis(),
                    }),
                )?;
            }
            AgenticEvent::DialogTurnCancelled {
                session_id,
                turn_id,
                subagent_parent_info,
            } => {
                self.app_handle.emit(
                    "agentic://dialog-turn-cancelled",
                    json!({
                        "sessionId": session_id,
                        "turnId": turn_id,
                        "subagentParentInfo": subagent_parent_info,
                    }),
                )?;
            }
            AgenticEvent::DialogTurnFailed {
                session_id,
                turn_id,
                error,
                error_category,
                error_detail,
                subagent_parent_info,
            } => {
                self.app_handle.emit(
                    "agentic://dialog-turn-failed",
                    json!({
                        "sessionId": session_id,
                        "turnId": turn_id,
                        "error": error,
                        "errorCategory": error_category,
                        "errorDetail": error_detail,
                        "subagentParentInfo": subagent_parent_info,
                    }),
                )?;
            }
            AgenticEvent::TokenUsageUpdated {
                session_id,
                turn_id,
                model_id,
                input_tokens,
                output_tokens,
                total_tokens,
                max_context_tokens,
                is_subagent,
            } => {
                self.app_handle.emit(
                    "agentic://token-usage-updated",
                    json!({
                        "sessionId": session_id,
                        "turnId": turn_id,
                        "modelId": model_id,
                        "inputTokens": input_tokens,
                        "outputTokens": output_tokens,
                        "totalTokens": total_tokens,
                        "maxContextTokens": max_context_tokens,
                        "isSubagent": is_subagent,
                    }),
                )?;
            }
            AgenticEvent::ContextCompressionStarted {
                session_id,
                turn_id,
                subagent_parent_info,
                compression_id,
                trigger,
                tokens_before,
                context_window,
                threshold,
            } => {
                self.app_handle.emit(
                    "agentic://context-compression-started",
                    json!({
                        "sessionId": session_id,
                        "turnId": turn_id,
                        "compressionId": compression_id,
                        "trigger": trigger,
                        "tokensBefore": tokens_before,
                        "contextWindow": context_window,
                        "threshold": threshold,
                        "subagentParentInfo": subagent_parent_info,
                    }),
                )?;
            }
            AgenticEvent::ContextCompressionCompleted {
                session_id,
                turn_id,
                subagent_parent_info,
                compression_id,
                compression_count,
                tokens_before,
                tokens_after,
                compression_ratio,
                duration_ms,
                has_summary,
                summary_source,
            } => {
                self.app_handle.emit(
                    "agentic://context-compression-completed",
                    json!({
                        "sessionId": session_id,
                        "turnId": turn_id,
                        "compressionId": compression_id,
                        "compressionCount": compression_count,
                        "tokensBefore": tokens_before,
                        "tokensAfter": tokens_after,
                        "compressionRatio": compression_ratio,
                        "durationMs": duration_ms,
                        "hasSummary": has_summary,
                        "summarySource": summary_source,
                        "subagentParentInfo": subagent_parent_info,
                    }),
                )?;
            }
            AgenticEvent::ContextCompressionFailed {
                session_id,
                turn_id,
                subagent_parent_info,
                compression_id,
                error,
            } => {
                self.app_handle.emit(
                    "agentic://context-compression-failed",
                    json!({
                        "sessionId": session_id,
                        "turnId": turn_id,
                        "compressionId": compression_id,
                        "error": error,
                        "subagentParentInfo": subagent_parent_info,
                    }),
                )?;
            }
            AgenticEvent::SessionStateChanged {
                session_id,
                new_state,
            } => {
                self.app_handle.emit(
                    "agentic://session-state-changed",
                    json!({
                        "sessionId": session_id,
                        "newState": new_state,
                    }),
                )?;
            }
            AgenticEvent::SessionModelAutoMigrated {
                session_id,
                previous_model_id,
                new_model_id,
                reason,
            } => {
                self.app_handle.emit(
                    "agentic://session-model-auto-migrated",
                    json!({
                        "sessionId": session_id,
                        "previousModelId": previous_model_id,
                        "newModelId": new_model_id,
                        "reason": reason,
                    }),
                )?;
            }
            AgenticEvent::ModelRoundCompleted {
                session_id,
                turn_id,
                round_id,
                has_tool_calls,
                subagent_parent_info,
            } => {
                self.app_handle.emit(
                    "agentic://model-round-completed",
                    json!({
                        "sessionId": session_id,
                        "turnId": turn_id,
                        "roundId": round_id,
                        "hasToolCalls": has_tool_calls,
                        "subagentParentInfo": subagent_parent_info,
                    }),
                )?;
            }
            _ => {
                warn!("Unhandled AgenticEvent type in TauriAdapter");
            }
        }
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
