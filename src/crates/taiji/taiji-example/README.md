# taiji-example — Reference ComputeNode Implementations

Canonical `ComputeNode` examples using only generic technical indicators (MA, RSI, MACD) — zero proprietary 太极 formula. Serves as the template for writing custom strategy crates.

## Architecture Position

```
taiji-engine (ComputeNode trait)
  └── taiji-example (MaCross)
```

## Strategies

| Strategy | Description |
|----------|-------------|
| `MaCross` | Classic MA dual-moving-average golden-cross/dead-cross. `fast_period=5`, `slow_period=20`. |

## Quick Start — Writing a Custom Node

```rust
use taiji_engine::node::{ComputeNode, NodeConfig, NodeId};
use taiji_engine::store::StateStore;
use taiji_engine::types::bar::RawBar;

pub struct MyNode { id: NodeId, /* params */ }

impl ComputeNode for MyNode {
    fn id(&self) -> NodeId { self.id.clone() }
    fn name(&self) -> &str { "my_node" }
    fn input_keys(&self) -> Vec<String> { vec!["bars:1m".into()] }
    fn output_keys(&self) -> Vec<String> { vec!["my_node:signal".into()] }
    fn on_bar(&mut self, bar: Arc<RawBar>, freq: &Freq, state: &mut StateStore) -> Result<()> {
        // Your logic here — read bars, compute indicator, write to StateStore
        Ok(())
    }
}
```

## License

SPDX-License-Identifier: Apache-2.0 OR MIT
