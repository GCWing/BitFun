use serde::{Deserialize, Serialize};

use super::{DebateContext, TokenUsage};
use taiji_llm::DecisionOutput;

/// Consensus strength after a debate round.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusLevel {
    /// All agents agree on the same direction.
    StrongConsensus,
    /// Majority agrees; minority holds (neutral) or has low-confidence disagreement.
    WeakConsensus,
    /// Equal split between opposing directions.
    Split,
    /// No direction can be determined (all hold, or irreconcilable conflict).
    Deadlock,
}

/// A single turn within a debate round.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebateTurn {
    /// Phase name: "opening" | "rebuttal" | "closing"
    pub phase: String,
    /// Agent identifier, e.g. "bull", "bear", "neutral"
    pub agent_id: String,
    /// Raw text content produced by the agent
    pub content: String,
    /// Structured decision if the agent produced one
    pub decision: Option<DecisionOutput>,
    /// Token usage for this turn
    pub token_usage: TokenUsage,
}

/// Complete record of a multi-agent debate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebateRecord {
    /// Unique debate identifier (UUID v4)
    pub debate_id: String,
    /// Context that triggered this debate
    pub context: DebateContext,
    /// All debate turns in chronological order
    pub turns: Vec<DebateTurn>,
    /// Final consensus level
    pub consensus: ConsensusLevel,
    /// Final decision from the DecisionAgent
    pub final_decision: DecisionOutput,
    /// Total token usage across all turns
    pub token_usage: TokenUsage,
    /// Wall-clock duration in milliseconds
    pub duration_ms: u64,
}

impl DebateRecord {
    /// Compute ConsensusLevel from the final positions of all debate agents
    /// and the decision agent's verdict.
    pub fn compute_consensus(agent_final_directions: &[(String, String, f64)]) -> ConsensusLevel {
        if agent_final_directions.is_empty() {
            return ConsensusLevel::Deadlock;
        }

        let long_count = agent_final_directions
            .iter()
            .filter(|(_, d, _)| d == "long")
            .count();
        let short_count = agent_final_directions
            .iter()
            .filter(|(_, d, _)| d == "short")
            .count();
        let hold_count = agent_final_directions
            .iter()
            .filter(|(_, d, _)| d == "hold")
            .count();
        let total = agent_final_directions.len();

        if long_count == total || short_count == total {
            ConsensusLevel::StrongConsensus
        } else if long_count > short_count + hold_count || short_count > long_count + hold_count {
            ConsensusLevel::WeakConsensus
        } else if long_count > 0 && short_count > 0 && long_count == short_count {
            ConsensusLevel::Split
        } else {
            ConsensusLevel::Deadlock
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consensus_strong_all_long() {
        let dirs = vec![
            ("bull".into(), "long".into(), 0.9),
            ("bear".into(), "long".into(), 0.8),
            ("neutral".into(), "long".into(), 0.7),
        ];
        assert_eq!(
            DebateRecord::compute_consensus(&dirs),
            ConsensusLevel::StrongConsensus
        );
    }

    #[test]
    fn consensus_weak_majority_long_one_hold() {
        let dirs = vec![
            ("bull".into(), "long".into(), 0.9),
            ("bear".into(), "long".into(), 0.8),
            ("neutral".into(), "hold".into(), 0.5),
        ];
        assert_eq!(
            DebateRecord::compute_consensus(&dirs),
            ConsensusLevel::WeakConsensus
        );
    }

    #[test]
    fn consensus_split_equal_long_short() {
        let dirs = vec![
            ("bull".into(), "long".into(), 0.9),
            ("bear".into(), "short".into(), 0.8),
            ("neutral".into(), "hold".into(), 0.5),
        ];
        assert_eq!(
            DebateRecord::compute_consensus(&dirs),
            ConsensusLevel::Split
        );
    }

    #[test]
    fn consensus_deadlock_all_hold() {
        let dirs = vec![
            ("bull".into(), "hold".into(), 0.6),
            ("bear".into(), "hold".into(), 0.5),
            ("neutral".into(), "hold".into(), 0.4),
        ];
        assert_eq!(
            DebateRecord::compute_consensus(&dirs),
            ConsensusLevel::Deadlock
        );
    }
}
