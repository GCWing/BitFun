//! Dialog turn helpers and statistics types.
//!
//! Historical note: this module used to define `DialogTurn` and
//! `DialogTurnState` structs that were never persisted nor read back —
//! the product on-disk shape lives in the core session persistence adapter,
//! and turn lifecycle state is tracked through `SessionState::Processing`
//! and `TurnStatus`. The orphan structs were removed; only `TurnStats`
//! and a small id-helper survive as provider-neutral turn facts.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use bitfun_events::AgenticEvent;

/// Generate a fresh turn id when callers do not supply one.
pub fn new_turn_id(provided: Option<String>) -> String {
    provided.unwrap_or_else(|| Uuid::new_v4().to_string())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TurnStats {
    pub total_rounds: usize,
    pub total_tools: usize,
    pub total_tokens: usize,
    pub duration_ms: u64,
}

/// Token usage aggregated across all model rounds in one dialog turn.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TurnTokenUsage {
    pub input_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<usize>,
    pub total_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<usize>,
}

impl TurnTokenUsage {
    fn merge_round(&mut self, round: Self) {
        self.input_tokens = self.input_tokens.saturating_add(round.input_tokens);
        self.output_tokens = self
            .output_tokens
            .zip(round.output_tokens)
            .map(|(current, next)| current.saturating_add(next));
        self.total_tokens = self.total_tokens.saturating_add(round.total_tokens);
        self.cached_tokens = self
            .cached_tokens
            .zip(round.cached_tokens)
            .map(|(current, next)| current.saturating_add(next));
    }

    pub fn accumulate_event<'a>(
        aggregate: &mut Option<Self>,
        event: &'a AgenticEvent,
        expected_turn_id: &str,
    ) -> Option<&'a str> {
        let AgenticEvent::TokenUsageUpdated {
            turn_id,
            model_config_id,
            input_tokens,
            output_tokens,
            total_tokens,
            cached_tokens,
            ..
        } = event
        else {
            return None;
        };
        if turn_id != expected_turn_id {
            return None;
        }

        let round = Self {
            input_tokens: *input_tokens,
            output_tokens: *output_tokens,
            total_tokens: *total_tokens,
            cached_tokens: *cached_tokens,
        };
        if let Some(total) = aggregate.as_mut() {
            total.merge_round(round);
        } else {
            *aggregate = Some(round);
        }
        Some(model_config_id)
    }
}

#[cfg(test)]
mod tests {
    use super::TurnTokenUsage;
    use bitfun_events::AgenticEvent;

    #[test]
    fn turn_usage_accumulates_rounds_and_ignores_other_turns() {
        let events = [
            usage_event("turn-1", 100, Some(25), 125, Some(40)),
            usage_event("turn-2", 900, Some(90), 990, Some(80)),
            usage_event("turn-1", 200, Some(50), 250, Some(80)),
        ];
        let mut usage = None;

        for event in &events {
            TurnTokenUsage::accumulate_event(&mut usage, event, "turn-1");
        }

        assert_eq!(
            usage,
            Some(TurnTokenUsage {
                input_tokens: 300,
                output_tokens: Some(75),
                total_tokens: 375,
                cached_tokens: Some(120),
            })
        );
    }

    #[test]
    fn turn_usage_keeps_optional_totals_unknown_if_any_round_omits_them() {
        let events = [
            usage_event("turn-1", 100, None, 100, Some(20)),
            usage_event("turn-1", 50, Some(10), 60, None),
        ];
        let mut usage = None;

        for event in &events {
            TurnTokenUsage::accumulate_event(&mut usage, event, "turn-1");
        }

        let usage = usage.expect("matching usage");
        assert_eq!(usage.input_tokens, 150);
        assert_eq!(usage.output_tokens, None);
        assert_eq!(usage.total_tokens, 160);
        assert_eq!(usage.cached_tokens, None);
    }

    fn usage_event(
        turn_id: &str,
        input_tokens: usize,
        output_tokens: Option<usize>,
        total_tokens: usize,
        cached_tokens: Option<usize>,
    ) -> AgenticEvent {
        AgenticEvent::TokenUsageUpdated {
            session_id: "session-1".to_string(),
            turn_id: turn_id.to_string(),
            model_config_id: "model-config".to_string(),
            effective_model_name: "provider-model".to_string(),
            input_tokens,
            output_tokens,
            total_tokens,
            max_context_tokens: Some(200_000),
            is_subagent: false,
            cached_tokens,
            token_details: None,
        }
    }
}
