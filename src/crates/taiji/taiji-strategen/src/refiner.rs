use std::sync::Arc;

use taiji_llm::{ChatMessage, ChatResponse, LlmClient, LlmConfig};

use crate::analyzer::AnalysisReport;
use crate::hypothesis::Hypothesis;

/// Refines a strategy hypothesis using LLM feedback based on backtest analysis.
pub struct HypothesisRefiner {
    pub(crate) llm_client: Arc<dyn LlmClient>,
    pub(crate) max_rounds: usize,
}

impl HypothesisRefiner {
    pub fn new(llm_client: Arc<dyn LlmClient>, max_rounds: usize) -> Self {
        Self {
            llm_client,
            max_rounds,
        }
    }

    /// Refine a hypothesis based on backtest analysis report.
    ///
    /// Sends the current hypothesis and analysis to the LLM, asking it to
    /// suggest improvements. Returns an improved hypothesis.
    pub async fn refine(
        &self,
        hypothesis: &Hypothesis,
        report: &AnalysisReport,
    ) -> Result<Hypothesis, anyhow::Error> {
        let prompt = build_refinement_prompt(hypothesis, report);

        let messages = vec![
            ChatMessage::system(
                "You are a quantitative strategy optimizer. \
                 Output ONLY valid JSON matching the requested schema. \
                 No explanation, no markdown, just the JSON object.",
            ),
            ChatMessage::user(&prompt),
        ];

        let config = LlmConfig {
            model: "gpt-4o".into(),
            temperature: 0.3,
            ..Default::default()
        };

        let response = self.llm_client.chat(&messages, &config).await?;
        let refined = parse_hypothesis_from_response(&response)?;

        Ok(refined)
    }

    pub fn max_rounds(&self) -> usize {
        self.max_rounds
    }

    /// Refine using a mock LLM that applies simple heuristic improvements.
    /// For testing only — does not require a real LLM connection.
    pub fn refine_mock(
        hypothesis: &Hypothesis,
        report: &AnalysisReport,
    ) -> Result<Hypothesis, String> {
        let mut refined = hypothesis.clone();

        if report.overfitting_flag {
            // Reduce complexity: remove the last entry condition if > 1
            if refined.entry_conditions.len() > 1 {
                refined.entry_conditions.pop();
            }
            // Tighten risk params
            if let Some(ref mut sl) = refined.risk_params.stop_loss {
                *sl *= 1.2; // widen stop loss
            }
        } else if report.deflated_sharpe > 1.0 {
            // Good performance: try adding a filter condition
            if refined.entry_conditions.len() < 5 {
                let new_cond = crate::hypothesis::Condition {
                    indicator: "RSI".into(),
                    params: serde_json::json!({"period": 14}),
                    operator: "<".into(),
                    value: 30.0,
                };
                refined.entry_conditions.push(new_cond);
            }
        } else {
            // Marginal: tweak condition thresholds slightly
            for cond in &mut refined.entry_conditions {
                cond.value *= 1.05;
            }
        }

        // Append refinement marker to name
        refined.name = format!("{}_v2", refined.name);

        Ok(refined)
    }
}

/// Build a refinement prompt for the LLM.
fn build_refinement_prompt(hypothesis: &Hypothesis, report: &AnalysisReport) -> String {
    let hypothesis_json = serde_json::to_string_pretty(hypothesis).unwrap_or_else(|_| "{}".into());

    format!(
        r#"You are optimizing a trading strategy. Here is the current hypothesis and backtest results.

## Current Hypothesis
```json
{}
```

## Backtest Analysis
- Deflated Sharpe Ratio: {:.4}
- Monte Carlo p-value: {:.4}
- Walk-Forward Robustness: {:.4}
- Overfitting Flag: {}

## Task
Suggest specific improvements to the hypothesis. You may:
- Adjust indicator parameters (e.g., MA period, RSI threshold)
- Add or remove entry/exit conditions (max 5 entry, max 8 total)
- Modify position sizing or risk parameters
- Change instruments

Output ONLY a valid JSON object with the same structure as the input hypothesis.
The JSON must have these fields: name, description, entry_conditions, exit_conditions, position_sizing, risk_params, instruments.

Each condition has: indicator, params, operator, value.
Position sizing has: method (fixed/kelly/risk_percent), value.
Risk params has: stop_loss (optional), take_profit (optional), max_holding_bars, max_drawdown (optional).
"#,
        hypothesis_json,
        report.deflated_sharpe,
        report.monte_carlo_pvalue,
        report.walk_forward_robustness,
        report.overfitting_flag,
    )
}

/// Parse a Hypothesis from LLM chat response JSON.
fn parse_hypothesis_from_response(response: &ChatResponse) -> Result<Hypothesis, anyhow::Error> {
    let content = response.content.trim();

    // Strip markdown code fences if present
    let json_str = if content.starts_with("```") {
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() >= 3 {
            lines[1..lines.len() - 1].join("\n")
        } else {
            content.to_string()
        }
    } else {
        content.to_string()
    };

    let hypothesis: Hypothesis = serde_json::from_str(&json_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse refined hypothesis JSON: {}", e))?;

    Ok(hypothesis)
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::AnalysisReport;
    use crate::hypothesis::{Condition, Hypothesis, PositionSizing, RiskParams};

    fn make_test_hypothesis() -> Hypothesis {
        Hypothesis {
            name: "test_strategy".into(),
            description: "Test strategy".into(),
            entry_conditions: vec![
                Condition {
                    indicator: "MA".into(),
                    params: serde_json::json!({"period": 5}),
                    operator: "cross_above".into(),
                    value: 0.0,
                },
                Condition {
                    indicator: "RSI".into(),
                    params: serde_json::json!({"period": 14}),
                    operator: "<".into(),
                    value: 30.0,
                },
            ],
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

    fn make_overfit_report() -> AnalysisReport {
        AnalysisReport {
            deflated_sharpe: 0.3,
            monte_carlo_pvalue: 0.12,
            walk_forward_robustness: -0.5,
            overfitting_flag: true,
        }
    }

    fn make_good_report() -> AnalysisReport {
        AnalysisReport {
            deflated_sharpe: 1.5,
            monte_carlo_pvalue: 0.02,
            walk_forward_robustness: -0.1,
            overfitting_flag: false,
        }
    }

    #[test]
    fn test_refine_mock_overfit_removes_condition() {
        let h = make_test_hypothesis();
        let report = make_overfit_report();
        let refined = HypothesisRefiner::refine_mock(&h, &report).expect("refine_mock");

        assert_eq!(refined.entry_conditions.len(), 1);
        assert!(refined.name.contains("_v2"));
        // Stop loss should be widened
        let new_sl = refined.risk_params.stop_loss.unwrap();
        assert!(new_sl > h.risk_params.stop_loss.unwrap());
    }

    #[test]
    fn test_refine_mock_good_performance_adds_condition() {
        let h = make_test_hypothesis();
        let report = make_good_report();
        let refined = HypothesisRefiner::refine_mock(&h, &report).expect("refine_mock");

        // Should have added a condition (from 2 → 3)
        assert_eq!(refined.entry_conditions.len(), 3);
        assert!(refined.name.contains("_v2"));
    }

    #[test]
    fn test_refine_mock_at_max_conditions_no_add() {
        let mut h = make_test_hypothesis();
        // Fill to 5 entry conditions
        h.entry_conditions = (0..5)
            .map(|i| Condition {
                indicator: format!("IND{}", i),
                params: serde_json::json!({}),
                operator: ">".into(),
                value: i as f64,
            })
            .collect();

        let report = make_good_report();
        let refined = HypothesisRefiner::refine_mock(&h, &report).expect("refine_mock");

        // Should not exceed 5
        assert_eq!(refined.entry_conditions.len(), 5);
    }

    #[test]
    fn test_parse_hypothesis_from_json_response() {
        let h = make_test_hypothesis();
        let json_str = serde_json::to_string(&h).unwrap();
        let response = ChatResponse {
            content: json_str,
            usage: taiji_llm::Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
            finish_reason: "stop".into(),
        };
        let parsed = parse_hypothesis_from_response(&response).expect("parse");
        assert_eq!(parsed.name, "test_strategy");
        assert_eq!(parsed.entry_conditions.len(), 2);
    }

    #[test]
    fn test_parse_hypothesis_from_code_fence() {
        let h = make_test_hypothesis();
        let json_str = serde_json::to_string(&h).unwrap();
        let content = format!("```json\n{}\n```", json_str);
        let response = ChatResponse {
            content,
            usage: taiji_llm::Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
            finish_reason: "stop".into(),
        };
        let parsed = parse_hypothesis_from_response(&response).expect("parse");
        assert_eq!(parsed.name, "test_strategy");
    }
}
