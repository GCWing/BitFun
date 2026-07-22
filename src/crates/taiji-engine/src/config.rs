use crate::error::{Result, TaijiError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarGenConfig {
    pub modes: Vec<String>,      // ["time", "volume", "range"]
    pub time_freqs: Vec<String>, // ["1m", "5m", "15m", "30m", "1h", "1d"]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceSpec {
    #[serde(rename = "type")]
    pub type_name: String,
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSpec {
    pub id: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub config: serde_json::Value,
    pub input_keys: Vec<String>,
    pub output_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub name: String,
    pub version: String,
    pub bar_gen: BarGenConfig,
    pub data_source: DataSourceSpec,
    pub nodes: Vec<NodeSpec>,
}

impl PipelineConfig {
    /// Deserialize PipelineConfig from a YAML string.
    pub fn from_yaml(yaml_str: &str) -> Result<Self> {
        serde_yaml::from_str(yaml_str).map_err(|e| TaijiError::Config(e.to_string()))
    }

    /// Validation: cycle detection, key existence, required fields.
    pub fn validate(&self) -> Result<()> {
        let mut errors: Vec<String> = Vec::new();

        // Required field check
        if self.name.is_empty() {
            errors.push("pipeline.name is required".into());
        }
        // data_source.type allows "none" — Phase 3 feed_tick_direct() mode does not use DataSource
        if self.data_source.type_name.is_empty() {
            errors.push("data_source.type is required".into());
        }
        if self.nodes.is_empty() {
            errors.push("at least one node is required".into());
        }

        // Key existence check: every input_key must have a corresponding output_key source.
        // Pipeline-internal keys (e.g., bars:*, signals:*) are maintained by the Pipeline itself
        // and do not require a ComputeNode output_keys declaration.
        const PIPELINE_INTERNAL_PREFIXES: &[&str] = &["bars:", "signals:"];

        let all_output_keys: std::collections::HashSet<&str> = self
            .nodes
            .iter()
            .flat_map(|n| n.output_keys.iter().map(|s| s.as_str()))
            .collect();

        for node in &self.nodes {
            for input_key in &node.input_keys {
                // Skip Pipeline-internal keys
                if PIPELINE_INTERNAL_PREFIXES
                    .iter()
                    .any(|p| input_key.starts_with(p))
                {
                    continue;
                }
                if !all_output_keys.contains(input_key.as_str()) {
                    errors.push(format!(
                        "node '{}' requires key '{}' but no node produces it",
                        node.id, input_key
                    ));
                }
            }
        }

        // Node ID uniqueness
        let mut ids = std::collections::HashSet::new();
        for node in &self.nodes {
            if !ids.insert(&node.id) {
                errors.push(format!("duplicate node id: '{}'", node.id));
            }
        }

        if !errors.is_empty() {
            return Err(TaijiError::Config(errors.join("; ")));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_config() {
        let config = PipelineConfig {
            name: "test".into(),
            version: "1.0".into(),
            bar_gen: BarGenConfig {
                modes: vec!["time".into()],
                time_freqs: vec!["1m".into()],
            },
            data_source: DataSourceSpec {
                type_name: "ctp".into(),
                config: serde_json::json!({}),
            },
            nodes: vec![NodeSpec {
                id: "n1".into(),
                type_name: "test".into(),
                config: serde_json::json!({}),
                input_keys: vec![],
                output_keys: vec!["out1".into()],
            }],
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_missing_key() {
        let config = PipelineConfig {
            name: "test".into(),
            version: "1.0".into(),
            bar_gen: BarGenConfig {
                modes: vec![],
                time_freqs: vec![],
            },
            data_source: DataSourceSpec {
                type_name: "ctp".into(),
                config: serde_json::json!({}),
            },
            nodes: vec![NodeSpec {
                id: "n1".into(),
                type_name: "test".into(),
                config: serde_json::json!({}),
                input_keys: vec!["missing_key".into()],
                output_keys: vec![],
            }],
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_empty_nodes() {
        let config = PipelineConfig {
            name: "test".into(),
            version: "1.0".into(),
            bar_gen: BarGenConfig {
                modes: vec![],
                time_freqs: vec![],
            },
            data_source: DataSourceSpec {
                type_name: "ctp".into(),
                config: serde_json::json!({}),
            },
            nodes: vec![],
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_from_yaml_example_pipeline() {
        let yaml_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../examples/example-pipeline.yaml"
        );
        let yaml_str = std::fs::read_to_string(yaml_path).expect("read example-pipeline.yaml");
        let config = PipelineConfig::from_yaml(&yaml_str).expect("parse YAML");
        assert_eq!(config.name, "example-ma-cross");
        assert_eq!(config.version, "1.0");
        assert_eq!(config.bar_gen.time_freqs, vec!["1m"]);
        assert_eq!(config.nodes.len(), 1);
        assert_eq!(config.nodes[0].id, "ma_cross");
        assert_eq!(config.nodes[0].type_name, "ma_cross");
        assert_eq!(
            config.nodes[0]
                .config
                .get("fast_period")
                .unwrap()
                .as_i64()
                .unwrap(),
            5
        );
        assert_eq!(
            config.nodes[0]
                .config
                .get("slow_period")
                .unwrap()
                .as_i64()
                .unwrap(),
            20
        );
        config.validate().expect("valid config");
    }
}
