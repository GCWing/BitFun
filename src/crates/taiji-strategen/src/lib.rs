//! taiji-strategen — Strategy generator (MIT)
//!
//! Five-stage pipeline for LLM-driven strategy generation:
//!
//! 1. **Hypothesis generation** — LLM produces a trading strategy hypothesis
//!    from a natural-language prompt.
//! 2. **Validation** — [`HypothesisValidator`] checks type safety, reasonability,
//!    and look-ahead bias.
//! 3. **Compilation** — [`StrategyCompiler`] converts Hypothesis → PipelineConfig YAML.
//! 4. **Backtest** — [`BacktestRunner`] runs the strategy against historical data.
//! 5. **Analysis + Refinement** — [`ResultAnalyzer`] computes Deflated Sharpe Ratio
//!    and Monte Carlo tests; [`HypothesisRefiner`] uses LLM feedback to improve
//!    the hypothesis (up to 5 rounds).
//!
//! # Anti-overfitting constraints
//!
//! - Max 5 entry conditions
//! - Max 8 adjustable parameters
//! - Walk-Forward 4-fold OOS validation
//! - Deflated Sharpe Ratio + Monte Carlo permutation test

pub mod analyzer;
pub mod compiler;
pub mod hypothesis;
pub mod pipeline;
pub mod refiner;

// Re-export primary types
pub use analyzer::{AnalysisReport, MonteCarloResult, ResultAnalyzer};
pub use compiler::StrategyCompiler;
pub use hypothesis::{
    Condition, Hypothesis, HypothesisValidator, PositionSizing, RiskParams, ValidationReport,
};
pub use pipeline::{RoundLog, StrategyGenPipeline, StrategyGenResult};
pub use refiner::HypothesisRefiner;
