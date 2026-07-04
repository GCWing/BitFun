//! Framework-neutral projection for product-facing agentic events.
//!
//! This module owns the stable event name and payload shape consumed by host
//! transports. Concrete delivery adapters should only emit the projected
//! envelope.

use crate::AgenticEvent;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgenticFrontendEvent {
    pub event_name: String,
    pub event_type: String,
    pub payload: Value,
}

impl AgenticFrontendEvent {
    pub fn new(
        event_name: impl Into<String>,
        event_type: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            event_name: event_name.into(),
            event_type: event_type.into(),
            payload,
        }
    }

    pub fn legacy_flat_message(&self) -> Value {
        let mut message = Map::new();
        match &self.payload {
            Value::Object(payload) => {
                for (key, value) in payload {
                    message.insert(key.clone(), value.clone());
                }
            }
            payload => {
                message.insert("payload".to_string(), payload.clone());
            }
        }
        if self.event_type == "dialog-turn-started" {
            message.remove("userInput");
        }
        message.insert("type".to_string(), Value::String(self.event_type.clone()));
        Value::Object(message)
    }
}

pub fn project_agentic_frontend_event(event: AgenticEvent) -> Option<AgenticFrontendEvent> {
    match event {
        AgenticEvent::SessionCreated {
            session_id,
            session_name,
            agent_type,
            workspace_path,
            remote_connection_id,
            remote_ssh_host,
        } => Some(AgenticFrontendEvent::new(
            "agentic://session-created",
            "session-created",
            json!({
                "sessionId": session_id,
                "sessionName": session_name,
                "agentType": agent_type,
                "workspacePath": workspace_path,
                "remoteConnectionId": remote_connection_id,
                "remoteSshHost": remote_ssh_host,
            }),
        )),
        AgenticEvent::SessionDeleted { session_id } => Some(AgenticFrontendEvent::new(
            "agentic://session-deleted",
            "session-deleted",
            json!({ "sessionId": session_id }),
        )),
        AgenticEvent::ImageAnalysisStarted {
            session_id,
            image_count,
            user_input,
            image_metadata,
        } => Some(AgenticFrontendEvent::new(
            "agentic://image-analysis-started",
            "image-analysis-started",
            json!({
                "sessionId": session_id,
                "imageCount": image_count,
                "userInput": user_input,
                "imageMetadata": image_metadata,
            }),
        )),
        AgenticEvent::ImageAnalysisCompleted {
            session_id,
            success,
            duration_ms,
        } => Some(AgenticFrontendEvent::new(
            "agentic://image-analysis-completed",
            "image-analysis-completed",
            json!({
                "sessionId": session_id,
                "success": success,
                "durationMs": duration_ms,
            }),
        )),
        AgenticEvent::DialogTurnStarted {
            session_id,
            turn_id,
            turn_index,
            user_input,
            original_user_input,
            user_message_metadata,
        } => Some(AgenticFrontendEvent::new(
            "agentic://dialog-turn-started",
            "dialog-turn-started",
            json!({
                "sessionId": session_id,
                "turnId": turn_id,
                "turnIndex": turn_index,
                "userInput": user_input,
                "originalUserInput": original_user_input,
                "userMessageMetadata": user_message_metadata,
            }),
        )),
        AgenticEvent::SubagentSessionLinked {
            session_id,
            parent_session_id,
            parent_dialog_turn_id,
            parent_tool_call_id,
            agent_type,
        } => Some(AgenticFrontendEvent::new(
            "agentic://subagent-session-linked",
            "subagent-session-linked",
            json!({
                "sessionId": session_id,
                "parentSessionId": parent_session_id,
                "parentDialogTurnId": parent_dialog_turn_id,
                "parentToolCallId": parent_tool_call_id,
                "agentType": agent_type,
            }),
        )),
        AgenticEvent::ModelRoundStarted {
            session_id,
            turn_id,
            round_id,
            round_group_id,
            round_index,
            model_id,
        } => Some(AgenticFrontendEvent::new(
            "agentic://model-round-started",
            "model-round-started",
            json!({
                "sessionId": session_id,
                "turnId": turn_id,
                "roundId": round_id,
                "roundGroupId": round_group_id,
                "roundIndex": round_index,
                "modelId": model_id,
            }),
        )),
        AgenticEvent::TextChunk {
            session_id,
            turn_id,
            round_id,
            attempt_id,
            attempt_index,
            text,
        } => Some(AgenticFrontendEvent::new(
            "agentic://text-chunk",
            "text-chunk",
            json!({
                "sessionId": session_id,
                "turnId": turn_id,
                "roundId": round_id,
                "attemptId": attempt_id,
                "attemptIndex": attempt_index,
                "text": text,
            }),
        )),
        AgenticEvent::ThinkingChunk {
            session_id,
            turn_id,
            round_id,
            attempt_id,
            attempt_index,
            content,
            is_end,
        } => Some(AgenticFrontendEvent::new(
            "agentic://text-chunk",
            "text-chunk",
            json!({
                "sessionId": session_id,
                "turnId": turn_id,
                "roundId": round_id,
                "attemptId": attempt_id,
                "attemptIndex": attempt_index,
                "text": content,
                "contentType": "thinking",
                "isThinkingEnd": is_end,
            }),
        )),
        AgenticEvent::ToolEvent {
            session_id,
            turn_id,
            round_id,
            attempt_id,
            attempt_index,
            tool_event,
        } => Some(AgenticFrontendEvent::new(
            "agentic://tool-event",
            "tool-event",
            json!({
                "sessionId": session_id,
                "turnId": turn_id,
                "roundId": round_id,
                "attemptId": attempt_id,
                "attemptIndex": attempt_index,
                "toolEvent": tool_event,
            }),
        )),
        AgenticEvent::DialogTurnCompleted {
            session_id,
            turn_id,
            partial_recovery_reason,
            success,
            finish_reason,
            has_final_response,
            ..
        } => Some(AgenticFrontendEvent::new(
            "agentic://dialog-turn-completed",
            "dialog-turn-completed",
            json!({
                "sessionId": session_id,
                "turnId": turn_id,
                "partialRecoveryReason": partial_recovery_reason,
                "success": success,
                "finishReason": finish_reason,
                "hasFinalResponse": has_final_response,
            }),
        )),
        AgenticEvent::SessionTitleGenerated {
            session_id,
            title,
            method,
        } => Some(AgenticFrontendEvent::new(
            "session_title_generated",
            "session_title_generated",
            json!({
                "sessionId": session_id,
                "title": title,
                "method": method,
                "timestamp": chrono::Utc::now().timestamp_millis(),
            }),
        )),
        AgenticEvent::DialogTurnCancelled {
            session_id,
            turn_id,
        } => Some(AgenticFrontendEvent::new(
            "agentic://dialog-turn-cancelled",
            "dialog-turn-cancelled",
            json!({
                "sessionId": session_id,
                "turnId": turn_id,
            }),
        )),
        AgenticEvent::DialogTurnFailed {
            session_id,
            turn_id,
            error,
            error_category,
            error_detail,
        } => Some(AgenticFrontendEvent::new(
            "agentic://dialog-turn-failed",
            "dialog-turn-failed",
            json!({
                "sessionId": session_id,
                "turnId": turn_id,
                "error": error,
                "errorCategory": error_category,
                "errorDetail": error_detail,
            }),
        )),
        AgenticEvent::TokenUsageUpdated {
            session_id,
            turn_id,
            model_id,
            input_tokens,
            output_tokens,
            total_tokens,
            max_context_tokens,
            is_subagent,
            cached_tokens,
            token_details,
        } => Some(AgenticFrontendEvent::new(
            "agentic://token-usage-updated",
            "token-usage-updated",
            json!({
                "sessionId": session_id,
                "turnId": turn_id,
                "modelId": model_id,
                "inputTokens": input_tokens,
                "outputTokens": output_tokens,
                "totalTokens": total_tokens,
                "maxContextTokens": max_context_tokens,
                "isSubagent": is_subagent,
                "cachedTokens": cached_tokens,
                "tokenDetails": token_details,
            }),
        )),
        AgenticEvent::ContextCompressionStarted {
            session_id,
            turn_id,
            compression_id,
            trigger,
            tokens_before,
            context_window,
            threshold,
        } => Some(AgenticFrontendEvent::new(
            "agentic://context-compression-started",
            "context-compression-started",
            json!({
                "sessionId": session_id,
                "turnId": turn_id,
                "compressionId": compression_id,
                "trigger": trigger,
                "tokensBefore": tokens_before,
                "contextWindow": context_window,
                "threshold": threshold,
            }),
        )),
        AgenticEvent::ContextCompressionCompleted {
            session_id,
            turn_id,
            compression_id,
            compression_count,
            tokens_before,
            tokens_after,
            compression_ratio,
            duration_ms,
            has_summary,
            summary_source,
        } => Some(AgenticFrontendEvent::new(
            "agentic://context-compression-completed",
            "context-compression-completed",
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
            }),
        )),
        AgenticEvent::ContextCompressionFailed {
            session_id,
            turn_id,
            compression_id,
            error,
        } => Some(AgenticFrontendEvent::new(
            "agentic://context-compression-failed",
            "context-compression-failed",
            json!({
                "sessionId": session_id,
                "turnId": turn_id,
                "compressionId": compression_id,
                "error": error,
            }),
        )),
        AgenticEvent::ThreadGoalUpdated { session_id, goal } => Some(AgenticFrontendEvent::new(
            "agentic://thread-goal-updated",
            "thread-goal-updated",
            json!({
                "sessionId": session_id,
                "goal": goal,
            }),
        )),
        AgenticEvent::SessionStateChanged {
            session_id,
            new_state,
        } => Some(AgenticFrontendEvent::new(
            "agentic://session-state-changed",
            "session-state-changed",
            json!({
                "sessionId": session_id,
                "newState": new_state,
            }),
        )),
        AgenticEvent::SessionModelAutoMigrated {
            session_id,
            previous_model_id,
            new_model_id,
            reason,
        } => Some(AgenticFrontendEvent::new(
            "agentic://session-model-auto-migrated",
            "session-model-auto-migrated",
            json!({
                "sessionId": session_id,
                "previousModelId": previous_model_id,
                "newModelId": new_model_id,
                "reason": reason,
            }),
        )),
        AgenticEvent::DeepReviewQueueStateChanged {
            session_id,
            turn_id,
            queue_state,
        } => Some(AgenticFrontendEvent::new(
            "agentic://deep-review-queue-state-changed",
            "deep-review-queue-state-changed",
            json!({
                "sessionId": session_id,
                "turnId": turn_id,
                "queueState": {
                    "toolId": queue_state.tool_id,
                    "subagentType": queue_state.subagent_type,
                    "status": queue_state.status,
                    "reason": queue_state.reason,
                    "queuedReviewerCount": queue_state.queued_reviewer_count,
                    "activeReviewerCount": queue_state.active_reviewer_count,
                    "effectiveParallelInstances": queue_state.effective_parallel_instances,
                    "optionalReviewerCount": queue_state.optional_reviewer_count,
                    "queueElapsedMs": queue_state.queue_elapsed_ms,
                    "runElapsedMs": queue_state.run_elapsed_ms,
                    "maxQueueWaitSeconds": queue_state.max_queue_wait_seconds,
                    "sessionConcurrencyHigh": queue_state.session_concurrency_high,
                },
            }),
        )),
        AgenticEvent::ModelRoundCompleted {
            session_id,
            turn_id,
            round_id,
            has_tool_calls,
            duration_ms,
            provider_id,
            model_id,
            model_alias,
            first_chunk_ms,
            first_visible_output_ms,
            stream_duration_ms,
            attempt_count,
            failure_category,
            token_details,
        } => Some(AgenticFrontendEvent::new(
            "agentic://model-round-completed",
            "model-round-completed",
            json!({
                "sessionId": session_id,
                "turnId": turn_id,
                "roundId": round_id,
                "hasToolCalls": has_tool_calls,
                "durationMs": duration_ms,
                "providerId": provider_id,
                "modelId": model_id,
                "modelAlias": model_alias,
                "firstChunkMs": first_chunk_ms,
                "firstVisibleOutputMs": first_visible_output_ms,
                "streamDurationMs": stream_duration_ms,
                "attemptCount": attempt_count,
                "failureCategory": failure_category,
                "tokenDetails": token_details,
            }),
        )),
        AgenticEvent::UserSteeringInjected {
            session_id,
            turn_id,
            round_index,
            steering_id,
            content,
            display_content,
        } => Some(AgenticFrontendEvent::new(
            "agentic://user-steering-injected",
            "user-steering-injected",
            json!({
                "sessionId": session_id,
                "turnId": turn_id,
                "roundIndex": round_index,
                "steeringId": steering_id,
                "content": content,
                "displayContent": display_content,
            }),
        )),
        AgenticEvent::SystemError { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeepReviewQueueReason, DeepReviewQueueState, DeepReviewQueueStatus};

    #[test]
    fn thinking_chunk_projects_to_legacy_text_chunk_event() {
        let projected = project_agentic_frontend_event(AgenticEvent::ThinkingChunk {
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
            round_id: "round-1".to_string(),
            attempt_id: Some("attempt-1".to_string()),
            attempt_index: Some(2),
            content: "thinking".to_string(),
            is_end: true,
        })
        .expect("projected");

        assert_eq!(projected.event_name, "agentic://text-chunk");
        assert_eq!(projected.event_type, "text-chunk");
        assert_eq!(projected.payload["contentType"], "thinking");
        assert_eq!(projected.payload["isThinkingEnd"], true);
        assert_eq!(projected.legacy_flat_message()["type"], "text-chunk");
    }

    #[test]
    fn legacy_flat_dialog_turn_started_preserves_existing_shape() {
        let projected = project_agentic_frontend_event(AgenticEvent::DialogTurnStarted {
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
            turn_index: 1,
            user_input: "raw input".to_string(),
            original_user_input: Some("original".to_string()),
            user_message_metadata: None,
        })
        .expect("projected");

        let message = projected.legacy_flat_message();

        assert_eq!(message["type"], "dialog-turn-started");
        assert_eq!(message["sessionId"], "session-1");
        assert!(message.get("userInput").is_none());
        assert_eq!(message["originalUserInput"], "original");
    }

    #[test]
    fn deep_review_queue_projection_preserves_camel_case_contract() {
        let projected = project_agentic_frontend_event(AgenticEvent::DeepReviewQueueStateChanged {
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
            queue_state: DeepReviewQueueState {
                tool_id: "tool-1".to_string(),
                subagent_type: "ReviewSecurity".to_string(),
                status: DeepReviewQueueStatus::QueuedForCapacity,
                reason: Some(DeepReviewQueueReason::ProviderConcurrencyLimit),
                queued_reviewer_count: 2,
                active_reviewer_count: Some(1),
                effective_parallel_instances: Some(2),
                optional_reviewer_count: Some(1),
                queue_elapsed_ms: Some(1200),
                run_elapsed_ms: None,
                max_queue_wait_seconds: Some(60),
                session_concurrency_high: true,
            },
        })
        .expect("projected");

        assert_eq!(
            projected.event_name,
            "agentic://deep-review-queue-state-changed"
        );
        assert_eq!(projected.payload["queueState"]["toolId"], "tool-1");
        assert_eq!(
            projected.payload["queueState"]["reason"],
            json!("provider_concurrency_limit")
        );
        assert_eq!(
            projected.payload["queueState"]["sessionConcurrencyHigh"],
            true
        );
    }

    #[test]
    fn session_title_projection_preserves_legacy_event_name_and_timestamp() {
        let projected = project_agentic_frontend_event(AgenticEvent::SessionTitleGenerated {
            session_id: "session-1".to_string(),
            title: "Title".to_string(),
            method: "model".to_string(),
        })
        .expect("projected");

        assert_eq!(projected.event_name, "session_title_generated");
        assert_eq!(projected.payload["sessionId"], "session-1");
        assert!(projected.payload["timestamp"].as_i64().is_some());
    }

    #[test]
    fn legacy_flat_message_keeps_projection_type_authoritative() {
        let event = AgenticFrontendEvent::new(
            "agentic://custom",
            "projected-type",
            json!({
                "type": "payload-type",
                "sessionId": "session-1",
            }),
        );

        let message = event.legacy_flat_message();

        assert_eq!(message["type"], "projected-type");
        assert_eq!(message["sessionId"], "session-1");
    }
}
