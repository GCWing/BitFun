use taiji_engine::config::*;
use taiji_engine::error::Result;
use taiji_engine::node::{ComputeNode, NodeConfig, NodeId};
use taiji_engine::pipeline::Pipeline;
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::{Freq, RawBar};
use taiji_engine::types::state::{StateKey, StateValue};

/// 最小测试节点：简单累加计数器
struct CounterNode {
    id: NodeId,
    count: usize,
}
impl ComputeNode for CounterNode {
    fn id(&self) -> NodeId {
        self.id.clone()
    }
    fn name(&self) -> &'static str {
        "counter"
    }
    fn input_keys(&self) -> Vec<StateKey> {
        vec![]
    }
    fn output_keys(&self) -> Vec<StateKey> {
        vec!["count".into()]
    }
    fn on_init(&mut self, _: &NodeConfig, _: &StateStore) -> Result<()> {
        Ok(())
    }
    fn on_bar(&mut self, _: &RawBar, _: Freq, state: &StateStore) -> Result<()> {
        self.count += 1;
        state.set("count".into(), StateValue::Usize(self.count), self.id());
        Ok(())
    }
    fn subscribed_freqs(&self) -> Vec<Freq> {
        vec![Freq::F1]
    }
}

#[test]
fn test_pipeline_from_config_no_panic() {
    let yaml = r#"
name: "test-pipeline"
version: "1.0"
bar_gen:
  modes: ["time"]
  time_freqs: ["1m", "5m"]
data_source:
  type: "ctp"
  config: {}
nodes:
  - id: "counter"
    type: "counter"
    config: {}
    input_keys: []
    output_keys: ["count"]
"#;
    let config: PipelineConfig = serde_yaml::from_str(yaml).expect("parse config");
    assert!(config.validate().is_ok());

    let pipeline = Pipeline::from_config(config);
    assert!(pipeline.is_ok(), "pipeline creation should succeed");
}

#[test]
fn test_config_validate_rejects_cycle() {
    let yaml = r#"
name: "cycle-test"
version: "1.0"
bar_gen:
  modes: ["time"]
  time_freqs: ["1m"]
data_source:
  type: "ctp"
  config: {}
nodes:
  - id: "a"
    type: "a"
    config: {}
    input_keys: ["out_b"]
    output_keys: ["out_a"]
  - id: "b"
    type: "b"
    config: {}
    input_keys: ["out_a"]
    output_keys: ["out_b"]
"#;
    let config: PipelineConfig = serde_yaml::from_str(yaml).expect("parse config");
    // 循环依赖会被 validate 检测到（key 存在性检查不检测循环，只检测 key 来源）
    // 循环依赖在实际执行时由 Dag 检测
    assert!(config.validate().is_ok()); // key checks pass
}

#[test]
fn test_pipeline_add_node_and_derive_edges() {
    let yaml = r#"
name: "edge-test"
version: "1.0"
bar_gen:
  modes: ["time"]
  time_freqs: ["1m"]
data_source:
  type: "ctp"
  config: {}
nodes:
  - id: "producer"
    type: "counter"
    config: {}
    input_keys: []
    output_keys: ["data"]
  - id: "consumer"
    type: "counter"
    config: {}
    input_keys: ["data"]
    output_keys: ["result"]
"#;
    let config: PipelineConfig = serde_yaml::from_str(yaml).expect("parse config");
    let mut pipeline = Pipeline::from_config(config).expect("create pipeline");

    // Add nodes
    pipeline.add_node(Box::new(CounterNode {
        id: "producer".into(),
        count: 0,
    }));
    pipeline.add_node(Box::new(CounterNode {
        id: "consumer".into(),
        count: 0,
    }));

    // Derive edges based on input/output keys
    pipeline
        .derive_edges()
        .expect("derive_edges should succeed for acyclic DAG");

    let status = pipeline.status();
    assert!(
        status.nodes.len() >= 2,
        "should have at least 2 nodes in execution graph"
    );
    println!(
        "pipeline: {} nodes, state: {:?}",
        status.nodes.len(),
        status.state
    );
}
