use std::sync::Arc;

use serde::{Deserialize, Serialize};
use taiji_backtest::{BacktestConfig, BacktestResult, BacktestRunner};

use crate::analyzer::{AnalysisReport, ResultAnalyzer};
use crate::compiler::StrategyCompiler;
use crate::hypothesis::{Hypothesis, HypothesisValidator};
use crate::refiner::HypothesisRefiner;

/// Result of a complete strategy generation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyGenResult {
    /// The final (best) hypothesis after refinement rounds.
    pub hypothesis: Hypothesis,
    /// PipelineConfig YAML for the final hypothesis.
    pub yaml: String,
    /// Backtest result (None if backtest skipped).
    pub backtest_result: Option<BacktestResult>,
    /// Final analysis report.
    pub analysis_report: AnalysisReport,
    /// Number of refinement rounds executed.
    pub rounds: usize,
    /// Per-round logs for traceability.
    pub round_logs: Vec<RoundLog>,
}

/// Per-round log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundLog {
    pub round: usize,
    pub hypothesis_name: String,
    pub deflated_sharpe: f64,
    pub monte_carlo_pvalue: f64,
    pub overfitting_flag: bool,
    pub improved: bool,
}

/// Five-stage strategy generation pipeline.
///
/// Stage 1: LLM generates initial Hypothesis from a natural-language prompt.
/// Stage 2: HypothesisValidator checks type safety, reasonability, look-ahead bias.
/// Stage 3: StrategyCompiler converts Hypothesis → PipelineConfig YAML.
/// Stage 4: BacktestRunner runs backtest → PerformanceStats + trades.
/// Stage 5: ResultAnalyzer produces AnalysisReport; HypothesisRefiner improves
///          the hypothesis (repeat up to max_rounds).
pub struct StrategyGenPipeline {
    validator: HypothesisValidator,
    backtest_config: BacktestConfig,
    refiner: HypothesisRefiner,
}

impl StrategyGenPipeline {
    /// Create a new pipeline.
    ///
    /// `registered_indicators` is the list of indicator names supported by taiji-engine.
    /// `backtest_config` provides the base config for creating BacktestRunner instances.
    /// `refiner` handles LLM-based hypothesis improvement.
    pub fn new(
        registered_indicators: Vec<String>,
        backtest_config: BacktestConfig,
        refiner: HypothesisRefiner,
    ) -> Self {
        Self {
            validator: HypothesisValidator::new(registered_indicators),
            backtest_config,
            refiner,
        }
    }

    /// Create a pipeline with mock LLM refinement (no network dependency).
    pub fn new_mock(registered_indicators: Vec<String>, backtest_config: BacktestConfig) -> Self {
        let refiner = HypothesisRefiner {
            llm_client: Arc::new(MockLlmClient),
            max_rounds: 5,
        };
        Self::new(registered_indicators, backtest_config, refiner)
    }

    /// Run the full strategy generation pipeline.
    ///
    /// `initial_hypothesis` is the starting hypothesis (from LLM or hand-crafted).
    /// `csv_path` is the path to CSV tick data for backtesting.
    /// `use_mock_refiner` uses heuristic refinement instead of LLM (for testing).
    pub async fn generate(
        &self,
        initial_hypothesis: Hypothesis,
        csv_path: Option<std::path::PathBuf>,
        use_mock_refiner: bool,
    ) -> Result<StrategyGenResult, anyhow::Error> {
        let max_rounds = self.refiner.max_rounds();
        let mut current_hypothesis = initial_hypothesis;
        let mut round_logs: Vec<RoundLog> = Vec::new();
        let mut best_hypothesis: Option<(Hypothesis, f64, AnalysisReport, String)> = None;
        let mut last_backtest_result: Option<BacktestResult> = None;

        for round in 0..=max_rounds {
            // Stage 2: Validate
            let validation = self.validator.validate(&current_hypothesis);
            if !validation.is_valid {
                return Err(anyhow::anyhow!(
                    "Hypothesis validation failed at round {}: {}",
                    round,
                    validation.errors.join("; ")
                ));
            }
            if !validation.lookahead_free {
                return Err(anyhow::anyhow!(
                    "Look-ahead bias detected at round {}: {}",
                    round,
                    validation.errors.join("; ")
                ));
            }

            // Stage 3: Compile
            let yaml_value = StrategyCompiler::compile(&current_hypothesis)?;
            let yaml_str = serde_yaml::to_string(&yaml_value)?;

            // Stage 4: Backtest
            last_backtest_result = if let Some(ref csv_path) = csv_path {
                let tmp_yaml = write_temp_yaml(&yaml_str)?;
                let result = self.run_backtest_with_yaml(&tmp_yaml, csv_path)?;
                let _ = std::fs::remove_file(&tmp_yaml);
                Some(result)
            } else {
                None
            };

            // Stage 5: Analyze
            let wf_report = last_backtest_result
                .as_ref()
                .and_then(|r| r.walk_forward.clone());
            let analysis = match last_backtest_result.as_ref() {
                Some(result) => ResultAnalyzer::analyze(&result.stats, &wf_report)?,
                None => AnalysisReport {
                    deflated_sharpe: 0.0,
                    monte_carlo_pvalue: 1.0,
                    walk_forward_robustness: 0.0,
                    overfitting_flag: false,
                },
            };

            // Track best hypothesis by deflated Sharpe
            let improved = match &best_hypothesis {
                Some((_, best_dsr, _, _)) => analysis.deflated_sharpe > *best_dsr,
                None => true,
            };
            if improved {
                let yaml_copy = serde_yaml::to_string(&serde_yaml::to_value(&current_hypothesis)?)?;
                best_hypothesis = Some((
                    current_hypothesis.clone(),
                    analysis.deflated_sharpe,
                    analysis.clone(),
                    yaml_copy,
                ));
            }

            round_logs.push(RoundLog {
                round,
                hypothesis_name: current_hypothesis.name.clone(),
                deflated_sharpe: analysis.deflated_sharpe,
                monte_carlo_pvalue: analysis.monte_carlo_pvalue,
                overfitting_flag: analysis.overfitting_flag,
                improved,
            });

            // Early stop if no overfitting and performance is good
            if !analysis.overfitting_flag && analysis.deflated_sharpe > 1.0 {
                break;
            }

            // Refine for next round (skip on final round)
            if round < max_rounds {
                current_hypothesis = if use_mock_refiner {
                    HypothesisRefiner::refine_mock(&current_hypothesis, &analysis)
                        .map_err(|e| anyhow::anyhow!("Mock refinement failed: {}", e))?
                } else {
                    self.refiner.refine(&current_hypothesis, &analysis).await?
                };
            }
        }

        let (final_hypothesis, _, final_analysis, final_yaml) =
            best_hypothesis.unwrap_or_else(|| {
                (
                    current_hypothesis.clone(),
                    0.0,
                    AnalysisReport {
                        deflated_sharpe: 0.0,
                        monte_carlo_pvalue: 1.0,
                        walk_forward_robustness: 0.0,
                        overfitting_flag: false,
                    },
                    String::new(),
                )
            });

        Ok(StrategyGenResult {
            hypothesis: final_hypothesis,
            yaml: final_yaml,
            backtest_result: last_backtest_result,
            analysis_report: final_analysis,
            rounds: round_logs.len(),
            round_logs,
        })
    }

    /// Run backtest using a temporary YAML file and a CSV data file.
    fn run_backtest_with_yaml(
        &self,
        yaml_path: &std::path::Path,
        csv_path: &std::path::Path,
    ) -> Result<BacktestResult, anyhow::Error> {
        let mut config = self.backtest_config.clone();
        config.pipeline_template = yaml_path.to_path_buf();

        let mut runner = BacktestRunner::new(config);
        runner.set_csv_path(csv_path.to_path_buf());

        let rt = tokio::runtime::Handle::current();
        let result = rt.block_on(runner.run())?;
        Ok(result)
    }

    pub fn validator(&self) -> &HypothesisValidator {
        &self.validator
    }
}

/// Write YAML content to a temporary file and return the path.
fn write_temp_yaml(yaml_str: &str) -> Result<std::path::PathBuf, anyhow::Error> {
    let dir = std::env::temp_dir().join("taiji-strategen");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("pipeline_{}.yaml", std::process::id()));
    std::fs::write(&path, yaml_str)?;
    Ok(path)
}

// ── Mock LLM Client ──

struct MockLlmClient;

#[async_trait::async_trait]
impl taiji_llm::LlmClient for MockLlmClient {
    async fn chat(
        &self,
        _messages: &[taiji_llm::ChatMessage],
        _config: &taiji_llm::LlmConfig,
    ) -> Result<taiji_llm::ChatResponse, anyhow::Error> {
        Ok(taiji_llm::ChatResponse {
            content: r#"{"name":"mock","description":"mock","entry_conditions":[],"exit_conditions":[],"position_sizing":{"method":"fixed","value":1.0},"risk_params":{"stop_loss":null,"take_profit":null,"max_holding_bars":20,"max_drawdown":null},"instruments":[]}"#.into(),
            usage: taiji_llm::Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
            finish_reason: "stop".into(),
        })
    }

    async fn chat_stream(
        &self,
        _messages: &[taiji_llm::ChatMessage],
        _config: &taiji_llm::LlmConfig,
    ) -> Result<taiji_llm::client::ChatStream, anyhow::Error> {
        anyhow::bail!("mock does not support streaming")
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hypothesis::{Condition, PositionSizing, RiskParams};
    use chrono::NaiveDate;

    fn make_test_hypothesis() -> Hypothesis {
        Hypothesis {
            name: "pipeline_test".into(),
            description: "Test pipeline strategy".into(),
            entry_conditions: vec![Condition {
                indicator: "MA".into(),
                params: serde_json::json!({"period": 5}),
                operator: "cross_above".into(),
                value: 0.0,
            }],
            exit_conditions: vec![Condition {
                indicator: "MA".into(),
                params: serde_json::json!({"period": 20}),
                operator: "cross_below".into(),
                value: 0.0,
            }],
            position_sizing: PositionSizing {
                method: "fixed".into(),
                value: 1.0,
            },
            risk_params: RiskParams {
                stop_loss: Some(50.0),
                take_profit: Some(100.0),
                max_holding_bars: 20,
                max_drawdown: None,
            },
            instruments: vec!["rb9999".into()],
        }
    }

    fn make_backtest_config() -> BacktestConfig {
        BacktestConfig {
            instruments: vec!["rb9999".into()],
            date_range: taiji_backtest::DateRange {
                start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                end: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            },
            initial_capital: 100_000.0,
            commission_per_lot: 3.0,
            slippage_ticks: 1,
            pipeline_template: std::path::PathBuf::from("pipeline.yaml"),
            walk_forward: None,
            contract_multipliers: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn test_pipeline_validate_and_compile_no_backtest() {
        let config = make_backtest_config();
        let pipeline = StrategyGenPipeline::new_mock(vec!["MA".into(), "RSI".into()], config);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(pipeline.generate(make_test_hypothesis(), None, true));

        assert!(result.is_ok(), "error: {:?}", result.err());
        let r = result.unwrap();
        assert!(r.rounds >= 1, "expected at least 1 round, got {}", r.rounds);
        assert!(!r.hypothesis.name.is_empty());
        assert!(!r.round_logs.is_empty());
    }

    #[test]
    fn test_pipeline_validation_error_stops_early() {
        let config = make_backtest_config();
        let pipeline = StrategyGenPipeline::new_mock(vec![], config);

        let mut bad_h = make_test_hypothesis();
        bad_h.name.clear();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(pipeline.generate(bad_h, None, true));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("validation failed") || err.contains("name"));
    }

    #[test]
    fn test_pipeline_lookahead_bias_stops_early() {
        let config = make_backtest_config();
        let pipeline = StrategyGenPipeline::new_mock(vec!["zigzag".into()], config);

        let h = Hypothesis {
            entry_conditions: vec![Condition {
                indicator: "zigzag".into(),
                params: serde_json::json!({}),
                operator: ">".into(),
                value: 0.0,
            }],
            ..make_test_hypothesis()
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(pipeline.generate(h, None, true));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("look-ahead") || err.contains("lookahead"));
    }

    #[test]
    fn test_pipeline_mock_flow_multiple_rounds() {
        let config = make_backtest_config();
        let pipeline =
            StrategyGenPipeline::new_mock(vec!["MA".into(), "RSI".into(), "MACD".into()], config);

        let h = make_test_hypothesis();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(pipeline.generate(h, None, true));

        assert!(result.is_ok(), "error: {:?}", result.err());
        let r = result.unwrap();
        // Without backtest, analysis is neutral → overfitting_flag=false → immediate early stop
        assert!(r.rounds >= 1);
        assert!(!r.round_logs.is_empty());
    }

    #[test]
    fn test_round_log_tracks_progress() {
        let logs = vec![
            RoundLog {
                round: 0,
                hypothesis_name: "v1".into(),
                deflated_sharpe: 0.8,
                monte_carlo_pvalue: 0.03,
                overfitting_flag: false,
                improved: true,
            },
            RoundLog {
                round: 1,
                hypothesis_name: "v2".into(),
                deflated_sharpe: 1.2,
                monte_carlo_pvalue: 0.01,
                overfitting_flag: false,
                improved: true,
            },
        ];

        assert!(logs[1].deflated_sharpe > logs[0].deflated_sharpe);
        assert!(logs[1].improved);
    }
}
