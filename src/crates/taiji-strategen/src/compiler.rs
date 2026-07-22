use std::path::Path;

use serde::{Deserialize, Serialize};
use taiji_engine::config::PipelineConfig;

use crate::hypothesis::Hypothesis;

/// Compiles a validated Hypothesis into a PipelineConfig YAML representation.
pub struct StrategyCompiler;

/// Serializable intermediate representation produced by the compiler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledStrategy {
    pub name: String,
    pub version: String,
    pub bar_gen: CompiledBarGen,
    pub data_source: CompiledDataSource,
    pub nodes: Vec<CompiledNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledBarGen {
    pub modes: Vec<String>,
    pub time_freqs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledDataSource {
    #[serde(rename = "type")]
    pub type_name: String,
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledNode {
    pub id: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub config: serde_json::Value,
    pub input_keys: Vec<String>,
    pub output_keys: Vec<String>,
}

impl StrategyCompiler {
    /// Compile a Hypothesis into a `serde_yaml::Value` representing a valid PipelineConfig.
    pub fn compile(hypothesis: &Hypothesis) -> Result<serde_yaml::Value, anyhow::Error> {
        let mut nodes: Vec<CompiledNode> = Vec::new();

        // Entry signal node
        if !hypothesis.entry_conditions.is_empty() {
            let entry_config = build_condition_config("entry", &hypothesis.entry_conditions);
            nodes.push(CompiledNode {
                id: "entry_signal".into(),
                type_name: "signal_generator".into(),
                config: entry_config,
                input_keys: vec!["bars:1m".into()],
                output_keys: vec!["signals:entry".into()],
            });
        }

        // Exit signal node
        if !hypothesis.exit_conditions.is_empty() {
            let exit_config = build_condition_config("exit", &hypothesis.exit_conditions);
            nodes.push(CompiledNode {
                id: "exit_signal".into(),
                type_name: "signal_generator".into(),
                config: exit_config,
                input_keys: vec!["bars:1m".into(), "signals:entry".into()],
                output_keys: vec!["signals:exit".into()],
            });
        }

        // Risk manager node
        let mut risk_config = serde_json::Map::new();
        risk_config.insert(
            "position_sizing".into(),
            serde_json::json!({
                "method": hypothesis.position_sizing.method,
                "value": hypothesis.position_sizing.value,
            }),
        );
        if let Some(sl) = hypothesis.risk_params.stop_loss {
            risk_config.insert("stop_loss".into(), serde_json::json!(sl));
        }
        if let Some(tp) = hypothesis.risk_params.take_profit {
            risk_config.insert("take_profit".into(), serde_json::json!(tp));
        }
        risk_config.insert(
            "max_holding_bars".into(),
            serde_json::json!(hypothesis.risk_params.max_holding_bars),
        );
        if let Some(md) = hypothesis.risk_params.max_drawdown {
            risk_config.insert("max_drawdown".into(), serde_json::json!(md));
        }

        // Collect input keys from upstream signal nodes
        let mut risk_input_keys: Vec<String> = vec!["bars:1m".into()];
        if !hypothesis.entry_conditions.is_empty() {
            risk_input_keys.push("signals:entry".into());
        }
        if !hypothesis.exit_conditions.is_empty() {
            risk_input_keys.push("signals:exit".into());
        }

        nodes.push(CompiledNode {
            id: "risk_manager".into(),
            type_name: "risk_manager".into(),
            config: serde_json::Value::Object(risk_config),
            input_keys: risk_input_keys,
            output_keys: vec!["signals:final".into()],
        });

        let compiled = CompiledStrategy {
            name: hypothesis.name.clone(),
            version: "1.0".into(),
            bar_gen: CompiledBarGen {
                modes: vec!["time".into()],
                time_freqs: vec!["1m".into()],
            },
            data_source: CompiledDataSource {
                type_name: "csv".into(),
                config: serde_json::json!({"instruments": hypothesis.instruments}),
            },
            nodes,
        };

        let yaml_value = serde_yaml::to_value(&compiled)?;
        Ok(yaml_value)
    }

    /// Compile a Hypothesis and write the PipelineConfig YAML to a file.
    pub fn compile_to_file(hypothesis: &Hypothesis, output: &Path) -> Result<(), anyhow::Error> {
        let yaml_value = Self::compile(hypothesis)?;
        let yaml_str = serde_yaml::to_string(&yaml_value)?;
        std::fs::write(output, yaml_str)?;
        Ok(())
    }

    /// Compile a Hypothesis directly into a PipelineConfig struct for in-memory use.
    pub fn compile_to_config(hypothesis: &Hypothesis) -> Result<PipelineConfig, anyhow::Error> {
        let yaml_value = Self::compile(hypothesis)?;
        let yaml_str = serde_yaml::to_string(&yaml_value)?;
        let config = PipelineConfig::from_yaml(&yaml_str)
            .map_err(|e| anyhow::anyhow!("Failed to parse compiled config: {}", e))?;
        Ok(config)
    }
}

/// Build a condition config JSON object from a list of conditions.
fn build_condition_config(
    direction: &str,
    conditions: &[crate::hypothesis::Condition],
) -> serde_json::Value {
    let conditions_json: Vec<serde_json::Value> = conditions
        .iter()
        .map(|c| {
            serde_json::json!({
                "indicator": c.indicator,
                "params": c.params,
                "operator": c.operator,
                "value": c.value,
            })
        })
        .collect();

    serde_json::json!({
        "direction": direction,
        "conditions": conditions_json,
    })
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hypothesis::{Condition, Hypothesis, PositionSizing, RiskParams};

    fn make_ma_cross_hypothesis() -> Hypothesis {
        Hypothesis {
            name: "ma_cross_test".into(),
            description: "MA cross strategy".into(),
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

    #[test]
    fn test_compile_produces_valid_yaml() {
        let h = make_ma_cross_hypothesis();
        let yaml_value = StrategyCompiler::compile(&h).expect("compile should succeed");
        let yaml_str = serde_yaml::to_string(&yaml_value).expect("serialize to YAML");

        // Verify the YAML contains expected structure
        assert!(yaml_str.contains("name:"));
        assert!(yaml_str.contains("ma_cross_test"));
        assert!(yaml_str.contains("entry_signal"));
        assert!(yaml_str.contains("exit_signal"));
        assert!(yaml_str.contains("risk_manager"));
        assert!(yaml_str.contains("1m"));
        assert!(yaml_str.contains("rb9999"));
    }

    #[test]
    fn test_compile_to_config_roundtrip() {
        let h = make_ma_cross_hypothesis();
        let config = StrategyCompiler::compile_to_config(&h).expect("compile_to_config");

        assert_eq!(config.name, "ma_cross_test");
        assert_eq!(config.version, "1.0");
        assert_eq!(config.bar_gen.time_freqs, vec!["1m"]);
        assert_eq!(config.nodes.len(), 3); // entry + exit + risk_manager

        // Verify node IDs
        let ids: Vec<&str> = config.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"entry_signal"));
        assert!(ids.contains(&"exit_signal"));
        assert!(ids.contains(&"risk_manager"));

        // Validate the config
        config.validate().expect("compiled config should be valid");
    }

    #[test]
    fn test_compile_to_config_passes_pipeline_validation() {
        let h = make_ma_cross_hypothesis();
        let config = StrategyCompiler::compile_to_config(&h).expect("compile_to_config");

        // Full PipelineConfig validation
        config
            .validate()
            .expect("PipelineConfig::validate should pass");
    }

    #[test]
    fn test_compile_to_file() {
        let h = make_ma_cross_hypothesis();
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test_pipeline.yaml");

        StrategyCompiler::compile_to_file(&h, &path).expect("compile_to_file");

        let content = std::fs::read_to_string(&path).expect("read file");
        assert!(content.contains("ma_cross_test"));
        assert!(content.contains("entry_signal"));
    }

    #[test]
    fn test_entry_only_hypothesis_no_exit_node() {
        let h = Hypothesis {
            exit_conditions: vec![],
            ..make_ma_cross_hypothesis()
        };
        let config = StrategyCompiler::compile_to_config(&h).expect("compile");
        let ids: Vec<&str> = config.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"entry_signal"));
        assert!(ids.contains(&"risk_manager"));
        assert!(!ids.contains(&"exit_signal"));
    }
}
