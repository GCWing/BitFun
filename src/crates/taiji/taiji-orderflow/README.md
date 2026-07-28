# taiji-orderflow — Order Flow Analysis

VPIN (Volume-synchronized Probability of Informed Trading / toxicity) and OFI (Order Flow Imbalance) for futures markets. Built on Welford's online statistics algorithm for O(1)-space streaming operation. Both indicators are `ComputeNode`s.

## Usage

```rust
use taiji_orderflow::vpin::VpinNode;
use taiji_orderflow::ofi::OfiNode;
use taiji_engine::node::NodeConfig;

let mut vpin = VpinNode::new("vpin_node", 50); // 50-volume buckets
vpin.on_init(&NodeConfig::from_json(json!({"bucket_size": 50}))?, &mut state)?;
vpin.on_tick(&tick, &mut state)?;
let vpin_score = vpin.current_toxicity();

let mut ofi = OfiNode::new("ofi_node", 5); // 5-level depth
ofi.on_tick(&tick, &mut state)?;
let ofi_direction = ofi.current_direction();
```

```bash
cargo add taiji-orderflow
```

## Modules

| Module | Description |
|--------|-------------|
| `welford` | `WelfordStats` — online mean/variance/CDF in O(1) space |
| `vpin` | `VpinNode` — volume-bucket VPIN with CDF-based toxicity scoring |
| `ofi` | `OfiNode` — 5-level order flow imbalance with buy/sell direction |
