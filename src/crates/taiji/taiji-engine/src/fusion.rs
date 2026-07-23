//! Two-phase signal fusion engine.
//!
//! Phase 1 — weighted voting:  Σ(wi × dir_i)
//!   If |fusion_score| > 0.2 the result is deterministic and returned immediately.
//!
//! Phase 2 — LLM contradiction resolution:
//!   If |fusion_score| ≤ 0.2 and an [`LlmClient`] is configured, the ambiguous
//!   signal set is sent to the LLM for a tie-breaking adjudication.

mod weight_calibrator;

pub use weight_calibrator::{ConfidenceBucket, WeightCalibrator};

use std::sync::Arc;

use crate::error::Result;

// ---------------------------------------------------------------------------
// Direction
// ---------------------------------------------------------------------------

/// Trading direction for a single agent output or fused result.
///
/// Consistent with `crate::types::signal::SignalAction` semantics:
/// `Long` ≈ buy/long entry, `Short` ≈ sell/short entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Direction {
    Long,
    Short,
    Neutral,
}

impl Direction {
    /// Convert direction to a numeric sign: +1 / -1 / 0.
    pub fn to_sign(self) -> f64 {
        match self {
            Direction::Long => 1.0,
            Direction::Short => -1.0,
            Direction::Neutral => 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Agent output (standalone definition; R6.4 debate/mod.rs not yet present)
// ---------------------------------------------------------------------------

/// Output from a single trading agent (structure, delta, magnet, etc.).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentOutput {
    pub agent_id: String,
    pub direction: Direction,
    pub confidence: f64,
}

// ---------------------------------------------------------------------------
// LLM client trait (standalone; R6.0 LlmClient not yet extracted)
// ---------------------------------------------------------------------------

/// Async trait for LLM-based contradiction adjudication.
///
/// Implementations must be `Send + Sync` so they can be shared via `Arc`.
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    /// Ask the LLM to break a tie among conflicting agent outputs.
    /// Returns a reasoning string that is parsed for directional intent.
    async fn adjudicate(&self, agent_outputs: &[AgentOutput]) -> Result<String>;
}

// ---------------------------------------------------------------------------
// Agent weights
// ---------------------------------------------------------------------------

/// Per-agent fusion weights.  Agent ids map to the corresponding field by name.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentWeights {
    pub structure: f64,
    pub delta: f64,
    pub magnet: f64,
    pub thrust: f64,
    pub resonance: f64,
    pub risk: f64,
}

impl Default for AgentWeights {
    fn default() -> Self {
        Self {
            structure: 0.20,
            delta: 0.15,
            magnet: 0.20,
            thrust: 0.20,
            resonance: 0.25,
            risk: 0.00,
        }
    }
}

impl AgentWeights {
    /// Resolve the weight for a given agent id.
    fn resolve(&self, agent_id: &str) -> f64 {
        match agent_id {
            "structure" => self.structure,
            "delta" => self.delta,
            "magnet" => self.magnet,
            "thrust" => self.thrust,
            "resonance" => self.resonance,
            "risk" => self.risk,
            _ => 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Fusion result types
// ---------------------------------------------------------------------------

/// Which phase produced the final decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FusionPhase {
    /// Phase 1: |fusion_score| > 0.2 — weighted vote was decisive.
    Phase1Deterministic,
    /// Phase 2: |fusion_score| ≤ 0.2 — LLM broke the tie.
    Phase2LLM,
}

/// One agent's contribution to the fusion score.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentContribution {
    pub agent_id: String,
    pub direction: Direction,
    pub confidence: f64,
    pub weight: f64,
    pub weighted_score: f64,
}

/// A single agent's vote captured in the debate record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentVote {
    pub agent_id: String,
    pub direction: Direction,
}

/// Record produced when Phase 2 LLM adjudication is invoked.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DebateRecord {
    pub tie_detected: bool,
    pub agent_votes: Vec<AgentVote>,
}

/// Complete fusion output.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FusionResult {
    pub direction: Direction,
    pub confidence: f64,
    pub fusion_score: f64,
    pub phase: FusionPhase,
    pub agent_contributions: Vec<AgentContribution>,
    pub debate_record: Option<DebateRecord>,
    pub llm_reasoning: Option<String>,
}

// ---------------------------------------------------------------------------
// Fusion engine
// ---------------------------------------------------------------------------

/// Two-phase signal fusion engine.
///
/// ```
/// # use std::sync::Arc;
/// # use taiji_engine::fusion::*;
/// let weights = AgentWeights::default();
/// let engine = FusionEngine::new(weights, None);
/// // let result = engine.fuse(&[...]).await;
/// ```
pub struct FusionEngine {
    pub weights: AgentWeights,
    pub llm_client: Option<Arc<dyn LlmClient>>,
    pub calibrator: WeightCalibrator,
}

impl FusionEngine {
    pub fn new(weights: AgentWeights, llm_client: Option<Arc<dyn LlmClient>>) -> Self {
        Self {
            weights,
            llm_client,
            calibrator: WeightCalibrator::new(10),
        }
    }

    /// Run the two-phase fusion pipeline.
    ///
    /// Phase 1 computes `Σ(wi × dir_i)` across all agent outputs.  If the
    /// absolute score exceeds 0.2 the result is returned as
    /// [`FusionPhase::Phase1Deterministic`].
    ///
    /// Otherwise Phase 2 invokes the configured [`LlmClient`] (when present)
    /// to break the tie.  A neutral direction is returned when no LLM client
    /// is available and the score is ambiguous.
    pub async fn fuse(&self, agent_outputs: &[AgentOutput]) -> Result<FusionResult> {
        let contributions: Vec<AgentContribution> = agent_outputs
            .iter()
            .map(|ao| {
                let weight = self.weights.resolve(&ao.agent_id);
                let sign = ao.direction.to_sign();
                AgentContribution {
                    agent_id: ao.agent_id.clone(),
                    direction: ao.direction,
                    confidence: ao.confidence,
                    weight,
                    weighted_score: weight * sign,
                }
            })
            .collect();

        let fusion_score: f64 = contributions.iter().map(|c| c.weighted_score).sum();
        let total_weight: f64 = contributions.iter().map(|c| c.weight).sum();
        let confidence = if total_weight > 0.0 {
            (fusion_score.abs() / total_weight).min(1.0)
        } else {
            0.0
        };

        // Phase 1: decisive weighted vote
        if fusion_score.abs() > 0.2 {
            let direction = if fusion_score > 0.0 {
                Direction::Long
            } else if fusion_score < 0.0 {
                Direction::Short
            } else {
                Direction::Neutral
            };

            return Ok(FusionResult {
                direction,
                confidence,
                fusion_score,
                phase: FusionPhase::Phase1Deterministic,
                agent_contributions: contributions,
                debate_record: None,
                llm_reasoning: None,
            });
        }

        // Phase 2: ambiguous — try LLM adjudication
        if let Some(ref llm) = self.llm_client {
            let reasoning = llm.adjudicate(agent_outputs).await?;
            let direction = parse_llm_direction(&reasoning);

            let debate_record = DebateRecord {
                tie_detected: true,
                agent_votes: agent_outputs
                    .iter()
                    .map(|ao| AgentVote {
                        agent_id: ao.agent_id.clone(),
                        direction: ao.direction,
                    })
                    .collect(),
            };

            return Ok(FusionResult {
                direction,
                confidence: 0.5,
                fusion_score,
                phase: FusionPhase::Phase2LLM,
                agent_contributions: contributions,
                debate_record: Some(debate_record),
                llm_reasoning: Some(reasoning),
            });
        }

        // No LLM → fallback to Neutral
        Ok(FusionResult {
            direction: Direction::Neutral,
            confidence: 0.0,
            fusion_score,
            phase: FusionPhase::Phase1Deterministic,
            agent_contributions: contributions,
            debate_record: None,
            llm_reasoning: None,
        })
    }

    /// Recalibrate per-agent weights from backtest accuracy statistics.
    ///
    /// `backtest_stats` is an array of `(agent_id, accuracy)` pairs where
    /// `accuracy` is in [0.0, 1.0].  Each matched agent's weight is replaced
    /// with the clamped accuracy value.
    pub fn recalibrate_weights(&mut self, backtest_stats: &[(String, f64)]) {
        for (agent_id, accuracy) in backtest_stats {
            let w = accuracy.clamp(0.0, 1.0);
            match agent_id.as_str() {
                "structure" => self.weights.structure = w,
                "delta" => self.weights.delta = w,
                "magnet" => self.weights.magnet = w,
                "thrust" => self.weights.thrust = w,
                "resonance" => self.weights.resonance = w,
                "risk" => self.weights.risk = w,
                _ => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract a directional decision from LLM reasoning text.
///
/// Looks for the keywords `LONG` or `SHORT` (case-insensitive) in the
/// reasoning string.  Defaults to `Neutral` when neither is found.
fn parse_llm_direction(reasoning: &str) -> Direction {
    let upper = reasoning.to_uppercase();
    if upper.contains("LONG") {
        Direction::Long
    } else if upper.contains("SHORT") {
        Direction::Short
    } else {
        Direction::Neutral
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- helpers ----------------------------------------------------------

    fn ao_long(id: &str) -> AgentOutput {
        AgentOutput {
            agent_id: id.to_string(),
            direction: Direction::Long,
            confidence: 0.8,
        }
    }

    fn ao_short(id: &str) -> AgentOutput {
        AgentOutput {
            agent_id: id.to_string(),
            direction: Direction::Short,
            confidence: 0.8,
        }
    }

    fn ao_neutral(id: &str) -> AgentOutput {
        AgentOutput {
            agent_id: id.to_string(),
            direction: Direction::Neutral,
            confidence: 0.5,
        }
    }

    // -- Phase 1: weighted vote -------------------------------------------

    /// 4 Long + 3 Short → fusion_score > 0.2 → Long.
    #[tokio::test]
    async fn test_phase1_four_long_three_short_yields_long() {
        let outputs = vec![
            ao_long("structure"),
            ao_long("magnet"),
            ao_long("resonance"),
            ao_long("unknown"),
            ao_short("delta"),
            ao_short("thrust"),
            ao_short("risk"),
        ];
        let engine = FusionEngine::new(AgentWeights::default(), None);
        let result = engine.fuse(&outputs).await.unwrap();

        assert_eq!(result.direction, Direction::Long);
        assert_eq!(result.phase, FusionPhase::Phase1Deterministic);
        assert!(result.fusion_score > 0.0);
        assert_eq!(result.agent_contributions.len(), 7);
    }

    /// 3 Short + 2 Long → fusion_score = -0.30 → Short.
    #[tokio::test]
    async fn test_phase1_three_short_two_long_yields_short() {
        let outputs = vec![
            ao_short("structure"),
            ao_short("magnet"),
            ao_short("resonance"),
            ao_long("delta"),
            ao_long("thrust"),
        ];
        let engine = FusionEngine::new(AgentWeights::default(), None);
        let result = engine.fuse(&outputs).await.unwrap();

        assert_eq!(result.direction, Direction::Short);
        assert_eq!(result.phase, FusionPhase::Phase1Deterministic);
        assert!(result.fusion_score < 0.0);
    }

    /// All Neutral → fusion_score = 0.0 but no LLM → Neutral fallback.
    #[tokio::test]
    async fn test_phase1_all_neutral_no_llm_yields_neutral() {
        let outputs = vec![ao_neutral("structure"), ao_neutral("delta")];
        let engine = FusionEngine::new(AgentWeights::default(), None);
        let result = engine.fuse(&outputs).await.unwrap();

        assert_eq!(result.direction, Direction::Neutral);
        // |0.0| = 0.0 ≤ 0.2 but no LLM → stays Phase1Deterministic with neutral
        assert_eq!(result.confidence, 0.0);
    }

    // -- Phase 2: LLM adjudication ----------------------------------------

    struct MockLlm {
        direction: Direction,
    }

    #[async_trait::async_trait]
    impl LlmClient for MockLlm {
        async fn adjudicate(&self, _agent_outputs: &[AgentOutput]) -> Result<String> {
            match self.direction {
                Direction::Long => Ok("LONG: bullish consensus despite near-tie".into()),
                Direction::Short => Ok("SHORT: downside risk outweighs bullish signals".into()),
                Direction::Neutral => Ok("NEUTRAL: no clear edge in either direction".into()),
            }
        }
    }

    /// |fusion_score| = 0.0 → Phase 2 triggers with MOCK LLM returning Long.
    #[tokio::test]
    async fn test_phase2_llm_adjudicates_tie_to_long() {
        let outputs = vec![ao_long("structure"), ao_short("delta")];
        let mock = Arc::new(MockLlm {
            direction: Direction::Long,
        });
        let engine = FusionEngine::new(AgentWeights::default(), Some(mock));
        let result = engine.fuse(&outputs).await.unwrap();

        assert_eq!(result.direction, Direction::Long);
        assert_eq!(result.phase, FusionPhase::Phase2LLM);
        assert!(result.debate_record.is_some());
        let dr = result.debate_record.as_ref().unwrap();
        assert!(dr.tie_detected);
        assert_eq!(dr.agent_votes.len(), 2);
        assert!(result.llm_reasoning.is_some());
        assert!(result.llm_reasoning.unwrap().contains("LONG"));
    }

    /// 3 Long + 3 Short → |fusion_score| = 0.0 → LLM returns Short.
    #[tokio::test]
    async fn test_phase2_balanced_six_agents_llm_short() {
        let outputs = vec![
            ao_long("structure"),
            ao_long("delta"),
            ao_long("magnet"),
            ao_short("thrust"),
            ao_short("resonance"),
            ao_short("risk"),
        ];
        let mock = Arc::new(MockLlm {
            direction: Direction::Short,
        });
        let engine = FusionEngine::new(AgentWeights::default(), Some(mock));
        let result = engine.fuse(&outputs).await.unwrap();

        assert_eq!(result.direction, Direction::Short);
        assert_eq!(result.phase, FusionPhase::Phase2LLM);
    }

    /// |fusion_score| = 0.1 (< 0.2) — close to tie — still triggers Phase 2.
    #[tokio::test]
    async fn test_phase2_near_tie_triggers_llm() {
        // 2 Long + 1 Short with equal weights → score = 2*1 + 1*(-1) = 1.0 → that's > 0.2.
        // Need score about 0.1:  2 Long + 1 Short + weights tuned.
        // Simpler: use 2 Long + 1 Short, but make Short weight 1.9 (custom weights).
        let weights = AgentWeights {
            structure: 1.0, // Long
            delta: 1.0,     // Long
            magnet: 1.9,    // Short → score = 1.0 + 1.0 - 1.9 = 0.1
            ..AgentWeights::default()
        };
        let outputs = vec![ao_long("structure"), ao_long("delta"), ao_short("magnet")];
        let mock = Arc::new(MockLlm {
            direction: Direction::Long,
        });
        let engine = FusionEngine::new(weights, Some(mock));
        let result = engine.fuse(&outputs).await.unwrap();

        assert_eq!(result.phase, FusionPhase::Phase2LLM);
        assert!((result.fusion_score.abs() - 0.1).abs() < 0.01);
        assert_eq!(result.direction, Direction::Long);
    }

    // -- recalibrate_weights -----------------------------------------------

    #[test]
    fn test_recalibrate_weights_updates_matched_agents() {
        let mut engine = FusionEngine::new(AgentWeights::default(), None);
        let stats = vec![("structure".to_string(), 0.92), ("delta".to_string(), 0.75)];
        engine.recalibrate_weights(&stats);

        assert!((engine.weights.structure - 0.92).abs() < f64::EPSILON);
        assert!((engine.weights.delta - 0.75).abs() < f64::EPSILON);
        // untouched
        assert!((engine.weights.magnet - 0.20).abs() < f64::EPSILON);
    }

    #[test]
    fn test_recalibrate_ignores_unknown_agent_ids() {
        let mut engine = FusionEngine::new(AgentWeights::default(), None);
        let stats = vec![("nonexistent".to_string(), 0.42)];
        engine.recalibrate_weights(&stats);

        // All weights should remain at default values
        assert!((engine.weights.structure - 0.20).abs() < f64::EPSILON);
        assert!((engine.weights.delta - 0.15).abs() < f64::EPSILON);
        assert!((engine.weights.magnet - 0.20).abs() < f64::EPSILON);
    }

    // -- direction to_sign -------------------------------------------------

    #[test]
    fn test_direction_to_sign() {
        assert!((Direction::Long.to_sign() - 1.0).abs() < f64::EPSILON);
        assert!((Direction::Short.to_sign() + 1.0).abs() < f64::EPSILON);
        assert!((Direction::Neutral.to_sign() - 0.0).abs() < f64::EPSILON);
    }

    // -- parse_llm_direction -----------------------------------------------

    #[test]
    fn test_parse_llm_direction_case_insensitive() {
        assert_eq!(parse_llm_direction("LONG: bullish"), Direction::Long);
        assert_eq!(parse_llm_direction("short: bearish"), Direction::Short);
        assert_eq!(
            parse_llm_direction("Recommend Long position"),
            Direction::Long
        );
        assert_eq!(parse_llm_direction("no clear signal"), Direction::Neutral);
        assert_eq!(parse_llm_direction(""), Direction::Neutral);
    }

    // -- FusionEngine::new stores calibrator -------------------------------

    #[test]
    fn test_engine_default_has_ten_bucket_calibrator() {
        let engine = FusionEngine::new(AgentWeights::default(), None);
        assert_eq!(engine.calibrator.buckets.len(), 10);
    }
}
