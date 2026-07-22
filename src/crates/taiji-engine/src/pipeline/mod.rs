//! Pipeline module

pub mod bar_gen;
pub mod reorg;
pub mod status;

use crate::config::PipelineConfig;
use crate::dag::Dag;
use crate::error::{Result, TaijiError};
use crate::factory::NodeFactory;
use crate::node::{ComputeNode, NodeConfig, NodeId};
use crate::pipeline::bar_gen::{AggMode, BarGenerator};
use crate::risk::{OrderDecision, RiskMonitor, RiskOrderRequest};
use crate::source::datasource::DataSource;
use crate::store::StateStore;
use crate::types::bar::{Freq, RawBar};
use crate::types::signal::Signal;
use crate::types::state::{SixCoreMetrics, StateValue};
use crate::types::tick::TickData;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::panic::{self, AssertUnwindSafe};
use tracing::{error, warn};

/// Node execution status
#[derive(Debug, Clone)]
pub struct NodeExecutionStatus {
    pub node_id: NodeId,
    pub executed: bool,
    pub signals_emitted: usize,
    pub error: Option<String>,
}

/// Return value of feed_tick
#[derive(Debug, Clone)]
pub struct TickResult {
    pub closed_bars: Vec<(Freq, RawBar)>,
    pub signals: Vec<Signal>,
    pub node_statuses: Vec<NodeExecutionStatus>,
}

impl TickResult {
    pub fn empty() -> Self {
        Self {
            closed_bars: Vec::new(),
            signals: Vec::new(),
            node_statuses: Vec::new(),
        }
    }
}

type NodeMap = Arc<RwLock<HashMap<NodeId, Arc<Mutex<Box<dyn ComputeNode>>>>>>;
type NodeEntry = (NodeId, Arc<Mutex<Box<dyn ComputeNode>>>);

/// Pipeline: DAG execution engine
#[allow(dead_code)]
pub struct Pipeline {
    data_source: Option<Box<dyn DataSource>>,
    node_factory: NodeFactory,
    nodes: NodeMap,
    state: Arc<StateStore>,
    execution_layers: Arc<RwLock<Vec<Vec<NodeId>>>>,
    bar_gen: Arc<RwLock<Option<crate::pipeline::bar_gen::BarGenerator>>>,
    config: Arc<RwLock<PipelineConfig>>,
    risk_monitor: Option<Box<dyn RiskMonitor>>,
}

impl Pipeline {
    pub fn from_config(config: PipelineConfig) -> Result<Self> {
        config.validate()?;

        let time_freqs: Vec<Freq> = config
            .bar_gen
            .time_freqs
            .iter()
            .filter_map(|s| match s.as_str() {
                "1m" => Some(Freq::F1),
                "5m" => Some(Freq::F5),
                "15m" => Some(Freq::F15),
                "30m" => Some(Freq::F30),
                "1h" => Some(Freq::F60),
                "1d" => Some(Freq::D),
                _ => None,
            })
            .collect();

        let modes: Vec<AggMode> = config
            .bar_gen
            .modes
            .iter()
            .filter_map(|s| match s.as_str() {
                "time" => Some(AggMode::Time),
                "volume" => Some(AggMode::Volume),
                "range" => Some(AggMode::Range),
                _ => None,
            })
            .collect();

        let bar_gen = if !time_freqs.is_empty() {
            Some(BarGenerator::new("default".into(), modes, time_freqs))
        } else {
            None
        };
        let factory = NodeFactory::new();

        Ok(Self {
            data_source: None,
            node_factory: factory,
            nodes: Arc::new(RwLock::new(HashMap::new())),
            state: Arc::new(StateStore::new()),
            execution_layers: Arc::new(RwLock::new(Vec::new())),
            bar_gen: Arc::new(RwLock::new(bar_gen)),
            config: Arc::new(RwLock::new(config)),
            risk_monitor: None,
        })
    }

    /// Set data source
    pub fn set_data_source(&mut self, ds: Box<dyn DataSource>) {
        self.data_source = Some(ds);
    }

    /// Set risk monitor
    pub fn set_risk_monitor(&mut self, monitor: Box<dyn RiskMonitor>) {
        self.risk_monitor = Some(monitor);
    }

    /// Derive edges by matching input_keys/output_keys
    pub fn derive_edges(&self) -> Result<()> {
        let nodes = self.nodes.read();
        let mut dag = Dag::new();
        for node_arc in nodes.values() {
            dag.add_node(node_arc.lock().id());
        }
        for node_a_arc in nodes.values() {
            let node_a = node_a_arc.lock();
            for out_key in node_a.output_keys() {
                for node_b_arc in nodes.values() {
                    if Arc::ptr_eq(node_a_arc, node_b_arc) {
                        continue;
                    }
                    let node_b = node_b_arc.lock();
                    if node_b.input_keys().contains(&out_key) {
                        dag.add_edge(node_a.id(), node_b.id());
                    }
                }
            }
        }

        match dag.sort() {
            Ok(layers) => {
                *self.execution_layers.write() = layers;
                Ok(())
            }
            Err(cycle_nodes) => {
                tracing::error!("circular dependency detected in DAG: {:?}", cycle_nodes);
                Err(TaijiError::CycleDetected(cycle_nodes))
            }
        }
    }

    /// Main loop: RawTick → SchemaAdapter → TickValidator → BarGenerator → DAG
    pub fn feed_tick(&mut self) -> Result<TickResult> {
        // 1. Get tick from data source
        let raw_tick = match &mut self.data_source {
            Some(ds) => match ds.next_raw()? {
                Some(t) => t,
                None => return Ok(TickResult::empty()),
            },
            None => return Ok(TickResult::empty()),
        };

        // 2. SchemaAdapter: map RawTick fields to normalized TickData
        let tick = TickData {
            instrument: raw_tick.instrument.clone(),
            timestamp_ms: raw_tick.timestamp,
            last_price: raw_tick.fields.get("price").copied().unwrap_or_else(|| {
                warn!("Required field 'price' missing from tick fields");
                0.0
            }),
            open_price: raw_tick.fields.get("open").copied().unwrap_or(0.0),
            highest_price: raw_tick.fields.get("high").copied().unwrap_or(0.0),
            lowest_price: raw_tick.fields.get("low").copied().unwrap_or(0.0),
            volume: raw_tick
                .fields
                .get("cum_volume")
                .or_else(|| raw_tick.fields.get("volume"))
                .copied()
                .unwrap_or_else(|| {
                    warn!("Required field 'volume' missing from tick fields");
                    0.0
                }),
            turnover: raw_tick
                .fields
                .get("cum_amount")
                .or_else(|| raw_tick.fields.get("amount"))
                .copied()
                .unwrap_or(0.0),
            open_interest: raw_tick
                .fields
                .get("cum_position")
                .or_else(|| raw_tick.fields.get("open_interest"))
                .copied()
                .unwrap_or(0.0),
            trade_type: raw_tick.fields.get("trade_type").copied(),
            bid_price1: raw_tick
                .fields
                .get("bid_p")
                .or_else(|| raw_tick.fields.get("bid_price1"))
                .copied()
                .unwrap_or(0.0),
            ask_price1: raw_tick
                .fields
                .get("ask_p")
                .or_else(|| raw_tick.fields.get("ask_price1"))
                .copied()
                .unwrap_or(0.0),
            ..Default::default()
        };

        self.process_tick(&tick)
    }

    /// Feed TickData directly (skip DataSource + SchemaAdapter).
    /// Used for Tauri commands and CSV replay scenarios — tick has already been parsed into TickData externally.
    pub fn feed_tick_direct(&mut self, tick: &TickData) -> Result<TickResult> {
        self.process_tick(tick)
    }

    /// Internal method: BarGenerator + DAG execution (shared by feed_tick / feed_tick_direct).
    fn process_tick(&mut self, tick: &TickData) -> Result<TickResult> {
        let mut result = TickResult::empty();

        // 1. BarGenerator
        if let Some(ref mut bg) = *self.bar_gen.write() {
            result.closed_bars = bg.update_tick(tick);
        }

        // 2. DAG execution
        self.execute_dag(result)
    }

    /// Execute DAG topological layers for each closed bar.
    /// Same-layer nodes run in parallel via thread::scope; filtering is sequential per layer.
    fn execute_dag(&mut self, mut result: TickResult) -> Result<TickResult> {
        for (freq, bar) in &result.closed_bars {
            // Append mode: read existing bars, append new bar (do not overwrite history)
            let key = format!("bars:{}", freq.freq_key());
            let existing: Option<Arc<Vec<Arc<RawBar>>>> = self.state.get(&key);
            let mut bars: Vec<Arc<RawBar>> = existing.map(|arc| (*arc).clone()).unwrap_or_default();
            bars.push(Arc::new(bar.clone()));
            self.state
                .set(key, StateValue::Bars(Arc::new(bars)), "bar_gen".into());

            // Snapshot execution layers under read lock, then release before spawning threads
            let layers: Vec<Vec<NodeId>> = self.execution_layers.read().clone();

            for layer in &layers {
                // Collect per-node results: (node_id, executed, signals, error)
                let layer_results: Vec<(NodeId, bool, Vec<Signal>, Option<String>)> =
                    if layer.len() == 1 {
                        // Sequential fast-path for single-node layers
                        let node_id = &layer[0];
                        let node_arc = self.nodes.read().get(node_id).cloned();
                        vec![Self::run_single_node(
                            node_arc.as_ref(),
                            node_id,
                            bar,
                            *freq,
                            &self.state,
                        )]
                    } else {
                        // Parallel execution for multi-node layers (same-layer nodes are independent)
                        let nodes_snapshot: Vec<NodeEntry> = {
                            let nodes = self.nodes.read();
                            layer
                                .iter()
                                .filter_map(|nid| {
                                    nodes.get(nid).map(|arc| (nid.clone(), Arc::clone(arc)))
                                })
                                .collect()
                        };
                        std::thread::scope(|s| {
                            let mut handles = Vec::new();
                            for (nid, node_arc) in &nodes_snapshot {
                                let node_arc = Arc::clone(node_arc);
                                let state = Arc::clone(&self.state);
                                let bar = bar.clone();
                                let freq = *freq;
                                let nid_clone = nid.clone();
                                handles.push((
                                    nid_clone.clone(),
                                    s.spawn(move || {
                                        Self::run_single_node(
                                            Some(&node_arc),
                                            &nid_clone,
                                            &bar,
                                            freq,
                                            &state,
                                        )
                                    }),
                                ));
                            }
                            handles
                                .into_iter()
                                .map(|(nid, h)| {
                                    let (nid2, executed, signals, error) = h.join().unwrap();
                                    debug_assert_eq!(nid, nid2);
                                    (nid, executed, signals, error)
                                })
                                .collect()
                        })
                    };

                // Sequential signal filtering & storage (after parallel execution)
                for (node_id, executed, signals, error) in layer_results {
                    if let Some(err) = error {
                        result.node_statuses.push(NodeExecutionStatus {
                            node_id,
                            executed: false,
                            signals_emitted: 0,
                            error: Some(err),
                        });
                        continue;
                    }
                    let filtered = if executed {
                        self.filter_signals(signals, bar)
                    } else {
                        vec![]
                    };
                    let count = filtered.len();
                    if !filtered.is_empty() {
                        self.state.set(
                            format!("signals:{}", node_id),
                            StateValue::Signals(Arc::new(filtered.clone())),
                            node_id.clone(),
                        );
                        result.signals.extend(filtered);
                    }
                    result.node_statuses.push(NodeExecutionStatus {
                        node_id,
                        executed,
                        signals_emitted: count,
                        error: None,
                    });
                }
            }
        }

        Ok(result)
    }

    /// Execute a single node (called from both sequential and parallel paths).
    fn run_single_node(
        node_arc: Option<&Arc<Mutex<Box<dyn ComputeNode>>>>,
        node_id: &NodeId,
        bar: &RawBar,
        freq: Freq,
        state: &StateStore,
    ) -> (NodeId, bool, Vec<Signal>, Option<String>) {
        let node_arc = match node_arc {
            Some(n) => n,
            None => {
                return (
                    node_id.clone(),
                    false,
                    vec![],
                    Some("node not found".into()),
                )
            }
        };
        let mut node = node_arc.lock();
        if !node.is_ready(state) {
            return (node_id.clone(), false, vec![], Some("not ready".into()));
        }

        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            match node.on_bar(bar, freq, state) {
                Ok(()) => match node.on_calculate(state) {
                    Ok(signals) => (node_id.clone(), true, signals, None::<String>),
                    Err(e) => (node_id.clone(), false, vec![], Some(e.to_string())),
                },
                Err(e) => (node_id.clone(), false, vec![], Some(e.to_string())),
            }
        }));

        match result {
            Ok(r) => r,
            Err(panic_payload) => {
                let msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                error!("node '{}' panicked: {}", node_id, msg);
                (
                    node_id.clone(),
                    false,
                    vec![],
                    Some(format!("panicked: {}", msg)),
                )
            }
        }
    }

    /// Run node signals through the RiskMonitor, filtering out rejected orders.
    fn filter_signals(&self, signals: Vec<Signal>, bar: &RawBar) -> Vec<Signal> {
        let monitor = match &self.risk_monitor {
            Some(m) => m,
            None => return signals,
        };

        signals
            .into_iter()
            .filter_map(|signal| {
                let order = RiskOrderRequest {
                    instrument: signal.instrument.clone(),
                    action: format!("{:?}", signal.action),
                    price: signal.entry.unwrap_or(bar.close),
                    volume: signal.size.unwrap_or(0.0),
                };
                match monitor.check_order(&order, &self.state) {
                    Ok(OrderDecision::Allow) => Some(signal),
                    Ok(OrderDecision::Reject(reason)) => {
                        tracing::warn!(
                            "RiskMonitor rejected signal from {}: {}",
                            signal.source,
                            reason
                        );
                        None
                    }
                    Ok(OrderDecision::Reduce(max_qty)) => {
                        let mut adjusted = signal.clone();
                        adjusted.size = Some(max_qty);
                        tracing::warn!(
                            "RiskMonitor reduced signal from {} to volume {}",
                            adjusted.source,
                            max_qty
                        );
                        Some(adjusted)
                    }
                    Err(e) => {
                        tracing::error!(
                            "RiskMonitor error on signal from {}: {}",
                            signal.source,
                            e
                        );
                        None
                    }
                }
            })
            .collect()
    }

    /// Add a node
    pub fn add_node(&mut self, node: Box<dyn ComputeNode>) {
        let id = node.id();
        self.nodes.write().insert(id, Arc::new(Mutex::new(node)));
    }

    /// Get completed bar sequence for a given frequency (from BarGenerator, read-only).
    pub fn bar_history(&self, freq: &Freq) -> Option<Vec<RawBar>> {
        self.bar_gen
            .read()
            .as_ref()
            .map(|bg| bg.bars(freq).to_vec())
    }

    /// Get bar history for all frequencies (for JSON export).
    pub fn all_bar_histories(&self) -> HashMap<Freq, Vec<RawBar>> {
        let mut map = HashMap::new();
        if let Some(ref bg) = *self.bar_gen.read() {
            for freq in bg.configured_freqs() {
                let bars = bg.bars(freq).to_vec();
                if !bars.is_empty() {
                    map.insert(*freq, bars);
                }
            }
        }
        map
    }

    /// Get a reference to the shared StateStore.
    pub fn state_store(&self) -> &StateStore {
        &self.state
    }

    /// Get an Arc clone of the shared StateStore (for cross-thread sharing).
    pub fn state_store_arc(&self) -> Arc<StateStore> {
        Arc::clone(&self.state)
    }

    /// Get status
    pub fn status(&self) -> status::PipelineStatus {
        let nodes: Vec<status::NodeStatus> = self
            .nodes
            .read()
            .values()
            .map(|node_arc| {
                let node = node_arc.lock();
                status::NodeStatus {
                    id: node.id(),
                    name: node.name().to_string(),
                    ready: node.is_ready(&self.state),
                    last_execution_ms: None,
                    signals_emitted: 0,
                    errors: 0,
                    state: status::NodeState::Idle,
                }
            })
            .collect();

        let pipeline_state =
            if self.data_source.is_none() || self.execution_layers.read().is_empty() {
                status::PipelineState::Initializing
            } else {
                status::PipelineState::Running
            };

        status::PipelineStatus {
            state: pipeline_state,
            nodes,
            uptime_secs: 0,
            total_ticks: 0,
            total_signals: 0,
            total_bars: 0,
        }
    }
}

// ── Phase 3 new methods ──

impl Pipeline {
    /// Serialize the content of the given keys in StateStore to a JSON Value.
    /// When keys is empty, export all keys.
    pub fn serialize_state(&self, keys: &[String]) -> serde_json::Value {
        let all_keys: Vec<String> = if keys.is_empty() {
            self.state.keys()
        } else {
            keys.to_vec()
        };

        let mut map = serde_json::Map::new();
        let mut instrument: Option<String> = None;
        let mut last_timestamp: Option<String> = None;
        let mut freq_label: Option<String> = None;

        for key in &all_keys {
            if let Some(value) = self.state.get_raw(key) {
                let json_val = serde_json::to_value(value.clone())
                    .unwrap_or_else(|_| serde_json::Value::String(format!("{:?}", value)));
                map.insert(key.clone(), json_val);

                // Extract metadata from bars
                if let StateValue::Bars(bars) = value {
                    if let Some(bar) = bars.last() {
                        instrument = Some(bar.symbol.to_string());
                        last_timestamp = Some(bar.dt.to_rfc3339());
                    }
                    if let Some(freq_part) = key.strip_prefix("bars:") {
                        freq_label = Some(freq_part.to_string());
                    }
                }
            }
        }

        if instrument.is_some() || last_timestamp.is_some() || freq_label.is_some() {
            let mut meta = serde_json::Map::new();
            if let Some(inst) = instrument {
                meta.insert("instrument".into(), serde_json::Value::String(inst));
            }
            if let Some(ts) = last_timestamp {
                meta.insert("timestamp".into(), serde_json::Value::String(ts));
            }
            if let Some(freq) = freq_label {
                meta.insert("freq".into(), serde_json::Value::String(freq));
            }
            map.insert("_meta".into(), serde_json::Value::Object(meta));
        }

        serde_json::Value::Object(map)
    }

    /// Compute six core metrics (long-open/short-open/long-close/short-close/net-long/net-short).
    /// Derived from the most recent t+1 bars in StateStore bars:{freq}.
    /// oi or delta is None → returns None.
    pub fn compute_six_core(&self, freq: Freq, t: usize) -> Option<SixCoreMetrics> {
        let key = format!("bars:{}", freq.freq_key());
        let bars: Arc<Vec<Arc<RawBar>>> = self.state.get(&key)?;

        let n = bars.len();
        if n <= t {
            return None;
        }

        // bars are sorted by time ascending, n-1 is the most recent bar
        let oi0 = bars[n - 1].open_interest?;
        let oi_t = bars[n - 1 - t].open_interest?;
        let oi_delta = oi0 - oi_t;

        let mut active_trade_diff = 0.0;
        let mut total_volume = 0.0;
        for i in (n - t)..n {
            active_trade_diff += bars[i].delta?;
            total_volume += bars[i].vol;
        }

        let long_open = (total_volume + oi_delta + active_trade_diff) / 2.0;
        let short_open = (total_volume + oi_delta - active_trade_diff) / 2.0;
        let long_close = (total_volume - oi_delta - active_trade_diff) / 2.0;
        let short_close = (total_volume - oi_delta + active_trade_diff) / 2.0;
        let net_long = oi_delta + active_trade_diff;
        let net_short = oi_delta - active_trade_diff;

        Some(SixCoreMetrics {
            oi_delta,
            active_trade_diff,
            total_volume,
            long_open,
            short_open,
            long_close,
            short_close,
            net_long,
            net_short,
        })
    }
}

// ── Phase 6 hot-reload methods ──

impl Pipeline {
    /// Register a node constructor for a given type name.
    /// Must be called before `swap_node` or `reload_config`.
    pub fn register_node_type(&mut self, type_name: &str, ctor: crate::factory::NodeConstructor) {
        self.node_factory.register(type_name, ctor);
    }

    /// Hot-swap a single node: replace the node identified by `node_id`
    /// with a new instance created via the registered NodeFactory.
    /// Does not interrupt ongoing `feed_tick` calls (protected by RwLock).
    pub fn swap_node(&self, node_id: &NodeId, new_config: NodeConfig) -> Result<()> {
        let type_name = new_config.type_name.clone();
        if type_name.is_empty() {
            return Err(TaijiError::Config(
                "swap_node: new_config.type_name is required".into(),
            ));
        }

        // 1. Create replacement node via factory
        let new_node = self.node_factory.create(&type_name, &new_config)?;
        let new_node_id = new_node.id();

        // 2. Atomic swap under write lock
        {
            let mut nodes = self.nodes.write();
            if !nodes.contains_key(node_id) {
                return Err(TaijiError::Config(format!(
                    "swap_node: node '{}' not found in pipeline",
                    node_id
                )));
            }
            nodes.insert(new_node_id.clone(), Arc::new(Mutex::new(new_node)));
        }

        // 3. Re-derive DAG edges (may change if input_keys/output_keys differ)
        self.derive_edges()?;

        tracing::info!(
            "swap_node: replaced node '{}' with new instance id='{}'",
            node_id,
            new_node_id
        );
        Ok(())
    }

    /// Reload pipeline configuration from a YAML file.
    /// Rebuilds bar_gen and nodes, but preserves the existing StateStore data.
    pub fn reload_config(&self, config_path: &Path) -> Result<()> {
        let yaml_str = std::fs::read_to_string(config_path).map_err(TaijiError::Io)?;
        let new_config = PipelineConfig::from_yaml(&yaml_str)?;

        // 1. Rebuild bar_gen
        let time_freqs: Vec<Freq> = new_config
            .bar_gen
            .time_freqs
            .iter()
            .filter_map(|s| match s.as_str() {
                "1m" => Some(Freq::F1),
                "5m" => Some(Freq::F5),
                "15m" => Some(Freq::F15),
                "30m" => Some(Freq::F30),
                "1h" => Some(Freq::F60),
                "1d" => Some(Freq::D),
                _ => None,
            })
            .collect();

        let modes: Vec<AggMode> = new_config
            .bar_gen
            .modes
            .iter()
            .filter_map(|s| match s.as_str() {
                "time" => Some(AggMode::Time),
                "volume" => Some(AggMode::Volume),
                "range" => Some(AggMode::Range),
                _ => None,
            })
            .collect();

        let new_bar_gen = if !time_freqs.is_empty() {
            Some(BarGenerator::new("default".into(), modes, time_freqs))
        } else {
            None
        };

        // 2. Rebuild nodes from config spec
        let mut new_nodes: HashMap<NodeId, Arc<Mutex<Box<dyn ComputeNode>>>> = HashMap::new();
        for spec in &new_config.nodes {
            let params: HashMap<String, serde_json::Value> =
                if let serde_json::Value::Object(map) = &spec.config {
                    map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
                } else {
                    HashMap::new()
                };

            let node_config = NodeConfig {
                type_name: spec.type_name.clone(),
                params,
            };
            let mut node = self
                .node_factory
                .create(&spec.type_name, &node_config)
                .map_err(|e| {
                    TaijiError::Config(format!(
                        "reload_config: failed to create node '{}' (type={}): {}",
                        spec.id, spec.type_name, e
                    ))
                })?;

            // Call on_init so node can re-read config params
            let store = StateStore::new();
            node.on_init(&node_config, &store).map_err(|e| {
                TaijiError::Config(format!(
                    "reload_config: failed to init node '{}': {}",
                    spec.id, e
                ))
            })?;

            new_nodes.insert(spec.id.clone(), Arc::new(Mutex::new(node)));
        }

        // 3. Atomically swap config + bar_gen + nodes (StateStore is preserved)
        *self.config.write() = new_config;
        *self.bar_gen.write() = new_bar_gen;
        *self.nodes.write() = new_nodes;

        // 4. Re-derive DAG edges
        self.derive_edges()?;

        tracing::info!("reload_config: pipeline configuration reloaded successfully");
        Ok(())
    }

    /// Return a clone of the current PipelineConfig.
    pub fn get_config(&self) -> PipelineConfig {
        self.config.read().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::bar::RawBar;
    use chrono::Utc;

    fn make_bar(oi: Option<f64>, delta: Option<f64>, vol: f64) -> RawBar {
        RawBar {
            symbol: "test".into(),
            dt: Utc::now(),
            freq: Freq::F1,
            id: 0,
            open: 0.0,
            high: 0.0,
            low: 0.0,
            close: 0.0,
            vol,
            amount: 0.0,
            open_interest: oi,
            delta,
        }
    }

    fn build_pipeline_with_bars(freq: Freq, bars: Vec<RawBar>) -> Pipeline {
        let config = crate::config::PipelineConfig {
            name: "test".into(),
            version: "1.0".into(),
            bar_gen: crate::config::BarGenConfig {
                modes: vec![],
                time_freqs: vec![],
            },
            data_source: crate::config::DataSourceSpec {
                type_name: "none".into(),
                config: serde_json::json!({}),
            },
            nodes: vec![crate::config::NodeSpec {
                id: "n1".into(),
                type_name: "dummy".into(),
                config: serde_json::json!({}),
                input_keys: vec![],
                output_keys: vec![],
            }],
        };
        let p = Pipeline::from_config(config).unwrap();
        let key = format!("bars:{}", freq.freq_key());
        p.state.set(
            key,
            StateValue::Bars(Arc::new(bars.into_iter().map(Arc::new).collect())),
            "test".into(),
        );
        p
    }

    #[test]
    fn test_compute_six_core_basic() {
        // bar[0] (newer): oi=110, delta=5, vol=200
        // bar[1] (older): oi=100, delta=3, vol=150
        let bar_old = make_bar(Some(100.0), Some(3.0), 150.0);
        let bar_new = make_bar(Some(110.0), Some(5.0), 200.0);
        let p = build_pipeline_with_bars(Freq::F1, vec![bar_old, bar_new]);

        let metrics = p.compute_six_core(Freq::F1, 1).unwrap();

        // 仓差 = 110 - 100 = 10
        assert!((metrics.oi_delta - 10.0).abs() < 1e-9);
        // 主动买卖差 = bar_new.delta = 5
        assert!((metrics.active_trade_diff - 5.0).abs() < 1e-9);
        // 总成交量 = bar_new.vol = 200
        assert!((metrics.total_volume - 200.0).abs() < 1e-9);
        // 多开 = (200 + 10 + 5) / 2 = 107.5
        assert!((metrics.long_open - 107.5).abs() < 1e-9);
        // 空开 = (200 + 10 - 5) / 2 = 102.5
        assert!((metrics.short_open - 102.5).abs() < 1e-9);
        // 多平 = (200 - 10 - 5) / 2 = 92.5
        assert!((metrics.long_close - 92.5).abs() < 1e-9);
        // 空平 = (200 - 10 + 5) / 2 = 97.5
        assert!((metrics.short_close - 97.5).abs() < 1e-9);
        // 净多 = 10 + 5 = 15
        assert!((metrics.net_long - 15.0).abs() < 1e-9);
        // 净空 = 10 - 5 = 5
        assert!((metrics.net_short - 5.0).abs() < 1e-9);
    }

    #[test]
    fn test_compute_six_core_oi_none_returns_none() {
        let bar_old = make_bar(Some(100.0), Some(3.0), 150.0);
        let bar_new = make_bar(None, Some(5.0), 200.0);
        let p = build_pipeline_with_bars(Freq::F1, vec![bar_old, bar_new]);

        assert!(p.compute_six_core(Freq::F1, 1).is_none());
    }

    #[test]
    fn test_compute_six_core_delta_none_returns_none() {
        let bar_old = make_bar(Some(100.0), Some(3.0), 150.0);
        let bar_new = make_bar(Some(110.0), None, 200.0);
        let p = build_pipeline_with_bars(Freq::F1, vec![bar_old, bar_new]);

        assert!(p.compute_six_core(Freq::F1, 1).is_none());
    }

    #[test]
    fn test_compute_six_core_insufficient_bars() {
        let bar = make_bar(Some(100.0), Some(3.0), 150.0);
        let p = build_pipeline_with_bars(Freq::F1, vec![bar]);

        // t=1 needs 2 bars, only 1 present
        assert!(p.compute_six_core(Freq::F1, 1).is_none());
    }

    #[test]
    fn test_serialize_state_all_keys() {
        let bar = make_bar(Some(100.0), Some(3.0), 150.0);
        let p = build_pipeline_with_bars(Freq::F1, vec![bar]);

        let json = p.serialize_state(&[]);
        let obj = json.as_object().unwrap();

        // Contains bars key and _meta
        assert!(obj.contains_key("bars:1m"));
        assert!(obj.contains_key("_meta"));
        assert_eq!(obj["_meta"]["instrument"].as_str().unwrap(), "test");
    }

    #[test]
    fn test_serialize_state_specific_keys() {
        let bar = make_bar(Some(100.0), Some(3.0), 150.0);
        let p = build_pipeline_with_bars(Freq::F1, vec![bar]);
        p.state
            .set("extra".into(), StateValue::F64(42.0), "test".into());

        // Only export the specified key
        let json = p.serialize_state(&["extra".to_string()]);
        let obj = json.as_object().unwrap();
        assert!(obj.contains_key("extra"));
        assert!(!obj.contains_key("bars:1m"));
    }

    // ── C1 regression test: after incremental bar feed, compute_six_core no longer returns None ──

    use crate::types::tick::TickData;
    use chrono::TimeZone;

    fn ts_utc(hour: u32, min: u32, sec: u32) -> i64 {
        Utc.with_ymd_and_hms(2026, 7, 22, hour, min, sec)
            .unwrap()
            .timestamp_millis()
    }

    fn make_tick(ts_ms: i64, price: f64, vol: f64, oi: f64, delta: f64) -> TickData {
        TickData {
            instrument: "rb9999".into(),
            trading_day: "20260722".into(),
            exchange_id: "SHFE".into(),
            exchange_inst_id: "rb9999".into(),
            last_price: price,
            pre_settlement_price: 0.0,
            pre_close_price: 0.0,
            pre_open_interest: 0.0,
            open_price: 0.0,
            highest_price: 0.0,
            lowest_price: 0.0,
            volume: vol,
            turnover: 0.0,
            open_interest: oi,
            close_price: 0.0,
            settlement_price: 0.0,
            upper_limit_price: 0.0,
            lower_limit_price: 0.0,
            pre_delta: 0.0,
            curr_delta: 0.0,
            update_time: String::new(),
            update_millisec: 0,
            bid_price1: 0.0,
            bid_volume1: 0,
            ask_price1: 0.0,
            ask_volume1: 0,
            bid_price2: 0.0,
            bid_volume2: 0,
            ask_price2: 0.0,
            ask_volume2: 0,
            bid_price3: 0.0,
            bid_volume3: 0,
            ask_price3: 0.0,
            ask_volume3: 0,
            bid_price4: 0.0,
            bid_volume4: 0,
            ask_price4: 0.0,
            ask_volume4: 0,
            bid_price5: 0.0,
            bid_volume5: 0,
            ask_price5: 0.0,
            ask_volume5: 0,
            average_price: 0.0,
            action_day: String::new(),
            trade_type: Some(delta),
            cum_volume: None,
            cum_position: None,
            timestamp_ms: ts_ms,
        }
    }

    #[test]
    fn test_compute_six_core_after_incremental_feed() {
        // Build 1m pipeline
        let config = crate::config::PipelineConfig {
            name: "c1_test".into(),
            version: "1.0".into(),
            bar_gen: crate::config::BarGenConfig {
                modes: vec!["time".into()],
                time_freqs: vec!["1m".into()],
            },
            data_source: crate::config::DataSourceSpec {
                type_name: "none".into(),
                config: serde_json::json!({}),
            },
            nodes: vec![crate::config::NodeSpec {
                id: "n1".into(),
                type_name: "dummy".into(),
                config: serde_json::json!({}),
                input_keys: vec![],
                output_keys: vec![],
            }],
        };
        let mut p = Pipeline::from_config(config).unwrap();

        // === Bar 09:00: oi=100, delta=1, vol=100 ===
        // tick 09:00:00 — create partial (vol increment=0, delta=0)
        p.feed_tick_direct(&make_tick(ts_utc(9, 0, 0), 4000.0, 0.0, 100.0, 0.0))
            .unwrap();
        assert!(p.compute_six_core(Freq::F1, 1).is_none());

        // tick 09:00:30 — update same bar (vol increment=100, delta=1)
        p.feed_tick_direct(&make_tick(ts_utc(9, 0, 30), 4010.0, 100.0, 100.0, 1.0))
            .unwrap();
        assert!(p.compute_six_core(Freq::F1, 1).is_none());

        // === Bar 09:01: oi=110, delta=1, vol=100 ===
        // tick 09:01:00 — close 09:00 bar, create 09:01 partial (delta=0 first tick)
        p.feed_tick_direct(&make_tick(ts_utc(9, 1, 0), 4020.0, 100.0, 110.0, 0.0))
            .unwrap();
        // Only 1 bar, t=1 still needs 2 bars
        assert!(p.compute_six_core(Freq::F1, 1).is_none());

        // tick 09:01:30 — update same bar (vol increment=100, delta=1)
        p.feed_tick_direct(&make_tick(ts_utc(9, 1, 30), 4030.0, 200.0, 110.0, 1.0))
            .unwrap();
        assert!(p.compute_six_core(Freq::F1, 1).is_none());

        // tick 09:02:00 — close 09:01 bar, now have 2 bars
        p.feed_tick_direct(&make_tick(ts_utc(9, 2, 0), 4040.0, 200.0, 110.0, 0.0))
            .unwrap();

        // compute_six_core(t=1) should return Some (no longer always None)
        let metrics = p.compute_six_core(Freq::F1, 1).unwrap();

        // Manual verification:
        //   bar[0] (newest, 09:01): oi=110, delta=1, vol=100
        //   bar[1] (oldest, 09:00): oi=100
        //   仓差 = 110 - 100 = 10
        //   主动买卖差 = bar[0].delta = 1
        //   总成交量 = bar[0].vol = 100
        assert!((metrics.oi_delta - 10.0).abs() < 1e-9);
        assert!((metrics.active_trade_diff - 1.0).abs() < 1e-9);
        assert!((metrics.total_volume - 100.0).abs() < 1e-9);
        assert!((metrics.long_open - 55.5).abs() < 1e-9);
        assert!((metrics.short_open - 54.5).abs() < 1e-9);
        assert!((metrics.long_close - 44.5).abs() < 1e-9);
        assert!((metrics.short_close - 45.5).abs() < 1e-9);
        assert!((metrics.net_long - 11.0).abs() < 1e-9);
        assert!((metrics.net_short - 9.0).abs() < 1e-9);
    }

    // ── RiskMonitor integration tests ──

    use crate::risk::{RiskFill, RiskPosition, RiskAction, RiskAlert, RiskConfig};
    use crate::types::signal::{Signal, SignalAction};

    /// Mock that rejects every order.
    struct RejectAllMonitor;
    impl RiskMonitor for RejectAllMonitor {
        fn init(&mut self, _config: &RiskConfig) -> Result<()> {
            Ok(())
        }
        fn check_order(&self, _order: &RiskOrderRequest, _state: &StateStore) -> Result<OrderDecision> {
            Ok(OrderDecision::Reject("blocked by test monitor".into()))
        }
        fn check_position(&self, _position: &RiskPosition, _state: &StateStore) -> Result<RiskAction> {
            Ok(RiskAction::None)
        }
        fn on_fill(&mut self, _fill: &RiskFill, _state: &StateStore) {}
        fn on_calculate(&mut self, _state: &StateStore) -> Result<Vec<RiskAlert>> {
            Ok(vec![])
        }
        fn enabled(&self) -> bool {
            true
        }
    }

    /// Mock that caps volume at a max value.
    struct CapVolumeMonitor {
        max_vol: f64,
    }
    impl RiskMonitor for CapVolumeMonitor {
        fn init(&mut self, _config: &RiskConfig) -> Result<()> {
            Ok(())
        }
        fn check_order(&self, order: &RiskOrderRequest, _state: &StateStore) -> Result<OrderDecision> {
            if order.volume > self.max_vol {
                Ok(OrderDecision::Reduce(self.max_vol))
            } else {
                Ok(OrderDecision::Allow)
            }
        }
        fn check_position(&self, _position: &RiskPosition, _state: &StateStore) -> Result<RiskAction> {
            Ok(RiskAction::None)
        }
        fn on_fill(&mut self, _fill: &RiskFill, _state: &StateStore) {}
        fn on_calculate(&mut self, _state: &StateStore) -> Result<Vec<RiskAlert>> {
            Ok(vec![])
        }
        fn enabled(&self) -> bool {
            true
        }
    }

    fn make_test_signal(instrument: &str, action: SignalAction, size: f64) -> Signal {
        Signal {
            timestamp: Utc::now(),
            instrument: instrument.into(),
            freq: Freq::F1,
            action,
            entry: Some(100.0),
            stop_loss: None,
            take_profit: None,
            size: Some(size),
            source: "test_node".into(),
            confidence: 1.0,
            metadata: std::collections::HashMap::new(),
            disclaimer: None,
        }
    }

    #[test]
    fn test_risk_monitor_rejects_order() {
        let bar = make_bar(Some(100.0), Some(1.0), 100.0);
        let config = crate::config::PipelineConfig {
            name: "reject_test".into(),
            version: "1.0".into(),
            bar_gen: crate::config::BarGenConfig {
                modes: vec![],
                time_freqs: vec![],
            },
            data_source: crate::config::DataSourceSpec {
                type_name: "none".into(),
                config: serde_json::json!({}),
            },
            nodes: vec![crate::config::NodeSpec {
                id: "n1".into(),
                type_name: "dummy".into(),
                config: serde_json::json!({}),
                input_keys: vec![],
                output_keys: vec![],
            }],
        };
        let mut p = Pipeline::from_config(config).unwrap();
        p.set_risk_monitor(Box::new(RejectAllMonitor));

        let signals = vec![make_test_signal("rb9999", SignalAction::Long, 10.0)];
        let filtered = p.filter_signals(signals, &bar);
        assert!(
            filtered.is_empty(),
            "RejectAllMonitor should block all orders"
        );
    }

    #[test]
    fn test_risk_monitor_none_passthrough() {
        let bar = make_bar(Some(100.0), Some(1.0), 100.0);
        let config = crate::config::PipelineConfig {
            name: "none_test".into(),
            version: "1.0".into(),
            bar_gen: crate::config::BarGenConfig {
                modes: vec![],
                time_freqs: vec![],
            },
            data_source: crate::config::DataSourceSpec {
                type_name: "none".into(),
                config: serde_json::json!({}),
            },
            nodes: vec![crate::config::NodeSpec {
                id: "n1".into(),
                type_name: "dummy".into(),
                config: serde_json::json!({}),
                input_keys: vec![],
                output_keys: vec![],
            }],
        };
        let p = Pipeline::from_config(config).unwrap();
        // No risk monitor set — all signals pass through

        let signals = vec![make_test_signal("rb9999", SignalAction::Short, 5.0)];
        let filtered = p.filter_signals(signals, &bar);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].size, Some(5.0));
    }

    #[test]
    fn test_risk_monitor_reduce_volume() {
        let bar = make_bar(Some(100.0), Some(1.0), 100.0);
        let config = crate::config::PipelineConfig {
            name: "reduce_test".into(),
            version: "1.0".into(),
            bar_gen: crate::config::BarGenConfig {
                modes: vec![],
                time_freqs: vec![],
            },
            data_source: crate::config::DataSourceSpec {
                type_name: "none".into(),
                config: serde_json::json!({}),
            },
            nodes: vec![crate::config::NodeSpec {
                id: "n1".into(),
                type_name: "dummy".into(),
                config: serde_json::json!({}),
                input_keys: vec![],
                output_keys: vec![],
            }],
        };
        let mut p = Pipeline::from_config(config).unwrap();
        p.set_risk_monitor(Box::new(CapVolumeMonitor { max_vol: 3.0 }));

        let signals = vec![make_test_signal("rb9999", SignalAction::Long, 10.0)];
        let filtered = p.filter_signals(signals, &bar);
        assert_eq!(filtered.len(), 1);
        assert_eq!(
            filtered[0].size,
            Some(3.0),
            "volume should be reduced to max_vol"
        );
    }

    #[test]
    fn test_risk_monitor_allows_small_order() {
        let bar = make_bar(Some(100.0), Some(1.0), 100.0);
        let config = crate::config::PipelineConfig {
            name: "allow_test".into(),
            version: "1.0".into(),
            bar_gen: crate::config::BarGenConfig {
                modes: vec![],
                time_freqs: vec![],
            },
            data_source: crate::config::DataSourceSpec {
                type_name: "none".into(),
                config: serde_json::json!({}),
            },
            nodes: vec![crate::config::NodeSpec {
                id: "n1".into(),
                type_name: "dummy".into(),
                config: serde_json::json!({}),
                input_keys: vec![],
                output_keys: vec![],
            }],
        };
        let mut p = Pipeline::from_config(config).unwrap();
        p.set_risk_monitor(Box::new(CapVolumeMonitor { max_vol: 10.0 }));

        let signals = vec![make_test_signal("rb9999", SignalAction::Short, 3.0)];
        let filtered = p.filter_signals(signals, &bar);
        assert_eq!(filtered.len(), 1);
        assert_eq!(
            filtered[0].size,
            Some(3.0),
            "small order should pass through unchanged"
        );
    }

    // ── R6.16 hot-reload tests ──

    use crate::types::state::StateKey;

    /// Mock node that always emits a Long signal.
    struct LongSignalNode {
        id: String,
        count: u64,
    }
    impl ComputeNode for LongSignalNode {
        fn id(&self) -> NodeId {
            self.id.clone()
        }
        fn name(&self) -> &'static str {
            "long_signal"
        }
        fn input_keys(&self) -> Vec<StateKey> {
            vec![]
        }
        fn output_keys(&self) -> Vec<StateKey> {
            vec!["signals:test".into()]
        }
        fn on_init(&mut self, _config: &NodeConfig, _state: &StateStore) -> Result<()> {
            Ok(())
        }
        fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> {
            self.count += 1;
            Ok(())
        }
        fn on_calculate(&mut self, _state: &StateStore) -> Result<Vec<Signal>> {
            Ok(vec![make_test_signal("rb9999", SignalAction::Long, 2.0)])
        }
    }

    /// Mock node that always emits a Short signal.
    struct ShortSignalNode {
        id: String,
        count: u64,
    }
    impl ComputeNode for ShortSignalNode {
        fn id(&self) -> NodeId {
            self.id.clone()
        }
        fn name(&self) -> &'static str {
            "short_signal"
        }
        fn input_keys(&self) -> Vec<StateKey> {
            vec![]
        }
        fn output_keys(&self) -> Vec<StateKey> {
            vec!["signals:test".into()]
        }
        fn on_init(&mut self, _config: &NodeConfig, _state: &StateStore) -> Result<()> {
            Ok(())
        }
        fn on_bar(&mut self, _bar: &RawBar, _period: Freq, _state: &StateStore) -> Result<()> {
            self.count += 1;
            Ok(())
        }
        fn on_calculate(&mut self, _state: &StateStore) -> Result<Vec<Signal>> {
            Ok(vec![make_test_signal("rb9999", SignalAction::Short, 2.0)])
        }
    }

    #[test]
    fn test_swap_node_continues_output() {
        let config = crate::config::PipelineConfig {
            name: "swap_test".into(),
            version: "1.0".into(),
            bar_gen: crate::config::BarGenConfig {
                modes: vec!["time".into()],
                time_freqs: vec!["1m".into()],
            },
            data_source: crate::config::DataSourceSpec {
                type_name: "none".into(),
                config: serde_json::json!({}),
            },
            nodes: vec![crate::config::NodeSpec {
                id: "signal_node".into(),
                type_name: "long_signal".into(),
                config: serde_json::json!({}),
                input_keys: vec![],
                output_keys: vec!["signals:test".into()],
            }],
        };
        let mut p = Pipeline::from_config(config).unwrap();

        // Register node types in pipeline factory
        p.register_node_type(
            "long_signal",
            Box::new(|_: &NodeConfig| {
                Ok(Box::new(LongSignalNode {
                    id: "signal_node".into(),
                    count: 0,
                }))
            }),
        );
        p.register_node_type(
            "short_signal",
            Box::new(|_: &NodeConfig| {
                Ok(Box::new(ShortSignalNode {
                    id: "signal_node".into(),
                    count: 0,
                }))
            }),
        );

        // Create initial node and derive edges
        let initial_node: Box<dyn ComputeNode> = Box::new(LongSignalNode {
            id: "signal_node".into(),
            count: 0,
        });
        p.add_node(initial_node);
        p.derive_edges().unwrap();

        // Feed ticks to produce first bar with Long signal
        p.feed_tick_direct(&make_tick(ts_utc(9, 0, 0), 4000.0, 0.0, 100.0, 0.0))
            .unwrap();
        p.feed_tick_direct(&make_tick(ts_utc(9, 0, 30), 4010.0, 100.0, 100.0, 1.0))
            .unwrap();
        let result_before = p
            .feed_tick_direct(&make_tick(ts_utc(9, 1, 0), 4020.0, 100.0, 110.0, 0.0))
            .unwrap();

        assert!(
            !result_before.signals.is_empty(),
            "should produce signals before swap"
        );
        assert_eq!(
            result_before.signals[0].action,
            SignalAction::Long,
            "original node produces Long signals"
        );

        // Swap node: LongSignalNode → ShortSignalNode
        let new_config = NodeConfig {
            type_name: "short_signal".into(),
            params: HashMap::new(),
        };
        p.swap_node(&"signal_node".into(), new_config).unwrap();

        // Feed ticks after swap — should now produce Short signals
        p.feed_tick_direct(&make_tick(ts_utc(9, 1, 30), 4030.0, 200.0, 110.0, 1.0))
            .unwrap();
        let result_after = p
            .feed_tick_direct(&make_tick(ts_utc(9, 2, 0), 4040.0, 200.0, 120.0, 0.0))
            .unwrap();

        assert!(
            !result_after.signals.is_empty(),
            "should continue producing signals after swap"
        );
        assert_eq!(
            result_after.signals[0].action,
            SignalAction::Short,
            "after swap, node should produce Short signals"
        );
    }

    #[test]
    fn test_swap_node_unknown_node_id() {
        let config = crate::config::PipelineConfig {
            name: "swap_err_test".into(),
            version: "1.0".into(),
            bar_gen: crate::config::BarGenConfig {
                modes: vec![],
                time_freqs: vec![],
            },
            data_source: crate::config::DataSourceSpec {
                type_name: "none".into(),
                config: serde_json::json!({}),
            },
            nodes: vec![crate::config::NodeSpec {
                id: "n1".into(),
                type_name: "dummy".into(),
                config: serde_json::json!({}),
                input_keys: vec![],
                output_keys: vec![],
            }],
        };
        let p = Pipeline::from_config(config).unwrap();

        let new_config = NodeConfig {
            type_name: "long_signal".into(),
            params: HashMap::new(),
        };
        let result = p.swap_node(&"nonexistent".into(), new_config);
        assert!(result.is_err(), "swap_node should fail for unknown node id");
    }

    #[test]
    fn test_reload_config_preserves_state() {
        // Build initial pipeline with bar_gen
        let config_a = crate::config::PipelineConfig {
            name: "reload_test".into(),
            version: "1.0".into(),
            bar_gen: crate::config::BarGenConfig {
                modes: vec!["time".into()],
                time_freqs: vec!["1m".into()],
            },
            data_source: crate::config::DataSourceSpec {
                type_name: "none".into(),
                config: serde_json::json!({}),
            },
            nodes: vec![crate::config::NodeSpec {
                id: "signal_node".into(),
                type_name: "long_signal".into(),
                config: serde_json::json!({}),
                input_keys: vec![],
                output_keys: vec!["signals:test".into()],
            }],
        };
        let mut p = Pipeline::from_config(config_a).unwrap();

        // Register node types
        p.register_node_type(
            "long_signal",
            Box::new(|_: &NodeConfig| {
                Ok(Box::new(LongSignalNode {
                    id: "signal_node".into(),
                    count: 0,
                }))
            }),
        );

        let initial_node: Box<dyn ComputeNode> = Box::new(LongSignalNode {
            id: "signal_node".into(),
            count: 0,
        });
        p.add_node(initial_node);
        p.derive_edges().unwrap();

        // Feed ticks to populate state
        p.feed_tick_direct(&make_tick(ts_utc(9, 0, 0), 4000.0, 0.0, 100.0, 0.0))
            .unwrap();
        p.feed_tick_direct(&make_tick(ts_utc(9, 0, 30), 4010.0, 100.0, 100.0, 1.0))
            .unwrap();
        let before = p
            .feed_tick_direct(&make_tick(ts_utc(9, 1, 0), 4020.0, 100.0, 110.0, 0.0))
            .unwrap();
        assert!(
            !before.signals.is_empty(),
            "should have signals before reload"
        );

        // Verify state has bars
        let bars_before: Option<Arc<Vec<Arc<RawBar>>>> =
            p.state_store().get(&"bars:1m".to_string());
        assert!(
            bars_before.is_some(),
            "state should have bars before reload"
        );
        let bar_count_before = bars_before.unwrap().len();

        // Write temporary YAML config for reload (same structure as original)
        let tmp_path = std::env::temp_dir().join("taiji_reload_test.yaml");
        let yaml_content = r#"
name: "reload_test_v2"
version: "2.0"
bar_gen:
  modes: ["time"]
  time_freqs: ["1m"]
data_source:
  type: "none"
  config: {}
nodes:
  - id: "signal_node"
    type: "long_signal"
    config: {}
    input_keys: []
    output_keys: ["signals:test"]
"#;
        std::fs::write(&tmp_path, yaml_content).unwrap();

        // Reload config
        p.reload_config(&tmp_path).unwrap();

        // Verify state preserved
        let bars_after: Option<Arc<Vec<Arc<RawBar>>>> = p.state_store().get(&"bars:1m".to_string());
        assert!(bars_after.is_some(), "state should have bars after reload");
        let bar_count_after = bars_after.unwrap().len();
        assert_eq!(
            bar_count_before, bar_count_after,
            "bar count should be preserved after reload_config"
        );

        // Feed more ticks after reload — pipeline should still work
        p.feed_tick_direct(&make_tick(ts_utc(9, 1, 30), 4030.0, 200.0, 110.0, 1.0))
            .unwrap();
        let after = p
            .feed_tick_direct(&make_tick(ts_utc(9, 2, 0), 4040.0, 200.0, 120.0, 0.0))
            .unwrap();
        assert!(
            !after.signals.is_empty(),
            "should produce signals after reload_config"
        );

        // Cleanup
        let _ = std::fs::remove_file(&tmp_path);
    }
}
