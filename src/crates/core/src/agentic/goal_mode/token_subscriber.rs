//! Accumulates per-turn billable tokens for active thread goals from model usage events.

use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::events::{AgenticEvent, EventSubscriber};
use crate::agentic::goal_mode::billable_tokens_from_counts;
use crate::util::errors::BitFunResult;
use log::debug;

pub struct ThreadGoalTokenSubscriber;

#[async_trait::async_trait]
impl EventSubscriber for ThreadGoalTokenSubscriber {
    async fn on_event(&self, event: &AgenticEvent) -> BitFunResult<()> {
        let AgenticEvent::TokenUsageUpdated {
            session_id,
            turn_id,
            input_tokens,
            output_tokens,
            is_subagent,
            cached_tokens,
            ..
        } = event
        else {
            return Ok(());
        };

        if *is_subagent {
            return Ok(());
        }

        let output_tokens = output_tokens.unwrap_or(0);
        let cached_tokens = cached_tokens.unwrap_or(0);
        let billable = billable_tokens_from_counts(*input_tokens, cached_tokens, output_tokens);
        if billable == 0 {
            return Ok(());
        }

        let Some(coordinator) = get_global_coordinator() else {
            return Ok(());
        };

        coordinator
            .thread_goal_runtime()
            .record_round_billable_tokens(turn_id, billable);

        debug!(
            "Thread goal token accounting: session_id={}, turn_id={}, billable={}",
            session_id, turn_id, billable
        );

        Ok(())
    }
}
