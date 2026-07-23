# taiji-bar — Tick-to-KLine Aggregation Engine

`ComputeNode` implementation: aggregates tick data into OHLCV bars with time-bucket alignment and delta classification. Reference: czsc BarGenerator (Apache 2.0).

## Architecture Position

```
taiji-engine (ComputeNode trait)
  └── taiji-bar (BarNode)
```

## Core API

| Type | Description |
|------|-------------|
| `BarNode` | `ComputeNode` impl — tick→bar aggregation with GM/CTP delta modes |
| `PartialBar` | Internal — accumulates tick OHLCV in incomplete bar, flushes on boundary cross |

## Quick Start

```rust
use taiji_bar::BarNode;
use taiji_engine::node::{ComputeNode, NodeConfig};
use taiji_engine::store::StateStore;

let config = NodeConfig::new("bar_gen")
    .with("freqs", serde_json::json!(["1m", "5m"]));
let mut node = BarNode::new(&config);
let mut state = StateStore::new();
node.on_init(&config, &mut state)?;

// Feed ticks — bars auto-close on time boundary
node.on_tick(&tick_data, &mut state)?;
```

## License

MIT — 与 workspace 一致。
