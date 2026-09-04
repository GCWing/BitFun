use super::SessionManager;
use crate::agentic::events::{AgenticEvent, EventSubscriber};
use log::warn;
use openbitfun_agent_runtime::event_bus::EventSubscriberResult;
use openbitfun_services_core::session::{SessionContextUsage, SessionContextUsageSource};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Persists the runtime-owned context usage used by every product surface.
pub struct SessionContextUsageSubscriber {
    session_manager: Arc<SessionManager>,
}

impl SessionContextUsageSubscriber {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self { session_manager }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn usage_from_event(event: &AgenticEvent) -> Option<(&str, SessionContextUsage)> {
    match event {
        AgenticEvent::TokenUsageUpdated {
            session_id,
            turn_id,
            input_tokens,
            output_tokens,
            total_tokens,
            ..
        } => Some((
            session_id,
            SessionContextUsage {
                turn_id: turn_id.clone(),
                input_tokens: *input_tokens as u64,
                output_tokens: output_tokens.map(|value| value as u64),
                total_tokens: *total_tokens as u64,
                timestamp: now_ms(),
                source: SessionContextUsageSource::ModelRequest,
            },
        )),
        AgenticEvent::ContextCompressionCompleted {
            session_id,
            turn_id,
            tokens_after,
            applied: true,
            ..
        } => Some((
            session_id,
            SessionContextUsage {
                turn_id: turn_id.clone(),
                input_tokens: *tokens_after as u64,
                output_tokens: None,
                total_tokens: *tokens_after as u64,
                timestamp: now_ms(),
                source: SessionContextUsageSource::ContextCompression,
            },
        )),
        _ => None,
    }
}

#[async_trait::async_trait]
impl EventSubscriber for SessionContextUsageSubscriber {
    async fn on_event(&self, event: &AgenticEvent) -> EventSubscriberResult {
        let Some((session_id, usage)) = usage_from_event(event) else {
            return Ok(());
        };

        if let Err(error) = self
            .session_manager
            .persist_current_context_usage(session_id, usage)
            .await
        {
            warn!(
                "Failed to persist session context usage: session_id={}, error={}",
                session_id, error
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_model_request_usage() {
        let event = AgenticEvent::TokenUsageUpdated {
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
            model_config_id: "model-config".to_string(),
            effective_model_name: "model".to_string(),
            input_tokens: 42_000,
            output_tokens: Some(1_500),
            total_tokens: 43_500,
            max_context_tokens: Some(128_000),
            is_subagent: false,
            cached_tokens: None,
            token_details: None,
        };

        let (session_id, usage) = usage_from_event(&event).expect("usage event");
        assert_eq!(session_id, "session-1");
        assert_eq!(usage.turn_id, "turn-1");
        assert_eq!(usage.input_tokens, 42_000);
        assert_eq!(usage.output_tokens, Some(1_500));
        assert_eq!(usage.total_tokens, 43_500);
        assert_eq!(usage.source, SessionContextUsageSource::ModelRequest);
    }

    #[test]
    fn maps_only_applied_context_compression() {
        let event = AgenticEvent::ContextCompressionCompleted {
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
            compression_id: "compression-1".to_string(),
            compression_count: 1,
            tokens_before: 90_000,
            tokens_after: 15_000,
            compression_ratio: 0.17,
            duration_ms: 500,
            has_summary: true,
            summary_source: "model".to_string(),
            applied: true,
        };

        let (_, usage) = usage_from_event(&event).expect("applied compression");
        assert_eq!(usage.input_tokens, 15_000);
        assert_eq!(usage.output_tokens, None);
        assert_eq!(usage.total_tokens, 15_000);
        assert_eq!(usage.source, SessionContextUsageSource::ContextCompression);

        let not_applied = AgenticEvent::ContextCompressionCompleted {
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
            compression_id: "compression-1".to_string(),
            compression_count: 1,
            tokens_before: 90_000,
            tokens_after: 15_000,
            compression_ratio: 0.17,
            duration_ms: 500,
            has_summary: true,
            summary_source: "model".to_string(),
            applied: false,
        };
        assert!(usage_from_event(&not_applied).is_none());
    }
}
