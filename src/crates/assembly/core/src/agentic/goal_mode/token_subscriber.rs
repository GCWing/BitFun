//! Accumulates per-turn billable tokens for active thread goals from model usage events.

use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::events::{AgenticEvent, EventSubscriber};
use bitfun_agent_runtime::event_bus::EventSubscriberResult;
use bitfun_agent_runtime::thread_goal::{
    should_record_thread_goal_token_usage, ThreadGoalTokenUsageFacts,
};
use log::debug;

pub struct ThreadGoalTokenSubscriber;

#[async_trait::async_trait]
impl EventSubscriber for ThreadGoalTokenSubscriber {
    async fn on_event(&self, event: &AgenticEvent) -> EventSubscriberResult {
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

        let Some(billable) = should_record_thread_goal_token_usage(ThreadGoalTokenUsageFacts {
            input_tokens: *input_tokens,
            output_tokens: *output_tokens,
            cached_tokens: *cached_tokens,
            is_subagent: *is_subagent,
        }) else {
            return Ok(());
        };

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
