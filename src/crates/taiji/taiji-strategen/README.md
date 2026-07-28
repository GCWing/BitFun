# taiji-strategen — LLM-Driven Strategy Generation

5-stage pipeline: Hypothesis generation → Validation → Compilation → Backtest → Analysis + Refinement. Anti-overfitting constraints: max 5 entry conditions, max 8 parameters, walk-forward 4-fold OOS, Deflated Sharpe Ratio + Monte Carlo permutation test. Up to 5 refinement rounds.

## Usage

```rust
use taiji_strategen::pipeline::StrategyGenPipeline;
use taiji_llm::client::LlmConfig;

let llm_config = LlmConfig::default();
let mut pipeline = StrategyGenPipeline::new(llm_config, Some("models/".into()));

let result = pipeline.run(
    "Generate a mean-reversion strategy for Chinese commodity futures. Use RSI and Bollinger Bands.",
    3, // max rounds
)?;

println!("Best Sharpe: {:.2}", result.best_sharpe);
println!("Generated YAML:\n{}", result.best_yaml);
```

```bash
cargo add taiji-strategen
```

## Modules

| Module | Description |
|--------|-------------|
| `hypothesis` | `Hypothesis` struct + `HypothesisValidator` (type safety, reasonability, look-ahead bias) |
| `compiler` | `StrategyCompiler` — Hypothesis → PipelineConfig YAML |
| `pipeline` | `StrategyGenPipeline` — 5-stage orchestrator with `RoundLog` tracking |
| `analyzer` | `ResultAnalyzer` — Deflated Sharpe Ratio, Monte Carlo permutation test |
| `refiner` | `HypothesisRefiner` — LLM-driven iterative refinement |
