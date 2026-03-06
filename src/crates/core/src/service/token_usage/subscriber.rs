//! Token usage event subscriber

use crate::agentic::events::{AgenticEvent, EventSubscriber};
use crate::service::token_usage::TokenUsageService;
use crate::util::errors::BitFunResult;
use log::{debug, error, info};
use std::sync::Arc;

/// Token usage event subscriber
///
/// Listens to TokenUsageUpdated events and records them
pub struct TokenUsageSubscriber {
    token_usage_service: Arc<TokenUsageService>,
}

impl TokenUsageSubscriber {
    pub fn new(token_usage_service: Arc<TokenUsageService>) -> Self {
        Self {
            token_usage_service,
        }
    }
}

#[async_trait::async_trait]
impl EventSubscriber for TokenUsageSubscriber {
    async fn on_event(&self, event: &AgenticEvent) -> BitFunResult<()> {
        if let AgenticEvent::TokenUsageUpdated {
            session_id,
            turn_id,
            model_id,
            input_tokens,
            output_tokens,
            total_tokens,
            is_subagent,
            ..
        } = event
        {
            let output = output_tokens.unwrap_or(0);
            let cached = 0;
            
            debug!(
                "Recording token usage: model={}, session={}, turn={}, input={}, output={}, total={}, is_subagent={}",
                model_id, session_id, turn_id, input_tokens, output, total_tokens, is_subagent
            );

            if let Err(e) = self
                .token_usage_service
                .record_usage(
                    model_id.clone(),
                    session_id.clone(),
                    turn_id.clone(),
                    *input_tokens as u32,
                    output as u32,
                    cached,
                    *is_subagent,
                )
                .await
            {
                error!("Failed to record token usage: {}", e);
            }
        }

        Ok(())
    }
}
