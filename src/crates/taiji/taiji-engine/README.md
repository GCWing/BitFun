# taiji-engine — DAG-Based Trading Engine

Core compute pipeline crate. Tick ingestion → bar generation → DAG execution → signal output.

## Architecture

```
                    ┌─────────────────────────────────────────┐
                    │            PipelineConfig (YAML)         │
                    │   nodes[].input_keys / output_keys       │
                    └──────────────┬──────────────────────────┘
                                   │
  ┌────────────────────────────────┼────────────────────────────────┐
  │                                ▼                                │
  │  ┌──────────┐   ┌──────────────┐   ┌──────────────┐            │
  │  │DataSource│──▶│SchemaAdapter │──▶│TickValidator │            │
  │  │(18 srcs) │   │(field map)   │   │(seq/gap/stale)│           │
  │  └──────────┘   └──────────────┘   └──────┬───────┘            │
  │                                           │                     │
  │                                           ▼                     │
  │                                   ┌──────────────┐             │
  │                                   │ BarGenerator  │             │
  │                                   │(1m/5m/15m/1h)│             │
  │                                   └──────┬───────┘             │
  │                                          │                      │
  │                                          ▼                      │
  │                              ┌───────────────────┐             │
  │                              │   Pipeline DAG    │             │
  │                              │ (Kahn topo sort)  │             │
  │                              └────────┬──────────┘             │
  │                                       │                         │
  │                          ┌────────────┼────────────┐            │
  │                          ▼            ▼            ▼            │
  │                    ┌──────────┐ ┌──────────┐ ┌──────────┐      │
  │                    │ComputeNod│ │ComputeNod│ │ComputeNod│      │
  │                    │  e A     │ │  e B     │ │  e C     │      │
  │                    └────┬─────┘ └────┬─────┘ └────┬─────┘      │
  │                         │            │            │             │
  │                         ▼            ▼            ▼             │
  │                    ┌──────────────────────────────────┐        │
  │                    │          StateStore               │        │
  │                    │   (shared key-value, provenance)  │        │
  │                    └──────────────────────────────────┘        │
  │                                │                                │
  │                                ▼                                │
  │                    ┌──────────────────────┐                     │
  │                    │       Signal[]        │                    │
  │                    │ (action + confidence) │                    │
  │                    └──────────────────────┘                     │
  └─────────────────────────────────────────────────────────────────┘
```

**Data flow**: `RawTick` → `SchemaAdapter` → `TickData` → `TickValidator` → `BarGenerator` → `RawBar` → `Pipeline DAG` (topological layers) → `ComputeNode.on_bar()` → `ComputeNode.on_calculate()` → `Signal[]`.

## Module Index

| Module | Path | Description |
|--------|------|-------------|
| **node** | `node.rs` | `ComputeNode` trait — pluggable compute unit with 7 lifecycle hooks |
| **pipeline** | `pipeline/mod.rs` | `Pipeline` — main execution engine; also `bar_gen`, `reorg`, `status` |
| **dag** | `dag.rs` | `Dag` — Kahn topological sort with cycle detection |
| **source** | `source/` | `DataSource` trait + `SchemaAdapter` + `TickValidator` + `DataSourceManager` |
| **store** | `store.rs` | `StateStore` — typed key-value store with provenance tracking |
| **signal** | `signal.rs` | `Signal` type + `SignalRegistry` (global descriptor registry) |
| **risk** | `risk.rs` | `RiskMonitor` trait — pluggable risk control (order/position/alerts) |
| **factory** | `factory.rs` | `NodeFactory` — constructor registry for `type_name` → `ComputeNode` |
| **config** | `config.rs` | `PipelineConfig` + `BarGenConfig` + `NodeSpec` — YAML deserialization |
| **error** | `error.rs` | `TaijiError` enum + `Result<T>` alias |
| **log** | `log.rs` | `Logger` with Off/Simple/Tracing modes |
| **types** | `types/` | Shared types: `TickData`, `RawBar`/`Freq`, `Signal`, `StateValue`, etc. |
| **state** | `types/state.rs` | `StateValue` enum (13 variants) + `FromStateValue` trait + `SixCoreMetrics` |

## Dependencies

- `serde` / `serde_json` — configuration and state serialization
- `chrono` — UTC timestamps for bars and ticks
- `thiserror` — error derive macros
- `parking_lot` — fast synchronization primitives
- `tracing` — structured logging
- `dashmap` — concurrent hash maps

## Quick Start

### 1. Define a PipelineConfig (YAML or programmatic)

```rust
use taiji_engine::config::{PipelineConfig, BarGenConfig, DataSourceSpec, NodeSpec};

let config = PipelineConfig {
    name: "my_strategy".into(),
    version: "1.0".into(),
    bar_gen: BarGenConfig {
        modes: vec!["time".into()],
        time_freqs: vec!["1m".into(), "5m".into()],
    },
    data_source: DataSourceSpec {
        type_name: "csv".into(),
        config: serde_json::json!({"path": "data/rb9999_2026.csv"}),
    },
    nodes: vec![
        NodeSpec {
            id: "ma_cross".into(),
            type_name: "ma_cross".into(),
            config: serde_json::json!({"fast": 5, "slow": 20}),
            input_keys: vec!["bars:1m".into()],
            output_keys: vec!["ma_cross:signal".into()],
        },
    ],
};
```

### 2. Register ComputeNode and create Pipeline

```rust
use taiji_engine::pipeline::Pipeline;
use taiji_engine::factory::NodeFactory;

let mut pipeline = Pipeline::from_config(config)?;

// Register your node constructor
pipeline.node_factory.register("ma_cross", Box::new(|cfg| {
    Ok(Box::new(MyMaCrossNode::new(cfg)))
}));

// Add node instances (the Pipeline wires them via input_keys/output_keys)
pipeline.add_node(Box::new(MyMaCrossNode::new(&node_config)));
pipeline.derive_edges()?;
```

### 3. Feed ticks and collect signals

```rust
// Feed ticks one at a time — BarGenerator auto-aggregates to bars
let mut all_signals = Vec::new();
loop {
    let result = pipeline.feed_tick()?;
    if result.signals.is_empty() && result.closed_bars.is_empty() {
        break; // data source exhausted
    }
    all_signals.extend(result.signals);
}

// Or feed pre-parsed TickData directly (CSV replay / Tauri bridge)
pipeline.feed_tick_direct(&tick_data)?;
```

## Key Traits

### ComputeNode

The core pluggable unit. Implement 7 lifecycle hooks:

| Hook | When called | Default behavior |
|------|-------------|------------------|
| `on_init(config, state)` | Pipeline initialization | no-op |
| `on_tick(tick, state)` | Every tick (before bar gen) | no-op |
| `on_bar(bar, freq, state)` | Bar closes for subscribed freqs | **required** |
| `on_calculate(state)` | After `on_bar`, before next tick | returns `vec![]` |
| `on_session_begin(date, state)` | Trading day starts | no-op |
| `on_session_end(date, state)` | Trading day ends | no-op |
| `is_ready(state)` | Before each execution | returns `true` |

Nodes communicate **only through StateStore** — no direct node-to-node calls.

### DataSource

Pluggable data feed. 18+ sources routed per-instrument with automatic failover.

```rust
pub trait DataSource: Send + Sync {
    fn name(&self) -> &'static str;
    fn schema(&self) -> Vec<FieldDef>;
    fn connect(&mut self, config: &DataSourceConfig) -> Result<()>;
    fn subscribe(&mut self, instruments: &[&str]) -> Result<()>;
    fn next_raw(&mut self) -> Result<Option<RawTick>>;
    fn health_check(&self) -> SourceHealth;
}
```

### RiskMonitor

Pluggable risk control. Each monitor checks one dimension.

```rust
pub trait RiskMonitor: Send + Sync {
    fn check_order(&self, order: &OrderRequest, state: &StateStore) -> Result<OrderDecision>;
    fn check_position(&self, position: &Position, state: &StateStore) -> Result<RiskAction>;
    fn on_calculate(&mut self, state: &mut StateStore) -> Result<Vec<RiskAlert>>;
    // ...
}
```

## DAG Execution Model

1. `Pipeline::derive_edges()` reads every node's `input_keys` and `output_keys` to infer edges.
2. `Dag::sort()` runs Kahn's algorithm — returns topological layers or `Err(CycleDetected)`.
3. On each closed bar, nodes execute layer by layer; nodes within a layer run serially.
4. A node only runs when `is_ready()` returns `true`, which allows warm-up gating.

## Related Crates

| Crate | Relationship |
|-------|-------------|
| `taiji-data` | Data source implementations (CSV, CTP, etc.) |
| `taiji-strategy-*` | Closed-source strategy crates — implement `ComputeNode` |
| `taiji-executor` | Order execution and position management |
| `taiji-publisher` | Multi-platform video publishing |
| `taiji-content` | Video rendering pipeline (Phase 4) |

## License

SPDX-License-Identifier: Apache-2.0 OR MIT
